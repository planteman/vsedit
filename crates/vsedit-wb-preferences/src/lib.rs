//! Settings editor service.

use std::collections::HashMap;
use std::fmt;

/// The type of a preference value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferenceType {
    String,
    Number,
    Boolean,
    Array,
    Object,
    Enum,
}

impl fmt::Display for PreferenceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String => write!(f, "string"),
            Self::Number => write!(f, "number"),
            Self::Boolean => write!(f, "boolean"),
            Self::Array => write!(f, "array"),
            Self::Object => write!(f, "object"),
            Self::Enum => write!(f, "enum"),
        }
    }
}

/// Scope in which a preference applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferenceScope {
    Application,
    Machine,
    Window,
    Resource,
    Language,
}

impl fmt::Display for PreferenceScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application => write!(f, "application"),
            Self::Machine => write!(f, "machine"),
            Self::Window => write!(f, "window"),
            Self::Resource => write!(f, "resource"),
            Self::Language => write!(f, "language"),
        }
    }
}

/// Errors returned by preference operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreferenceError {
    KeyNotFound(String),
    TypeMismatch(String),
    InvalidValue(String),
}

impl fmt::Display for PreferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeyNotFound(k) => write!(f, "preference key not found: {k}"),
            Self::TypeMismatch(msg) => write!(f, "type mismatch: {msg}"),
            Self::InvalidValue(msg) => write!(f, "invalid value: {msg}"),
        }
    }
}

/// Describes a registered preference.
#[derive(Debug, Clone)]
pub struct PreferenceDescriptor {
    pub key: String,
    pub preference_type: PreferenceType,
    pub default_value: String,
    pub description: String,
    pub enum_values: Vec<String>,
    pub scope: PreferenceScope,
}

impl PreferenceDescriptor {
    /// Builder method to set the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

/// Service for managing user/workspace preferences.
pub struct PreferencesService {
    descriptors: Vec<PreferenceDescriptor>,
    overrides: HashMap<String, String>,
}

impl PreferencesService {
    pub fn new() -> Self {
        Self {
            descriptors: Vec::new(),
            overrides: HashMap::new(),
        }
    }

    pub fn register(&mut self, descriptor: PreferenceDescriptor) {
        self.descriptors.push(descriptor);
    }

    pub fn set_override(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.overrides.insert(key.into(), value.into());
    }

    /// Returns the override value if set, otherwise the default from the descriptor.
    /// Panics if the key is not registered.
    pub fn get_value(&self, key: &str) -> &str {
        if let Some(v) = self.overrides.get(key) {
            return v.as_str();
        }
        self.descriptors
            .iter()
            .find(|d| d.key == key)
            .map(|d| d.default_value.as_str())
            .expect("preference key not registered")
    }

    pub fn get_descriptors_by_scope(&self, scope: PreferenceScope) -> Vec<&PreferenceDescriptor> {
        self.descriptors
            .iter()
            .filter(|d| d.scope == scope)
            .collect()
    }

    pub fn has_override(&self, key: &str) -> bool {
        self.overrides.contains_key(key)
    }

    pub fn reset(&mut self, key: &str) -> bool {
        self.overrides.remove(key).is_some()
    }

    /// Returns the value for a key, or a `PreferenceError` if not registered.
    pub fn try_get_value(&self, key: &str) -> Result<&str, PreferenceError> {
        if let Some(v) = self.overrides.get(key) {
            return Ok(v.as_str());
        }
        self.descriptors
            .iter()
            .find(|d| d.key == key)
            .map(|d| d.default_value.as_str())
            .ok_or_else(|| PreferenceError::KeyNotFound(key.to_string()))
    }

    /// Returns the keys of all currently overridden preferences.
    pub fn list_overrides(&self) -> Vec<&str> {
        self.overrides.keys().map(|k| k.as_str()).collect()
    }

    /// Removes all overrides.
    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }

    /// Returns the number of registered descriptors.
    pub fn descriptor_count(&self) -> usize {
        self.descriptors.len()
    }

    /// Search descriptors whose key contains the given substring.
    pub fn search(&self, pattern: &str) -> Vec<&PreferenceDescriptor> {
        self.descriptors
            .iter()
            .filter(|d| d.key.contains(pattern))
            .collect()
    }

    /// Get a descriptor by its exact key.
    pub fn get_descriptor(&self, key: &str) -> Option<&PreferenceDescriptor> {
        self.descriptors.iter().find(|d| d.key == key)
    }

    /// Check whether a key is registered.
    pub fn has_key(&self, key: &str) -> bool {
        self.descriptors.iter().any(|d| d.key == key)
    }
}

impl Default for PreferencesService {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a change to a preference value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceChangeEvent {
    pub key: String,
    pub old_value: Option<String>,
    pub new_value: String,
}

impl fmt::Display for PreferenceChangeEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let old = self
            .old_value
            .as_deref()
            .unwrap_or("<unset>");
        write!(f, "{}: {} -> {}", self.key, old, self.new_value)
    }
}

/// Rules that can be used to validate a preference value.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationRule {
    /// The value must not be empty.
    NonEmpty,
    /// The value must have at least this many characters.
    MinLength(usize),
    /// The value must have at most this many characters.
    MaxLength(usize),
    /// The value must be one of the listed options.
    OneOf(Vec<String>),
    /// The value, parsed as f64, must fall within the given inclusive range.
    NumericRange(f64, f64),
}

impl ValidationRule {
    /// Validate `value` against this rule, returning `Ok(())` on success or
    /// an error message describing the violation.
    pub fn validate(&self, value: &str) -> Result<(), String> {
        match self {
            Self::NonEmpty => {
                if value.is_empty() {
                    Err("value must not be empty".to_string())
                } else {
                    Ok(())
                }
            }
            Self::MinLength(min) => {
                if value.len() < *min {
                    Err(format!(
                        "value length {} is less than minimum {min}",
                        value.len()
                    ))
                } else {
                    Ok(())
                }
            }
            Self::MaxLength(max) => {
                if value.len() > *max {
                    Err(format!(
                        "value length {} exceeds maximum {max}",
                        value.len()
                    ))
                } else {
                    Ok(())
                }
            }
            Self::OneOf(options) => {
                if options.iter().any(|o| o == value) {
                    Ok(())
                } else {
                    Err(format!(
                        "value {value:?} is not one of {:?}",
                        options
                    ))
                }
            }
            Self::NumericRange(lo, hi) => {
                let n: f64 = value
                    .parse()
                    .map_err(|_| format!("{value:?} is not a valid number"))?;
                if n < *lo || n > *hi {
                    Err(format!("{n} is outside range [{lo}, {hi}]"))
                } else {
                    Ok(())
                }
            }
        }
    }
}

impl fmt::Display for ValidationRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonEmpty => write!(f, "non-empty"),
            Self::MinLength(n) => write!(f, "min length {n}"),
            Self::MaxLength(n) => write!(f, "max length {n}"),
            Self::OneOf(opts) => write!(f, "one of {:?}", opts),
            Self::NumericRange(lo, hi) => write!(f, "numeric range [{lo}, {hi}]"),
        }
    }
}

impl PreferencesService {
    /// Set an override only if `value` passes all supplied validation rules.
    pub fn set_override_checked(
        &mut self,
        key: &str,
        value: &str,
        rules: &[ValidationRule],
    ) -> Result<(), PreferenceError> {
        for rule in rules {
            rule.validate(value)
                .map_err(PreferenceError::InvalidValue)?;
        }
        self.set_override(key, value);
        Ok(())
    }

    /// Returns `(key, effective_value)` for every registered descriptor.
    pub fn get_all_values(&self) -> Vec<(&str, &str)> {
        self.descriptors
            .iter()
            .map(|d| {
                let value = self
                    .overrides
                    .get(&d.key)
                    .map(|v| v.as_str())
                    .unwrap_or(d.default_value.as_str());
                (d.key.as_str(), value)
            })
            .collect()
    }

    /// Returns `(key, value)` pairs for all current overrides.
    pub fn export_overrides(&self) -> Vec<(&str, &str)> {
        self.overrides
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }

    /// Bulk-set overrides from a slice of `(key, value)` pairs.
    pub fn import_overrides(&mut self, overrides: &[(&str, &str)]) {
        for (key, value) in overrides {
            self.overrides
                .insert((*key).to_string(), (*value).to_string());
        }
    }

    /// Returns descriptors whose `preference_type` matches the given type.
    pub fn descriptors_of_type(&self, ptype: PreferenceType) -> Vec<&PreferenceDescriptor> {
        self.descriptors
            .iter()
            .filter(|d| d.preference_type == ptype)
            .collect()
    }
}

/// Accumulated statistics for wb-preferences operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbPreferencesStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbPreferencesStats {
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
    pub fn merge(&mut self, other: &WbPreferencesStats) {
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

impl Default for WbPreferencesStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbPreferencesStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbPreferencesStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-preferences.
#[derive(Debug, Clone)]
pub struct WbPreferencesValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbPreferencesValidator {
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

impl Default for WbPreferencesValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// preference_reset_to_default — individual setting reset
// ---------------------------------------------------------------------------

/// Result of resetting a preference to its default value.
#[derive(Debug, Clone)]
pub struct PreferenceResetResult {
    pub key: String,
    pub old_value: String,
    pub new_value: String,
    pub was_already_default: bool,
}

impl fmt::Display for PreferenceResetResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.was_already_default {
            write!(f, "'{}' already at default", self.key)
        } else {
            write!(f, "'{}': '{}' -> '{}'", self.key, self.old_value, self.new_value)
        }
    }
}

/// Reset a single preference to its default value.
/// Returns the old and new values.
pub fn preference_reset_to_default(
    service: &mut PreferencesService,
    key: &str,
) -> Result<PreferenceResetResult, PreferenceError> {
    let descriptor = service
        .get_descriptor(key)
        .ok_or_else(|| PreferenceError::KeyNotFound(key.to_string()))?;
    let default_value = descriptor.default_value.clone();
    let old_value = service.get_value(key).to_string();
    let was_already_default = old_value == default_value;
    if !was_already_default {
        service.reset(key);
    }
    Ok(PreferenceResetResult {
        key: key.to_string(),
        old_value,
        new_value: default_value,
        was_already_default,
    })
}

/// Reset all preferences in a given scope to their defaults.
pub fn preference_reset_scope(
    service: &mut PreferencesService,
    scope: PreferenceScope,
) -> Vec<PreferenceResetResult> {
    let keys: Vec<String> = service
        .get_descriptors_by_scope(scope)
        .iter()
        .map(|d| d.key.clone())
        .collect();
    keys.iter()
        .filter_map(|k| preference_reset_to_default(service, k).ok())
        .collect()
}

/// Count how many preferences differ from their default values.
pub fn preference_count_modified(service: &PreferencesService) -> usize {
    service
        .list_overrides()
        .len()
}

// ---------------------------------------------------------------------------
// Additional helpers
// ---------------------------------------------------------------------------

impl PreferencesService {
    /// Returns the number of active overrides.
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }

    /// Returns the keys of all registered preference descriptors.
    pub fn keys(&self) -> Vec<&str> {
        self.descriptors.iter().map(|d| d.key.as_str()).collect()
    }

    /// Returns a human-readable summary of the service state.
    pub fn summary(&self) -> String {
        let total = self.descriptors.len();
        let overridden = self.overrides.len();
        let scopes: Vec<PreferenceScope> = vec![
            PreferenceScope::Application,
            PreferenceScope::Machine,
            PreferenceScope::Window,
            PreferenceScope::Resource,
            PreferenceScope::Language,
        ];
        let mut scope_counts = Vec::new();
        for scope in &scopes {
            let count = self.descriptors.iter().filter(|d| d.scope == *scope).count();
            if count > 0 {
                scope_counts.push(format!("{scope}: {count}"));
            }
        }
        format!(
            "{total} preferences ({overridden} overridden) [{}]",
            scope_counts.join(", ")
        )
    }
}

impl fmt::Display for PreferencesService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PreferencesService({} descriptors, {} overrides)",
            self.descriptors.len(),
            self.overrides.len(),
        )
    }
}

impl PreferenceDescriptor {
    /// Returns `true` if this preference is of type `Boolean`.
    pub fn is_boolean(&self) -> bool {
        self.preference_type == PreferenceType::Boolean
    }

    /// Returns `true` if this preference has enum values defined.
    pub fn is_enum(&self) -> bool {
        !self.enum_values.is_empty()
    }
}

impl PreferenceScope {
    /// Returns `true` for user-level scopes (Application or Window).
    pub fn is_user_scope(&self) -> bool {
        matches!(self, Self::Application | Self::Window)
    }
}

impl PreferenceType {
    /// Returns `true` if this type is numeric (`Number`).
    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Number)
    }
}

impl ValidationRule {
    /// Returns a human-readable description of this rule.
    pub fn description(&self) -> &str {
        match self {
            Self::NonEmpty => "value must not be empty",
            Self::MinLength(_) => "value must meet minimum length",
            Self::MaxLength(_) => "value must not exceed maximum length",
            Self::OneOf(_) => "value must be one of the allowed options",
            Self::NumericRange(_, _) => "value must be within the numeric range",
        }
    }
}

// ---------------------------------------------------------------------------
// Preference schema validation
// ---------------------------------------------------------------------------

/// Validate a value against its preference descriptor's type.
///
/// Returns `Ok(())` if the value is compatible with the descriptor's type,
/// or an error describing the mismatch.
pub fn validate_value_type(descriptor: &PreferenceDescriptor, value: &str) -> Result<(), PreferenceError> {
    match descriptor.preference_type {
        PreferenceType::Boolean => {
            if value != "true" && value != "false" {
                return Err(PreferenceError::TypeMismatch(format!(
                    "expected boolean, got '{value}'"
                )));
            }
        }
        PreferenceType::Number => {
            if value.parse::<f64>().is_err() {
                return Err(PreferenceError::TypeMismatch(format!(
                    "expected number, got '{value}'"
                )));
            }
        }
        PreferenceType::Enum => {
            if !descriptor.enum_values.iter().any(|v| v == value) {
                return Err(PreferenceError::InvalidValue(format!(
                    "'{value}' is not one of {:?}",
                    descriptor.enum_values
                )));
            }
        }
        _ => {}
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Preference migration
// ---------------------------------------------------------------------------

/// A migration rule that renames a preference key.
#[derive(Debug, Clone)]
pub struct PreferenceMigration {
    pub old_key: String,
    pub new_key: String,
    /// Optional value transformer. If `None`, the value is kept as-is.
    pub transform: Option<fn(&str) -> String>,
}

impl PreferenceMigration {
    /// Create a simple key rename migration.
    pub fn rename(old_key: impl Into<String>, new_key: impl Into<String>) -> Self {
        Self {
            old_key: old_key.into(),
            new_key: new_key.into(),
            transform: None,
        }
    }

    /// Create a migration that also transforms the value.
    pub fn rename_and_transform(
        old_key: impl Into<String>,
        new_key: impl Into<String>,
        transform: fn(&str) -> String,
    ) -> Self {
        Self {
            old_key: old_key.into(),
            new_key: new_key.into(),
            transform: Some(transform),
        }
    }
}

/// Apply a list of migrations to a preferences service.
///
/// For each migration, if the old key has an override, it is moved to the new
/// key (with optional value transformation). Returns the number of migrations applied.
pub fn apply_migrations(service: &mut PreferencesService, migrations: &[PreferenceMigration]) -> usize {
    let mut applied = 0;
    for migration in migrations {
        if service.has_override(&migration.old_key) {
            let old_value = service.try_get_value(&migration.old_key)
                .unwrap_or_default()
                .to_string();
            let new_value = match &migration.transform {
                Some(f) => f(&old_value),
                None => old_value,
            };
            service.reset(&migration.old_key);
            service.set_override(&migration.new_key, new_value);
            applied += 1;
        }
    }
    applied
}

// ---------------------------------------------------------------------------
// Preference diff computation
// ---------------------------------------------------------------------------

/// Represents a difference between two preference snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreferenceDiff {
    /// A key was added (not in `before`, present in `after`).
    Added { key: String, value: String },
    /// A key was removed (present in `before`, not in `after`).
    Removed { key: String, value: String },
    /// A key's value changed.
    Changed { key: String, old_value: String, new_value: String },
}

/// Compute the diff between two override snapshots.
pub fn compute_preference_diff(
    before: &HashMap<String, String>,
    after: &HashMap<String, String>,
) -> Vec<PreferenceDiff> {
    let mut diffs = Vec::new();
    for (key, val) in after {
        match before.get(key) {
            Some(old_val) if old_val != val => {
                diffs.push(PreferenceDiff::Changed {
                    key: key.clone(),
                    old_value: old_val.clone(),
                    new_value: val.clone(),
                });
            }
            None => {
                diffs.push(PreferenceDiff::Added {
                    key: key.clone(),
                    value: val.clone(),
                });
            }
            _ => {}
        }
    }
    for (key, val) in before {
        if !after.contains_key(key) {
            diffs.push(PreferenceDiff::Removed {
                key: key.clone(),
                value: val.clone(),
            });
        }
    }
    diffs.sort_by(|a, b| {
        let key_a = match a {
            PreferenceDiff::Added { key, .. }
            | PreferenceDiff::Removed { key, .. }
            | PreferenceDiff::Changed { key, .. } => key,
        };
        let key_b = match b {
            PreferenceDiff::Added { key, .. }
            | PreferenceDiff::Removed { key, .. }
            | PreferenceDiff::Changed { key, .. } => key,
        };
        key_a.cmp(key_b)
    });
    diffs
}

// ---------------------------------------------------------------------------
// Preference inheritance (layered scopes)
// ---------------------------------------------------------------------------

/// Resolves a preference value across layered scopes.
///
/// Values in layers with higher indices override those in lower indices.
/// Each layer is a `HashMap<String, String>` of overrides.
pub fn resolve_layered(key: &str, layers: &[&HashMap<String, String>], default: &str) -> String {
    for layer in layers.iter().rev() {
        if let Some(val) = layer.get(key) {
            return val.clone();
        }
    }
    default.to_string()
}

// ---------------------------------------------------------------------------
// Extended PreferencesService methods
// ---------------------------------------------------------------------------

impl PreferencesService {
    /// Validate all current overrides against the supplied rules map.
    ///
    /// Returns a list of `(key, error_message)` for every override that
    /// fails validation.  Keys not present in the rules map are skipped.
    pub fn validate(&self, rules: &HashMap<String, Vec<ValidationRule>>) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        for (key, value) in &self.overrides {
            if let Some(key_rules) = rules.get(key) {
                for rule in key_rules {
                    if let Err(msg) = rule.validate(value) {
                        errors.push((key.clone(), msg));
                    }
                }
            }
        }
        errors
    }

    /// Bulk-set multiple overrides at once.
    ///
    /// Returns the number of overrides actually written (including updates).
    pub fn bulk_set(&mut self, pairs: &[(&str, &str)]) -> usize {
        let mut count = 0;
        for (key, value) in pairs {
            self.overrides.insert((*key).to_string(), (*value).to_string());
            count += 1;
        }
        count
    }

    /// Compute the diff between the current overrides and a previous snapshot.
    ///
    /// Returns `(added, changed, removed)` key lists.
    pub fn diff(
        &self,
        previous: &HashMap<String, String>,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        let mut added = Vec::new();
        let mut changed = Vec::new();
        let mut removed = Vec::new();

        for (key, val) in &self.overrides {
            match previous.get(key) {
                Some(old_val) if old_val != val => changed.push(key.clone()),
                None => added.push(key.clone()),
                _ => {}
            }
        }
        for key in previous.keys() {
            if !self.overrides.contains_key(key) {
                removed.push(key.clone());
            }
        }
        (added, changed, removed)
    }

    /// Snapshot the current overrides as an owned map.
    pub fn snapshot_overrides(&self) -> HashMap<String, String> {
        self.overrides.clone()
    }
}

impl PreferenceDescriptor {
    /// Returns `true` if this descriptor matches the given scope.
    pub fn matches_scope(&self, scope: PreferenceScope) -> bool {
        self.scope == scope
    }

    /// Returns `true` if the key starts with the given prefix.
    pub fn has_prefix(&self, prefix: &str) -> bool {
        self.key.starts_with(prefix)
    }
}

impl PreferenceScope {
    /// Returns `true` for scopes that are file-specific.
    pub fn is_resource_level(&self) -> bool {
        matches!(self, PreferenceScope::Resource | PreferenceScope::Language)
    }

    /// Returns the numeric priority of this scope (higher = more specific).
    pub fn priority(&self) -> u8 {
        match self {
            Self::Application => 0,
            Self::Machine => 1,
            Self::Window => 2,
            Self::Resource => 3,
            Self::Language => 4,
        }
    }

    /// Returns all scopes in priority order (least to most specific).
    pub fn all_ordered() -> &'static [PreferenceScope] {
        &[
            PreferenceScope::Application,
            PreferenceScope::Machine,
            PreferenceScope::Window,
            PreferenceScope::Resource,
            PreferenceScope::Language,
        ]
    }
}

// ---------------------------------------------------------------------------
// Settings schema validation
// ---------------------------------------------------------------------------

/// Schema definition for a preference, used for rich validation.
#[derive(Debug, Clone)]
pub struct PreferenceSchema {
    pub key: String,
    pub preference_type: PreferenceType,
    pub required: bool,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub pattern: Option<String>,
    pub allowed_values: Vec<String>,
    pub deprecated: bool,
    pub deprecation_message: Option<String>,
}

impl PreferenceSchema {
    pub fn new(key: impl Into<String>, preference_type: PreferenceType) -> Self {
        Self {
            key: key.into(),
            preference_type,
            required: false,
            min_value: None,
            max_value: None,
            pattern: None,
            allowed_values: Vec::new(),
            deprecated: false,
            deprecation_message: None,
        }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min_value = Some(min);
        self.max_value = Some(max);
        self
    }

    pub fn allowed(mut self, values: Vec<String>) -> Self {
        self.allowed_values = values;
        self
    }

    pub fn deprecated_with(mut self, message: impl Into<String>) -> Self {
        self.deprecated = true;
        self.deprecation_message = Some(message.into());
        self
    }

    /// Validate a value against this schema.
    pub fn validate_value(&self, value: &str) -> Vec<String> {
        let mut errors = Vec::new();

        if self.required && value.is_empty() {
            errors.push(format!("{}: value is required", self.key));
        }

        if self.deprecated {
            errors.push(format!(
                "{}: deprecated – {}",
                self.key,
                self.deprecation_message.as_deref().unwrap_or("no longer supported")
            ));
        }

        match self.preference_type {
            PreferenceType::Number => {
                if let Ok(n) = value.parse::<f64>() {
                    if let Some(min) = self.min_value {
                        if n < min {
                            errors.push(format!("{}: value {} below minimum {}", self.key, n, min));
                        }
                    }
                    if let Some(max) = self.max_value {
                        if n > max {
                            errors.push(format!("{}: value {} above maximum {}", self.key, n, max));
                        }
                    }
                } else if !value.is_empty() {
                    errors.push(format!("{}: expected a number, got '{}'", self.key, value));
                }
            }
            PreferenceType::Boolean => {
                if !value.is_empty() && value != "true" && value != "false" {
                    errors.push(format!("{}: expected boolean, got '{}'", self.key, value));
                }
            }
            PreferenceType::Enum => {
                if !self.allowed_values.is_empty() && !self.allowed_values.iter().any(|v| v == value) {
                    errors.push(format!(
                        "{}: '{}' not in allowed values [{}]",
                        self.key,
                        value,
                        self.allowed_values.join(", ")
                    ));
                }
            }
            _ => {}
        }

        errors
    }
}

// ---------------------------------------------------------------------------
// Schema registry – validate entire service state
// ---------------------------------------------------------------------------

/// A registry that holds schemas and can validate a `PreferencesService` against them.
#[derive(Debug, Clone, Default)]
pub struct SchemaRegistry {
    schemas: Vec<PreferenceSchema>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self { schemas: Vec::new() }
    }

    pub fn register(&mut self, schema: PreferenceSchema) {
        self.schemas.push(schema);
    }

    pub fn len(&self) -> usize {
        self.schemas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }

    /// Validate all overrides in a service against registered schemas.
    pub fn validate_service(&self, service: &PreferencesService) -> Vec<String> {
        let mut all_errors = Vec::new();
        for schema in &self.schemas {
            if let Some(value) = service.overrides.get(&schema.key) {
                all_errors.extend(schema.validate_value(value));
            }
        }
        all_errors
    }

    /// Find deprecated settings that are currently overridden.
    pub fn deprecated_overrides(&self, service: &PreferencesService) -> Vec<String> {
        self.schemas
            .iter()
            .filter(|s| s.deprecated && service.has_override(&s.key))
            .map(|s| s.key.clone())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Type coercion helpers
// ---------------------------------------------------------------------------

/// Attempt to coerce a string value to a target `PreferenceType`.
///
/// Returns the coerced string representation or an error.
pub fn coerce_value(value: &str, target: PreferenceType) -> Result<String, PreferenceError> {
    match target {
        PreferenceType::Boolean => match value.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok("true".to_string()),
            "false" | "0" | "no" | "off" => Ok("false".to_string()),
            _ => Err(PreferenceError::TypeMismatch(format!(
                "cannot coerce '{}' to boolean",
                value
            ))),
        },
        PreferenceType::Number => {
            value
                .parse::<f64>()
                .map(|n| n.to_string())
                .map_err(|_| PreferenceError::TypeMismatch(format!(
                    "cannot coerce '{}' to number",
                    value
                )))
        }
        PreferenceType::String => Ok(value.to_string()),
        _ => Ok(value.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Multi-layer settings merge
// ---------------------------------------------------------------------------

/// Represents a named settings layer (e.g. "default", "user", "workspace", "folder").
#[derive(Debug, Clone)]
pub struct SettingsLayer {
    pub name: String,
    pub values: HashMap<String, String>,
}

impl SettingsLayer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            values: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.values.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }
}

/// Merge multiple settings layers in priority order (last wins).
///
/// Returns a flat map of key → resolved value, plus a parallel map of
/// key → originating layer name.
pub fn merge_layers(layers: &[&SettingsLayer]) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut merged: HashMap<String, String> = HashMap::new();
    let mut origins: HashMap<String, String> = HashMap::new();
    for layer in layers {
        for (k, v) in &layer.values {
            merged.insert(k.clone(), v.clone());
            origins.insert(k.clone(), layer.name.clone());
        }
    }
    (merged, origins)
}

/// Determine which keys in the effective merged result are overridden
/// (i.e. differ from the first layer that defines them).
pub fn detect_overridden_keys(layers: &[&SettingsLayer]) -> Vec<String> {
    let mut first_seen: HashMap<String, String> = HashMap::new();
    let mut effective: HashMap<String, String> = HashMap::new();

    for layer in layers {
        for (k, v) in &layer.values {
            first_seen.entry(k.clone()).or_insert_with(|| v.clone());
            effective.insert(k.clone(), v.clone());
        }
    }

    let mut overridden: Vec<String> = effective
        .iter()
        .filter(|(k, v)| first_seen.get(*k).map_or(false, |first| first != *v))
        .map(|(k, _)| k.clone())
        .collect();
    overridden.sort();
    overridden
}

// ---------------------------------------------------------------------------
// Settings grouping / categorisation
// ---------------------------------------------------------------------------

impl PreferencesService {
    /// Group all registered descriptors by their dotted key prefix.
    ///
    /// For a key like `"editor.fontSize"`, the group is `"editor"`.
    /// Keys without a dot are placed under `""`.
    pub fn group_by_prefix(&self) -> HashMap<String, Vec<&PreferenceDescriptor>> {
        let mut groups: HashMap<String, Vec<&PreferenceDescriptor>> = HashMap::new();
        for d in &self.descriptors {
            let prefix = d.key.split('.').next().unwrap_or("").to_string();
            groups.entry(prefix).or_default().push(d);
        }
        groups
    }

    /// Return all keys whose current effective value differs from the default.
    pub fn modified_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        for d in &self.descriptors {
            let effective = self.overrides.get(&d.key).map(|s| s.as_str()).unwrap_or(&d.default_value);
            if effective != d.default_value {
                keys.push(d.key.clone());
            }
        }
        keys.sort();
        keys
    }

    /// Collect descriptors that have a description containing a search term (case-insensitive).
    pub fn search_descriptions(&self, term: &str) -> Vec<&PreferenceDescriptor> {
        let lower = term.to_lowercase();
        self.descriptors
            .iter()
            .filter(|d| d.description.to_lowercase().contains(&lower))
            .collect()
    }

    /// Apply a function to every override value in-place.
    pub fn transform_overrides(&mut self, f: fn(&str) -> String) {
        let keys: Vec<String> = self.overrides.keys().cloned().collect();
        for key in keys {
            if let Some(val) = self.overrides.get(&key) {
                let new_val = f(val);
                self.overrides.insert(key, new_val);
            }
        }
    }

    /// Remove overrides for keys that are no longer registered (stale keys).
    pub fn prune_stale_overrides(&mut self) -> Vec<String> {
        let registered: std::collections::HashSet<&str> = self.descriptors.iter().map(|d| d.key.as_str()).collect();
        let stale: Vec<String> = self
            .overrides
            .keys()
            .filter(|k| !registered.contains(k.as_str()))
            .cloned()
            .collect();
        for k in &stale {
            self.overrides.remove(k);
        }
        stale
    }
}


// === Preferences Search Index ===

/// Preferences Search Index implementation.
#[derive(Debug, Clone)]
pub struct PreferencesSearchIndex {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: PreferencesSearchIndexStats,
}

/// Statistics for PreferencesSearchIndex.
#[derive(Debug, Clone, Default)]
pub struct PreferencesSearchIndexStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl PreferencesSearchIndexStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl PreferencesSearchIndex {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: PreferencesSearchIndexStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &PreferencesSearchIndexStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for PreferencesSearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

// === Preferences Modified Badge ===

/// Priority level for PreferencesModifiedBadge items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PreferencesModifiedBadgePriority {
    Low,
    Normal,
    High,
    Critical,
}

impl PreferencesModifiedBadgePriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for PreferencesModifiedBadgePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Preferences Modified Badge implementation.
#[derive(Debug, Clone)]
pub struct PreferencesModifiedBadge {
    items: Vec<PreferencesModifiedBadgeItem>,
    max_items: usize,
    default_priority: PreferencesModifiedBadgePriority,
}

/// A single item in PreferencesModifiedBadge.
#[derive(Debug, Clone)]
pub struct PreferencesModifiedBadgeItem {
    pub id: String,
    pub label: String,
    pub priority: PreferencesModifiedBadgePriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl PreferencesModifiedBadgeItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: PreferencesModifiedBadgePriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: PreferencesModifiedBadgePriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl PreferencesModifiedBadge {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: PreferencesModifiedBadgePriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: PreferencesModifiedBadgeItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<PreferencesModifiedBadgeItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&PreferencesModifiedBadgeItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: PreferencesModifiedBadgePriority) -> Vec<&PreferencesModifiedBadgeItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&PreferencesModifiedBadgeItem> {
        let mut sorted: Vec<&PreferencesModifiedBadgeItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&PreferencesModifiedBadgeItem> {
        let mut sorted: Vec<&PreferencesModifiedBadgeItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&PreferencesModifiedBadgeItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: PreferencesModifiedBadgePriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> PreferencesModifiedBadgePriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &PreferencesModifiedBadgeItem> {
        self.items.iter()
    }
}

impl Default for PreferencesModifiedBadge {
    fn default() -> Self {
        Self::new()
    }
}


/// Workbench preference configuration manager.
#[derive(Debug, Clone)]
pub struct WbPreferencesConfig {
    entries: Vec<WbPreferencesEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workbench preference entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WbPreferencesEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WbPreferencesEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl WbPreferencesConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WbPreferencesEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&WbPreferencesEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WbPreferencesEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WbPreferencesEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&WbPreferencesEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WbPreferencesEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<WbPreferencesEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for wb_preferences
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbPreferencesRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbPreferencesRingBuf {
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
pub struct XaWbPreferencesCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbPreferencesCounter {
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

impl Default for XaWbPreferencesCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 221
// ---------------------------------------------------------------------------

/// Generic object pool `Xc221Pool<T>`.
pub struct Xc221Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc221Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc221PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc221Pool<T> {
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
    pub fn stats(&self) -> Xc221PoolStats {
        Xc221PoolStats {
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

impl<T> Default for Xc221Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc221Scheduler`.
pub struct Xc221Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc221Scheduler {
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

impl Default for Xc221Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_221 hash for the given byte slice.
pub fn xc_221_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_221 convention.
pub fn xc_221_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_51 deepening: state machine + event bus ---

/// States for the Xd51 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd51State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd51State {
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
pub struct Xd51Transition {
    pub from: Xd51State,
    pub to: Xd51State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd51StateMachine {
    current: Xd51State,
    history: Vec<Xd51Transition>,
    step_counter: usize,
}

impl Xd51StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd51State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd51State {
        self.current
    }

    pub fn history(&self) -> &[Xd51Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd51State) -> Result<Xd51State, String> {
        let allowed = match (self.current, target) {
            (Xd51State::Idle, Xd51State::Running) => true,
            (Xd51State::Running, Xd51State::Paused) => true,
            (Xd51State::Running, Xd51State::Done) => true,
            (Xd51State::Paused, Xd51State::Running) => true,
            (Xd51State::Paused, Xd51State::Done) => true,
            (Xd51State::Done, Xd51State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_51: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd51Transition {
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
            "Xd51SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd51State> {
        let prefix = "Xd51SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd51State::Idle),
            "Running" => Some(Xd51State::Running),
            "Paused" => Some(Xd51State::Paused),
            "Done" => Some(Xd51State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd51State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd51 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd51Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd51Event {
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

type Xd51HandlerFn = Box<dyn Fn(&Xd51Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd51EventBus {
    handlers: Vec<(usize, Option<String>, Xd51HandlerFn)>,
    next_id: usize,
    published: Vec<Xd51Event>,
}

impl Xd51EventBus {
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
        F: Fn(&Xd51Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd51Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd51Event) {
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

    pub fn published_events(&self) -> &[Xd51Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(key: &str, default: &str, scope: PreferenceScope) -> PreferenceDescriptor {
        PreferenceDescriptor {
            key: key.to_string(),
            preference_type: PreferenceType::String,
            default_value: default.to_string(),
            description: String::new(),
            enum_values: vec![],
            scope,
        }
    }

    #[test]
    fn default_and_override() {
        let mut svc = PreferencesService::new();
        svc.register(desc("editor.fontSize", "14", PreferenceScope::Window));
        assert_eq!(svc.get_value("editor.fontSize"), "14");
        svc.set_override("editor.fontSize", "16");
        assert_eq!(svc.get_value("editor.fontSize"), "16");
        assert!(svc.has_override("editor.fontSize"));
    }

    #[test]
    fn reset_override() {
        let mut svc = PreferencesService::new();
        svc.register(desc("theme", "dark", PreferenceScope::Application));
        svc.set_override("theme", "light");
        assert!(svc.reset("theme"));
        assert!(!svc.has_override("theme"));
        assert_eq!(svc.get_value("theme"), "dark");
    }

    #[test]
    fn descriptors_by_scope() {
        let mut svc = PreferencesService::new();
        svc.register(desc("a", "1", PreferenceScope::Window));
        svc.register(desc("b", "2", PreferenceScope::Machine));
        svc.register(desc("c", "3", PreferenceScope::Window));
        assert_eq!(
            svc.get_descriptors_by_scope(PreferenceScope::Window).len(),
            2
        );
    }

    #[test]
    fn try_get_value_success() {
        let mut svc = PreferencesService::new();
        svc.register(desc("k", "v", PreferenceScope::Application));
        assert_eq!(svc.try_get_value("k"), Ok("v"));
    }

    #[test]
    fn try_get_value_not_found() {
        let svc = PreferencesService::new();
        assert_eq!(
            svc.try_get_value("missing"),
            Err(PreferenceError::KeyNotFound("missing".to_string()))
        );
    }

    #[test]
    fn try_get_value_returns_override() {
        let mut svc = PreferencesService::new();
        svc.register(desc("k", "default", PreferenceScope::Window));
        svc.set_override("k", "custom");
        assert_eq!(svc.try_get_value("k"), Ok("custom"));
    }

    #[test]
    fn list_overrides_empty() {
        let svc = PreferencesService::new();
        assert!(svc.list_overrides().is_empty());
    }

    #[test]
    fn list_overrides_returns_keys() {
        let mut svc = PreferencesService::new();
        svc.set_override("a", "1");
        svc.set_override("b", "2");
        let mut keys = svc.list_overrides();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn reset_all_clears_overrides() {
        let mut svc = PreferencesService::new();
        svc.register(desc("x", "10", PreferenceScope::Window));
        svc.set_override("x", "20");
        svc.set_override("y", "30");
        svc.reset_all();
        assert!(!svc.has_override("x"));
        assert!(!svc.has_override("y"));
        assert_eq!(svc.get_value("x"), "10");
    }

    #[test]
    fn descriptor_count() {
        let mut svc = PreferencesService::new();
        assert_eq!(svc.descriptor_count(), 0);
        svc.register(desc("a", "1", PreferenceScope::Window));
        svc.register(desc("b", "2", PreferenceScope::Machine));
        assert_eq!(svc.descriptor_count(), 2);
    }

    #[test]
    fn search_descriptors() {
        let mut svc = PreferencesService::new();
        svc.register(desc("editor.fontSize", "14", PreferenceScope::Window));
        svc.register(desc("editor.tabSize", "4", PreferenceScope::Window));
        svc.register(desc("terminal.font", "mono", PreferenceScope::Machine));
        let results = svc.search("editor");
        assert_eq!(results.len(), 2);
        let results = svc.search("terminal");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "terminal.font");
    }

    #[test]
    fn get_descriptor_found() {
        let mut svc = PreferencesService::new();
        svc.register(desc("theme", "dark", PreferenceScope::Application));
        let d = svc.get_descriptor("theme").unwrap();
        assert_eq!(d.default_value, "dark");
    }

    #[test]
    fn get_descriptor_not_found() {
        let svc = PreferencesService::new();
        assert!(svc.get_descriptor("nope").is_none());
    }

    #[test]
    fn has_key_check() {
        let mut svc = PreferencesService::new();
        assert!(!svc.has_key("k"));
        svc.register(desc("k", "v", PreferenceScope::Window));
        assert!(svc.has_key("k"));
    }

    #[test]
    fn with_description_builder() {
        let d = desc("k", "v", PreferenceScope::Window)
            .with_description("A test preference");
        assert_eq!(d.description, "A test preference");
    }

    #[test]
    fn preference_type_display() {
        assert_eq!(PreferenceType::String.to_string(), "string");
        assert_eq!(PreferenceType::Number.to_string(), "number");
        assert_eq!(PreferenceType::Boolean.to_string(), "boolean");
        assert_eq!(PreferenceType::Enum.to_string(), "enum");
    }

    #[test]
    fn preference_scope_display() {
        assert_eq!(PreferenceScope::Application.to_string(), "application");
        assert_eq!(PreferenceScope::Machine.to_string(), "machine");
        assert_eq!(PreferenceScope::Window.to_string(), "window");
        assert_eq!(PreferenceScope::Resource.to_string(), "resource");
        assert_eq!(PreferenceScope::Language.to_string(), "language");
    }

    #[test]
    fn preference_error_display() {
        let e = PreferenceError::KeyNotFound("x".into());
        assert_eq!(e.to_string(), "preference key not found: x");
        let e = PreferenceError::TypeMismatch("expected number".into());
        assert_eq!(e.to_string(), "type mismatch: expected number");
        let e = PreferenceError::InvalidValue("out of range".into());
        assert_eq!(e.to_string(), "invalid value: out of range");
    }

    #[test]
    fn test_preference_change_event_display_with_old() {
        let evt = PreferenceChangeEvent {
            key: "editor.fontSize".to_string(),
            old_value: Some("14".to_string()),
            new_value: "16".to_string(),
        };
        assert_eq!(evt.to_string(), "editor.fontSize: 14 -> 16");
    }

    #[test]
    fn test_preference_change_event_display_without_old() {
        let evt = PreferenceChangeEvent {
            key: "theme".to_string(),
            old_value: None,
            new_value: "dark".to_string(),
        };
        assert_eq!(evt.to_string(), "theme: <unset> -> dark");
    }

    #[test]
    fn test_validation_rule_non_empty() {
        let rule = ValidationRule::NonEmpty;
        assert!(rule.validate("hello").is_ok());
        assert!(rule.validate("").is_err());
    }

    #[test]
    fn test_validation_rule_min_length() {
        let rule = ValidationRule::MinLength(3);
        assert!(rule.validate("abc").is_ok());
        assert!(rule.validate("abcd").is_ok());
        assert!(rule.validate("ab").is_err());
    }

    #[test]
    fn test_validation_rule_max_length() {
        let rule = ValidationRule::MaxLength(5);
        assert!(rule.validate("hello").is_ok());
        assert!(rule.validate("hi").is_ok());
        assert!(rule.validate("toolong").is_err());
    }

    #[test]
    fn test_validation_rule_one_of() {
        let rule = ValidationRule::OneOf(vec![
            "dark".to_string(),
            "light".to_string(),
            "auto".to_string(),
        ]);
        assert!(rule.validate("dark").is_ok());
        assert!(rule.validate("light").is_ok());
        assert!(rule.validate("blue").is_err());
    }

    #[test]
    fn test_validation_rule_numeric_range() {
        let rule = ValidationRule::NumericRange(1.0, 100.0);
        assert!(rule.validate("50").is_ok());
        assert!(rule.validate("1").is_ok());
        assert!(rule.validate("100").is_ok());
        assert!(rule.validate("0").is_err());
        assert!(rule.validate("101").is_err());
        assert!(rule.validate("not_a_number").is_err());
    }

    #[test]
    fn test_validation_rule_display() {
        assert_eq!(ValidationRule::NonEmpty.to_string(), "non-empty");
        assert_eq!(ValidationRule::MinLength(3).to_string(), "min length 3");
        assert_eq!(ValidationRule::MaxLength(10).to_string(), "max length 10");
        assert_eq!(
            ValidationRule::OneOf(vec!["a".into(), "b".into()]).to_string(),
            "one of [\"a\", \"b\"]"
        );
        assert_eq!(
            ValidationRule::NumericRange(0.0, 99.0).to_string(),
            "numeric range [0, 99]"
        );
    }

    #[test]
    fn test_set_override_checked_valid() {
        let mut svc = PreferencesService::new();
        svc.register(desc("size", "14", PreferenceScope::Window));
        let rules = [
            ValidationRule::NonEmpty,
            ValidationRule::NumericRange(8.0, 72.0),
        ];
        assert!(svc.set_override_checked("size", "16", &rules).is_ok());
        assert_eq!(svc.get_value("size"), "16");
    }

    #[test]
    fn test_set_override_checked_invalid() {
        let mut svc = PreferencesService::new();
        svc.register(desc("size", "14", PreferenceScope::Window));
        let rules = [ValidationRule::NumericRange(8.0, 72.0)];
        let result = svc.set_override_checked("size", "200", &rules);
        assert!(result.is_err());
        // Value should NOT have changed.
        assert_eq!(svc.get_value("size"), "14");
    }

    #[test]
    fn test_get_all_values() {
        let mut svc = PreferencesService::new();
        svc.register(desc("a", "1", PreferenceScope::Window));
        svc.register(desc("b", "2", PreferenceScope::Machine));
        svc.set_override("b", "22");
        let all = svc.get_all_values();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&("a", "1")));
        assert!(all.contains(&("b", "22")));
    }

    #[test]
    fn test_export_and_import_overrides() {
        let mut svc = PreferencesService::new();
        svc.register(desc("x", "10", PreferenceScope::Window));
        svc.register(desc("y", "20", PreferenceScope::Window));
        svc.set_override("x", "100");
        svc.set_override("y", "200");

        let exported = svc.export_overrides();
        assert_eq!(exported.len(), 2);

        let mut svc2 = PreferencesService::new();
        svc2.register(desc("x", "10", PreferenceScope::Window));
        svc2.register(desc("y", "20", PreferenceScope::Window));
        svc2.import_overrides(&exported);
        assert_eq!(svc2.get_value("x"), "100");
        assert_eq!(svc2.get_value("y"), "200");
    }

    #[test]
    fn test_descriptors_of_type() {
        let mut svc = PreferencesService::new();
        svc.register(PreferenceDescriptor {
            key: "a".to_string(),
            preference_type: PreferenceType::String,
            default_value: "hello".to_string(),
            description: String::new(),
            enum_values: vec![],
            scope: PreferenceScope::Window,
        });
        svc.register(PreferenceDescriptor {
            key: "b".to_string(),
            preference_type: PreferenceType::Number,
            default_value: "42".to_string(),
            description: String::new(),
            enum_values: vec![],
            scope: PreferenceScope::Window,
        });
        svc.register(PreferenceDescriptor {
            key: "c".to_string(),
            preference_type: PreferenceType::String,
            default_value: "world".to_string(),
            description: String::new(),
            enum_values: vec![],
            scope: PreferenceScope::Machine,
        });
        let strings = svc.descriptors_of_type(PreferenceType::String);
        assert_eq!(strings.len(), 2);
        let numbers = svc.descriptors_of_type(PreferenceType::Number);
        assert_eq!(numbers.len(), 1);
        assert_eq!(numbers[0].key, "b");
        let booleans = svc.descriptors_of_type(PreferenceType::Boolean);
        assert!(booleans.is_empty());
    }

    #[test]
    fn wb_preferences_stats_new_defaults() {
        let stats = WbPreferencesStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_preferences_stats_record_success() {
        let mut stats = WbPreferencesStats::new();
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
    fn wb_preferences_stats_record_failure() {
        let mut stats = WbPreferencesStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_preferences_stats_reset() {
        let mut stats = WbPreferencesStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_preferences_stats_merge() {
        let mut a = WbPreferencesStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbPreferencesStats::new();
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
    fn wb_preferences_stats_display() {
        let mut stats = WbPreferencesStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_preferences_stats_default() {
        let stats = WbPreferencesStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_preferences_validator_accepts_valid_name() {
        let v = WbPreferencesValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_preferences_validator_rejects_empty() {
        let v = WbPreferencesValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_preferences_validator_rejects_too_long() {
        let v = WbPreferencesValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_preferences_validator_forbidden_prefix() {
        let v = WbPreferencesValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_preferences_validator_allowed_chars() {
        let v = WbPreferencesValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_preferences_validator_range() {
        let v = WbPreferencesValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_preferences_sanitize_removes_control() {
        let result = WbPreferencesValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_preferences_truncate_short_string() {
        assert_eq!(WbPreferencesValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_preferences_truncate_long_string() {
        let result = WbPreferencesValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_preferences_is_ascii_printable() {
        assert!(WbPreferencesValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbPreferencesValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- preference_reset_to_default tests ----------------------------------

    fn make_pref_service() -> PreferencesService {
        let mut svc = PreferencesService::new();
        svc.register(PreferenceDescriptor {
            key: "editor.fontSize".into(),
            preference_type: PreferenceType::Number,
            default_value: "14".into(),
            description: String::new(),
            enum_values: vec![],
            scope: PreferenceScope::Window,
        });
        svc.set_override("editor.fontSize", "16");
        svc.register(PreferenceDescriptor {
            key: "editor.tabSize".into(),
            preference_type: PreferenceType::Number,
            default_value: "4".into(),
            description: String::new(),
            enum_values: vec![],
            scope: PreferenceScope::Window,
        });
        svc
    }

    #[test]
    fn reset_modified_preference() {
        let mut svc = make_pref_service();
        let result = preference_reset_to_default(&mut svc, "editor.fontSize").unwrap();
        assert!(!result.was_already_default);
        assert_eq!(result.old_value, "16");
        assert_eq!(result.new_value, "14");
    }

    #[test]
    fn reset_already_default() {
        let mut svc = make_pref_service();
        let result = preference_reset_to_default(&mut svc, "editor.tabSize").unwrap();
        assert!(result.was_already_default);
    }

    #[test]
    fn reset_not_found() {
        let mut svc = make_pref_service();
        let result = preference_reset_to_default(&mut svc, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn reset_scope() {
        let mut svc = make_pref_service();
        let results = preference_reset_scope(&mut svc, PreferenceScope::Window);
        assert!(!results.is_empty());
    }

    #[test]
    fn count_modified() {
        let svc = make_pref_service();
        let count = preference_count_modified(&svc);
        assert_eq!(count, 1); // only fontSize differs
    }

    #[test]
    fn reset_result_display_modified() {
        let r = PreferenceResetResult {
            key: "editor.fontSize".into(),
            old_value: "16".into(),
            new_value: "14".into(),
            was_already_default: false,
        };
        let s = format!("{r}");
        assert!(s.contains("16"));
        assert!(s.contains("14"));
    }

    #[test]
    fn reset_result_display_default() {
        let r = PreferenceResetResult {
            key: "x".into(),
            old_value: "1".into(),
            new_value: "1".into(),
            was_already_default: true,
        };
        assert!(format!("{r}").contains("already at default"));
    }

    #[test]
    fn override_count_tracks_overrides() {
        let mut svc = PreferencesService::new();
        svc.register(desc("a", "1", PreferenceScope::Window));
        svc.register(desc("b", "2", PreferenceScope::Window));
        assert_eq!(svc.override_count(), 0);
        svc.set_override("a", "10");
        assert_eq!(svc.override_count(), 1);
        svc.set_override("b", "20");
        assert_eq!(svc.override_count(), 2);
    }

    #[test]
    fn keys_returns_all_registered_keys() {
        let mut svc = PreferencesService::new();
        svc.register(desc("editor.fontSize", "14", PreferenceScope::Window));
        svc.register(desc("editor.tabSize", "4", PreferenceScope::Resource));
        let keys = svc.keys();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"editor.fontSize"));
        assert!(keys.contains(&"editor.tabSize"));
    }

    #[test]
    fn summary_includes_counts() {
        let mut svc = PreferencesService::new();
        svc.register(desc("a", "1", PreferenceScope::Window));
        svc.register(desc("b", "2", PreferenceScope::Application));
        svc.set_override("a", "10");
        let s = svc.summary();
        assert!(s.contains("2 preferences"));
        assert!(s.contains("1 overridden"));
    }

    #[test]
    fn preferences_service_display() {
        let mut svc = PreferencesService::new();
        svc.register(desc("x", "1", PreferenceScope::Window));
        let s = format!("{svc}");
        assert!(s.contains("1 descriptors"));
        assert!(s.contains("0 overrides"));
    }

    #[test]
    fn descriptor_is_boolean_and_is_enum() {
        let mut d = desc("flag", "true", PreferenceScope::Window);
        d.preference_type = PreferenceType::Boolean;
        assert!(d.is_boolean());
        assert!(!d.is_enum());
        d.enum_values = vec!["a".into(), "b".into()];
        assert!(d.is_enum());
    }

    #[test]
    fn preference_scope_is_user_scope() {
        assert!(PreferenceScope::Application.is_user_scope());
        assert!(PreferenceScope::Window.is_user_scope());
        assert!(!PreferenceScope::Machine.is_user_scope());
        assert!(!PreferenceScope::Resource.is_user_scope());
        assert!(!PreferenceScope::Language.is_user_scope());
    }

    #[test]
    fn preference_type_is_numeric() {
        assert!(PreferenceType::Number.is_numeric());
        assert!(!PreferenceType::String.is_numeric());
        assert!(!PreferenceType::Boolean.is_numeric());
    }

    #[test]
    fn validation_rule_description() {
        assert_eq!(ValidationRule::NonEmpty.description(), "value must not be empty");
        assert_eq!(
            ValidationRule::MinLength(3).description(),
            "value must meet minimum length"
        );
        assert_eq!(
            ValidationRule::NumericRange(0.0, 100.0).description(),
            "value must be within the numeric range"
        );
    }

    #[test]
    fn validate_value_type_boolean() {
        let d = desc("flag", "true", PreferenceScope::Application);
        let mut d = d;
        d.preference_type = PreferenceType::Boolean;
        assert!(validate_value_type(&d, "true").is_ok());
        assert!(validate_value_type(&d, "false").is_ok());
        assert!(validate_value_type(&d, "yes").is_err());
    }

    #[test]
    fn validate_value_type_number() {
        let mut d = desc("size", "12", PreferenceScope::Application);
        d.preference_type = PreferenceType::Number;
        assert!(validate_value_type(&d, "42").is_ok());
        assert!(validate_value_type(&d, "3.14").is_ok());
        assert!(validate_value_type(&d, "abc").is_err());
    }

    #[test]
    fn validate_value_type_enum() {
        let mut d = desc("theme", "dark", PreferenceScope::Application);
        d.preference_type = PreferenceType::Enum;
        d.enum_values = vec!["dark".into(), "light".into()];
        assert!(validate_value_type(&d, "dark").is_ok());
        assert!(validate_value_type(&d, "blue").is_err());
    }

    #[test]
    fn apply_migrations_renames_key() {
        let mut svc = PreferencesService::new();
        svc.register(desc("old.key", "default", PreferenceScope::Application));
        svc.register(desc("new.key", "default", PreferenceScope::Application));
        svc.set_override("old.key", "custom_value");
        let migrations = vec![PreferenceMigration::rename("old.key", "new.key")];
        let count = apply_migrations(&mut svc, &migrations);
        assert_eq!(count, 1);
        assert!(!svc.has_override("old.key"));
        assert!(svc.has_override("new.key"));
        assert_eq!(svc.get_value("new.key"), "custom_value");
    }

    #[test]
    fn compute_preference_diff_detects_changes() {
        let mut before = HashMap::new();
        before.insert("a".into(), "1".into());
        before.insert("b".into(), "2".into());
        let mut after = HashMap::new();
        after.insert("a".into(), "1".into()); // unchanged
        after.insert("b".into(), "3".into()); // changed
        after.insert("c".into(), "4".into()); // added
        let diffs = compute_preference_diff(&before, &after);
        assert_eq!(diffs.len(), 2); // changed + added (no removed since 'a' still present)
        assert!(diffs.iter().any(|d| matches!(d, PreferenceDiff::Changed { key, .. } if key == "b")));
        assert!(diffs.iter().any(|d| matches!(d, PreferenceDiff::Added { key, .. } if key == "c")));
    }

    #[test]
    fn resolve_layered_uses_highest_priority() {
        let mut global = HashMap::new();
        global.insert("theme".into(), "dark".into());
        let mut workspace = HashMap::new();
        workspace.insert("theme".into(), "light".into());
        let result = resolve_layered("theme", &[&global, &workspace], "default");
        assert_eq!(result, "light");
        let result_missing = resolve_layered("font", &[&global, &workspace], "monospace");
        assert_eq!(result_missing, "monospace");
    }

    // -- New functionality tests --

    #[test]
    fn validate_overrides_catches_errors() {
        let mut svc = PreferencesService::new();
        svc.register(desc("editor.fontSize", "14", PreferenceScope::Window));
        svc.set_override("editor.fontSize", "abc");
        let mut rules = HashMap::new();
        rules.insert(
            "editor.fontSize".to_string(),
            vec![ValidationRule::NumericRange(8.0, 72.0)],
        );
        let errors = svc.validate(&rules);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].0, "editor.fontSize");
    }

    #[test]
    fn validate_overrides_passes() {
        let mut svc = PreferencesService::new();
        svc.register(desc("editor.fontSize", "14", PreferenceScope::Window));
        svc.set_override("editor.fontSize", "16");
        let mut rules = HashMap::new();
        rules.insert(
            "editor.fontSize".to_string(),
            vec![ValidationRule::NumericRange(8.0, 72.0)],
        );
        let errors = svc.validate(&rules);
        assert!(errors.is_empty());
    }

    #[test]
    fn bulk_set_overrides() {
        let mut svc = PreferencesService::new();
        svc.register(desc("a", "1", PreferenceScope::Window));
        svc.register(desc("b", "2", PreferenceScope::Window));
        let count = svc.bulk_set(&[("a", "10"), ("b", "20")]);
        assert_eq!(count, 2);
        assert_eq!(svc.get_value("a"), "10");
        assert_eq!(svc.get_value("b"), "20");
    }

    #[test]
    fn diff_detects_changes() {
        let mut svc = PreferencesService::new();
        svc.register(desc("a", "1", PreferenceScope::Window));
        svc.register(desc("b", "2", PreferenceScope::Window));
        svc.set_override("a", "10");
        let prev = svc.snapshot_overrides();

        svc.set_override("a", "99"); // changed
        svc.set_override("b", "20"); // added
        let (added, changed, removed) = svc.diff(&prev);
        assert!(added.contains(&"b".to_string()));
        assert!(changed.contains(&"a".to_string()));
        assert!(removed.is_empty());
    }

    #[test]
    fn diff_detects_removed() {
        let mut svc = PreferencesService::new();
        svc.register(desc("x", "0", PreferenceScope::Window));
        svc.set_override("x", "1");
        let prev = svc.snapshot_overrides();
        svc.reset("x");
        let (added, changed, removed) = svc.diff(&prev);
        assert!(added.is_empty());
        assert!(changed.is_empty());
        assert!(removed.contains(&"x".to_string()));
    }

    #[test]
    fn descriptor_matches_scope() {
        let d = desc("a", "1", PreferenceScope::Window);
        assert!(d.matches_scope(PreferenceScope::Window));
        assert!(!d.matches_scope(PreferenceScope::Machine));
    }

    #[test]
    fn descriptor_has_prefix() {
        let d = desc("editor.fontSize", "14", PreferenceScope::Window);
        assert!(d.has_prefix("editor."));
        assert!(!d.has_prefix("terminal."));
    }

    #[test]
    fn scope_is_resource_level() {
        assert!(PreferenceScope::Resource.is_resource_level());
        assert!(PreferenceScope::Language.is_resource_level());
        assert!(!PreferenceScope::Window.is_resource_level());
        assert!(!PreferenceScope::Application.is_resource_level());
    }

    // -------------------------------------------------------------------
    // New tests for added functionality
    // -------------------------------------------------------------------

    #[test]
    fn scope_priority_ordering() {
        assert!(PreferenceScope::Language.priority() > PreferenceScope::Application.priority());
        assert!(PreferenceScope::Resource.priority() > PreferenceScope::Window.priority());
        let ordered = PreferenceScope::all_ordered();
        assert_eq!(ordered.len(), 5);
        for w in ordered.windows(2) {
            assert!(w[0].priority() < w[1].priority());
        }
    }

    #[test]
    fn schema_validates_number_range() {
        let schema = PreferenceSchema::new("editor.fontSize", PreferenceType::Number)
            .range(8.0, 72.0);
        assert!(schema.validate_value("14").is_empty());
        assert!(!schema.validate_value("4").is_empty());
        assert!(!schema.validate_value("100").is_empty());
        assert!(!schema.validate_value("abc").is_empty());
    }

    #[test]
    fn schema_validates_boolean() {
        let schema = PreferenceSchema::new("editor.wordWrap", PreferenceType::Boolean);
        assert!(schema.validate_value("true").is_empty());
        assert!(schema.validate_value("false").is_empty());
        assert!(!schema.validate_value("maybe").is_empty());
    }

    #[test]
    fn schema_validates_enum_allowed() {
        let schema = PreferenceSchema::new("editor.cursorStyle", PreferenceType::Enum)
            .allowed(vec!["line".into(), "block".into(), "underline".into()]);
        assert!(schema.validate_value("line").is_empty());
        assert!(!schema.validate_value("crosshair").is_empty());
    }

    #[test]
    fn schema_required_rejects_empty() {
        let schema = PreferenceSchema::new("x", PreferenceType::String).required();
        let errs = schema.validate_value("");
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("required"));
    }

    #[test]
    fn schema_deprecated_emits_warning() {
        let schema = PreferenceSchema::new("old.setting", PreferenceType::String)
            .deprecated_with("use new.setting instead");
        let errs = schema.validate_value("anything");
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("deprecated"));
        assert!(errs[0].contains("use new.setting instead"));
    }

    #[test]
    fn schema_registry_validates_service() {
        let mut svc = PreferencesService::new();
        svc.register(desc("editor.fontSize", "14", PreferenceScope::Window));
        svc.set_override("editor.fontSize", "999");

        let mut registry = SchemaRegistry::new();
        registry.register(
            PreferenceSchema::new("editor.fontSize", PreferenceType::Number).range(8.0, 72.0),
        );
        let errs = registry.validate_service(&svc);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("above maximum"));
    }

    #[test]
    fn schema_registry_deprecated_overrides() {
        let mut svc = PreferencesService::new();
        svc.register(desc("old.key", "v", PreferenceScope::Window));
        svc.set_override("old.key", "v2");

        let mut registry = SchemaRegistry::new();
        registry.register(
            PreferenceSchema::new("old.key", PreferenceType::String)
                .deprecated_with("removed"),
        );
        let dep = registry.deprecated_overrides(&svc);
        assert_eq!(dep, vec!["old.key"]);
    }

    #[test]
    fn coerce_boolean_values() {
        assert_eq!(coerce_value("yes", PreferenceType::Boolean).unwrap(), "true");
        assert_eq!(coerce_value("0", PreferenceType::Boolean).unwrap(), "false");
        assert_eq!(coerce_value("on", PreferenceType::Boolean).unwrap(), "true");
        assert_eq!(coerce_value("off", PreferenceType::Boolean).unwrap(), "false");
        assert!(coerce_value("maybe", PreferenceType::Boolean).is_err());
    }

    #[test]
    fn coerce_number_values() {
        assert_eq!(coerce_value("42", PreferenceType::Number).unwrap(), "42");
        assert!(coerce_value("abc", PreferenceType::Number).is_err());
    }

    #[test]
    fn merge_layers_last_wins() {
        let mut defaults = SettingsLayer::new("defaults");
        defaults.set("editor.fontSize", "14");
        defaults.set("editor.tabSize", "4");

        let mut user = SettingsLayer::new("user");
        user.set("editor.fontSize", "16");

        let mut workspace = SettingsLayer::new("workspace");
        workspace.set("editor.fontSize", "18");
        workspace.set("editor.wordWrap", "on");

        let (merged, origins) = merge_layers(&[&defaults, &user, &workspace]);
        assert_eq!(merged["editor.fontSize"], "18");
        assert_eq!(merged["editor.tabSize"], "4");
        assert_eq!(merged["editor.wordWrap"], "on");
        assert_eq!(origins["editor.fontSize"], "workspace");
        assert_eq!(origins["editor.tabSize"], "defaults");
    }

    #[test]
    fn detect_overridden_keys_works() {
        let mut base = SettingsLayer::new("base");
        base.set("a", "1");
        base.set("b", "2");

        let mut top = SettingsLayer::new("top");
        top.set("a", "99");

        let overridden = detect_overridden_keys(&[&base, &top]);
        assert_eq!(overridden, vec!["a"]);
    }

    #[test]
    fn group_by_prefix_groups_correctly() {
        let mut svc = PreferencesService::new();
        svc.register(desc("editor.fontSize", "14", PreferenceScope::Window));
        svc.register(desc("editor.tabSize", "4", PreferenceScope::Window));
        svc.register(desc("terminal.shell", "/bin/sh", PreferenceScope::Machine));
        svc.register(desc("standalone", "x", PreferenceScope::Application));

        let groups = svc.group_by_prefix();
        assert_eq!(groups["editor"].len(), 2);
        assert_eq!(groups["terminal"].len(), 1);
        assert_eq!(groups["standalone"].len(), 1);
    }

    #[test]
    fn modified_keys_detects_changes() {
        let mut svc = PreferencesService::new();
        svc.register(desc("a", "1", PreferenceScope::Window));
        svc.register(desc("b", "2", PreferenceScope::Window));
        svc.set_override("a", "99");
        let modified = svc.modified_keys();
        assert_eq!(modified, vec!["a"]);
    }

    #[test]
    fn search_descriptions_finds_matches() {
        let mut svc = PreferencesService::new();
        let mut d = desc("editor.fontSize", "14", PreferenceScope::Window);
        d.description = "Controls the font size in pixels".to_string();
        svc.register(d);
        let mut d2 = desc("editor.tabSize", "4", PreferenceScope::Window);
        d2.description = "The number of spaces a tab is equal to".to_string();
        svc.register(d2);

        let results = svc.search_descriptions("font");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "editor.fontSize");
    }

    #[test]
    fn transform_overrides_applies_fn() {
        let mut svc = PreferencesService::new();
        svc.register(desc("a", "x", PreferenceScope::Window));
        svc.set_override("a", "hello");
        svc.transform_overrides(|v| v.to_uppercase());
        assert_eq!(svc.get_value("a"), "HELLO");
    }

    #[test]
    fn prune_stale_overrides_removes_unregistered() {
        let mut svc = PreferencesService::new();
        svc.register(desc("a", "1", PreferenceScope::Window));
        svc.set_override("a", "2");
        svc.set_override("ghost", "boo");
        let stale = svc.prune_stale_overrides();
        assert_eq!(stale, vec!["ghost"]);
        assert!(!svc.has_override("ghost"));
        assert!(svc.has_override("a"));
    }

    #[test]
    fn schema_registry_is_empty() {
        let reg = SchemaRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn coerce_string_is_identity() {
        assert_eq!(coerce_value("anything", PreferenceType::String).unwrap(), "anything");
    }

    #[test]
    fn preferencesSearchIndex_new() {
        let s = PreferencesSearchIndex::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn preferencesSearchIndex_add_contains() {
        let mut s = PreferencesSearchIndex::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn preferencesSearchIndex_add_duplicate() {
        let mut s = PreferencesSearchIndex::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn preferencesSearchIndex_remove() {
        let mut s = PreferencesSearchIndex::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn preferencesSearchIndex_capacity() {
        let s = PreferencesSearchIndex::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn preferencesSearchIndex_search() {
        let mut s = PreferencesSearchIndex::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn preferencesSearchIndex_stats() {
        let mut s = PreferencesSearchIndex::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn preferencesModifiedBadge_new() {
        let m = PreferencesModifiedBadge::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn preferencesModifiedBadge_add_find() {
        let mut m = PreferencesModifiedBadge::new();
        m.add(PreferencesModifiedBadgeItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn preferencesModifiedBadge_priority_filter() {
        let mut m = PreferencesModifiedBadge::new();
        m.add(PreferencesModifiedBadgeItem::new("a", "A").with_priority(PreferencesModifiedBadgePriority::High));
        m.add(PreferencesModifiedBadgeItem::new("b", "B").with_priority(PreferencesModifiedBadgePriority::Low));
        m.add(PreferencesModifiedBadgeItem::new("c", "C").with_priority(PreferencesModifiedBadgePriority::High));
        assert_eq!(m.by_priority(PreferencesModifiedBadgePriority::High).len(), 2);
    }

    #[test]
    fn preferencesModifiedBadge_remove() {
        let mut m = PreferencesModifiedBadge::new();
        m.add(PreferencesModifiedBadgeItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn preferencesModifiedBadge_search() {
        let mut m = PreferencesModifiedBadge::new();
        m.add(PreferencesModifiedBadgeItem::new("id1", "Hello World"));
        m.add(PreferencesModifiedBadgeItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn preferencesModifiedBadge_total_weight() {
        let mut m = PreferencesModifiedBadge::new();
        m.add(PreferencesModifiedBadgeItem::new("a", "A").with_priority(PreferencesModifiedBadgePriority::Critical));
        m.add(PreferencesModifiedBadgeItem::new("b", "B").with_priority(PreferencesModifiedBadgePriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn preferencesModifiedBadge_capacity_limit() {
        let mut m = PreferencesModifiedBadge::new().with_max_items(2);
        m.add(PreferencesModifiedBadgeItem::new("1", "one"));
        m.add(PreferencesModifiedBadgeItem::new("2", "two"));
        assert!(!m.add(PreferencesModifiedBadgeItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn preferencesModifiedBadge_sorted_by_priority() {
        let mut m = PreferencesModifiedBadge::new();
        m.add(PreferencesModifiedBadgeItem::new("lo", "Low").with_priority(PreferencesModifiedBadgePriority::Low));
        m.add(PreferencesModifiedBadgeItem::new("hi", "High").with_priority(PreferencesModifiedBadgePriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn preferencesModifiedBadge_item_metadata() {
        let mut item = PreferencesModifiedBadgeItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn preferencesSearchIndex_enabled_toggle() {
        let mut s = PreferencesSearchIndex::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn preferencesModifiedBadge_priority_display() {
        assert_eq!(format!("{}", PreferencesModifiedBadgePriority::High), "high");
        assert_eq!(format!("{}", PreferencesModifiedBadgePriority::Low), "low");
    }


    #[test]
    fn wb_preferences_entry_creation() {
        let e = WbPreferencesEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wb_preferences_entry_with_priority() {
        let e = WbPreferencesEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wb_preferences_entry_metadata() {
        let e = WbPreferencesEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wb_preferences_entry_remove_meta() {
        let mut e = WbPreferencesEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wb_preferences_entry_activate_deactivate() {
        let mut e = WbPreferencesEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wb_preferences_config_add_sorted() {
        let mut c = WbPreferencesConfig::new(10);
        c.add(WbPreferencesEntry::new("lo", "Lo").with_priority(1));
        c.add(WbPreferencesEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wb_preferences_config_capacity() {
        let mut c = WbPreferencesConfig::new(1);
        assert!(c.add(WbPreferencesEntry::new("a", "A")));
        assert!(!c.add(WbPreferencesEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wb_preferences_config_remove() {
        let mut c = WbPreferencesConfig::new(10);
        c.add(WbPreferencesEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wb_preferences_config_get() {
        let mut c = WbPreferencesConfig::new(10);
        c.add(WbPreferencesEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wb_preferences_config_active_entries() {
        let mut c = WbPreferencesConfig::new(10);
        c.add(WbPreferencesEntry::new("a", "A"));
        c.add(WbPreferencesEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wb_preferences_config_enable_disable() {
        let mut c = WbPreferencesConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wb_preferences_config_clear() {
        let mut c = WbPreferencesConfig::new(10);
        c.add(WbPreferencesEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wb_preferences_config_find_by_label() {
        let mut c = WbPreferencesConfig::new(10);
        c.add(WbPreferencesEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wb_preferences_config_top_n() {
        let mut c = WbPreferencesConfig::new(10);
        c.add(WbPreferencesEntry::new("a", "A").with_priority(1));
        c.add(WbPreferencesEntry::new("b", "B").with_priority(2));
        c.add(WbPreferencesEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wb_preferences_config_deactivate_activate_all() {
        let mut c = WbPreferencesConfig::new(10);
        c.add(WbPreferencesEntry::new("a", "A"));
        c.add(WbPreferencesEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wb_preferences_config_highest_priority() {
        let mut c = WbPreferencesConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WbPreferencesEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wb_preferences_config_contains() {
        let mut c = WbPreferencesConfig::new(10);
        c.add(WbPreferencesEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wb_preferences_config_labels() {
        let mut c = WbPreferencesConfig::new(10);
        c.add(WbPreferencesEntry::new("a", "Alpha"));
        c.add(WbPreferencesEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wb_preferences_config_drain_inactive() {
        let mut c = WbPreferencesConfig::new(10);
        c.add(WbPreferencesEntry::new("a", "A"));
        c.add(WbPreferencesEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for wb_preferences
    #[test]
    fn xa_wb_preferences_ring_new() {
        let rb = super::XaWbPreferencesRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_preferences_ring_push_len() {
        let mut rb = super::XaWbPreferencesRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_preferences_ring_wrap() {
        let mut rb = super::XaWbPreferencesRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_preferences_ring_mean_empty() {
        let rb = super::XaWbPreferencesRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_preferences_ring_mean_values() {
        let mut rb = super::XaWbPreferencesRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_preferences_ring_min_max() {
        let mut rb = super::XaWbPreferencesRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_preferences_ring_iter() {
        let mut rb = super::XaWbPreferencesRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_preferences_counter_new() {
        let c = super::XaWbPreferencesCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_preferences_counter_inc() {
        let mut c = super::XaWbPreferencesCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_preferences_counter_inc_by() {
        let mut c = super::XaWbPreferencesCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_preferences_counter_reset() {
        let mut c = super::XaWbPreferencesCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_preferences_counter_clear() {
        let mut c = super::XaWbPreferencesCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_preferences_counter_default() {
        let c = super::XaWbPreferencesCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 221 ----

    #[test]
    fn xc_221_pool_new_empty() {
        let pool: super::Xc221Pool<i32> = super::Xc221Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_221_pool_release_acquire() {
        let mut pool = super::Xc221Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_221_pool_acquire_empty() {
        let mut pool: super::Xc221Pool<i32> = super::Xc221Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_221_pool_full() {
        let mut pool = super::Xc221Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_221_pool_drain() {
        let mut pool = super::Xc221Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_221_pool_stats() {
        let mut pool = super::Xc221Pool::new(8);
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
    fn xc_221_pool_clear() {
        let mut pool = super::Xc221Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_221_pool_shrink() {
        let mut pool = super::Xc221Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_221_pool_default() {
        let pool: super::Xc221Pool<String> = super::Xc221Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_221_pool_extend() {
        let mut pool = super::Xc221Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_221_pool_retain() {
        let mut pool = super::Xc221Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_221_scheduler_round_robin() {
        let mut sched = super::Xc221Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_221_scheduler_empty() {
        let mut sched = super::Xc221Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_221_scheduler_reset() {
        let mut sched = super::Xc221Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_221_scheduler_add_remove() {
        let mut sched = super::Xc221Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_221_scheduler_targets() {
        let sched = super::Xc221Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_221_hash_empty() {
        assert_eq!(super::xc_221_hash(b""), 5381);
    }

    #[test]
    fn xc_221_hash_data() {
        let h = super::xc_221_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_221_hash(b"hello"), h);
    }

    #[test]
    fn xc_221_reverse_str() {
        assert_eq!(super::xc_221_reverse("abc"), "cba");
        assert_eq!(super::xc_221_reverse(""), "");
    }


    // --- xd_51 deepening tests ---

    #[test]
    fn xd_51_sm_initial_state() {
        let sm = Xd51StateMachine::new();
        assert_eq!(sm.current_state(), Xd51State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_51_sm_valid_idle_to_running() {
        let mut sm = Xd51StateMachine::new();
        assert!(sm.transition(Xd51State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd51State::Running);
    }

    #[test]
    fn xd_51_sm_valid_running_to_paused() {
        let mut sm = Xd51StateMachine::new();
        sm.transition(Xd51State::Running).unwrap();
        assert!(sm.transition(Xd51State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd51State::Paused);
    }

    #[test]
    fn xd_51_sm_valid_running_to_done() {
        let mut sm = Xd51StateMachine::new();
        sm.transition(Xd51State::Running).unwrap();
        assert!(sm.transition(Xd51State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd51State::Done);
    }

    #[test]
    fn xd_51_sm_valid_paused_to_running() {
        let mut sm = Xd51StateMachine::new();
        sm.transition(Xd51State::Running).unwrap();
        sm.transition(Xd51State::Paused).unwrap();
        assert!(sm.transition(Xd51State::Running).is_ok());
    }

    #[test]
    fn xd_51_sm_valid_done_to_idle() {
        let mut sm = Xd51StateMachine::new();
        sm.transition(Xd51State::Running).unwrap();
        sm.transition(Xd51State::Done).unwrap();
        assert!(sm.transition(Xd51State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd51State::Idle);
    }

    #[test]
    fn xd_51_sm_invalid_idle_to_done() {
        let mut sm = Xd51StateMachine::new();
        assert!(sm.transition(Xd51State::Done).is_err());
    }

    #[test]
    fn xd_51_sm_invalid_idle_to_paused() {
        let mut sm = Xd51StateMachine::new();
        assert!(sm.transition(Xd51State::Paused).is_err());
    }

    #[test]
    fn xd_51_sm_history_tracking() {
        let mut sm = Xd51StateMachine::new();
        sm.transition(Xd51State::Running).unwrap();
        sm.transition(Xd51State::Paused).unwrap();
        sm.transition(Xd51State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd51State::Idle);
        assert_eq!(sm.history()[0].to, Xd51State::Running);
        assert_eq!(sm.history()[1].from, Xd51State::Running);
        assert_eq!(sm.history()[2].to, Xd51State::Done);
    }

    #[test]
    fn xd_51_sm_serialize_deserialize() {
        let mut sm = Xd51StateMachine::new();
        sm.transition(Xd51State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd51StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd51State::Running));
    }

    #[test]
    fn xd_51_sm_deserialize_invalid() {
        assert_eq!(Xd51StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_51_sm_reset() {
        let mut sm = Xd51StateMachine::new();
        sm.transition(Xd51State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd51State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_51_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd51EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd51Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_51_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd51EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd51Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd51Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_51_bus_unsubscribe() {
        let mut bus = Xd51EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_51_event_kind_and_payload() {
        let e = Xd51Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd51Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_51_bus_clear_history() {
        let mut bus = Xd51EventBus::new();
        bus.publish(Xd51Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_51_sm_step_counter_increments() {
        let mut sm = Xd51StateMachine::new();
        sm.transition(Xd51State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd51State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }

}
