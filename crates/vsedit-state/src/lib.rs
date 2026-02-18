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
pub struct StateMigrationEntry {
    pub from_version: u32,
    pub to_version: u32,
    pub description: String,
}

impl StateMigrationEntry {
    pub fn new(from: u32, to: u32, description: impl Into<String>) -> Self {
        Self { from_version: from, to_version: to, description: description.into() }
    }
}

impl fmt::Display for StateMigrationEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Migration v{} -> v{}: {}", self.from_version, self.to_version, self.description)
    }
}

/// Result of applying a migration chain.
#[derive(Debug, Clone)]
pub struct MigrationResult {
    pub applied: Vec<StateMigrationEntry>,
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
        applied: vec![StateMigrationEntry::new(from_version, target_version, "state migration")],
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

// ---------------------------------------------------------------------------
// Dirty tracking – knows whether state has been modified since last save
// ---------------------------------------------------------------------------

/// Wraps a `StateService` with change-tracking so callers know when
/// state has been modified since the last acknowledged save point.
pub struct DirtyTracker {
    dirty: bool,
    changes_since_save: u64,
}

impl DirtyTracker {
    pub fn new() -> Self {
        Self { dirty: false, changes_since_save: 0 }
    }

    /// Mark the state as dirty (a mutation happened).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.changes_since_save += 1;
    }

    /// Acknowledge a save – resets the dirty flag and change counter.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
        self.changes_since_save = 0;
    }

    /// Returns `true` if state has been modified since the last save.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Number of mutations since the last save.
    pub fn changes_since_save(&self) -> u64 {
        self.changes_since_save
    }
}

impl Default for DirtyTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// State value type-conversion helpers
// ---------------------------------------------------------------------------

/// Helpers that parse stored string values into typed Rust values.
pub struct StateValueParser;

impl StateValueParser {
    /// Parse a stored value as `i64`.
    pub fn as_i64(value: &str) -> Option<i64> {
        value.parse().ok()
    }

    /// Parse a stored value as `f64`.
    pub fn as_f64(value: &str) -> Option<f64> {
        value.parse().ok()
    }

    /// Parse a stored value as `bool` (accepts "true"/"false"/"1"/"0").
    pub fn as_bool(value: &str) -> Option<bool> {
        match value {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        }
    }

    /// Parse a comma-separated value into a `Vec<String>`.
    pub fn as_string_list(value: &str) -> Vec<String> {
        if value.is_empty() {
            return Vec::new();
        }
        value.split(',').map(|s| s.trim().to_string()).collect()
    }

    /// Encode a list of strings as a comma-separated value.
    pub fn from_string_list(items: &[&str]) -> String {
        items.join(",")
    }
}

// ---------------------------------------------------------------------------
// Typed getters on StateService
// ---------------------------------------------------------------------------

impl StateService {
    /// Get a value parsed as `i64`.
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(StateValueParser::as_i64)
    }

    /// Get a value parsed as `f64`.
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(StateValueParser::as_f64)
    }

    /// Get a value parsed as `bool`.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(StateValueParser::as_bool)
    }

    /// Get a value parsed as a comma-separated list.
    pub fn get_string_list(&self, key: &str) -> Vec<String> {
        self.get(key)
            .map(StateValueParser::as_string_list)
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Snapshot restore on StateService
// ---------------------------------------------------------------------------

impl StateService {
    /// Restore state from a previously captured `StateSnapshot`, replacing
    /// all current entries.
    pub fn restore(&mut self, snapshot: &StateSnapshot) {
        self.state = snapshot.entries.clone();
    }

    /// Apply a diff to the current state (replay additions, removals, changes).
    pub fn apply_diff(&mut self, diffs: &[StateDiff]) {
        for diff in diffs {
            match diff {
                StateDiff::Added { key, value } => {
                    // Additions default to Global scope
                    self.set(key.clone(), value.clone(), StateScope::Global);
                }
                StateDiff::Removed { key, .. } => {
                    self.remove(key);
                }
                StateDiff::Changed { key, new_value, .. } => {
                    if let Some(entry) = self.state.get_mut(key) {
                        entry.value = new_value.clone();
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Prefix queries on StateService
// ---------------------------------------------------------------------------

impl StateService {
    /// Return all entries whose key starts with the given prefix.
    pub fn prefix_query(&self, prefix: &str) -> Vec<(&str, &str)> {
        self.state
            .values()
            .filter(|s| s.key.starts_with(prefix))
            .map(|s| (s.key.as_str(), s.value.as_str()))
            .collect()
    }

    /// Remove all entries whose key starts with the given prefix.
    /// Returns the number of entries removed.
    pub fn prefix_remove(&mut self, prefix: &str) -> usize {
        let keys: Vec<String> = self.state
            .keys()
            .filter(|k| k.starts_with(prefix))
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
// Batch operations on StateService
// ---------------------------------------------------------------------------

impl StateService {
    /// Atomically set multiple key-value pairs, rolling back all changes
    /// if any key fails validation.
    pub fn try_set_batch(
        &mut self,
        entries: &[(&str, &str, StateScope)],
    ) -> Result<usize, StateError> {
        // Validate everything first
        for (key, value, _scope) in entries {
            validate_key(key)?;
            validate_value(key, value)?;
        }
        // All valid – apply
        for (key, value, scope) in entries {
            self.set(*key, *value, *scope);
        }
        Ok(entries.len())
    }

    /// Remove multiple keys at once. Returns the number actually removed.
    pub fn remove_batch(&mut self, keys: &[&str]) -> usize {
        keys.iter().filter(|k| self.state.remove(**k).is_some()).count()
    }

    /// Swap the values of two keys. Both must exist.
    pub fn swap(&mut self, key_a: &str, key_b: &str) -> Result<(), StateError> {
        let val_a = self.state.get(key_a)
            .ok_or_else(|| StateError::KeyNotFound(key_a.to_string()))?
            .value.clone();
        let val_b = self.state.get(key_b)
            .ok_or_else(|| StateError::KeyNotFound(key_b.to_string()))?
            .value.clone();

        self.state.get_mut(key_a).unwrap().value = val_b;
        self.state.get_mut(key_b).unwrap().value = val_a;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// State merge strategies
// ---------------------------------------------------------------------------

/// Strategy for resolving conflicts when merging two `StateService` instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Incoming values overwrite existing values on key collision.
    Overwrite,
    /// Existing values are kept on key collision.
    KeepExisting,
    /// Only merge keys that don't already exist.
    AddOnly,
}

impl StateService {
    /// Merge another `StateService` using the specified strategy.
    pub fn merge_with_strategy(&mut self, other: &StateService, strategy: MergeStrategy) {
        for (key, stored) in &other.state {
            match strategy {
                MergeStrategy::Overwrite => {
                    self.state.insert(key.clone(), stored.clone());
                }
                MergeStrategy::KeepExisting => {
                    if !self.state.contains_key(key) {
                        self.state.insert(key.clone(), stored.clone());
                    }
                }
                MergeStrategy::AddOnly => {
                    if !self.state.contains_key(key) {
                        self.state.insert(key.clone(), stored.clone());
                    }
                }
            }
        }
    }
}


// ---------------------------------------------------------------------------
// StateSnapshotDiff
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StateSnapshotDiff {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl StateSnapshotDiff {
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

impl Default for StateSnapshotDiff {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for StateSnapshotDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "StateSnapshotDiff({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// StateMigrationHandler
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StateMigrationHandler {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl StateMigrationHandler {
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

impl Default for StateMigrationHandler {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for StateMigrationHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "StateMigrationHandler({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// StateSnapshotDiffSnapshot — point-in-time snapshot of StateSnapshotDiff state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StateSnapshotDiffSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl StateSnapshotDiffSnapshot {
    pub fn capture(source: &StateSnapshotDiff, timestamp: u64) -> Self {
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

impl fmt::Display for StateSnapshotDiffSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// StateMigrationHandlerStats — aggregate statistics for StateMigrationHandler
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct StateMigrationHandlerStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl StateMigrationHandlerStats {
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

impl fmt::Display for StateMigrationHandlerStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// StateSnapshotDiffConfig — configuration for StateSnapshotDiff
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StateSnapshotDiffConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl StateSnapshotDiffConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for StateSnapshotDiffConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for StateSnapshotDiffConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// StateMigration – migrate state between versions
// ---------------------------------------------------------------------------

/// A migration from one state version to another.
pub struct StateMigration {
    migrations: Vec<(u32, Box<dyn Fn(&mut HashMap<String, String>)>)>,
}

impl StateMigration {
    pub fn new() -> Self {
        Self { migrations: Vec::new() }
    }

    /// Register a migration for a specific version.
    pub fn register_migration<F: Fn(&mut HashMap<String, String>) + 'static>(
        &mut self,
        version: u32,
        migrator: F,
    ) {
        self.migrations.push((version, Box::new(migrator)));
        self.migrations.sort_by_key(|(v, _)| *v);
    }

    /// Check if data at `current_version` needs migration.
    pub fn needs_migration(&self, current_version: u32) -> bool {
        self.migrations.iter().any(|(v, _)| *v > current_version)
    }

    /// Apply all migrations from `current_version` up to `target_version`.
    pub fn apply_migrations_up_to(
        &self,
        data: &mut HashMap<String, String>,
        current_version: u32,
        target_version: u32,
    ) -> u32 {
        let mut applied_up_to = current_version;
        for (v, migrator) in &self.migrations {
            if *v > current_version && *v <= target_version {
                migrator(data);
                applied_up_to = *v;
            }
        }
        applied_up_to
    }

    /// Return the highest registered migration version.
    pub fn current_version(&self) -> u32 {
        self.migrations.last().map(|(v, _)| *v).unwrap_or(0)
    }

    pub fn migration_count(&self) -> usize {
        self.migrations.len()
    }
}

// ---------------------------------------------------------------------------
// StateSnapshotCapture – capture state at a point in time
// ---------------------------------------------------------------------------

/// A captured snapshot of state data.
#[derive(Debug, Clone)]
pub struct StateSnapshotCapture {
    pub id: u64,
    pub timestamp: u64,
    pub data: HashMap<String, String>,
}

impl StateSnapshotCapture {
    pub fn new(id: u64, timestamp: u64, data: HashMap<String, String>) -> Self {
        Self { id, timestamp, data }
    }

    /// Restore state from this snapshot into the given map.
    pub fn restore(&self, target: &mut HashMap<String, String>) {
        target.clear();
        for (k, v) in &self.data {
            target.insert(k.clone(), v.clone());
        }
    }

    /// Compute changed keys between this snapshot and a previous one.
    pub fn diff_from(&self, previous: &StateSnapshotCapture) -> Vec<String> {
        let mut changed = Vec::new();
        for (k, v) in &self.data {
            match previous.data.get(k) {
                None => changed.push(k.clone()),
                Some(old_v) if old_v != v => changed.push(k.clone()),
                _ => {}
            }
        }
        for k in previous.data.keys() {
            if !self.data.contains_key(k) {
                changed.push(k.clone());
            }
        }
        changed
    }

    pub fn snapshot_size(&self) -> usize {
        self.data.iter().map(|(k, v)| k.len() + v.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// StateScopeHierarchy – hierarchical scoped state access
// ---------------------------------------------------------------------------

/// A hierarchical scope with inheritance for state lookups.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StateScopeLevel {
    GlobalLevel,
    WorkspaceLevel,
    WindowLevel,
    EditorLevel,
}

impl StateScopeLevel {
    /// Return the parent scope, if any.
    pub fn parent_scope(&self) -> Option<StateScopeLevel> {
        match self {
            StateScopeLevel::EditorLevel => Some(StateScopeLevel::WindowLevel),
            StateScopeLevel::WindowLevel => Some(StateScopeLevel::WorkspaceLevel),
            StateScopeLevel::WorkspaceLevel => Some(StateScopeLevel::GlobalLevel),
            StateScopeLevel::GlobalLevel => None,
        }
    }

    /// Look up a key, walking up the hierarchy.
    pub fn inherit_from<'a>(
        &self,
        key: &str,
        stores: &'a HashMap<StateScopeLevel, HashMap<String, String>>,
    ) -> Option<&'a str> {
        if let Some(store) = stores.get(self) {
            if let Some(v) = store.get(key) {
                return Some(v.as_str());
            }
        }
        self.parent_scope().and_then(|p| p.inherit_from(key, stores))
    }

    /// Check if this scope is a descendant of another.
    pub fn is_descendant(&self, ancestor: &StateScopeLevel) -> bool {
        let mut current = self.parent_scope();
        while let Some(scope) = current {
            if scope == *ancestor {
                return true;
            }
            current = scope.parent_scope();
        }
        false
    }
}


/// Configuration manager for state functionality.
pub struct StateConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl StateConfig {
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

    pub fn merge(&mut self, other: &StateConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for state operations.
pub struct StateRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl StateRateTracker {
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

/// Validation result collector for state.
pub struct StateValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl StateValidationCollector {
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

    pub fn merge(&mut self, other: &StateValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Global and scoped state management — extended utilities (yj)
// ---------------------------------------------------------------------------

/// Metric accumulator for state operations.
#[derive(Debug, Clone)]
pub struct YjMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YjMetrics {
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

/// Sliding-window rate counter for state.
#[derive(Debug, Clone)]
pub struct YjRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YjRateWindow {
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

/// A small LRU-style cache for state lookups.
#[derive(Debug, Clone)]
pub struct YjLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YjLruCache {
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
// xa_ extended helpers for state
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaStateRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaStateRingBuf {
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
pub struct XaStateCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaStateCounter {
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

impl Default for XaStateCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 163
// ---------------------------------------------------------------------------

/// Generic object pool `Xc163Pool<T>`.
pub struct Xc163Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc163Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc163PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc163Pool<T> {
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
    pub fn stats(&self) -> Xc163PoolStats {
        Xc163PoolStats {
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

impl<T> Default for Xc163Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc163Scheduler`.
pub struct Xc163Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc163Scheduler {
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

impl Default for Xc163Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_163 hash for the given byte slice.
pub fn xc_163_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_163 convention.
pub fn xc_163_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_87 deepening: state machine + event bus ---

/// States for the Xd87 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd87State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd87State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd87Transition {
    pub from: Xd87State,
    pub to: Xd87State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd87StateMachine {
    current: Xd87State,
    history: Vec<Xd87Transition>,
    step_counter: usize,
}

impl Xd87StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd87State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd87State {
        self.current
    }

    pub fn history(&self) -> &[Xd87Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd87State) -> Result<Xd87State, String> {
        let allowed = match (self.current, target) {
            (Xd87State::Idle, Xd87State::Running) => true,
            (Xd87State::Running, Xd87State::Paused) => true,
            (Xd87State::Running, Xd87State::Done) => true,
            (Xd87State::Paused, Xd87State::Running) => true,
            (Xd87State::Paused, Xd87State::Done) => true,
            (Xd87State::Done, Xd87State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_87: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd87Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd87SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd87State> {
        let prefix = "Xd87SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd87State::Idle),
            "Running" => Some(Xd87State::Running),
            "Paused" => Some(Xd87State::Paused),
            "Done" => Some(Xd87State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd87State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd87 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd87Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd87Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd87HandlerFn = Box<dyn Fn(&Xd87Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd87EventBus {
    handlers: Vec<(usize, Option<String>, Xd87HandlerFn)>,
    next_id: usize,
    published: Vec<Xd87Event>,
}

impl Xd87EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd87Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd87Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd87Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd87Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #109
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf109Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf109TrieNode {
    children: std::collections::HashMap<char, Xf109TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf109Trie {
    root: Xf109TrieNode,
    count: usize,
}

impl Xf109Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf109TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf109TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf109TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf109BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf109BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 162).
pub struct Xh162SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh162SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 204 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 162).
pub struct Xh162BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh162BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 162).
pub struct Xi162Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi162Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi162Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi162Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 162).
pub struct Xi162IntervalTree {
    xi_intervals: Vec<Xi162Interval>,
}

impl Xi162IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi162Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi162Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi162Interval) -> Vec<&Xi162Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi162Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi162Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi162Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi162Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi162Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi162Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 162) ---

/// Disjoint set / union-find for crate 162.
pub struct Xj162UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj162UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ162_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 162.
pub struct Xj162BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj162BTreeNode<K, V>>>,
    len: usize,
}

struct Xj162BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj162BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj162BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ162_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ162_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj162BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj162BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj162BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj162BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_162 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk162SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk162SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk162DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk162DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_162).
#[derive(Debug, Clone)]
pub struct Xl162Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl162Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_162).
#[derive(Debug, Clone)]
pub struct Xl162SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl162SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm162MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm162MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm162Tokenizer {
    text: String,
}

impl Xm162Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 162.
pub struct Xn162Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn162Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 162 -----

#[derive(Debug, Clone)]
struct Xn162AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn162AvlNode<K, V>>>,
    right: Option<Box<Xn162AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 162.
#[derive(Debug, Clone)]
pub struct Xn162AVL<K, V> {
    root: Option<Box<Xn162AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn162AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn162AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn162AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn162AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn162AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn162AvlNode<K, V>>) -> Box<Xn162AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn162AvlNode<K, V>>) -> Box<Xn162AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn162AvlNode<K, V>>) -> Box<Xn162AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn162AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn162AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn162AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn162AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn162AvlNode<K, V>>) -> &Xn162AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn162AvlNode<K, V>>) -> (Box<Xn162AvlNode<K, V>>, Option<Box<Xn162AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn162AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn162AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn162AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn162AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn162AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn162AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn162AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo162RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo162Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo162RBNode<K, V> {
    key: K,
    value: V,
    color: Xo162Color,
    left: Option<Box<Xo162RBNode<K, V>>>,
    right: Option<Box<Xo162RBNode<K, V>>>,
}

/// A red-black tree map for crate 162.
#[derive(Debug, Clone)]
pub struct Xo162RedBlack<K, V> {
    root: Option<Box<Xo162RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo162RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo162Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo162RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo162RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo162RBNode {
                    key, value, color: Xo162Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo162RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo162Color::Red)
    }

    fn xo_balance(mut h: Box<Xo162RBNode<K, V>>) -> Box<Xo162RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo162Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo162RBNode<K, V>>) -> Box<Xo162RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo162Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo162RBNode<K, V>>) -> Box<Xo162RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo162Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo162RBNode<K, V>>) {
        h.color = Xo162Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo162Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo162Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo162Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo162RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo162RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo162RBNode<K, V>) -> (K, V, Option<Box<Xo162RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo162RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo162Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo162RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo162ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 162.
#[derive(Debug, Clone)]
pub struct Xo162ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo162ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo162#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo162#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 162).
#[derive(Debug)]
pub struct Xp162SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp162Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp162Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp162Node<K, V>>>,
    xp_right: Option<Box<Xp162Node<K, V>>>,
}

impl<K: Ord, V> Xp162Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp162SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp162SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp162Node<K, V>>>, key: &K) -> Option<Box<Xp162Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp162Node<K, V>>) -> Box<Xp162Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp162Node<K, V>>) -> Box<Xp162Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp162Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp162Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp162Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
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
        let m = StateMigrationEntry::new(1, 2, "rename theme keys");
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

    // ── DirtyTracker, typed getters, restore, prefix, batch, merge strategy ──

    #[test]
    fn dirty_tracker_lifecycle() {
        let mut dt = DirtyTracker::new();
        assert!(!dt.is_dirty());
        assert_eq!(dt.changes_since_save(), 0);

        dt.mark_dirty();
        dt.mark_dirty();
        assert!(dt.is_dirty());
        assert_eq!(dt.changes_since_save(), 2);

        dt.mark_saved();
        assert!(!dt.is_dirty());
        assert_eq!(dt.changes_since_save(), 0);
    }

    #[test]
    fn value_parser_i64() {
        assert_eq!(StateValueParser::as_i64("42"), Some(42));
        assert_eq!(StateValueParser::as_i64("-7"), Some(-7));
        assert_eq!(StateValueParser::as_i64("nope"), None);
    }

    #[test]
    fn value_parser_f64() {
        assert!((StateValueParser::as_f64("3.14").unwrap() - 3.14).abs() < f64::EPSILON);
        assert_eq!(StateValueParser::as_f64("bad"), None);
    }

    #[test]
    fn value_parser_bool() {
        assert_eq!(StateValueParser::as_bool("true"), Some(true));
        assert_eq!(StateValueParser::as_bool("false"), Some(false));
        assert_eq!(StateValueParser::as_bool("1"), Some(true));
        assert_eq!(StateValueParser::as_bool("0"), Some(false));
        assert_eq!(StateValueParser::as_bool("yes"), None);
    }

    #[test]
    fn value_parser_string_list_round_trip() {
        let encoded = StateValueParser::from_string_list(&["a", "b", "c"]);
        assert_eq!(encoded, "a,b,c");
        let decoded = StateValueParser::as_string_list(&encoded);
        assert_eq!(decoded, vec!["a", "b", "c"]);
        assert!(StateValueParser::as_string_list("").is_empty());
    }

    #[test]
    fn typed_getters_on_state_service() {
        let mut svc = StateService::new();
        svc.set("count", "42", StateScope::Global);
        svc.set("ratio", "2.5", StateScope::Global);
        svc.set("enabled", "true", StateScope::Global);
        svc.set("tags", "rust,editor,fast", StateScope::Global);

        assert_eq!(svc.get_i64("count"), Some(42));
        assert!((svc.get_f64("ratio").unwrap() - 2.5).abs() < f64::EPSILON);
        assert_eq!(svc.get_bool("enabled"), Some(true));
        assert_eq!(svc.get_string_list("tags"), vec!["rust", "editor", "fast"]);

        assert_eq!(svc.get_i64("missing"), None);
        assert!(svc.get_string_list("missing").is_empty());
    }

    #[test]
    fn restore_from_snapshot() {
        let mut svc = StateService::new();
        svc.set("a", "1", StateScope::Global);
        svc.set("b", "2", StateScope::Workspace);
        let snap = StateSnapshot::capture(&svc, "checkpoint");

        // Mutate
        svc.set("a", "changed", StateScope::Global);
        svc.set("c", "new", StateScope::Window);
        svc.remove("b");
        assert_eq!(svc.key_count(), 2);

        // Restore
        svc.restore(&snap);
        assert_eq!(svc.key_count(), 2);
        assert_eq!(svc.get("a"), Some("1"));
        assert_eq!(svc.get("b"), Some("2"));
        assert!(!svc.has("c"));
    }

    #[test]
    fn apply_diff_replays_changes() {
        let mut svc = StateService::new();
        svc.set("x", "10", StateScope::Global);
        svc.set("y", "20", StateScope::Global);

        let diffs = vec![
            StateDiff::Added { key: "z".into(), value: "30".into() },
            StateDiff::Changed { key: "x".into(), old_value: "10".into(), new_value: "99".into() },
            StateDiff::Removed { key: "y".into(), value: "20".into() },
        ];
        svc.apply_diff(&diffs);

        assert_eq!(svc.get("x"), Some("99"));
        assert!(!svc.has("y"));
        assert_eq!(svc.get("z"), Some("30"));
    }

    #[test]
    fn prefix_query_and_remove() {
        let mut svc = StateService::new();
        svc.set("editor.fontSize", "14", StateScope::Global);
        svc.set("editor.tabSize", "4", StateScope::Global);
        svc.set("theme.name", "dark", StateScope::Global);

        let editor_entries = svc.prefix_query("editor.");
        assert_eq!(editor_entries.len(), 2);

        let removed = svc.prefix_remove("editor.");
        assert_eq!(removed, 2);
        assert_eq!(svc.key_count(), 1);
        assert_eq!(svc.get("theme.name"), Some("dark"));
    }

    #[test]
    fn try_set_batch_validates_then_applies() {
        let mut svc = StateService::new();
        let ok = svc.try_set_batch(&[
            ("a", "1", StateScope::Global),
            ("b", "2", StateScope::Workspace),
        ]);
        assert_eq!(ok.unwrap(), 2);
        assert_eq!(svc.key_count(), 2);

        // Batch with an invalid key should reject everything
        let mut svc2 = StateService::new();
        let err = svc2.try_set_batch(&[
            ("good", "val", StateScope::Global),
            ("", "bad_key", StateScope::Global),
        ]);
        assert!(err.is_err());
        assert_eq!(svc2.key_count(), 0); // nothing applied
    }

    #[test]
    fn remove_batch_removes_multiple() {
        let mut svc = StateService::new();
        svc.set("a", "1", StateScope::Global);
        svc.set("b", "2", StateScope::Global);
        svc.set("c", "3", StateScope::Global);
        let removed = svc.remove_batch(&["a", "c", "nonexistent"]);
        assert_eq!(removed, 2);
        assert_eq!(svc.key_count(), 1);
        assert_eq!(svc.get("b"), Some("2"));
    }

    #[test]
    fn swap_exchanges_values() {
        let mut svc = StateService::new();
        svc.set("x", "alpha", StateScope::Global);
        svc.set("y", "beta", StateScope::Workspace);
        svc.swap("x", "y").unwrap();
        assert_eq!(svc.get("x"), Some("beta"));
        assert_eq!(svc.get("y"), Some("alpha"));
        // Scopes unchanged
        assert_eq!(svc.get_scope("x"), Some(StateScope::Global));
        assert_eq!(svc.get_scope("y"), Some(StateScope::Workspace));
    }

    #[test]
    fn swap_error_on_missing_key() {
        let mut svc = StateService::new();
        svc.set("a", "1", StateScope::Global);
        assert!(svc.swap("a", "missing").is_err());
    }

    #[test]
    fn merge_with_strategy_overwrite() {
        let mut a = StateService::new();
        a.set("k", "old", StateScope::Global);
        let mut b = StateService::new();
        b.set("k", "new", StateScope::Workspace);
        b.set("k2", "extra", StateScope::Global);

        a.merge_with_strategy(&b, MergeStrategy::Overwrite);
        assert_eq!(a.get("k"), Some("new"));
        assert_eq!(a.get("k2"), Some("extra"));
    }

    #[test]
    fn merge_with_strategy_keep_existing() {
        let mut a = StateService::new();
        a.set("k", "old", StateScope::Global);
        let mut b = StateService::new();
        b.set("k", "new", StateScope::Workspace);
        b.set("k2", "extra", StateScope::Global);

        a.merge_with_strategy(&b, MergeStrategy::KeepExisting);
        assert_eq!(a.get("k"), Some("old")); // kept
        assert_eq!(a.get("k2"), Some("extra")); // added
    }

    #[test]
    fn merge_with_strategy_add_only() {
        let mut a = StateService::new();
        a.set("existing", "keep_me", StateScope::Global);
        let mut b = StateService::new();
        b.set("existing", "ignored", StateScope::Workspace);
        b.set("new_key", "added", StateScope::Global);

        a.merge_with_strategy(&b, MergeStrategy::AddOnly);
        assert_eq!(a.get("existing"), Some("keep_me"));
        assert_eq!(a.get("new_key"), Some("added"));
    }

    #[test] fn stateSnapshotDiff_new() { let s = StateSnapshotDiff::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn stateSnapshotDiff_add() { let mut s = StateSnapshotDiff::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn stateSnapshotDiff_remove() { let mut s = StateSnapshotDiff::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn stateSnapshotDiff_config() { let mut s = StateSnapshotDiff::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn stateSnapshotDiff_nav() { let mut s = StateSnapshotDiff::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn stateSnapshotDiff_filter() { let mut s = StateSnapshotDiff::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn stateSnapshotDiff_display() { assert!(format!("{}", StateSnapshotDiff::new()).contains("StateSnapshotDiff")); }
    #[test] fn stateMigrationHandler_new() { let s = StateMigrationHandler::new(); assert!(s.is_empty()); }
    #[test] fn stateMigrationHandler_add() { let mut s = StateMigrationHandler::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn stateMigrationHandler_active() { let mut s = StateMigrationHandler::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn stateMigrationHandler_error() { let mut s = StateMigrationHandler::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn stateMigrationHandler_rm_group() { let mut s = StateMigrationHandler::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn stateMigrationHandler_display() { assert!(format!("{}", StateMigrationHandler::new()).contains("StateMigrationHandler")); }


    #[test] fn stateSnapshotDiff_snap_capture() {
        let s = StateSnapshotDiff::new();
        let snap = StateSnapshotDiffSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn stateSnapshotDiff_snap_stale() {
        let s = StateSnapshotDiff::new();
        let snap = StateSnapshotDiffSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn stateSnapshotDiff_snap_diff() {
        let s = StateSnapshotDiff::new();
        let s1v = StateSnapshotDiffSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn stateSnapshotDiff_snap_display() {
        let s = StateSnapshotDiff::new();
        let snap = StateSnapshotDiffSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn stateMigrationHandler_stats_record() {
        let mut st = StateMigrationHandlerStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn stateMigrationHandler_stats_hit_ratio() {
        let mut st = StateMigrationHandlerStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn stateMigrationHandler_stats_merge() {
        let mut a = StateMigrationHandlerStats::new();
        a.total_adds = 5;
        let mut b = StateMigrationHandlerStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn stateMigrationHandler_stats_display() {
        let st = StateMigrationHandlerStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn stateSnapshotDiff_config_default() {
        let c = StateSnapshotDiffConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn stateSnapshotDiff_config_builder() {
        let c = StateSnapshotDiffConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn stateSnapshotDiff_config_labels() {
        let mut c = StateSnapshotDiffConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn stateSnapshotDiff_config_cleanup_threshold() {
        let c = StateSnapshotDiffConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn stateSnapshotDiff_config_display() {
        assert!(format!("{}", StateSnapshotDiffConfig::new()).contains("Config"));
    }
    #[test] fn stateMigrationHandler_stats_peaks() {
        let mut st = StateMigrationHandlerStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- StateMigration -----------------------------------------------------

    #[test]
    fn migration_register_and_count() {
        let mut m = StateMigration::new();
        m.register_migration(1, |data| { data.insert("migrated".into(), "1".into()); });
        m.register_migration(2, |data| { data.insert("v2".into(), "yes".into()); });
        assert_eq!(m.migration_count(), 2);
        assert_eq!(m.current_version(), 2);
    }

    #[test]
    fn migration_needs_migration() {
        let mut m = StateMigration::new();
        m.register_migration(2, |_| {});
        assert!(m.needs_migration(1));
        assert!(!m.needs_migration(2));
    }

    #[test]
    fn migration_apply() {
        let mut m = StateMigration::new();
        m.register_migration(1, |d| { d.insert("a".into(), "1".into()); });
        m.register_migration(2, |d| { d.insert("b".into(), "2".into()); });
        let mut data = HashMap::new();
        let version = m.apply_migrations_up_to(&mut data, 0, 2);
        assert_eq!(version, 2);
        assert_eq!(data.get("a").unwrap(), "1");
        assert_eq!(data.get("b").unwrap(), "2");
    }

    #[test]
    fn migration_partial_apply() {
        let mut m = StateMigration::new();
        m.register_migration(1, |d| { d.insert("a".into(), "1".into()); });
        m.register_migration(3, |d| { d.insert("c".into(), "3".into()); });
        let mut data = HashMap::new();
        let version = m.apply_migrations_up_to(&mut data, 0, 2);
        assert_eq!(version, 1);
        assert!(data.contains_key("a"));
        assert!(!data.contains_key("c"));
    }

    // -- StateSnapshotCapture -----------------------------------------------

    #[test]
    fn snapshot_restore() {
        let data: HashMap<String, String> = [("k".into(), "v".into())].into_iter().collect();
        let snap = StateSnapshotCapture::new(1, 1000, data);
        let mut target = HashMap::new();
        target.insert("old".into(), "val".into());
        snap.restore(&mut target);
        assert_eq!(target.get("k").unwrap(), "v");
        assert!(!target.contains_key("old"));
    }

    #[test]
    fn snapshot_diff() {
        let old_data: HashMap<String, String> = [("a".into(), "1".into()), ("b".into(), "2".into())].into_iter().collect();
        let new_data: HashMap<String, String> = [("a".into(), "1".into()), ("b".into(), "3".into()), ("c".into(), "4".into())].into_iter().collect();
        let old_snap = StateSnapshotCapture::new(1, 100, old_data);
        let new_snap = StateSnapshotCapture::new(2, 200, new_data);
        let diff = new_snap.diff_from(&old_snap);
        assert!(diff.contains(&"b".to_string()));
        assert!(diff.contains(&"c".to_string()));
    }

    #[test]
    fn snapshot_size() {
        let data: HashMap<String, String> = [("key".into(), "value".into())].into_iter().collect();
        let snap = StateSnapshotCapture::new(1, 0, data);
        assert_eq!(snap.snapshot_size(), 8); // "key" + "value" = 3 + 5
    }

    // -- StateScopeLevel ----------------------------------------------------

    #[test]
    fn scope_parent() {
        assert_eq!(StateScopeLevel::EditorLevel.parent_scope(), Some(StateScopeLevel::WindowLevel));
        assert_eq!(StateScopeLevel::GlobalLevel.parent_scope(), None);
    }

    #[test]
    fn scope_is_descendant() {
        assert!(StateScopeLevel::EditorLevel.is_descendant(&StateScopeLevel::GlobalLevel));
        assert!(!StateScopeLevel::GlobalLevel.is_descendant(&StateScopeLevel::EditorLevel));
    }

    #[test]
    fn scope_inherit_from() {
        let mut stores = HashMap::new();
        let mut global = HashMap::new();
        global.insert("theme".to_string(), "dark".to_string());
        stores.insert(StateScopeLevel::GlobalLevel, global);
        stores.insert(StateScopeLevel::EditorLevel, HashMap::new());
        let val = StateScopeLevel::EditorLevel.inherit_from("theme", &stores);
        assert_eq!(val, Some("dark"));
    }


    #[test]
    fn state_config_new() {
        let cfg = StateConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn state_config_set_get() {
        let mut cfg = StateConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn state_config_remove() {
        let mut cfg = StateConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn state_config_keys_sorted() {
        let mut cfg = StateConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn state_config_bump_version() {
        let mut cfg = StateConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn state_config_clear() {
        let mut cfg = StateConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn state_config_merge() {
        let mut cfg1 = StateConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = StateConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn state_config_disable() {
        let mut cfg = StateConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn state_rate_tracker_empty() {
        let rt = StateRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn state_rate_tracker_record() {
        let mut rt = StateRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn state_rate_tracker_prune() {
        let mut rt = StateRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn state_validator_valid() {
        let v = StateValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn state_validator_errors() {
        let mut v = StateValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn state_validator_clear() {
        let mut v = StateValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn state_validator_merge() {
        let mut v1 = StateValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = StateValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn state_rate_tracker_clear() {
        let mut rt = StateRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yj_metrics_empty() {
        let m = YjMetrics::new("state");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yj_metrics_record_and_mean() {
        let mut m = YjMetrics::new("state");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yj_metrics_min_max() {
        let mut m = YjMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yj_metrics_variance_and_std() {
        let mut m = YjMetrics::new("v");
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
    fn yj_metrics_percentile() {
        let mut m = YjMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yj_metrics_merge() {
        let mut a = YjMetrics::new("a");
        a.record(1.0);
        let mut b = YjMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yj_metrics_reset() {
        let mut m = YjMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yj_rate_window_empty() {
        let rw = YjRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yj_rate_window_tick_and_rate() {
        let mut rw = YjRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yj_lru_cache_basic() {
        let mut c = YjLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yj_lru_cache_contains_and_keys() {
        let mut c = YjLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yj_lru_cache_remove() {
        let mut c = YjLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yj_metrics_sum() {
        let mut m = YjMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yj_metrics_label() {
        let m = YjMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yj_lru_cache_clear() {
        let mut c = YjLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for state
    #[test]
    fn xa_state_ring_new() {
        let rb = super::XaStateRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_state_ring_push_len() {
        let mut rb = super::XaStateRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_state_ring_wrap() {
        let mut rb = super::XaStateRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_state_ring_mean_empty() {
        let rb = super::XaStateRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_state_ring_mean_values() {
        let mut rb = super::XaStateRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_state_ring_min_max() {
        let mut rb = super::XaStateRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_state_ring_iter() {
        let mut rb = super::XaStateRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_state_counter_new() {
        let c = super::XaStateCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_state_counter_inc() {
        let mut c = super::XaStateCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_state_counter_inc_by() {
        let mut c = super::XaStateCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_state_counter_reset() {
        let mut c = super::XaStateCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_state_counter_clear() {
        let mut c = super::XaStateCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_state_counter_default() {
        let c = super::XaStateCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 163 ----

    #[test]
    fn xc_163_pool_new_empty() {
        let pool: super::Xc163Pool<i32> = super::Xc163Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_163_pool_release_acquire() {
        let mut pool = super::Xc163Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_163_pool_acquire_empty() {
        let mut pool: super::Xc163Pool<i32> = super::Xc163Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_163_pool_full() {
        let mut pool = super::Xc163Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_163_pool_drain() {
        let mut pool = super::Xc163Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_163_pool_stats() {
        let mut pool = super::Xc163Pool::new(8);
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
    fn xc_163_pool_clear() {
        let mut pool = super::Xc163Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_163_pool_shrink() {
        let mut pool = super::Xc163Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_163_pool_default() {
        let pool: super::Xc163Pool<String> = super::Xc163Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_163_pool_extend() {
        let mut pool = super::Xc163Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_163_pool_retain() {
        let mut pool = super::Xc163Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_163_scheduler_round_robin() {
        let mut sched = super::Xc163Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_163_scheduler_empty() {
        let mut sched = super::Xc163Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_163_scheduler_reset() {
        let mut sched = super::Xc163Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_163_scheduler_add_remove() {
        let mut sched = super::Xc163Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_163_scheduler_targets() {
        let sched = super::Xc163Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_163_hash_empty() {
        assert_eq!(super::xc_163_hash(b""), 5381);
    }

    #[test]
    fn xc_163_hash_data() {
        let h = super::xc_163_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_163_hash(b"hello"), h);
    }

    #[test]
    fn xc_163_reverse_str() {
        assert_eq!(super::xc_163_reverse("abc"), "cba");
        assert_eq!(super::xc_163_reverse(""), "");
    }


    // --- xd_87 deepening tests ---

    #[test]
    fn xd_87_sm_initial_state() {
        let sm = Xd87StateMachine::new();
        assert_eq!(sm.current_state(), Xd87State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_87_sm_valid_idle_to_running() {
        let mut sm = Xd87StateMachine::new();
        assert!(sm.transition(Xd87State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd87State::Running);
    }

    #[test]
    fn xd_87_sm_valid_running_to_paused() {
        let mut sm = Xd87StateMachine::new();
        sm.transition(Xd87State::Running).unwrap();
        assert!(sm.transition(Xd87State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd87State::Paused);
    }

    #[test]
    fn xd_87_sm_valid_running_to_done() {
        let mut sm = Xd87StateMachine::new();
        sm.transition(Xd87State::Running).unwrap();
        assert!(sm.transition(Xd87State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd87State::Done);
    }

    #[test]
    fn xd_87_sm_valid_paused_to_running() {
        let mut sm = Xd87StateMachine::new();
        sm.transition(Xd87State::Running).unwrap();
        sm.transition(Xd87State::Paused).unwrap();
        assert!(sm.transition(Xd87State::Running).is_ok());
    }

    #[test]
    fn xd_87_sm_valid_done_to_idle() {
        let mut sm = Xd87StateMachine::new();
        sm.transition(Xd87State::Running).unwrap();
        sm.transition(Xd87State::Done).unwrap();
        assert!(sm.transition(Xd87State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd87State::Idle);
    }

    #[test]
    fn xd_87_sm_invalid_idle_to_done() {
        let mut sm = Xd87StateMachine::new();
        assert!(sm.transition(Xd87State::Done).is_err());
    }

    #[test]
    fn xd_87_sm_invalid_idle_to_paused() {
        let mut sm = Xd87StateMachine::new();
        assert!(sm.transition(Xd87State::Paused).is_err());
    }

    #[test]
    fn xd_87_sm_history_tracking() {
        let mut sm = Xd87StateMachine::new();
        sm.transition(Xd87State::Running).unwrap();
        sm.transition(Xd87State::Paused).unwrap();
        sm.transition(Xd87State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd87State::Idle);
        assert_eq!(sm.history()[0].to, Xd87State::Running);
        assert_eq!(sm.history()[1].from, Xd87State::Running);
        assert_eq!(sm.history()[2].to, Xd87State::Done);
    }

    #[test]
    fn xd_87_sm_serialize_deserialize() {
        let mut sm = Xd87StateMachine::new();
        sm.transition(Xd87State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd87StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd87State::Running));
    }

    #[test]
    fn xd_87_sm_deserialize_invalid() {
        assert_eq!(Xd87StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_87_sm_reset() {
        let mut sm = Xd87StateMachine::new();
        sm.transition(Xd87State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd87State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_87_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd87EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd87Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_87_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd87EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd87Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd87Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_87_bus_unsubscribe() {
        let mut bus = Xd87EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_87_event_kind_and_payload() {
        let e = Xd87Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd87Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_87_bus_clear_history() {
        let mut bus = Xd87EventBus::new();
        bus.publish(Xd87Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_87_sm_step_counter_increments() {
        let mut sm = Xd87StateMachine::new();
        sm.transition(Xd87State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd87State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #109 --

    #[test]
    fn xf109_trie_insert_search() {
        let mut t = Xf109Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf109_trie_starts_with() {
        let mut t = Xf109Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf109_trie_remove() {
        let mut t = Xf109Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf109_trie_word_count() {
        let mut t = Xf109Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf109_trie_longest_prefix() {
        let mut t = Xf109Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf109_trie_all_words() {
        let mut t = Xf109Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf109_trie_autocomplete() {
        let mut t = Xf109Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf109_trie_empty_search() {
        let t = Xf109Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf109_bloom_add_contains() {
        let mut bf = Xf109BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf109_bloom_probably_absent() {
        let bf = Xf109BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf109_bloom_false_positive_rate() {
        let mut bf = Xf109BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf109_bloom_clear() {
        let mut bf = Xf109BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf109_bloom_union() {
        let mut a = Xf109BloomFilter::xf_new(512, 2);
        let mut b = Xf109BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf109_bloom_intersection_estimate() {
        let mut a = Xf109BloomFilter::xf_new(512, 2);
        let mut b = Xf109BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf109_bloom_union_size_mismatch() {
        let a = Xf109BloomFilter::xf_new(256, 2);
        let b = Xf109BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh162_skip_insert_contains() {
        let mut sl = super::Xh162SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh162_skip_remove() {
        let mut sl = super::Xh162SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh162_skip_len() {
        let mut sl = super::Xh162SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh162_skip_range_query() {
        let mut sl = super::Xh162SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh162_skip_floor_ceiling() {
        let mut sl = super::Xh162SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh162_skip_rank() {
        let mut sl = super::Xh162SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh162_skip_empty() {
        let sl = super::Xh162SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh162_skip_duplicates() {
        let mut sl = super::Xh162SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh162_bitset_set_test() {
        let mut bs = super::Xh162BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh162_bitset_clear_count() {
        let mut bs = super::Xh162BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh162_bitset_and_or_xor() {
        let mut a = super::Xh162BitSet::xh_new(128);
        let mut b = super::Xh162BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh162_bitset_iter_ones() {
        let mut bs = super::Xh162BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh162_bitset_first_last() {
        let mut bs = super::Xh162BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh162_bitset_empty() {
        let bs = super::Xh162BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi162_deque_push_pop_back() {
        let mut dq = super::Xi162Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi162_deque_push_pop_front() {
        let mut dq = super::Xi162Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi162_deque_mixed_ops() {
        let mut dq = super::Xi162Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi162_deque_get_and_split() {
        let mut dq = super::Xi162Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi162_deque_rotate_left() {
        let mut dq = super::Xi162Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi162_deque_rotate_right() {
        let mut dq = super::Xi162Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi162_deque_grow() {
        let mut dq = super::Xi162Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi162_deque_empty() {
        let dq = super::Xi162Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi162_interval_tree_insert_query() {
        let mut tree = super::Xi162IntervalTree::xi_new();
        tree.xi_insert(super::Xi162Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi162Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi162Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi162_interval_tree_overlap() {
        let mut tree = super::Xi162IntervalTree::xi_new();
        tree.xi_insert(super::Xi162Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi162Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi162Interval::xi_new(12, 20));
        let q = super::Xi162Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi162_interval_tree_remove() {
        let mut tree = super::Xi162IntervalTree::xi_new();
        tree.xi_insert(super::Xi162Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi162Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi162_interval_tree_gaps() {
        let mut tree = super::Xi162IntervalTree::xi_new();
        tree.xi_insert(super::Xi162Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi162Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi162Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi162Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi162Interval::xi_new(8, 10));
    }

    #[test]
    fn xi162_interval_tree_merge() {
        let mut tree = super::Xi162IntervalTree::xi_new();
        tree.xi_insert(super::Xi162Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi162Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi162Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi162Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi162Interval::xi_new(10, 15));
    }

    #[test]
    fn xi162_interval_tree_all() {
        let mut tree = super::Xi162IntervalTree::xi_new();
        tree.xi_insert(super::Xi162Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi162Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi162_interval_tree_empty() {
        let tree = super::Xi162IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi162_interval_tree_contains_point() {
        let iv = super::Xi162Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 162) ---

    #[test]
    fn xj_162_uf_make_and_find() {
        let mut uf = super::Xj162UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_162_uf_union_connected() {
        let mut uf = super::Xj162UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_162_uf_component_count() {
        let mut uf = super::Xj162UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_162_uf_component_size() {
        let mut uf = super::Xj162UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_162_uf_largest_component() {
        let mut uf = super::Xj162UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_162_uf_many_elements() {
        let mut uf = super::Xj162UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_162_uf_separate_components() {
        let mut uf = super::Xj162UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_162_uf_path_compression() {
        let mut uf = super::Xj162UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_162_bt_insert_get() {
        let mut bt = super::Xj162BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_162_bt_contains_len() {
        let mut bt = super::Xj162BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_162_bt_replace() {
        let mut bt = super::Xj162BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_162_bt_remove() {
        let mut bt = super::Xj162BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_162_bt_keys_values() {
        let mut bt = super::Xj162BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_162_bt_range() {
        let mut bt = super::Xj162BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_162_bt_min_max() {
        let mut bt = super::Xj162BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_162_bt_many_inserts() {
        let mut bt = super::Xj162BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_162 segment tree tests ---

    #[test]
    fn xk_162_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk162SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_162_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk162SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_162_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk162SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_162_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk162SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_162_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk162SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_162_st_single_element() {
        let data = vec![42];
        let st = super::Xk162SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_162_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk162SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_162_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk162SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_162 disjoint intervals tests ---

    #[test]
    fn xk_162_di_add_and_count() {
        let mut di = super::Xk162DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_162_di_merge_overlap() {
        let mut di = super::Xk162DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_162_di_contains() {
        let mut di = super::Xk162DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_162_di_remove() {
        let mut di = super::Xk162DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_162_di_covered_length() {
        let mut di = super::Xk162DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_162_di_gaps() {
        let mut di = super::Xk162DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_162_di_merge_adjacent() {
        let mut di = super::Xk162DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_162_di_empty() {
        let di = super::Xk162DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_162_rope_new_empty() {
        let rope = super::Xl162Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_162_rope_from_str() {
        let rope = super::Xl162Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_162_rope_insert_at() {
        let mut rope = super::Xl162Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_162_rope_delete_range() {
        let mut rope = super::Xl162Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_162_rope_char_at() {
        let rope = super::Xl162Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_162_rope_split_concat() {
        let rope = super::Xl162Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_162_rope_line_count() {
        let rope = super::Xl162Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_162_rope_line_at() {
        let rope = super::Xl162Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_162_sa_build_and_search() {
        let sa = super::Xl162SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_162_sa_count() {
        let sa = super::Xl162SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_162_sa_longest_repeated() {
        let sa = super::Xl162SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_162_sa_all_positions() {
        let sa = super::Xl162SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_162_sa_len() {
        let sa = super::Xl162SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_162_sa_empty() {
        let sa = super::Xl162SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_162_rope_slice() {
        let rope = super::Xl162Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_162_sa_search_start() {
        let sa = super::Xl162SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_162_sparse_set_get() {
        let mut m = super::Xm162MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_162_sparse_row_col() {
        let mut m = super::Xm162MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_162_sparse_transpose() {
        let mut m = super::Xm162MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_162_sparse_multiply_vec() {
        let mut m = super::Xm162MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_162_sparse_nnz_density() {
        let mut m = super::Xm162MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_162_sparse_clear() {
        let mut m = super::Xm162MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_162_sparse_overwrite_zero() {
        let mut m = super::Xm162MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_162_tokenizer_basic() {
        let t = super::Xm162Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_162_tokenizer_count() {
        let t = super::Xm162Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_162_tokenizer_unique() {
        let t = super::Xm162Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_162_tokenizer_frequency() {
        let t = super::Xm162Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_162_tokenizer_delimiter() {
        let t = super::Xm162Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_162_tokenizer_whitespace() {
        let t = super::Xm162Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_162_tokenizer_empty() {
        let t = super::Xm162Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 162 ----

    #[test]
    fn xn_162_fenwick_prefix_sum() {
        let mut ft = super::Xn162Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_162_fenwick_range_sum() {
        let mut ft = super::Xn162Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_162_fenwick_point_query() {
        let mut ft = super::Xn162Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_162_fenwick_len() {
        let ft = super::Xn162Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_162_fenwick_multiple_updates() {
        let mut ft = super::Xn162Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_162_fenwick_single_element() {
        let mut ft = super::Xn162Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_162_fenwick_find_kth() {
        let mut ft = super::Xn162Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_162_fenwick_negative_delta() {
        let mut ft = super::Xn162Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 162 ----

    #[test]
    fn xn_162_avl_insert_get() {
        let mut m = super::Xn162AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_162_avl_remove() {
        let mut m = super::Xn162AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_162_avl_in_order() {
        let mut m = super::Xn162AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_162_avl_min_max() {
        let mut m = super::Xn162AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_162_avl_floor_ceiling() {
        let mut m = super::Xn162AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_162_avl_height_balanced() {
        let mut m = super::Xn162AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_162_avl_overwrite() {
        let mut m = super::Xn162AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_162_avl_empty() {
        let m: super::Xn162AVL<i32, i32> = super::Xn162AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo162RedBlack tests ---

    #[test]
    fn xo_162_rb_insert_and_get() {
        let mut tree = super::Xo162RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_162_rb_len_and_empty() {
        let mut tree = super::Xo162RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_162_rb_min_max() {
        let mut tree = super::Xo162RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_162_rb_contains() {
        let mut tree = super::Xo162RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_162_rb_remove() {
        let mut tree = super::Xo162RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_162_rb_in_order() {
        let mut tree = super::Xo162RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_162_rb_black_height() {
        let mut tree = super::Xo162RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_162_rb_overwrite() {
        let mut tree = super::Xo162RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo162ConsistentHash tests ---

    #[test]
    fn xo_162_ch_add_and_count() {
        let mut ring = super::Xo162ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_162_ch_remove_node() {
        let mut ring = super::Xo162ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_162_ch_get_node() {
        let mut ring = super::Xo162ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_162_ch_empty_ring() {
        let ring = super::Xo162ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_162_ch_distribution() {
        let mut ring = super::Xo162ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_162_ch_rebalance() {
        let mut ring = super::Xo162ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_162_ch_virtual_nodes() {
        let mut ring = super::Xo162ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_162_ch_consistent_lookup() {
        let mut ring = super::Xo162ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_162_splay_insert_get() {
        let mut t = super::Xp162SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_162_splay_remove() {
        let mut t = super::Xp162SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_162_splay_count_increases() {
        let mut t = super::Xp162SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_162_splay_depth() {
        let mut t = super::Xp162SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_162_splay_len_empty() {
        let t = super::Xp162SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_162_splay_min_max() {
        let mut t = super::Xp162SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_162_splay_overwrite() {
        let mut t = super::Xp162SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_162_splay_remove_missing() {
        let mut t = super::Xp162SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }

}
