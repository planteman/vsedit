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
#[derive(Debug, Clone)]
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
}
