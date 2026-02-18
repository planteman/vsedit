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

/// A configuration editor that tracks changes and supports undo.
pub struct ConfigEditor {
    model: ConfigurationModel,
    undo_stack: Vec<ConfigEditOp>,
}

/// A single edit operation for undo support.
#[derive(Debug, Clone)]
pub enum ConfigEditOp {
    Set {
        key: String,
        old_value: Option<String>,
        old_scope: Option<ConfigurationScope>,
    },
    Remove {
        key: String,
        old_value: String,
        old_scope: ConfigurationScope,
    },
}

impl ConfigEditor {
    /// Create a new editor wrapping a configuration model.
    pub fn new(model: ConfigurationModel) -> Self {
        Self {
            model,
            undo_stack: Vec::new(),
        }
    }

    /// Set a configuration value, recording the change for undo.
    pub fn set(&mut self, key: String, value: String, scope: ConfigurationScope) {
        let old = self.model.get(&key).map(|v| v.to_string());
        let old_scope = self.model.get_with_scope(&key).map(|(_, s)| *s);
        self.undo_stack.push(ConfigEditOp::Set {
            key: key.clone(),
            old_value: old,
            old_scope,
        });
        self.model.set(key, value, scope);
    }

    /// Remove a configuration key, recording the change for undo.
    pub fn remove(&mut self, key: &str) -> bool {
        if let Some((val, scope)) = self
            .model
            .get_with_scope(key)
            .map(|(v, s)| (v.to_string(), *s))
        {
            self.undo_stack.push(ConfigEditOp::Remove {
                key: key.to_string(),
                old_value: val,
                old_scope: scope,
            });
            self.model.remove(key)
        } else {
            false
        }
    }

    /// Undo the last edit operation.
    pub fn undo(&mut self) -> bool {
        match self.undo_stack.pop() {
            Some(ConfigEditOp::Set {
                key,
                old_value,
                old_scope,
            }) => {
                match (old_value, old_scope) {
                    (Some(val), Some(scope)) => {
                        self.model.set(key, val, scope);
                    }
                    _ => {
                        self.model.remove(&key);
                    }
                }
                true
            }
            Some(ConfigEditOp::Remove {
                key,
                old_value,
                old_scope,
            }) => {
                self.model.set(key, old_value, old_scope);
                true
            }
            None => false,
        }
    }

    /// Number of undoable operations.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get a reference to the underlying model.
    pub fn model(&self) -> &ConfigurationModel {
        &self.model
    }

    /// Consume the editor and return the model.
    pub fn into_model(self) -> ConfigurationModel {
        self.model
    }
}

impl ConfigurationModel {
    /// Search for entries whose key or description contains the keyword (case-insensitive).
    pub fn search(&self, keyword: &str) -> Vec<&ConfigurationEntry> {
        let lower = keyword.to_lowercase();
        self.entries
            .values()
            .filter(|e| {
                e.key.to_lowercase().contains(&lower)
                    || e.value.to_lowercase().contains(&lower)
                    || e.description
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&lower)
            })
            .collect()
    }
}

/// Search for configuration entries by keyword.
pub fn config_search<'a>(
    model: &'a ConfigurationModel,
    keyword: &str,
) -> Vec<&'a ConfigurationEntry> {
    model.search(keyword)
}

/// Expected type for a configuration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigValueType {
    String,
    Integer,
    Float,
    Boolean,
}

impl fmt::Display for ConfigValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigValueType::String => write!(f, "string"),
            ConfigValueType::Integer => write!(f, "integer"),
            ConfigValueType::Float => write!(f, "float"),
            ConfigValueType::Boolean => write!(f, "boolean"),
        }
    }
}

/// A named snapshot of a [`ConfigurationModel`] at a point in time.
#[derive(Debug, Clone)]
pub struct ConfigSnapshot {
    pub label: String,
    pub entries: Vec<ConfigurationEntry>,
    pub timestamp_epoch_ms: u64,
}

impl ConfigSnapshot {
    /// Capture a snapshot of the given model.
    pub fn capture(model: &ConfigurationModel, label: &str, timestamp_epoch_ms: u64) -> Self {
        Self {
            label: label.to_string(),
            entries: model.snapshot(),
            timestamp_epoch_ms,
        }
    }

    /// Restore this snapshot into a fresh [`ConfigurationModel`].
    pub fn restore(&self) -> ConfigurationModel {
        let mut model = ConfigurationModel::new();
        for entry in &self.entries {
            if let Some(ref desc) = entry.description {
                model.set_with_description(
                    entry.key.clone(),
                    entry.value.clone(),
                    entry.scope,
                    desc.clone(),
                );
            } else {
                model.set(entry.key.clone(), entry.value.clone(), entry.scope);
            }
        }
        model
    }

    /// Number of entries in this snapshot.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

/// Manages a history of [`ConfigSnapshot`]s.
pub struct ConfigSnapshotHistory {
    snapshots: Vec<ConfigSnapshot>,
    max_snapshots: usize,
}

impl ConfigSnapshotHistory {
    /// Create a new history with the given maximum capacity.
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            max_snapshots,
        }
    }

    /// Push a snapshot. If the history exceeds `max_snapshots`, the oldest is removed.
    pub fn push(&mut self, snapshot: ConfigSnapshot) {
        if self.snapshots.len() >= self.max_snapshots {
            self.snapshots.remove(0);
        }
        self.snapshots.push(snapshot);
    }

    /// Return the most recent snapshot, if any.
    pub fn latest(&self) -> Option<&ConfigSnapshot> {
        self.snapshots.last()
    }

    /// Number of snapshots currently stored.
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Return all snapshot labels.
    pub fn labels(&self) -> Vec<&str> {
        self.snapshots.iter().map(|s| s.label.as_str()).collect()
    }

    /// Find a snapshot by label.
    pub fn find_by_label(&self, label: &str) -> Option<&ConfigSnapshot> {
        self.snapshots.iter().find(|s| s.label == label)
    }
}

/// A single migration rule that renames or transforms a configuration key.
#[derive(Debug, Clone)]
pub struct ConfigMigrationRule {
    pub old_key: String,
    pub new_key: String,
    pub transform_value: Option<fn(&str) -> String>,
}

/// Migrates configuration entries from old keys to new keys.
pub struct ConfigMigration {
    rules: Vec<ConfigMigrationRule>,
}

impl ConfigMigration {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Register a simple key rename (value is preserved).
    pub fn rename(mut self, old_key: &str, new_key: &str) -> Self {
        self.rules.push(ConfigMigrationRule {
            old_key: old_key.to_string(),
            new_key: new_key.to_string(),
            transform_value: None,
        });
        self
    }

    /// Register a key rename with a value transformation.
    pub fn rename_with_transform(
        mut self,
        old_key: &str,
        new_key: &str,
        transform: fn(&str) -> String,
    ) -> Self {
        self.rules.push(ConfigMigrationRule {
            old_key: old_key.to_string(),
            new_key: new_key.to_string(),
            transform_value: Some(transform),
        });
        self
    }

    /// Apply all migration rules to the model, returning the number of keys migrated.
    pub fn apply(&self, model: &mut ConfigurationModel) -> usize {
        let mut migrated = 0;
        for rule in &self.rules {
            if let Some((value, scope)) = model
                .get_with_scope(&rule.old_key)
                .map(|(v, s)| (v.to_string(), *s))
            {
                let new_value = match rule.transform_value {
                    Some(f) => f(&value),
                    None => value,
                };
                model.set(rule.new_key.clone(), new_value, scope);
                model.remove(&rule.old_key);
                migrated += 1;
            }
        }
        migrated
    }

    /// Number of registered rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for ConfigMigration {
    fn default() -> Self {
        Self::new()
    }
}

/// Exports a [`ConfigurationModel`] to a simple key=value text format.
pub struct ConfigExporter;

impl ConfigExporter {
    /// Export all entries as sorted `key = value` lines.
    pub fn to_kv_string(model: &ConfigurationModel) -> String {
        let mut entries: Vec<_> = model.snapshot();
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        entries
            .iter()
            .map(|e| format!("{} = {}", e.key, e.value))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Export entries filtered by scope as sorted `key = value` lines.
    pub fn to_kv_string_by_scope(
        model: &ConfigurationModel,
        scope: ConfigurationScope,
    ) -> String {
        let mut entries: Vec<_> = model
            .snapshot()
            .into_iter()
            .filter(|e| e.scope == scope)
            .collect();
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        entries
            .iter()
            .map(|e| format!("{} = {}", e.key, e.value))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Export entries as `# description\nkey = value` blocks where descriptions exist.
    pub fn to_commented_kv_string(model: &ConfigurationModel) -> String {
        let mut entries: Vec<_> = model.snapshot();
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        let mut lines = Vec::new();
        for e in &entries {
            if let Some(ref desc) = e.description {
                lines.push(format!("# {}", desc));
            }
            lines.push(format!("{} = {}", e.key, e.value));
        }
        lines.join("\n")
    }
}

/// Validate that a configuration value matches the expected type.
pub fn config_validate_value(value: &str, expected_type: ConfigValueType) -> Result<(), String> {
    match expected_type {
        ConfigValueType::String => Ok(()),
        ConfigValueType::Integer => value
            .parse::<i64>()
            .map(|_| ())
            .map_err(|_| format!("'{}' is not a valid integer", value)),
        ConfigValueType::Float => value
            .parse::<f64>()
            .map(|_| ())
            .map_err(|_| format!("'{}' is not a valid float", value)),
        ConfigValueType::Boolean => match value {
            "true" | "false" | "1" | "0" => Ok(()),
            _ => Err(format!("'{}' is not a valid boolean", value)),
        },
    }
}

// ---------------------------------------------------------------------------
// Configuration key path utilities
// ---------------------------------------------------------------------------

/// Split a dotted configuration key into its segments.
/// Returns `None` if the key is empty.
pub fn config_key_segments(key: &str) -> Option<Vec<&str>> {
    if key.is_empty() {
        return None;
    }
    Some(key.split('.').collect())
}

/// Return the top-level namespace of a dotted key (everything before the first dot).
pub fn config_key_namespace(key: &str) -> Option<&str> {
    key.split('.').next().filter(|s| !s.is_empty())
}

/// Return the leaf portion of a dotted key (everything after the last dot).
pub fn config_key_leaf(key: &str) -> Option<&str> {
    key.rsplit('.').next().filter(|s| !s.is_empty() && key.contains('.'))
}

/// Count the depth (number of dots + 1) of a configuration key.
pub fn config_key_depth(key: &str) -> usize {
    if key.is_empty() {
        return 0;
    }
    key.split('.').count()
}

/// Check whether `parent_key` is a prefix namespace of `child_key`.
/// E.g. `"editor"` is a parent of `"editor.fontSize"`.
pub fn config_key_is_parent(parent_key: &str, child_key: &str) -> bool {
    if parent_key.is_empty() || child_key.is_empty() {
        return false;
    }
    child_key.starts_with(parent_key) && child_key.as_bytes().get(parent_key.len()) == Some(&b'.')
}

/// Collect all distinct top-level namespaces from a model.
pub fn config_namespaces(model: &ConfigurationModel) -> Vec<String> {
    let mut ns: Vec<String> = model
        .keys()
        .iter()
        .filter_map(|k| config_key_namespace(k))
        .map(|s| s.to_string())
        .collect();
    ns.sort();
    ns.dedup();
    ns
}

/// Group configuration entries by their top-level namespace.
pub fn config_group_by_namespace<'a>(
    model: &'a ConfigurationModel,
) -> HashMap<String, Vec<&'a ConfigurationEntry>> {
    let mut groups: HashMap<String, Vec<&'a ConfigurationEntry>> = HashMap::new();
    for entry in model.get_entries_by_prefix("") {
        if let Some(ns) = config_key_namespace(&entry.key) {
            groups.entry(ns.to_string()).or_default().push(entry);
        }
    }
    groups
}


// ---------------------------------------------------------------------------
// Configuration filtering and transformation utilities
// ---------------------------------------------------------------------------

/// Return keys matching a glob-like pattern where `*` matches any segment.
/// Only supports trailing wildcard: `"editor.*"` matches `"editor.fontSize"`.
pub fn config_keys_matching(model: &ConfigurationModel, pattern: &str) -> Vec<String> {
    if let Some(prefix) = pattern.strip_suffix(".*") {
        model
            .keys()
            .into_iter()
            .filter(|k| k.starts_with(prefix) && k.len() > prefix.len() && k.as_bytes()[prefix.len()] == b'.')
            .map(|k| k.to_string())
            .collect()
    } else {
        model
            .keys()
            .into_iter()
            .filter(|k| *k == pattern)
            .map(|k| k.to_string())
            .collect()
    }
}

/// Count the number of entries at each scope.
pub fn count_by_scope(model: &ConfigurationModel) -> HashMap<ConfigurationScope, usize> {
    let mut counts: HashMap<ConfigurationScope, usize> = HashMap::new();
    for entry in model.snapshot() {
        *counts.entry(entry.scope).or_insert(0) += 1;
    }
    counts
}

/// Produce a sorted list of `"key=value"` strings from the model.
pub fn config_to_sorted_pairs(model: &ConfigurationModel) -> Vec<String> {
    let mut pairs: Vec<String> = model
        .snapshot()
        .iter()
        .map(|e| format!("{}={}", e.key, e.value))
        .collect();
    pairs.sort();
    pairs
}

/// Return keys whose values are parseable as an integer.
pub fn numeric_config_keys(model: &ConfigurationModel) -> Vec<String> {
    model
        .snapshot()
        .into_iter()
        .filter(|e| e.value.parse::<i64>().is_ok())
        .map(|e| e.key)
        .collect()
}

/// Return keys whose values are parseable as a boolean (`"true"` or `"false"`).
pub fn boolean_config_keys(model: &ConfigurationModel) -> Vec<String> {
    model
        .snapshot()
        .into_iter()
        .filter(|e| e.value == "true" || e.value == "false")
        .map(|e| e.key)
        .collect()
}

/// Apply a set of overrides from `(key, value)` pairs at the given scope.
pub fn apply_overrides(
    model: &mut ConfigurationModel,
    overrides: &[(&str, &str)],
    scope: ConfigurationScope,
) -> usize {
    let mut count = 0;
    for (key, value) in overrides {
        model.set(key.to_string(), value.to_string(), scope);
        count += 1;
    }
    count
}

/// Return entries where the key has exactly `depth` dot-separated segments.
pub fn config_entries_at_depth(model: &ConfigurationModel, depth: usize) -> Vec<String> {
    model
        .keys()
        .into_iter()
        .filter(|k| k.split('.').count() == depth)
        .map(|k| k.to_string())
        .collect()
}

/// Produce a human-readable summary of the configuration model.
pub fn config_summary(model: &ConfigurationModel) -> String {
    let total = model.entry_count();
    let ns = config_namespaces(model);
    format!(
        "{} entries across {} namespaces",
        total,
        ns.len()
    )
}

// ---------------------------------------------------------------------------
// Configuration migrator
// ---------------------------------------------------------------------------

/// Applies a set of [`ConfigMigrationRule`]s to a model, tracking which rules
/// were applied and which keys were skipped because they were already absent.
pub struct ConfigMigrator {
    rules: Vec<ConfigMigrationRule>,
}

impl ConfigMigrator {
    /// Create a migrator from a pre-built [`ConfigMigration`].
    pub fn from_migration(migration: &ConfigMigration) -> Self {
        Self {
            rules: migration.rules.clone(),
        }
    }

    /// Create a migrator from a raw set of rules.
    pub fn from_rules(rules: Vec<ConfigMigrationRule>) -> Self {
        Self { rules }
    }

    /// Apply all rules, returning a report of what happened.
    pub fn apply(&self, model: &mut ConfigurationModel) -> ConfigMigratorReport {
        let mut applied = Vec::new();
        let mut skipped = Vec::new();
        for rule in &self.rules {
            if let Some((value, scope)) = model
                .get_with_scope(&rule.old_key)
                .map(|(v, s)| (v.to_string(), *s))
            {
                let new_value = match rule.transform_value {
                    Some(f) => f(&value),
                    None => value,
                };
                model.set(rule.new_key.clone(), new_value, scope);
                model.remove(&rule.old_key);
                applied.push(rule.old_key.clone());
            } else {
                skipped.push(rule.old_key.clone());
            }
        }
        ConfigMigratorReport { applied, skipped }
    }

    /// Number of registered rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

/// Report produced by [`ConfigMigrator::apply`].
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigMigratorReport {
    /// Old keys that were found and migrated.
    pub applied: Vec<String>,
    /// Old keys that were not present in the model.
    pub skipped: Vec<String>,
}

impl ConfigMigratorReport {
    pub fn applied_count(&self) -> usize {
        self.applied.len()
    }

    pub fn skipped_count(&self) -> usize {
        self.skipped.len()
    }

    pub fn total(&self) -> usize {
        self.applied.len() + self.skipped.len()
    }
}

impl fmt::Display for ConfigMigratorReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "migrated {} keys, skipped {}",
            self.applied.len(),
            self.skipped.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Configuration profile manager
// ---------------------------------------------------------------------------

/// Manages named configuration profiles backed by [`ConfigSnapshot`]s.
pub struct ConfigProfileManager {
    profiles: HashMap<String, ConfigSnapshot>,
    active_profile: Option<String>,
}

impl ConfigProfileManager {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            active_profile: None,
        }
    }

    /// Save a snapshot of `model` under `name`.
    pub fn save(&mut self, name: &str, model: &ConfigurationModel, timestamp_ms: u64) {
        let snapshot = ConfigSnapshot::capture(model, name, timestamp_ms);
        self.profiles.insert(name.to_string(), snapshot);
    }

    /// Load a previously saved profile into a fresh model.
    pub fn load(&self, name: &str) -> Option<ConfigurationModel> {
        self.profiles.get(name).map(|s| s.restore())
    }

    /// Set the active profile name. Returns `false` if the profile does not exist.
    pub fn switch(&mut self, name: &str) -> bool {
        if self.profiles.contains_key(name) {
            self.active_profile = Some(name.to_string());
            true
        } else {
            false
        }
    }

    /// The currently active profile name.
    pub fn active(&self) -> Option<&str> {
        self.active_profile.as_deref()
    }

    /// Remove a profile by name. Returns `true` if it existed.
    pub fn remove(&mut self, name: &str) -> bool {
        let removed = self.profiles.remove(name).is_some();
        if self.active_profile.as_deref() == Some(name) {
            self.active_profile = None;
        }
        removed
    }

    /// List all profile names in sorted order.
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.profiles.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Number of stored profiles.
    pub fn count(&self) -> usize {
        self.profiles.len()
    }
}

impl Default for ConfigProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Detailed configuration diff with old/new values
// ---------------------------------------------------------------------------

/// A single changed entry in a [`ConfigProfileDiff`].
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigChangedEntry {
    pub key: String,
    pub old_value: String,
    pub new_value: String,
}

/// Detailed diff between two [`ConfigurationModel`]s, including old/new values
/// for changed keys.  (Note: [`ConfigurationDiff`] already provides a simpler
/// key-only diff.)
pub struct ConfigProfileDiff {
    pub added: Vec<(String, String)>,
    pub removed: Vec<(String, String)>,
    pub changed: Vec<ConfigChangedEntry>,
}

impl ConfigProfileDiff {
    /// Compute a detailed diff between `old` and `new`.
    pub fn compute(old: &ConfigurationModel, new: &ConfigurationModel) -> Self {
        let old_keys: std::collections::HashSet<String> =
            old.keys().into_iter().map(|k| k.to_string()).collect();
        let new_keys: std::collections::HashSet<String> =
            new.keys().into_iter().map(|k| k.to_string()).collect();

        let mut added: Vec<(String, String)> = new_keys
            .difference(&old_keys)
            .map(|k| (k.clone(), new.get(k).unwrap_or("").to_string()))
            .collect();
        added.sort_by(|a, b| a.0.cmp(&b.0));

        let mut removed: Vec<(String, String)> = old_keys
            .difference(&new_keys)
            .map(|k| (k.clone(), old.get(k).unwrap_or("").to_string()))
            .collect();
        removed.sort_by(|a, b| a.0.cmp(&b.0));

        let mut changed: Vec<ConfigChangedEntry> = old_keys
            .intersection(&new_keys)
            .filter_map(|k| {
                let ov = old.get(k).unwrap_or("");
                let nv = new.get(k).unwrap_or("");
                if ov != nv {
                    Some(ConfigChangedEntry {
                        key: k.clone(),
                        old_value: ov.to_string(),
                        new_value: nv.to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();
        changed.sort_by(|a, b| a.key.cmp(&b.key));

        Self { added, removed, changed }
    }

    /// Whether there are no differences.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// Total number of differences.
    pub fn total(&self) -> usize {
        self.added.len() + self.removed.len() + self.changed.len()
    }
}

impl fmt::Display for ConfigProfileDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "diff: +{} -{} ~{}",
            self.added.len(),
            self.removed.len(),
            self.changed.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Configuration import validator
// ---------------------------------------------------------------------------

/// Error produced when validating imported configuration data.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigImportError {
    InvalidKey(String),
    InvalidValue { key: String, reason: String },
    InvalidScope(String),
}

impl fmt::Display for ConfigImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigImportError::InvalidKey(k) => write!(f, "invalid key: {}", k),
            ConfigImportError::InvalidValue { key, reason } => {
                write!(f, "invalid value for '{}': {}", key, reason)
            }
            ConfigImportError::InvalidScope(s) => write!(f, "invalid scope: {}", s),
        }
    }
}

/// Validates imported configuration data before it is applied to a model.
pub struct ConfigImportValidator {
    max_value_len: usize,
    allowed_scopes: Vec<ConfigurationScope>,
}

impl ConfigImportValidator {
    pub fn new() -> Self {
        Self {
            max_value_len: 4096,
            allowed_scopes: vec![
                ConfigurationScope::Default,
                ConfigurationScope::User,
                ConfigurationScope::Workspace,
                ConfigurationScope::WorkspaceFolder,
                ConfigurationScope::Memory,
            ],
        }
    }

    /// Set the maximum allowed value length.
    pub fn max_value_len(mut self, max: usize) -> Self {
        self.max_value_len = max;
        self
    }

    /// Restrict the set of allowed scopes.
    pub fn allowed_scopes(mut self, scopes: Vec<ConfigurationScope>) -> Self {
        self.allowed_scopes = scopes;
        self
    }

    /// Validate a single entry, returning all errors found.
    pub fn validate_entry(&self, entry: &ConfigurationEntry) -> Vec<ConfigImportError> {
        let mut errors = Vec::new();
        if ConfigurationValidator::validate_key(&entry.key).is_err() {
            errors.push(ConfigImportError::InvalidKey(entry.key.clone()));
        }
        if entry.value.len() > self.max_value_len {
            errors.push(ConfigImportError::InvalidValue {
                key: entry.key.clone(),
                reason: format!(
                    "length {} exceeds max {}",
                    entry.value.len(),
                    self.max_value_len
                ),
            });
        }
        if !self.allowed_scopes.contains(&entry.scope) {
            errors.push(ConfigImportError::InvalidScope(format!("{}", entry.scope)));
        }
        errors
    }

    /// Validate a batch of entries, returning a combined list of errors.
    pub fn validate_all(&self, entries: &[ConfigurationEntry]) -> Vec<ConfigImportError> {
        entries
            .iter()
            .flat_map(|e| self.validate_entry(e))
            .collect()
    }

    /// Returns `true` when every entry passes validation.
    pub fn is_valid(&self, entries: &[ConfigurationEntry]) -> bool {
        self.validate_all(entries).is_empty()
    }
}

impl Default for ConfigImportValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// ConfigOverrideChain
// ---------------------------------------------------------------------------

/// Layered configuration with default/user/workspace/folder levels.
#[derive(Debug, Clone)]
pub struct ConfigOverrideChain {
    layers: HashMap<ConfigurationScope, HashMap<String, String>>,
}

impl ConfigOverrideChain {
    pub fn new() -> Self {
        Self {
            layers: HashMap::new(),
        }
    }

    pub fn override_at_level(&mut self, level: ConfigurationScope, key: &str, value: &str) {
        self.layers
            .entry(level)
            .or_default()
            .insert(key.to_string(), value.to_string());
    }

    pub fn get_effective_value(&self, key: &str) -> Option<&str> {
        let order = [
            ConfigurationScope::WorkspaceFolder,
            ConfigurationScope::Workspace,
            ConfigurationScope::User,
            ConfigurationScope::Default,
        ];
        for scope in &order {
            if let Some(map) = self.layers.get(scope) {
                if let Some(val) = map.get(key) {
                    return Some(val.as_str());
                }
            }
        }
        None
    }

    pub fn get_source_of(&self, key: &str) -> Option<ConfigurationScope> {
        let order = [
            ConfigurationScope::WorkspaceFolder,
            ConfigurationScope::Workspace,
            ConfigurationScope::User,
            ConfigurationScope::Default,
        ];
        for scope in &order {
            if let Some(map) = self.layers.get(scope) {
                if map.contains_key(key) {
                    return Some(*scope);
                }
            }
        }
        None
    }

    pub fn levels_with_value(&self, key: &str) -> Vec<ConfigurationScope> {
        let mut result = Vec::new();
        for (scope, map) in &self.layers {
            if map.contains_key(key) {
                result.push(*scope);
            }
        }
        result
    }

    pub fn reset_level(&mut self, level: ConfigurationScope) {
        self.layers.remove(&level);
    }
}

// ---------------------------------------------------------------------------
// ConfigChangeEvent
// ---------------------------------------------------------------------------

/// Represents a configuration change.
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    pub key: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub scope: ConfigurationScope,
}

impl ConfigChangeEvent {
    pub fn new(key: &str, old: Option<&str>, new: Option<&str>, scope: ConfigurationScope) -> Self {
        Self {
            key: key.to_string(),
            old_value: old.map(|s| s.to_string()),
            new_value: new.map(|s| s.to_string()),
            scope,
        }
    }

    pub fn affects_key(&self, key: &str) -> bool {
        self.key == key || self.key.starts_with(&format!("{key}."))
    }

    pub fn is_addition(&self) -> bool {
        self.old_value.is_none() && self.new_value.is_some()
    }

    pub fn is_removal(&self) -> bool {
        self.old_value.is_some() && self.new_value.is_none()
    }

    pub fn is_modification(&self) -> bool {
        self.old_value.is_some() && self.new_value.is_some() && self.old_value != self.new_value
    }

    pub fn batch_changes(events: &[ConfigChangeEvent]) -> Vec<String> {
        events.iter().map(|e| e.key.clone()).collect()
    }
}

// ---------------------------------------------------------------------------
// ConfigSchemaEntry
// ---------------------------------------------------------------------------

/// Schema for a configuration key.
#[derive(Debug, Clone)]
pub struct ConfigSchemaEntry {
    pub key: String,
    pub value_type: String,
    pub default: String,
    pub description: String,
    pub enum_values: Option<Vec<String>>,
}

impl ConfigSchemaEntry {
    pub fn new(key: &str, value_type: &str, default: &str, description: &str) -> Self {
        Self {
            key: key.to_string(),
            value_type: value_type.to_string(),
            default: default.to_string(),
            description: description.to_string(),
            enum_values: None,
        }
    }

    pub fn with_enum_values(mut self, values: Vec<String>) -> Self {
        self.enum_values = Some(values);
        self
    }

    pub fn validate_value(&self, value: &str) -> bool {
        if let Some(ref enums) = self.enum_values {
            return enums.iter().any(|e| e == value);
        }
        self.is_valid_type(value)
    }

    pub fn is_valid_type(&self, value: &str) -> bool {
        match self.value_type.as_str() {
            "boolean" => value == "true" || value == "false",
            "integer" => value.parse::<i64>().is_ok(),
            "number" => value.parse::<f64>().is_ok(),
            "string" => true,
            _ => true,
        }
    }

    pub fn format_default(&self) -> String {
        format!("{} (default: {})", self.key, self.default)
    }
}


// ---------------------------------------------------------------------------
// wb_config – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XWbConfigLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XWbConfigPanelState {
    pub region: XWbConfigLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XWbConfigPanelState {
    pub fn new(region: XWbConfigLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_wb_config_total_visible_area(panels: &[XWbConfigPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_wb_config_count_in_region(
    panels: &[XWbConfigPanelState],
    region: XWbConfigLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_wb_config_widest_panel(panels: &[XWbConfigPanelState]) -> Option<&XWbConfigPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_wb_config_collapse_region(
    panels: &mut [XWbConfigPanelState],
    region: XWbConfigLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XWbConfigLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XWbConfigLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// wb_config – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for workbench configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWbConfigConfigMergeStrategy {
    Replace,
    Merge,
    Append,
    Ignore,
}

impl YWbConfigConfigMergeStrategy {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Replace => 0,
            Self::Merge => 1,
            Self::Append => 2,
            Self::Ignore => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Replace => "Replace",
            Self::Merge => "Merge",
            Self::Append => "Append",
            Self::Ignore => "Ignore",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWbConfigConfigMergeStrategy] {
        &[
            YWbConfigConfigMergeStrategy::Replace,
            YWbConfigConfigMergeStrategy::Merge,
            YWbConfigConfigMergeStrategy::Append,
            YWbConfigConfigMergeStrategy::Ignore,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWbConfigConfigMergeStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks config snapshot data.
#[derive(Debug, Clone)]
pub struct YWbConfigConfigSnapshot {
    pub entries: Vec<(String, String)>,
    pub timestamp: u64,
    pub label: String,
}

impl YWbConfigConfigSnapshot {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            timestamp: 0,
            label: String::new(),
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWbConfigConfigSnapshot({}: {:?})", "entries", self.entries)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_wb_config_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_wb_config_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_wb_config_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_wb_config_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_wb_config_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_wb_config_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_wb_config_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_wb_config_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// wb_config – Extended config diff report helpers
// ---------------------------------------------------------------------------

/// Priority levels for config diff report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWbConfigPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWbConfigPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZWbConfigPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWbConfigPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks config diff report data.
#[derive(Debug, Clone)]
pub struct ZWbConfigConfigDiffReport {
    pub changed_keys: Vec<String>,
    pub added_count: usize,
    pub removed_count: usize,
}

impl ZWbConfigConfigDiffReport {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            changed_keys: Vec::new(),
            added_count: 0,
            removed_count: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.changed_keys.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.changed_keys.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.changed_keys.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWbConfigConfigDiffReport[added_count={:?}, removed_count={:?}]", self.added_count, self.removed_count)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for config diff report.
pub fn z_wb_config_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_wb_config_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_wb_config_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_wb_config_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_wb_config_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_wb_config_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_wb_config_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
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

    #[test]
    fn config_editor_set_and_undo() {
        let model = ConfigurationModel::new();
        let mut editor = ConfigEditor::new(model);
        editor.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        assert_eq!(editor.model().get("editor.fontSize"), Some("14"));
        assert!(editor.undo());
        assert_eq!(editor.model().get("editor.fontSize"), None);
    }

    #[test]
    fn config_editor_remove_and_undo() {
        let mut model = ConfigurationModel::new();
        model.set("editor.tabSize".into(), "4".into(), ConfigurationScope::User);
        let mut editor = ConfigEditor::new(model);
        assert!(editor.remove("editor.tabSize"));
        assert_eq!(editor.model().get("editor.tabSize"), None);
        assert!(editor.undo());
        assert_eq!(editor.model().get("editor.tabSize"), Some("4"));
    }

    #[test]
    fn config_editor_undo_empty() {
        let mut editor = ConfigEditor::new(ConfigurationModel::new());
        assert!(!editor.undo());
    }

    #[test]
    fn config_editor_undo_count() {
        let mut editor = ConfigEditor::new(ConfigurationModel::new());
        editor.set("a.b".into(), "1".into(), ConfigurationScope::Default);
        editor.set("c.d".into(), "2".into(), ConfigurationScope::User);
        assert_eq!(editor.undo_count(), 2);
    }

    #[test]
    fn config_search_by_key() {
        let mut model = ConfigurationModel::new();
        model.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        model.set("editor.tabSize".into(), "4".into(), ConfigurationScope::User);
        model.set("terminal.fontSize".into(), "12".into(), ConfigurationScope::User);
        let results = config_search(&model, "editor");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn config_search_case_insensitive() {
        let mut model = ConfigurationModel::new();
        model.set("Editor.FontSize".into(), "14".into(), ConfigurationScope::User);
        let results = config_search(&model, "editor");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn config_validate_value_integer() {
        assert!(config_validate_value("42", ConfigValueType::Integer).is_ok());
        assert!(config_validate_value("abc", ConfigValueType::Integer).is_err());
    }

    #[test]
    fn config_validate_value_boolean() {
        assert!(config_validate_value("true", ConfigValueType::Boolean).is_ok());
        assert!(config_validate_value("false", ConfigValueType::Boolean).is_ok());
        assert!(config_validate_value("yes", ConfigValueType::Boolean).is_err());
    }

    #[test]
    fn config_snapshot_capture_and_restore() {
        let mut model = ConfigurationModel::new();
        model.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        model.set_with_description(
            "editor.tabSize".into(),
            "4".into(),
            ConfigurationScope::User,
            "Tab width".into(),
        );
        let snap = ConfigSnapshot::capture(&model, "before-refactor", 1000);
        assert_eq!(snap.label, "before-refactor");
        assert_eq!(snap.entry_count(), 2);

        // Mutate the original and restore
        model.clear();
        assert_eq!(model.entry_count(), 0);

        let restored = snap.restore();
        assert_eq!(restored.get("editor.fontSize"), Some("14"));
        assert_eq!(restored.get("editor.tabSize"), Some("4"));
        assert_eq!(restored.get_description("editor.tabSize"), Some("Tab width"));
    }

    #[test]
    fn config_snapshot_history_push_and_evict() {
        let mut history = ConfigSnapshotHistory::new(2);
        assert!(history.is_empty());

        let model = ConfigurationModel::new();
        history.push(ConfigSnapshot::capture(&model, "snap1", 100));
        history.push(ConfigSnapshot::capture(&model, "snap2", 200));
        assert_eq!(history.len(), 2);
        assert_eq!(history.labels(), vec!["snap1", "snap2"]);

        // Third push evicts the oldest
        history.push(ConfigSnapshot::capture(&model, "snap3", 300));
        assert_eq!(history.len(), 2);
        assert_eq!(history.labels(), vec!["snap2", "snap3"]);
        assert!(history.find_by_label("snap1").is_none());
        assert!(history.find_by_label("snap3").is_some());
        assert_eq!(history.latest().unwrap().label, "snap3");
    }

    #[test]
    fn config_migration_rename_keys() {
        let mut model = ConfigurationModel::new();
        model.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        model.set("editor.tabSize".into(), "4".into(), ConfigurationScope::Workspace);

        let migration = ConfigMigration::new()
            .rename("editor.fontSize", "editor.font.size")
            .rename("editor.tabSize", "editor.tab.size");
        assert_eq!(migration.rule_count(), 2);

        let count = migration.apply(&mut model);
        assert_eq!(count, 2);
        assert_eq!(model.get("editor.font.size"), Some("14"));
        assert_eq!(model.get("editor.tab.size"), Some("4"));
        assert!(!model.has("editor.fontSize"));
        assert!(!model.has("editor.tabSize"));
        // Scope is preserved
        let (_, scope) = model.get_with_scope("editor.tab.size").unwrap();
        assert_eq!(*scope, ConfigurationScope::Workspace);
    }

    #[test]
    fn config_migration_with_transform() {
        let mut model = ConfigurationModel::new();
        model.set("editor.wordWrap".into(), "on".into(), ConfigurationScope::User);

        let migration = ConfigMigration::new().rename_with_transform(
            "editor.wordWrap",
            "editor.word.wrap",
            |v| if v == "on" { "true".to_string() } else { "false".to_string() },
        );
        let count = migration.apply(&mut model);
        assert_eq!(count, 1);
        assert_eq!(model.get("editor.word.wrap"), Some("true"));
        assert!(!model.has("editor.wordWrap"));
    }

    #[test]
    fn config_exporter_to_kv_string() {
        let mut model = ConfigurationModel::new();
        model.set("b.key".into(), "2".into(), ConfigurationScope::User);
        model.set("a.key".into(), "1".into(), ConfigurationScope::User);

        let output = ConfigExporter::to_kv_string(&model);
        assert_eq!(output, "a.key = 1\nb.key = 2");
    }

    #[test]
    fn config_exporter_by_scope_filters() {
        let mut model = ConfigurationModel::new();
        model.set("a.key".into(), "1".into(), ConfigurationScope::User);
        model.set("b.key".into(), "2".into(), ConfigurationScope::Workspace);

        let user_output = ConfigExporter::to_kv_string_by_scope(&model, ConfigurationScope::User);
        assert_eq!(user_output, "a.key = 1");
        let ws_output = ConfigExporter::to_kv_string_by_scope(&model, ConfigurationScope::Workspace);
        assert_eq!(ws_output, "b.key = 2");
    }

    #[test]
    fn config_exporter_commented_kv_string() {
        let mut model = ConfigurationModel::new();
        model.set_with_description(
            "a.key".into(),
            "1".into(),
            ConfigurationScope::User,
            "The A key".into(),
        );
        model.set("b.key".into(), "2".into(), ConfigurationScope::User);

        let output = ConfigExporter::to_commented_kv_string(&model);
        assert!(output.contains("# The A key"));
        assert!(output.contains("a.key = 1"));
        assert!(output.contains("b.key = 2"));
    }

    #[test]
    fn config_key_segments_normal() {
        let segs = config_key_segments("editor.fontSize").unwrap();
        assert_eq!(segs, vec!["editor", "fontSize"]);
    }

    #[test]
    fn config_key_segments_empty() {
        assert!(config_key_segments("").is_none());
    }

    #[test]
    fn config_key_segments_single() {
        let segs = config_key_segments("editor").unwrap();
        assert_eq!(segs, vec!["editor"]);
    }

    #[test]
    fn config_key_namespace_extracts_first() {
        assert_eq!(config_key_namespace("editor.fontSize"), Some("editor"));
        assert_eq!(config_key_namespace("a.b.c"), Some("a"));
    }

    #[test]
    fn config_key_namespace_empty() {
        assert_eq!(config_key_namespace(""), None);
    }

    #[test]
    fn config_key_leaf_extracts_last() {
        assert_eq!(config_key_leaf("editor.fontSize"), Some("fontSize"));
        assert_eq!(config_key_leaf("a.b.c"), Some("c"));
    }

    #[test]
    fn config_key_leaf_no_dot() {
        assert_eq!(config_key_leaf("editor"), None);
    }

    #[test]
    fn config_key_depth_counts_segments() {
        assert_eq!(config_key_depth(""), 0);
        assert_eq!(config_key_depth("editor"), 1);
        assert_eq!(config_key_depth("editor.fontSize"), 2);
        assert_eq!(config_key_depth("a.b.c.d"), 4);
    }

    #[test]
    fn config_key_is_parent_true() {
        assert!(config_key_is_parent("editor", "editor.fontSize"));
        assert!(config_key_is_parent("a", "a.b.c"));
    }

    #[test]
    fn config_key_is_parent_false() {
        assert!(!config_key_is_parent("editor", "editor"));
        assert!(!config_key_is_parent("editor", "editorConfig.x"));
        assert!(!config_key_is_parent("", "a.b"));
    }

    #[test]
    fn config_namespaces_collects_unique() {
        let mut model = ConfigurationModel::new();
        model.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        model.set("editor.tabSize".into(), "4".into(), ConfigurationScope::User);
        model.set("terminal.shell".into(), "bash".into(), ConfigurationScope::User);
        let ns = config_namespaces(&model);
        assert_eq!(ns.len(), 2);
        assert!(ns.contains(&"editor".to_string()));
        assert!(ns.contains(&"terminal".to_string()));
    }

    #[test]
    fn config_group_by_namespace_groups_correctly() {
        let mut model = ConfigurationModel::new();
        model.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        model.set("editor.tabSize".into(), "4".into(), ConfigurationScope::User);
        model.set("terminal.shell".into(), "bash".into(), ConfigurationScope::Workspace);
        let groups = config_group_by_namespace(&model);
        assert_eq!(groups.get("editor").unwrap().len(), 2);
        assert_eq!(groups.get("terminal").unwrap().len(), 1);
    }

    #[test]
    fn config_keys_matching_wildcard() {
        let mut model = ConfigurationModel::new();
        model.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        model.set("editor.tabSize".into(), "4".into(), ConfigurationScope::User);
        model.set("terminal.shell".into(), "bash".into(), ConfigurationScope::Workspace);
        let matches = config_keys_matching(&model, "editor.*");
        assert_eq!(matches.len(), 2);
        assert!(matches.contains(&"editor.fontSize".to_string()));
        assert!(matches.contains(&"editor.tabSize".to_string()));
    }

    #[test]
    fn config_keys_matching_exact() {
        let mut model = ConfigurationModel::new();
        model.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        let matches = config_keys_matching(&model, "editor.fontSize");
        assert_eq!(matches.len(), 1);
        let no_match = config_keys_matching(&model, "nonexistent");
        assert!(no_match.is_empty());
    }

    #[test]
    fn count_by_scope_correct() {
        let mut model = ConfigurationModel::new();
        model.set("a".into(), "1".into(), ConfigurationScope::User);
        model.set("b".into(), "2".into(), ConfigurationScope::User);
        model.set("c".into(), "3".into(), ConfigurationScope::Workspace);
        let counts = count_by_scope(&model);
        assert_eq!(counts[&ConfigurationScope::User], 2);
        assert_eq!(counts[&ConfigurationScope::Workspace], 1);
    }

    #[test]
    fn config_to_sorted_pairs_sorted() {
        let mut model = ConfigurationModel::new();
        model.set("z.key".into(), "zval".into(), ConfigurationScope::User);
        model.set("a.key".into(), "aval".into(), ConfigurationScope::User);
        let pairs = config_to_sorted_pairs(&model);
        assert_eq!(pairs[0], "a.key=aval");
        assert_eq!(pairs[1], "z.key=zval");
    }

    #[test]
    fn numeric_config_keys_filters() {
        let mut model = ConfigurationModel::new();
        model.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        model.set("editor.wordWrap".into(), "on".into(), ConfigurationScope::User);
        model.set("editor.tabSize".into(), "4".into(), ConfigurationScope::User);
        let nums = numeric_config_keys(&model);
        assert_eq!(nums.len(), 2);
        assert!(!nums.contains(&"editor.wordWrap".to_string()));
    }

    #[test]
    fn boolean_config_keys_filters() {
        let mut model = ConfigurationModel::new();
        model.set("editor.minimap".into(), "true".into(), ConfigurationScope::User);
        model.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        model.set("editor.wordWrap".into(), "false".into(), ConfigurationScope::User);
        let bools = boolean_config_keys(&model);
        assert_eq!(bools.len(), 2);
    }

    #[test]
    fn apply_overrides_sets_values() {
        let mut model = ConfigurationModel::new();
        let count = apply_overrides(&mut model, &[("a", "1"), ("b", "2")], ConfigurationScope::Workspace);
        assert_eq!(count, 2);
        assert_eq!(model.get("a"), Some("1"));
        assert_eq!(model.get("b"), Some("2"));
    }

    #[test]
    fn config_entries_at_depth_works() {
        let mut model = ConfigurationModel::new();
        model.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        model.set("a.b.c".into(), "deep".into(), ConfigurationScope::User);
        model.set("simple".into(), "val".into(), ConfigurationScope::User);
        let depth2 = config_entries_at_depth(&model, 2);
        assert_eq!(depth2.len(), 1);
        assert!(depth2.contains(&"editor.fontSize".to_string()));
        let depth3 = config_entries_at_depth(&model, 3);
        assert_eq!(depth3.len(), 1);
    }

    #[test]
    fn config_summary_format() {
        let mut model = ConfigurationModel::new();
        model.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        model.set("terminal.shell".into(), "bash".into(), ConfigurationScope::User);
        let s = config_summary(&model);
        assert!(s.contains("2 entries"));
        assert!(s.contains("2 namespaces"));
    }

    // -----------------------------------------------------------------------
    // ConfigMigrator tests
    // -----------------------------------------------------------------------

    #[test]
    fn migrator_applies_rename_rules() {
        let mut model = ConfigurationModel::new();
        model.set("old.key".into(), "val".into(), ConfigurationScope::User);
        let migration = ConfigMigration::new().rename("old.key", "new.key");
        let migrator = ConfigMigrator::from_migration(&migration);
        let report = migrator.apply(&mut model);
        assert_eq!(report.applied_count(), 1);
        assert_eq!(report.skipped_count(), 0);
        assert_eq!(model.get("new.key"), Some("val"));
        assert!(!model.has("old.key"));
    }

    #[test]
    fn migrator_skips_absent_keys() {
        let mut model = ConfigurationModel::new();
        model.set("other.key".into(), "x".into(), ConfigurationScope::Default);
        let migration = ConfigMigration::new().rename("missing.key", "new.key");
        let migrator = ConfigMigrator::from_migration(&migration);
        let report = migrator.apply(&mut model);
        assert_eq!(report.applied_count(), 0);
        assert_eq!(report.skipped_count(), 1);
        assert!(report.skipped.contains(&"missing.key".to_string()));
    }

    #[test]
    fn migrator_applies_value_transform() {
        let mut model = ConfigurationModel::new();
        model.set("editor.tabSize".into(), "4".into(), ConfigurationScope::User);
        let migration = ConfigMigration::new()
            .rename_with_transform("editor.tabSize", "editor.indentSize", |v| {
                format!("{}px", v)
            });
        let migrator = ConfigMigrator::from_migration(&migration);
        let report = migrator.apply(&mut model);
        assert_eq!(report.applied_count(), 1);
        assert_eq!(model.get("editor.indentSize"), Some("4px"));
    }

    #[test]
    fn migrator_report_display() {
        let report = ConfigMigratorReport {
            applied: vec!["a.b".into()],
            skipped: vec!["c.d".into(), "e.f".into()],
        };
        let s = format!("{}", report);
        assert!(s.contains("migrated 1"));
        assert!(s.contains("skipped 2"));
    }

    // -----------------------------------------------------------------------
    // ConfigProfileManager tests
    // -----------------------------------------------------------------------

    #[test]
    fn profile_manager_save_and_load() {
        let mut pm = ConfigProfileManager::new();
        let mut model = ConfigurationModel::new();
        model.set("editor.fontSize".into(), "14".into(), ConfigurationScope::User);
        pm.save("default", &model, 1000);
        let loaded = pm.load("default").unwrap();
        assert_eq!(loaded.get("editor.fontSize"), Some("14"));
    }

    #[test]
    fn profile_manager_switch_and_active() {
        let mut pm = ConfigProfileManager::new();
        let model = ConfigurationModel::new();
        pm.save("profile-a", &model, 1);
        pm.save("profile-b", &model, 2);
        assert!(pm.switch("profile-a"));
        assert_eq!(pm.active(), Some("profile-a"));
        assert!(!pm.switch("nonexistent"));
        assert_eq!(pm.active(), Some("profile-a"));
    }

    #[test]
    fn profile_manager_remove_clears_active() {
        let mut pm = ConfigProfileManager::new();
        let model = ConfigurationModel::new();
        pm.save("temp", &model, 1);
        pm.switch("temp");
        assert!(pm.remove("temp"));
        assert_eq!(pm.active(), None);
        assert_eq!(pm.count(), 0);
    }

    #[test]
    fn profile_manager_names_sorted() {
        let mut pm = ConfigProfileManager::new();
        let model = ConfigurationModel::new();
        pm.save("zeta", &model, 1);
        pm.save("alpha", &model, 2);
        pm.save("mid", &model, 3);
        assert_eq!(pm.names(), vec!["alpha", "mid", "zeta"]);
    }

    // -----------------------------------------------------------------------
    // ConfigProfileDiff tests
    // -----------------------------------------------------------------------

    #[test]
    fn profile_diff_detects_added_removed_changed() {
        let mut old = ConfigurationModel::new();
        old.set("keep.same".into(), "1".into(), ConfigurationScope::User);
        old.set("will.change".into(), "old".into(), ConfigurationScope::User);
        old.set("will.remove".into(), "gone".into(), ConfigurationScope::User);

        let mut new = ConfigurationModel::new();
        new.set("keep.same".into(), "1".into(), ConfigurationScope::User);
        new.set("will.change".into(), "new".into(), ConfigurationScope::User);
        new.set("will.add".into(), "fresh".into(), ConfigurationScope::User);

        let diff = ConfigProfileDiff::compute(&old, &new);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].0, "will.add");
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].0, "will.remove");
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.changed[0].old_value, "old");
        assert_eq!(diff.changed[0].new_value, "new");
        assert_eq!(diff.total(), 3);
        assert!(!diff.is_empty());
    }

    #[test]
    fn profile_diff_empty_for_identical_models() {
        let mut m = ConfigurationModel::new();
        m.set("a.b".into(), "1".into(), ConfigurationScope::Default);
        let diff = ConfigProfileDiff::compute(&m, &m);
        assert!(diff.is_empty());
        assert_eq!(diff.total(), 0);
    }

    #[test]
    fn profile_diff_display() {
        let old = ConfigurationModel::new();
        let mut new = ConfigurationModel::new();
        new.set("x.y".into(), "1".into(), ConfigurationScope::User);
        let diff = ConfigProfileDiff::compute(&old, &new);
        let s = format!("{}", diff);
        assert!(s.contains("+1"));
        assert!(s.contains("-0"));
    }

    // -----------------------------------------------------------------------
    // ConfigImportValidator tests
    // -----------------------------------------------------------------------

    #[test]
    fn import_validator_accepts_valid_entries() {
        let validator = ConfigImportValidator::new();
        let entry = ConfigurationEntry {
            key: "editor.fontSize".into(),
            value: "14".into(),
            scope: ConfigurationScope::User,
            description: None,
        };
        assert!(validator.is_valid(&[entry]));
    }

    #[test]
    fn import_validator_rejects_bad_key() {
        let validator = ConfigImportValidator::new();
        let entry = ConfigurationEntry {
            key: "".into(),
            value: "v".into(),
            scope: ConfigurationScope::User,
            description: None,
        };
        let errors = validator.validate_entry(&entry);
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], ConfigImportError::InvalidKey(_)));
    }

    #[test]
    fn import_validator_rejects_long_value() {
        let validator = ConfigImportValidator::new().max_value_len(5);
        let entry = ConfigurationEntry {
            key: "a.b".into(),
            value: "toolong".into(),
            scope: ConfigurationScope::User,
            description: None,
        };
        let errors = validator.validate_entry(&entry);
        assert!(errors.iter().any(|e| matches!(e, ConfigImportError::InvalidValue { .. })));
    }

    #[test]
    fn import_validator_rejects_disallowed_scope() {
        let validator = ConfigImportValidator::new()
            .allowed_scopes(vec![ConfigurationScope::User]);
        let entry = ConfigurationEntry {
            key: "a.b".into(),
            value: "v".into(),
            scope: ConfigurationScope::Memory,
            description: None,
        };
        let errors = validator.validate_entry(&entry);
        assert!(errors.iter().any(|e| matches!(e, ConfigImportError::InvalidScope(_))));
    }

    // -- ConfigOverrideChain -----------------------------------------------

    #[test]
    fn override_chain_effective_value() {
        let mut chain = ConfigOverrideChain::new();
        chain.override_at_level(ConfigurationScope::Default, "editor.fontSize", "14");
        chain.override_at_level(ConfigurationScope::User, "editor.fontSize", "16");
        assert_eq!(chain.get_effective_value("editor.fontSize"), Some("16"));
    }

    #[test]
    fn override_chain_source_of() {
        let mut chain = ConfigOverrideChain::new();
        chain.override_at_level(ConfigurationScope::Default, "k", "v");
        assert_eq!(chain.get_source_of("k"), Some(ConfigurationScope::Default));
    }

    #[test]
    fn override_chain_workspace_overrides_user() {
        let mut chain = ConfigOverrideChain::new();
        chain.override_at_level(ConfigurationScope::User, "k", "user");
        chain.override_at_level(ConfigurationScope::Workspace, "k", "ws");
        assert_eq!(chain.get_effective_value("k"), Some("ws"));
    }

    #[test]
    fn override_chain_reset_level() {
        let mut chain = ConfigOverrideChain::new();
        chain.override_at_level(ConfigurationScope::User, "k", "v");
        chain.reset_level(ConfigurationScope::User);
        assert_eq!(chain.get_effective_value("k"), None);
    }

    // -- ConfigChangeEvent -------------------------------------------------

    #[test]
    fn change_event_is_addition() {
        let ev = ConfigChangeEvent::new("k", None, Some("v"), ConfigurationScope::User);
        assert!(ev.is_addition());
        assert!(!ev.is_removal());
        assert!(!ev.is_modification());
    }

    #[test]
    fn change_event_is_removal() {
        let ev = ConfigChangeEvent::new("k", Some("v"), None, ConfigurationScope::User);
        assert!(ev.is_removal());
    }

    #[test]
    fn change_event_is_modification() {
        let ev = ConfigChangeEvent::new("k", Some("a"), Some("b"), ConfigurationScope::User);
        assert!(ev.is_modification());
    }

    #[test]
    fn change_event_affects_key() {
        let ev = ConfigChangeEvent::new("editor.fontSize", None, Some("14"), ConfigurationScope::User);
        assert!(ev.affects_key("editor.fontSize"));
        assert!(ev.affects_key("editor"));
        assert!(!ev.affects_key("terminal"));
    }

    // -- ConfigSchemaEntry -------------------------------------------------

    #[test]
    fn schema_validate_boolean() {
        let entry = ConfigSchemaEntry::new("k", "boolean", "true", "desc");
        assert!(entry.validate_value("true"));
        assert!(entry.validate_value("false"));
        assert!(!entry.validate_value("yes"));
    }

    #[test]
    fn schema_validate_enum() {
        let entry = ConfigSchemaEntry::new("k", "string", "a", "desc")
            .with_enum_values(vec!["a".into(), "b".into()]);
        assert!(entry.validate_value("a"));
        assert!(!entry.validate_value("c"));
    }

    #[test]
    fn schema_format_default() {
        let entry = ConfigSchemaEntry::new("editor.tabSize", "integer", "4", "Tab size");
        assert_eq!(entry.format_default(), "editor.tabSize (default: 4)");
    }

    #[test]
    fn schema_validate_integer() {
        let entry = ConfigSchemaEntry::new("k", "integer", "0", "desc");
        assert!(entry.validate_value("42"));
        assert!(!entry.validate_value("abc"));
    }


    // -- wb_config additional tests -------------------------------------------

    #[test]
    fn x_wb_config_panel_state_new() {
        let p = XWbConfigPanelState::new(XWbConfigLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XWbConfigLayoutRegion::Sidebar);
    }

    #[test]
    fn x_wb_config_panel_area() {
        let p = XWbConfigPanelState::new(XWbConfigLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_wb_config_panel_toggle() {
        let mut p = XWbConfigPanelState::new(XWbConfigLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_wb_config_panel_resize() {
        let mut p = XWbConfigPanelState::new(XWbConfigLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_wb_config_panel_is_narrow() {
        let mut p = XWbConfigPanelState::new(XWbConfigLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_wb_config_total_visible_area_basic() {
        let panels = vec![
            XWbConfigPanelState::new(XWbConfigLayoutRegion::Sidebar, "a"),
            XWbConfigPanelState::new(XWbConfigLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_wb_config_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_wb_config_total_visible_area_hidden() {
        let mut panels = vec![
            XWbConfigPanelState::new(XWbConfigLayoutRegion::Sidebar, "a"),
            XWbConfigPanelState::new(XWbConfigLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_wb_config_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_wb_config_count_in_region_basic() {
        let panels = vec![
            XWbConfigPanelState::new(XWbConfigLayoutRegion::Sidebar, "a"),
            XWbConfigPanelState::new(XWbConfigLayoutRegion::Sidebar, "b"),
            XWbConfigPanelState::new(XWbConfigLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_wb_config_count_in_region(&panels, XWbConfigLayoutRegion::Sidebar), 2);
        assert_eq!(x_wb_config_count_in_region(&panels, XWbConfigLayoutRegion::Editor), 1);
        assert_eq!(x_wb_config_count_in_region(&panels, XWbConfigLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_wb_config_widest_panel_basic() {
        let mut panels = vec![
            XWbConfigPanelState::new(XWbConfigLayoutRegion::Sidebar, "narrow"),
            XWbConfigPanelState::new(XWbConfigLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_wb_config_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_wb_config_collapse_region_basic() {
        let mut panels = vec![
            XWbConfigPanelState::new(XWbConfigLayoutRegion::Sidebar, "a"),
            XWbConfigPanelState::new(XWbConfigLayoutRegion::Sidebar, "b"),
            XWbConfigPanelState::new(XWbConfigLayoutRegion::Editor, "c"),
        ];
        x_wb_config_collapse_region(&mut panels, XWbConfigLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_wb_config_layout_constraint_clamp() {
        let lc = XWbConfigLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_wb_config_layout_constraint_satisfied() {
        let lc = XWbConfigLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_wb_config_widest_panel_empty() {
        let panels: Vec<XWbConfigPanelState> = vec![];
        assert!(x_wb_config_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_wb_config_layout_region_eq() {
        assert_eq!(XWbConfigLayoutRegion::Sidebar, XWbConfigLayoutRegion::Sidebar);
        assert_ne!(XWbConfigLayoutRegion::Sidebar, XWbConfigLayoutRegion::Panel);
    }


    // -- wb_config extended domain tests ----------------------------------------

    #[test]
    fn y_wb_config_enum_index() {
        assert_eq!(YWbConfigConfigMergeStrategy::Replace.index(), 0);
        assert_eq!(YWbConfigConfigMergeStrategy::Merge.index(), 1);
        assert_eq!(YWbConfigConfigMergeStrategy::Append.index(), 2);
        assert_eq!(YWbConfigConfigMergeStrategy::Ignore.index(), 3);
    }

    #[test]
    fn y_wb_config_enum_label() {
        assert_eq!(YWbConfigConfigMergeStrategy::Replace.label(), "Replace");
        assert_eq!(YWbConfigConfigMergeStrategy::Merge.label(), "Merge");
        assert_eq!(YWbConfigConfigMergeStrategy::Append.label(), "Append");
        assert_eq!(YWbConfigConfigMergeStrategy::Ignore.label(), "Ignore");
    }

    #[test]
    fn y_wb_config_enum_all() {
        let all = YWbConfigConfigMergeStrategy::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_wb_config_enum_is_default() {
        assert!(YWbConfigConfigMergeStrategy::Replace.is_default());
        assert!(!YWbConfigConfigMergeStrategy::Ignore.is_default());
    }

    #[test]
    fn y_wb_config_enum_display() {
        assert_eq!(format!("{}", YWbConfigConfigMergeStrategy::Replace), "Replace");
    }

    #[test]
    fn y_wb_config_struct_new() {
        let s = YWbConfigConfigSnapshot::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_wb_config_struct_clear() {
        let mut s = YWbConfigConfigSnapshot::new();
        s.entries.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_wb_config_fingerprint_deterministic() {
        let h1 = y_wb_config_fingerprint("hello");
        let h2 = y_wb_config_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_wb_config_fingerprint("a"), y_wb_config_fingerprint("b"));
    }

    #[test]
    fn y_wb_config_truncate_short() {
        assert_eq!(y_wb_config_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_wb_config_truncate_long() {
        let r = y_wb_config_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_wb_config_normalize_key_basic() {
        assert_eq!(y_wb_config_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_wb_config_split_path_basic() {
        let parts = y_wb_config_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_wb_config_count_occurrences_basic() {
        assert_eq!(y_wb_config_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_wb_config_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_wb_config_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_wb_config_in_range_basic() {
        assert!(y_wb_config_in_range(5, 1, 10));
        assert!(y_wb_config_in_range(1, 1, 10));
        assert!(y_wb_config_in_range(10, 1, 10));
        assert!(!y_wb_config_in_range(0, 1, 10));
        assert!(!y_wb_config_in_range(11, 1, 10));
    }

    #[test]
    fn y_wb_config_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_wb_config_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_wb_config_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_wb_config_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- wb_config Z-extended tests -----------------------------------------------

    #[test]
    fn z_wb_config_priority_weight() {
        assert_eq!(ZWbConfigPriority::Idle.weight(), 0);
        assert_eq!(ZWbConfigPriority::Normal.weight(), 2);
        assert_eq!(ZWbConfigPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_wb_config_priority_label() {
        assert_eq!(ZWbConfigPriority::Low.label(), "low");
        assert_eq!(ZWbConfigPriority::High.label(), "high");
    }

    #[test]
    fn z_wb_config_priority_is_elevated() {
        assert!(!ZWbConfigPriority::Normal.is_elevated());
        assert!(ZWbConfigPriority::High.is_elevated());
        assert!(ZWbConfigPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_wb_config_priority_display() {
        assert_eq!(format!("{}", ZWbConfigPriority::Idle), "idle");
    }

    #[test]
    fn z_wb_config_priority_all_asc() {
        let all = ZWbConfigPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWbConfigPriority::Idle);
        assert_eq!(all[4], ZWbConfigPriority::Realtime);
    }

    #[test]
    fn z_wb_config_struct_new() {
        let s = ZWbConfigConfigDiffReport::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_wb_config_struct_toggled_clone() {
        let s = ZWbConfigConfigDiffReport::new();
        let t = s.toggled_clone();
        let _ = t.removed_count;
    }

    #[test]
    fn z_wb_config_rolling_hash_deterministic() {
        let h1 = z_wb_config_rolling_hash(b"test");
        let h2 = z_wb_config_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_wb_config_rolling_hash(b"a"), z_wb_config_rolling_hash(b"b"));
    }

    #[test]
    fn z_wb_config_pad_to_basic() {
        assert_eq!(z_wb_config_pad_to("hi", 5), "hi   ");
        assert_eq!(z_wb_config_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_wb_config_is_identifier_basic() {
        assert!(z_wb_config_is_identifier("foo_bar"));
        assert!(z_wb_config_is_identifier("abc123"));
        assert!(!z_wb_config_is_identifier(""));
        assert!(!z_wb_config_is_identifier("has space"));
    }

    #[test]
    fn z_wb_config_levenshtein_basic() {
        assert_eq!(z_wb_config_levenshtein("", ""), 0);
        assert_eq!(z_wb_config_levenshtein("abc", "abc"), 0);
        assert_eq!(z_wb_config_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_wb_config_unique_words_basic() {
        let w = z_wb_config_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_wb_config_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_wb_config_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_wb_config_common_prefix_basic() {
        assert_eq!(z_wb_config_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_wb_config_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_wb_config_struct_clear() {
        let mut s = ZWbConfigConfigDiffReport::new();
        s.changed_keys.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_wb_config_rolling_hash_empty() {
        let h = z_wb_config_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }
}
