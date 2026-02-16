//! Workbench configuration service.

use std::collections::HashMap;
use std::fmt;

/// The scope at which a configuration value is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigurationScope {
    Default,
    User,
    Workspace,
    WorkspaceFolder,
    Memory,
}

/// A single configuration entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigurationEntry {
    pub key: String,
    pub value: String,
    pub scope: ConfigurationScope,
    pub description: Option<String>,
}

impl ConfigurationEntry {
    /// Builder method to set a description on this entry.
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }
}

impl fmt::Display for ConfigurationScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigurationScope::Default => write!(f, "Default"),
            ConfigurationScope::User => write!(f, "User"),
            ConfigurationScope::Workspace => write!(f, "Workspace"),
            ConfigurationScope::WorkspaceFolder => write!(f, "WorkspaceFolder"),
            ConfigurationScope::Memory => write!(f, "Memory"),
        }
    }
}

impl fmt::Display for ConfigurationEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} = {} ({})", self.key, self.value, self.scope)
    }
}

/// In-memory configuration model.
pub struct ConfigurationModel {
    entries: HashMap<String, ConfigurationEntry>,
}

impl ConfigurationModel {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: String, value: String, scope: ConfigurationScope) {
        self.entries.insert(
            key.clone(),
            ConfigurationEntry {
                key,
                value,
                scope,
                description: None,
            },
        );
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|e| e.value.as_str())
    }

    pub fn get_with_scope(&self, key: &str) -> Option<(&str, &ConfigurationScope)> {
        self.entries
            .get(key)
            .map(|e| (e.value.as_str(), &e.scope))
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.entries.remove(key).is_some()
    }

    pub fn get_keys_by_scope(&self, scope: ConfigurationScope) -> Vec<&str> {
        self.entries
            .values()
            .filter(|e| e.scope == scope)
            .map(|e| e.key.as_str())
            .collect()
    }

    pub fn has(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Set a configuration entry with a description.
    pub fn set_with_description(
        &mut self,
        key: String,
        value: String,
        scope: ConfigurationScope,
        description: String,
    ) {
        self.entries.insert(
            key.clone(),
            ConfigurationEntry {
                key,
                value,
                scope,
                description: Some(description),
            },
        );
    }

    /// Returns the value for `key`, or `default` if the key is absent.
    pub fn get_or_default(&self, key: &str, default: &str) -> String {
        self.entries
            .get(key)
            .map(|e| e.value.clone())
            .unwrap_or_else(|| default.to_string())
    }

    /// Returns the description associated with `key`, if any.
    pub fn get_description(&self, key: &str) -> Option<&str> {
        self.entries
            .get(key)
            .and_then(|e| e.description.as_deref())
    }

    /// Returns all keys present in the model.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.keys().map(|k| k.as_str()).collect()
    }

    /// Returns all entries whose key starts with `prefix`.
    pub fn get_entries_by_prefix(&self, prefix: &str) -> Vec<&ConfigurationEntry> {
        self.entries
            .values()
            .filter(|e| e.key.starts_with(prefix))
            .collect()
    }

    /// Merges entries from `other` into this model. Entries in `other` take precedence.
    pub fn merge(&mut self, other: &ConfigurationModel) {
        for (key, entry) in &other.entries {
            self.entries.insert(key.clone(), entry.clone());
        }
    }

    /// Removes all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns a cloned snapshot of all entries.
    pub fn snapshot(&self) -> Vec<ConfigurationEntry> {
        self.entries.values().cloned().collect()
    }
}

impl Default for ConfigurationModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors returned by [`ConfigurationValidator`] methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValidationError {
    EmptyKey,
    MissingDotSeparator,
    StartsWithDot,
    EndsWithDot,
    ValueTooLong { max: usize, actual: usize },
}

impl fmt::Display for ConfigValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigValidationError::EmptyKey => write!(f, "key must not be empty"),
            ConfigValidationError::MissingDotSeparator => {
                write!(f, "key must contain a '.' separator")
            }
            ConfigValidationError::StartsWithDot => write!(f, "key must not start with '.'"),
            ConfigValidationError::EndsWithDot => write!(f, "key must not end with '.'"),
            ConfigValidationError::ValueTooLong { max, actual } => {
                write!(f, "value length {} exceeds maximum {}", actual, max)
            }
        }
    }
}

/// Stateless validator for configuration keys and values.
pub struct ConfigurationValidator;

impl ConfigurationValidator {
    /// Validates that `key` is non-empty, contains a `.` separator,
    /// and does not start or end with `.`.
    pub fn validate_key(key: &str) -> Result<(), ConfigValidationError> {
        if key.is_empty() {
            return Err(ConfigValidationError::EmptyKey);
        }
        if key.starts_with('.') {
            return Err(ConfigValidationError::StartsWithDot);
        }
        if key.ends_with('.') {
            return Err(ConfigValidationError::EndsWithDot);
        }
        if !key.contains('.') {
            return Err(ConfigValidationError::MissingDotSeparator);
        }
        Ok(())
    }

    /// Validates that `value` does not exceed `max_len` bytes.
    pub fn validate_value(value: &str, max_len: usize) -> Result<(), ConfigValidationError> {
        if value.len() > max_len {
            return Err(ConfigValidationError::ValueTooLong {
                max: max_len,
                actual: value.len(),
            });
        }
        Ok(())
    }

    /// Convenience predicate wrapping [`validate_key`].
    pub fn is_valid_key(key: &str) -> bool {
        Self::validate_key(key).is_ok()
    }
}

/// Represents the difference between two [`ConfigurationModel`]s.
pub struct ConfigurationDiff {
    /// Keys present in `new` but absent from `old`.
    pub added: Vec<String>,
    /// Keys present in `old` but absent from `new`.
    pub removed: Vec<String>,
    /// Keys present in both but with different values.
    pub changed: Vec<String>,
}

impl ConfigurationDiff {
    /// Computes the diff between `old` and `new` models.
    pub fn compute(old: &ConfigurationModel, new: &ConfigurationModel) -> Self {
        let old_keys: std::collections::HashSet<&str> =
            old.entries.keys().map(|k| k.as_str()).collect();
        let new_keys: std::collections::HashSet<&str> =
            new.entries.keys().map(|k| k.as_str()).collect();

        let mut added: Vec<String> = new_keys
            .difference(&old_keys)
            .map(|k| k.to_string())
            .collect();
        added.sort();

        let mut removed: Vec<String> = old_keys
            .difference(&new_keys)
            .map(|k| k.to_string())
            .collect();
        removed.sort();

        let mut changed: Vec<String> = old_keys
            .intersection(&new_keys)
            .filter(|k| {
                old.entries.get(**k).map(|e| &e.value)
                    != new.entries.get(**k).map(|e| &e.value)
            })
            .map(|k| k.to_string())
            .collect();
        changed.sort();

        Self {
            added,
            removed,
            changed,
        }
    }

    /// Returns `true` when there are no additions, removals, or changes.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// Total number of added, removed, and changed keys.
    pub fn total_changes(&self) -> usize {
        self.added.len() + self.removed.len() + self.changed.len()
    }
}

/// Layers multiple [`ConfigurationModel`]s with increasing priority.
pub struct ConfigurationOverrideModel {
    layers: Vec<ConfigurationModel>,
}

impl ConfigurationOverrideModel {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Adds a layer. Later layers have higher priority.
    pub fn add_layer(&mut self, model: ConfigurationModel) {
        self.layers.push(model);
    }

    /// Resolves `key` by searching layers from highest to lowest priority.
    pub fn resolve(&self, key: &str) -> Option<&str> {
        for layer in self.layers.iter().rev() {
            if let Some(val) = layer.get(key) {
                return Some(val);
            }
        }
        None
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Returns the union of all keys across every layer.
    pub fn all_keys(&self) -> Vec<String> {
        let mut set = std::collections::HashSet::new();
        for layer in &self.layers {
            for key in layer.entries.keys() {
                set.insert(key.clone());
            }
        }
        let mut keys: Vec<String> = set.into_iter().collect();
        keys.sort();
        keys
    }
}

impl Default for ConfigurationOverrideModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for wb-config operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbConfigStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbConfigStats {
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
    pub fn merge(&mut self, other: &WbConfigStats) {
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

impl Default for WbConfigStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbConfigStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbConfigStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-config.
#[derive(Debug, Clone)]
pub struct WbConfigValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbConfigValidator {
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

impl Default for WbConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let mut model = ConfigurationModel::new();
        model.set(
            "editor.fontSize".to_string(),
            "14".to_string(),
            ConfigurationScope::User,
        );
        assert_eq!(model.get("editor.fontSize"), Some("14"));
        let (val, scope) = model.get_with_scope("editor.fontSize").unwrap();
        assert_eq!(val, "14");
        assert_eq!(*scope, ConfigurationScope::User);
    }

    #[test]
    fn remove_and_has() {
        let mut model = ConfigurationModel::new();
        model.set("key".to_string(), "val".to_string(), ConfigurationScope::Default);
        assert!(model.has("key"));
        assert!(model.remove("key"));
        assert!(!model.has("key"));
        assert!(!model.remove("key"));
    }

    #[test]
    fn keys_by_scope() {
        let mut model = ConfigurationModel::new();
        model.set("a".to_string(), "1".to_string(), ConfigurationScope::User);
        model.set("b".to_string(), "2".to_string(), ConfigurationScope::User);
        model.set("c".to_string(), "3".to_string(), ConfigurationScope::Workspace);
        let user_keys = model.get_keys_by_scope(ConfigurationScope::User);
        assert_eq!(user_keys.len(), 2);
        assert_eq!(model.get_keys_by_scope(ConfigurationScope::Workspace).len(), 1);
        assert_eq!(model.entry_count(), 3);
    }

    #[test]
    fn with_description_builder() {
        let entry = ConfigurationEntry {
            key: "editor.tabSize".to_string(),
            value: "4".to_string(),
            scope: ConfigurationScope::User,
            description: None,
        }
        .with_description("Number of spaces per tab");
        assert_eq!(entry.description.as_deref(), Some("Number of spaces per tab"));
    }

    #[test]
    fn set_with_description_and_get_description() {
        let mut model = ConfigurationModel::new();
        model.set_with_description(
            "editor.wordWrap".to_string(),
            "on".to_string(),
            ConfigurationScope::User,
            "Controls word wrapping".to_string(),
        );
        assert_eq!(model.get("editor.wordWrap"), Some("on"));
        assert_eq!(
            model.get_description("editor.wordWrap"),
            Some("Controls word wrapping")
        );
        assert_eq!(model.get_description("missing"), None);
    }

    #[test]
    fn get_or_default_returns_value_or_fallback() {
        let mut model = ConfigurationModel::new();
        model.set("theme".to_string(), "dark".to_string(), ConfigurationScope::User);
        assert_eq!(model.get_or_default("theme", "light"), "dark");
        assert_eq!(model.get_or_default("missing", "light"), "light");
    }

    #[test]
    fn keys_returns_all_keys() {
        let mut model = ConfigurationModel::new();
        model.set("a".to_string(), "1".to_string(), ConfigurationScope::Default);
        model.set("b".to_string(), "2".to_string(), ConfigurationScope::User);
        let mut keys = model.keys();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn get_entries_by_prefix_filters_correctly() {
        let mut model = ConfigurationModel::new();
        model.set("editor.fontSize".to_string(), "14".to_string(), ConfigurationScope::User);
        model.set("editor.tabSize".to_string(), "4".to_string(), ConfigurationScope::User);
        model.set("terminal.fontSize".to_string(), "12".to_string(), ConfigurationScope::User);
        let editor_entries = model.get_entries_by_prefix("editor.");
        assert_eq!(editor_entries.len(), 2);
        assert!(editor_entries.iter().all(|e| e.key.starts_with("editor.")));
        assert_eq!(model.get_entries_by_prefix("terminal.").len(), 1);
        assert_eq!(model.get_entries_by_prefix("nonexistent.").len(), 0);
    }

    #[test]
    fn merge_other_takes_precedence() {
        let mut base = ConfigurationModel::new();
        base.set("a".to_string(), "1".to_string(), ConfigurationScope::Default);
        base.set("b".to_string(), "2".to_string(), ConfigurationScope::Default);

        let mut overlay = ConfigurationModel::new();
        overlay.set("b".to_string(), "20".to_string(), ConfigurationScope::User);
        overlay.set("c".to_string(), "30".to_string(), ConfigurationScope::User);

        base.merge(&overlay);
        assert_eq!(base.get("a"), Some("1"));
        assert_eq!(base.get("b"), Some("20"));
        assert_eq!(base.get("c"), Some("30"));
        assert_eq!(base.entry_count(), 3);
    }

    #[test]
    fn clear_and_snapshot() {
        let mut model = ConfigurationModel::new();
        model.set("x".to_string(), "1".to_string(), ConfigurationScope::User);
        model.set("y".to_string(), "2".to_string(), ConfigurationScope::Workspace);

        let snap = model.snapshot();
        assert_eq!(snap.len(), 2);

        model.clear();
        assert_eq!(model.entry_count(), 0);
        assert!(!model.has("x"));

        // snapshot is independent of the cleared model
        assert_eq!(snap.len(), 2);
    }

    #[test]
    fn display_impls() {
        assert_eq!(format!("{}", ConfigurationScope::Default), "Default");
        assert_eq!(format!("{}", ConfigurationScope::User), "User");
        assert_eq!(format!("{}", ConfigurationScope::Workspace), "Workspace");
        assert_eq!(format!("{}", ConfigurationScope::WorkspaceFolder), "WorkspaceFolder");
        assert_eq!(format!("{}", ConfigurationScope::Memory), "Memory");

        let entry = ConfigurationEntry {
            key: "editor.fontSize".to_string(),
            value: "14".to_string(),
            scope: ConfigurationScope::User,
            description: None,
        };
        assert_eq!(format!("{}", entry), "editor.fontSize = 14 (User)");
    }

    #[test]
    fn test_validate_key_valid() {
        assert!(ConfigurationValidator::validate_key("editor.fontSize").is_ok());
        assert!(ConfigurationValidator::validate_key("a.b.c").is_ok());
    }

    #[test]
    fn test_validate_key_empty() {
        assert_eq!(
            ConfigurationValidator::validate_key(""),
            Err(ConfigValidationError::EmptyKey)
        );
    }

    #[test]
    fn test_validate_key_no_dot() {
        assert_eq!(
            ConfigurationValidator::validate_key("nodot"),
            Err(ConfigValidationError::MissingDotSeparator)
        );
    }

    #[test]
    fn test_validate_key_starts_with_dot() {
        assert_eq!(
            ConfigurationValidator::validate_key(".leading"),
            Err(ConfigValidationError::StartsWithDot)
        );
    }

    #[test]
    fn test_validate_key_ends_with_dot() {
        assert_eq!(
            ConfigurationValidator::validate_key("trailing."),
            Err(ConfigValidationError::EndsWithDot)
        );
    }

    #[test]
    fn test_validate_value_ok() {
        assert!(ConfigurationValidator::validate_value("hello", 10).is_ok());
        assert!(ConfigurationValidator::validate_value("exact", 5).is_ok());
    }

    #[test]
    fn test_validate_value_too_long() {
        assert_eq!(
            ConfigurationValidator::validate_value("toolong", 3),
            Err(ConfigValidationError::ValueTooLong { max: 3, actual: 7 })
        );
    }

    #[test]
    fn test_config_diff_additions() {
        let old = ConfigurationModel::new();
        let mut new = ConfigurationModel::new();
        new.set("a.b".to_string(), "1".to_string(), ConfigurationScope::User);
        let diff = ConfigurationDiff::compute(&old, &new);
        assert_eq!(diff.added, vec!["a.b"]);
        assert!(diff.removed.is_empty());
        assert!(diff.changed.is_empty());
        assert_eq!(diff.total_changes(), 1);
    }

    #[test]
    fn test_config_diff_removals() {
        let mut old = ConfigurationModel::new();
        old.set("a.b".to_string(), "1".to_string(), ConfigurationScope::User);
        let new = ConfigurationModel::new();
        let diff = ConfigurationDiff::compute(&old, &new);
        assert!(diff.added.is_empty());
        assert_eq!(diff.removed, vec!["a.b"]);
        assert!(diff.changed.is_empty());
    }

    #[test]
    fn test_config_diff_changes() {
        let mut old = ConfigurationModel::new();
        old.set("a.b".to_string(), "1".to_string(), ConfigurationScope::User);
        let mut new = ConfigurationModel::new();
        new.set("a.b".to_string(), "2".to_string(), ConfigurationScope::User);
        let diff = ConfigurationDiff::compute(&old, &new);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.changed, vec!["a.b"]);
    }

    #[test]
    fn test_config_diff_empty() {
        let mut a = ConfigurationModel::new();
        a.set("x.y".to_string(), "v".to_string(), ConfigurationScope::Default);
        let mut b = ConfigurationModel::new();
        b.set("x.y".to_string(), "v".to_string(), ConfigurationScope::Default);
        let diff = ConfigurationDiff::compute(&a, &b);
        assert!(diff.is_empty());
        assert_eq!(diff.total_changes(), 0);
    }

    #[test]
    fn test_override_model_resolve() {
        let mut base = ConfigurationModel::new();
        base.set("editor.fontSize".to_string(), "12".to_string(), ConfigurationScope::Default);
        base.set("editor.tabSize".to_string(), "4".to_string(), ConfigurationScope::Default);

        let mut user = ConfigurationModel::new();
        user.set("editor.fontSize".to_string(), "16".to_string(), ConfigurationScope::User);

        let mut over = ConfigurationOverrideModel::new();
        over.add_layer(base);
        over.add_layer(user);

        assert_eq!(over.resolve("editor.fontSize"), Some("16"));
        assert_eq!(over.resolve("editor.tabSize"), Some("4"));
        assert_eq!(over.resolve("missing.key"), None);
        assert_eq!(over.layer_count(), 2);
    }

    #[test]
    fn test_override_model_all_keys() {
        let mut a = ConfigurationModel::new();
        a.set("x.a".to_string(), "1".to_string(), ConfigurationScope::Default);
        a.set("x.b".to_string(), "2".to_string(), ConfigurationScope::Default);

        let mut b = ConfigurationModel::new();
        b.set("x.b".to_string(), "3".to_string(), ConfigurationScope::User);
        b.set("x.c".to_string(), "4".to_string(), ConfigurationScope::User);

        let mut over = ConfigurationOverrideModel::new();
        over.add_layer(a);
        over.add_layer(b);

        let keys = over.all_keys();
        assert_eq!(keys, vec!["x.a", "x.b", "x.c"]);
    }

    #[test]
    fn test_config_validation_error_display() {
        assert_eq!(
            format!("{}", ConfigValidationError::EmptyKey),
            "key must not be empty"
        );
        assert_eq!(
            format!("{}", ConfigValidationError::MissingDotSeparator),
            "key must contain a '.' separator"
        );
        assert_eq!(
            format!("{}", ConfigValidationError::StartsWithDot),
            "key must not start with '.'"
        );
        assert_eq!(
            format!("{}", ConfigValidationError::EndsWithDot),
            "key must not end with '.'"
        );
        assert_eq!(
            format!("{}", ConfigValidationError::ValueTooLong { max: 5, actual: 10 }),
            "value length 10 exceeds maximum 5"
        );
    }

    #[test]
    fn test_is_valid_key() {
        assert!(ConfigurationValidator::is_valid_key("editor.fontSize"));
        assert!(!ConfigurationValidator::is_valid_key(""));
        assert!(!ConfigurationValidator::is_valid_key("nodot"));
        assert!(!ConfigurationValidator::is_valid_key(".leading"));
        assert!(!ConfigurationValidator::is_valid_key("trailing."));
    }

    #[test]
    fn wb_config_stats_new_defaults() {
        let stats = WbConfigStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_config_stats_record_success() {
        let mut stats = WbConfigStats::new();
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
    fn wb_config_stats_record_failure() {
        let mut stats = WbConfigStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_config_stats_reset() {
        let mut stats = WbConfigStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_config_stats_merge() {
        let mut a = WbConfigStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbConfigStats::new();
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
    fn wb_config_stats_display() {
        let mut stats = WbConfigStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_config_stats_default() {
        let stats = WbConfigStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_config_validator_accepts_valid_name() {
        let v = WbConfigValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_config_validator_rejects_empty() {
        let v = WbConfigValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_config_validator_rejects_too_long() {
        let v = WbConfigValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_config_validator_forbidden_prefix() {
        let v = WbConfigValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_config_validator_allowed_chars() {
        let v = WbConfigValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_config_validator_range() {
        let v = WbConfigValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_config_sanitize_removes_control() {
        let result = WbConfigValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_config_truncate_short_string() {
        assert_eq!(WbConfigValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_config_truncate_long_string() {
        let result = WbConfigValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_config_is_ascii_printable() {
        assert!(WbConfigValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbConfigValidator::is_ascii_printable("Hello\x00World"));
    }
}
