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
}
