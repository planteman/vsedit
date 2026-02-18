//! Settings editor UI.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

/// The data type of a setting value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingType {
    String,
    Number,
    Boolean,
    Enum,
    Array,
    Object,
}

impl fmt::Display for SettingType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingType::String => write!(f, "string"),
            SettingType::Number => write!(f, "number"),
            SettingType::Boolean => write!(f, "boolean"),
            SettingType::Enum => write!(f, "enum"),
            SettingType::Array => write!(f, "array"),
            SettingType::Object => write!(f, "object"),
        }
    }
}

/// Scope at which a setting applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingScope {
    User,
    Workspace,
    WorkspaceFolder,
    Language,
}

impl fmt::Display for SettingScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingScope::User => write!(f, "User"),
            SettingScope::Workspace => write!(f, "Workspace"),
            SettingScope::WorkspaceFolder => write!(f, "Workspace Folder"),
            SettingScope::Language => write!(f, "Language"),
        }
    }
}

/// Errors that can occur when manipulating settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    SettingNotFound(String),
    InvalidValue(String),
    ReadOnly(String),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::SettingNotFound(key) => write!(f, "setting not found: {key}"),
            SettingsError::InvalidValue(msg) => write!(f, "invalid value: {msg}"),
            SettingsError::ReadOnly(key) => write!(f, "setting is read-only: {key}"),
        }
    }
}

/// A single setting displayed in the settings editor.
#[derive(Debug, Clone)]
pub struct SettingItem {
    pub key: String,
    pub label: String,
    pub description: String,
    pub setting_type: SettingType,
    pub default_value: String,
    pub current_value: Option<String>,
    pub enum_values: Vec<String>,
    pub scope: SettingScope,
}

impl SettingItem {
    /// Returns `true` if the current value differs from the default.
    pub fn is_modified(&self) -> bool {
        match &self.current_value {
            Some(v) => v != &self.default_value,
            None => false,
        }
    }

    /// Resets the setting to its default by clearing the current value.
    pub fn reset(&mut self) {
        self.current_value = None;
    }

    /// Validates and sets the current value.
    ///
    /// For `Enum` settings the value must be one of the allowed `enum_values`.
    /// For `Boolean` settings the value must be `"true"` or `"false"`.
    /// For `Number` settings the value must parse as an `f64`.
    pub fn set_value(&mut self, value: String) -> Result<(), SettingsError> {
        match &self.setting_type {
            SettingType::Enum => {
                if !self.enum_values.contains(&value) {
                    return Err(SettingsError::InvalidValue(format!(
                        "'{value}' is not one of {:?}",
                        self.enum_values
                    )));
                }
            }
            SettingType::Boolean => {
                if value != "true" && value != "false" {
                    return Err(SettingsError::InvalidValue(format!(
                        "expected 'true' or 'false', got '{value}'"
                    )));
                }
            }
            SettingType::Number => {
                if value.parse::<f64>().is_err() {
                    return Err(SettingsError::InvalidValue(format!(
                        "'{value}' is not a valid number"
                    )));
                }
            }
            _ => {}
        }
        self.current_value = Some(value);
        Ok(())
    }
}

/// Builder for constructing [`SettingItem`] instances.
#[derive(Debug)]
pub struct SettingItemBuilder {
    key: String,
    label: String,
    description: String,
    setting_type: SettingType,
    default_value: String,
    current_value: Option<String>,
    enum_values: Vec<String>,
    scope: SettingScope,
}

impl SettingItemBuilder {
    pub fn new(key: impl Into<String>, setting_type: SettingType) -> Self {
        let key = key.into();
        Self {
            label: key.clone(),
            key,
            description: String::new(),
            setting_type,
            default_value: String::new(),
            current_value: None,
            enum_values: Vec::new(),
            scope: SettingScope::User,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn default_value(mut self, val: impl Into<String>) -> Self {
        self.default_value = val.into();
        self
    }

    pub fn current_value(mut self, val: impl Into<String>) -> Self {
        self.current_value = Some(val.into());
        self
    }

    pub fn enum_values(mut self, vals: Vec<String>) -> Self {
        self.enum_values = vals;
        self
    }

    pub fn scope(mut self, scope: SettingScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn build(self) -> SettingItem {
        SettingItem {
            key: self.key,
            label: self.label,
            description: self.description,
            setting_type: self.setting_type,
            default_value: self.default_value,
            current_value: self.current_value,
            enum_values: self.enum_values,
            scope: self.scope,
        }
    }
}

/// A registry that owns and manages a collection of settings.
#[derive(Debug, Default)]
pub struct SettingsRegistry {
    items: Vec<SettingItem>,
}

impl SettingsRegistry {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Adds a setting to the registry.
    pub fn add(&mut self, item: SettingItem) {
        self.items.push(item);
    }

    /// Returns a reference to a setting by key.
    pub fn get(&self, key: &str) -> Result<&SettingItem, SettingsError> {
        self.items
            .iter()
            .find(|s| s.key == key)
            .ok_or_else(|| SettingsError::SettingNotFound(key.to_string()))
    }

    /// Sets the value of a setting by key.
    pub fn set(&mut self, key: &str, value: String) -> Result<(), SettingsError> {
        let item = self
            .items
            .iter_mut()
            .find(|s| s.key == key)
            .ok_or_else(|| SettingsError::SettingNotFound(key.to_string()))?;
        item.set_value(value)
    }

    /// Resets a setting to its default by key.
    pub fn reset(&mut self, key: &str) -> Result<(), SettingsError> {
        let item = self
            .items
            .iter_mut()
            .find(|s| s.key == key)
            .ok_or_else(|| SettingsError::SettingNotFound(key.to_string()))?;
        item.reset();
        Ok(())
    }

    /// Returns keys of all modified settings.
    pub fn list_modified(&self) -> Vec<&str> {
        self.items
            .iter()
            .filter(|s| s.is_modified())
            .map(|s| s.key.as_str())
            .collect()
    }

    /// Returns a slice of all settings.
    pub fn all(&self) -> &[SettingItem] {
        &self.items
    }
}

/// Filter criteria for searching settings.
#[derive(Debug, Clone)]
pub struct SettingsFilter {
    pub query: String,
    pub scope: Option<SettingScope>,
    pub modified_only: bool,
}

impl SettingsFilter {
    /// Creates an empty filter that matches everything.
    pub fn empty() -> Self {
        Self {
            query: String::new(),
            scope: None,
            modified_only: false,
        }
    }
}

/// Group settings by their scope, returning a map from scope to indices.
pub fn group_settings_by_scope(settings: &[SettingItem]) -> HashMap<SettingScope, Vec<usize>> {
    let mut map: HashMap<SettingScope, Vec<usize>> = HashMap::new();
    for (i, item) in settings.iter().enumerate() {
        map.entry(item.scope).or_default().push(i);
    }
    map
}

/// Return indices of settings matching the given filter.
pub fn filter_settings(settings: &[SettingItem], filter: &SettingsFilter) -> Vec<usize> {
    let query_lower = filter.query.to_lowercase();
    settings
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            if filter.modified_only && item.current_value.is_none() {
                return false;
            }
            if let Some(scope) = filter.scope {
                if item.scope != scope {
                    return false;
                }
            }
            if !query_lower.is_empty() {
                let matches_key = item.key.to_lowercase().contains(&query_lower);
                let matches_label = item.label.to_lowercase().contains(&query_lower);
                let matches_desc = item.description.to_lowercase().contains(&query_lower);
                if !(matches_key || matches_label || matches_desc) {
                    return false;
                }
            }
            true
        })
        .map(|(i, _)| i)
        .collect()
}

/// A record of a single setting change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingChange {
    pub key: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub timestamp_ms: u64,
}

/// An append-only log of setting changes with undo support.
#[derive(Debug, Default)]
pub struct SettingsChangeLog {
    changes: Vec<SettingChange>,
}

impl SettingsChangeLog {
    pub fn new() -> Self {
        Self { changes: Vec::new() }
    }

    /// Record a change.
    pub fn record(&mut self, change: SettingChange) {
        self.changes.push(change);
    }

    /// Remove and return the most recent change.
    pub fn undo_last(&mut self) -> Option<SettingChange> {
        self.changes.pop()
    }

    /// Return all changes for a specific key.
    pub fn get_changes_for_key(&self, key: &str) -> Vec<&SettingChange> {
        self.changes.iter().filter(|c| c.key == key).collect()
    }

    /// Return the most recent `n` changes (newest last).
    pub fn get_recent(&self, n: usize) -> &[SettingChange] {
        let len = self.changes.len();
        if n >= len {
            &self.changes
        } else {
            &self.changes[len - n..]
        }
    }

    /// Clear all recorded changes.
    pub fn clear(&mut self) {
        self.changes.clear();
    }
}

/// Validate that a setting key is dot-separated alphanumeric segments.
///
/// Each segment must be non-empty and consist only of ASCII alphanumeric
/// characters or underscores. There must be at least two segments.
pub fn validate_setting_key(key: &str) -> Result<(), SettingsError> {
    if key.is_empty() {
        return Err(SettingsError::InvalidValue("key must not be empty".into()));
    }
    let segments: Vec<&str> = key.split('.').collect();
    if segments.len() < 2 {
        return Err(SettingsError::InvalidValue(
            "key must have at least two dot-separated segments".into(),
        ));
    }
    for seg in &segments {
        if seg.is_empty() {
            return Err(SettingsError::InvalidValue(
                "key segments must not be empty".into(),
            ));
        }
        if !seg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(SettingsError::InvalidValue(format!(
                "segment '{seg}' contains invalid characters"
            )));
        }
    }
    Ok(())
}

/// Minimal JSON exporter/importer using only the standard library.
pub struct SettingsExporter;

impl SettingsExporter {
    /// Serialize all settings in a registry to a JSON string.
    pub fn to_json(registry: &SettingsRegistry) -> String {
        let mut out = String::from("{\n");
        let items = registry.all();
        for (i, item) in items.iter().enumerate() {
            let value = item
                .current_value
                .as_deref()
                .unwrap_or(&item.default_value);
            out.push_str(&format!(
                "  \"{}\": \"{}\"",
                Self::escape(item.key.as_str()),
                Self::escape(value),
            ));
            if i + 1 < items.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push('}');
        out
    }

    /// Parse a simple JSON object of `"key": "value"` pairs.
    pub fn from_json(json: &str) -> Result<Vec<(String, String)>, SettingsError> {
        let trimmed = json.trim();
        if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
            return Err(SettingsError::InvalidValue("expected JSON object".into()));
        }
        let inner = &trimmed[1..trimmed.len() - 1];
        let mut result = Vec::new();
        for part in inner.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let Some(colon) = part.find(':') else {
                return Err(SettingsError::InvalidValue(format!(
                    "missing ':' in '{part}'"
                )));
            };
            let raw_key = part[..colon].trim();
            let raw_val = part[colon + 1..].trim();
            let key = Self::unquote(raw_key)?;
            let val = Self::unquote(raw_val)?;
            result.push((key, val));
        }
        Ok(result)
    }

    fn escape(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    }

    fn unquote(s: &str) -> Result<String, SettingsError> {
        let s = s.trim();
        if s.len() < 2 || !s.starts_with('"') || !s.ends_with('"') {
            return Err(SettingsError::InvalidValue(format!(
                "expected quoted string, got '{s}'"
            )));
        }
        Ok(s[1..s.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\n", "\n")
            .replace("\\\\", "\\"))
    }
}

/// The result of diffing two settings registries.
#[derive(Debug, Clone, Default)]
pub struct SettingsDiff {
    /// Keys only in `a`.
    pub only_in_a: Vec<String>,
    /// Keys only in `b`.
    pub only_in_b: Vec<String>,
    /// Keys present in both but with different effective values (current or default).
    pub changed: Vec<(String, String, String)>,
}

/// Compare two registries and return a diff.
pub fn diff_settings(a: &SettingsRegistry, b: &SettingsRegistry) -> SettingsDiff {
    let a_map: HashMap<&str, &str> = a
        .all()
        .iter()
        .map(|s| {
            (
                s.key.as_str(),
                s.current_value.as_deref().unwrap_or(&s.default_value),
            )
        })
        .collect();
    let b_map: HashMap<&str, &str> = b
        .all()
        .iter()
        .map(|s| {
            (
                s.key.as_str(),
                s.current_value.as_deref().unwrap_or(&s.default_value),
            )
        })
        .collect();

    let mut diff = SettingsDiff::default();
    for (&key, &val_a) in &a_map {
        match b_map.get(key) {
            Some(&val_b) if val_a != val_b => {
                diff.changed
                    .push((key.to_string(), val_a.to_string(), val_b.to_string()));
            }
            None => diff.only_in_a.push(key.to_string()),
            _ => {}
        }
    }
    for &key in b_map.keys() {
        if !a_map.contains_key(key) {
            diff.only_in_b.push(key.to_string());
        }
    }
    diff.only_in_a.sort();
    diff.only_in_b.sort();
    diff.changed.sort_by(|x, y| x.0.cmp(&y.0));
    diff
}

/// Exports settings from a registry to a simple key=value text representation,
/// with an optional key prefix filter.
pub struct SettingsFilteredExporter<'a> {
    registry: &'a SettingsRegistry,
    prefix: Option<&'a str>,
}

impl<'a> SettingsFilteredExporter<'a> {
    pub fn new(registry: &'a SettingsRegistry) -> Self {
        Self { registry, prefix: None }
    }

    /// Only export settings whose key starts with `prefix`.
    pub fn with_prefix(mut self, prefix: &'a str) -> Self {
        self.prefix = Some(prefix);
        self
    }

    /// Serializes matching settings to a JSON-like string of `"key": "value"` pairs.
    pub fn to_json_string(&self) -> String {
        let items: Vec<_> = self.registry.all().iter()
            .filter(|item| self.prefix.map_or(true, |p| item.key.starts_with(p)))
            .collect();
        let mut out = String::from("{\n");
        for (i, item) in items.iter().enumerate() {
            let val = item.current_value.as_deref().unwrap_or(&item.default_value);
            out.push_str(&format!("  \"{}\": \"{}\"", item.key, val));
            if i + 1 < items.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push('}');
        out
    }
}

/// The UI control to render for a setting.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingControl {
    /// A checkbox (for boolean settings).
    Checkbox { checked: bool },
    /// A text input field.
    TextInput { placeholder: String },
    /// A dropdown/select with options.
    Dropdown {
        options: Vec<String>,
        selected: Option<usize>,
    },
    /// A numeric spinner with min/max bounds.
    NumberInput {
        value: f64,
        min: f64,
        max: f64,
        step: f64,
    },
}

impl SettingControl {
    /// Infer the appropriate control from a `SettingItem`.
    pub fn from_setting(item: &SettingItem) -> Self {
        match &item.setting_type {
            SettingType::Boolean => {
                let checked = item
                    .current_value
                    .as_deref()
                    .unwrap_or(&item.default_value)
                    == "true";
                SettingControl::Checkbox { checked }
            }
            SettingType::Enum => {
                let current = item
                    .current_value
                    .as_deref()
                    .unwrap_or(&item.default_value);
                let selected = item.enum_values.iter().position(|v| v == current);
                SettingControl::Dropdown {
                    options: item.enum_values.clone(),
                    selected,
                }
            }
            SettingType::Number => {
                let val: f64 = item
                    .current_value
                    .as_deref()
                    .unwrap_or(&item.default_value)
                    .parse()
                    .unwrap_or(0.0);
                SettingControl::NumberInput {
                    value: val,
                    min: 0.0,
                    max: f64::MAX,
                    step: 1.0,
                }
            }
            _ => SettingControl::TextInput {
                placeholder: item.default_value.clone(),
            },
        }
    }
}

impl fmt::Display for SettingControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingControl::Checkbox { checked } => {
                write!(f, "[{}]", if *checked { "x" } else { " " })
            }
            SettingControl::TextInput { placeholder } => {
                write!(f, "[____] ({})", placeholder)
            }
            SettingControl::Dropdown { options, selected } => {
                let sel = selected
                    .map(|i| options[i].as_str())
                    .unwrap_or("none");
                write!(f, "[v {}] ({} options)", sel, options.len())
            }
            SettingControl::NumberInput { value, .. } => write!(f, "[{}]", value),
        }
    }
}

/// A constraint that can validate setting values.
#[derive(Debug, Clone)]
pub enum SettingValidation {
    /// Value must contain the given substring.
    Pattern(String),
    /// Numeric value must be within [min, max].
    Range { min: f64, max: f64 },
    /// String length must be within [min_len, max_len].
    Length { min_len: usize, max_len: usize },
    /// Value must be one of the listed options.
    OneOf(Vec<String>),
}

impl SettingValidation {
    /// Validate a value string against this constraint.
    pub fn validate(&self, value: &str) -> Result<(), String> {
        match self {
            SettingValidation::Pattern(pattern) => {
                if !value.contains(pattern.as_str()) {
                    return Err(format!("value must contain '{}'", pattern));
                }
            }
            SettingValidation::Range { min, max } => {
                let num: f64 = value
                    .parse()
                    .map_err(|_| format!("'{}' is not a number", value))?;
                if num < *min || num > *max {
                    return Err(format!("{} is outside range [{}, {}]", num, min, max));
                }
            }
            SettingValidation::Length { min_len, max_len } => {
                let len = value.len();
                if len < *min_len || len > *max_len {
                    return Err(format!(
                        "length {} is outside [{}, {}]",
                        len, min_len, max_len
                    ));
                }
            }
            SettingValidation::OneOf(options) => {
                if !options.contains(&value.to_string()) {
                    return Err(format!("'{}' is not one of {:?}", value, options));
                }
            }
        }
        Ok(())
    }
}

/// Generate a JSON patch showing only settings that differ from defaults.
///
/// Returns a JSON string with only the modified key-value pairs.
pub fn settings_to_json_patch(registry: &SettingsRegistry) -> String {
    let modified: Vec<&SettingItem> = registry.all().iter().filter(|s| s.is_modified()).collect();
    if modified.is_empty() {
        return "{}".to_string();
    }
    let mut out = String::from("{\n");
    for (i, item) in modified.iter().enumerate() {
        let val = item
            .current_value
            .as_deref()
            .unwrap_or(&item.default_value);
        out.push_str(&format!("  \"{}\": \"{}\"", item.key, val));
        if i + 1 < modified.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push('}');
    out
}

/// Compute the number of settings that differ from their defaults.
pub fn modified_settings_count(registry: &SettingsRegistry) -> usize {
    registry.all().iter().filter(|s| s.is_modified()).count()
}

// ---------------------------------------------------------------------------
// SettingType helpers
// ---------------------------------------------------------------------------

impl SettingType {
    /// Returns all type variants.
    pub fn all() -> &'static [SettingType] {
        &[
            SettingType::String,
            SettingType::Number,
            SettingType::Boolean,
            SettingType::Enum,
            SettingType::Array,
            SettingType::Object,
        ]
    }

    /// Returns true if this is a scalar type (not a container).
    pub fn is_scalar(&self) -> bool {
        matches!(self, SettingType::String | SettingType::Number | SettingType::Boolean)
    }

    /// Returns a suitable default value representation for this type.
    pub fn default_value_str(&self) -> &'static str {
        match self {
            SettingType::String => "\"\"",
            SettingType::Number => "0",
            SettingType::Boolean => "false",
            SettingType::Enum => "\"\"",
            SettingType::Array => "[]",
            SettingType::Object => "{}",
        }
    }
}

// ---------------------------------------------------------------------------
// SettingScope helpers
// ---------------------------------------------------------------------------

impl SettingScope {
    /// Returns all scope variants.
    pub fn all() -> &'static [SettingScope] {
        &[
            SettingScope::User,
            SettingScope::Workspace,
            SettingScope::WorkspaceFolder,
            SettingScope::Language,
        ]
    }

    /// Parse a scope from a string.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "user" => Some(Self::User),
            "workspace" => Some(Self::Workspace),
            "workspacefolder" | "folder" => Some(Self::WorkspaceFolder),
            "language" => Some(Self::Language),
            _ => None,
        }
    }

    /// Returns the relative priority (higher = more specific).
    pub fn priority(&self) -> u8 {
        match self {
            Self::User => 0,
            Self::Workspace => 1,
            Self::WorkspaceFolder => 2,
            Self::Language => 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Setting key utilities
// ---------------------------------------------------------------------------

/// Splits a dotted setting key into its parts.
pub fn split_setting_key(key: &str) -> Vec<&str> {
    key.split('.').collect()
}

/// Returns the category prefix of a setting key (e.g., "editor" from "editor.fontSize").
pub fn setting_category(key: &str) -> &str {
    key.split('.').next().unwrap_or(key)
}

/// Returns all unique categories from a list of setting items.
pub fn unique_categories(settings: &[SettingItem]) -> Vec<String> {
    let mut cats: Vec<String> = settings.iter()
        .map(|s| setting_category(&s.key).to_string())
        .collect();
    cats.sort();
    cats.dedup();
    cats
}

/// Counts settings by type.
pub fn count_by_type(settings: &[SettingItem]) -> std::collections::HashMap<String, usize> {
    let mut map = std::collections::HashMap::new();
    for s in settings {
        *map.entry(format!("{}", s.setting_type)).or_insert(0) += 1;
    }
    map
}

/// Iterator over setting keys matching a prefix.
pub struct SettingPrefixIter<'a> {
    settings: &'a [SettingItem],
    prefix: String,
    index: usize,
}

impl<'a> SettingPrefixIter<'a> {
    pub fn new(settings: &'a [SettingItem], prefix: &str) -> Self {
        Self {
            settings,
            prefix: prefix.to_string(),
            index: 0,
        }
    }
}

impl<'a> Iterator for SettingPrefixIter<'a> {
    type Item = &'a SettingItem;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.settings.len() {
            let item = &self.settings[self.index];
            self.index += 1;
            if item.key.starts_with(&self.prefix) {
                return Some(item);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// SettingValidator – configurable per-setting validation rules
// ---------------------------------------------------------------------------

/// A configurable validator that maps setting keys to validation rules.
///
/// Unlike `SettingValidation` which is a single constraint, `SettingValidator`
/// owns a collection of rules keyed by setting key and applies all matching
/// rules when validating a value.
#[derive(Debug, Clone, Default)]
pub struct SettingValidator {
    rules: HashMap<String, Vec<ValidationRule>>,
}

/// A single validation rule with a human-readable description.
#[derive(Debug, Clone)]
pub struct ValidationRule {
    pub description: String,
    pub kind: ValidationRuleKind,
}

/// The kind of check performed by a [`ValidationRule`].
#[derive(Debug, Clone)]
pub enum ValidationRuleKind {
    /// String length must be within [min, max].
    StringLength { min: usize, max: usize },
    /// Numeric value must be within [min, max].
    NumberRange { min: f64, max: f64 },
    /// Value must be one of the listed members.
    EnumMembership(Vec<String>),
    /// Value must contain the given substring pattern.
    RegexPattern(String),
}

impl fmt::Display for ValidationRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description)
    }
}

impl fmt::Display for ValidationRuleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationRuleKind::StringLength { min, max } => {
                write!(f, "string length [{min}, {max}]")
            }
            ValidationRuleKind::NumberRange { min, max } => {
                write!(f, "number range [{min}, {max}]")
            }
            ValidationRuleKind::EnumMembership(opts) => {
                write!(f, "one of {:?}", opts)
            }
            ValidationRuleKind::RegexPattern(pat) => {
                write!(f, "matches pattern '{pat}'")
            }
        }
    }
}

impl SettingValidator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a validation rule for a setting key.
    pub fn add_rule(&mut self, key: impl Into<String>, rule: ValidationRule) {
        self.rules.entry(key.into()).or_default().push(rule);
    }

    /// Validate a value against all rules registered for `key`.
    ///
    /// Returns a list of human-readable error messages for each failing rule.
    /// An empty vec means the value is valid.
    pub fn validate(&self, key: &str, value: &str) -> Vec<String> {
        let Some(rules) = self.rules.get(key) else {
            return Vec::new();
        };
        let mut errors = Vec::new();
        for rule in rules {
            if let Err(msg) = rule.kind.check(value) {
                errors.push(format!("{}: {}", rule.description, msg));
            }
        }
        errors
    }

    /// Returns true if the value passes all rules for the given key.
    pub fn is_valid(&self, key: &str, value: &str) -> bool {
        self.validate(key, value).is_empty()
    }

    /// Returns the number of rules registered for a key.
    pub fn rule_count(&self, key: &str) -> usize {
        self.rules.get(key).map_or(0, |r| r.len())
    }

    /// Returns all keys that have at least one rule.
    pub fn keys_with_rules(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.rules.keys().map(|k| k.as_str()).collect();
        keys.sort();
        keys
    }
}

impl ValidationRuleKind {
    fn check(&self, value: &str) -> Result<(), String> {
        match self {
            ValidationRuleKind::StringLength { min, max } => {
                let len = value.len();
                if len < *min || len > *max {
                    return Err(format!("length {len} outside [{min}, {max}]"));
                }
            }
            ValidationRuleKind::NumberRange { min, max } => {
                let num: f64 = value
                    .parse()
                    .map_err(|_| format!("'{value}' is not a number"))?;
                if num < *min || num > *max {
                    return Err(format!("{num} outside [{min}, {max}]"));
                }
            }
            ValidationRuleKind::EnumMembership(opts) => {
                if !opts.iter().any(|o| o == value) {
                    return Err(format!("'{value}' not in {:?}", opts));
                }
            }
            ValidationRuleKind::RegexPattern(pat) => {
                if !value.contains(pat.as_str()) {
                    return Err(format!("'{value}' does not contain '{pat}'"));
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SettingsDiff – Display impl
// ---------------------------------------------------------------------------

impl fmt::Display for SettingsDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.only_in_a.is_empty() && self.only_in_b.is_empty() && self.changed.is_empty() {
            return write!(f, "no differences");
        }
        for key in &self.only_in_a {
            writeln!(f, "- {key}")?;
        }
        for key in &self.only_in_b {
            writeln!(f, "+ {key}")?;
        }
        for (key, old, new) in &self.changed {
            writeln!(f, "~ {key}: {old} -> {new}")?;
        }
        Ok(())
    }
}

impl SettingsDiff {
    /// Returns true if there are no differences.
    pub fn is_empty(&self) -> bool {
        self.only_in_a.is_empty() && self.only_in_b.is_empty() && self.changed.is_empty()
    }

    /// Total number of differences (added + removed + changed).
    pub fn total(&self) -> usize {
        self.only_in_a.len() + self.only_in_b.len() + self.changed.len()
    }
}

// ---------------------------------------------------------------------------
// SettingsTextExporter – key=value plain text format
// ---------------------------------------------------------------------------

/// Serializes settings to a simple `key=value` text format (one per line)
/// and parses it back.
pub struct SettingsTextExporter;

impl SettingsTextExporter {
    /// Serialize all settings in a registry to `key=value` lines.
    pub fn to_text(registry: &SettingsRegistry) -> String {
        let mut out = String::new();
        for item in registry.all() {
            let value = item
                .current_value
                .as_deref()
                .unwrap_or(&item.default_value);
            out.push_str(&item.key);
            out.push('=');
            // Escape newlines and backslashes in the value.
            for ch in value.chars() {
                match ch {
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    _ => out.push(ch),
                }
            }
            out.push('\n');
        }
        out
    }

    /// Parse `key=value` lines back into pairs.
    ///
    /// Lines starting with `#` are treated as comments and skipped.
    /// Empty lines are skipped.
    pub fn from_text(text: &str) -> Result<Vec<(String, String)>, SettingsError> {
        let mut result = Vec::new();
        for (lineno, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some(eq_pos) = trimmed.find('=') else {
                return Err(SettingsError::InvalidValue(format!(
                    "line {}: missing '=' in '{trimmed}'",
                    lineno + 1
                )));
            };
            let key = trimmed[..eq_pos].trim().to_string();
            let raw_val = &trimmed[eq_pos + 1..];
            let val = raw_val
                .replace("\\n", "\n")
                .replace("\\\\", "\\");
            result.push((key, val));
        }
        Ok(result)
    }

    /// Apply parsed key=value pairs to a registry, setting each value.
    pub fn apply_to_registry(
        registry: &mut SettingsRegistry,
        pairs: &[(String, String)],
    ) -> Vec<SettingsError> {
        let mut errors = Vec::new();
        for (key, value) in pairs {
            if let Err(e) = registry.set(key, value.clone()) {
                errors.push(e);
            }
        }
        errors
    }
}

// ---------------------------------------------------------------------------
// SettingSearchIndex – fast full-text search
// ---------------------------------------------------------------------------

/// A pre-built index for fast full-text search across setting keys, labels,
/// and descriptions.
///
/// Tokens are lowercased words extracted from keys (split on `.`), labels,
/// and descriptions. Searching returns indices of matching `SettingItem`s.
#[derive(Debug, Clone)]
pub struct SettingSearchIndex {
    /// For each setting index, the set of lowercased tokens.
    entries: Vec<(usize, Vec<String>)>,
}

impl SettingSearchIndex {
    /// Build an index from a slice of settings.
    pub fn build(settings: &[SettingItem]) -> Self {
        let mut entries = Vec::with_capacity(settings.len());
        for (i, item) in settings.iter().enumerate() {
            let mut tokens = Vec::new();
            // Tokenize key segments
            for seg in item.key.split('.') {
                tokens.push(seg.to_lowercase());
            }
            // Tokenize label words
            for word in item.label.split_whitespace() {
                tokens.push(word.to_lowercase());
            }
            // Tokenize description words
            for word in item.description.split_whitespace() {
                tokens.push(word.to_lowercase());
            }
            entries.push((i, tokens));
        }
        Self { entries }
    }

    /// Search for settings matching the query string.
    ///
    /// The query is split into words; a setting matches if every query word
    /// is a substring of at least one of its tokens.
    pub fn search(&self, query: &str) -> Vec<usize> {
        let query_words: Vec<String> = query
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect();
        if query_words.is_empty() {
            return self.entries.iter().map(|(i, _)| *i).collect();
        }
        let mut results = Vec::new();
        for (idx, tokens) in &self.entries {
            let all_match = query_words.iter().all(|qw| {
                tokens.iter().any(|tok| tok.contains(qw.as_str()))
            });
            if all_match {
                results.push(*idx);
            }
        }
        results
    }

    /// Returns the number of indexed settings.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the index has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Display for SettingSearchIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SettingSearchIndex({} entries)", self.entries.len())
    }
}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

impl From<&str> for SettingsError {
    fn from(s: &str) -> Self {
        SettingsError::InvalidValue(s.to_string())
    }
}

impl From<String> for SettingsError {
    fn from(s: String) -> Self {
        SettingsError::InvalidValue(s)
    }
}

impl From<SettingItem> for SettingChange {
    fn from(item: SettingItem) -> Self {
        SettingChange {
            key: item.key,
            old_value: None,
            new_value: item.current_value,
            timestamp_ms: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// SettingsRegistry – merge support
// ---------------------------------------------------------------------------

impl SettingsRegistry {
    /// Merge settings from `other` into this registry.
    ///
    /// For keys present in both, the value from `other` wins.
    /// Keys only in `other` are added. Keys only in `self` are kept.
    pub fn merge(&mut self, other: &SettingsRegistry) {
        let existing: HashSet<String> = self.items.iter().map(|i| i.key.clone()).collect();
        for other_item in other.all() {
            if existing.contains(&other_item.key) {
                if let Some(item) = self.items.iter_mut().find(|i| i.key == other_item.key) {
                    item.current_value = other_item.current_value.clone();
                }
            } else {
                self.items.push(other_item.clone());
            }
        }
    }

    /// Returns the effective value (current or default) for a key.
    pub fn effective_value(&self, key: &str) -> Result<&str, SettingsError> {
        let item = self.get(key)?;
        Ok(item.current_value.as_deref().unwrap_or(&item.default_value))
    }

    /// Returns all setting keys as a sorted list.
    pub fn keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.items.iter().map(|i| i.key.as_str()).collect();
        keys.sort();
        keys
    }
}

// ---------------------------------------------------------------------------
// SettingsSearchIndex – full-text search over settings entries
// ---------------------------------------------------------------------------

/// Where a search query matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchLocation {
    Key,
    Label,
    Description,
}

impl fmt::Display for MatchLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchLocation::Key => write!(f, "key"),
            MatchLocation::Label => write!(f, "label"),
            MatchLocation::Description => write!(f, "description"),
        }
    }
}

/// A single search hit returned by [`SettingsSearchIndex::search`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub key: String,
    pub score: u32,
    pub match_location: MatchLocation,
}

impl fmt::Display for SearchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (score={}, {})", self.key, self.score, self.match_location)
    }
}

#[derive(Debug, Clone)]
struct SearchEntry {
    key: String,
    label: String,
    description: String,
}

/// Full-text search index for settings.
#[derive(Debug, Clone, Default)]
pub struct SettingsSearchIndex {
    entries: Vec<SearchEntry>,
}

impl SettingsSearchIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entry to the index.
    pub fn add_entry(&mut self, key: &str, label: &str, description: &str) {
        self.entries.push(SearchEntry {
            key: key.to_string(),
            label: label.to_string(),
            description: description.to_string(),
        });
    }

    /// Search the index. Results are sorted by descending score.
    /// Scoring: exact key match = 100, key contains = 80, label contains = 50,
    /// description contains = 20.
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let q = query.to_lowercase();
        let mut results: Vec<SearchResult> = Vec::new();

        for entry in &self.entries {
            let key_lower = entry.key.to_lowercase();
            let label_lower = entry.label.to_lowercase();
            let desc_lower = entry.description.to_lowercase();

            if key_lower == q {
                results.push(SearchResult {
                    key: entry.key.clone(),
                    score: 100,
                    match_location: MatchLocation::Key,
                });
            } else if key_lower.contains(&q) {
                results.push(SearchResult {
                    key: entry.key.clone(),
                    score: 80,
                    match_location: MatchLocation::Key,
                });
            } else if label_lower.contains(&q) {
                results.push(SearchResult {
                    key: entry.key.clone(),
                    score: 50,
                    match_location: MatchLocation::Label,
                });
            } else if desc_lower.contains(&q) {
                results.push(SearchResult {
                    key: entry.key.clone(),
                    score: 20,
                    match_location: MatchLocation::Description,
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Display for SettingsSearchIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SearchIndex({} entries)", self.entries.len())
    }
}

// ---------------------------------------------------------------------------
// SettingsModifiedIndicator – tracks settings that differ from defaults
// ---------------------------------------------------------------------------

/// Tracks which settings currently differ from their default values.
#[derive(Debug, Clone, Default)]
pub struct SettingsModifiedIndicator {
    modified: HashMap<String, (String, String)>,
}

impl SettingsModifiedIndicator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `key` has been modified. If `current_value` equals
    /// `default_value` the entry is removed (no longer modified).
    pub fn mark_modified(&mut self, key: &str, current_value: &str, default_value: &str) {
        if current_value == default_value {
            self.modified.remove(key);
        } else {
            self.modified.insert(
                key.to_string(),
                (current_value.to_string(), default_value.to_string()),
            );
        }
    }

    pub fn is_modified(&self, key: &str) -> bool {
        self.modified.contains_key(key)
    }

    pub fn modified_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.modified.keys().map(|k| k.as_str()).collect();
        keys.sort();
        keys
    }

    pub fn modified_count(&self) -> usize {
        self.modified.len()
    }

    pub fn clear(&mut self, key: &str) {
        self.modified.remove(key);
    }
}

impl fmt::Display for SettingsModifiedIndicator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ModifiedIndicator({} modified)", self.modified.len())
    }
}

// ---------------------------------------------------------------------------
// SettingsResetHandler – manages reset-to-default operations
// ---------------------------------------------------------------------------

/// Stores default values and provides reset operations.
#[derive(Debug, Clone, Default)]
pub struct SettingsResetHandler {
    defaults: HashMap<String, String>,
}

impl SettingsResetHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_default(&mut self, key: &str, default_value: &str) {
        self.defaults.insert(key.to_string(), default_value.to_string());
    }

    pub fn get_default(&self, key: &str) -> Option<&str> {
        self.defaults.get(key).map(|s| s.as_str())
    }

    /// Return the default value for `key`, consuming it as an owned `String`.
    pub fn reset(&self, key: &str) -> Option<String> {
        self.defaults.get(key).cloned()
    }

    /// Return all registered defaults as `(key, default_value)` pairs, sorted
    /// by key.
    pub fn reset_all(&self) -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = self
            .defaults
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs
    }

    pub fn has_default(&self, key: &str) -> bool {
        self.defaults.contains_key(key)
    }
}

impl fmt::Display for SettingsResetHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ResetHandler({} defaults)", self.defaults.len())
    }
}

// ---------------------------------------------------------------------------
// SettingsTableOfContents – hierarchical section navigator
// ---------------------------------------------------------------------------

/// A single node in the table-of-contents tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocEntry {
    pub path: String,
    pub label: String,
    pub children: Vec<TocEntry>,
}

impl TocEntry {
    fn new(path: &str, label: &str) -> Self {
        Self {
            path: path.to_string(),
            label: label.to_string(),
            children: Vec::new(),
        }
    }

    fn depth(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
        }
    }
}

impl Default for TocEntry {
    fn default() -> Self {
        Self {
            path: String::new(),
            label: String::new(),
            children: Vec::new(),
        }
    }
}

impl fmt::Display for TocEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.label, self.path)?;
        if !self.children.is_empty() {
            write!(f, " [{} children]", self.children.len())?;
        }
        Ok(())
    }
}

/// Hierarchical section navigator for the settings UI.
#[derive(Debug, Clone, Default)]
pub struct SettingsTableOfContents {
    roots: Vec<TocEntry>,
}

impl SettingsTableOfContents {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a section. The `path` uses dot-separated segments to express
    /// hierarchy (e.g. `"editor.font"` creates an `"editor"` root with a
    /// `"font"` child).
    pub fn add_section(&mut self, path: &str, label: &str) {
        let parts: Vec<&str> = path.split('.').collect();
        Self::insert(&mut self.roots, &parts, 0, path, label);
    }

    fn insert(
        nodes: &mut Vec<TocEntry>,
        parts: &[&str],
        idx: usize,
        full_path: &str,
        label: &str,
    ) {
        if idx >= parts.len() {
            return;
        }

        let segment = parts[idx];
        let partial_path: String = parts[..=idx].join(".");

        let pos = nodes.iter().position(|n| n.path == partial_path);
        let node_idx = match pos {
            Some(i) => i,
            None => {
                let entry_label = if idx == parts.len() - 1 { label } else { segment };
                nodes.push(TocEntry::new(&partial_path, entry_label));
                nodes.len() - 1
            }
        };

        // Update label if this is the final segment.
        if idx == parts.len() - 1 {
            nodes[node_idx].label = label.to_string();
        }

        if idx + 1 < parts.len() {
            Self::insert(&mut nodes[node_idx].children, parts, idx + 1, full_path, label);
        }
    }

    /// Return references to the top-level sections.
    pub fn sections(&self) -> Vec<&TocEntry> {
        self.roots.iter().collect()
    }

    /// Find a section by its full dot-separated path.
    pub fn find_section(&self, path: &str) -> Option<&TocEntry> {
        let parts: Vec<&str> = path.split('.').collect();
        Self::find_in(&self.roots, &parts, 0)
    }

    fn find_in<'a>(nodes: &'a [TocEntry], parts: &[&str], idx: usize) -> Option<&'a TocEntry> {
        if idx >= parts.len() {
            return None;
        }
        let partial: String = parts[..=idx].join(".");
        let node = nodes.iter().find(|n| n.path == partial)?;
        if idx == parts.len() - 1 {
            Some(node)
        } else {
            Self::find_in(&node.children, parts, idx + 1)
        }
    }

    /// Maximum nesting depth (0 when empty).
    pub fn depth(&self) -> usize {
        self.roots.iter().map(|r| r.depth()).max().unwrap_or(0)
    }
}

impl fmt::Display for SettingsTableOfContents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TableOfContents({} roots, depth={})", self.roots.len(), self.depth())
    }
}


// ─── SettB Builder & Validator ─────────────────────────────

/// Builder for constructing settings UI configurations.
#[derive(Debug, Clone)]
pub struct SettBBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl SettBBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<SettBCfg, SettBBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(SettBBuildErr { errors }); }
        Ok(SettBCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated settings UI configuration.
#[derive(Debug, Clone)]
pub struct SettBCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl SettBCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &SettBCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for SettBCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SettBCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct SettBBuildErr { pub errors: Vec<String> }

impl fmt::Display for SettBBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SettBBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for SettBBuildErr {}

// ─── SettF Formatter ───────────────────────────────────────

/// Formatting options for settings UI output.
#[derive(Debug, Clone)]
pub struct SettFFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for SettFFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl SettFFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for settings UI data.
pub struct SettFFmt {
    options: SettFFmtOpts,
}

impl SettFFmt {
    pub fn new(options: SettFFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: SettFFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}


/// Configuration manager for settings_ui functionality.
pub struct SettingsUiConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl SettingsUiConfig {
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

    pub fn merge(&mut self, other: &SettingsUiConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for settings_ui operations.
pub struct SettingsUiRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl SettingsUiRateTracker {
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

/// Validation result collector for settings_ui.
pub struct SettingsUiValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl SettingsUiValidator {
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

    pub fn merge(&mut self, other: &SettingsUiValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Settings UI component tree — extended utilities (yf)
// ---------------------------------------------------------------------------

/// Metric accumulator for settings_ui operations.
#[derive(Debug, Clone)]
pub struct YfMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YfMetrics {
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

/// Sliding-window rate counter for settings_ui.
#[derive(Debug, Clone)]
pub struct YfRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YfRateWindow {
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

/// A small LRU-style cache for settings_ui lookups.
#[derive(Debug, Clone)]
pub struct YfLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YfLruCache {
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
// xa_ extended helpers for settings_ui
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaSettingsUiRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaSettingsUiRingBuf {
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
pub struct XaSettingsUiCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaSettingsUiCounter {
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

impl Default for XaSettingsUiCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 157
// ---------------------------------------------------------------------------

/// Generic object pool `Xc157Pool<T>`.
pub struct Xc157Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc157Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc157PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc157Pool<T> {
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
    pub fn stats(&self) -> Xc157PoolStats {
        Xc157PoolStats {
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

impl<T> Default for Xc157Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc157Scheduler`.
pub struct Xc157Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc157Scheduler {
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

impl Default for Xc157Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_157 hash for the given byte slice.
pub fn xc_157_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_157 convention.
pub fn xc_157_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_77 deepening: state machine + event bus ---

/// States for the Xd77 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd77State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd77State {
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
pub struct Xd77Transition {
    pub from: Xd77State,
    pub to: Xd77State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd77StateMachine {
    current: Xd77State,
    history: Vec<Xd77Transition>,
    step_counter: usize,
}

impl Xd77StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd77State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd77State {
        self.current
    }

    pub fn history(&self) -> &[Xd77Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd77State) -> Result<Xd77State, String> {
        let allowed = match (self.current, target) {
            (Xd77State::Idle, Xd77State::Running) => true,
            (Xd77State::Running, Xd77State::Paused) => true,
            (Xd77State::Running, Xd77State::Done) => true,
            (Xd77State::Paused, Xd77State::Running) => true,
            (Xd77State::Paused, Xd77State::Done) => true,
            (Xd77State::Done, Xd77State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_77: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd77Transition {
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
            "Xd77SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd77State> {
        let prefix = "Xd77SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd77State::Idle),
            "Running" => Some(Xd77State::Running),
            "Paused" => Some(Xd77State::Paused),
            "Done" => Some(Xd77State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd77State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd77 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd77Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd77Event {
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

type Xd77HandlerFn = Box<dyn Fn(&Xd77Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd77EventBus {
    handlers: Vec<(usize, Option<String>, Xd77HandlerFn)>,
    next_id: usize,
    published: Vec<Xd77Event>,
}

impl Xd77EventBus {
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
        F: Fn(&Xd77Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd77Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd77Event) {
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

    pub fn published_events(&self) -> &[Xd77Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #96
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf96Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf96TrieNode {
    children: std::collections::HashMap<char, Xf96TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf96Trie {
    root: Xf96TrieNode,
    count: usize,
}

impl Xf96Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf96TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf96TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf96TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf96BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf96BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 156).
pub struct Xh156SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh156SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 198 as u64,
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

/// A compact bit set supporting boolean operations (variant 156).
pub struct Xh156BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh156BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 156).
pub struct Xi156Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi156Deque<T> {
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
pub struct Xi156Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi156Interval {
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

/// A simple interval tree (variant 156).
pub struct Xi156IntervalTree {
    xi_intervals: Vec<Xi156Interval>,
}

impl Xi156IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi156Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi156Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi156Interval) -> Vec<&Xi156Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi156Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi156Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi156Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi156Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi156Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi156Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 156) ---

/// Disjoint set / union-find for crate 156.
pub struct Xj156UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj156UnionFind {
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

const XJ156_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 156.
pub struct Xj156BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj156BTreeNode<K, V>>>,
    len: usize,
}

struct Xj156BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj156BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj156BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ156_BTREE_ORDER - 1
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
        let mid = XJ156_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj156BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj156BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj156BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj156BTreeNode::xj_new_leaf();
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


// --- xk_156 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk156SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk156SegmentTree {
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
pub struct Xk156DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk156DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_156).
#[derive(Debug, Clone)]
pub struct Xl156Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl156Rope {
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

/// Suffix array for efficient string searching (xl_156).
#[derive(Debug, Clone)]
pub struct Xl156SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl156SuffixArray {
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
pub struct Xm156MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm156MatrixSparse {
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
pub struct Xm156Tokenizer {
    text: String,
}

impl Xm156Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 156.
pub struct Xn156Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn156Fenwick {
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

// ----- AVL tree map — crate 156 -----

#[derive(Debug, Clone)]
struct Xn156AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn156AvlNode<K, V>>>,
    right: Option<Box<Xn156AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 156.
#[derive(Debug, Clone)]
pub struct Xn156AVL<K, V> {
    root: Option<Box<Xn156AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn156AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn156AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn156AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn156AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn156AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn156AvlNode<K, V>>) -> Box<Xn156AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn156AvlNode<K, V>>) -> Box<Xn156AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn156AvlNode<K, V>>) -> Box<Xn156AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn156AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn156AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn156AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn156AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn156AvlNode<K, V>>) -> &Xn156AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn156AvlNode<K, V>>) -> (Box<Xn156AvlNode<K, V>>, Option<Box<Xn156AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn156AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn156AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn156AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn156AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn156AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn156AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn156AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo156RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo156Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo156RBNode<K, V> {
    key: K,
    value: V,
    color: Xo156Color,
    left: Option<Box<Xo156RBNode<K, V>>>,
    right: Option<Box<Xo156RBNode<K, V>>>,
}

/// A red-black tree map for crate 156.
#[derive(Debug, Clone)]
pub struct Xo156RedBlack<K, V> {
    root: Option<Box<Xo156RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo156RedBlack<K, V> {
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
            r.color = Xo156Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo156RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo156RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo156RBNode {
                    key, value, color: Xo156Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo156RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo156Color::Red)
    }

    fn xo_balance(mut h: Box<Xo156RBNode<K, V>>) -> Box<Xo156RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo156Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo156RBNode<K, V>>) -> Box<Xo156RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo156Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo156RBNode<K, V>>) -> Box<Xo156RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo156Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo156RBNode<K, V>>) {
        h.color = Xo156Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo156Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo156Color::Black; }
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
            r.color = Xo156Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo156RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo156RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo156RBNode<K, V>) -> (K, V, Option<Box<Xo156RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo156RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo156Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo156RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo156ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 156.
#[derive(Debug, Clone)]
pub struct Xo156ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo156ConsistentHash {
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
            let vkey = format!("{}#xo156#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo156#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 156).
#[derive(Debug)]
pub struct Xp156SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp156Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp156Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp156Node<K, V>>>,
    xp_right: Option<Box<Xp156Node<K, V>>>,
}

impl<K: Ord, V> Xp156Node<K, V> {
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

impl<K: Ord, V> Default for Xp156SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp156SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp156Node<K, V>>>, key: &K) -> Option<Box<Xp156Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp156Node<K, V>>) -> Box<Xp156Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp156Node<K, V>>) -> Box<Xp156Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp156Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp156Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp156Node::xp_new(key, val));
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


// --------------- Xq156Treap ---------------

use std::cmp::Ordering as Xq156Ord;

struct Xq156TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq156TreapNode<K, V>>>,
    right: Option<Box<Xq156TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq156Treap<K, V> {
    root: Option<Box<Xq156TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq156TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_156_size<K, V>(node: &Option<Box<Xq156TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_156_update_size<K, V>(node: &mut Xq156TreapNode<K, V>) {
    node.size = 1 + xq_156_size(&node.left) + xq_156_size(&node.right);
}

fn xq_156_rotate_right<K, V>(mut node: Box<Xq156TreapNode<K, V>>) -> Box<Xq156TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_156_update_size(&mut node);
    left.right = Some(node);
    xq_156_update_size(&mut left);
    left
}

fn xq_156_rotate_left<K, V>(mut node: Box<Xq156TreapNode<K, V>>) -> Box<Xq156TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_156_update_size(&mut node);
    right.left = Some(node);
    xq_156_update_size(&mut right);
    right
}

fn xq_156_insert_node<K: Ord, V>(
    node: Option<Box<Xq156TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq156TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq156TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq156Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq156Ord::Less => {
                let (new_left, old) = xq_156_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_156_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_156_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq156Ord::Greater => {
                let (new_right, old) = xq_156_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_156_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_156_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_156_remove_node<K: Ord, V>(
    node: Option<Box<Xq156TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq156TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq156Ord::Less => {
                let (new_left, old) = xq_156_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_156_update_size(&mut n);
                (Some(n), old)
            }
            Xq156Ord::Greater => {
                let (new_right, old) = xq_156_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_156_update_size(&mut n);
                (Some(n), old)
            }
            Xq156Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_156_rotate_right(n);
                    let (new_right, old) = xq_156_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_156_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_156_rotate_left(n);
                    let (new_left, old) = xq_156_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_156_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_156_find_min<K, V>(node: &Option<Box<Xq156TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_156_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_156_find_max<K, V>(node: &Option<Box<Xq156TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_156_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_156_rank<K: Ord, V>(node: &Option<Box<Xq156TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq156Ord::Less => xq_156_rank(&n.left, key),
            Xq156Ord::Equal => xq_156_size(&n.left),
            Xq156Ord::Greater => 1 + xq_156_size(&n.left) + xq_156_rank(&n.right, key),
        },
    }
}

fn xq_156_kth<K, V>(node: &Option<Box<Xq156TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_156_size(&n.left);
        if k < left_size {
            xq_156_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_156_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_156_in_order<K: Clone, V>(node: &Option<Box<Xq156TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_156_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_156_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq156Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 156 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_156_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq156Ord::Equal => return Some(&n.value),
                Xq156Ord::Less => cur = &n.left,
                Xq156Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_156_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_156_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_156_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_156_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_156_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_156_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_156_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq156VEBTree ---------------

pub struct Xq156VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq156VEBTree>>,
    clusters: Vec<Option<Box<Xq156VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq156VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq156VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq156VEBTree::xq_new(self.sqrt_lo)));
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


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr156KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr156KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr156BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr156KDNode {
    xr_point: Xr156KDPoint,
    xr_left: Option<Box<Xr156KDNode>>,
    xr_right: Option<Box<Xr156KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr156KDTree {
    xr_root: Option<Box<Xr156KDNode>>,
    xr_size: usize,
}

impl Xr156KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr156KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr156KDNode>>,
        point: Xr156KDPoint,
        depth: usize,
    ) -> Box<Xr156KDNode> {
        match node {
            None => Box::new(Xr156KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr156KDPoint) -> Option<Xr156KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr156KDNode>,
        query: &Xr156KDPoint,
        depth: usize,
        best: &mut Xr156KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr156KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr156KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr156KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr156KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr156KDNode>>, pts: &mut Vec<Xr156KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr156KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr156BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr156BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs156PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs156PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs156PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs156PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs156ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs156ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs156ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs156RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs156RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs156RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs156CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs156CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs156CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_settings() -> Vec<SettingItem> {
        vec![
            SettingItem {
                key: "editor.fontSize".into(),
                label: "Font Size".into(),
                description: "Controls the font size in pixels".into(),
                setting_type: SettingType::Number,
                default_value: "14".into(),
                current_value: Some("16".into()),
                enum_values: vec![],
                scope: SettingScope::User,
            },
            SettingItem {
                key: "editor.tabSize".into(),
                label: "Tab Size".into(),
                description: "The number of spaces a tab equals".into(),
                setting_type: SettingType::Number,
                default_value: "4".into(),
                current_value: None,
                enum_values: vec![],
                scope: SettingScope::Workspace,
            },
            SettingItem {
                key: "files.autoSave".into(),
                label: "Auto Save".into(),
                description: "Controls auto save of editors".into(),
                setting_type: SettingType::Enum,
                default_value: "off".into(),
                current_value: Some("afterDelay".into()),
                enum_values: vec!["off".into(), "afterDelay".into(), "onWindowChange".into()],
                scope: SettingScope::User,
            },
        ]
    }

    #[test]
    fn filter_by_query() {
        let settings = sample_settings();
        let filter = SettingsFilter { query: "font".into(), scope: None, modified_only: false };
        let result = filter_settings(&settings, &filter);
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn filter_modified_only() {
        let settings = sample_settings();
        let filter = SettingsFilter { query: String::new(), scope: None, modified_only: true };
        let result = filter_settings(&settings, &filter);
        assert_eq!(result, vec![0, 2]);
    }

    #[test]
    fn filter_by_scope() {
        let settings = sample_settings();
        let filter = SettingsFilter {
            query: String::new(),
            scope: Some(SettingScope::Workspace),
            modified_only: false,
        };
        let result = filter_settings(&settings, &filter);
        assert_eq!(result, vec![1]);
    }

    // --- new tests ---

    #[test]
    fn setting_type_display() {
        assert_eq!(SettingType::String.to_string(), "string");
        assert_eq!(SettingType::Number.to_string(), "number");
        assert_eq!(SettingType::Boolean.to_string(), "boolean");
        assert_eq!(SettingType::Object.to_string(), "object");
    }

    #[test]
    fn setting_scope_display() {
        assert_eq!(SettingScope::User.to_string(), "User");
        assert_eq!(SettingScope::WorkspaceFolder.to_string(), "Workspace Folder");
        assert_eq!(SettingScope::Language.to_string(), "Language");
    }

    #[test]
    fn settings_error_display() {
        let e = SettingsError::SettingNotFound("foo".into());
        assert_eq!(e.to_string(), "setting not found: foo");
        let e = SettingsError::InvalidValue("bad".into());
        assert_eq!(e.to_string(), "invalid value: bad");
        let e = SettingsError::ReadOnly("bar".into());
        assert_eq!(e.to_string(), "setting is read-only: bar");
    }

    #[test]
    fn is_modified_and_reset() {
        let mut item = sample_settings().remove(0);
        assert!(item.is_modified());
        item.reset();
        assert!(!item.is_modified());
        assert!(item.current_value.is_none());
    }

    #[test]
    fn set_value_valid_enum() {
        let mut item = sample_settings().remove(2);
        assert!(item.set_value("onWindowChange".into()).is_ok());
        assert_eq!(item.current_value.as_deref(), Some("onWindowChange"));
    }

    #[test]
    fn set_value_invalid_enum() {
        let mut item = sample_settings().remove(2);
        let err = item.set_value("never".into()).unwrap_err();
        assert!(matches!(err, SettingsError::InvalidValue(_)));
    }

    #[test]
    fn set_value_invalid_number() {
        let mut item = sample_settings().remove(0);
        let err = item.set_value("abc".into()).unwrap_err();
        assert!(matches!(err, SettingsError::InvalidValue(_)));
    }

    #[test]
    fn set_value_boolean_validation() {
        let mut item = SettingItemBuilder::new("x", SettingType::Boolean)
            .default_value("true")
            .build();
        assert!(item.set_value("false".into()).is_ok());
        assert!(item.set_value("yes".into()).is_err());
    }

    #[test]
    fn builder_defaults() {
        let item = SettingItemBuilder::new("editor.wrap", SettingType::Boolean)
            .label("Word Wrap")
            .description("Toggle word wrap")
            .default_value("false")
            .scope(SettingScope::Workspace)
            .build();
        assert_eq!(item.key, "editor.wrap");
        assert_eq!(item.label, "Word Wrap");
        assert_eq!(item.scope, SettingScope::Workspace);
        assert!(item.current_value.is_none());
    }

    #[test]
    fn registry_add_get_set_reset() {
        let mut reg = SettingsRegistry::new();
        reg.add(sample_settings().remove(0));
        assert!(reg.get("editor.fontSize").is_ok());
        assert!(reg.get("missing").is_err());

        reg.set("editor.fontSize", "20".into()).unwrap();
        assert_eq!(reg.get("editor.fontSize").unwrap().current_value.as_deref(), Some("20"));

        reg.reset("editor.fontSize").unwrap();
        assert!(reg.get("editor.fontSize").unwrap().current_value.is_none());
    }

    #[test]
    fn registry_list_modified() {
        let mut reg = SettingsRegistry::new();
        for item in sample_settings() {
            reg.add(item);
        }
        let modified = reg.list_modified();
        assert_eq!(modified, vec!["editor.fontSize", "files.autoSave"]);
    }

    #[test]
    fn settings_filter_empty() {
        let f = SettingsFilter::empty();
        assert!(f.query.is_empty());
        assert!(f.scope.is_none());
        assert!(!f.modified_only);
        let settings = sample_settings();
        assert_eq!(filter_settings(&settings, &f).len(), 3);
    }

    #[test]
    fn group_by_scope() {
        let settings = sample_settings();
        let groups = group_settings_by_scope(&settings);
        assert_eq!(groups[&SettingScope::User], vec![0, 2]);
        assert_eq!(groups[&SettingScope::Workspace], vec![1]);
        assert!(!groups.contains_key(&SettingScope::Language));
    }

    // --- change log tests ---

    #[test]
    fn change_log_record_and_undo() {
        let mut log = SettingsChangeLog::new();
        log.record(SettingChange {
            key: "editor.fontSize".into(),
            old_value: Some("14".into()),
            new_value: Some("16".into()),
            timestamp_ms: 1000,
        });
        log.record(SettingChange {
            key: "editor.tabSize".into(),
            old_value: None,
            new_value: Some("2".into()),
            timestamp_ms: 2000,
        });
        assert_eq!(log.get_recent(10).len(), 2);
        let undone = log.undo_last().unwrap();
        assert_eq!(undone.key, "editor.tabSize");
        assert_eq!(log.get_recent(10).len(), 1);
    }

    #[test]
    fn change_log_get_changes_for_key() {
        let mut log = SettingsChangeLog::new();
        log.record(SettingChange {
            key: "a.b".into(),
            old_value: None,
            new_value: Some("1".into()),
            timestamp_ms: 100,
        });
        log.record(SettingChange {
            key: "c.d".into(),
            old_value: None,
            new_value: Some("2".into()),
            timestamp_ms: 200,
        });
        log.record(SettingChange {
            key: "a.b".into(),
            old_value: Some("1".into()),
            new_value: Some("3".into()),
            timestamp_ms: 300,
        });
        assert_eq!(log.get_changes_for_key("a.b").len(), 2);
        assert_eq!(log.get_changes_for_key("c.d").len(), 1);
        assert_eq!(log.get_changes_for_key("x.y").len(), 0);
    }

    #[test]
    fn change_log_clear() {
        let mut log = SettingsChangeLog::new();
        log.record(SettingChange {
            key: "a.b".into(),
            old_value: None,
            new_value: Some("1".into()),
            timestamp_ms: 0,
        });
        log.clear();
        assert_eq!(log.get_recent(10).len(), 0);
        assert!(log.undo_last().is_none());
    }

    // --- validate_setting_key tests ---

    #[test]
    fn validate_key_valid() {
        assert!(validate_setting_key("editor.fontSize").is_ok());
        assert!(validate_setting_key("files.auto_save.delay").is_ok());
        assert!(validate_setting_key("a.b").is_ok());
    }

    #[test]
    fn validate_key_invalid() {
        assert!(validate_setting_key("").is_err());
        assert!(validate_setting_key("nosegment").is_err());
        assert!(validate_setting_key("a..b").is_err());
        assert!(validate_setting_key(".a.b").is_err());
        assert!(validate_setting_key("a.b-c").is_err());
        assert!(validate_setting_key("a.b ").is_err());
    }

    // --- exporter tests ---

    #[test]
    fn exporter_roundtrip() {
        let mut reg = SettingsRegistry::new();
        reg.add(
            SettingItemBuilder::new("editor.fontSize", SettingType::Number)
                .default_value("14")
                .current_value("16")
                .build(),
        );
        reg.add(
            SettingItemBuilder::new("editor.tabSize", SettingType::Number)
                .default_value("4")
                .build(),
        );
        let json = SettingsExporter::to_json(&reg);
        let pairs = SettingsExporter::from_json(&json).unwrap();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("editor.fontSize".into(), "16".into()));
        assert_eq!(pairs[1], ("editor.tabSize".into(), "4".into()));
    }

    #[test]
    fn exporter_invalid_json() {
        assert!(SettingsExporter::from_json("not json").is_err());
        assert!(SettingsExporter::from_json("{ bad }").is_err());
    }

    // --- diff tests ---

    #[test]
    fn diff_identical_registries() {
        let mut a = SettingsRegistry::new();
        a.add(
            SettingItemBuilder::new("x.y", SettingType::String)
                .default_value("hello")
                .build(),
        );
        let mut b = SettingsRegistry::new();
        b.add(
            SettingItemBuilder::new("x.y", SettingType::String)
                .default_value("hello")
                .build(),
        );
        let d = diff_settings(&a, &b);
        assert!(d.only_in_a.is_empty());
        assert!(d.only_in_b.is_empty());
        assert!(d.changed.is_empty());
    }

    #[test]
    fn diff_detects_changes_and_unique_keys() {
        let mut a = SettingsRegistry::new();
        a.add(
            SettingItemBuilder::new("shared.key", SettingType::String)
                .default_value("aaa")
                .build(),
        );
        a.add(
            SettingItemBuilder::new("only.a", SettingType::String)
                .default_value("x")
                .build(),
        );

        let mut b = SettingsRegistry::new();
        b.add(
            SettingItemBuilder::new("shared.key", SettingType::String)
                .default_value("bbb")
                .build(),
        );
        b.add(
            SettingItemBuilder::new("only.b", SettingType::String)
                .default_value("y")
                .build(),
        );

        let d = diff_settings(&a, &b);
        assert_eq!(d.only_in_a, vec!["only.a"]);
        assert_eq!(d.only_in_b, vec!["only.b"]);
        assert_eq!(d.changed.len(), 1);
        assert_eq!(d.changed[0], ("shared.key".into(), "aaa".into(), "bbb".into()));
    }

    #[test]
    fn filtered_exporter_empty_registry() {
        let reg = SettingsRegistry::new();
        let exp = SettingsFilteredExporter::new(&reg);
        assert_eq!(exp.to_json_string(), "{\n}");
    }

    #[test]
    fn filtered_exporter_uses_current_value() {
        let mut reg = SettingsRegistry::new();
        reg.add(SettingItem {
            key: "a.b".into(),
            label: "AB".into(),
            description: String::new(),
            setting_type: SettingType::String,
            default_value: "default".into(),
            current_value: Some("custom".into()),
            enum_values: vec![],
            scope: SettingScope::User,
        });
        let json = SettingsFilteredExporter::new(&reg).to_json_string();
        assert!(json.contains("\"a.b\": \"custom\""));
    }

    #[test]
    fn filtered_exporter_with_prefix() {
        let mut reg = SettingsRegistry::new();
        reg.add(SettingItem {
            key: "editor.fontSize".into(),
            label: "Font Size".into(),
            description: String::new(),
            setting_type: SettingType::Number,
            default_value: "14".into(),
            current_value: None,
            enum_values: vec![],
            scope: SettingScope::User,
        });
        reg.add(SettingItem {
            key: "terminal.shell".into(),
            label: "Shell".into(),
            description: String::new(),
            setting_type: SettingType::String,
            default_value: "/bin/bash".into(),
            current_value: None,
            enum_values: vec![],
            scope: SettingScope::User,
        });
        let json = SettingsFilteredExporter::new(&reg)
            .with_prefix("editor.")
            .to_json_string();
        assert!(json.contains("editor.fontSize"));
        assert!(!json.contains("terminal.shell"));
    }

    #[test]
    fn setting_control_checkbox() {
        let item = SettingItemBuilder::new("editor.wordWrap", SettingType::Boolean)
            .default_value("false")
            .current_value("true")
            .build();
        let ctrl = SettingControl::from_setting(&item);
        assert!(matches!(ctrl, SettingControl::Checkbox { checked: true }));
    }

    #[test]
    fn setting_control_dropdown() {
        let item = SettingItemBuilder::new("editor.tabSize", SettingType::Enum)
            .default_value("4")
            .enum_values(vec!["2".into(), "4".into(), "8".into()])
            .build();
        let ctrl = SettingControl::from_setting(&item);
        match ctrl {
            SettingControl::Dropdown { options, selected } => {
                assert_eq!(options.len(), 3);
                assert_eq!(selected, Some(1));
            }
            _ => panic!("expected dropdown"),
        }
    }

    #[test]
    fn setting_control_number() {
        let item = SettingItemBuilder::new("editor.fontSize", SettingType::Number)
            .default_value("14")
            .build();
        let ctrl = SettingControl::from_setting(&item);
        assert!(
            matches!(ctrl, SettingControl::NumberInput { value, .. } if (value - 14.0).abs() < f64::EPSILON)
        );
    }

    #[test]
    fn setting_control_text() {
        let item = SettingItemBuilder::new("editor.fontFamily", SettingType::String)
            .default_value("Consolas")
            .build();
        let ctrl = SettingControl::from_setting(&item);
        assert!(matches!(ctrl, SettingControl::TextInput { .. }));
    }

    #[test]
    fn setting_validation_range_ok() {
        let v = SettingValidation::Range {
            min: 8.0,
            max: 72.0,
        };
        assert!(v.validate("14").is_ok());
        assert!(v.validate("4").is_err());
        assert!(v.validate("100").is_err());
    }

    #[test]
    fn setting_validation_length() {
        let v = SettingValidation::Length {
            min_len: 1,
            max_len: 10,
        };
        assert!(v.validate("hello").is_ok());
        assert!(v.validate("").is_err());
        assert!(v.validate("a very long string").is_err());
    }

    #[test]
    fn setting_validation_one_of() {
        let v = SettingValidation::OneOf(vec!["on".into(), "off".into(), "auto".into()]);
        assert!(v.validate("on").is_ok());
        assert!(v.validate("maybe").is_err());
    }

    #[test]
    fn settings_to_json_patch_only_modified() {
        let mut registry = SettingsRegistry::new();
        registry.add(
            SettingItemBuilder::new("a.b", SettingType::String)
                .default_value("default")
                .current_value("changed")
                .build(),
        );
        registry.add(
            SettingItemBuilder::new("c.d", SettingType::String)
                .default_value("same")
                .build(),
        );
        let patch = settings_to_json_patch(&registry);
        assert!(patch.contains("a.b"));
        assert!(!patch.contains("c.d"));
        assert_eq!(modified_settings_count(&registry), 1);
    }

    #[test]
    fn test_setting_type_all() {
        assert_eq!(SettingType::all().len(), 6);
    }

    #[test]
    fn test_setting_type_is_scalar() {
        assert!(SettingType::String.is_scalar());
        assert!(SettingType::Number.is_scalar());
        assert!(SettingType::Boolean.is_scalar());
        assert!(!SettingType::Array.is_scalar());
        assert!(!SettingType::Object.is_scalar());
    }

    #[test]
    fn test_setting_type_default_value_str() {
        assert_eq!(SettingType::Boolean.default_value_str(), "false");
        assert_eq!(SettingType::Array.default_value_str(), "[]");
    }

    #[test]
    fn test_setting_scope_all() {
        assert_eq!(SettingScope::all().len(), 4);
    }

    #[test]
    fn test_setting_scope_from_str_opt() {
        assert_eq!(SettingScope::from_str_opt("user"), Some(SettingScope::User));
        assert_eq!(SettingScope::from_str_opt("folder"), Some(SettingScope::WorkspaceFolder));
        assert_eq!(SettingScope::from_str_opt("bogus"), None);
    }

    #[test]
    fn test_setting_scope_priority() {
        assert!(SettingScope::Language.priority() > SettingScope::User.priority());
    }

    #[test]
    fn test_split_setting_key() {
        assert_eq!(split_setting_key("editor.fontSize"), vec!["editor", "fontSize"]);
        assert_eq!(split_setting_key("simple"), vec!["simple"]);
    }

    #[test]
    fn test_setting_category_fn() {
        assert_eq!(setting_category("editor.fontSize"), "editor");
        assert_eq!(setting_category("standalone"), "standalone");
    }

    #[test]
    fn test_setting_prefix_iter() {
        let items = vec![
            SettingItem { key: "editor.fontSize".into(), label: "".into(), setting_type: SettingType::Number, scope: SettingScope::User, description: "".into(), default_value: "14".into(), current_value: None, enum_values: vec![] },
            SettingItem { key: "terminal.shell".into(), label: "".into(), setting_type: SettingType::String, scope: SettingScope::User, description: "".into(), default_value: "bash".into(), current_value: None, enum_values: vec![] },
            SettingItem { key: "editor.tabSize".into(), label: "".into(), setting_type: SettingType::Number, scope: SettingScope::User, description: "".into(), default_value: "4".into(), current_value: None, enum_values: vec![] },
        ];
        let editor_items: Vec<_> = SettingPrefixIter::new(&items, "editor.").collect();
        assert_eq!(editor_items.len(), 2);
    }

    // --- SettingValidator tests ---

    #[test]
    fn validator_string_length_rule() {
        let mut v = SettingValidator::new();
        v.add_rule("editor.fontFamily", ValidationRule {
            description: "font family length".into(),
            kind: ValidationRuleKind::StringLength { min: 1, max: 50 },
        });
        assert!(v.is_valid("editor.fontFamily", "Consolas"));
        assert!(!v.is_valid("editor.fontFamily", ""));
        assert_eq!(v.rule_count("editor.fontFamily"), 1);
        assert_eq!(v.rule_count("unknown.key"), 0);
    }

    #[test]
    fn validator_number_range_rule() {
        let mut v = SettingValidator::new();
        v.add_rule("editor.fontSize", ValidationRule {
            description: "font size range".into(),
            kind: ValidationRuleKind::NumberRange { min: 8.0, max: 72.0 },
        });
        assert!(v.is_valid("editor.fontSize", "14"));
        assert!(!v.is_valid("editor.fontSize", "4"));
        assert!(!v.is_valid("editor.fontSize", "100"));
        assert!(!v.is_valid("editor.fontSize", "abc"));
    }

    #[test]
    fn validator_enum_membership_rule() {
        let mut v = SettingValidator::new();
        v.add_rule("editor.cursorStyle", ValidationRule {
            description: "cursor style".into(),
            kind: ValidationRuleKind::EnumMembership(vec![
                "line".into(), "block".into(), "underline".into(),
            ]),
        });
        assert!(v.is_valid("editor.cursorStyle", "block"));
        assert!(!v.is_valid("editor.cursorStyle", "triangle"));
    }

    #[test]
    fn validator_regex_pattern_rule() {
        let mut v = SettingValidator::new();
        v.add_rule("files.exclude", ValidationRule {
            description: "must contain glob star".into(),
            kind: ValidationRuleKind::RegexPattern("*".into()),
        });
        assert!(v.is_valid("files.exclude", "*.log"));
        assert!(!v.is_valid("files.exclude", "readme"));
    }

    #[test]
    fn validator_multiple_rules_per_key() {
        let mut v = SettingValidator::new();
        v.add_rule("editor.fontFamily", ValidationRule {
            description: "min length".into(),
            kind: ValidationRuleKind::StringLength { min: 1, max: 100 },
        });
        v.add_rule("editor.fontFamily", ValidationRule {
            description: "must contain a".into(),
            kind: ValidationRuleKind::RegexPattern("a".into()),
        });
        assert!(v.is_valid("editor.fontFamily", "Cascadia"));
        // Fails second rule (no 'a')
        let errors = v.validate("editor.fontFamily", "Consol");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("must contain a"));
        // Fails first rule (empty)
        let errors = v.validate("editor.fontFamily", "");
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn validator_keys_with_rules() {
        let mut v = SettingValidator::new();
        v.add_rule("b.key", ValidationRule {
            description: "r".into(),
            kind: ValidationRuleKind::StringLength { min: 0, max: 10 },
        });
        v.add_rule("a.key", ValidationRule {
            description: "r".into(),
            kind: ValidationRuleKind::StringLength { min: 0, max: 10 },
        });
        let keys = v.keys_with_rules();
        assert_eq!(keys, vec!["a.key", "b.key"]);
    }

    // --- SettingsDiff Display / helpers ---

    #[test]
    fn settings_diff_display_empty() {
        let d = SettingsDiff::default();
        assert!(d.is_empty());
        assert_eq!(d.total(), 0);
        assert_eq!(format!("{d}"), "no differences");
    }

    #[test]
    fn settings_diff_display_with_changes() {
        let d = SettingsDiff {
            only_in_a: vec!["removed.key".into()],
            only_in_b: vec!["added.key".into()],
            changed: vec![("changed.key".into(), "old".into(), "new".into())],
        };
        assert!(!d.is_empty());
        assert_eq!(d.total(), 3);
        let text = format!("{d}");
        assert!(text.contains("- removed.key"));
        assert!(text.contains("+ added.key"));
        assert!(text.contains("~ changed.key: old -> new"));
    }

    // --- SettingsTextExporter tests ---

    #[test]
    fn text_exporter_roundtrip() {
        let mut reg = SettingsRegistry::new();
        reg.add(
            SettingItemBuilder::new("editor.fontSize", SettingType::Number)
                .default_value("14")
                .current_value("16")
                .build(),
        );
        reg.add(
            SettingItemBuilder::new("editor.tabSize", SettingType::Number)
                .default_value("4")
                .build(),
        );
        let text = SettingsTextExporter::to_text(&reg);
        assert!(text.contains("editor.fontSize=16\n"));
        assert!(text.contains("editor.tabSize=4\n"));

        let pairs = SettingsTextExporter::from_text(&text).unwrap();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("editor.fontSize".into(), "16".into()));
        assert_eq!(pairs[1], ("editor.tabSize".into(), "4".into()));
    }

    #[test]
    fn text_exporter_comments_and_blanks() {
        let text = "# this is a comment\n\neditor.fontSize=14\n# another\nterminal.shell=/bin/zsh\n";
        let pairs = SettingsTextExporter::from_text(text).unwrap();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0, "editor.fontSize");
        assert_eq!(pairs[1].0, "terminal.shell");
    }

    #[test]
    fn text_exporter_escaping() {
        let mut reg = SettingsRegistry::new();
        reg.add(
            SettingItemBuilder::new("msg.greeting", SettingType::String)
                .default_value("hello\nworld")
                .build(),
        );
        let text = SettingsTextExporter::to_text(&reg);
        assert!(text.contains("msg.greeting=hello\\nworld"));
        let pairs = SettingsTextExporter::from_text(&text).unwrap();
        assert_eq!(pairs[0].1, "hello\nworld");
    }

    #[test]
    fn text_exporter_missing_equals() {
        let result = SettingsTextExporter::from_text("no_equals_here\n");
        assert!(result.is_err());
    }

    #[test]
    fn text_exporter_apply_to_registry() {
        let mut reg = SettingsRegistry::new();
        reg.add(
            SettingItemBuilder::new("editor.fontSize", SettingType::Number)
                .default_value("14")
                .build(),
        );
        let pairs = vec![
            ("editor.fontSize".to_string(), "20".to_string()),
            ("missing.key".to_string(), "val".to_string()),
        ];
        let errors = SettingsTextExporter::apply_to_registry(&mut reg, &pairs);
        assert_eq!(errors.len(), 1); // missing.key not found
        assert_eq!(reg.effective_value("editor.fontSize").unwrap(), "20");
    }

    // --- SettingSearchIndex tests ---

    #[test]
    fn search_index_basic() {
        let settings = sample_settings();
        let index = SettingSearchIndex::build(&settings);
        assert_eq!(index.len(), 3);
        assert!(!index.is_empty());
        assert_eq!(format!("{index}"), "SettingSearchIndex(3 entries)");
    }

    #[test]
    fn search_index_finds_by_key_segment() {
        let settings = sample_settings();
        let index = SettingSearchIndex::build(&settings);
        let results = index.search("editor");
        // All three match: fontSize/tabSize have "editor" in key, autoSave has "editors" in description
        assert_eq!(results, vec![0, 1, 2]);
    }

    #[test]
    fn search_index_finds_by_label() {
        let settings = sample_settings();
        let index = SettingSearchIndex::build(&settings);
        let results = index.search("Font");
        assert_eq!(results, vec![0]);
    }

    #[test]
    fn search_index_finds_by_description_word() {
        let settings = sample_settings();
        let index = SettingSearchIndex::build(&settings);
        let results = index.search("pixels");
        assert_eq!(results, vec![0]);
    }

    #[test]
    fn search_index_multi_word_query() {
        let settings = sample_settings();
        let index = SettingSearchIndex::build(&settings);
        // Must match all words
        let results = index.search("editor font");
        assert_eq!(results, vec![0]); // only fontSize matches both
    }

    #[test]
    fn search_index_empty_query_returns_all() {
        let settings = sample_settings();
        let index = SettingSearchIndex::build(&settings);
        let results = index.search("");
        assert_eq!(results.len(), 3);
    }

    // --- From impls tests ---

    #[test]
    fn settings_error_from_str() {
        let e: SettingsError = "bad value".into();
        assert_eq!(e, SettingsError::InvalidValue("bad value".into()));
    }

    #[test]
    fn settings_error_from_string() {
        let e: SettingsError = String::from("oops").into();
        assert_eq!(e, SettingsError::InvalidValue("oops".into()));
    }

    #[test]
    fn setting_change_from_item() {
        let item = SettingItemBuilder::new("editor.fontSize", SettingType::Number)
            .default_value("14")
            .current_value("16")
            .build();
        let change: SettingChange = item.into();
        assert_eq!(change.key, "editor.fontSize");
        assert_eq!(change.new_value, Some("16".into()));
        assert!(change.old_value.is_none());
    }

    // --- Registry merge / helpers ---

    #[test]
    fn registry_merge() {
        let mut a = SettingsRegistry::new();
        a.add(SettingItemBuilder::new("editor.fontSize", SettingType::Number)
            .default_value("14").build());
        a.add(SettingItemBuilder::new("editor.tabSize", SettingType::Number)
            .default_value("4").build());

        let mut b = SettingsRegistry::new();
        b.add(SettingItemBuilder::new("editor.fontSize", SettingType::Number)
            .default_value("14").current_value("20").build());
        b.add(SettingItemBuilder::new("terminal.shell", SettingType::String)
            .default_value("/bin/bash").build());

        a.merge(&b);
        assert_eq!(a.effective_value("editor.fontSize").unwrap(), "20");
        assert_eq!(a.effective_value("editor.tabSize").unwrap(), "4");
        assert_eq!(a.effective_value("terminal.shell").unwrap(), "/bin/bash");
    }

    #[test]
    fn registry_keys_sorted() {
        let mut reg = SettingsRegistry::new();
        reg.add(SettingItemBuilder::new("z.key", SettingType::String).default_value("").build());
        reg.add(SettingItemBuilder::new("a.key", SettingType::String).default_value("").build());
        reg.add(SettingItemBuilder::new("m.key", SettingType::String).default_value("").build());
        assert_eq!(reg.keys(), vec!["a.key", "m.key", "z.key"]);
    }

    // --- ValidationRuleKind Display ---

    #[test]
    fn validation_rule_kind_display() {
        let k = ValidationRuleKind::StringLength { min: 1, max: 50 };
        assert_eq!(format!("{k}"), "string length [1, 50]");
        let k = ValidationRuleKind::NumberRange { min: 0.0, max: 100.0 };
        assert_eq!(format!("{k}"), "number range [0, 100]");
        let k = ValidationRuleKind::EnumMembership(vec!["a".into(), "b".into()]);
        assert!(format!("{k}").contains("one of"));
        let k = ValidationRuleKind::RegexPattern("test".into());
        assert!(format!("{k}").contains("test"));
    }

    // --- SettingsSearchIndex ---

    #[test]
    fn search_index_empty() {
        let idx = SettingsSearchIndex::new();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
        assert!(idx.search("anything").is_empty());
        assert_eq!(format!("{idx}"), "SearchIndex(0 entries)");
    }

    #[test]
    fn search_index_exact_key_scores_highest() {
        let mut idx = SettingsSearchIndex::new();
        idx.add_entry("editor.fontSize", "Font Size", "Controls the font size");
        idx.add_entry("editor.fontFamily", "Font Family", "Controls the font family");
        idx.add_entry("terminal.fontSize", "Terminal Font", "fontSize for terminal");

        let results = idx.search("editor.fontSize");
        assert_eq!(results[0].key, "editor.fontSize");
        assert_eq!(results[0].score, 100);
        assert_eq!(results[0].match_location, MatchLocation::Key);
    }

    #[test]
    fn search_index_scoring_order() {
        let mut idx = SettingsSearchIndex::new();
        idx.add_entry("theme.color", "Theme Color", "Pick a color theme");
        idx.add_entry("editor.tab", "Color Scheme", "Set colors");
        idx.add_entry("misc.opt", "Option", "Some color option");

        let results = idx.search("color");
        assert!(results.len() >= 3);
        assert!(results[0].score >= results[1].score);
        assert!(results[1].score >= results[2].score);
    }

    #[test]
    fn search_result_display() {
        let r = SearchResult { key: "k".into(), score: 50, match_location: MatchLocation::Label };
        assert!(format!("{r}").contains("label"));
    }

    // --- SettingsModifiedIndicator ---

    #[test]
    fn modified_indicator_basic() {
        let mut ind = SettingsModifiedIndicator::new();
        assert_eq!(ind.modified_count(), 0);
        ind.mark_modified("a", "1", "2");
        assert!(ind.is_modified("a"));
        assert_eq!(ind.modified_count(), 1);
        assert_eq!(ind.modified_keys(), vec!["a"]);
        ind.clear("a");
        assert!(!ind.is_modified("a"));
        assert_eq!(ind.modified_count(), 0);
    }

    #[test]
    fn modified_indicator_same_value_not_modified() {
        let mut ind = SettingsModifiedIndicator::new();
        ind.mark_modified("x", "val", "val");
        assert!(!ind.is_modified("x"));
    }

    #[test]
    fn modified_indicator_display() {
        let ind = SettingsModifiedIndicator::new();
        assert!(format!("{ind}").contains("0 modified"));
    }

    // --- SettingsResetHandler ---

    #[test]
    fn reset_handler_register_and_get() {
        let mut rh = SettingsResetHandler::new();
        rh.register_default("font", "14");
        assert!(rh.has_default("font"));
        assert_eq!(rh.get_default("font"), Some("14"));
        assert_eq!(rh.reset("font"), Some("14".to_string()));
        assert_eq!(rh.get_default("missing"), None);
    }

    #[test]
    fn reset_handler_reset_all_sorted() {
        let mut rh = SettingsResetHandler::new();
        rh.register_default("z.key", "z");
        rh.register_default("a.key", "a");
        let all = rh.reset_all();
        assert_eq!(all[0].0, "a.key");
        assert_eq!(all[1].0, "z.key");
        assert!(format!("{rh}").contains("2 defaults"));
    }

    // --- SettingsTableOfContents ---

    #[test]
    fn toc_empty() {
        let toc = SettingsTableOfContents::new();
        assert_eq!(toc.depth(), 0);
        assert!(toc.sections().is_empty());
        assert!(toc.find_section("x").is_none());
    }

    #[test]
    fn toc_nested_sections() {
        let mut toc = SettingsTableOfContents::new();
        toc.add_section("editor", "Editor");
        toc.add_section("editor.font", "Font");
        toc.add_section("editor.font.size", "Size");
        toc.add_section("terminal", "Terminal");

        assert_eq!(toc.sections().len(), 2);
        assert_eq!(toc.depth(), 3);

        let font = toc.find_section("editor.font").unwrap();
        assert_eq!(font.label, "Font");
        assert_eq!(font.children.len(), 1);

        assert!(toc.find_section("editor.font.size").is_some());
        assert!(toc.find_section("nonexistent").is_none());
    }

    #[test]
    fn toc_display() {
        let mut toc = SettingsTableOfContents::new();
        toc.add_section("a", "A");
        assert!(format!("{toc}").contains("1 roots"));
    }

    #[test]
    fn toc_entry_display() {
        let entry = TocEntry::new("editor.font", "Font");
        assert!(format!("{entry}").contains("Font"));
        assert!(format!("{entry}").contains("editor.font"));
    }

    #[test]
    fn settb_builder_valid() {
        let cfg = SettBBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn settb_builder_empty_name() {
        let r = SettBBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn settb_builder_bad_priority() {
        assert!(SettBBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn settb_builder_zero_max() {
        assert!(SettBBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn settb_cfg_merge() {
        let mut a = SettBBuilder::new("a").property("x", "1").build().unwrap();
        let b = SettBBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn settb_cfg_display() {
        let cfg = SettBBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }

    #[test]
    fn settf_fmt_list() {
        let f = SettFFmt::new(SettFFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn settf_fmt_kv() {
        let f = SettFFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn settf_fmt_section() {
        let f = SettFFmt::new(SettFFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn settf_fmt_truncate() {
        let f = SettFFmt::new(SettFFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn settf_fmt_opts_defaults() {
        let o = SettFFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn settings_ui_config_new() {
        let cfg = SettingsUiConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn settings_ui_config_set_get() {
        let mut cfg = SettingsUiConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn settings_ui_config_remove() {
        let mut cfg = SettingsUiConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn settings_ui_config_keys_sorted() {
        let mut cfg = SettingsUiConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn settings_ui_config_bump_version() {
        let mut cfg = SettingsUiConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn settings_ui_config_clear() {
        let mut cfg = SettingsUiConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn settings_ui_config_merge() {
        let mut cfg1 = SettingsUiConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = SettingsUiConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn settings_ui_config_disable() {
        let mut cfg = SettingsUiConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn settings_ui_rate_tracker_empty() {
        let rt = SettingsUiRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn settings_ui_rate_tracker_record() {
        let mut rt = SettingsUiRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn settings_ui_rate_tracker_prune() {
        let mut rt = SettingsUiRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn settings_ui_validator_valid() {
        let v = SettingsUiValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn settings_ui_validator_errors() {
        let mut v = SettingsUiValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn settings_ui_validator_clear() {
        let mut v = SettingsUiValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn settings_ui_validator_merge() {
        let mut v1 = SettingsUiValidator::new();
        v1.add_error("e1");
        let mut v2 = SettingsUiValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn settings_ui_rate_tracker_clear() {
        let mut rt = SettingsUiRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yf_metrics_empty() {
        let m = YfMetrics::new("settings_ui");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yf_metrics_record_and_mean() {
        let mut m = YfMetrics::new("settings_ui");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yf_metrics_min_max() {
        let mut m = YfMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yf_metrics_variance_and_std() {
        let mut m = YfMetrics::new("v");
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
    fn yf_metrics_percentile() {
        let mut m = YfMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yf_metrics_merge() {
        let mut a = YfMetrics::new("a");
        a.record(1.0);
        let mut b = YfMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yf_metrics_reset() {
        let mut m = YfMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yf_rate_window_empty() {
        let rw = YfRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yf_rate_window_tick_and_rate() {
        let mut rw = YfRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yf_lru_cache_basic() {
        let mut c = YfLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yf_lru_cache_contains_and_keys() {
        let mut c = YfLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yf_lru_cache_remove() {
        let mut c = YfLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yf_metrics_sum() {
        let mut m = YfMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yf_metrics_label() {
        let m = YfMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yf_lru_cache_clear() {
        let mut c = YfLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for settings_ui
    #[test]
    fn xa_settings_ui_ring_new() {
        let rb = super::XaSettingsUiRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_settings_ui_ring_push_len() {
        let mut rb = super::XaSettingsUiRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_settings_ui_ring_wrap() {
        let mut rb = super::XaSettingsUiRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_settings_ui_ring_mean_empty() {
        let rb = super::XaSettingsUiRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_settings_ui_ring_mean_values() {
        let mut rb = super::XaSettingsUiRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_settings_ui_ring_min_max() {
        let mut rb = super::XaSettingsUiRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_settings_ui_ring_iter() {
        let mut rb = super::XaSettingsUiRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_settings_ui_counter_new() {
        let c = super::XaSettingsUiCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_settings_ui_counter_inc() {
        let mut c = super::XaSettingsUiCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_settings_ui_counter_inc_by() {
        let mut c = super::XaSettingsUiCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_settings_ui_counter_reset() {
        let mut c = super::XaSettingsUiCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_settings_ui_counter_clear() {
        let mut c = super::XaSettingsUiCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_settings_ui_counter_default() {
        let c = super::XaSettingsUiCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 157 ----

    #[test]
    fn xc_157_pool_new_empty() {
        let pool: super::Xc157Pool<i32> = super::Xc157Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_157_pool_release_acquire() {
        let mut pool = super::Xc157Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_157_pool_acquire_empty() {
        let mut pool: super::Xc157Pool<i32> = super::Xc157Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_157_pool_full() {
        let mut pool = super::Xc157Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_157_pool_drain() {
        let mut pool = super::Xc157Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_157_pool_stats() {
        let mut pool = super::Xc157Pool::new(8);
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
    fn xc_157_pool_clear() {
        let mut pool = super::Xc157Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_157_pool_shrink() {
        let mut pool = super::Xc157Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_157_pool_default() {
        let pool: super::Xc157Pool<String> = super::Xc157Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_157_pool_extend() {
        let mut pool = super::Xc157Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_157_pool_retain() {
        let mut pool = super::Xc157Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_157_scheduler_round_robin() {
        let mut sched = super::Xc157Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_157_scheduler_empty() {
        let mut sched = super::Xc157Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_157_scheduler_reset() {
        let mut sched = super::Xc157Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_157_scheduler_add_remove() {
        let mut sched = super::Xc157Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_157_scheduler_targets() {
        let sched = super::Xc157Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_157_hash_empty() {
        assert_eq!(super::xc_157_hash(b""), 5381);
    }

    #[test]
    fn xc_157_hash_data() {
        let h = super::xc_157_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_157_hash(b"hello"), h);
    }

    #[test]
    fn xc_157_reverse_str() {
        assert_eq!(super::xc_157_reverse("abc"), "cba");
        assert_eq!(super::xc_157_reverse(""), "");
    }


    // --- xd_77 deepening tests ---

    #[test]
    fn xd_77_sm_initial_state() {
        let sm = Xd77StateMachine::new();
        assert_eq!(sm.current_state(), Xd77State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_77_sm_valid_idle_to_running() {
        let mut sm = Xd77StateMachine::new();
        assert!(sm.transition(Xd77State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd77State::Running);
    }

    #[test]
    fn xd_77_sm_valid_running_to_paused() {
        let mut sm = Xd77StateMachine::new();
        sm.transition(Xd77State::Running).unwrap();
        assert!(sm.transition(Xd77State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd77State::Paused);
    }

    #[test]
    fn xd_77_sm_valid_running_to_done() {
        let mut sm = Xd77StateMachine::new();
        sm.transition(Xd77State::Running).unwrap();
        assert!(sm.transition(Xd77State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd77State::Done);
    }

    #[test]
    fn xd_77_sm_valid_paused_to_running() {
        let mut sm = Xd77StateMachine::new();
        sm.transition(Xd77State::Running).unwrap();
        sm.transition(Xd77State::Paused).unwrap();
        assert!(sm.transition(Xd77State::Running).is_ok());
    }

    #[test]
    fn xd_77_sm_valid_done_to_idle() {
        let mut sm = Xd77StateMachine::new();
        sm.transition(Xd77State::Running).unwrap();
        sm.transition(Xd77State::Done).unwrap();
        assert!(sm.transition(Xd77State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd77State::Idle);
    }

    #[test]
    fn xd_77_sm_invalid_idle_to_done() {
        let mut sm = Xd77StateMachine::new();
        assert!(sm.transition(Xd77State::Done).is_err());
    }

    #[test]
    fn xd_77_sm_invalid_idle_to_paused() {
        let mut sm = Xd77StateMachine::new();
        assert!(sm.transition(Xd77State::Paused).is_err());
    }

    #[test]
    fn xd_77_sm_history_tracking() {
        let mut sm = Xd77StateMachine::new();
        sm.transition(Xd77State::Running).unwrap();
        sm.transition(Xd77State::Paused).unwrap();
        sm.transition(Xd77State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd77State::Idle);
        assert_eq!(sm.history()[0].to, Xd77State::Running);
        assert_eq!(sm.history()[1].from, Xd77State::Running);
        assert_eq!(sm.history()[2].to, Xd77State::Done);
    }

    #[test]
    fn xd_77_sm_serialize_deserialize() {
        let mut sm = Xd77StateMachine::new();
        sm.transition(Xd77State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd77StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd77State::Running));
    }

    #[test]
    fn xd_77_sm_deserialize_invalid() {
        assert_eq!(Xd77StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_77_sm_reset() {
        let mut sm = Xd77StateMachine::new();
        sm.transition(Xd77State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd77State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_77_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd77EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd77Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_77_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd77EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd77Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd77Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_77_bus_unsubscribe() {
        let mut bus = Xd77EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_77_event_kind_and_payload() {
        let e = Xd77Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd77Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_77_bus_clear_history() {
        let mut bus = Xd77EventBus::new();
        bus.publish(Xd77Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_77_sm_step_counter_increments() {
        let mut sm = Xd77StateMachine::new();
        sm.transition(Xd77State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd77State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #96 --

    #[test]
    fn xf96_trie_insert_search() {
        let mut t = Xf96Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf96_trie_starts_with() {
        let mut t = Xf96Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf96_trie_remove() {
        let mut t = Xf96Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf96_trie_word_count() {
        let mut t = Xf96Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf96_trie_longest_prefix() {
        let mut t = Xf96Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf96_trie_all_words() {
        let mut t = Xf96Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf96_trie_autocomplete() {
        let mut t = Xf96Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf96_trie_empty_search() {
        let t = Xf96Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf96_bloom_add_contains() {
        let mut bf = Xf96BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf96_bloom_probably_absent() {
        let bf = Xf96BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf96_bloom_false_positive_rate() {
        let mut bf = Xf96BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf96_bloom_clear() {
        let mut bf = Xf96BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf96_bloom_union() {
        let mut a = Xf96BloomFilter::xf_new(512, 2);
        let mut b = Xf96BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf96_bloom_intersection_estimate() {
        let mut a = Xf96BloomFilter::xf_new(512, 2);
        let mut b = Xf96BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf96_bloom_union_size_mismatch() {
        let a = Xf96BloomFilter::xf_new(256, 2);
        let b = Xf96BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh156_skip_insert_contains() {
        let mut sl = super::Xh156SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh156_skip_remove() {
        let mut sl = super::Xh156SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh156_skip_len() {
        let mut sl = super::Xh156SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh156_skip_range_query() {
        let mut sl = super::Xh156SkipList::xh_new(4);
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
    fn xh156_skip_floor_ceiling() {
        let mut sl = super::Xh156SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh156_skip_rank() {
        let mut sl = super::Xh156SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh156_skip_empty() {
        let sl = super::Xh156SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh156_skip_duplicates() {
        let mut sl = super::Xh156SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh156_bitset_set_test() {
        let mut bs = super::Xh156BitSet::xh_new(256);
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
    fn xh156_bitset_clear_count() {
        let mut bs = super::Xh156BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh156_bitset_and_or_xor() {
        let mut a = super::Xh156BitSet::xh_new(128);
        let mut b = super::Xh156BitSet::xh_new(128);
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
    fn xh156_bitset_iter_ones() {
        let mut bs = super::Xh156BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh156_bitset_first_last() {
        let mut bs = super::Xh156BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh156_bitset_empty() {
        let bs = super::Xh156BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi156_deque_push_pop_back() {
        let mut dq = super::Xi156Deque::xi_new(4);
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
    fn xi156_deque_push_pop_front() {
        let mut dq = super::Xi156Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi156_deque_mixed_ops() {
        let mut dq = super::Xi156Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi156_deque_get_and_split() {
        let mut dq = super::Xi156Deque::xi_new(8);
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
    fn xi156_deque_rotate_left() {
        let mut dq = super::Xi156Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi156_deque_rotate_right() {
        let mut dq = super::Xi156Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi156_deque_grow() {
        let mut dq = super::Xi156Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi156_deque_empty() {
        let dq = super::Xi156Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi156_interval_tree_insert_query() {
        let mut tree = super::Xi156IntervalTree::xi_new();
        tree.xi_insert(super::Xi156Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi156Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi156Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi156_interval_tree_overlap() {
        let mut tree = super::Xi156IntervalTree::xi_new();
        tree.xi_insert(super::Xi156Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi156Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi156Interval::xi_new(12, 20));
        let q = super::Xi156Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi156_interval_tree_remove() {
        let mut tree = super::Xi156IntervalTree::xi_new();
        tree.xi_insert(super::Xi156Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi156Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi156_interval_tree_gaps() {
        let mut tree = super::Xi156IntervalTree::xi_new();
        tree.xi_insert(super::Xi156Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi156Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi156Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi156Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi156Interval::xi_new(8, 10));
    }

    #[test]
    fn xi156_interval_tree_merge() {
        let mut tree = super::Xi156IntervalTree::xi_new();
        tree.xi_insert(super::Xi156Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi156Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi156Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi156Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi156Interval::xi_new(10, 15));
    }

    #[test]
    fn xi156_interval_tree_all() {
        let mut tree = super::Xi156IntervalTree::xi_new();
        tree.xi_insert(super::Xi156Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi156Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi156_interval_tree_empty() {
        let tree = super::Xi156IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi156_interval_tree_contains_point() {
        let iv = super::Xi156Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 156) ---

    #[test]
    fn xj_156_uf_make_and_find() {
        let mut uf = super::Xj156UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_156_uf_union_connected() {
        let mut uf = super::Xj156UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_156_uf_component_count() {
        let mut uf = super::Xj156UnionFind::xj_new();
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
    fn xj_156_uf_component_size() {
        let mut uf = super::Xj156UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_156_uf_largest_component() {
        let mut uf = super::Xj156UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_156_uf_many_elements() {
        let mut uf = super::Xj156UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_156_uf_separate_components() {
        let mut uf = super::Xj156UnionFind::xj_new();
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
    fn xj_156_uf_path_compression() {
        let mut uf = super::Xj156UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_156_bt_insert_get() {
        let mut bt = super::Xj156BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_156_bt_contains_len() {
        let mut bt = super::Xj156BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_156_bt_replace() {
        let mut bt = super::Xj156BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_156_bt_remove() {
        let mut bt = super::Xj156BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_156_bt_keys_values() {
        let mut bt = super::Xj156BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_156_bt_range() {
        let mut bt = super::Xj156BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_156_bt_min_max() {
        let mut bt = super::Xj156BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_156_bt_many_inserts() {
        let mut bt = super::Xj156BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_156 segment tree tests ---

    #[test]
    fn xk_156_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk156SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_156_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk156SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_156_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk156SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_156_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk156SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_156_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk156SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_156_st_single_element() {
        let data = vec![42];
        let st = super::Xk156SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_156_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk156SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_156_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk156SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_156 disjoint intervals tests ---

    #[test]
    fn xk_156_di_add_and_count() {
        let mut di = super::Xk156DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_156_di_merge_overlap() {
        let mut di = super::Xk156DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_156_di_contains() {
        let mut di = super::Xk156DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_156_di_remove() {
        let mut di = super::Xk156DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_156_di_covered_length() {
        let mut di = super::Xk156DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_156_di_gaps() {
        let mut di = super::Xk156DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_156_di_merge_adjacent() {
        let mut di = super::Xk156DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_156_di_empty() {
        let di = super::Xk156DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_156_rope_new_empty() {
        let rope = super::Xl156Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_156_rope_from_str() {
        let rope = super::Xl156Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_156_rope_insert_at() {
        let mut rope = super::Xl156Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_156_rope_delete_range() {
        let mut rope = super::Xl156Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_156_rope_char_at() {
        let rope = super::Xl156Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_156_rope_split_concat() {
        let rope = super::Xl156Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_156_rope_line_count() {
        let rope = super::Xl156Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_156_rope_line_at() {
        let rope = super::Xl156Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_156_sa_build_and_search() {
        let sa = super::Xl156SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_156_sa_count() {
        let sa = super::Xl156SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_156_sa_longest_repeated() {
        let sa = super::Xl156SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_156_sa_all_positions() {
        let sa = super::Xl156SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_156_sa_len() {
        let sa = super::Xl156SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_156_sa_empty() {
        let sa = super::Xl156SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_156_rope_slice() {
        let rope = super::Xl156Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_156_sa_search_start() {
        let sa = super::Xl156SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_156_sparse_set_get() {
        let mut m = super::Xm156MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_156_sparse_row_col() {
        let mut m = super::Xm156MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_156_sparse_transpose() {
        let mut m = super::Xm156MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_156_sparse_multiply_vec() {
        let mut m = super::Xm156MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_156_sparse_nnz_density() {
        let mut m = super::Xm156MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_156_sparse_clear() {
        let mut m = super::Xm156MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_156_sparse_overwrite_zero() {
        let mut m = super::Xm156MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_156_tokenizer_basic() {
        let t = super::Xm156Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_156_tokenizer_count() {
        let t = super::Xm156Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_156_tokenizer_unique() {
        let t = super::Xm156Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_156_tokenizer_frequency() {
        let t = super::Xm156Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_156_tokenizer_delimiter() {
        let t = super::Xm156Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_156_tokenizer_whitespace() {
        let t = super::Xm156Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_156_tokenizer_empty() {
        let t = super::Xm156Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 156 ----

    #[test]
    fn xn_156_fenwick_prefix_sum() {
        let mut ft = super::Xn156Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_156_fenwick_range_sum() {
        let mut ft = super::Xn156Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_156_fenwick_point_query() {
        let mut ft = super::Xn156Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_156_fenwick_len() {
        let ft = super::Xn156Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_156_fenwick_multiple_updates() {
        let mut ft = super::Xn156Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_156_fenwick_single_element() {
        let mut ft = super::Xn156Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_156_fenwick_find_kth() {
        let mut ft = super::Xn156Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_156_fenwick_negative_delta() {
        let mut ft = super::Xn156Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 156 ----

    #[test]
    fn xn_156_avl_insert_get() {
        let mut m = super::Xn156AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_156_avl_remove() {
        let mut m = super::Xn156AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_156_avl_in_order() {
        let mut m = super::Xn156AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_156_avl_min_max() {
        let mut m = super::Xn156AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_156_avl_floor_ceiling() {
        let mut m = super::Xn156AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_156_avl_height_balanced() {
        let mut m = super::Xn156AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_156_avl_overwrite() {
        let mut m = super::Xn156AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_156_avl_empty() {
        let m: super::Xn156AVL<i32, i32> = super::Xn156AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo156RedBlack tests ---

    #[test]
    fn xo_156_rb_insert_and_get() {
        let mut tree = super::Xo156RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_156_rb_len_and_empty() {
        let mut tree = super::Xo156RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_156_rb_min_max() {
        let mut tree = super::Xo156RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_156_rb_contains() {
        let mut tree = super::Xo156RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_156_rb_remove() {
        let mut tree = super::Xo156RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_156_rb_in_order() {
        let mut tree = super::Xo156RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_156_rb_black_height() {
        let mut tree = super::Xo156RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_156_rb_overwrite() {
        let mut tree = super::Xo156RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo156ConsistentHash tests ---

    #[test]
    fn xo_156_ch_add_and_count() {
        let mut ring = super::Xo156ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_156_ch_remove_node() {
        let mut ring = super::Xo156ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_156_ch_get_node() {
        let mut ring = super::Xo156ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_156_ch_empty_ring() {
        let ring = super::Xo156ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_156_ch_distribution() {
        let mut ring = super::Xo156ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_156_ch_rebalance() {
        let mut ring = super::Xo156ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_156_ch_virtual_nodes() {
        let mut ring = super::Xo156ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_156_ch_consistent_lookup() {
        let mut ring = super::Xo156ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_156_splay_insert_get() {
        let mut t = super::Xp156SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_156_splay_remove() {
        let mut t = super::Xp156SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_156_splay_count_increases() {
        let mut t = super::Xp156SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_156_splay_depth() {
        let mut t = super::Xp156SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_156_splay_len_empty() {
        let t = super::Xp156SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_156_splay_min_max() {
        let mut t = super::Xp156SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_156_splay_overwrite() {
        let mut t = super::Xp156SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_156_splay_remove_missing() {
        let mut t = super::Xp156SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_156 treap tests ----
    #[test]
    fn xq_156_treap_empty() {
        let t = super::Xq156Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_156_treap_insert_get() {
        let mut t = super::Xq156Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_156_treap_overwrite() {
        let mut t = super::Xq156Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_156_treap_remove() {
        let mut t = super::Xq156Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_156_treap_min_max() {
        let mut t = super::Xq156Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_156_treap_rank() {
        let mut t = super::Xq156Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_156_treap_kth() {
        let mut t = super::Xq156Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_156_treap_in_order() {
        let mut t = super::Xq156Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_156 VEB tree tests ----
    #[test]
    fn xq_156_veb_empty() {
        let v = super::Xq156VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_156_veb_insert_contains() {
        let mut v = super::Xq156VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_156_veb_min_max() {
        let mut v = super::Xq156VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_156_veb_delete() {
        let mut v = super::Xq156VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_156_veb_successor() {
        let mut v = super::Xq156VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_156_veb_predecessor() {
        let mut v = super::Xq156VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_156_veb_count() {
        let mut v = super::Xq156VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_156_veb_duplicate_insert() {
        let mut v = super::Xq156VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_156_kdtree_empty() {
        let tree = super::Xr156KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_156_kdtree_insert_one() {
        let mut tree = super::Xr156KDTree::xr_new();
        tree.xr_insert(super::Xr156KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_156_kdtree_insert_multiple() {
        let mut tree = super::Xr156KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr156KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_156_kdtree_nearest_neighbor() {
        let mut tree = super::Xr156KDTree::xr_new();
        tree.xr_insert(super::Xr156KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr156KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr156KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_156_kdtree_nn_empty() {
        let tree = super::Xr156KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr156KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_156_kdtree_range_search() {
        let mut tree = super::Xr156KDTree::xr_new();
        tree.xr_insert(super::Xr156KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr156KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr156KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_156_kdtree_range_empty() {
        let mut tree = super::Xr156KDTree::xr_new();
        tree.xr_insert(super::Xr156KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_156_kdtree_all_points() {
        let mut tree = super::Xr156KDTree::xr_new();
        tree.xr_insert(super::Xr156KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr156KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_156_kdtree_depth() {
        let mut tree = super::Xr156KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr156KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_156_kdtree_bounding_box() {
        let mut tree = super::Xr156KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr156KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr156KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_156_persistent_array_new() {
        let arr = super::Xs156PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_156_persistent_array_push() {
        let mut arr = super::Xs156PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_156_persistent_array_set() {
        let mut arr = super::Xs156PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_156_persistent_array_diff() {
        let mut arr = super::Xs156PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_156_persistent_array_rollback() {
        let mut arr = super::Xs156PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_156_persistent_array_history() {
        let mut arr = super::Xs156PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_156_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs156PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_156_persistent_array_from_vec() {
        let arr = super::Xs156PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_156_concurrent_queue_new() {
        let q = super::Xs156ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_156_concurrent_queue_push_pop() {
        let mut q = super::Xs156ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_156_concurrent_queue_full() {
        let mut q = super::Xs156ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_156_concurrent_queue_drain() {
        let mut q = super::Xs156ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_156_concurrent_queue_try_pop() {
        let mut q = super::Xs156ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_156_concurrent_queue_clear() {
        let mut q = super::Xs156ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_156_range_map_new() {
        let rm = super::Xs156RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_156_range_map_insert_get() {
        let mut rm = super::Xs156RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_156_range_map_overlap() {
        let mut rm = super::Xs156RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_156_range_map_remove() {
        let mut rm = super::Xs156RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_156_range_map_gaps() {
        let mut rm = super::Xs156RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_156_range_map_coverage() {
        let mut rm = super::Xs156RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_156_range_map_contains() {
        let mut rm = super::Xs156RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_156_range_map_clear() {
        let mut rm = super::Xs156RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_156_circular_buffer_new() {
        let buf = super::Xs156CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_156_circular_buffer_push_pop() {
        let mut buf = super::Xs156CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_156_circular_buffer_overwrite() {
        let mut buf = super::Xs156CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_156_circular_buffer_peek() {
        let mut buf = super::Xs156CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_156_circular_buffer_is_full() {
        let mut buf = super::Xs156CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_156_circular_buffer_iter() {
        let mut buf = super::Xs156CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_156_circular_buffer_clear() {
        let mut buf = super::Xs156CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_156_circular_buffer_to_vec() {
        let mut buf = super::Xs156CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }

}