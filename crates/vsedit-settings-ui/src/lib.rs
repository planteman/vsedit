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
}
