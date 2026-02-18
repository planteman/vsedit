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


// ---------------------------------------------------------------------------
// xb_ utilities – batch 62
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer62 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer62 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_62(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_62<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_62<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_62(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_62(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 205
// ---------------------------------------------------------------------------

/// Generic object pool `Xc205Pool<T>`.
pub struct Xc205Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc205Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc205PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc205Pool<T> {
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
    pub fn stats(&self) -> Xc205PoolStats {
        Xc205PoolStats {
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

impl<T> Default for Xc205Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc205Scheduler`.
pub struct Xc205Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc205Scheduler {
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

impl Default for Xc205Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_205 hash for the given byte slice.
pub fn xc_205_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_205 convention.
pub fn xc_205_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe75 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe75Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe75PipelineError {
    pub stage: Xe75Stage,
    pub message: String,
}

impl std::fmt::Display for Xe75PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe75Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe75Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe75PipelineError>>>,
    stage_names: Vec<Xe75Stage>,
}

impl Xe75Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe75PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe75Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe75PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe75Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe75PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe75Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe75PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe75Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe75PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe75Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe75CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe75CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe75Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe75CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe75CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe75Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe75CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_75_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe75CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_75_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe75CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_75_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe75PipelineError> {
    Ok(data)
}

pub fn xe_75_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe75PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_75_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe75PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_75_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe75PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_75_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe75PipelineError> {
    Err(Xe75PipelineError {
        stage: Xe75Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_73: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg73Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg73Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg73Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_73: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg73Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg73Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg73Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg73Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 204).
pub struct Xh204SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh204SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 246 as u64,
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

/// A compact bit set supporting boolean operations (variant 204).
pub struct Xh204BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh204BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 204).
pub struct Xi204Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi204Deque<T> {
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
pub struct Xi204Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi204Interval {
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

/// A simple interval tree (variant 204).
pub struct Xi204IntervalTree {
    xi_intervals: Vec<Xi204Interval>,
}

impl Xi204IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi204Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi204Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi204Interval) -> Vec<&Xi204Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi204Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi204Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi204Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi204Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi204Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi204Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 204) ---

/// Disjoint set / union-find for crate 204.
pub struct Xj204UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj204UnionFind {
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

const XJ204_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 204.
pub struct Xj204BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj204BTreeNode<K, V>>>,
    len: usize,
}

struct Xj204BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj204BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj204BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ204_BTREE_ORDER - 1
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
        let mid = XJ204_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj204BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj204BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj204BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj204BTreeNode::xj_new_leaf();
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


// --- xk_204 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk204SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk204SegmentTree {
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
pub struct Xk204DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk204DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_204).
#[derive(Debug, Clone)]
pub struct Xl204Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl204Rope {
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

/// Suffix array for efficient string searching (xl_204).
#[derive(Debug, Clone)]
pub struct Xl204SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl204SuffixArray {
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
pub struct Xm204MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm204MatrixSparse {
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
pub struct Xm204Tokenizer {
    text: String,
}

impl Xm204Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 204.
pub struct Xn204Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn204Fenwick {
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

// ----- AVL tree map — crate 204 -----

#[derive(Debug, Clone)]
struct Xn204AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn204AvlNode<K, V>>>,
    right: Option<Box<Xn204AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 204.
#[derive(Debug, Clone)]
pub struct Xn204AVL<K, V> {
    root: Option<Box<Xn204AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn204AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn204AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn204AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn204AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn204AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn204AvlNode<K, V>>) -> Box<Xn204AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn204AvlNode<K, V>>) -> Box<Xn204AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn204AvlNode<K, V>>) -> Box<Xn204AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn204AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn204AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn204AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn204AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn204AvlNode<K, V>>) -> &Xn204AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn204AvlNode<K, V>>) -> (Box<Xn204AvlNode<K, V>>, Option<Box<Xn204AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn204AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn204AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn204AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn204AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn204AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn204AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn204AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo204RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo204Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo204RBNode<K, V> {
    key: K,
    value: V,
    color: Xo204Color,
    left: Option<Box<Xo204RBNode<K, V>>>,
    right: Option<Box<Xo204RBNode<K, V>>>,
}

/// A red-black tree map for crate 204.
#[derive(Debug, Clone)]
pub struct Xo204RedBlack<K, V> {
    root: Option<Box<Xo204RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo204RedBlack<K, V> {
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
            r.color = Xo204Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo204RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo204RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo204RBNode {
                    key, value, color: Xo204Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo204RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo204Color::Red)
    }

    fn xo_balance(mut h: Box<Xo204RBNode<K, V>>) -> Box<Xo204RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo204Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo204RBNode<K, V>>) -> Box<Xo204RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo204Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo204RBNode<K, V>>) -> Box<Xo204RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo204Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo204RBNode<K, V>>) {
        h.color = Xo204Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo204Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo204Color::Black; }
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
            r.color = Xo204Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo204RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo204RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo204RBNode<K, V>) -> (K, V, Option<Box<Xo204RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo204RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo204Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo204RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo204ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 204.
#[derive(Debug, Clone)]
pub struct Xo204ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo204ConsistentHash {
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
            let vkey = format!("{}#xo204#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo204#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 204).
#[derive(Debug)]
pub struct Xp204SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp204Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp204Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp204Node<K, V>>>,
    xp_right: Option<Box<Xp204Node<K, V>>>,
}

impl<K: Ord, V> Xp204Node<K, V> {
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

impl<K: Ord, V> Default for Xp204SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp204SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp204Node<K, V>>>, key: &K) -> Option<Box<Xp204Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp204Node<K, V>>) -> Box<Xp204Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp204Node<K, V>>) -> Box<Xp204Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp204Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp204Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp204Node::xp_new(key, val));
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


// --------------- Xq204Treap ---------------

use std::cmp::Ordering as Xq204Ord;

struct Xq204TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq204TreapNode<K, V>>>,
    right: Option<Box<Xq204TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq204Treap<K, V> {
    root: Option<Box<Xq204TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq204TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_204_size<K, V>(node: &Option<Box<Xq204TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_204_update_size<K, V>(node: &mut Xq204TreapNode<K, V>) {
    node.size = 1 + xq_204_size(&node.left) + xq_204_size(&node.right);
}

fn xq_204_rotate_right<K, V>(mut node: Box<Xq204TreapNode<K, V>>) -> Box<Xq204TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_204_update_size(&mut node);
    left.right = Some(node);
    xq_204_update_size(&mut left);
    left
}

fn xq_204_rotate_left<K, V>(mut node: Box<Xq204TreapNode<K, V>>) -> Box<Xq204TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_204_update_size(&mut node);
    right.left = Some(node);
    xq_204_update_size(&mut right);
    right
}

fn xq_204_insert_node<K: Ord, V>(
    node: Option<Box<Xq204TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq204TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq204TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq204Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq204Ord::Less => {
                let (new_left, old) = xq_204_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_204_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_204_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq204Ord::Greater => {
                let (new_right, old) = xq_204_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_204_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_204_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_204_remove_node<K: Ord, V>(
    node: Option<Box<Xq204TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq204TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq204Ord::Less => {
                let (new_left, old) = xq_204_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_204_update_size(&mut n);
                (Some(n), old)
            }
            Xq204Ord::Greater => {
                let (new_right, old) = xq_204_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_204_update_size(&mut n);
                (Some(n), old)
            }
            Xq204Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_204_rotate_right(n);
                    let (new_right, old) = xq_204_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_204_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_204_rotate_left(n);
                    let (new_left, old) = xq_204_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_204_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_204_find_min<K, V>(node: &Option<Box<Xq204TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_204_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_204_find_max<K, V>(node: &Option<Box<Xq204TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_204_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_204_rank<K: Ord, V>(node: &Option<Box<Xq204TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq204Ord::Less => xq_204_rank(&n.left, key),
            Xq204Ord::Equal => xq_204_size(&n.left),
            Xq204Ord::Greater => 1 + xq_204_size(&n.left) + xq_204_rank(&n.right, key),
        },
    }
}

fn xq_204_kth<K, V>(node: &Option<Box<Xq204TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_204_size(&n.left);
        if k < left_size {
            xq_204_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_204_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_204_in_order<K: Clone, V>(node: &Option<Box<Xq204TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_204_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_204_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq204Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 204 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_204_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq204Ord::Equal => return Some(&n.value),
                Xq204Ord::Less => cur = &n.left,
                Xq204Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_204_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_204_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_204_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_204_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_204_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_204_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_204_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq204VEBTree ---------------

pub struct Xq204VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq204VEBTree>>,
    clusters: Vec<Option<Box<Xq204VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq204VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq204VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq204VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
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

    #[test]
    fn xb_ring_buffer_62_push_and_len() {
        let mut rb = super::XbRingBuffer62::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_62_overwrite() {
        let mut rb = super::XbRingBuffer62::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_62_get_out_of_bounds() {
        let rb = super::XbRingBuffer62::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_62_drain_all() {
        let mut rb = super::XbRingBuffer62::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_62_peek_front_back() {
        let mut rb = super::XbRingBuffer62::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_62_clear() {
        let mut rb = super::XbRingBuffer62::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_62_capacity() {
        let rb = super::XbRingBuffer62::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_62_basic() {
        let h = super::xb_fnv1a_62(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_62(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_62_different_inputs() {
        let h1 = super::xb_fnv1a_62(b"abc");
        let h2 = super::xb_fnv1a_62(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_62_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_62(&data);
        let dec = super::xb_rle_decode_62(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_62_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_62(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_62(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_62_values() {
        assert!((super::xb_clamp_62(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_62(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_62(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_62_values() {
        assert!((super::xb_lerp_62(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_62(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_62(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_62_wrap_around_twice() {
        let mut rb = super::XbRingBuffer62::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 205 ----

    #[test]
    fn xc_205_pool_new_empty() {
        let pool: super::Xc205Pool<i32> = super::Xc205Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_205_pool_release_acquire() {
        let mut pool = super::Xc205Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_205_pool_acquire_empty() {
        let mut pool: super::Xc205Pool<i32> = super::Xc205Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_205_pool_full() {
        let mut pool = super::Xc205Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_205_pool_drain() {
        let mut pool = super::Xc205Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_205_pool_stats() {
        let mut pool = super::Xc205Pool::new(8);
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
    fn xc_205_pool_clear() {
        let mut pool = super::Xc205Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_205_pool_shrink() {
        let mut pool = super::Xc205Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_205_pool_default() {
        let pool: super::Xc205Pool<String> = super::Xc205Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_205_pool_extend() {
        let mut pool = super::Xc205Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_205_pool_retain() {
        let mut pool = super::Xc205Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_205_scheduler_round_robin() {
        let mut sched = super::Xc205Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_205_scheduler_empty() {
        let mut sched = super::Xc205Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_205_scheduler_reset() {
        let mut sched = super::Xc205Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_205_scheduler_add_remove() {
        let mut sched = super::Xc205Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_205_scheduler_targets() {
        let sched = super::Xc205Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_205_hash_empty() {
        assert_eq!(super::xc_205_hash(b""), 5381);
    }

    #[test]
    fn xc_205_hash_data() {
        let h = super::xc_205_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_205_hash(b"hello"), h);
    }

    #[test]
    fn xc_205_reverse_str() {
        assert_eq!(super::xc_205_reverse("abc"), "cba");
        assert_eq!(super::xc_205_reverse(""), "");
    }


    #[test]
    fn xe_75_pipeline_empty() {
        let p = super::Xe75Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_75_pipeline_parse_stage() {
        let p = super::Xe75Pipeline::new()
            .add_parse(super::xe_75_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_75_pipeline_transform_double() {
        let p = super::Xe75Pipeline::new()
            .add_transform(super::xe_75_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_75_pipeline_validate_reverse() {
        let p = super::Xe75Pipeline::new()
            .add_validate(super::xe_75_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_75_pipeline_emit_filter() {
        let p = super::Xe75Pipeline::new()
            .add_emit(super::xe_75_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_75_pipeline_multi_stage() {
        let p = super::Xe75Pipeline::new()
            .add_parse(super::xe_75_pipeline_identity)
            .add_transform(super::xe_75_pipeline_double)
            .add_validate(super::xe_75_pipeline_reverse)
            .add_emit(super::xe_75_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_75_pipeline_error_propagation() {
        let p = super::Xe75Pipeline::new()
            .add_parse(super::xe_75_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe75Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_75_pipeline_compose() {
        let p1 = super::Xe75Pipeline::new()
            .add_parse(super::xe_75_pipeline_identity);
        let p2 = super::Xe75Pipeline::new()
            .add_transform(super::xe_75_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_75_pipeline_error_display() {
        let e = super::Xe75PipelineError {
            stage: super::Xe75Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_75_cache_put_get() {
        let mut c = super::Xe75Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_75_cache_miss() {
        let mut c: super::Xe75Cache<&str, i32> = super::Xe75Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_75_cache_ttl_expiry() {
        let mut c = super::Xe75Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_75_cache_evict() {
        let mut c = super::Xe75Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_75_cache_capacity() {
        let mut c = super::Xe75Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_75_cache_stats() {
        let mut c = super::Xe75Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_75_cache_clear() {
        let mut c = super::Xe75Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_73 graph tests ------------------------------------------------

    #[test]
    fn xg_73_graph_empty() {
        let g = super::Xg73Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_73_graph_add_node() {
        let mut g = super::Xg73Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_73_graph_add_edge() {
        let mut g = super::Xg73Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_73_graph_neighbors() {
        let mut g = super::Xg73Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_73_graph_has_path() {
        let mut g = super::Xg73Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_73_graph_self_path() {
        let g = super::Xg73Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_73_graph_topo_sort() {
        let mut g = super::Xg73Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_73_graph_cycle_detect_false() {
        let mut g = super::Xg73Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_73_graph_cycle_detect_true() {
        let mut g = super::Xg73Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_73 heap tests -------------------------------------------------

    #[test]
    fn xg_73_heap_empty() {
        let h: super::Xg73Heap<i32> = super::Xg73Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_73_heap_push_pop() {
        let mut h = super::Xg73Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_73_heap_peek() {
        let mut h = super::Xg73Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_73_heap_drain_sorted() {
        let mut h = super::Xg73Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_73_heap_merge() {
        let mut a = super::Xg73Heap::new();
        let mut b = super::Xg73Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_73_heap_default() {
        let h: super::Xg73Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_73_graph_default() {
        let g: super::Xg73Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh204_skip_insert_contains() {
        let mut sl = super::Xh204SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh204_skip_remove() {
        let mut sl = super::Xh204SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh204_skip_len() {
        let mut sl = super::Xh204SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh204_skip_range_query() {
        let mut sl = super::Xh204SkipList::xh_new(4);
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
    fn xh204_skip_floor_ceiling() {
        let mut sl = super::Xh204SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh204_skip_rank() {
        let mut sl = super::Xh204SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh204_skip_empty() {
        let sl = super::Xh204SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh204_skip_duplicates() {
        let mut sl = super::Xh204SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh204_bitset_set_test() {
        let mut bs = super::Xh204BitSet::xh_new(256);
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
    fn xh204_bitset_clear_count() {
        let mut bs = super::Xh204BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh204_bitset_and_or_xor() {
        let mut a = super::Xh204BitSet::xh_new(128);
        let mut b = super::Xh204BitSet::xh_new(128);
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
    fn xh204_bitset_iter_ones() {
        let mut bs = super::Xh204BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh204_bitset_first_last() {
        let mut bs = super::Xh204BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh204_bitset_empty() {
        let bs = super::Xh204BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi204_deque_push_pop_back() {
        let mut dq = super::Xi204Deque::xi_new(4);
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
    fn xi204_deque_push_pop_front() {
        let mut dq = super::Xi204Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi204_deque_mixed_ops() {
        let mut dq = super::Xi204Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi204_deque_get_and_split() {
        let mut dq = super::Xi204Deque::xi_new(8);
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
    fn xi204_deque_rotate_left() {
        let mut dq = super::Xi204Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi204_deque_rotate_right() {
        let mut dq = super::Xi204Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi204_deque_grow() {
        let mut dq = super::Xi204Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi204_deque_empty() {
        let dq = super::Xi204Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi204_interval_tree_insert_query() {
        let mut tree = super::Xi204IntervalTree::xi_new();
        tree.xi_insert(super::Xi204Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi204Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi204Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi204_interval_tree_overlap() {
        let mut tree = super::Xi204IntervalTree::xi_new();
        tree.xi_insert(super::Xi204Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi204Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi204Interval::xi_new(12, 20));
        let q = super::Xi204Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi204_interval_tree_remove() {
        let mut tree = super::Xi204IntervalTree::xi_new();
        tree.xi_insert(super::Xi204Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi204Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi204_interval_tree_gaps() {
        let mut tree = super::Xi204IntervalTree::xi_new();
        tree.xi_insert(super::Xi204Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi204Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi204Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi204Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi204Interval::xi_new(8, 10));
    }

    #[test]
    fn xi204_interval_tree_merge() {
        let mut tree = super::Xi204IntervalTree::xi_new();
        tree.xi_insert(super::Xi204Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi204Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi204Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi204Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi204Interval::xi_new(10, 15));
    }

    #[test]
    fn xi204_interval_tree_all() {
        let mut tree = super::Xi204IntervalTree::xi_new();
        tree.xi_insert(super::Xi204Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi204Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi204_interval_tree_empty() {
        let tree = super::Xi204IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi204_interval_tree_contains_point() {
        let iv = super::Xi204Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 204) ---

    #[test]
    fn xj_204_uf_make_and_find() {
        let mut uf = super::Xj204UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_204_uf_union_connected() {
        let mut uf = super::Xj204UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_204_uf_component_count() {
        let mut uf = super::Xj204UnionFind::xj_new();
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
    fn xj_204_uf_component_size() {
        let mut uf = super::Xj204UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_204_uf_largest_component() {
        let mut uf = super::Xj204UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_204_uf_many_elements() {
        let mut uf = super::Xj204UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_204_uf_separate_components() {
        let mut uf = super::Xj204UnionFind::xj_new();
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
    fn xj_204_uf_path_compression() {
        let mut uf = super::Xj204UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_204_bt_insert_get() {
        let mut bt = super::Xj204BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_204_bt_contains_len() {
        let mut bt = super::Xj204BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_204_bt_replace() {
        let mut bt = super::Xj204BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_204_bt_remove() {
        let mut bt = super::Xj204BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_204_bt_keys_values() {
        let mut bt = super::Xj204BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_204_bt_range() {
        let mut bt = super::Xj204BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_204_bt_min_max() {
        let mut bt = super::Xj204BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_204_bt_many_inserts() {
        let mut bt = super::Xj204BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_204 segment tree tests ---

    #[test]
    fn xk_204_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk204SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_204_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk204SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_204_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk204SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_204_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk204SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_204_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk204SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_204_st_single_element() {
        let data = vec![42];
        let st = super::Xk204SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_204_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk204SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_204_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk204SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_204 disjoint intervals tests ---

    #[test]
    fn xk_204_di_add_and_count() {
        let mut di = super::Xk204DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_204_di_merge_overlap() {
        let mut di = super::Xk204DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_204_di_contains() {
        let mut di = super::Xk204DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_204_di_remove() {
        let mut di = super::Xk204DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_204_di_covered_length() {
        let mut di = super::Xk204DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_204_di_gaps() {
        let mut di = super::Xk204DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_204_di_merge_adjacent() {
        let mut di = super::Xk204DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_204_di_empty() {
        let di = super::Xk204DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_204_rope_new_empty() {
        let rope = super::Xl204Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_204_rope_from_str() {
        let rope = super::Xl204Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_204_rope_insert_at() {
        let mut rope = super::Xl204Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_204_rope_delete_range() {
        let mut rope = super::Xl204Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_204_rope_char_at() {
        let rope = super::Xl204Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_204_rope_split_concat() {
        let rope = super::Xl204Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_204_rope_line_count() {
        let rope = super::Xl204Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_204_rope_line_at() {
        let rope = super::Xl204Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_204_sa_build_and_search() {
        let sa = super::Xl204SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_204_sa_count() {
        let sa = super::Xl204SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_204_sa_longest_repeated() {
        let sa = super::Xl204SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_204_sa_all_positions() {
        let sa = super::Xl204SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_204_sa_len() {
        let sa = super::Xl204SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_204_sa_empty() {
        let sa = super::Xl204SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_204_rope_slice() {
        let rope = super::Xl204Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_204_sa_search_start() {
        let sa = super::Xl204SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_204_sparse_set_get() {
        let mut m = super::Xm204MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_204_sparse_row_col() {
        let mut m = super::Xm204MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_204_sparse_transpose() {
        let mut m = super::Xm204MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_204_sparse_multiply_vec() {
        let mut m = super::Xm204MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_204_sparse_nnz_density() {
        let mut m = super::Xm204MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_204_sparse_clear() {
        let mut m = super::Xm204MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_204_sparse_overwrite_zero() {
        let mut m = super::Xm204MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_204_tokenizer_basic() {
        let t = super::Xm204Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_204_tokenizer_count() {
        let t = super::Xm204Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_204_tokenizer_unique() {
        let t = super::Xm204Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_204_tokenizer_frequency() {
        let t = super::Xm204Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_204_tokenizer_delimiter() {
        let t = super::Xm204Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_204_tokenizer_whitespace() {
        let t = super::Xm204Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_204_tokenizer_empty() {
        let t = super::Xm204Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 204 ----

    #[test]
    fn xn_204_fenwick_prefix_sum() {
        let mut ft = super::Xn204Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_204_fenwick_range_sum() {
        let mut ft = super::Xn204Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_204_fenwick_point_query() {
        let mut ft = super::Xn204Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_204_fenwick_len() {
        let ft = super::Xn204Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_204_fenwick_multiple_updates() {
        let mut ft = super::Xn204Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_204_fenwick_single_element() {
        let mut ft = super::Xn204Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_204_fenwick_find_kth() {
        let mut ft = super::Xn204Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_204_fenwick_negative_delta() {
        let mut ft = super::Xn204Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 204 ----

    #[test]
    fn xn_204_avl_insert_get() {
        let mut m = super::Xn204AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_204_avl_remove() {
        let mut m = super::Xn204AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_204_avl_in_order() {
        let mut m = super::Xn204AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_204_avl_min_max() {
        let mut m = super::Xn204AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_204_avl_floor_ceiling() {
        let mut m = super::Xn204AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_204_avl_height_balanced() {
        let mut m = super::Xn204AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_204_avl_overwrite() {
        let mut m = super::Xn204AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_204_avl_empty() {
        let m: super::Xn204AVL<i32, i32> = super::Xn204AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo204RedBlack tests ---

    #[test]
    fn xo_204_rb_insert_and_get() {
        let mut tree = super::Xo204RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_204_rb_len_and_empty() {
        let mut tree = super::Xo204RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_204_rb_min_max() {
        let mut tree = super::Xo204RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_204_rb_contains() {
        let mut tree = super::Xo204RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_204_rb_remove() {
        let mut tree = super::Xo204RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_204_rb_in_order() {
        let mut tree = super::Xo204RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_204_rb_black_height() {
        let mut tree = super::Xo204RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_204_rb_overwrite() {
        let mut tree = super::Xo204RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo204ConsistentHash tests ---

    #[test]
    fn xo_204_ch_add_and_count() {
        let mut ring = super::Xo204ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_204_ch_remove_node() {
        let mut ring = super::Xo204ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_204_ch_get_node() {
        let mut ring = super::Xo204ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_204_ch_empty_ring() {
        let ring = super::Xo204ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_204_ch_distribution() {
        let mut ring = super::Xo204ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_204_ch_rebalance() {
        let mut ring = super::Xo204ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_204_ch_virtual_nodes() {
        let mut ring = super::Xo204ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_204_ch_consistent_lookup() {
        let mut ring = super::Xo204ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_204_splay_insert_get() {
        let mut t = super::Xp204SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_204_splay_remove() {
        let mut t = super::Xp204SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_204_splay_count_increases() {
        let mut t = super::Xp204SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_204_splay_depth() {
        let mut t = super::Xp204SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_204_splay_len_empty() {
        let t = super::Xp204SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_204_splay_min_max() {
        let mut t = super::Xp204SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_204_splay_overwrite() {
        let mut t = super::Xp204SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_204_splay_remove_missing() {
        let mut t = super::Xp204SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_204 treap tests ----
    #[test]
    fn xq_204_treap_empty() {
        let t = super::Xq204Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_204_treap_insert_get() {
        let mut t = super::Xq204Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_204_treap_overwrite() {
        let mut t = super::Xq204Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_204_treap_remove() {
        let mut t = super::Xq204Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_204_treap_min_max() {
        let mut t = super::Xq204Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_204_treap_rank() {
        let mut t = super::Xq204Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_204_treap_kth() {
        let mut t = super::Xq204Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_204_treap_in_order() {
        let mut t = super::Xq204Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_204 VEB tree tests ----
    #[test]
    fn xq_204_veb_empty() {
        let v = super::Xq204VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_204_veb_insert_contains() {
        let mut v = super::Xq204VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_204_veb_min_max() {
        let mut v = super::Xq204VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_204_veb_delete() {
        let mut v = super::Xq204VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_204_veb_successor() {
        let mut v = super::Xq204VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_204_veb_predecessor() {
        let mut v = super::Xq204VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_204_veb_count() {
        let mut v = super::Xq204VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_204_veb_duplicate_insert() {
        let mut v = super::Xq204VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }

}
