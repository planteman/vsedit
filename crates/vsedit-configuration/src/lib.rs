//! Settings registry and overrides for vsedit.
//!
//! Equivalent to VS Code's `vs/platform/configuration/common/configuration.ts`.
//! Manages user settings, workspace settings, and defaults with layered
//! merging and dot-notation path resolution.

use std::fmt;
use std::collections::HashMap;
use std::sync::RwLock;

use serde::de::DeserializeOwned;
use serde_json::Value;

pub use vsedit_json::parse_jsonc;

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
// SettingType — typed enum for schema types
// ---------------------------------------------------------------------------

/// The JSON-schema type of a configuration setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingType {
    String,
    Number,
    Boolean,
    Array,
    Object,
}

impl SettingType {
    /// Returns the JSON-schema type name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Array => "array",
            Self::Object => "object",
        }
    }
}

impl std::fmt::Display for SettingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// SettingSchema — rich schema for extension-registered settings
// ---------------------------------------------------------------------------

/// Schema for a single configuration setting, matching VS Code's
/// `IConfigurationPropertySchema`.
#[derive(Debug, Clone)]
pub struct SettingSchema {
    /// Fully-qualified dot-notation key (e.g. `"editor.fontSize"`).
    pub key: String,
    /// The JSON type of the setting.
    pub setting_type: SettingType,
    /// Default value.
    pub default: Value,
    /// Human-readable description.
    pub description: String,
    /// Optional list of allowed enum values.
    pub enum_values: Option<Vec<Value>>,
    /// Optional human-readable descriptions for each enum value.
    pub enum_descriptions: Option<Vec<String>>,
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
    /// Optional list of allowed enum values.
    pub enum_values: Option<Vec<Value>>,
    /// Optional human-readable descriptions for each enum value.
    pub enum_descriptions: Option<Vec<String>>,
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
    /// Settings registered via the `SettingSchema` API.
    settings: HashMap<String, SettingSchema>,
}

impl ConfigurationRegistry {
    /// Create an empty configuration registry.
    pub fn new() -> Self {
        Self {
            properties: HashMap::new(),
            settings: HashMap::new(),
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

    /// Register a single setting via its [`SettingSchema`].
    pub fn register_setting(&mut self, schema: SettingSchema) {
        // Also register into the property map for defaults generation.
        self.properties.insert(
            schema.key.clone(),
            ConfigurationPropertySchema {
                property_type: schema.setting_type.as_str().to_string(),
                default: schema.default.clone(),
                description: schema.description.clone(),
                scope: ConfigurationScope::Window,
                enum_values: schema.enum_values.clone(),
                enum_descriptions: schema.enum_descriptions.clone(),
            },
        );
        self.settings.insert(schema.key.clone(), schema);
    }

    /// Returns the [`SettingSchema`] for a key, if registered via
    /// [`register_setting`](Self::register_setting).
    pub fn get_schema(&self, key: &str) -> Option<&SettingSchema> {
        self.settings.get(key)
    }

    /// Returns all registered [`SettingSchema`] entries.
    pub fn all_schemas(&self) -> impl Iterator<Item = &SettingSchema> {
        self.settings.values()
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
    pub effective_value: Option<Value>,
}

// Keep legacy alias for backward compatibility.
impl InspectResult {
    /// Returns the effective merged value (alias for `effective_value`).
    pub fn merged_value(&self) -> Option<&Value> {
        self.effective_value.as_ref()
    }
}

// ---------------------------------------------------------------------------
// ConfigurationTarget
// ---------------------------------------------------------------------------

/// Target layer for a configuration update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigurationTarget {
    /// Write to the defaults layer.
    Default,
    /// Write to the user global layer (`~/.config/vsedit/settings.json`).
    User,
    /// Write to the workspace layer (`.vscode/settings.json`).
    Workspace,
    /// Write to the folder layer (per-folder `.vscode/settings.json`).
    WorkspaceFolder,
    /// Write to the in-memory layer (transient).
    Memory,
}

// Backward-compatible alias
impl ConfigurationTarget {
    /// Alias for `User` matching old `UserGlobal` name.
    pub const USER_GLOBAL: Self = Self::User;
    /// Alias for `WorkspaceFolder` matching old `Folder` name.
    pub const FOLDER: Self = Self::WorkspaceFolder;
}

// ---------------------------------------------------------------------------
// Configuration — merged multi-layer configuration
// ---------------------------------------------------------------------------

/// Merged configuration assembled from multiple layers.
///
/// Layers (in priority order, highest wins):
/// 1. Memory (transient overrides)
/// 2. WorkspaceFolder (per-folder settings)
/// 3. Workspace (`.vscode/settings.json`)
/// 4. User (`~/.config/vsedit/settings.json`)
/// 5. Defaults (from [`ConfigurationRegistry`])
pub struct Configuration {
    defaults: ConfigurationModel,
    user: ConfigurationModel,
    workspace: ConfigurationModel,
    workspace_folder: ConfigurationModel,
    memory: ConfigurationModel,
}

impl Configuration {
    /// Create a new configuration with all layers empty.
    pub fn new() -> Self {
        Self {
            defaults: ConfigurationModel::new(),
            user: ConfigurationModel::new(),
            workspace: ConfigurationModel::new(),
            workspace_folder: ConfigurationModel::new(),
            memory: ConfigurationModel::new(),
        }
    }

    /// Create a configuration seeded with a defaults layer.
    pub fn with_defaults(defaults: ConfigurationModel) -> Self {
        Self {
            defaults,
            user: ConfigurationModel::new(),
            workspace: ConfigurationModel::new(),
            workspace_folder: ConfigurationModel::new(),
            memory: ConfigurationModel::new(),
        }
    }

    /// Set a specific layer to the given model.
    pub fn set_layer(&mut self, target: ConfigurationTarget, model: ConfigurationModel) {
        match target {
            ConfigurationTarget::Default => self.defaults = model,
            ConfigurationTarget::User => self.user = model,
            ConfigurationTarget::Workspace => self.workspace = model,
            ConfigurationTarget::WorkspaceFolder => self.workspace_folder = model,
            ConfigurationTarget::Memory => self.memory = model,
        }
    }

    /// Get the effective (merged) value at a dot-notation path.
    pub fn get_value<T: DeserializeOwned>(&self, section: &str) -> Option<T> {
        let merged = self.merged_model();
        merged.get_value(section)
    }

    /// Get the effective raw JSON value at a dot-notation path, walking layers
    /// from most specific (Memory) to least specific (Default).
    pub fn get_effective_value(&self, key: &str) -> Option<Value> {
        // Walk layers from most specific to least
        for layer in &[
            &self.memory,
            &self.workspace_folder,
            &self.workspace,
            &self.user,
            &self.defaults,
        ] {
            if let Some(v) = layer.get_raw_value(key) {
                return Some(v.clone());
            }
        }
        None
    }

    /// Inspect a key, returning the value from each layer and the merged
    /// result.
    pub fn inspect(&self, section: &str) -> InspectResult {
        let merged = self.merged_model();
        InspectResult {
            default_value: self.defaults.get_raw_value(section).cloned(),
            user_value: self.user.get_raw_value(section).cloned(),
            workspace_value: self.workspace.get_raw_value(section).cloned(),
            folder_value: self.workspace_folder.get_raw_value(section).cloned(),
            memory_value: self.memory.get_raw_value(section).cloned(),
            effective_value: merged.get_raw_value(section).cloned(),
        }
    }

    /// Update a value in the specified target layer.
    /// Returns the set of affected dot-notation keys.
    pub fn update(&mut self, section: &str, value: Value, target: ConfigurationTarget) -> Vec<String> {
        let layer = match target {
            ConfigurationTarget::Default => &mut self.defaults,
            ConfigurationTarget::User => &mut self.user,
            ConfigurationTarget::Workspace => &mut self.workspace,
            ConfigurationTarget::WorkspaceFolder => &mut self.workspace_folder,
            ConfigurationTarget::Memory => &mut self.memory,
        };
        layer.set_value(section, value);
        vec![section.to_string()]
    }

    /// Build the merged model from all layers.
    fn merged_model(&self) -> ConfigurationModel {
        let mut merged = self.defaults.clone();
        merged.merge(&self.user);
        merged.merge(&self.workspace);
        merged.merge(&self.workspace_folder);
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
// ConfigurationChangeEvent
// ---------------------------------------------------------------------------

/// Event fired when configuration values change.
#[derive(Debug, Clone)]
pub struct ConfigurationChangeEvent {
    /// The set of affected dot-notation keys.
    pub affected_keys: Vec<String>,
    /// Which target layer changed.
    pub source: ConfigurationTarget,
}

impl ConfigurationChangeEvent {
    /// Returns `true` if the change affects the given section prefix.
    ///
    /// A change to `"editor.fontSize"` affects `"editor"`, `"editor.fontSize"`,
    /// and `"editor.fontSize.sub"` but not `"terminal"`.
    pub fn affects_configuration(&self, section: &str) -> bool {
        self.affected_keys.iter().any(|key| {
            key == section
                || key.starts_with(&format!("{section}."))
                || section.starts_with(&format!("{key}."))
        })
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
/// read-write lock, with change events.
pub struct ConfigurationService {
    inner: RwLock<Configuration>,
    change_emitter: vsedit_events::Emitter<ConfigurationChangeEvent>,
}

impl ConfigurationService {
    /// Create a service from an initial [`Configuration`].
    pub fn new(configuration: Configuration) -> Self {
        Self {
            inner: RwLock::new(configuration),
            change_emitter: vsedit_events::Emitter::new(),
        }
    }

    /// Returns the `onDidChangeConfiguration` event.
    pub fn on_did_change_configuration(&self) -> vsedit_events::Event<ConfigurationChangeEvent> {
        self.change_emitter.event()
    }

    /// Get the effective raw JSON value at a key (walks layers top-down).
    pub fn get_effective_value(&self, key: &str) -> Option<Value> {
        let guard = self.inner.read().unwrap();
        guard.get_effective_value(key)
    }

    /// Get a snapshot reference to inspect a value.
    pub fn inspect(&self, section: &str) -> InspectResult {
        let guard = self.inner.read().unwrap();
        guard.inspect(section)
    }

    /// Replace a full layer and fire a change event with all top-level keys.
    pub fn set_layer(&self, target: ConfigurationTarget, model: ConfigurationModel) {
        let keys = collect_leaf_keys(model.as_value(), "");
        {
            let mut guard = self.inner.write().unwrap();
            guard.set_layer(target, model);
        }
        if !keys.is_empty() {
            self.change_emitter.fire(&ConfigurationChangeEvent {
                affected_keys: keys,
                source: target,
            });
        }
    }

    /// Update a value in the specified target layer and fire a change event.
    pub fn update_value_at(
        &self,
        section: &str,
        value: Value,
        target: ConfigurationTarget,
    ) {
        {
            let mut guard = self.inner.write().unwrap();
            guard.update(section, value, target);
        }
        self.change_emitter.fire(&ConfigurationChangeEvent {
            affected_keys: vec![section.to_string()],
            source: target,
        });
    }
}

impl IConfigurationService for ConfigurationService {
    fn get_value<T: DeserializeOwned>(&self, section: &str) -> Option<T> {
        let guard = self.inner.read().unwrap();
        guard.get_value(section)
    }

    fn update_value(&self, section: &str, value: Value, scope: ConfigurationScope) {
        let target = scope_to_target(scope);
        {
            let mut guard = self.inner.write().unwrap();
            guard.update(section, value.clone(), target);
        }
        self.change_emitter.fire(&ConfigurationChangeEvent {
            affected_keys: vec![section.to_string()],
            source: target,
        });
    }
}

/// Map a [`ConfigurationScope`] to the [`ConfigurationTarget`] layer where
/// writes should land.
fn scope_to_target(scope: ConfigurationScope) -> ConfigurationTarget {
    match scope {
        ConfigurationScope::Application | ConfigurationScope::Machine => {
            ConfigurationTarget::User
        }
        ConfigurationScope::Window => ConfigurationTarget::User,
        ConfigurationScope::Resource => ConfigurationTarget::WorkspaceFolder,
        ConfigurationScope::LanguageOverridable => ConfigurationTarget::WorkspaceFolder,
    }
}

/// Collect all dot-notation leaf keys from a JSON value.
fn collect_leaf_keys(value: &Value, prefix: &str) -> Vec<String> {
    let mut keys = Vec::new();
    if let Value::Object(map) = value {
        for (k, v) in map {
            let full = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{prefix}.{k}")
            };
            if v.is_object() {
                keys.extend(collect_leaf_keys(v, &full));
            } else {
                keys.push(full);
            }
        }
    }
    keys
}

// ---------------------------------------------------------------------------
// Default VS Code settings registration
// ---------------------------------------------------------------------------

/// Register 30+ common VS Code compatible default settings into the registry.
pub fn register_default_settings(registry: &mut ConfigurationRegistry) {
    use serde_json::json;

    let settings: Vec<SettingSchema> = vec![
        // -- editor --
        SettingSchema {
            key: "editor.fontSize".into(),
            setting_type: SettingType::Number,
            default: json!(14),
            description: "Controls the font size in pixels.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.fontFamily".into(),
            setting_type: SettingType::String,
            default: json!("monospace"),
            description: "Controls the font family.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.tabSize".into(),
            setting_type: SettingType::Number,
            default: json!(4),
            description: "The number of spaces a tab is equal to.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.insertSpaces".into(),
            setting_type: SettingType::Boolean,
            default: json!(true),
            description: "Insert spaces when pressing Tab.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.wordWrap".into(),
            setting_type: SettingType::String,
            default: json!("off"),
            description: "Controls how lines should wrap.".into(),
            enum_values: Some(vec![json!("off"), json!("on"), json!("wordWrapColumn"), json!("bounded")]),
            enum_descriptions: Some(vec![
                "Lines will never wrap.".into(),
                "Lines will wrap at the viewport width.".into(),
                "Lines will wrap at wordWrapColumn.".into(),
                "Lines will wrap at the minimum of viewport and wordWrapColumn.".into(),
            ]),
        },
        SettingSchema {
            key: "editor.wordWrapColumn".into(),
            setting_type: SettingType::Number,
            default: json!(80),
            description: "Controls the wrapping column of the editor.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.minimap.enabled".into(),
            setting_type: SettingType::Boolean,
            default: json!(true),
            description: "Controls whether the minimap is shown.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.renderWhitespace".into(),
            setting_type: SettingType::String,
            default: json!("selection"),
            description: "Controls how the editor should render whitespace characters.".into(),
            enum_values: Some(vec![json!("none"), json!("boundary"), json!("selection"), json!("trailing"), json!("all")]),
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.cursorStyle".into(),
            setting_type: SettingType::String,
            default: json!("line"),
            description: "Controls the cursor style.".into(),
            enum_values: Some(vec![json!("line"), json!("block"), json!("underline"), json!("line-thin"), json!("block-outline"), json!("underline-thin")]),
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.cursorBlinking".into(),
            setting_type: SettingType::String,
            default: json!("blink"),
            description: "Controls the cursor animation style.".into(),
            enum_values: Some(vec![json!("blink"), json!("smooth"), json!("phase"), json!("expand"), json!("solid")]),
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.formatOnSave".into(),
            setting_type: SettingType::Boolean,
            default: json!(false),
            description: "Format a file on save.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.formatOnPaste".into(),
            setting_type: SettingType::Boolean,
            default: json!(false),
            description: "Format pasted content.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.formatOnType".into(),
            setting_type: SettingType::Boolean,
            default: json!(false),
            description: "Format the line after typing.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.autoClosingBrackets".into(),
            setting_type: SettingType::String,
            default: json!("languageDefined"),
            description: "Controls whether the editor should automatically close brackets.".into(),
            enum_values: Some(vec![json!("always"), json!("languageDefined"), json!("beforeWhitespace"), json!("never")]),
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.autoClosingQuotes".into(),
            setting_type: SettingType::String,
            default: json!("languageDefined"),
            description: "Controls whether the editor should automatically close quotes.".into(),
            enum_values: Some(vec![json!("always"), json!("languageDefined"), json!("beforeWhitespace"), json!("never")]),
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.lineNumbers".into(),
            setting_type: SettingType::String,
            default: json!("on"),
            description: "Controls the display of line numbers.".into(),
            enum_values: Some(vec![json!("off"), json!("on"), json!("relative"), json!("interval")]),
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.rulers".into(),
            setting_type: SettingType::Array,
            default: json!([]),
            description: "Render vertical rulers after a certain number of monospace characters.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.scrollBeyondLastLine".into(),
            setting_type: SettingType::Boolean,
            default: json!(true),
            description: "Controls whether the editor will scroll beyond the last line.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.detectIndentation".into(),
            setting_type: SettingType::Boolean,
            default: json!(true),
            description: "Controls whether editor.tabSize and editor.insertSpaces will be automatically detected.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "editor.trimAutoWhitespace".into(),
            setting_type: SettingType::Boolean,
            default: json!(true),
            description: "Remove trailing auto inserted whitespace.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        // -- files --
        SettingSchema {
            key: "files.autoSave".into(),
            setting_type: SettingType::String,
            default: json!("off"),
            description: "Controls auto save of editors that have unsaved changes.".into(),
            enum_values: Some(vec![json!("off"), json!("afterDelay"), json!("onFocusChange"), json!("onWindowChange")]),
            enum_descriptions: None,
        },
        SettingSchema {
            key: "files.autoSaveDelay".into(),
            setting_type: SettingType::Number,
            default: json!(1000),
            description: "Controls the delay in ms after which an editor with unsaved changes is saved automatically.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "files.encoding".into(),
            setting_type: SettingType::String,
            default: json!("utf8"),
            description: "The default character set encoding to use when reading and writing files.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "files.eol".into(),
            setting_type: SettingType::String,
            default: json!("auto"),
            description: "The default end of line character.".into(),
            enum_values: Some(vec![json!("\\n"), json!("\\r\\n"), json!("auto")]),
            enum_descriptions: None,
        },
        SettingSchema {
            key: "files.trimTrailingWhitespace".into(),
            setting_type: SettingType::Boolean,
            default: json!(false),
            description: "When enabled, will trim trailing whitespace when saving a file.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "files.insertFinalNewline".into(),
            setting_type: SettingType::Boolean,
            default: json!(false),
            description: "When enabled, insert a final new line at the end of the file when saving it.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "files.trimFinalNewlines".into(),
            setting_type: SettingType::Boolean,
            default: json!(false),
            description: "When enabled, will trim all new lines after the final new line at the end of the file when saving it.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        // -- workbench --
        SettingSchema {
            key: "workbench.colorTheme".into(),
            setting_type: SettingType::String,
            default: json!("Default Dark Modern"),
            description: "Specifies the color theme used in the workbench.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "workbench.iconTheme".into(),
            setting_type: SettingType::String,
            default: json!("vs-seti"),
            description: "Specifies the file icon theme used in the workbench.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "workbench.startupEditor".into(),
            setting_type: SettingType::String,
            default: json!("welcomePage"),
            description: "Controls which editor is shown at startup.".into(),
            enum_values: Some(vec![json!("none"), json!("welcomePage"), json!("readme"), json!("newUntitledFile"), json!("welcomePageInEmptyWorkbench")]),
            enum_descriptions: None,
        },
        SettingSchema {
            key: "workbench.editor.showTabs".into(),
            setting_type: SettingType::String,
            default: json!("multiple"),
            description: "Controls whether opened editors should show as individual tabs.".into(),
            enum_values: Some(vec![json!("multiple"), json!("single"), json!("none")]),
            enum_descriptions: None,
        },
        // -- terminal --
        SettingSchema {
            key: "terminal.integrated.shell.linux".into(),
            setting_type: SettingType::String,
            default: json!("/bin/bash"),
            description: "The path of the shell that the terminal uses on Linux.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "terminal.integrated.fontSize".into(),
            setting_type: SettingType::Number,
            default: json!(14),
            description: "Controls the font size in pixels of the terminal.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "terminal.integrated.fontFamily".into(),
            setting_type: SettingType::String,
            default: json!("monospace"),
            description: "Controls the font family of the terminal.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
        SettingSchema {
            key: "terminal.integrated.cursorStyle".into(),
            setting_type: SettingType::String,
            default: json!("block"),
            description: "Controls the style of terminal cursor.".into(),
            enum_values: Some(vec![json!("block"), json!("underline"), json!("line")]),
            enum_descriptions: None,
        },
        // -- search --
        SettingSchema {
            key: "search.exclude".into(),
            setting_type: SettingType::Object,
            default: json!({"**/node_modules": true, "**/bower_components": true}),
            description: "Configure glob patterns for excluding files and folders in searches.".into(),
            enum_values: None,
            enum_descriptions: None,
        },
    ];

    for schema in settings {
        registry.register_setting(schema);
    }
}

// ---------------------------------------------------------------------------
// Config file I/O
// ---------------------------------------------------------------------------

use std::path::PathBuf;

/// Return the default configuration directory (`~/.config/vsedit`).
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("vsedit")
}

/// Load user settings from the default config path.
///
/// Returns an empty JSON object if the file does not exist.
pub fn load_user_settings() -> Result<Value, Box<dyn std::error::Error>> {
    let settings_path = config_dir().join("settings.json");
    load_json_file(&settings_path)
}

/// Save user settings to the default config path.
///
/// Creates the config directory if it does not already exist.
pub fn save_user_settings(settings: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let settings_path = dir.join("settings.json");
    let content = serde_json::to_string_pretty(settings)?;
    std::fs::write(settings_path, content)?;
    Ok(())
}

/// Load user keybindings from the default config path.
///
/// Returns an empty `Vec` if the file does not exist.
pub fn load_user_keybindings() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let keybindings_path = config_dir().join("keybindings.json");
    if keybindings_path.exists() {
        let content = std::fs::read_to_string(&keybindings_path)?;
        let cleaned = vsedit_json::strip_comments(&content);
        Ok(serde_json::from_str(&cleaned)?)
    } else {
        Ok(Vec::new())
    }
}

/// Load a JSON or JSONC file into a `serde_json::Value`.
///
/// Returns an empty JSON object when the file does not exist.
pub fn load_json_file(path: &std::path::Path) -> Result<Value, Box<dyn std::error::Error>> {
    if path.exists() {
        let content = std::fs::read_to_string(path)?;
        let cleaned = vsedit_json::strip_comments(&content);
        Ok(serde_json::from_str(&cleaned)?)
    } else {
        Ok(Value::Object(Default::default()))
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
    use std::sync::Arc;

    fn prop(ptype: &str, default: Value, desc: &str, scope: ConfigurationScope) -> ConfigurationPropertySchema {
        ConfigurationPropertySchema {
            property_type: ptype.into(),
            default,
            description: desc.into(),
            scope,
            enum_values: None,
            enum_descriptions: None,
        }
    }

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
        config.set_layer(ConfigurationTarget::User, user);

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
        config.set_layer(ConfigurationTarget::User, user);
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
        config.set_layer(ConfigurationTarget::User, user);
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
        config.set_layer(ConfigurationTarget::User, user);

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
        config.set_layer(ConfigurationTarget::User, user);

        let result = config.inspect("editor.fontSize");
        assert_eq!(result.default_value, Some(json!(14)));
        assert_eq!(result.user_value, Some(json!(16)));
        assert_eq!(result.workspace_value, None);
        assert_eq!(result.folder_value, None);
        assert_eq!(result.memory_value, None);
        assert_eq!(result.effective_value, Some(json!(16)));
    }

    // -- Configuration: update ----------------------------------------------

    #[test]
    fn config_update_writes_to_target() {
        let mut config = Configuration::new();
        config.update("editor.fontSize", json!(14), ConfigurationTarget::Default);
        config.update(
            "editor.fontSize",
            json!(16),
            ConfigurationTarget::User,
        );

        let result = config.inspect("editor.fontSize");
        assert_eq!(result.default_value, Some(json!(14)));
        assert_eq!(result.user_value, Some(json!(16)));
        assert_eq!(result.effective_value, Some(json!(16)));
    }

    // -- Configuration: get_effective_value ---------------------------------

    #[test]
    fn config_get_effective_value_walks_layers() {
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.fontSize", json!(14));
        defaults.set_value("editor.tabSize", json!(4));

        let mut user = ConfigurationModel::new();
        user.set_value("editor.fontSize", json!(16));

        let mut config = Configuration::with_defaults(defaults);
        config.set_layer(ConfigurationTarget::User, user);

        assert_eq!(config.get_effective_value("editor.fontSize"), Some(json!(16)));
        assert_eq!(config.get_effective_value("editor.tabSize"), Some(json!(4)));
        assert_eq!(config.get_effective_value("nonexistent"), None);
    }

    #[test]
    fn config_get_effective_value_workspace_folder_wins() {
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.fontSize", json!(14));

        let mut ws = ConfigurationModel::new();
        ws.set_value("editor.fontSize", json!(18));

        let mut folder = ConfigurationModel::new();
        folder.set_value("editor.fontSize", json!(20));

        let mut config = Configuration::with_defaults(defaults);
        config.set_layer(ConfigurationTarget::Workspace, ws);
        config.set_layer(ConfigurationTarget::WorkspaceFolder, folder);

        assert_eq!(config.get_effective_value("editor.fontSize"), Some(json!(20)));
    }

    // -- ConfigurationRegistry ----------------------------------------------

    #[test]
    fn registry_register_and_get_defaults() {
        let mut registry = ConfigurationRegistry::new();

        let mut props = HashMap::new();
        props.insert("fontSize".to_string(), prop("number", json!(14), "Font size in pixels", ConfigurationScope::Window));
        props.insert("tabSize".to_string(), prop("number", json!(4), "Tab size in spaces", ConfigurationScope::Resource));

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
        props.insert("myGlobalSetting".to_string(), prop("boolean", json!(false), "A global setting", ConfigurationScope::Application));

        registry.register_configuration("", props);

        let schema = registry.get_property("myGlobalSetting").unwrap();
        assert_eq!(schema.default, json!(false));
    }

    // -- SettingSchema registration -----------------------------------------

    #[test]
    fn register_setting_schema() {
        let mut registry = ConfigurationRegistry::new();
        registry.register_setting(SettingSchema {
            key: "editor.fontSize".into(),
            setting_type: SettingType::Number,
            default: json!(14),
            description: "Font size".into(),
            enum_values: None,
            enum_descriptions: None,
        });

        let schema = registry.get_schema("editor.fontSize").unwrap();
        assert_eq!(schema.setting_type, SettingType::Number);
        assert_eq!(schema.default, json!(14));

        // Also available as a property
        let prop = registry.get_property("editor.fontSize").unwrap();
        assert_eq!(prop.default, json!(14));
    }

    #[test]
    fn register_setting_with_enum_values() {
        let mut registry = ConfigurationRegistry::new();
        registry.register_setting(SettingSchema {
            key: "editor.wordWrap".into(),
            setting_type: SettingType::String,
            default: json!("off"),
            description: "Word wrap".into(),
            enum_values: Some(vec![json!("off"), json!("on"), json!("bounded")]),
            enum_descriptions: Some(vec!["No wrap".into(), "Wrap".into(), "Bounded".into()]),
        });

        let schema = registry.get_schema("editor.wordWrap").unwrap();
        assert_eq!(schema.enum_values.as_ref().unwrap().len(), 3);
        assert_eq!(schema.enum_descriptions.as_ref().unwrap()[0], "No wrap");
    }

    #[test]
    fn all_schemas_iterator() {
        let mut registry = ConfigurationRegistry::new();
        registry.register_setting(SettingSchema {
            key: "a.b".into(),
            setting_type: SettingType::Boolean,
            default: json!(true),
            description: "test".into(),
            enum_values: None,
            enum_descriptions: None,
        });
        registry.register_setting(SettingSchema {
            key: "c.d".into(),
            setting_type: SettingType::String,
            default: json!("x"),
            description: "test2".into(),
            enum_values: None,
            enum_descriptions: None,
        });

        let schemas: Vec<_> = registry.all_schemas().collect();
        assert_eq!(schemas.len(), 2);
    }

    #[test]
    fn setting_type_as_str() {
        assert_eq!(SettingType::String.as_str(), "string");
        assert_eq!(SettingType::Number.as_str(), "number");
        assert_eq!(SettingType::Boolean.as_str(), "boolean");
        assert_eq!(SettingType::Array.as_str(), "array");
        assert_eq!(SettingType::Object.as_str(), "object");
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

    // -- ConfigurationService: get_effective_value --------------------------

    #[test]
    fn service_get_effective_value() {
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.fontSize", json!(14));

        let config = Configuration::with_defaults(defaults);
        let service = ConfigurationService::new(config);

        assert_eq!(service.get_effective_value("editor.fontSize"), Some(json!(14)));
        assert_eq!(service.get_effective_value("nonexistent"), None);
    }

    // -- ConfigurationChangeEvent -------------------------------------------

    #[test]
    fn change_event_affects_configuration() {
        let event = ConfigurationChangeEvent {
            affected_keys: vec!["editor.fontSize".into()],
            source: ConfigurationTarget::User,
        };

        assert!(event.affects_configuration("editor"));
        assert!(event.affects_configuration("editor.fontSize"));
        assert!(!event.affects_configuration("terminal"));
        assert!(!event.affects_configuration("editor.tabSize"));
    }

    #[test]
    fn change_event_affects_parent_section() {
        let event = ConfigurationChangeEvent {
            affected_keys: vec!["editor".into()],
            source: ConfigurationTarget::Workspace,
        };

        assert!(event.affects_configuration("editor"));
        assert!(event.affects_configuration("editor.fontSize"));
        assert!(!event.affects_configuration("files"));
    }

    #[test]
    fn change_event_multiple_keys() {
        let event = ConfigurationChangeEvent {
            affected_keys: vec!["editor.fontSize".into(), "files.autoSave".into()],
            source: ConfigurationTarget::User,
        };

        assert!(event.affects_configuration("editor"));
        assert!(event.affects_configuration("files"));
        assert!(!event.affects_configuration("terminal"));
    }

    // -- ConfigurationService: change events --------------------------------

    #[test]
    fn service_fires_change_event_on_update() {
        let config = Configuration::new();
        let service = Arc::new(ConfigurationService::new(config));

        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);

        let event = service.on_did_change_configuration();
        let _handle = event.on(move |e: &ConfigurationChangeEvent| {
            events_clone.lock().unwrap().push(e.clone());
        });

        service.update_value("editor.fontSize", json!(16), ConfigurationScope::Window);

        let captured = events.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert!(captured[0].affects_configuration("editor.fontSize"));
        assert_eq!(captured[0].source, ConfigurationTarget::User);
    }

    #[test]
    fn service_fires_change_event_on_set_layer() {
        let config = Configuration::new();
        let service = ConfigurationService::new(config);

        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);

        let event = service.on_did_change_configuration();
        let _handle = event.on(move |e: &ConfigurationChangeEvent| {
            events_clone.lock().unwrap().push(e.clone());
        });

        let mut ws = ConfigurationModel::new();
        ws.set_value("editor.wordWrap", json!("on"));
        service.set_layer(ConfigurationTarget::Workspace, ws);

        let captured = events.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert!(captured[0].affects_configuration("editor.wordWrap"));
    }

    #[test]
    fn service_update_value_at() {
        let config = Configuration::new();
        let service = ConfigurationService::new(config);

        service.update_value_at("editor.fontSize", json!(20), ConfigurationTarget::Workspace);

        let result = service.inspect("editor.fontSize");
        assert_eq!(result.workspace_value, Some(json!(20)));
    }

    // -- Full integration: registry → defaults → service --------------------

    #[test]
    fn full_integration() {
        let mut registry = ConfigurationRegistry::new();

        let mut editor_props = HashMap::new();
        editor_props.insert("fontSize".to_string(), prop("number", json!(14), "Font size", ConfigurationScope::Window));
        editor_props.insert("tabSize".to_string(), prop("number", json!(4), "Tab size", ConfigurationScope::Resource));
        editor_props.insert("wordWrap".to_string(), prop("string", json!("off"), "Word wrap mode", ConfigurationScope::LanguageOverridable));
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
        assert_eq!(result.effective_value, Some(json!(16)));
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

    // -- Config file I/O ----------------------------------------------------

    #[test]
    fn load_json_file_missing_returns_empty_object() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let val = load_json_file(&path).unwrap();
        assert_eq!(val, json!({}));
    }

    #[test]
    fn load_json_file_with_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{
                // editor font size
                "editor.fontSize": 16,
                /* tab size */
                "editor.tabSize": 2
            }"#,
        )
        .unwrap();
        let val = load_json_file(&path).unwrap();
        assert_eq!(val["editor.fontSize"], json!(16));
        assert_eq!(val["editor.tabSize"], json!(2));
    }

    #[test]
    fn save_and_load_settings_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("settings.json");
        let settings = json!({"editor.fontSize": 18, "editor.tabSize": 4});
        let content = serde_json::to_string_pretty(&settings).unwrap();
        std::fs::write(&config_path, &content).unwrap();

        let loaded = load_json_file(&config_path).unwrap();
        assert_eq!(loaded, settings);
    }

    #[test]
    fn load_keybindings_missing_returns_empty_vec() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keybindings.json");
        assert!(!path.exists());
        let result: Vec<Value> = if path.exists() {
            let content = std::fs::read_to_string(&path).unwrap();
            let cleaned = vsedit_json::strip_comments(&content);
            serde_json::from_str(&cleaned).unwrap()
        } else {
            Vec::new()
        };
        assert!(result.is_empty());
    }

    #[test]
    fn load_keybindings_with_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keybindings.json");
        std::fs::write(
            &path,
            r#"[
                // Open file
                {"key": "ctrl+o", "command": "workbench.action.files.openFile"},
                /* Save */
                {"key": "ctrl+s", "command": "workbench.action.files.save"}
            ]"#,
        )
        .unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let cleaned = vsedit_json::strip_comments(&content);
        let bindings: Vec<Value> = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0]["key"], json!("ctrl+o"));
        assert_eq!(bindings[1]["command"], json!("workbench.action.files.save"));
    }

    #[test]
    fn config_dir_ends_with_vsedit() {
        let dir = config_dir();
        assert_eq!(dir.file_name().unwrap(), "vsedit");
    }

    // -- JSONC re-export ----------------------------------------------------

    #[test]
    fn parse_jsonc_reexport_works() {
        let val = parse_jsonc(r#"{ /* comment */ "a": 1 }"#).unwrap();
        assert_eq!(val["a"], json!(1));
    }

    // -- Default settings ---------------------------------------------------

    #[test]
    fn register_default_settings_count() {
        let mut registry = ConfigurationRegistry::new();
        register_default_settings(&mut registry);
        assert!(registry.len() >= 30, "expected at least 30 settings, got {}", registry.len());
    }

    #[test]
    fn default_settings_have_expected_keys() {
        let mut registry = ConfigurationRegistry::new();
        register_default_settings(&mut registry);

        let defaults = registry.get_defaults();
        assert_eq!(defaults.get_value::<i64>("editor.fontSize"), Some(14));
        assert_eq!(defaults.get_value::<i64>("editor.tabSize"), Some(4));
        assert_eq!(defaults.get_value::<bool>("editor.insertSpaces"), Some(true));
        assert_eq!(defaults.get_value::<String>("editor.wordWrap").as_deref(), Some("off"));
        assert_eq!(defaults.get_value::<bool>("editor.minimap.enabled"), Some(true));
        assert_eq!(defaults.get_value::<String>("editor.cursorStyle").as_deref(), Some("line"));
        assert_eq!(defaults.get_value::<bool>("editor.formatOnSave"), Some(false));
        assert_eq!(defaults.get_value::<bool>("editor.formatOnPaste"), Some(false));
        assert_eq!(defaults.get_value::<String>("files.autoSave").as_deref(), Some("off"));
        assert_eq!(defaults.get_value::<String>("files.encoding").as_deref(), Some("utf8"));
        assert_eq!(defaults.get_value::<bool>("files.trimTrailingWhitespace"), Some(false));
        assert_eq!(defaults.get_value::<String>("workbench.colorTheme").as_deref(), Some("Default Dark Modern"));
        assert_eq!(defaults.get_value::<String>("workbench.iconTheme").as_deref(), Some("vs-seti"));
        assert_eq!(defaults.get_value::<String>("terminal.integrated.shell.linux").as_deref(), Some("/bin/bash"));
        assert_eq!(defaults.get_value::<i64>("terminal.integrated.fontSize"), Some(14));
    }

    #[test]
    fn default_settings_schemas_accessible() {
        let mut registry = ConfigurationRegistry::new();
        register_default_settings(&mut registry);

        let schema = registry.get_schema("editor.wordWrap").unwrap();
        assert_eq!(schema.setting_type, SettingType::String);
        assert!(schema.enum_values.is_some());
        assert!(schema.enum_values.as_ref().unwrap().contains(&json!("on")));
    }

    #[test]
    fn default_settings_integration_with_service() {
        let mut registry = ConfigurationRegistry::new();
        register_default_settings(&mut registry);
        let defaults = registry.get_defaults();
        let config = Configuration::with_defaults(defaults);
        let service = ConfigurationService::new(config);

        // Verify a default
        let font: Option<i64> = service.get_value("editor.fontSize");
        assert_eq!(font, Some(14));

        // Override with user setting
        service.update_value("editor.fontSize", json!(20), ConfigurationScope::Window);
        let font: Option<i64> = service.get_value("editor.fontSize");
        assert_eq!(font, Some(20));

        // Inspect still shows default
        let result = service.inspect("editor.fontSize");
        assert_eq!(result.default_value, Some(json!(14)));
        assert_eq!(result.user_value, Some(json!(20)));
    }

    // -- InspectResult: all five layers -------------------------------------

    #[test]
    fn inspect_all_layers() {
        let mut config = Configuration::new();
        config.update("a", json!(1), ConfigurationTarget::Default);
        config.update("a", json!(2), ConfigurationTarget::User);
        config.update("a", json!(3), ConfigurationTarget::Workspace);
        config.update("a", json!(4), ConfigurationTarget::WorkspaceFolder);
        config.update("a", json!(5), ConfigurationTarget::Memory);

        let r = config.inspect("a");
        assert_eq!(r.default_value, Some(json!(1)));
        assert_eq!(r.user_value, Some(json!(2)));
        assert_eq!(r.workspace_value, Some(json!(3)));
        assert_eq!(r.folder_value, Some(json!(4)));
        assert_eq!(r.memory_value, Some(json!(5)));
        assert_eq!(r.effective_value, Some(json!(5)));
    }

    #[test]
    fn inspect_merged_value_alias() {
        let mut config = Configuration::new();
        config.update("x", json!(42), ConfigurationTarget::Default);
        let r = config.inspect("x");
        assert_eq!(r.merged_value(), Some(&json!(42)));
    }

    // -- ConfigurationTarget constants --------------------------------------

    #[test]
    fn target_aliases() {
        assert_eq!(ConfigurationTarget::USER_GLOBAL, ConfigurationTarget::User);
        assert_eq!(ConfigurationTarget::FOLDER, ConfigurationTarget::WorkspaceFolder);
    }

    // -- collect_leaf_keys --------------------------------------------------

    #[test]
    fn collect_leaf_keys_basic() {
        let val = json!({"editor": {"fontSize": 14, "tabSize": 4}, "files": {"autoSave": "off"}});
        let mut keys = collect_leaf_keys(&val, "");
        keys.sort();
        assert_eq!(keys, vec!["editor.fontSize", "editor.tabSize", "files.autoSave"]);
    }
}
