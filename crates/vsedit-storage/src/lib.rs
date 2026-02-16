//! Persistent key-value storage.
//!
//! Equivalent to VS Code's `vs/platform/storage/common/storage.ts`.
//! Uses SQLite for persistent storage, with scopes for global/workspace data.
//!
//! Sub-modules provide higher-level APIs:
//! - [`memento`] — typed key-value store for extension state
//! - [`window_state`] — JSON-based window layout persistence
//! - [`backup`] — crash-recovery backups for dirty files
//! - [`secret`] — secret storage with OS keychain integration
//! - [`recent`] — recently opened files and workspaces

use std::fmt;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

pub mod backup;
pub mod memento;
pub mod recent;
pub mod secret;
pub mod window_state;

/// Storage scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageScope {
    /// Global storage (shared across all workspaces).
    Global,
    /// Profile-specific storage.
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

    /// Get all key-value pairs as a HashMap.
    pub fn get_all(&self) -> HashMap<String, String> {
        self.entries().into_iter().collect()
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
            StorageScope::Global | StorageScope::Profile => self.global.get(key),
            StorageScope::Workspace => self.workspace.as_ref().and_then(|w| w.get(key)),
        }
    }

    pub fn set(&self, key: &str, value: &str, scope: StorageScope) -> StorageResult<()> {
        match scope {
            StorageScope::Global | StorageScope::Profile => self.global.set(key, value),
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
            StorageScope::Global | StorageScope::Profile => self.global.remove(key),
            StorageScope::Workspace => {
                if let Some(w) = &self.workspace {
                    w.remove(key)
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Get all key-value pairs for a scope.
    pub fn get_all(&self, scope: StorageScope) -> HashMap<String, String> {
        match scope {
            StorageScope::Global | StorageScope::Profile => self.global.get_all(),
            StorageScope::Workspace => self
                .workspace
                .as_ref()
                .map(|w| w.get_all())
                .unwrap_or_default(),
        }
    }

    /// Get a reference to the global storage.
    pub fn global(&self) -> &Storage {
        &self.global
    }

    /// Get a reference to the workspace storage, if any.
    pub fn workspace(&self) -> Option<&Storage> {
        self.workspace.as_ref()
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

    /// Delete a key from a scope.
    pub fn delete(&self, key: &str, scope: StorageScope) -> StorageResult<()> {
        self.remove(key, scope)
    }
}

// ---------------------------------------------------------------------------
// Display impls
// ---------------------------------------------------------------------------

impl std::fmt::Display for StorageScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageScope::Global => write!(f, "Global"),
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

/// Accumulated statistics for storage operations.
#[derive(Debug, Clone, PartialEq)]
pub struct StorageStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl StorageStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &StorageStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for StorageStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StorageStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StorageStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for storage.
#[derive(Debug, Clone)]
pub struct StorageValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl StorageValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for StorageValidator {
    fn default() -> Self {
        Self::new()
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
        assert_eq!(format!("{}", StorageScope::Global), "Global");
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

    #[test]
    fn get_all_returns_hashmap() {
        let store = Storage::in_memory().unwrap();
        store.set("a", "1").unwrap();
        store.set("b", "2").unwrap();
        let all = store.get_all();
        assert_eq!(all.len(), 2);
        assert_eq!(all.get("a"), Some(&"1".to_string()));
        assert_eq!(all.get("b"), Some(&"2".to_string()));
    }

    #[test]
    fn storage_service_global_scope() {
        let global = Storage::in_memory().unwrap();
        let svc = StorageService::new(global);
        svc.set("key", "val", StorageScope::Global).unwrap();
        assert_eq!(svc.get("key", StorageScope::Global), Some("val".to_string()));
        // Global and Profile share the same backing store
        assert_eq!(svc.get("key", StorageScope::Profile), Some("val".to_string()));
    }

    #[test]
    fn storage_service_get_all() {
        let global = Storage::in_memory().unwrap();
        let workspace = Storage::in_memory().unwrap();
        let svc = StorageService::new(global).with_workspace(workspace);
        svc.set("a", "1", StorageScope::Global).unwrap();
        svc.set("b", "2", StorageScope::Workspace).unwrap();
        let global_all = svc.get_all(StorageScope::Global);
        assert_eq!(global_all.len(), 1);
        assert_eq!(global_all.get("a"), Some(&"1".to_string()));
        let ws_all = svc.get_all(StorageScope::Workspace);
        assert_eq!(ws_all.len(), 1);
        assert_eq!(ws_all.get("b"), Some(&"2".to_string()));
    }

    #[test]
    fn storage_service_delete() {
        let global = Storage::in_memory().unwrap();
        let svc = StorageService::new(global);
        svc.set("key", "val", StorageScope::Global).unwrap();
        svc.delete("key", StorageScope::Global).unwrap();
        assert!(!svc.has("key", StorageScope::Global));
    }

    #[test]
    fn storage_service_accessors() {
        let global = Storage::in_memory().unwrap();
        let workspace = Storage::in_memory().unwrap();
        let svc = StorageService::new(global).with_workspace(workspace);
        assert!(svc.global().is_empty());
        assert!(svc.workspace().unwrap().is_empty());
    }

    #[test]
    fn storage_stats_new_defaults() {
        let stats = StorageStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn storage_stats_record_success() {
        let mut stats = StorageStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn storage_stats_record_failure() {
        let mut stats = StorageStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn storage_stats_reset() {
        let mut stats = StorageStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn storage_stats_merge() {
        let mut a = StorageStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = StorageStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn storage_stats_display() {
        let mut stats = StorageStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn storage_stats_default() {
        let stats = StorageStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn storage_validator_accepts_valid_name() {
        let v = StorageValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn storage_validator_rejects_empty() {
        let v = StorageValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn storage_validator_rejects_too_long() {
        let v = StorageValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn storage_validator_forbidden_prefix() {
        let v = StorageValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn storage_validator_allowed_chars() {
        let v = StorageValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn storage_validator_range() {
        let v = StorageValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn storage_sanitize_removes_control() {
        let result = StorageValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn storage_truncate_short_string() {
        assert_eq!(StorageValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn storage_truncate_long_string() {
        let result = StorageValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn storage_is_ascii_printable() {
        assert!(StorageValidator::is_ascii_printable("Hello World 123"));
        assert!(!StorageValidator::is_ascii_printable("Hello\x00World"));
    }
}
