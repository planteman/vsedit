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
}
