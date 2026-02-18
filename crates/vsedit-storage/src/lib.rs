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

/// An in-memory key-value storage backend backed by a HashMap.
/// Useful for testing or ephemeral storage that doesn't need persistence.
pub struct StorageDatabase {
    data: HashMap<String, String>,
    pub version: u32,
}

impl StorageDatabase {
    /// Create a new empty in-memory database.
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            version: 1,
        }
    }

    /// Get a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(|s| s.as_str())
    }

    /// Set a value.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.data.insert(key.into(), value.into());
    }

    /// Remove a key, returning its previous value.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.data.remove(key)
    }

    /// Check if a key exists.
    pub fn has(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Get the number of stored items.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the database is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get all keys sorted alphabetically.
    pub fn keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.data.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        keys
    }

    /// Clear all data.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Get the current schema version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Export all data as a Vec of (key, value) pairs.
    pub fn export(&self) -> Vec<(String, String)> {
        let mut entries: Vec<(String, String)> = self
            .data
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }
}

impl Default for StorageDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StorageDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StorageDatabase(v{}, {} entries)",
            self.version,
            self.data.len()
        )
    }
}

/// A namespaced view into a StorageDatabase. Keys are automatically prefixed.
pub struct StorageNamespace<'a> {
    db: &'a mut StorageDatabase,
    prefix: String,
}

/// Create a namespaced accessor for a storage database.
pub fn storage_namespace<'a>(
    db: &'a mut StorageDatabase,
    namespace: &str,
) -> StorageNamespace<'a> {
    StorageNamespace {
        db,
        prefix: format!("{}.", namespace),
    }
}

impl<'a> StorageNamespace<'a> {
    /// Get a value within this namespace.
    pub fn get(&self, key: &str) -> Option<&str> {
        let full_key = format!("{}{}", self.prefix, key);
        self.db.get(&full_key)
    }

    /// Set a value within this namespace.
    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        let full_key = format!("{}{}", self.prefix, key);
        self.db.set(full_key, value);
    }

    /// Remove a key within this namespace.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        let full_key = format!("{}{}", self.prefix, key);
        self.db.remove(&full_key)
    }

    /// Check if a key exists within this namespace.
    pub fn has(&self, key: &str) -> bool {
        let full_key = format!("{}{}", self.prefix, key);
        self.db.has(&full_key)
    }

    /// Get all keys in this namespace (without the prefix).
    pub fn keys(&self) -> Vec<String> {
        self.db
            .keys()
            .iter()
            .filter(|k| k.starts_with(&self.prefix))
            .map(|k| k[self.prefix.len()..].to_string())
            .collect()
    }

    /// The prefix used by this namespace.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
}

/// A migration step that transforms data from one version to the next.
pub struct StorageMigration {
    pub from_version: u32,
    pub to_version: u32,
    pub description: String,
}

/// Apply migrations to a StorageDatabase to bring it up to `target_version`.
/// `key_renames` is a list of (version, old_key, new_key) tuples describing
/// key renames to apply at each version step.
pub fn storage_migrate(
    db: &mut StorageDatabase,
    target_version: u32,
    key_renames: &[(u32, &str, &str)],
) -> Vec<StorageMigration> {
    let mut applied = Vec::new();
    let mut current = db.version();

    while current < target_version {
        let next = current + 1;
        // Apply any key renames for this version step
        for (ver, old_key, new_key) in key_renames {
            if *ver == next {
                if let Some(val) = db.remove(old_key) {
                    db.set(new_key.to_string(), val);
                }
            }
        }
        applied.push(StorageMigration {
            from_version: current,
            to_version: next,
            description: format!("Migrated v{} -> v{}", current, next),
        });
        current = next;
    }
    db.version = target_version;
    applied
}

// ---------------------------------------------------------------------------
// StorageQuota
// ---------------------------------------------------------------------------

/// Tracks storage usage against a configured quota.
#[derive(Debug, Clone, PartialEq)]
pub struct StorageQuota {
    max_keys: usize,
    max_total_bytes: usize,
    current_keys: usize,
    current_bytes: usize,
}

impl StorageQuota {
    /// Create a quota with maximum key count and total byte limit.
    pub fn new(max_keys: usize, max_total_bytes: usize) -> Self {
        Self {
            max_keys,
            max_total_bytes,
            current_keys: 0,
            current_bytes: 0,
        }
    }

    /// Update usage counters from a [`StorageDatabase`].
    pub fn compute_usage(&mut self, db: &StorageDatabase) {
        self.current_keys = db.len();
        self.current_bytes = db.export().iter().map(|(k, v)| k.len() + v.len()).sum();
    }

    /// Returns true if adding `key_bytes` + `value_bytes` would exceed quota.
    pub fn would_exceed(&self, key_bytes: usize, value_bytes: usize) -> bool {
        self.current_keys + 1 > self.max_keys
            || self.current_bytes + key_bytes + value_bytes > self.max_total_bytes
    }

    /// Percentage of key quota used (0.0–100.0).
    pub fn key_usage_percent(&self) -> f64 {
        if self.max_keys == 0 {
            return 100.0;
        }
        (self.current_keys as f64 / self.max_keys as f64) * 100.0
    }

    /// Percentage of byte quota used (0.0–100.0).
    pub fn byte_usage_percent(&self) -> f64 {
        if self.max_total_bytes == 0 {
            return 100.0;
        }
        (self.current_bytes as f64 / self.max_total_bytes as f64) * 100.0
    }

    /// Remaining key capacity.
    pub fn remaining_keys(&self) -> usize {
        self.max_keys.saturating_sub(self.current_keys)
    }

    /// Remaining byte capacity.
    pub fn remaining_bytes(&self) -> usize {
        self.max_total_bytes.saturating_sub(self.current_bytes)
    }

    pub fn current_keys(&self) -> usize {
        self.current_keys
    }

    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }
}

impl fmt::Display for StorageQuota {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Quota: {}/{} keys, {}/{} bytes",
            self.current_keys, self.max_keys, self.current_bytes, self.max_total_bytes,
        )
    }
}

// ---------------------------------------------------------------------------
// StorageExporter
// ---------------------------------------------------------------------------

/// Serialize/deserialize a [`StorageDatabase`] to/from a HashMap.
pub struct StorageExporter;

impl StorageExporter {
    /// Export the database into a HashMap.
    pub fn to_map(db: &StorageDatabase) -> HashMap<String, String> {
        db.export().into_iter().collect()
    }

    /// Import from a HashMap, replacing all existing data.
    pub fn from_map(map: &HashMap<String, String>) -> StorageDatabase {
        let mut db = StorageDatabase::new();
        for (k, v) in map {
            db.set(k.clone(), v.clone());
        }
        db
    }

    /// Merge a map into an existing database. Existing keys are overwritten.
    pub fn merge_map(db: &mut StorageDatabase, map: &HashMap<String, String>) {
        for (k, v) in map {
            db.set(k.clone(), v.clone());
        }
    }

    /// Export only keys matching a prefix.
    pub fn export_prefix(db: &StorageDatabase, prefix: &str) -> HashMap<String, String> {
        db.export()
            .into_iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// StorageChangeLog
// ---------------------------------------------------------------------------

/// A recorded change in storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageChange {
    pub key: String,
    pub kind: StorageChangeKind,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub sequence: u64,
}

/// The type of change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageChangeKind {
    Set,
    Remove,
    Clear,
}

impl fmt::Display for StorageChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageChangeKind::Set => write!(f, "SET"),
            StorageChangeKind::Remove => write!(f, "REMOVE"),
            StorageChangeKind::Clear => write!(f, "CLEAR"),
        }
    }
}

/// Tracks changes made to a storage database.
pub struct StorageChangeLog {
    changes: Vec<StorageChange>,
    next_seq: u64,
}

impl StorageChangeLog {
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
            next_seq: 1,
        }
    }

    /// Record a SET operation.
    pub fn record_set(&mut self, key: &str, old_value: Option<&str>, new_value: &str) {
        self.changes.push(StorageChange {
            key: key.to_string(),
            kind: StorageChangeKind::Set,
            old_value: old_value.map(|s| s.to_string()),
            new_value: Some(new_value.to_string()),
            sequence: self.next_seq,
        });
        self.next_seq += 1;
    }

    /// Record a REMOVE operation.
    pub fn record_remove(&mut self, key: &str, old_value: Option<&str>) {
        self.changes.push(StorageChange {
            key: key.to_string(),
            kind: StorageChangeKind::Remove,
            old_value: old_value.map(|s| s.to_string()),
            new_value: None,
            sequence: self.next_seq,
        });
        self.next_seq += 1;
    }

    /// Record a CLEAR operation.
    pub fn record_clear(&mut self, keys: &[String]) {
        for key in keys {
            self.changes.push(StorageChange {
                key: key.clone(),
                kind: StorageChangeKind::Clear,
                old_value: None,
                new_value: None,
                sequence: self.next_seq,
            });
            self.next_seq += 1;
        }
    }

    /// All changes since the log was created.
    pub fn all_changes(&self) -> &[StorageChange] {
        &self.changes
    }

    /// Changes for a specific key.
    pub fn changes_for_key(&self, key: &str) -> Vec<&StorageChange> {
        self.changes.iter().filter(|c| c.key == key).collect()
    }

    /// Number of recorded changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Whether no changes have been recorded.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Clear all recorded changes.
    pub fn clear(&mut self) {
        self.changes.clear();
    }

    /// Get all unique keys that were changed.
    pub fn changed_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.changes.iter().map(|c| c.key.as_str()).collect();
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    /// Changes since a given sequence number.
    pub fn changes_since(&self, seq: u64) -> Vec<&StorageChange> {
        self.changes.iter().filter(|c| c.sequence > seq).collect()
    }
}

impl Default for StorageChangeLog {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Storage utility functions
// ---------------------------------------------------------------------------

/// Returns `true` if the storage contains any keys that start with `prefix`.
pub fn has_prefix(store: &Storage, prefix: &str) -> bool {
    store.keys().iter().any(|k| k.starts_with(prefix))
}

/// Returns the total byte-length of all values stored in the storage.
pub fn total_value_bytes(store: &Storage) -> usize {
    store
        .get_all()
        .values()
        .map(|v| v.len())
        .sum()
}

/// Returns keys that have duplicate values in the storage.
pub fn keys_with_duplicate_values(store: &Storage) -> Vec<String> {
    let all = store.get_all();
    let mut value_counts: HashMap<&str, usize> = HashMap::new();
    for v in all.values() {
        *value_counts.entry(v.as_str()).or_insert(0) += 1;
    }
    let mut dupes: Vec<String> = all
        .iter()
        .filter(|(_, v)| value_counts.get(v.as_str()).copied().unwrap_or(0) > 1)
        .map(|(k, _)| k.clone())
        .collect();
    dupes.sort();
    dupes
}

/// Copies all entries from `src` to `dst`, overwriting existing keys.
pub fn copy_all(src: &Storage, dst: &Storage) -> StorageResult<usize> {
    let entries = src.get_all();
    for (k, v) in &entries {
        dst.set(k, v)?;
    }
    Ok(entries.len())
}

/// Returns the keys whose values are valid integers.
pub fn integer_keys(store: &Storage) -> Vec<String> {
    store
        .get_all()
        .into_iter()
        .filter(|(_, v)| v.parse::<i64>().is_ok())
        .map(|(k, _)| k)
        .collect()
}

/// Returns the keys whose values are valid booleans ("true" / "false").
pub fn boolean_keys(store: &Storage) -> Vec<String> {
    store
        .get_all()
        .into_iter()
        .filter(|(_, v)| v == "true" || v == "false")
        .map(|(k, _)| k)
        .collect()
}

// ---------------------------------------------------------------------------
// Namespace helpers
// ---------------------------------------------------------------------------

/// A namespaced view over a [`Storage`], prefixing all keys with a namespace.
pub struct NamespacedStorage<'a> {
    storage: &'a Storage,
    prefix: String,
}

impl<'a> NamespacedStorage<'a> {
    pub fn new(storage: &'a Storage, namespace: &str) -> Self {
        Self {
            storage,
            prefix: format!("{namespace}."),
        }
    }

    fn prefixed_key(&self, key: &str) -> String {
        format!("{}{}", self.prefix, key)
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.storage.get(&self.prefixed_key(key))
    }

    pub fn set(&self, key: &str, value: &str) -> StorageResult<()> {
        self.storage.set(&self.prefixed_key(key), value)
    }

    pub fn remove(&self, key: &str) -> StorageResult<()> {
        self.storage.remove(&self.prefixed_key(key))
    }

    pub fn has(&self, key: &str) -> bool {
        self.storage.has(&self.prefixed_key(key))
    }

    pub fn keys(&self) -> Vec<String> {
        self.storage
            .keys()
            .into_iter()
            .filter_map(|k| k.strip_prefix(&self.prefix).map(String::from))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.keys().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entries(&self) -> Vec<(String, String)> {
        self.storage
            .entries()
            .into_iter()
            .filter_map(|(k, v)| {
                k.strip_prefix(&self.prefix)
                    .map(|stripped| (stripped.to_string(), v))
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Diff / merge helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageDiff {
    LeftOnly { key: String, value: String },
    RightOnly { key: String, value: String },
    Changed { key: String, left: String, right: String },
}

/// Compute the differences between two stores.
pub fn diff_stores(left: &Storage, right: &Storage) -> Vec<StorageDiff> {
    let l = left.get_all();
    let r = right.get_all();
    let mut diffs = Vec::new();
    for (k, lv) in &l {
        match r.get(k.as_str()) {
            Some(rv) if *rv != *lv => diffs.push(StorageDiff::Changed {
                key: k.clone(), left: lv.clone(), right: rv.clone(),
            }),
            None => diffs.push(StorageDiff::LeftOnly { key: k.clone(), value: lv.clone() }),
            _ => {}
        }
    }
    for (k, rv) in &r {
        if !l.contains_key(k.as_str()) {
            diffs.push(StorageDiff::RightOnly { key: k.clone(), value: rv.clone() });
        }
    }
    diffs.sort_by(|a, b| {
        let ka = match a { StorageDiff::LeftOnly { key, .. } | StorageDiff::RightOnly { key, .. } | StorageDiff::Changed { key, .. } => key };
        let kb = match b { StorageDiff::LeftOnly { key, .. } | StorageDiff::RightOnly { key, .. } | StorageDiff::Changed { key, .. } => key };
        ka.cmp(kb)
    });
    diffs
}

/// Merge entries from `src` into `dst`, only setting keys that do not already exist.
pub fn merge_missing(src: &Storage, dst: &Storage) -> StorageResult<usize> {
    let mut count = 0;
    for (k, v) in src.entries() {
        if !dst.has(&k) {
            dst.set(&k, &v)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Returns keys whose values exceed the given byte length.
pub fn keys_exceeding_length(store: &Storage, max_len: usize) -> Vec<String> {
    store.get_all().into_iter().filter(|(_, v)| v.len() > max_len).map(|(k, _)| k).collect()
}

// -- StorageMigrator for schema upgrades -------------------------------------

/// Tracks schema version and applies migrations.
pub struct StorageMigrator {
    current_version: u32,
    migrations: Vec<(u32, String)>,
}

impl StorageMigrator {
    pub fn new() -> Self {
        Self {
            current_version: 0,
            migrations: Vec::new(),
        }
    }

    /// Register a migration for a target version.
    pub fn add_migration(&mut self, version: u32, sql: impl Into<String>) {
        self.migrations.push((version, sql.into()));
        self.migrations.sort_by_key(|(v, _)| *v);
    }

    /// Get the current schema version.
    pub fn current_version(&self) -> u32 {
        self.current_version
    }

    /// Apply pending migrations. Returns the number of migrations applied.
    pub fn apply(&mut self, store: &Storage) -> StorageResult<u32> {
        let pending: Vec<_> = self
            .migrations
            .iter()
            .filter(|(v, _)| *v > self.current_version)
            .cloned()
            .collect();
        let count = pending.len() as u32;
        for (version, _sql) in &pending {
            // In a real implementation we'd execute the SQL
            self.current_version = *version;
        }
        // Store the version
        store.set("__schema_version", &self.current_version.to_string())?;
        Ok(count)
    }

    /// Load current version from storage.
    pub fn load_version(&mut self, store: &Storage) {
        if let Some(v) = store.get("__schema_version") {
            self.current_version = v.parse().unwrap_or(0);
        }
    }

    /// Number of pending migrations.
    pub fn pending_count(&self) -> usize {
        self.migrations
            .iter()
            .filter(|(v, _)| *v > self.current_version)
            .count()
    }
}

impl Default for StorageMigrator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StorageMigrator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Migrator(v{}, {} pending)",
            self.current_version,
            self.pending_count()
        )
    }
}

// -- StorageCompaction -------------------------------------------------------

/// Statistics about storage compaction.
#[derive(Debug, Clone)]
pub struct CompactionStats {
    pub keys_before: usize,
    pub keys_after: usize,
    pub removed: usize,
}

impl fmt::Display for CompactionStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Compaction: {} -> {} ({} removed)",
            self.keys_before, self.keys_after, self.removed
        )
    }
}

/// Remove entries with empty values from the store.
pub fn compact_empty_values(store: &Storage) -> StorageResult<CompactionStats> {
    let all = store.get_all();
    let keys_before = all.len();
    let empty_keys: Vec<String> = all
        .into_iter()
        .filter(|(_, v)| v.is_empty())
        .map(|(k, _)| k)
        .collect();
    let removed = empty_keys.len();
    for key in &empty_keys {
        store.remove(key)?;
    }
    Ok(CompactionStats {
        keys_before,
        keys_after: keys_before - removed,
        removed,
    })
}

/// Remove entries matching a prefix.
pub fn remove_by_prefix(store: &Storage, prefix: &str) -> StorageResult<usize> {
    let keys: Vec<String> = store
        .keys()
        .into_iter()
        .filter(|k| k.starts_with(prefix))
        .collect();
    let count = keys.len();
    for key in &keys {
        store.remove(key)?;
    }
    Ok(count)
}

// -- StorageKeyNamespace for scoped storage ----------------------------------

/// Provides namespaced access to a storage instance.
pub struct StorageKeyNamespace<'a> {
    store: &'a Storage,
    prefix: String,
}

impl<'a> StorageKeyNamespace<'a> {
    pub fn new(store: &'a Storage, namespace: &str) -> Self {
        Self {
            store,
            prefix: format!("{namespace}."),
        }
    }

    fn prefixed_key(&self, key: &str) -> String {
        format!("{}{}", self.prefix, key)
    }

    pub fn set(&self, key: &str, value: &str) -> StorageResult<()> {
        self.store.set(&self.prefixed_key(key), value)
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.store.get(&self.prefixed_key(key))
    }

    pub fn delete(&self, key: &str) -> StorageResult<()> {
        self.store.remove(&self.prefixed_key(key))
    }

    /// Return all keys in this namespace (without the prefix).
    pub fn keys(&self) -> Vec<String> {
        self.store
            .keys()
            .into_iter()
            .filter_map(|k| k.strip_prefix(&self.prefix).map(|s| s.to_string()))
            .collect()
    }

    /// Count keys in this namespace.
    pub fn key_count(&self) -> usize {
        self.keys().len()
    }

    /// Get the namespace prefix.
    pub fn namespace(&self) -> &str {
        self.prefix.trim_end_matches('.')
    }
}

impl fmt::Display for StorageKeyNamespace<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Namespace('{}')", self.namespace())
    }
}

// -- Storage backup/restore --------------------------------------------------

/// Create a backup of all storage entries as a HashMap.
pub fn backup_storage(store: &Storage) -> HashMap<String, String> {
    store.get_all()
}

/// Restore entries from a backup, overwriting existing values.
pub fn restore_storage(
    store: &Storage,
    backup: &HashMap<String, String>,
) -> StorageResult<usize> {
    let mut count = 0;
    for (key, value) in backup {
        store.set(key, value)?;
        count += 1;
    }
    Ok(count)
}


// ---------------------------------------------------------------------------
// StorageQuotaEnforcer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StorageQuotaEnforcer {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl StorageQuotaEnforcer {
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

impl Default for StorageQuotaEnforcer {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for StorageQuotaEnforcer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "StorageQuotaEnforcer({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// StorageKeyListing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StorageKeyListing {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl StorageKeyListing {
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

impl Default for StorageKeyListing {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for StorageKeyListing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "StorageKeyListing({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// StorageQuotaEnforcerSnapshot — point-in-time snapshot of StorageQuotaEnforcer state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StorageQuotaEnforcerSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl StorageQuotaEnforcerSnapshot {
    pub fn capture(source: &StorageQuotaEnforcer, timestamp: u64) -> Self {
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

impl fmt::Display for StorageQuotaEnforcerSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// StorageKeyListingStats — aggregate statistics for StorageKeyListing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct StorageKeyListingStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl StorageKeyListingStats {
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

impl fmt::Display for StorageKeyListingStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// StorageQuotaEnforcerConfig — configuration for StorageQuotaEnforcer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StorageQuotaEnforcerConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl StorageQuotaEnforcerConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for StorageQuotaEnforcerConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for StorageQuotaEnforcerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// StorageQuotaV2 – enforce storage limits
// ---------------------------------------------------------------------------

/// Enforces quotas on a key-value store.
#[derive(Debug, Clone)]
pub struct StorageQuotaV2 {
    pub max_keys: usize,
    pub max_value_bytes: usize,
    pub max_total_bytes: usize,
    current_keys: usize,
    current_total_bytes: usize,
}

impl StorageQuotaV2 {
    pub fn new(max_keys: usize, max_value_bytes: usize, max_total_bytes: usize) -> Self {
        Self { max_keys, max_value_bytes, max_total_bytes, current_keys: 0, current_total_bytes: 0 }
    }

    pub fn check_key_quota(&self) -> bool {
        self.current_keys < self.max_keys
    }

    pub fn check_value_size(&self, value: &str) -> bool {
        value.len() <= self.max_value_bytes
    }

    pub fn check_total_size(&self, additional: usize) -> bool {
        self.current_total_bytes + additional <= self.max_total_bytes
    }

    pub fn usage_percentage(&self) -> f64 {
        if self.max_total_bytes == 0 { return 100.0; }
        (self.current_total_bytes as f64 / self.max_total_bytes as f64) * 100.0
    }

    /// Record that a key-value pair was added.
    pub fn record_add(&mut self, key_len: usize, value_len: usize) {
        self.current_keys += 1;
        self.current_total_bytes += key_len + value_len;
    }

    /// Record that a key-value pair was removed.
    pub fn record_remove(&mut self, key_len: usize, value_len: usize) {
        self.current_keys = self.current_keys.saturating_sub(1);
        self.current_total_bytes = self.current_total_bytes.saturating_sub(key_len + value_len);
    }
}

// ---------------------------------------------------------------------------
// StorageMigrationHelper – rename/delete/transform stored keys
// ---------------------------------------------------------------------------

/// Helps migrate storage by renaming keys and transforming values.
#[derive(Debug, Clone, Default)]
pub struct StorageMigrationHelper {
    log: Vec<String>,
}

impl StorageMigrationHelper {
    pub fn new() -> Self { Self::default() }

    /// Rename a key in the store.
    pub fn rename_key(&mut self, store: &mut HashMap<String, String>, old: &str, new: &str) -> bool {
        if let Some(val) = store.remove(old) {
            store.insert(new.to_string(), val);
            self.log.push(format!("renamed {} -> {}", old, new));
            true
        } else {
            false
        }
    }

    /// Delete all keys with the given prefix.
    pub fn delete_by_prefix(&mut self, store: &mut HashMap<String, String>, prefix: &str) -> usize {
        let keys: Vec<String> = store.keys().filter(|k| k.starts_with(prefix)).cloned().collect();
        let count = keys.len();
        for k in &keys {
            store.remove(k);
        }
        if count > 0 {
            self.log.push(format!("deleted {} keys with prefix {}", count, prefix));
        }
        count
    }

    /// Transform all values matching a key prefix.
    pub fn transform_values<F: Fn(&str) -> String>(
        &mut self,
        store: &mut HashMap<String, String>,
        prefix: &str,
        transform: F,
    ) -> usize {
        let keys: Vec<String> = store.keys().filter(|k| k.starts_with(prefix)).cloned().collect();
        let count = keys.len();
        for k in &keys {
            if let Some(v) = store.get(k) {
                let new_v = transform(v);
                store.insert(k.clone(), new_v);
            }
        }
        count
    }

    pub fn migration_log(&self) -> &[String] { &self.log }
}

// ---------------------------------------------------------------------------
// StorageNamespaceV2 – prefix-based namespacing
// ---------------------------------------------------------------------------

/// Provides prefix-based namespacing over a flat key-value store.
#[derive(Debug, Clone)]
pub struct StorageNamespaceV2 {
    separator: String,
}

impl StorageNamespaceV2 {
    pub fn new(separator: &str) -> Self {
        Self { separator: separator.to_string() }
    }

    /// Create a namespaced key.
    pub fn namespaced_key(&self, namespace: &str, key: &str) -> String {
        format!("{}{}{}", namespace, self.separator, key)
    }

    /// Return all keys belonging to a namespace.
    pub fn keys_in_namespace<'a>(&self, store: &'a HashMap<String, String>, namespace: &str) -> Vec<&'a str> {
        let prefix = format!("{}{}", namespace, self.separator);
        store.keys().filter(|k| k.starts_with(&prefix)).map(|k| k.as_str()).collect()
    }

    /// Delete all keys in a namespace.
    pub fn delete_namespace(&self, store: &mut HashMap<String, String>, namespace: &str) -> usize {
        let prefix = format!("{}{}", namespace, self.separator);
        let keys: Vec<String> = store.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
        let count = keys.len();
        for k in keys { store.remove(&k); }
        count
    }

    /// Count keys in a namespace.
    pub fn namespace_size(&self, store: &HashMap<String, String>, namespace: &str) -> usize {
        let prefix = format!("{}{}", namespace, self.separator);
        store.keys().filter(|k| k.starts_with(&prefix)).count()
    }

    /// List all distinct namespaces in the store.
    pub fn list_namespaces(&self, store: &HashMap<String, String>) -> Vec<String> {
        let mut ns: Vec<String> = store
            .keys()
            .filter_map(|k| k.split(&self.separator).next().map(|s| s.to_string()))
            .collect();
        ns.sort();
        ns.dedup();
        ns
    }
}


/// Configuration manager for storage functionality.
pub struct StorageConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl StorageConfig {
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

    pub fn merge(&mut self, other: &StorageConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for storage operations.
pub struct StorageRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl StorageRateTracker {
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

/// Validation result collector for storage.
pub struct StorageValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl StorageValidationCollector {
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

    pub fn merge(&mut self, other: &StorageValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Key-value and scoped storage — extended utilities (yb)
// ---------------------------------------------------------------------------

/// Metric accumulator for storage operations.
#[derive(Debug, Clone)]
pub struct YbMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YbMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for storage.
#[derive(Debug, Clone)]
pub struct YbRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YbRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for storage lookups.
#[derive(Debug, Clone)]
pub struct YbLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YbLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for storage
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaStorageRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaStorageRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaStorageCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaStorageCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaStorageCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 166
// ---------------------------------------------------------------------------

/// Generic object pool `Xc166Pool<T>`.
pub struct Xc166Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc166Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc166PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc166Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc166PoolStats {
        Xc166PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc166Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc166Scheduler`.
pub struct Xc166Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc166Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc166Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_166 hash for the given byte slice.
pub fn xc_166_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_166 convention.
pub fn xc_166_reverse(s: &str) -> String {
    s.chars().rev().collect()
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
    fn storage_validator_accepts_and_rejects() {
        let mut v = StorageValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad key");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad key"));
    }

    #[test]
    fn storage_validator_warnings() {
        let mut v = StorageValidationCollector::new();
        v.add_warning("deprecated key");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn storage_validator_clear_and_merge() {
        let mut v = StorageValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = StorageValidationCollector::new();
        a.add_error("a_err");
        let mut b = StorageValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    #[test]
    fn storage_database_basic() {
        let mut db = StorageDatabase::new();
        assert!(db.is_empty());
        db.set("key1", "value1");
        db.set("key2", "value2");
        assert_eq!(db.len(), 2);
        assert_eq!(db.get("key1"), Some("value1"));
        assert!(db.has("key2"));
    }

    #[test]
    fn storage_database_remove() {
        let mut db = StorageDatabase::new();
        db.set("a", "1");
        assert_eq!(db.remove("a"), Some("1".to_string()));
        assert!(!db.has("a"));
    }

    #[test]
    fn storage_database_export() {
        let mut db = StorageDatabase::new();
        db.set("b", "2");
        db.set("a", "1");
        let exported = db.export();
        assert_eq!(
            exported,
            vec![("a".into(), "1".into()), ("b".into(), "2".into())]
        );
    }

    #[test]
    fn storage_database_display() {
        let db = StorageDatabase::new();
        assert_eq!(db.to_string(), "StorageDatabase(v1, 0 entries)");
    }

    #[test]
    fn storage_namespace_scoped_access() {
        let mut db = StorageDatabase::new();
        {
            let mut ns = storage_namespace(&mut db, "editor");
            ns.set("fontSize", "14");
            ns.set("tabSize", "4");
            assert_eq!(ns.get("fontSize"), Some("14"));
            assert!(ns.has("tabSize"));
        }
        // Keys should be prefixed in the underlying database
        assert_eq!(db.get("editor.fontSize"), Some("14"));
    }

    #[test]
    fn storage_namespace_keys() {
        let mut db = StorageDatabase::new();
        db.set("ns1.a", "1");
        db.set("ns1.b", "2");
        db.set("ns2.c", "3");
        let ns = storage_namespace(&mut db, "ns1");
        let keys = ns.keys();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"a".to_string()));
        assert!(keys.contains(&"b".to_string()));
    }

    #[test]
    fn storage_migrate_renames_keys() {
        let mut db = StorageDatabase::new();
        db.set("old_setting", "value");
        let migrations = storage_migrate(&mut db, 2, &[(2, "old_setting", "new_setting")]);
        assert_eq!(migrations.len(), 1);
        assert_eq!(db.get("new_setting"), Some("value"));
        assert!(!db.has("old_setting"));
        assert_eq!(db.version(), 2);
    }

    #[test]
    fn storage_migrate_no_ops_if_current() {
        let mut db = StorageDatabase::new();
        let migrations = storage_migrate(&mut db, 1, &[]);
        assert!(migrations.is_empty());
    }

    // ── StorageQuota / Exporter / ChangeLog tests ──

    #[test]
    fn storage_quota_usage_tracking() {
        let mut db = StorageDatabase::new();
        db.set("key1", "value1");
        db.set("key2", "value2");
        let mut quota = StorageQuota::new(10, 1000);
        quota.compute_usage(&db);
        assert_eq!(quota.current_keys(), 2);
        assert!(quota.current_bytes() > 0);
        assert_eq!(quota.remaining_keys(), 8);
        assert!(!quota.would_exceed(4, 6));
    }

    #[test]
    fn storage_quota_exceed_detection() {
        let mut quota = StorageQuota::new(2, 50);
        quota.current_keys = 2;
        quota.current_bytes = 45;
        assert!(quota.would_exceed(3, 5));
        assert!(quota.key_usage_percent() >= 99.0);
    }

    #[test]
    fn storage_exporter_roundtrip() {
        let mut db = StorageDatabase::new();
        db.set("alpha", "1");
        db.set("beta", "2");
        let map = StorageExporter::to_map(&db);
        let db2 = StorageExporter::from_map(&map);
        assert_eq!(db2.get("alpha"), Some("1"));
        assert_eq!(db2.get("beta"), Some("2"));
        assert_eq!(db2.len(), 2);
    }

    #[test]
    fn storage_exporter_export_prefix() {
        let mut db = StorageDatabase::new();
        db.set("app.theme", "dark");
        db.set("app.font", "mono");
        db.set("user.name", "alice");
        let map = StorageExporter::export_prefix(&db, "app.");
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("app.theme"));
        assert!(!map.contains_key("user.name"));
    }

    #[test]
    fn storage_changelog_records_changes() {
        let mut log = StorageChangeLog::new();
        log.record_set("key1", None, "val1");
        log.record_set("key1", Some("val1"), "val2");
        log.record_remove("key1", Some("val2"));
        assert_eq!(log.len(), 3);
        assert_eq!(log.changed_keys(), vec!["key1"]);
        let key_changes = log.changes_for_key("key1");
        assert_eq!(key_changes.len(), 3);
        assert_eq!(key_changes[0].kind, StorageChangeKind::Set);
        assert_eq!(key_changes[2].kind, StorageChangeKind::Remove);
    }

    #[test]
    fn storage_changelog_changes_since() {
        let mut log = StorageChangeLog::new();
        log.record_set("a", None, "1");
        log.record_set("b", None, "2");
        log.record_set("c", None, "3");
        let since = log.changes_since(1);
        assert_eq!(since.len(), 2);
        assert_eq!(since[0].key, "b");
    }

    // --- new tests ---

    #[test]
    fn test_has_prefix_true() {
        let store = Storage::in_memory().unwrap();
        store.set("editor.fontSize", "14").unwrap();
        assert!(has_prefix(&store, "editor."));
    }

    #[test]
    fn test_has_prefix_false() {
        let store = Storage::in_memory().unwrap();
        store.set("theme", "dark").unwrap();
        assert!(!has_prefix(&store, "editor."));
    }

    #[test]
    fn test_total_value_bytes() {
        let store = Storage::in_memory().unwrap();
        store.set("a", "hello").unwrap(); // 5
        store.set("b", "hi").unwrap(); // 2
        assert_eq!(total_value_bytes(&store), 7);
    }

    #[test]
    fn test_total_value_bytes_empty() {
        let store = Storage::in_memory().unwrap();
        assert_eq!(total_value_bytes(&store), 0);
    }

    #[test]
    fn test_keys_with_duplicate_values() {
        let store = Storage::in_memory().unwrap();
        store.set("a", "same").unwrap();
        store.set("b", "same").unwrap();
        store.set("c", "different").unwrap();
        let dupes = keys_with_duplicate_values(&store);
        assert!(dupes.contains(&"a".to_string()));
        assert!(dupes.contains(&"b".to_string()));
        assert!(!dupes.contains(&"c".to_string()));
    }

    #[test]
    fn test_copy_all_between_stores() {
        let src = Storage::in_memory().unwrap();
        let dst = Storage::in_memory().unwrap();
        src.set("x", "1").unwrap();
        src.set("y", "2").unwrap();
        let count = copy_all(&src, &dst).unwrap();
        assert_eq!(count, 2);
        assert_eq!(dst.get("x"), Some("1".to_string()));
        assert_eq!(dst.get("y"), Some("2".to_string()));
    }

    #[test]
    fn test_integer_keys() {
        let store = Storage::in_memory().unwrap();
        store.set("port", "8080").unwrap();
        store.set("name", "app").unwrap();
        store.set("count", "-5").unwrap();
        let mut ik = integer_keys(&store);
        ik.sort();
        assert_eq!(ik, vec!["count", "port"]);
    }

    #[test]
    fn test_boolean_keys() {
        let store = Storage::in_memory().unwrap();
        store.set("enabled", "true").unwrap();
        store.set("verbose", "false").unwrap();
        store.set("name", "app").unwrap();
        let mut bk = boolean_keys(&store);
        bk.sort();
        assert_eq!(bk, vec!["enabled", "verbose"]);
    }

    #[test]
    fn namespaced_get_set() {
        let store = Storage::in_memory().unwrap();
        let ns = NamespacedStorage::new(&store, "ext.myext");
        ns.set("color", "blue").unwrap();
        assert_eq!(ns.get("color"), Some("blue".to_string()));
        assert_eq!(store.get("ext.myext.color"), Some("blue".to_string()));
        assert!(ns.has("color"));
        assert!(!ns.has("missing"));
    }

    #[test]
    fn namespaced_keys_and_entries() {
        let store = Storage::in_memory().unwrap();
        let ns = NamespacedStorage::new(&store, "app");
        ns.set("a", "1").unwrap();
        ns.set("b", "2").unwrap();
        store.set("other", "3").unwrap();
        let mut keys = ns.keys();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
        assert_eq!(ns.len(), 2);
        assert!(!ns.is_empty());
    }

    #[test]
    fn namespaced_remove() {
        let store = Storage::in_memory().unwrap();
        let ns = NamespacedStorage::new(&store, "ns");
        ns.set("x", "10").unwrap();
        assert!(ns.has("x"));
        ns.remove("x").unwrap();
        assert!(!ns.has("x"));
    }

    #[test]
    fn diff_stores_detects_differences() {
        let left = Storage::in_memory().unwrap();
        let right = Storage::in_memory().unwrap();
        left.set("a", "1").unwrap();
        left.set("b", "same").unwrap();
        left.set("c", "left_val").unwrap();
        right.set("b", "same").unwrap();
        right.set("c", "right_val").unwrap();
        right.set("d", "4").unwrap();
        let diffs = diff_stores(&left, &right);
        assert_eq!(diffs.len(), 3);
        assert_eq!(diffs[0], StorageDiff::LeftOnly { key: "a".into(), value: "1".into() });
        assert_eq!(diffs[1], StorageDiff::Changed { key: "c".into(), left: "left_val".into(), right: "right_val".into() });
        assert_eq!(diffs[2], StorageDiff::RightOnly { key: "d".into(), value: "4".into() });
    }

    #[test]
    fn diff_stores_identical() {
        let left = Storage::in_memory().unwrap();
        let right = Storage::in_memory().unwrap();
        left.set("x", "1").unwrap();
        right.set("x", "1").unwrap();
        assert!(diff_stores(&left, &right).is_empty());
    }

    #[test]
    fn merge_missing_only_adds_new_keys() {
        let src = Storage::in_memory().unwrap();
        let dst = Storage::in_memory().unwrap();
        src.set("a", "1").unwrap();
        src.set("b", "2").unwrap();
        dst.set("b", "existing").unwrap();
        let count = merge_missing(&src, &dst).unwrap();
        assert_eq!(count, 1);
        assert_eq!(dst.get("a"), Some("1".to_string()));
        assert_eq!(dst.get("b"), Some("existing".to_string()));
    }

    #[test]
    fn keys_exceeding_length_filters() {
        let store = Storage::in_memory().unwrap();
        store.set("short", "ab").unwrap();
        store.set("long", "abcdefghij").unwrap();
        store.set("exact", "abcde").unwrap();
        let mut keys = keys_exceeding_length(&store, 5);
        keys.sort();
        assert_eq!(keys, vec!["long"]);
    }

    // -- StorageMigrator tests ------------------------------------------------

    #[test]
    fn migrator_apply_pending() {
        let store = Storage::in_memory().unwrap();
        let mut migrator = StorageMigrator::new();
        migrator.add_migration(1, "CREATE TABLE t1 (id INT)");
        migrator.add_migration(2, "ALTER TABLE t1 ADD col TEXT");
        assert_eq!(migrator.pending_count(), 2);
        let applied = migrator.apply(&store).unwrap();
        assert_eq!(applied, 2);
        assert_eq!(migrator.current_version(), 2);
        assert_eq!(migrator.pending_count(), 0);
    }

    #[test]
    fn migrator_load_version() {
        let store = Storage::in_memory().unwrap();
        store.set("__schema_version", "3").unwrap();
        let mut migrator = StorageMigrator::new();
        migrator.load_version(&store);
        assert_eq!(migrator.current_version(), 3);
    }

    #[test]
    fn migrator_display() {
        let migrator = StorageMigrator::new();
        let s = migrator.to_string();
        assert!(s.contains("v0"));
    }

    // -- StorageCompaction tests ----------------------------------------------

    #[test]
    fn compact_empty_values_removes() {
        let store = Storage::in_memory().unwrap();
        store.set("good", "value").unwrap();
        store.set("empty", "").unwrap();
        let stats = compact_empty_values(&store).unwrap();
        assert_eq!(stats.removed, 1);
        assert_eq!(stats.keys_after, 1);
        assert!(store.get("good").is_some());
        assert!(store.get("empty").is_none());
    }

    #[test]
    fn remove_by_prefix_removes_matching() {
        let store = Storage::in_memory().unwrap();
        store.set("cache.a", "1").unwrap();
        store.set("cache.b", "2").unwrap();
        store.set("config.x", "3").unwrap();
        let removed = remove_by_prefix(&store, "cache.").unwrap();
        assert_eq!(removed, 2);
        assert!(store.get("config.x").is_some());
    }

    // -- StorageKeyNamespace tests --------------------------------------------

    #[test]
    fn namespace_set_and_get() {
        let store = Storage::in_memory().unwrap();
        let ns = StorageKeyNamespace::new(&store, "myext");
        ns.set("key1", "val1").unwrap();
        assert_eq!(ns.get("key1"), Some("val1".to_string()));
        assert_eq!(store.get("myext.key1"), Some("val1".to_string()));
    }

    #[test]
    fn namespace_keys() {
        let store = Storage::in_memory().unwrap();
        let ns = StorageKeyNamespace::new(&store, "ns");
        ns.set("a", "1").unwrap();
        ns.set("b", "2").unwrap();
        store.set("other", "3").unwrap();
        let mut keys = ns.keys();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
        assert_eq!(ns.key_count(), 2);
    }

    #[test]
    fn namespace_display() {
        let store = Storage::in_memory().unwrap();
        let ns = StorageKeyNamespace::new(&store, "test");
        assert_eq!(ns.to_string(), "Namespace('test')");
    }

    // -- Backup/restore tests -------------------------------------------------

    #[test]
    fn backup_and_restore() {
        let store = Storage::in_memory().unwrap();
        store.set("a", "1").unwrap();
        store.set("b", "2").unwrap();
        let backup = backup_storage(&store);
        assert_eq!(backup.len(), 2);

        let store2 = Storage::in_memory().unwrap();
        let count = restore_storage(&store2, &backup).unwrap();
        assert_eq!(count, 2);
        assert_eq!(store2.get("a"), Some("1".to_string()));
    }

    #[test]
    fn compaction_stats_display() {
        let stats = CompactionStats {
            keys_before: 10,
            keys_after: 8,
            removed: 2,
        };
        let s = stats.to_string();
        assert!(s.contains("10 -> 8"));
        assert!(s.contains("2 removed"));
    }

    #[test] fn storageQuotaEnforcer_new() { let s = StorageQuotaEnforcer::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn storageQuotaEnforcer_add() { let mut s = StorageQuotaEnforcer::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn storageQuotaEnforcer_remove() { let mut s = StorageQuotaEnforcer::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn storageQuotaEnforcer_config() { let mut s = StorageQuotaEnforcer::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn storageQuotaEnforcer_nav() { let mut s = StorageQuotaEnforcer::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn storageQuotaEnforcer_filter() { let mut s = StorageQuotaEnforcer::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn storageQuotaEnforcer_display() { assert!(format!("{}", StorageQuotaEnforcer::new()).contains("StorageQuotaEnforcer")); }
    #[test] fn storageKeyListing_new() { let s = StorageKeyListing::new(); assert!(s.is_empty()); }
    #[test] fn storageKeyListing_add() { let mut s = StorageKeyListing::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn storageKeyListing_active() { let mut s = StorageKeyListing::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn storageKeyListing_error() { let mut s = StorageKeyListing::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn storageKeyListing_rm_group() { let mut s = StorageKeyListing::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn storageKeyListing_display() { assert!(format!("{}", StorageKeyListing::new()).contains("StorageKeyListing")); }


    #[test] fn storageQuotaEnforcer_snap_capture() {
        let s = StorageQuotaEnforcer::new();
        let snap = StorageQuotaEnforcerSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn storageQuotaEnforcer_snap_stale() {
        let s = StorageQuotaEnforcer::new();
        let snap = StorageQuotaEnforcerSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn storageQuotaEnforcer_snap_diff() {
        let s = StorageQuotaEnforcer::new();
        let s1v = StorageQuotaEnforcerSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn storageQuotaEnforcer_snap_display() {
        let s = StorageQuotaEnforcer::new();
        let snap = StorageQuotaEnforcerSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn storageKeyListing_stats_record() {
        let mut st = StorageKeyListingStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn storageKeyListing_stats_hit_ratio() {
        let mut st = StorageKeyListingStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn storageKeyListing_stats_merge() {
        let mut a = StorageKeyListingStats::new();
        a.total_adds = 5;
        let mut b = StorageKeyListingStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn storageKeyListing_stats_display() {
        let st = StorageKeyListingStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn storageQuotaEnforcer_config_default() {
        let c = StorageQuotaEnforcerConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn storageQuotaEnforcer_config_builder() {
        let c = StorageQuotaEnforcerConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn storageQuotaEnforcer_config_labels() {
        let mut c = StorageQuotaEnforcerConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn storageQuotaEnforcer_config_cleanup_threshold() {
        let c = StorageQuotaEnforcerConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn storageQuotaEnforcer_config_display() {
        assert!(format!("{}", StorageQuotaEnforcerConfig::new()).contains("Config"));
    }
    #[test] fn storageKeyListing_stats_peaks() {
        let mut st = StorageKeyListingStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- StorageQuotaV2 -------------------------------------------------------

    #[test]
    fn quota_check_key() {
        let mut q = StorageQuotaV2::new(2, 1024, 4096);
        assert!(q.check_key_quota());
        q.record_add(3, 5);
        q.record_add(3, 5);
        assert!(!q.check_key_quota());
    }

    #[test]
    fn quota_check_value_size() {
        let q = StorageQuotaV2::new(100, 10, 1000);
        assert!(q.check_value_size("short"));
        assert!(!q.check_value_size("this is a really long value!"));
    }

    #[test]
    fn quota_check_total_size() {
        let mut q = StorageQuotaV2::new(100, 1024, 20);
        q.record_add(5, 10);
        assert!(q.check_total_size(3));
        assert!(!q.check_total_size(10));
    }

    #[test]
    fn quota_usage_percentage() {
        let mut q = StorageQuotaV2::new(100, 1024, 100);
        q.record_add(5, 45);
        assert!((q.usage_percentage() - 50.0).abs() < 0.1);
    }

    // -- StorageMigrationHelper --------------------------------------------

    #[test]
    fn migration_rename_key() {
        let mut store: HashMap<String, String> = [("old_key".into(), "val".into())].into_iter().collect();
        let mut helper = StorageMigrationHelper::new();
        assert!(helper.rename_key(&mut store, "old_key", "new_key"));
        assert_eq!(store.get("new_key").unwrap(), "val");
        assert!(!store.contains_key("old_key"));
        assert_eq!(helper.migration_log().len(), 1);
    }

    #[test]
    fn migration_delete_by_prefix() {
        let mut store: HashMap<String, String> = [
            ("cache.a".into(), "1".into()),
            ("cache.b".into(), "2".into()),
            ("data.c".into(), "3".into()),
        ].into_iter().collect();
        let mut helper = StorageMigrationHelper::new();
        assert_eq!(helper.delete_by_prefix(&mut store, "cache."), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn migration_transform_values() {
        let mut store: HashMap<String, String> = [("x.a".into(), "hello".into())].into_iter().collect();
        let mut helper = StorageMigrationHelper::new();
        helper.transform_values(&mut store, "x.", |v| v.to_uppercase());
        assert_eq!(store.get("x.a").unwrap(), "HELLO");
    }

    // -- StorageNamespaceV2 ---------------------------------------------------

    #[test]
    fn namespace_key() {
        let ns = StorageNamespaceV2::new(".");
        assert_eq!(ns.namespaced_key("ext", "theme"), "ext.theme");
    }

    #[test]
    fn namespace_keys_in() {
        let ns = StorageNamespaceV2::new(".");
        let store: HashMap<String, String> = [
            ("ext.a".into(), "1".into()),
            ("ext.b".into(), "2".into()),
            ("other.c".into(), "3".into()),
        ].into_iter().collect();
        assert_eq!(ns.keys_in_namespace(&store, "ext").len(), 2);
    }

    #[test]
    fn namespace_delete() {
        let ns = StorageNamespaceV2::new(".");
        let mut store: HashMap<String, String> = [
            ("ext.a".into(), "1".into()),
            ("other.b".into(), "2".into()),
        ].into_iter().collect();
        assert_eq!(ns.delete_namespace(&mut store, "ext"), 1);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn namespace_list() {
        let ns = StorageNamespaceV2::new(".");
        let store: HashMap<String, String> = [
            ("ext.a".into(), "1".into()),
            ("core.b".into(), "2".into()),
        ].into_iter().collect();
        let namespaces = ns.list_namespaces(&store);
        assert!(namespaces.contains(&"ext".to_string()));
        assert!(namespaces.contains(&"core".to_string()));
    }


    #[test]
    fn storage_config_new() {
        let cfg = StorageConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn storage_config_set_get() {
        let mut cfg = StorageConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn storage_config_remove() {
        let mut cfg = StorageConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn storage_config_keys_sorted() {
        let mut cfg = StorageConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn storage_config_bump_version() {
        let mut cfg = StorageConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn storage_config_clear() {
        let mut cfg = StorageConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn storage_config_merge() {
        let mut cfg1 = StorageConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = StorageConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn storage_config_disable() {
        let mut cfg = StorageConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn storage_rate_tracker_empty() {
        let rt = StorageRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn storage_rate_tracker_record() {
        let mut rt = StorageRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn storage_rate_tracker_prune() {
        let mut rt = StorageRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn storage_validator_valid() {
        let v = StorageValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn storage_validator_errors() {
        let mut v = StorageValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn storage_validator_clear() {
        let mut v = StorageValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn storage_validator_merge() {
        let mut v1 = StorageValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = StorageValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn storage_rate_tracker_clear() {
        let mut rt = StorageRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yb_metrics_empty() {
        let m = YbMetrics::new("storage");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yb_metrics_record_and_mean() {
        let mut m = YbMetrics::new("storage");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yb_metrics_min_max() {
        let mut m = YbMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yb_metrics_variance_and_std() {
        let mut m = YbMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn yb_metrics_percentile() {
        let mut m = YbMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yb_metrics_merge() {
        let mut a = YbMetrics::new("a");
        a.record(1.0);
        let mut b = YbMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yb_metrics_reset() {
        let mut m = YbMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yb_rate_window_empty() {
        let rw = YbRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yb_rate_window_tick_and_rate() {
        let mut rw = YbRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yb_lru_cache_basic() {
        let mut c = YbLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yb_lru_cache_contains_and_keys() {
        let mut c = YbLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yb_lru_cache_remove() {
        let mut c = YbLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yb_metrics_sum() {
        let mut m = YbMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yb_metrics_label() {
        let m = YbMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yb_lru_cache_clear() {
        let mut c = YbLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for storage
    #[test]
    fn xa_storage_ring_new() {
        let rb = super::XaStorageRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_storage_ring_push_len() {
        let mut rb = super::XaStorageRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_storage_ring_wrap() {
        let mut rb = super::XaStorageRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_storage_ring_mean_empty() {
        let rb = super::XaStorageRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_storage_ring_mean_values() {
        let mut rb = super::XaStorageRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_storage_ring_min_max() {
        let mut rb = super::XaStorageRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_storage_ring_iter() {
        let mut rb = super::XaStorageRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_storage_counter_new() {
        let c = super::XaStorageCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_storage_counter_inc() {
        let mut c = super::XaStorageCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_storage_counter_inc_by() {
        let mut c = super::XaStorageCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_storage_counter_reset() {
        let mut c = super::XaStorageCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_storage_counter_clear() {
        let mut c = super::XaStorageCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_storage_counter_default() {
        let c = super::XaStorageCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 166 ----

    #[test]
    fn xc_166_pool_new_empty() {
        let pool: super::Xc166Pool<i32> = super::Xc166Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_166_pool_release_acquire() {
        let mut pool = super::Xc166Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_166_pool_acquire_empty() {
        let mut pool: super::Xc166Pool<i32> = super::Xc166Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_166_pool_full() {
        let mut pool = super::Xc166Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_166_pool_drain() {
        let mut pool = super::Xc166Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_166_pool_stats() {
        let mut pool = super::Xc166Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_166_pool_clear() {
        let mut pool = super::Xc166Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_166_pool_shrink() {
        let mut pool = super::Xc166Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_166_pool_default() {
        let pool: super::Xc166Pool<String> = super::Xc166Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_166_pool_extend() {
        let mut pool = super::Xc166Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_166_pool_retain() {
        let mut pool = super::Xc166Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_166_scheduler_round_robin() {
        let mut sched = super::Xc166Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_166_scheduler_empty() {
        let mut sched = super::Xc166Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_166_scheduler_reset() {
        let mut sched = super::Xc166Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_166_scheduler_add_remove() {
        let mut sched = super::Xc166Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_166_scheduler_targets() {
        let sched = super::Xc166Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_166_hash_empty() {
        assert_eq!(super::xc_166_hash(b""), 5381);
    }

    #[test]
    fn xc_166_hash_data() {
        let h = super::xc_166_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_166_hash(b"hello"), h);
    }

    #[test]
    fn xc_166_reverse_str() {
        assert_eq!(super::xc_166_reverse("abc"), "cba");
        assert_eq!(super::xc_166_reverse(""), "");
    }

}
