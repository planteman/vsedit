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

}