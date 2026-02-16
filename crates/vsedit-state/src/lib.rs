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
}
