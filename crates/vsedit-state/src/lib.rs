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

}
