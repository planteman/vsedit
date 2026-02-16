//! Persistent application state.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur when working with state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    /// The key is empty or contains only whitespace.
    InvalidKey(String),
    /// The value exceeds the maximum allowed length.
    ValueTooLong { key: String, len: usize, max: usize },
    /// The requested key was not found.
    KeyNotFound(String),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::InvalidKey(k) => write!(f, "invalid key: {:?}", k),
            StateError::ValueTooLong { key, len, max } => {
                write!(f, "value for {:?} is {} bytes (max {})", key, len, max)
            }
            StateError::KeyNotFound(k) => write!(f, "key not found: {:?}", k),
        }
    }
}

impl std::error::Error for StateError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateScope {
    Global,
    Workspace,
    Window,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredState {
    pub key: String,
    pub value: String,
    pub scope: StateScope,
}

pub struct StateService {
    state: HashMap<String, StoredState>,
}

impl StateService {
    pub fn new() -> Self {
        Self {
            state: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>, scope: StateScope) {
        let key = key.into();
        let stored = StoredState {
            key: key.clone(),
            value: value.into(),
            scope,
        };
        self.state.insert(key, stored);
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.state.get(key).map(|s| s.value.as_str())
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.state.remove(key).is_some()
    }

    pub fn get_by_scope(&self, scope: StateScope) -> Vec<(&str, &str)> {
        self.state
            .values()
            .filter(|s| s.scope == scope)
            .map(|s| (s.key.as_str(), s.value.as_str()))
            .collect()
    }

    pub fn clear_scope(&mut self, scope: StateScope) {
        self.state.retain(|_, v| v.scope != scope);
    }

    pub fn key_count(&self) -> usize {
        self.state.len()
    }

    pub fn get_or_default<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get(key).unwrap_or(default)
    }

    pub fn has(&self, key: &str) -> bool {
        self.state.contains_key(key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.state.keys().map(|k| k.as_str()).collect()
    }

    pub fn set_many(&mut self, entries: Vec<(&str, &str, StateScope)>) {
        for (key, value, scope) in entries {
            self.set(key, value, scope);
        }
    }

    pub fn get_scope(&self, key: &str) -> Option<StateScope> {
        self.state.get(key).map(|s| s.scope)
    }

    pub fn update<F: FnOnce(&str) -> String>(&mut self, key: &str, updater: F) -> bool {
        if let Some(entry) = self.state.get_mut(key) {
            entry.value = updater(&entry.value);
            true
        } else {
            false
        }
    }

    pub fn merge(&mut self, other: &StateService) {
        for (key, stored) in &other.state {
            self.state.insert(key.clone(), stored.clone());
        }
    }

    pub fn snapshot(&self) -> Vec<StoredState> {
        self.state.values().cloned().collect()
    }
}

impl Default for StateService {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for StateService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StateService")
            .field("key_count", &self.state.len())
            .finish()
    }
}

impl fmt::Display for StateService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StateService({} keys)", self.state.len())
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Maximum byte-length allowed for a single stored value.
pub const MAX_VALUE_LEN: usize = 64 * 1024;

fn validate_key(key: &str) -> Result<(), StateError> {
    if key.trim().is_empty() {
        return Err(StateError::InvalidKey(key.to_string()));
    }
    Ok(())
}

fn validate_value(key: &str, value: &str) -> Result<(), StateError> {
    if value.len() > MAX_VALUE_LEN {
        return Err(StateError::ValueTooLong {
            key: key.to_string(),
            len: value.len(),
            max: MAX_VALUE_LEN,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Validated / business-logic methods on StateService
// ---------------------------------------------------------------------------

impl StateService {
    /// Like [`set`](Self::set) but validates key and value first.
    pub fn try_set(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
        scope: StateScope,
    ) -> Result<(), StateError> {
        let key = key.into();
        let value = value.into();
        validate_key(&key)?;
        validate_value(&key, &value)?;
        self.set(key, value, scope);
        Ok(())
    }

    /// Get a value or return [`StateError::KeyNotFound`].
    pub fn get_or_err(&self, key: &str) -> Result<&str, StateError> {
        self.get(key)
            .ok_or_else(|| StateError::KeyNotFound(key.to_string()))
    }

    /// Returns the number of entries for a given scope.
    pub fn scope_count(&self, scope: StateScope) -> usize {
        self.state.values().filter(|s| s.scope == scope).count()
    }

    /// Returns `true` when the service contains no entries.
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }

    /// Rename a key, preserving its value and scope.
    /// Returns an error if the old key does not exist or the new key is invalid.
    pub fn rename_key(&mut self, old: &str, new: &str) -> Result<(), StateError> {
        validate_key(new)?;
        let entry = self
            .state
            .remove(old)
            .ok_or_else(|| StateError::KeyNotFound(old.to_string()))?;
        let new_key = new.to_string();
        self.state.insert(
            new_key.clone(),
            StoredState {
                key: new_key,
                value: entry.value,
                scope: entry.scope,
            },
        );
        Ok(())
    }

    /// Increment a numeric value stored at `key`. Returns the new value.
    /// If the stored value is not a valid `i64`, it is treated as `0`.
    pub fn increment(&mut self, key: &str, delta: i64) -> Result<i64, StateError> {
        let entry = self
            .state
            .get_mut(key)
            .ok_or_else(|| StateError::KeyNotFound(key.to_string()))?;
        let current: i64 = entry.value.parse().unwrap_or(0);
        let next = current + delta;
        entry.value = next.to_string();
        Ok(next)
    }

    /// Collect all unique scopes currently stored.
    pub fn active_scopes(&self) -> Vec<StateScope> {
        let mut scopes: Vec<StateScope> = self
            .state
            .values()
            .map(|s| s.scope)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        scopes.sort_by_key(|s| *s as u8);
        scopes
    }
}

// ---------------------------------------------------------------------------
// Builder for StoredState
// ---------------------------------------------------------------------------

/// Fluent builder for [`StoredState`].
#[derive(Debug, Clone)]
pub struct StoredStateBuilder {
    key: Option<String>,
    value: Option<String>,
    scope: StateScope,
}

impl StoredStateBuilder {
    pub fn new() -> Self {
        Self {
            key: None,
            value: None,
            scope: StateScope::Global,
        }
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn scope(mut self, scope: StateScope) -> Self {
        self.scope = scope;
        self
    }

    /// Build a [`StoredState`], validating key and value.
    pub fn build(self) -> Result<StoredState, StateError> {
        let key = self.key.unwrap_or_default();
        let value = self.value.unwrap_or_default();
        validate_key(&key)?;
        validate_value(&key, &value)?;
        Ok(StoredState {
            key,
            value,
            scope: self.scope,
        })
    }
}

impl Default for StoredStateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StateScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateScope::Global => write!(f, "Global"),
            StateScope::Workspace => write!(f, "Workspace"),
            StateScope::Window => write!(f, "Window"),
        }
    }
}

impl fmt::Display for StoredState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} = {}", self.scope, self.key, self.value)
    }
}

/// Accumulated statistics for state operations.
#[derive(Debug, Clone, PartialEq)]
pub struct StateStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl StateStats {
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
    pub fn merge(&mut self, other: &StateStats) {
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

impl Default for StateStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StateStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StateStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for state.
#[derive(Debug, Clone)]
pub struct StateValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl StateValidator {
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

impl Default for StateValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Per-workspace state
// ---------------------------------------------------------------------------

/// Per-workspace state storage identified by a workspace ID.
#[derive(Debug, Clone)]
pub struct WorkspaceState {
    pub workspace_id: String,
    store: HashMap<String, String>,
}

impl WorkspaceState {
    pub fn new(workspace_id: impl Into<String>) -> Self {
        Self { workspace_id: workspace_id.into(), store: HashMap::new() }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.store.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.store.get(key).map(|s| s.as_str())
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.store.remove(key).is_some()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.store.keys().map(|s| s.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    pub fn clear(&mut self) {
        self.store.clear();
    }

    /// Export all entries as a list of (key, value) pairs for serialization.
    pub fn export(&self) -> Vec<(&str, &str)> {
        self.store.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
    }

    /// Import entries from a list of (key, value) pairs.
    pub fn import(&mut self, entries: &[(&str, &str)]) {
        for (k, v) in entries {
            self.store.insert(k.to_string(), v.to_string());
        }
    }
}

impl fmt::Display for WorkspaceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WorkspaceState(id={}, keys={})", self.workspace_id, self.store.len())
    }
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------

/// Global state that persists across all workspaces.
#[derive(Debug, Clone)]
pub struct GlobalState {
    store: HashMap<String, String>,
    pub version: u32,
}

impl GlobalState {
    pub fn new() -> Self {
        Self { store: HashMap::new(), version: 1 }
    }

    pub fn with_version(version: u32) -> Self {
        Self { store: HashMap::new(), version }
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.store.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.store.get(key).map(|s| s.as_str())
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.store.remove(key).is_some()
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    pub fn clear(&mut self) {
        self.store.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.store.keys().map(|s| s.as_str()).collect()
    }

    /// Export all entries for serialization.
    pub fn export(&self) -> Vec<(&str, &str)> {
        self.store.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
    }
}

impl Default for GlobalState {
    fn default() -> Self { Self::new() }
}

impl fmt::Display for GlobalState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GlobalState(v{}, keys={})", self.version, self.store.len())
    }
}

// ---------------------------------------------------------------------------
// State migration
// ---------------------------------------------------------------------------

/// A migration step that transforms state from one version to the next.
#[derive(Debug, Clone)]
pub struct StateMigration {
    pub from_version: u32,
    pub to_version: u32,
    pub description: String,
}

impl StateMigration {
    pub fn new(from: u32, to: u32, description: impl Into<String>) -> Self {
        Self { from_version: from, to_version: to, description: description.into() }
    }
}

impl fmt::Display for StateMigration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Migration v{} -> v{}: {}", self.from_version, self.to_version, self.description)
    }
}

/// Result of applying a migration chain.
#[derive(Debug, Clone)]
pub struct MigrationResult {
    pub applied: Vec<StateMigration>,
    pub final_version: u32,
    pub keys_renamed: usize,
    pub keys_removed: usize,
}

/// Apply a series of key renames and removals as part of a state migration.
pub fn state_migration(
    state: &mut GlobalState,
    renames: &[(&str, &str)],
    removals: &[&str],
    target_version: u32,
) -> MigrationResult {
    let from_version = state.version();
    let mut keys_renamed = 0;
    let mut keys_removed = 0;

    for (old_key, new_key) in renames {
        if let Some(val) = state.get(old_key).map(|s| s.to_string()) {
            state.remove(old_key);
            state.set(*new_key, val);
            keys_renamed += 1;
        }
    }

    for key in removals {
        if state.remove(key) {
            keys_removed += 1;
        }
    }

    state.version = target_version;

    MigrationResult {
        applied: vec![StateMigration::new(from_version, target_version, "state migration")],
        final_version: target_version,
        keys_renamed,
        keys_removed,
    }
}

/// Check if migration is needed between two versions.
pub fn migration_needed(current_version: u32, target_version: u32) -> bool {
    current_version < target_version
}

// ── StateSnapshot ──

/// A complete snapshot of a `StateService` at a point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSnapshot {
    pub entries: HashMap<String, StoredState>,
    pub label: String,
}

impl StateSnapshot {
    /// Capture a snapshot from the given `StateService`.
    pub fn capture(svc: &StateService, label: impl Into<String>) -> Self {
        Self {
            entries: svc.state.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            label: label.into(),
        }
    }

    /// Returns the number of entries in this snapshot.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get a value from the snapshot by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|s| s.value.as_str())
    }

    /// Returns all keys in this snapshot.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.keys().map(|k| k.as_str()).collect()
    }
}

impl fmt::Display for StateSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StateSnapshot({}, {} entries)", self.label, self.entries.len())
    }
}

// ── StateDiffEngine ──

/// Represents a single difference between two state snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateDiff {
    /// Key exists in new snapshot but not in old.
    Added { key: String, value: String },
    /// Key exists in old snapshot but not in new.
    Removed { key: String, value: String },
    /// Key exists in both but with different values.
    Changed { key: String, old_value: String, new_value: String },
}

impl fmt::Display for StateDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateDiff::Added { key, value } => write!(f, "+ {} = {}", key, value),
            StateDiff::Removed { key, value } => write!(f, "- {} = {}", key, value),
            StateDiff::Changed { key, old_value, new_value } => {
                write!(f, "~ {} : {} -> {}", key, old_value, new_value)
            }
        }
    }
}

/// Computes diffs between two `StateSnapshot`s.
pub struct StateDiffEngine;

impl StateDiffEngine {
    /// Compute the list of differences between `old` and `new` snapshots.
    pub fn diff(old: &StateSnapshot, new: &StateSnapshot) -> Vec<StateDiff> {
        let mut diffs = Vec::new();

        // Find added and changed
        for (key, new_entry) in &new.entries {
            match old.entries.get(key) {
                Some(old_entry) if old_entry.value != new_entry.value => {
                    diffs.push(StateDiff::Changed {
                        key: key.clone(),
                        old_value: old_entry.value.clone(),
                        new_value: new_entry.value.clone(),
                    });
                }
                None => {
                    diffs.push(StateDiff::Added {
                        key: key.clone(),
                        value: new_entry.value.clone(),
                    });
                }
                _ => {} // unchanged
            }
        }

        // Find removed
        for (key, old_entry) in &old.entries {
            if !new.entries.contains_key(key) {
                diffs.push(StateDiff::Removed {
                    key: key.clone(),
                    value: old_entry.value.clone(),
                });
            }
        }

        diffs.sort_by(|a, b| {
            let key_a = match a {
                StateDiff::Added { key, .. } | StateDiff::Removed { key, .. } | StateDiff::Changed { key, .. } => key,
            };
            let key_b = match b {
                StateDiff::Added { key, .. } | StateDiff::Removed { key, .. } | StateDiff::Changed { key, .. } => key,
            };
            key_a.cmp(key_b)
        });

        diffs
    }

    /// Returns true if the two snapshots are identical.
    pub fn is_equal(old: &StateSnapshot, new: &StateSnapshot) -> bool {
        Self::diff(old, new).is_empty()
    }

    /// Returns only the keys that differ between two snapshots.
    pub fn changed_keys(old: &StateSnapshot, new: &StateSnapshot) -> Vec<String> {
        Self::diff(old, new)
            .into_iter()
            .map(|d| match d {
                StateDiff::Added { key, .. } | StateDiff::Removed { key, .. } | StateDiff::Changed { key, .. } => key,
            })
            .collect()
    }
}

// ── StateSubscription ──

/// A subscriber callback identifier.
pub type SubscriptionId = u64;

/// Manages subscriptions to state changes.
pub struct StateSubscription {
    next_id: SubscriptionId,
    /// Subscriptions stored as (id, key_prefix) pairs.
    subscriptions: Vec<(SubscriptionId, String)>,
}

impl StateSubscription {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            subscriptions: Vec::new(),
        }
    }

    /// Subscribe to changes for keys matching the given prefix.
    /// Returns a subscription ID that can be used to unsubscribe.
    pub fn subscribe(&mut self, key_prefix: impl Into<String>) -> SubscriptionId {
        let id = self.next_id;
        self.next_id += 1;
        self.subscriptions.push((id, key_prefix.into()));
        id
    }

    /// Remove a subscription by ID. Returns true if found.
    pub fn unsubscribe(&mut self, id: SubscriptionId) -> bool {
        let len = self.subscriptions.len();
        self.subscriptions.retain(|(sid, _)| *sid != id);
        self.subscriptions.len() < len
    }

    /// Returns the subscription IDs that match the given key.
    pub fn matching_subscriptions(&self, key: &str) -> Vec<SubscriptionId> {
        self.subscriptions
            .iter()
            .filter(|(_, prefix)| key.starts_with(prefix.as_str()))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Returns the total number of active subscriptions.
    pub fn count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Returns true if there are no subscriptions.
    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }
}

impl Default for StateSubscription {
    fn default() -> Self {
        Self::new()
    }
}

// ── Namespace support on StateService ──

impl StateService {
    /// Set a value under a namespace, using "namespace.key" format.
    pub fn ns_set(&mut self, namespace: &str, key: &str, value: impl Into<String>, scope: StateScope) {
        let full_key = format!("{}.{}", namespace, key);
        self.set(full_key, value, scope);
    }

    /// Get a value under a namespace.
    pub fn ns_get(&self, namespace: &str, key: &str) -> Option<&str> {
        let full_key = format!("{}.{}", namespace, key);
        self.get(&full_key)
    }

    /// Returns all keys within a namespace.
    pub fn ns_keys(&self, namespace: &str) -> Vec<&str> {
        let prefix = format!("{}.", namespace);
        self.state
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .map(|k| k.as_str())
            .collect()
    }

    /// Remove all keys within a namespace. Returns the number of keys removed.
    pub fn ns_clear(&mut self, namespace: &str) -> usize {
        let prefix = format!("{}.", namespace);
        let keys: Vec<String> = self.state
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        let count = keys.len();
        for k in keys {
            self.state.remove(&k);
        }
        count
    }
}

// ---------------------------------------------------------------------------
// StateService — query and bulk operations
// ---------------------------------------------------------------------------

impl StateService {
    /// Return all entries whose value contains the given substring.
    pub fn search_values(&self, query: &str) -> Vec<(&str, &str)> {
        self.state
            .values()
            .filter(|s| s.value.contains(query))
            .map(|s| (s.key.as_str(), s.value.as_str()))
            .collect()
    }

    /// Return the total byte size of all stored values.
    pub fn total_value_bytes(&self) -> usize {
        self.state.values().map(|s| s.value.len()).sum()
    }

    /// Return all keys sorted alphabetically.
    pub fn sorted_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.state.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        keys
    }

    /// Retain only entries matching a predicate on (key, value).
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) -> usize {
        let before = self.state.len();
        self.state.retain(|k, v| f(k, &v.value));
        before - self.state.len()
    }

    /// Copy all entries from one scope to another, overwriting on key collision.
    pub fn copy_scope(&mut self, from: StateScope, to: StateScope) -> usize {
        let entries: Vec<(String, String)> = self.state
            .values()
            .filter(|s| s.scope == from)
            .map(|s| (s.key.clone(), s.value.clone()))
            .collect();
        let count = entries.len();
        for (key, value) in entries {
            self.set(key, value, to);
        }
        count
    }
}

/// Export all entries of a `StateService` as a sorted key=value string.
pub fn export_state(svc: &StateService) -> String {
    let mut entries: Vec<(&str, &str)> = svc.state
        .values()
        .map(|s| (s.key.as_str(), s.value.as_str()))
        .collect();
    entries.sort_by_key(|(k, _)| *k);
    entries.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Import state entries from a key=value string (one per line).
pub fn import_state(svc: &mut StateService, text: &str, scope: StateScope) -> usize {
    let mut count = 0;
    for line in text.lines() {
        if let Some(eq_pos) = line.find('=') {
            let key = &line[..eq_pos];
            let value = &line[eq_pos + 1..];
            if !key.trim().is_empty() {
                svc.set(key, value, scope);
                count += 1;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let mut svc = StateService::new();
        svc.set("theme", "dark", StateScope::Global);
        assert_eq!(svc.get("theme"), Some("dark"));
        assert_eq!(svc.get("missing"), None);
    }

    #[test]
    fn remove_and_count() {
        let mut svc = StateService::new();
        svc.set("a", "1", StateScope::Global);
        svc.set("b", "2", StateScope::Workspace);
        assert_eq!(svc.key_count(), 2);
        assert!(svc.remove("a"));
        assert!(!svc.remove("a"));
        assert_eq!(svc.key_count(), 1);
    }

    #[test]
    fn scope_filtering_and_clear() {
        let mut svc = StateService::new();
        svc.set("a", "1", StateScope::Global);
        svc.set("b", "2", StateScope::Workspace);
        svc.set("c", "3", StateScope::Global);
        assert_eq!(svc.get_by_scope(StateScope::Global).len(), 2);
        assert_eq!(svc.get_by_scope(StateScope::Workspace).len(), 1);
        svc.clear_scope(StateScope::Global);
        assert_eq!(svc.key_count(), 1);
        assert_eq!(svc.get("b"), Some("2"));
    }

    #[test]
    fn get_or_default_returns_stored_value() {
        let mut svc = StateService::new();
        svc.set("lang", "rust", StateScope::Global);
        assert_eq!(svc.get_or_default("lang", "python"), "rust");
        assert_eq!(svc.get_or_default("missing", "fallback"), "fallback");
    }

    #[test]
    fn has_checks_existence() {
        let mut svc = StateService::new();
        assert!(!svc.has("x"));
        svc.set("x", "1", StateScope::Window);
        assert!(svc.has("x"));
        svc.remove("x");
        assert!(!svc.has("x"));
    }

    #[test]
    fn keys_returns_all_keys() {
        let mut svc = StateService::new();
        svc.set("a", "1", StateScope::Global);
        svc.set("b", "2", StateScope::Workspace);
        let mut keys = svc.keys();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn set_many_inserts_multiple() {
        let mut svc = StateService::new();
        svc.set_many(vec![
            ("x", "10", StateScope::Global),
            ("y", "20", StateScope::Window),
            ("z", "30", StateScope::Workspace),
        ]);
        assert_eq!(svc.key_count(), 3);
        assert_eq!(svc.get("y"), Some("20"));
        assert_eq!(svc.get_scope("z"), Some(StateScope::Workspace));
    }

    #[test]
    fn update_transforms_existing_value() {
        let mut svc = StateService::new();
        svc.set("count", "5", StateScope::Global);
        let updated = svc.update("count", |v| {
            let n: i32 = v.parse().unwrap();
            (n + 1).to_string()
        });
        assert!(updated);
        assert_eq!(svc.get("count"), Some("6"));
        assert!(!svc.update("missing", |_| "nope".into()));
    }

    #[test]
    fn merge_copies_entries() {
        let mut a = StateService::new();
        a.set("k1", "v1", StateScope::Global);

        let mut b = StateService::new();
        b.set("k2", "v2", StateScope::Workspace);
        b.set("k1", "overwritten", StateScope::Window);

        a.merge(&b);
        assert_eq!(a.key_count(), 2);
        assert_eq!(a.get("k1"), Some("overwritten"));
        assert_eq!(a.get_scope("k1"), Some(StateScope::Window));
        assert_eq!(a.get("k2"), Some("v2"));
    }

    #[test]
    fn snapshot_clones_all_entries() {
        let mut svc = StateService::new();
        svc.set("a", "1", StateScope::Global);
        svc.set("b", "2", StateScope::Workspace);
        let snap = svc.snapshot();
        assert_eq!(snap.len(), 2);
        svc.set("a", "changed", StateScope::Global);
        let original_a = snap.iter().find(|s| s.key == "a").unwrap();
        assert_eq!(original_a.value, "1");
    }

    #[test]
    fn display_impls() {
        assert_eq!(StateScope::Global.to_string(), "Global");
        assert_eq!(StateScope::Workspace.to_string(), "Workspace");
        assert_eq!(StateScope::Window.to_string(), "Window");

        let stored = StoredState {
            key: "theme".into(),
            value: "dark".into(),
            scope: StateScope::Global,
        };
        assert_eq!(stored.to_string(), "[Global] theme = dark");
    }

    // -----------------------------------------------------------------------
    // New tests
    // -----------------------------------------------------------------------

    #[test]
    fn try_set_rejects_empty_key() {
        let mut svc = StateService::new();
        let err = svc.try_set("", "val", StateScope::Global).unwrap_err();
        assert_eq!(err, StateError::InvalidKey("".into()));
    }

    #[test]
    fn try_set_rejects_whitespace_key() {
        let mut svc = StateService::new();
        assert!(svc.try_set("  \t", "v", StateScope::Global).is_err());
    }

    #[test]
    fn try_set_rejects_oversized_value() {
        let mut svc = StateService::new();
        let big = "x".repeat(MAX_VALUE_LEN + 1);
        let err = svc.try_set("k", big, StateScope::Global).unwrap_err();
        match err {
            StateError::ValueTooLong { len, max, .. } => {
                assert_eq!(len, MAX_VALUE_LEN + 1);
                assert_eq!(max, MAX_VALUE_LEN);
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn try_set_accepts_valid_entry() {
        let mut svc = StateService::new();
        svc.try_set("lang", "rust", StateScope::Workspace).unwrap();
        assert_eq!(svc.get("lang"), Some("rust"));
    }

    #[test]
    fn get_or_err_returns_error_for_missing() {
        let svc = StateService::new();
        assert_eq!(
            svc.get_or_err("nope"),
            Err(StateError::KeyNotFound("nope".into()))
        );
    }

    #[test]
    fn scope_count_and_is_empty() {
        let mut svc = StateService::new();
        assert!(svc.is_empty());
        svc.set("a", "1", StateScope::Global);
        svc.set("b", "2", StateScope::Global);
        svc.set("c", "3", StateScope::Window);
        assert!(!svc.is_empty());
        assert_eq!(svc.scope_count(StateScope::Global), 2);
        assert_eq!(svc.scope_count(StateScope::Window), 1);
        assert_eq!(svc.scope_count(StateScope::Workspace), 0);
    }

    #[test]
    fn rename_key_works() {
        let mut svc = StateService::new();
        svc.set("old_name", "data", StateScope::Workspace);
        svc.rename_key("old_name", "new_name").unwrap();
        assert!(!svc.has("old_name"));
        assert_eq!(svc.get("new_name"), Some("data"));
        assert_eq!(svc.get_scope("new_name"), Some(StateScope::Workspace));
    }

    #[test]
    fn rename_key_errors_on_missing() {
        let mut svc = StateService::new();
        assert!(svc.rename_key("no_such", "x").is_err());
    }

    #[test]
    fn increment_works() {
        let mut svc = StateService::new();
        svc.set("counter", "10", StateScope::Global);
        assert_eq!(svc.increment("counter", 5).unwrap(), 15);
        assert_eq!(svc.increment("counter", -3).unwrap(), 12);
        assert_eq!(svc.get("counter"), Some("12"));
    }

    #[test]
    fn increment_treats_non_numeric_as_zero() {
        let mut svc = StateService::new();
        svc.set("bad", "hello", StateScope::Global);
        assert_eq!(svc.increment("bad", 7).unwrap(), 7);
    }

    #[test]
    fn active_scopes_returns_unique_set() {
        let mut svc = StateService::new();
        svc.set("a", "1", StateScope::Window);
        svc.set("b", "2", StateScope::Window);
        svc.set("c", "3", StateScope::Global);
        let scopes = svc.active_scopes();
        assert_eq!(scopes, vec![StateScope::Global, StateScope::Window]);
    }

    #[test]
    fn builder_produces_valid_state() {
        let stored = StoredStateBuilder::new()
            .key("editor.fontSize")
            .value("14")
            .scope(StateScope::Workspace)
            .build()
            .unwrap();
        assert_eq!(stored.key, "editor.fontSize");
        assert_eq!(stored.value, "14");
        assert_eq!(stored.scope, StateScope::Workspace);
    }

    #[test]
    fn builder_rejects_empty_key() {
        let result = StoredStateBuilder::new().value("v").build();
        assert!(result.is_err());
    }

    #[test]
    fn state_service_display_debug() {
        let mut svc = StateService::new();
        svc.set("a", "1", StateScope::Global);
        assert_eq!(format!("{}", svc), "StateService(1 keys)");
        let dbg = format!("{:?}", svc);
        assert!(dbg.contains("key_count"));
    }

    #[test]
    fn state_error_display() {
        let e = StateError::InvalidKey(" ".into());
        assert!(e.to_string().contains("invalid key"));
        let e2 = StateError::ValueTooLong {
            key: "k".into(),
            len: 100,
            max: 50,
        };
        assert!(e2.to_string().contains("100"));
    }

    #[test]
    fn stored_state_partial_eq() {
        let a = StoredState {
            key: "k".into(),
            value: "v".into(),
            scope: StateScope::Global,
        };
        let b = a.clone();
        assert_eq!(a, b);
        let c = StoredState {
            key: "k".into(),
            value: "different".into(),
            scope: StateScope::Global,
        };
        assert_ne!(a, c);
    }

    #[test]
    fn state_stats_new_defaults() {
        let stats = StateStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn state_stats_record_success() {
        let mut stats = StateStats::new();
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
    fn state_stats_record_failure() {
        let mut stats = StateStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn state_stats_reset() {
        let mut stats = StateStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn state_stats_merge() {
        let mut a = StateStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = StateStats::new();
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
    fn state_stats_display() {
        let mut stats = StateStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn state_stats_default() {
        let stats = StateStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn state_validator_accepts_valid_name() {
        let v = StateValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn state_validator_rejects_empty() {
        let v = StateValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn state_validator_rejects_too_long() {
        let v = StateValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn state_validator_forbidden_prefix() {
        let v = StateValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn state_validator_allowed_chars() {
        let v = StateValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn state_validator_range() {
        let v = StateValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn state_sanitize_removes_control() {
        let result = StateValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn state_truncate_short_string() {
        assert_eq!(StateValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn state_truncate_long_string() {
        let result = StateValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn state_is_ascii_printable() {
        assert!(StateValidator::is_ascii_printable("Hello World 123"));
        assert!(!StateValidator::is_ascii_printable("Hello\x00World"));
    }

    // -----------------------------------------------------------------------
    // WorkspaceState / GlobalState / Migration tests
    // -----------------------------------------------------------------------

    #[test]
    fn workspace_state_basic() {
        let mut ws = WorkspaceState::new("ws-123");
        ws.set("theme", "dark");
        assert_eq!(ws.get("theme"), Some("dark"));
        assert_eq!(ws.workspace_id, "ws-123");
        assert_eq!(ws.len(), 1);
    }

    #[test]
    fn workspace_state_remove_and_clear() {
        let mut ws = WorkspaceState::new("ws");
        ws.set("a", "1");
        ws.set("b", "2");
        assert!(ws.remove("a"));
        assert!(!ws.remove("a"));
        assert_eq!(ws.len(), 1);
        ws.clear();
        assert!(ws.is_empty());
    }

    #[test]
    fn workspace_state_export_import() {
        let mut ws = WorkspaceState::new("ws");
        ws.set("key1", "val1");
        ws.set("key2", "val2");
        let exported = ws.export();
        let mut ws2 = WorkspaceState::new("ws2");
        ws2.import(&exported);
        assert_eq!(ws2.get("key1"), Some("val1"));
        assert_eq!(ws2.get("key2"), Some("val2"));
    }

    #[test]
    fn global_state_basic() {
        let mut gs = GlobalState::new();
        assert_eq!(gs.version(), 1);
        gs.set("lastOpened", "/home/user/project");
        assert_eq!(gs.get("lastOpened"), Some("/home/user/project"));
        assert_eq!(gs.len(), 1);
    }

    #[test]
    fn global_state_with_version() {
        let gs = GlobalState::with_version(5);
        assert_eq!(gs.version(), 5);
    }

    #[test]
    fn global_state_display() {
        let mut gs = GlobalState::new();
        gs.set("a", "1");
        let s = format!("{}", gs);
        assert!(s.contains("v1"));
        assert!(s.contains("keys=1"));
    }

    #[test]
    fn state_migration_rename_keys() {
        let mut gs = GlobalState::new();
        gs.set("old.setting", "value");
        gs.set("keep.me", "intact");
        let result = state_migration(&mut gs, &[("old.setting", "new.setting")], &[], 2);
        assert_eq!(gs.get("new.setting"), Some("value"));
        assert!(gs.get("old.setting").is_none());
        assert_eq!(gs.get("keep.me"), Some("intact"));
        assert_eq!(result.keys_renamed, 1);
        assert_eq!(result.final_version, 2);
        assert_eq!(gs.version(), 2);
    }

    #[test]
    fn state_migration_remove_keys() {
        let mut gs = GlobalState::new();
        gs.set("obsolete", "gone");
        gs.set("keep", "stays");
        let result = state_migration(&mut gs, &[], &["obsolete", "nonexistent"], 2);
        assert!(gs.get("obsolete").is_none());
        assert_eq!(gs.get("keep"), Some("stays"));
        assert_eq!(result.keys_removed, 1);
    }

    #[test]
    fn state_migration_combined() {
        let mut gs = GlobalState::new();
        gs.set("old_name", "data");
        gs.set("remove_me", "bye");
        let result = state_migration(&mut gs, &[("old_name", "new_name")], &["remove_me"], 3);
        assert_eq!(gs.get("new_name"), Some("data"));
        assert!(gs.get("remove_me").is_none());
        assert_eq!(result.keys_renamed, 1);
        assert_eq!(result.keys_removed, 1);
    }

    #[test]
    fn migration_needed_check() {
        assert!(migration_needed(1, 2));
        assert!(!migration_needed(2, 2));
        assert!(!migration_needed(3, 2));
    }

    #[test]
    fn workspace_state_display() {
        let ws = WorkspaceState::new("test-ws");
        let s = format!("{}", ws);
        assert!(s.contains("test-ws"));
    }

    #[test]
    fn state_migration_display() {
        let m = StateMigration::new(1, 2, "rename theme keys");
        let s = format!("{}", m);
        assert!(s.contains("v1"));
        assert!(s.contains("v2"));
        assert!(s.contains("rename theme keys"));
    }

    // ── New tests ──

    #[test]
    fn state_snapshot_capture() {
        let mut svc = StateService::new();
        svc.set("theme", "dark", StateScope::Global);
        svc.set("font", "mono", StateScope::Workspace);
        let snap = StateSnapshot::capture(&svc, "test_snap");
        assert_eq!(snap.len(), 2);
        assert_eq!(snap.label, "test_snap");
        assert_eq!(snap.get("theme"), Some("dark"));
        assert_eq!(snap.get("font"), Some("mono"));
        assert_eq!(snap.get("missing"), None);
    }

    #[test]
    fn state_diff_added_removed_changed() {
        let mut svc = StateService::new();
        svc.set("a", "1", StateScope::Global);
        svc.set("b", "2", StateScope::Global);
        let old = StateSnapshot::capture(&svc, "old");

        svc.set("b", "99", StateScope::Global); // changed
        svc.remove("a");                          // removed
        svc.set("c", "3", StateScope::Global);   // added
        let new = StateSnapshot::capture(&svc, "new");

        let diffs = StateDiffEngine::diff(&old, &new);
        assert_eq!(diffs.len(), 3);
        assert!(diffs.iter().any(|d| matches!(d, StateDiff::Removed { key, .. } if key == "a")));
        assert!(diffs.iter().any(|d| matches!(d, StateDiff::Changed { key, old_value, new_value } if key == "b" && old_value == "2" && new_value == "99")));
        assert!(diffs.iter().any(|d| matches!(d, StateDiff::Added { key, .. } if key == "c")));
    }

    #[test]
    fn state_diff_equal_snapshots() {
        let mut svc = StateService::new();
        svc.set("x", "1", StateScope::Global);
        let s1 = StateSnapshot::capture(&svc, "s1");
        let s2 = StateSnapshot::capture(&svc, "s2");
        assert!(StateDiffEngine::is_equal(&s1, &s2));
        assert!(StateDiffEngine::changed_keys(&s1, &s2).is_empty());
    }

    #[test]
    fn state_subscription_matching() {
        let mut sub = StateSubscription::new();
        let id1 = sub.subscribe("editor.");
        let id2 = sub.subscribe("theme.");
        let _id3 = sub.subscribe("editor.font");

        assert_eq!(sub.count(), 3);
        // "editor.fontSize" matches both "editor." and "editor.font" prefixes
        let matches = sub.matching_subscriptions("editor.fontSize");
        assert_eq!(matches.len(), 2);
        assert!(matches.contains(&id1));

        let matches3 = sub.matching_subscriptions("theme.color");
        assert_eq!(matches3.len(), 1);
        assert_eq!(matches3[0], id2);
    }

    #[test]
    fn state_subscription_unsubscribe() {
        let mut sub = StateSubscription::new();
        let id1 = sub.subscribe("a");
        let _id2 = sub.subscribe("b");
        assert_eq!(sub.count(), 2);
        assert!(sub.unsubscribe(id1));
        assert_eq!(sub.count(), 1);
        assert!(!sub.unsubscribe(id1)); // already removed
    }

    #[test]
    fn state_namespace_set_get() {
        let mut svc = StateService::new();
        svc.ns_set("editor", "fontSize", "14", StateScope::Global);
        svc.ns_set("editor", "fontFamily", "Mono", StateScope::Global);
        svc.ns_set("theme", "name", "dark", StateScope::Workspace);

        assert_eq!(svc.ns_get("editor", "fontSize"), Some("14"));
        assert_eq!(svc.ns_get("editor", "fontFamily"), Some("Mono"));
        assert_eq!(svc.ns_get("theme", "name"), Some("dark"));
        assert_eq!(svc.ns_get("theme", "missing"), None);
    }

    #[test]
    fn state_namespace_keys_and_clear() {
        let mut svc = StateService::new();
        svc.ns_set("editor", "a", "1", StateScope::Global);
        svc.ns_set("editor", "b", "2", StateScope::Global);
        svc.ns_set("theme", "c", "3", StateScope::Global);

        let editor_keys = svc.ns_keys("editor");
        assert_eq!(editor_keys.len(), 2);
        assert_eq!(svc.key_count(), 3);

        let removed = svc.ns_clear("editor");
        assert_eq!(removed, 2);
        assert_eq!(svc.key_count(), 1);
        assert_eq!(svc.ns_get("theme", "c"), Some("3"));
    }

    #[test]
    fn state_diff_display() {
        let d = StateDiff::Added { key: "k".into(), value: "v".into() };
        assert_eq!(format!("{}", d), "+ k = v");
        let d2 = StateDiff::Removed { key: "k".into(), value: "v".into() };
        assert_eq!(format!("{}", d2), "- k = v");
        let d3 = StateDiff::Changed { key: "k".into(), old_value: "a".into(), new_value: "b".into() };
        assert_eq!(format!("{}", d3), "~ k : a -> b");
    }

    // -- New tests ----------------------------------------------------------

    #[test]
    fn search_values_finds_matches() {
        let mut svc = StateService::new();
        svc.set("theme", "dark-mode", StateScope::Global);
        svc.set("font", "monospace", StateScope::Global);
        svc.set("editor.mode", "dark", StateScope::Workspace);

        let results = svc.search_values("dark");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn total_value_bytes_sums_correctly() {
        let mut svc = StateService::new();
        svc.set("a", "hello", StateScope::Global);     // 5 bytes
        svc.set("b", "world!!", StateScope::Global);    // 7 bytes
        assert_eq!(svc.total_value_bytes(), 12);
    }

    #[test]
    fn sorted_keys_returns_alphabetical() {
        let mut svc = StateService::new();
        svc.set("z", "1", StateScope::Global);
        svc.set("a", "2", StateScope::Global);
        svc.set("m", "3", StateScope::Global);
        assert_eq!(svc.sorted_keys(), vec!["a", "m", "z"]);
    }

    #[test]
    fn retain_removes_non_matching() {
        let mut svc = StateService::new();
        svc.set("keep.a", "1", StateScope::Global);
        svc.set("keep.b", "2", StateScope::Global);
        svc.set("remove.c", "3", StateScope::Global);
        let removed = svc.retain(|k, _| k.starts_with("keep"));
        assert_eq!(removed, 1);
        assert_eq!(svc.key_count(), 2);
    }

    #[test]
    fn copy_scope_copies_entries() {
        let mut svc = StateService::new();
        svc.set("a", "1", StateScope::Global);
        svc.set("b", "2", StateScope::Global);
        svc.set("c", "3", StateScope::Workspace);
        let copied = svc.copy_scope(StateScope::Global, StateScope::Window);
        assert_eq!(copied, 2);
        assert_eq!(svc.get_scope("a"), Some(StateScope::Window));
    }

    #[test]
    fn export_import_round_trip() {
        let mut svc = StateService::new();
        svc.set("alpha", "one", StateScope::Global);
        svc.set("beta", "two", StateScope::Global);
        let exported = export_state(&svc);
        assert!(exported.contains("alpha=one"));
        assert!(exported.contains("beta=two"));

        let mut svc2 = StateService::new();
        let imported = import_state(&mut svc2, &exported, StateScope::Workspace);
        assert_eq!(imported, 2);
        assert_eq!(svc2.get("alpha"), Some("one"));
        assert_eq!(svc2.get("beta"), Some("two"));
        assert_eq!(svc2.get_scope("alpha"), Some(StateScope::Workspace));
    }
}
