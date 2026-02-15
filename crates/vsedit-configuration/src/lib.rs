//! Settings registry and overrides for vsedit.
//!
//! Equivalent to VS Code's `vs/platform/configuration/common/configuration.ts`.
//! Manages user settings, workspace settings, and defaults with layered
//! merging and dot-notation path resolution.

use std::collections::HashMap;
use std::sync::RwLock;

use serde::de::DeserializeOwned;
use serde_json::Value;

// ---------------------------------------------------------------------------
// ConfigurationScope
// ---------------------------------------------------------------------------

/// The scope at which a configuration setting applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigurationScope {
    /// Global — applies everywhere, stored once per installation.
    Application,
    /// Machine-specific settings (not synced across devices).
    Machine,
    /// Per-window settings.
    Window,
    /// Per-folder or per-file settings.
    Resource,
    /// Can be overridden per language identifier.
    LanguageOverridable,
}

// ---------------------------------------------------------------------------
// ConfigurationPropertySchema
// ---------------------------------------------------------------------------

/// Schema for a single configuration property.
#[derive(Debug, Clone)]
pub struct ConfigurationPropertySchema {
    /// The JSON type: `"string"`, `"number"`, `"boolean"`, `"array"`, `"object"`.
    pub property_type: String,
    /// Default value for this property.
    pub default: Value,
    /// Human-readable description.
    pub description: String,
    /// The scope at which this setting applies.
    pub scope: ConfigurationScope,
}

// ---------------------------------------------------------------------------
// ConfigurationRegistry
// ---------------------------------------------------------------------------

/// Registry of configuration schemas (settings definitions).
///
/// Extensions and built-in features register their configuration sections and
/// properties here. The registry tracks all known settings along with their
/// types, defaults, descriptions, and scopes.
pub struct ConfigurationRegistry {
    /// Map from fully-qualified property key (e.g. `"editor.fontSize"`) to its
    /// schema definition.
    properties: HashMap<String, ConfigurationPropertySchema>,
}

impl ConfigurationRegistry {
    /// Create an empty configuration registry.
    pub fn new() -> Self {
        Self {
            properties: HashMap::new(),
        }
    }

    /// Register a set of configuration properties under a section prefix.
    ///
    /// Each key in `properties` is a dotted property name relative to the
    /// section. For example, registering section `"editor"` with a property
    /// `"fontSize"` produces the fully-qualified key `"editor.fontSize"`.
    ///
    /// If `section` is empty the property keys are used as-is.
    pub fn register_configuration(
        &mut self,
        section: &str,
        properties: HashMap<String, ConfigurationPropertySchema>,
    ) {
        for (key, schema) in properties {
            let full_key = if section.is_empty() {
                key
            } else {
                format!("{section}.{key}")
            };
            self.properties.insert(full_key, schema);
        }
    }

    /// Returns the schema for a fully-qualified property key, if registered.
    pub fn get_property(&self, key: &str) -> Option<&ConfigurationPropertySchema> {
        self.properties.get(key)
    }

    /// Build a [`ConfigurationModel`] containing the default value for every
    /// registered property.
    pub fn get_defaults(&self) -> ConfigurationModel {
        let mut model = ConfigurationModel::new();
        for (key, schema) in &self.properties {
            model.set_value(key, schema.default.clone());
        }
        model
    }

    /// Returns the number of registered properties.
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    /// Returns `true` if no properties are registered.
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }
}

impl Default for ConfigurationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ConfigurationModel — a single layer of configuration values
// ---------------------------------------------------------------------------

/// A single layer of configuration values (e.g. user settings, workspace
/// settings, or defaults).
///
/// Values are stored as a flat `serde_json::Value::Object` but accessed
/// through dot-notation paths like `"editor.fontSize"`.
#[derive(Debug, Clone)]
pub struct ConfigurationModel {
    contents: Value,
}

impl ConfigurationModel {
    /// Create an empty configuration model.
    pub fn new() -> Self {
        Self {
            contents: Value::Object(serde_json::Map::new()),
        }
    }

    /// Parse a configuration model from a JSON or JSONC string.
    pub fn from_jsonc(input: &str) -> Result<Self, vsedit_json::ParseError> {
        let value = vsedit_json::parse_jsonc(input)?;
        Ok(Self { contents: value })
    }

    /// Get a typed value at the given dot-notation path.
    ///
    /// Returns `None` if the path does not exist or the value cannot be
    /// deserialized to `T`.
    pub fn get_value<T: DeserializeOwned>(&self, section: &str) -> Option<T> {
        let raw = self.get_raw_value(section)?;
        serde_json::from_value(raw.clone()).ok()
    }

    /// Get the raw `serde_json::Value` at the given dot-notation path.
    pub fn get_raw_value(&self, section: &str) -> Option<&Value> {
        if section.is_empty() {
            return Some(&self.contents);
        }
        let segments: Vec<&str> = section.split('.').collect();
        vsedit_json::get_value_at_path(&self.contents, &segments)
    }

    /// Set a value at the given dot-notation path, creating intermediate
    /// objects as needed.
    pub fn set_value(&mut self, section: &str, value: Value) {
        if section.is_empty() {
            self.contents = value;
            return;
        }
        let segments: Vec<&str> = section.split('.').collect();
        ensure_path(&mut self.contents, &segments, value);
    }

    /// Remove a value at the given dot-notation path.
    pub fn remove_value(&mut self, section: &str) {
        if section.is_empty() {
            self.contents = Value::Object(serde_json::Map::new());
            return;
        }
        let segments: Vec<&str> = section.split('.').collect();
        remove_at_path(&mut self.contents, &segments);
    }

    /// Returns true if this model has no values.
    pub fn is_empty(&self) -> bool {
        match &self.contents {
            Value::Object(map) => map.is_empty(),
            _ => false,
        }
    }

    /// Returns the underlying JSON value.
    pub fn as_value(&self) -> &Value {
        &self.contents
    }

    /// Merge another model into this one. Values from `other` override values
    /// in `self`.
    pub fn merge(&mut self, other: &ConfigurationModel) {
        self.contents = merge_values(&self.contents, &other.contents);
    }
}

impl Default for ConfigurationModel {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// InspectResult
// ---------------------------------------------------------------------------

/// Result of inspecting a configuration value showing which layer provides it.
#[derive(Debug, Clone)]
pub struct InspectResult {
    /// The value from the defaults layer.
    pub default_value: Option<Value>,
    /// The value from the user global layer.
    pub user_value: Option<Value>,
    /// The value from the workspace layer.
    pub workspace_value: Option<Value>,
    /// The value from the folder layer.
    pub folder_value: Option<Value>,
    /// The value from the memory layer.
    pub memory_value: Option<Value>,
    /// The effective merged value.
    pub merged_value: Option<Value>,
}

// ---------------------------------------------------------------------------
// ConfigurationTarget
// ---------------------------------------------------------------------------

/// Target layer for a configuration update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigurationTarget {
    /// Write to the defaults layer.
    Default,
    /// Write to the user global layer.
    UserGlobal,
    /// Write to the workspace layer.
    Workspace,
    /// Write to the folder layer.
    Folder,
    /// Write to the in-memory layer.
    Memory,
}

// ---------------------------------------------------------------------------
// Configuration — merged multi-layer configuration
// ---------------------------------------------------------------------------

/// Merged configuration assembled from multiple layers.
///
/// Layers (in priority order, highest wins):
/// 1. Memory (transient overrides)
/// 2. Folder (per-folder settings)
/// 3. Workspace (`.vscode/settings.json`)
/// 4. User global (`~/.config/vsedit/settings.json`)
/// 5. Defaults (from [`ConfigurationRegistry`])
pub struct Configuration {
    defaults: ConfigurationModel,
    user_global: ConfigurationModel,
    workspace: ConfigurationModel,
    folder: ConfigurationModel,
    memory: ConfigurationModel,
}

impl Configuration {
    /// Create a new configuration with all layers empty.
    pub fn new() -> Self {
        Self {
            defaults: ConfigurationModel::new(),
            user_global: ConfigurationModel::new(),
            workspace: ConfigurationModel::new(),
            folder: ConfigurationModel::new(),
            memory: ConfigurationModel::new(),
        }
    }

    /// Create a configuration seeded with a defaults layer.
    pub fn with_defaults(defaults: ConfigurationModel) -> Self {
        Self {
            defaults,
            user_global: ConfigurationModel::new(),
            workspace: ConfigurationModel::new(),
            folder: ConfigurationModel::new(),
            memory: ConfigurationModel::new(),
        }
    }

    /// Set a specific layer to the given model.
    pub fn set_layer(&mut self, target: ConfigurationTarget, model: ConfigurationModel) {
        match target {
            ConfigurationTarget::Default => self.defaults = model,
            ConfigurationTarget::UserGlobal => self.user_global = model,
            ConfigurationTarget::Workspace => self.workspace = model,
            ConfigurationTarget::Folder => self.folder = model,
            ConfigurationTarget::Memory => self.memory = model,
        }
    }

    /// Get the effective (merged) value at a dot-notation path.
    pub fn get_value<T: DeserializeOwned>(&self, section: &str) -> Option<T> {
        let merged = self.merged_model();
        merged.get_value(section)
    }

    /// Inspect a key, returning the value from each layer and the merged
    /// result.
    pub fn inspect(&self, section: &str) -> InspectResult {
        let merged = self.merged_model();
        InspectResult {
            default_value: self.defaults.get_raw_value(section).cloned(),
            user_value: self.user_global.get_raw_value(section).cloned(),
            workspace_value: self.workspace.get_raw_value(section).cloned(),
            folder_value: self.folder.get_raw_value(section).cloned(),
            memory_value: self.memory.get_raw_value(section).cloned(),
            merged_value: merged.get_raw_value(section).cloned(),
        }
    }

    /// Update a value in the specified target layer.
    pub fn update(&mut self, section: &str, value: Value, target: ConfigurationTarget) {
        let layer = match target {
            ConfigurationTarget::Default => &mut self.defaults,
            ConfigurationTarget::UserGlobal => &mut self.user_global,
            ConfigurationTarget::Workspace => &mut self.workspace,
            ConfigurationTarget::Folder => &mut self.folder,
            ConfigurationTarget::Memory => &mut self.memory,
        };
        layer.set_value(section, value);
    }

    /// Build the merged model from all layers.
    fn merged_model(&self) -> ConfigurationModel {
        let mut merged = self.defaults.clone();
        merged.merge(&self.user_global);
        merged.merge(&self.workspace);
        merged.merge(&self.folder);
        merged.merge(&self.memory);
        merged
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// IConfigurationService trait
// ---------------------------------------------------------------------------

/// Service trait for configuration access.
///
/// Implementations provide typed access to the merged configuration and
/// allow updating values in the appropriate layer.
pub trait IConfigurationService: Send + Sync {
    /// Get the effective (merged) value at a dot-notation path.
    fn get_value<T: DeserializeOwned>(&self, section: &str) -> Option<T>;

    /// Update a value, targeting the layer appropriate for the given scope.
    fn update_value(&self, section: &str, value: Value, scope: ConfigurationScope);
}

// ---------------------------------------------------------------------------
// ConfigurationService — default implementation
// ---------------------------------------------------------------------------

/// Default [`IConfigurationService`] backed by a [`Configuration`] behind a
/// read-write lock.
pub struct ConfigurationService {
    inner: RwLock<Configuration>,
}

impl ConfigurationService {
    /// Create a service from an initial [`Configuration`].
    pub fn new(configuration: Configuration) -> Self {
        Self {
            inner: RwLock::new(configuration),
        }
    }

    /// Get a snapshot reference to inspect a value.
    pub fn inspect(&self, section: &str) -> InspectResult {
        let guard = self.inner.read().unwrap();
        guard.inspect(section)
    }

    /// Replace a full layer.
    pub fn set_layer(&self, target: ConfigurationTarget, model: ConfigurationModel) {
        let mut guard = self.inner.write().unwrap();
        guard.set_layer(target, model);
    }
}

impl IConfigurationService for ConfigurationService {
    fn get_value<T: DeserializeOwned>(&self, section: &str) -> Option<T> {
        let guard = self.inner.read().unwrap();
        guard.get_value(section)
    }

    fn update_value(&self, section: &str, value: Value, scope: ConfigurationScope) {
        let target = scope_to_target(scope);
        let mut guard = self.inner.write().unwrap();
        guard.update(section, value, target);
    }
}

/// Map a [`ConfigurationScope`] to the [`ConfigurationTarget`] layer where
/// writes should land.
fn scope_to_target(scope: ConfigurationScope) -> ConfigurationTarget {
    match scope {
        ConfigurationScope::Application | ConfigurationScope::Machine => {
            ConfigurationTarget::UserGlobal
        }
        ConfigurationScope::Window => ConfigurationTarget::UserGlobal,
        ConfigurationScope::Resource => ConfigurationTarget::Folder,
        ConfigurationScope::LanguageOverridable => ConfigurationTarget::Folder,
    }
}

// ---------------------------------------------------------------------------
// JSON path helpers
// ---------------------------------------------------------------------------

/// Walk into `root` along `segments`, creating intermediate objects, and set
/// the leaf to `value`.
fn ensure_path(root: &mut Value, segments: &[&str], value: Value) {
    if segments.is_empty() {
        return;
    }

    let mut current = root;
    for &key in &segments[..segments.len() - 1] {
        if !current.is_object() {
            *current = Value::Object(serde_json::Map::new());
        }
        current = current
            .as_object_mut()
            .unwrap()
            .entry(key)
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
    }

    let last = segments.last().unwrap();
    if !current.is_object() {
        *current = Value::Object(serde_json::Map::new());
    }
    current
        .as_object_mut()
        .unwrap()
        .insert((*last).to_string(), value);
}

/// Walk into `root` along `segments` and remove the leaf key.
fn remove_at_path(root: &mut Value, segments: &[&str]) {
    if segments.is_empty() {
        return;
    }
    if segments.len() == 1 {
        if let Some(obj) = root.as_object_mut() {
            obj.remove(segments[0]);
        }
        return;
    }

    let mut current = root;
    for &key in &segments[..segments.len() - 1] {
        match current.get_mut(key) {
            Some(v) => current = v,
            None => return,
        }
    }

    if let Some(obj) = current.as_object_mut() {
        obj.remove(*segments.last().unwrap());
    }
}

/// Deep-merge two JSON values. Object values are recursively merged; other
/// types are replaced by the `overlay` value.
fn merge_values(base: &Value, overlay: &Value) -> Value {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            let mut merged = base_map.clone();
            for (key, overlay_val) in overlay_map {
                let new_val = if let Some(base_val) = merged.get(key) {
                    merge_values(base_val, overlay_val)
                } else {
                    overlay_val.clone()
                };
                merged.insert(key.clone(), new_val);
            }
            Value::Object(merged)
        }
        (_, overlay) => overlay.clone(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- ConfigurationModel: dot-path resolution ----------------------------

    #[test]
    fn model_set_and_get_simple() {
        let mut model = ConfigurationModel::new();
        model.set_value("editor.fontSize", json!(14));

        let val: Option<i64> = model.get_value("editor.fontSize");
        assert_eq!(val, Some(14));
    }

    #[test]
    fn model_set_and_get_nested() {
        let mut model = ConfigurationModel::new();
        model.set_value("editor.font.family", json!("Fira Code"));

        let val: Option<String> = model.get_value("editor.font.family");
        assert_eq!(val.as_deref(), Some("Fira Code"));
    }

    #[test]
    fn model_get_missing_returns_none() {
        let model = ConfigurationModel::new();
        let val: Option<i64> = model.get_value("does.not.exist");
        assert_eq!(val, None);
    }

    #[test]
    fn model_get_partial_path_returns_object() {
        let mut model = ConfigurationModel::new();
        model.set_value("editor.fontSize", json!(14));
        model.set_value("editor.tabSize", json!(4));

        let raw = model.get_raw_value("editor").unwrap();
        assert!(raw.is_object());
        assert_eq!(raw.get("fontSize"), Some(&json!(14)));
        assert_eq!(raw.get("tabSize"), Some(&json!(4)));
    }

    #[test]
    fn model_set_overwrites_existing() {
        let mut model = ConfigurationModel::new();
        model.set_value("editor.fontSize", json!(14));
        model.set_value("editor.fontSize", json!(16));

        let val: Option<i64> = model.get_value("editor.fontSize");
        assert_eq!(val, Some(16));
    }

    #[test]
    fn model_remove_value() {
        let mut model = ConfigurationModel::new();
        model.set_value("editor.fontSize", json!(14));
        model.set_value("editor.tabSize", json!(4));
        model.remove_value("editor.fontSize");

        let val: Option<i64> = model.get_value("editor.fontSize");
        assert_eq!(val, None);
        let tab: Option<i64> = model.get_value("editor.tabSize");
        assert_eq!(tab, Some(4));
    }

    #[test]
    fn model_is_empty() {
        let model = ConfigurationModel::new();
        assert!(model.is_empty());

        let mut model2 = ConfigurationModel::new();
        model2.set_value("a", json!(1));
        assert!(!model2.is_empty());
    }

    // -- ConfigurationModel: JSONC parsing ----------------------------------

    #[test]
    fn model_from_jsonc_plain() {
        let model =
            ConfigurationModel::from_jsonc(r#"{"editor": {"fontSize": 14}}"#).unwrap();
        let val: Option<i64> = model.get_value("editor.fontSize");
        assert_eq!(val, Some(14));
    }

    #[test]
    fn model_from_jsonc_with_comments() {
        let input = r#"{
            // Font size for the editor
            "editor": {
                "fontSize": 14,
                /* Tab size */
                "tabSize": 4,
            },
        }"#;
        let model = ConfigurationModel::from_jsonc(input).unwrap();
        let font: Option<i64> = model.get_value("editor.fontSize");
        assert_eq!(font, Some(14));
        let tab: Option<i64> = model.get_value("editor.tabSize");
        assert_eq!(tab, Some(4));
    }

    #[test]
    fn model_from_jsonc_invalid() {
        let result = ConfigurationModel::from_jsonc("{invalid}");
        assert!(result.is_err());
    }

    // -- ConfigurationModel: merge ------------------------------------------

    #[test]
    fn model_merge_simple() {
        let mut base = ConfigurationModel::new();
        base.set_value("editor.fontSize", json!(14));
        base.set_value("editor.tabSize", json!(4));

        let mut overlay = ConfigurationModel::new();
        overlay.set_value("editor.fontSize", json!(16));
        overlay.set_value("editor.wordWrap", json!("on"));

        base.merge(&overlay);

        let font: Option<i64> = base.get_value("editor.fontSize");
        assert_eq!(font, Some(16));
        let tab: Option<i64> = base.get_value("editor.tabSize");
        assert_eq!(tab, Some(4));
        let wrap: Option<String> = base.get_value("editor.wordWrap");
        assert_eq!(wrap.as_deref(), Some("on"));
    }

    // -- Configuration: layer merging ---------------------------------------

    #[test]
    fn config_defaults_layer() {
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.fontSize", json!(14));

        let config = Configuration::with_defaults(defaults);
        let val: Option<i64> = config.get_value("editor.fontSize");
        assert_eq!(val, Some(14));
    }

    #[test]
    fn config_user_overrides_defaults() {
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.fontSize", json!(14));

        let mut user = ConfigurationModel::new();
        user.set_value("editor.fontSize", json!(16));

        let mut config = Configuration::with_defaults(defaults);
        config.set_layer(ConfigurationTarget::UserGlobal, user);

        let val: Option<i64> = config.get_value("editor.fontSize");
        assert_eq!(val, Some(16));
    }

    #[test]
    fn config_workspace_overrides_user() {
        let mut user = ConfigurationModel::new();
        user.set_value("editor.fontSize", json!(16));

        let mut ws = ConfigurationModel::new();
        ws.set_value("editor.fontSize", json!(18));

        let mut config = Configuration::new();
        config.set_layer(ConfigurationTarget::UserGlobal, user);
        config.set_layer(ConfigurationTarget::Workspace, ws);

        let val: Option<i64> = config.get_value("editor.fontSize");
        assert_eq!(val, Some(18));
    }

    #[test]
    fn config_memory_overrides_all() {
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.fontSize", json!(14));

        let mut user = ConfigurationModel::new();
        user.set_value("editor.fontSize", json!(16));

        let mut memory = ConfigurationModel::new();
        memory.set_value("editor.fontSize", json!(20));

        let mut config = Configuration::with_defaults(defaults);
        config.set_layer(ConfigurationTarget::UserGlobal, user);
        config.set_layer(ConfigurationTarget::Memory, memory);

        let val: Option<i64> = config.get_value("editor.fontSize");
        assert_eq!(val, Some(20));
    }

    #[test]
    fn config_partial_overrides_preserve_siblings() {
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.fontSize", json!(14));
        defaults.set_value("editor.tabSize", json!(4));

        let mut user = ConfigurationModel::new();
        user.set_value("editor.fontSize", json!(16));

        let mut config = Configuration::with_defaults(defaults);
        config.set_layer(ConfigurationTarget::UserGlobal, user);

        let font: Option<i64> = config.get_value("editor.fontSize");
        assert_eq!(font, Some(16));
        let tab: Option<i64> = config.get_value("editor.tabSize");
        assert_eq!(tab, Some(4));
    }

    // -- Configuration: inspect ---------------------------------------------

    #[test]
    fn config_inspect_shows_layers() {
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.fontSize", json!(14));

        let mut user = ConfigurationModel::new();
        user.set_value("editor.fontSize", json!(16));

        let mut config = Configuration::with_defaults(defaults);
        config.set_layer(ConfigurationTarget::UserGlobal, user);

        let result = config.inspect("editor.fontSize");
        assert_eq!(result.default_value, Some(json!(14)));
        assert_eq!(result.user_value, Some(json!(16)));
        assert_eq!(result.workspace_value, None);
        assert_eq!(result.folder_value, None);
        assert_eq!(result.memory_value, None);
        assert_eq!(result.merged_value, Some(json!(16)));
    }

    // -- Configuration: update ----------------------------------------------

    #[test]
    fn config_update_writes_to_target() {
        let mut config = Configuration::new();
        config.update("editor.fontSize", json!(14), ConfigurationTarget::Default);
        config.update(
            "editor.fontSize",
            json!(16),
            ConfigurationTarget::UserGlobal,
        );

        let result = config.inspect("editor.fontSize");
        assert_eq!(result.default_value, Some(json!(14)));
        assert_eq!(result.user_value, Some(json!(16)));
        assert_eq!(result.merged_value, Some(json!(16)));
    }

    // -- ConfigurationRegistry ----------------------------------------------

    #[test]
    fn registry_register_and_get_defaults() {
        let mut registry = ConfigurationRegistry::new();

        let mut props = HashMap::new();
        props.insert(
            "fontSize".to_string(),
            ConfigurationPropertySchema {
                property_type: "number".into(),
                default: json!(14),
                description: "Font size in pixels".into(),
                scope: ConfigurationScope::Window,
            },
        );
        props.insert(
            "tabSize".to_string(),
            ConfigurationPropertySchema {
                property_type: "number".into(),
                default: json!(4),
                description: "Tab size in spaces".into(),
                scope: ConfigurationScope::Resource,
            },
        );

        registry.register_configuration("editor", props);
        assert_eq!(registry.len(), 2);

        let schema = registry.get_property("editor.fontSize").unwrap();
        assert_eq!(schema.default, json!(14));
        assert_eq!(schema.scope, ConfigurationScope::Window);

        let defaults = registry.get_defaults();
        let font: Option<i64> = defaults.get_value("editor.fontSize");
        assert_eq!(font, Some(14));
        let tab: Option<i64> = defaults.get_value("editor.tabSize");
        assert_eq!(tab, Some(4));
    }

    #[test]
    fn registry_empty_section() {
        let mut registry = ConfigurationRegistry::new();

        let mut props = HashMap::new();
        props.insert(
            "myGlobalSetting".to_string(),
            ConfigurationPropertySchema {
                property_type: "boolean".into(),
                default: json!(false),
                description: "A global setting".into(),
                scope: ConfigurationScope::Application,
            },
        );

        registry.register_configuration("", props);

        let schema = registry.get_property("myGlobalSetting").unwrap();
        assert_eq!(schema.default, json!(false));
    }

    // -- IConfigurationService via ConfigurationService ---------------------

    #[test]
    fn service_get_and_update() {
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.fontSize", json!(14));

        let config = Configuration::with_defaults(defaults);
        let service = ConfigurationService::new(config);

        let val: Option<i64> = service.get_value("editor.fontSize");
        assert_eq!(val, Some(14));

        service.update_value("editor.fontSize", json!(18), ConfigurationScope::Window);

        let val: Option<i64> = service.get_value("editor.fontSize");
        assert_eq!(val, Some(18));
    }

    #[test]
    fn service_scope_routes_to_correct_layer() {
        let config = Configuration::new();
        let service = ConfigurationService::new(config);

        service.update_value(
            "files.autoSave",
            json!("afterDelay"),
            ConfigurationScope::Resource,
        );

        let result = service.inspect("files.autoSave");
        // Resource scope writes to the folder layer
        assert_eq!(result.folder_value, Some(json!("afterDelay")));
        assert_eq!(result.user_value, None);
    }

    #[test]
    fn service_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ConfigurationService>();
    }

    // -- Full integration: registry → defaults → service --------------------

    #[test]
    fn full_integration() {
        let mut registry = ConfigurationRegistry::new();

        let mut editor_props = HashMap::new();
        editor_props.insert(
            "fontSize".to_string(),
            ConfigurationPropertySchema {
                property_type: "number".into(),
                default: json!(14),
                description: "Font size".into(),
                scope: ConfigurationScope::Window,
            },
        );
        editor_props.insert(
            "tabSize".to_string(),
            ConfigurationPropertySchema {
                property_type: "number".into(),
                default: json!(4),
                description: "Tab size".into(),
                scope: ConfigurationScope::Resource,
            },
        );
        editor_props.insert(
            "wordWrap".to_string(),
            ConfigurationPropertySchema {
                property_type: "string".into(),
                default: json!("off"),
                description: "Word wrap mode".into(),
                scope: ConfigurationScope::LanguageOverridable,
            },
        );
        registry.register_configuration("editor", editor_props);

        // Build defaults from the registry
        let defaults = registry.get_defaults();
        let config = Configuration::with_defaults(defaults);
        let service = ConfigurationService::new(config);

        // Verify defaults
        let font: Option<i64> = service.get_value("editor.fontSize");
        assert_eq!(font, Some(14));

        // User overrides font size
        service.update_value("editor.fontSize", json!(16), ConfigurationScope::Window);
        let font: Option<i64> = service.get_value("editor.fontSize");
        assert_eq!(font, Some(16));

        // Workspace override for word wrap (via JSONC parsing)
        let ws_settings = r#"{
            // Enable word wrap
            "editor": {
                "wordWrap": "on",
            },
        }"#;
        let ws_model = ConfigurationModel::from_jsonc(ws_settings).unwrap();
        service.set_layer(ConfigurationTarget::Workspace, ws_model);

        let wrap: Option<String> = service.get_value("editor.wordWrap");
        assert_eq!(wrap.as_deref(), Some("on"));

        // Tab size should still be the default
        let tab: Option<i64> = service.get_value("editor.tabSize");
        assert_eq!(tab, Some(4));

        // Inspect shows layers
        let result = service.inspect("editor.fontSize");
        assert_eq!(result.default_value, Some(json!(14)));
        assert_eq!(result.user_value, Some(json!(16)));
        assert_eq!(result.merged_value, Some(json!(16)));
    }

    // -- Dot-path edge cases ------------------------------------------------

    #[test]
    fn single_segment_path() {
        let mut model = ConfigurationModel::new();
        model.set_value("debug", json!(true));

        let val: Option<bool> = model.get_value("debug");
        assert_eq!(val, Some(true));
    }

    #[test]
    fn deeply_nested_path() {
        let mut model = ConfigurationModel::new();
        model.set_value("a.b.c.d.e", json!(42));

        let val: Option<i64> = model.get_value("a.b.c.d.e");
        assert_eq!(val, Some(42));
    }

    #[test]
    fn empty_section_returns_root() {
        let mut model = ConfigurationModel::new();
        model.set_value("a", json!(1));

        let raw = model.get_raw_value("").unwrap();
        assert!(raw.is_object());
    }

    #[test]
    fn get_value_type_mismatch_returns_none() {
        let mut model = ConfigurationModel::new();
        model.set_value("editor.fontSize", json!("not a number"));

        let val: Option<i64> = model.get_value("editor.fontSize");
        assert_eq!(val, None);
    }

    // -- Deep merge ---------------------------------------------------------

    #[test]
    fn deep_merge_nested_objects() {
        let mut base = ConfigurationModel::new();
        base.set_value("editor.font.family", json!("monospace"));
        base.set_value("editor.font.size", json!(14));

        let mut overlay = ConfigurationModel::new();
        overlay.set_value("editor.font.size", json!(16));
        overlay.set_value("editor.font.weight", json!("bold"));

        base.merge(&overlay);

        let family: Option<String> = base.get_value("editor.font.family");
        assert_eq!(family.as_deref(), Some("monospace"));
        let size: Option<i64> = base.get_value("editor.font.size");
        assert_eq!(size, Some(16));
        let weight: Option<String> = base.get_value("editor.font.weight");
        assert_eq!(weight.as_deref(), Some("bold"));
    }
}
