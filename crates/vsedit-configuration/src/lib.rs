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

// ---------------------------------------------------------------------------
// ConfigurationOverrideScope – workspace→folder→language overrides
// ---------------------------------------------------------------------------

/// Scope levels for configuration overrides, ordered from broadest to narrowest.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConfigurationOverrideScope {
    Workspace,
    Folder(String),
    Language(String),
}

impl ConfigurationOverrideScope {
    /// Returns a numeric priority (higher = more specific).
    pub fn priority(&self) -> u8 {
        match self {
            Self::Workspace => 0,
            Self::Folder(_) => 1,
            Self::Language(_) => 2,
        }
    }

    /// Check if this scope is more specific than `other`.
    pub fn overrides(&self, other: &Self) -> bool {
        self.priority() > other.priority()
    }
}

impl fmt::Display for ConfigurationOverrideScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace => write!(f, "workspace"),
            Self::Folder(p) => write!(f, "folder:{}", p),
            Self::Language(l) => write!(f, "[{}]", l),
        }
    }
}

/// An override entry: a scope plus the value to apply.
#[derive(Debug, Clone)]
pub struct ConfigurationOverride {
    pub scope: ConfigurationOverrideScope,
    pub key: String,
    pub value: Value,
}

/// Resolve a configuration key through a chain of overrides. The most specific
/// scope wins. Returns the resolved value, or `None` if no override matches.
pub fn resolve_override(overrides: &[ConfigurationOverride], key: &str) -> Option<Value> {
    let mut best: Option<&ConfigurationOverride> = None;
    for ov in overrides {
        if ov.key == key {
            match &best {
                Some(b) if ov.scope.overrides(&b.scope) => best = Some(ov),
                None => best = Some(ov),
                _ => {}
            }
        }
    }
    best.map(|o| o.value.clone())
}

// ---------------------------------------------------------------------------
// ConfigurationLock – concurrent access wrapper
// ---------------------------------------------------------------------------

/// A configuration store protected by a read-write lock for concurrent access.
pub struct ConfigurationLock {
    inner: RwLock<HashMap<String, Value>>,
}

impl ConfigurationLock {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Read a configuration value.
    pub fn get(&self, key: &str) -> Option<Value> {
        let guard = self.inner.read().ok()?;
        guard.get(key).cloned()
    }

    /// Write a configuration value.
    pub fn set(&self, key: impl Into<String>, value: Value) -> bool {
        match self.inner.write() {
            Ok(mut guard) => {
                guard.insert(key.into(), value);
                true
            }
            Err(_) => false,
        }
    }

    /// Remove a configuration value.
    pub fn remove(&self, key: &str) -> Option<Value> {
        let mut guard = self.inner.write().ok()?;
        guard.remove(key)
    }

    /// Snapshot all keys.
    pub fn keys(&self) -> Vec<String> {
        match self.inner.read() {
            Ok(guard) => guard.keys().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }
}

impl Default for ConfigurationLock {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ConfigurationChangeDebouncer
// ---------------------------------------------------------------------------

/// Batches configuration changes so that listeners are not notified on every
/// keystroke. Collects changed keys and provides a `flush` method.
#[derive(Debug, Clone)]
pub struct ConfigurationChangeDebouncer {
    pending_keys: Vec<String>,
    debounce_ms: u64,
}

impl ConfigurationChangeDebouncer {
    pub fn new(debounce_ms: u64) -> Self {
        Self {
            pending_keys: Vec::new(),
            debounce_ms,
        }
    }

    /// Record a changed key.
    pub fn record(&mut self, key: impl Into<String>) {
        let k = key.into();
        if !self.pending_keys.contains(&k) {
            self.pending_keys.push(k);
        }
    }

    /// Flush all pending keys, returning them and clearing the buffer.
    pub fn flush(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_keys)
    }

    /// True if there are pending changes.
    pub fn has_pending(&self) -> bool {
        !self.pending_keys.is_empty()
    }

    /// Current debounce delay.
    pub fn debounce_ms(&self) -> u64 {
        self.debounce_ms
    }

    /// Number of pending keys.
    pub fn pending_count(&self) -> usize {
        self.pending_keys.len()
    }
}

// ---------------------------------------------------------------------------
// Configuration inheritance chain builder
// ---------------------------------------------------------------------------

/// An entry in the inheritance chain with a label and its values.
#[derive(Debug, Clone)]
pub struct InheritanceLayer {
    pub label: String,
    pub values: HashMap<String, Value>,
}

/// Builds a merged configuration by applying layers in order (later layers override earlier ones).
pub struct InheritanceChainBuilder {
    layers: Vec<InheritanceLayer>,
}

impl InheritanceChainBuilder {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Add a layer to the end of the chain (highest priority).
    pub fn add_layer(&mut self, label: impl Into<String>, values: HashMap<String, Value>) {
        self.layers.push(InheritanceLayer {
            label: label.into(),
            values,
        });
    }

    /// Resolve a single key through the chain (last layer wins).
    pub fn resolve(&self, key: &str) -> Option<Value> {
        for layer in self.layers.iter().rev() {
            if let Some(v) = layer.values.get(key) {
                return Some(v.clone());
            }
        }
        None
    }

    /// Build a merged configuration from all layers.
    pub fn build(&self) -> HashMap<String, Value> {
        let mut result = HashMap::new();
        for layer in &self.layers {
            for (k, v) in &layer.values {
                result.insert(k.clone(), v.clone());
            }
        }
        result
    }

    /// The labels of all layers in order.
    pub fn layer_names(&self) -> Vec<&str> {
        self.layers.iter().map(|l| l.label.as_str()).collect()
    }

    /// Number of layers.
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
}

impl Default for InheritanceChainBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// ConfigProfileSwitcher - config profile switcher
// ---------------------------------------------------------------------------

/// Severity level for config profile switcher issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigProfileSwitcherSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ConfigProfileSwitcherSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [ConfigProfileSwitcher].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigProfileSwitcherEntry {
    pub id: String,
    pub label: String,
    pub severity: ConfigProfileSwitcherSeverity,
    pub detail: Option<String>,
    pub profile_count: usize,
    enabled: bool,
}

impl ConfigProfileSwitcherEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: ConfigProfileSwitcherSeverity::Low,
            detail: None,
            profile_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: ConfigProfileSwitcherSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_profile_count(mut self, val: usize) -> Self {
        self.profile_count = val;
        self
    }

    pub fn is_active(&self) -> bool {
        self.enabled && self.severity >= ConfigProfileSwitcherSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.profile_count, det)
    }
}

impl fmt::Display for ConfigProfileSwitcherEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [ConfigProfileSwitcherEntry] items.
#[derive(Debug, Clone)]
pub struct ConfigProfileSwitcher {
    entries: Vec<ConfigProfileSwitcherEntry>,
    name: String,
    capacity: usize,
}

impl ConfigProfileSwitcher {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: ConfigProfileSwitcherEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<ConfigProfileSwitcherEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&ConfigProfileSwitcherEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn profile_count(&self) -> usize { self.entries.len() }

    pub fn is_active(&self) -> bool {
        self.entries.iter().any(|e| e.is_active())
    }

    pub fn entries_by_severity(&self, severity: ConfigProfileSwitcherSeverity) -> Vec<&ConfigProfileSwitcherEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= ConfigProfileSwitcherSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&ConfigProfileSwitcherEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&ConfigProfileSwitcherEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// ConfigValidationReport - config validation report
// ---------------------------------------------------------------------------

/// Configuration for [ConfigValidationReport].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValidationReportConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub error_count: usize,
}

impl ConfigValidationReportConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, error_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_error_count(mut self, val: usize) -> Self { self.error_count = val; self }
}

impl Default for ConfigValidationReportConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ConfigValidationReport].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValidationReportItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ConfigValidationReportItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn is_valid(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ConfigValidationReportItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ConfigValidationReportItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ConfigValidationReport {
    config: ConfigValidationReportConfig,
    items: Vec<ConfigValidationReportItem>,
}

impl ConfigValidationReport {
    pub fn new(config: ConfigValidationReportConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ConfigValidationReportItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ConfigValidationReportItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ConfigValidationReportItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn error_count(&self) -> usize { self.items.len() }

    pub fn is_valid(&self) -> bool {
        self.items.iter().any(|i| i.is_valid())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ConfigValidationReportItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ConfigValidationReportItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ConfigValidationReportConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// configuration – Data validation and analysis helpers
// ---------------------------------------------------------------------------

/// Result of validating a value against a schema-like rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XConfigurationValidationResult {
    Ok,
    Error(String),
    Warning(String),
}

impl XConfigurationValidationResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Ok => None,
            Self::Error(m) | Self::Warning(m) => Some(m),
        }
    }
}

/// A key-value pair with optional metadata tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XConfigurationTaggedEntry {
    pub key: String,
    pub value: String,
    pub tag: Option<String>,
}

impl XConfigurationTaggedEntry {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self { key: key.into(), value: value.into(), tag: None }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn matches_tag(&self, tag: &str) -> bool {
        self.tag.as_deref() == Some(tag)
    }
}

/// Validate that a string is non-empty and within a max length.
pub fn x_configuration_validate_string(value: &str, max_len: usize) -> XConfigurationValidationResult {
    if value.is_empty() {
        return XConfigurationValidationResult::Error("value must not be empty".into());
    }
    if value.len() > max_len {
        return XConfigurationValidationResult::Error(
            format!("value exceeds max length of {max_len}"),
        );
    }
    XConfigurationValidationResult::Ok
}

/// Validate that a number falls within an inclusive range.
pub fn x_configuration_validate_range(value: i64, min: i64, max: i64) -> XConfigurationValidationResult {
    if value < min || value > max {
        XConfigurationValidationResult::Error(
            format!("{value} is outside range [{min}, {max}]"),
        )
    } else {
        XConfigurationValidationResult::Ok
    }
}

/// Filter entries by tag, returning only matching ones.
pub fn x_configuration_filter_by_tag<'a>(
    entries: &'a [XConfigurationTaggedEntry],
    tag: &str,
) -> Vec<&'a XConfigurationTaggedEntry> {
    entries.iter().filter(|e| e.matches_tag(tag)).collect()
}

/// Group entries by their tag (entries without a tag go under `"_untagged"`).
pub fn x_configuration_group_by_tag(
    entries: &[XConfigurationTaggedEntry],
) -> std::collections::HashMap<String, Vec<&XConfigurationTaggedEntry>> {
    let mut map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for e in entries {
        let key = e.tag.clone().unwrap_or_else(|| "_untagged".into());
        map.entry(key).or_default().push(e);
    }
    map
}

/// Compute a simple digest of a string (DJB2 hash).
pub fn x_configuration_djb2_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for b in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

/// Deduplicate entries by key, keeping the first occurrence.
pub fn x_configuration_dedup_entries(entries: Vec<XConfigurationTaggedEntry>) -> Vec<XConfigurationTaggedEntry> {
    let mut seen = std::collections::HashSet::new();
    entries.into_iter().filter(|e| seen.insert(e.key.clone())).collect()
}



// ---------------------------------------------------------------------------
// configuration – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for configuration management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YConfigurationConfigSource {
    Default,
    User,
    Workspace,
    Remote,
}

impl YConfigurationConfigSource {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Default => 0,
            Self::User => 1,
            Self::Workspace => 2,
            Self::Remote => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::User => "User",
            Self::Workspace => "Workspace",
            Self::Remote => "Remote",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YConfigurationConfigSource] {
        &[
            YConfigurationConfigSource::Default,
            YConfigurationConfigSource::User,
            YConfigurationConfigSource::Workspace,
            YConfigurationConfigSource::Remote,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YConfigurationConfigSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks config override data.
#[derive(Debug, Clone)]
pub struct YConfigurationConfigOverride {
    pub key: String,
    pub value: String,
    pub source: String,
}

impl YConfigurationConfigOverride {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            key: String::new(),
            value: String::new(),
            source: String::new(),
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YConfigurationConfigOverride({}: {:?})", "key", self.key)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_configuration_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_configuration_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_configuration_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_configuration_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_configuration_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_configuration_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_configuration_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_configuration_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// configuration – Extended config validation helpers
// ---------------------------------------------------------------------------

/// Priority levels for config validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZConfigurationPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZConfigurationPriority {
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
    pub fn all_asc() -> [ZConfigurationPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZConfigurationPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks config validation data.
#[derive(Debug, Clone)]
pub struct ZConfigurationConfigValidation {
    pub errors: Vec<(String, String)>,
    pub strict: bool,
    pub schema_version: u32,
}

impl ZConfigurationConfigValidation {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            strict: false,
            schema_version: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.errors.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZConfigurationConfigValidation[strict={:?}, schema_version={:?}]", self.strict, self.schema_version)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for config validation.
pub fn z_configuration_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_configuration_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_configuration_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_configuration_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_configuration_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_configuration_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_configuration_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 58
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer58 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer58 {
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
pub fn xb_fnv1a_58(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_58<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_58<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_58(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_58(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 22
// ---------------------------------------------------------------------------

/// Generic object pool `Xc22Pool<T>`.
pub struct Xc22Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc22Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc22PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc22Pool<T> {
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
    pub fn stats(&self) -> Xc22PoolStats {
        Xc22PoolStats {
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

impl<T> Default for Xc22Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc22Scheduler`.
pub struct Xc22Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc22Scheduler {
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

impl Default for Xc22Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_22 hash for the given byte slice.
pub fn xc_22_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_22 convention.
pub fn xc_22_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe71 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe71Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe71PipelineError {
    pub stage: Xe71Stage,
    pub message: String,
}

impl std::fmt::Display for Xe71PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe71Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe71Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe71PipelineError>>>,
    stage_names: Vec<Xe71Stage>,
}

impl Xe71Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe71PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe71Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe71PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe71Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe71PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe71Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe71PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe71Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe71PipelineError> {
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

    pub fn compose(mut self, other: Xe71Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe71CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe71CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe71Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe71CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe71CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe71Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe71CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_71_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe71CacheEntry {
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

    fn xe_71_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe71CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_71_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe71PipelineError> {
    Ok(data)
}

pub fn xe_71_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe71PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_71_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe71PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_71_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe71PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_71_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe71PipelineError> {
    Err(Xe71PipelineError {
        stage: Xe71Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_69: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg69Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg69Graph {
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

impl Default for Xg69Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_69: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg69Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg69Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg69Heap<T>) {
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

impl<T: Ord> Default for Xg69Heap<T> {
    fn default() -> Self { Self::new() }
}

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

    // -- ConfigurationOverrideScope tests --

    #[test]
    fn override_scope_priority() {
        let ws = ConfigurationOverrideScope::Workspace;
        let folder = ConfigurationOverrideScope::Folder("src".into());
        let lang = ConfigurationOverrideScope::Language("rust".into());
        assert!(folder.overrides(&ws));
        assert!(lang.overrides(&folder));
        assert!(!ws.overrides(&lang));
    }

    #[test]
    fn override_scope_display() {
        assert_eq!(format!("{}", ConfigurationOverrideScope::Workspace), "workspace");
        assert_eq!(format!("{}", ConfigurationOverrideScope::Folder("src".into())), "folder:src");
        assert_eq!(format!("{}", ConfigurationOverrideScope::Language("rust".into())), "[rust]");
    }

    #[test]
    fn resolve_override_most_specific_wins() {
        let overrides = vec![
            ConfigurationOverride {
                scope: ConfigurationOverrideScope::Workspace,
                key: "editor.tabSize".into(),
                value: json!(4),
            },
            ConfigurationOverride {
                scope: ConfigurationOverrideScope::Language("rust".into()),
                key: "editor.tabSize".into(),
                value: json!(2),
            },
        ];
        assert_eq!(resolve_override(&overrides, "editor.tabSize"), Some(json!(2)));
    }

    #[test]
    fn resolve_override_not_found() {
        assert_eq!(resolve_override(&[], "missing"), None);
    }

    // -- ConfigurationLock tests --

    #[test]
    fn config_lock_get_set() {
        let lock = ConfigurationLock::new();
        assert!(lock.get("key").is_none());
        assert!(lock.set("key", json!(42)));
        assert_eq!(lock.get("key"), Some(json!(42)));
    }

    #[test]
    fn config_lock_remove() {
        let lock = ConfigurationLock::default();
        lock.set("a", json!(1));
        assert_eq!(lock.remove("a"), Some(json!(1)));
        assert!(lock.get("a").is_none());
    }

    #[test]
    fn config_lock_keys() {
        let lock = ConfigurationLock::new();
        lock.set("x", json!(1));
        lock.set("y", json!(2));
        let mut keys = lock.keys();
        keys.sort();
        assert_eq!(keys, vec!["x", "y"]);
    }

    // -- ConfigurationChangeDebouncer tests --

    #[test]
    fn debouncer_record_and_flush() {
        let mut d = ConfigurationChangeDebouncer::new(100);
        d.record("editor.fontSize");
        d.record("editor.tabSize");
        d.record("editor.fontSize"); // duplicate ignored
        assert_eq!(d.pending_count(), 2);
        assert!(d.has_pending());
        let flushed = d.flush();
        assert_eq!(flushed.len(), 2);
        assert!(!d.has_pending());
    }

    #[test]
    fn debouncer_ms() {
        let d = ConfigurationChangeDebouncer::new(250);
        assert_eq!(d.debounce_ms(), 250);
    }

    // -- InheritanceChainBuilder tests --

    #[test]
    fn inheritance_chain_resolve_last_wins() {
        let mut builder = InheritanceChainBuilder::new();
        let mut defaults = HashMap::new();
        defaults.insert("tabSize".into(), json!(4));
        defaults.insert("fontSize".into(), json!(12));
        builder.add_layer("defaults", defaults);

        let mut user = HashMap::new();
        user.insert("tabSize".into(), json!(2));
        builder.add_layer("user", user);

        assert_eq!(builder.resolve("tabSize"), Some(json!(2)));
        assert_eq!(builder.resolve("fontSize"), Some(json!(12)));
        assert_eq!(builder.resolve("missing"), None);
    }

    #[test]
    fn inheritance_chain_build_merges() {
        let mut builder = InheritanceChainBuilder::default();
        let mut l1 = HashMap::new();
        l1.insert("a".into(), json!(1));
        builder.add_layer("base", l1);
        let mut l2 = HashMap::new();
        l2.insert("b".into(), json!(2));
        builder.add_layer("overlay", l2);
        let merged = builder.build();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged["a"], json!(1));
        assert_eq!(merged["b"], json!(2));
    }

    #[test]
    fn inheritance_chain_layer_names() {
        let mut builder = InheritanceChainBuilder::new();
        builder.add_layer("defaults", HashMap::new());
        builder.add_layer("user", HashMap::new());
        assert_eq!(builder.layer_names(), vec!["defaults", "user"]);
        assert_eq!(builder.len(), 2);
    }

#[test]
    fn configprofileswitcher_severity_ordering() {
        assert!(ConfigProfileSwitcherSeverity::Critical > ConfigProfileSwitcherSeverity::High);
        assert!(ConfigProfileSwitcherSeverity::High > ConfigProfileSwitcherSeverity::Medium);
        assert!(ConfigProfileSwitcherSeverity::Medium > ConfigProfileSwitcherSeverity::Low);
    }

    #[test]
    fn configprofileswitcher_severity_display() {
        assert_eq!(ConfigProfileSwitcherSeverity::Low.to_string(), "low");
        assert_eq!(ConfigProfileSwitcherSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn configprofileswitcher_entry_creation() {
        let e = ConfigProfileSwitcherEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, ConfigProfileSwitcherSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn configprofileswitcher_entry_builder() {
        let e = ConfigProfileSwitcherEntry::new("e2", "Entry 2")
            .with_severity(ConfigProfileSwitcherSeverity::High)
            .with_detail("some detail")
            .with_profile_count(42);
        assert_eq!(e.severity, ConfigProfileSwitcherSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.profile_count, 42);
    }

    #[test]
    fn configprofileswitcher_entry_enable_disable() {
        let mut e = ConfigProfileSwitcherEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn configprofileswitcher_add_and_count() {
        let mut mgr = ConfigProfileSwitcher::new("test");
        mgr.add(ConfigProfileSwitcherEntry::new("a", "A"));
        mgr.add(ConfigProfileSwitcherEntry::new("b", "B").with_severity(ConfigProfileSwitcherSeverity::High));
        assert_eq!(mgr.profile_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn configprofileswitcher_remove() {
        let mut mgr = ConfigProfileSwitcher::new("test");
        mgr.add(ConfigProfileSwitcherEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn configprofileswitcher_capacity() {
        let mut mgr = ConfigProfileSwitcher::new("test").with_capacity(1);
        assert!(mgr.add(ConfigProfileSwitcherEntry::new("a", "A")));
        assert!(!mgr.add(ConfigProfileSwitcherEntry::new("b", "B")));
    }

    #[test]
    fn configprofileswitcher_sorted_by_severity() {
        let mut mgr = ConfigProfileSwitcher::new("test");
        mgr.add(ConfigProfileSwitcherEntry::new("lo", "Low"));
        mgr.add(ConfigProfileSwitcherEntry::new("hi", "High").with_severity(ConfigProfileSwitcherSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, ConfigProfileSwitcherSeverity::Critical);
    }

    #[test]
    fn configprofileswitcher_summary() {
        let mgr = ConfigProfileSwitcher::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn configvalidationreport_config_defaults() {
        let cfg = ConfigValidationReportConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn configvalidationreport_item_creation() {
        let item = ConfigValidationReportItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn configvalidationreport_add_and_get() {
        let mut mgr = ConfigValidationReport::new(ConfigValidationReportConfig::new("test"));
        mgr.add(ConfigValidationReportItem::new("k1", "v1"));
        assert_eq!(mgr.error_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn configvalidationreport_remove_item() {
        let mut mgr = ConfigValidationReport::new(ConfigValidationReportConfig::new("test"));
        mgr.add(ConfigValidationReportItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn configvalidationreport_sorted_by_priority() {
        let mut mgr = ConfigValidationReport::new(ConfigValidationReportConfig::new("test"));
        mgr.add(ConfigValidationReportItem::new("lo", "low").with_priority(1));
        mgr.add(ConfigValidationReportItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn configvalidationreport_items_with_tag() {
        let mut mgr = ConfigValidationReport::new(ConfigValidationReportConfig::new("test"));
        mgr.add(ConfigValidationReportItem::new("a", "1").with_tag("x"));
        mgr.add(ConfigValidationReportItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn configvalidationreport_report() {
        let mgr = ConfigValidationReport::new(ConfigValidationReportConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    // -- configuration additional tests -------------------------------------------

    #[test]
    fn x_configuration_validation_ok() {
        let r = x_configuration_validate_string("hello", 100);
        assert!(r.is_ok());
        assert!(r.message().is_none());
    }

    #[test]
    fn x_configuration_validation_empty() {
        let r = x_configuration_validate_string("", 100);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("empty"));
    }

    #[test]
    fn x_configuration_validation_too_long() {
        let r = x_configuration_validate_string("abcdef", 3);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("max length"));
    }

    #[test]
    fn x_configuration_validate_range_ok() {
        assert!(x_configuration_validate_range(5, 1, 10).is_ok());
        assert!(x_configuration_validate_range(1, 1, 10).is_ok());
        assert!(x_configuration_validate_range(10, 1, 10).is_ok());
    }

    #[test]
    fn x_configuration_validate_range_out() {
        assert!(!x_configuration_validate_range(0, 1, 10).is_ok());
        assert!(!x_configuration_validate_range(11, 1, 10).is_ok());
    }

    #[test]
    fn x_configuration_tagged_entry_basic() {
        let e = XConfigurationTaggedEntry::new("k", "v");
        assert_eq!(e.key, "k");
        assert_eq!(e.value, "v");
        assert!(e.tag.is_none());
    }

    #[test]
    fn x_configuration_tagged_entry_with_tag() {
        let e = XConfigurationTaggedEntry::new("k", "v").with_tag("important");
        assert!(e.matches_tag("important"));
        assert!(!e.matches_tag("other"));
    }

    #[test]
    fn x_configuration_filter_by_tag_basic() {
        let entries = vec![
            XConfigurationTaggedEntry::new("a", "1").with_tag("x"),
            XConfigurationTaggedEntry::new("b", "2").with_tag("y"),
            XConfigurationTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let filtered = x_configuration_filter_by_tag(&entries, "x");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_configuration_group_by_tag_basic() {
        let entries = vec![
            XConfigurationTaggedEntry::new("a", "1").with_tag("x"),
            XConfigurationTaggedEntry::new("b", "2"),
            XConfigurationTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let groups = x_configuration_group_by_tag(&entries);
        assert_eq!(groups["x"].len(), 2);
        assert_eq!(groups["_untagged"].len(), 1);
    }

    #[test]
    fn x_configuration_djb2_hash_deterministic() {
        let h1 = x_configuration_djb2_hash("hello");
        let h2 = x_configuration_djb2_hash("hello");
        assert_eq!(h1, h2);
        assert_ne!(x_configuration_djb2_hash("hello"), x_configuration_djb2_hash("world"));
    }

    #[test]
    fn x_configuration_dedup_entries_basic() {
        let entries = vec![
            XConfigurationTaggedEntry::new("a", "1"),
            XConfigurationTaggedEntry::new("a", "2"),
            XConfigurationTaggedEntry::new("b", "3"),
        ];
        let deduped = x_configuration_dedup_entries(entries);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].value, "1");
    }

    #[test]
    fn x_configuration_validation_result_warning() {
        let w = XConfigurationValidationResult::Warning("low disk".into());
        assert!(!w.is_ok());
        assert_eq!(w.message(), Some("low disk"));
    }

    #[test]
    fn x_configuration_filter_by_tag_empty() {
        let entries: Vec<XConfigurationTaggedEntry> = vec![];
        assert!(x_configuration_filter_by_tag(&entries, "x").is_empty());
    }

    #[test]
    fn x_configuration_tagged_entry_no_tag_match() {
        let e = XConfigurationTaggedEntry::new("k", "v");
        assert!(!e.matches_tag("any"));
    }


    // -- configuration extended domain tests ----------------------------------------

    #[test]
    fn y_configuration_enum_index() {
        assert_eq!(YConfigurationConfigSource::Default.index(), 0);
        assert_eq!(YConfigurationConfigSource::User.index(), 1);
        assert_eq!(YConfigurationConfigSource::Workspace.index(), 2);
        assert_eq!(YConfigurationConfigSource::Remote.index(), 3);
    }

    #[test]
    fn y_configuration_enum_label() {
        assert_eq!(YConfigurationConfigSource::Default.label(), "Default");
        assert_eq!(YConfigurationConfigSource::User.label(), "User");
        assert_eq!(YConfigurationConfigSource::Workspace.label(), "Workspace");
        assert_eq!(YConfigurationConfigSource::Remote.label(), "Remote");
    }

    #[test]
    fn y_configuration_enum_all() {
        let all = YConfigurationConfigSource::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_configuration_enum_is_default() {
        assert!(YConfigurationConfigSource::Default.is_default());
        assert!(!YConfigurationConfigSource::Remote.is_default());
    }

    #[test]
    fn y_configuration_enum_display() {
        assert_eq!(format!("{}", YConfigurationConfigSource::Default), "Default");
    }

    #[test]
    fn y_configuration_struct_new() {
        let s = YConfigurationConfigOverride::new();
        let _ = s.summary();
    }

    #[test]
    fn y_configuration_fingerprint_deterministic() {
        let h1 = y_configuration_fingerprint("hello");
        let h2 = y_configuration_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_configuration_fingerprint("a"), y_configuration_fingerprint("b"));
    }

    #[test]
    fn y_configuration_truncate_short() {
        assert_eq!(y_configuration_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_configuration_truncate_long() {
        let r = y_configuration_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_configuration_normalize_key_basic() {
        assert_eq!(y_configuration_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_configuration_split_path_basic() {
        let parts = y_configuration_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_configuration_count_occurrences_basic() {
        assert_eq!(y_configuration_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_configuration_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_configuration_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_configuration_in_range_basic() {
        assert!(y_configuration_in_range(5, 1, 10));
        assert!(y_configuration_in_range(1, 1, 10));
        assert!(y_configuration_in_range(10, 1, 10));
        assert!(!y_configuration_in_range(0, 1, 10));
        assert!(!y_configuration_in_range(11, 1, 10));
    }

    #[test]
    fn y_configuration_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_configuration_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_configuration_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_configuration_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- configuration Z-extended tests -----------------------------------------------

    #[test]
    fn z_configuration_priority_weight() {
        assert_eq!(ZConfigurationPriority::Idle.weight(), 0);
        assert_eq!(ZConfigurationPriority::Normal.weight(), 2);
        assert_eq!(ZConfigurationPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_configuration_priority_label() {
        assert_eq!(ZConfigurationPriority::Low.label(), "low");
        assert_eq!(ZConfigurationPriority::High.label(), "high");
    }

    #[test]
    fn z_configuration_priority_is_elevated() {
        assert!(!ZConfigurationPriority::Normal.is_elevated());
        assert!(ZConfigurationPriority::High.is_elevated());
        assert!(ZConfigurationPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_configuration_priority_display() {
        assert_eq!(format!("{}", ZConfigurationPriority::Idle), "idle");
    }

    #[test]
    fn z_configuration_priority_all_asc() {
        let all = ZConfigurationPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZConfigurationPriority::Idle);
        assert_eq!(all[4], ZConfigurationPriority::Realtime);
    }

    #[test]
    fn z_configuration_struct_new() {
        let s = ZConfigurationConfigValidation::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_configuration_struct_toggled_clone() {
        let s = ZConfigurationConfigValidation::new();
        let t = s.toggled_clone();
        let _ = t.schema_version;
    }

    #[test]
    fn z_configuration_rolling_hash_deterministic() {
        let h1 = z_configuration_rolling_hash(b"test");
        let h2 = z_configuration_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_configuration_rolling_hash(b"a"), z_configuration_rolling_hash(b"b"));
    }

    #[test]
    fn z_configuration_pad_to_basic() {
        assert_eq!(z_configuration_pad_to("hi", 5), "hi   ");
        assert_eq!(z_configuration_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_configuration_is_identifier_basic() {
        assert!(z_configuration_is_identifier("foo_bar"));
        assert!(z_configuration_is_identifier("abc123"));
        assert!(!z_configuration_is_identifier(""));
        assert!(!z_configuration_is_identifier("has space"));
    }

    #[test]
    fn z_configuration_levenshtein_basic() {
        assert_eq!(z_configuration_levenshtein("", ""), 0);
        assert_eq!(z_configuration_levenshtein("abc", "abc"), 0);
        assert_eq!(z_configuration_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_configuration_unique_words_basic() {
        let w = z_configuration_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_configuration_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_configuration_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_configuration_common_prefix_basic() {
        assert_eq!(z_configuration_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_configuration_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_configuration_struct_clear() {
        let mut s = ZConfigurationConfigValidation::new();
        s.errors.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_configuration_rolling_hash_empty() {
        let h = z_configuration_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_58_push_and_len() {
        let mut rb = super::XbRingBuffer58::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_58_overwrite() {
        let mut rb = super::XbRingBuffer58::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_58_get_out_of_bounds() {
        let rb = super::XbRingBuffer58::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_58_drain_all() {
        let mut rb = super::XbRingBuffer58::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_58_peek_front_back() {
        let mut rb = super::XbRingBuffer58::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_58_clear() {
        let mut rb = super::XbRingBuffer58::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_58_capacity() {
        let rb = super::XbRingBuffer58::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_58_basic() {
        let h = super::xb_fnv1a_58(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_58(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_58_different_inputs() {
        let h1 = super::xb_fnv1a_58(b"abc");
        let h2 = super::xb_fnv1a_58(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_58_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_58(&data);
        let dec = super::xb_rle_decode_58(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_58_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_58(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_58(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_58_values() {
        assert!((super::xb_clamp_58(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_58(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_58(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_58_values() {
        assert!((super::xb_lerp_58(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_58(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_58(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_58_wrap_around_twice() {
        let mut rb = super::XbRingBuffer58::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 22 ----

    #[test]
    fn xc_22_pool_new_empty() {
        let pool: super::Xc22Pool<i32> = super::Xc22Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_22_pool_release_acquire() {
        let mut pool = super::Xc22Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_22_pool_acquire_empty() {
        let mut pool: super::Xc22Pool<i32> = super::Xc22Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_22_pool_full() {
        let mut pool = super::Xc22Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_22_pool_drain() {
        let mut pool = super::Xc22Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_22_pool_stats() {
        let mut pool = super::Xc22Pool::new(8);
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
    fn xc_22_pool_clear() {
        let mut pool = super::Xc22Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_22_pool_shrink() {
        let mut pool = super::Xc22Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_22_pool_default() {
        let pool: super::Xc22Pool<String> = super::Xc22Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_22_pool_extend() {
        let mut pool = super::Xc22Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_22_pool_retain() {
        let mut pool = super::Xc22Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_22_scheduler_round_robin() {
        let mut sched = super::Xc22Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_22_scheduler_empty() {
        let mut sched = super::Xc22Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_22_scheduler_reset() {
        let mut sched = super::Xc22Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_22_scheduler_add_remove() {
        let mut sched = super::Xc22Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_22_scheduler_targets() {
        let sched = super::Xc22Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_22_hash_empty() {
        assert_eq!(super::xc_22_hash(b""), 5381);
    }

    #[test]
    fn xc_22_hash_data() {
        let h = super::xc_22_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_22_hash(b"hello"), h);
    }

    #[test]
    fn xc_22_reverse_str() {
        assert_eq!(super::xc_22_reverse("abc"), "cba");
        assert_eq!(super::xc_22_reverse(""), "");
    }


    #[test]
    fn xe_71_pipeline_empty() {
        let p = super::Xe71Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_71_pipeline_parse_stage() {
        let p = super::Xe71Pipeline::new()
            .add_parse(super::xe_71_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_71_pipeline_transform_double() {
        let p = super::Xe71Pipeline::new()
            .add_transform(super::xe_71_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_71_pipeline_validate_reverse() {
        let p = super::Xe71Pipeline::new()
            .add_validate(super::xe_71_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_71_pipeline_emit_filter() {
        let p = super::Xe71Pipeline::new()
            .add_emit(super::xe_71_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_71_pipeline_multi_stage() {
        let p = super::Xe71Pipeline::new()
            .add_parse(super::xe_71_pipeline_identity)
            .add_transform(super::xe_71_pipeline_double)
            .add_validate(super::xe_71_pipeline_reverse)
            .add_emit(super::xe_71_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_71_pipeline_error_propagation() {
        let p = super::Xe71Pipeline::new()
            .add_parse(super::xe_71_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe71Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_71_pipeline_compose() {
        let p1 = super::Xe71Pipeline::new()
            .add_parse(super::xe_71_pipeline_identity);
        let p2 = super::Xe71Pipeline::new()
            .add_transform(super::xe_71_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_71_pipeline_error_display() {
        let e = super::Xe71PipelineError {
            stage: super::Xe71Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_71_cache_put_get() {
        let mut c = super::Xe71Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_71_cache_miss() {
        let mut c: super::Xe71Cache<&str, i32> = super::Xe71Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_71_cache_ttl_expiry() {
        let mut c = super::Xe71Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_71_cache_evict() {
        let mut c = super::Xe71Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_71_cache_capacity() {
        let mut c = super::Xe71Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_71_cache_stats() {
        let mut c = super::Xe71Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_71_cache_clear() {
        let mut c = super::Xe71Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_69 graph tests ------------------------------------------------

    #[test]
    fn xg_69_graph_empty() {
        let g = super::Xg69Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_69_graph_add_node() {
        let mut g = super::Xg69Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_69_graph_add_edge() {
        let mut g = super::Xg69Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_69_graph_neighbors() {
        let mut g = super::Xg69Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_69_graph_has_path() {
        let mut g = super::Xg69Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_69_graph_self_path() {
        let g = super::Xg69Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_69_graph_topo_sort() {
        let mut g = super::Xg69Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_69_graph_cycle_detect_false() {
        let mut g = super::Xg69Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_69_graph_cycle_detect_true() {
        let mut g = super::Xg69Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_69 heap tests -------------------------------------------------

    #[test]
    fn xg_69_heap_empty() {
        let h: super::Xg69Heap<i32> = super::Xg69Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_69_heap_push_pop() {
        let mut h = super::Xg69Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_69_heap_peek() {
        let mut h = super::Xg69Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_69_heap_drain_sorted() {
        let mut h = super::Xg69Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_69_heap_merge() {
        let mut a = super::Xg69Heap::new();
        let mut b = super::Xg69Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_69_heap_default() {
        let h: super::Xg69Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_69_graph_default() {
        let g: super::Xg69Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }

}
