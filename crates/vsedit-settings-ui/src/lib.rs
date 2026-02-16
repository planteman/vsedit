//! Settings editor UI.

use std::collections::HashMap;
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
}
