//! Extension host process management
//!
//! Manages extension descriptions parsed from `package.json` and tracks
//! extension host lifecycle state and activation.

use serde::Deserialize;
use vsedit_events::{Emitter, Event};
use vsedit_uri::VsUri;

// ---------------------------------------------------------------------------
// Contribution types
// ---------------------------------------------------------------------------

/// A command contributed by an extension.
#[derive(Debug, Clone)]
pub struct ContributedCommand {
    pub command: String,
    pub title: String,
    pub category: Option<String>,
}

/// A language contributed by an extension.
#[derive(Debug, Clone)]
pub struct ContributedLanguage {
    pub id: String,
    pub extensions: Vec<String>,
    pub aliases: Vec<String>,
}

/// A TextMate grammar contributed by an extension.
#[derive(Debug, Clone)]
pub struct ContributedGrammar {
    pub language: String,
    pub scope_name: String,
    pub path: String,
}

/// A color theme contributed by an extension.
#[derive(Debug, Clone)]
pub struct ContributedTheme {
    pub label: String,
    pub ui_theme: String,
    pub path: String,
}

/// A keybinding contributed by an extension.
#[derive(Debug, Clone)]
pub struct ContributedKeybinding {
    pub command: String,
    pub key: String,
    pub when: Option<String>,
}

/// All contribution points from a single extension.
#[derive(Debug, Clone)]
pub struct ExtensionContributions {
    pub commands: Vec<ContributedCommand>,
    pub languages: Vec<ContributedLanguage>,
    pub grammars: Vec<ContributedGrammar>,
    pub themes: Vec<ContributedTheme>,
    pub keybindings: Vec<ContributedKeybinding>,
    pub views: Vec<serde_json::Value>,
    pub menus: serde_json::Value,
    pub configuration: Vec<serde_json::Value>,
}

impl Default for ExtensionContributions {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            languages: Vec::new(),
            grammars: Vec::new(),
            themes: Vec::new(),
            keybindings: Vec::new(),
            views: Vec::new(),
            menus: serde_json::Value::Object(serde_json::Map::new()),
            configuration: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// ExtensionKind
// ---------------------------------------------------------------------------

/// Where the extension should run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionKind {
    UI,
    Workspace,
    Both,
}

// ---------------------------------------------------------------------------
// ExtensionDescription
// ---------------------------------------------------------------------------

/// Metadata for a single extension, parsed from its `package.json`.
#[derive(Debug, Clone)]
pub struct ExtensionDescription {
    /// Unique identifier (`publisher.name`).
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub publisher: String,
    /// Entry-point JS file (relative to the extension root).
    pub main: Option<String>,
    pub activation_events: Vec<String>,
    pub contributes: ExtensionContributions,
    pub extension_kind: ExtensionKind,
    pub is_builtin: bool,
    pub location: VsUri,
}

// -- serde helper structs for package.json ----------------------------------

#[derive(Deserialize)]
struct PackageJson {
    name: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    version: Option<String>,
    publisher: Option<String>,
    main: Option<String>,
    #[serde(rename = "activationEvents", default)]
    activation_events: Vec<String>,
    #[serde(rename = "extensionKind")]
    extension_kind: Option<serde_json::Value>,
    contributes: Option<ContributesJson>,
}

#[derive(Deserialize, Default)]
struct ContributesJson {
    #[serde(default)]
    commands: Vec<CommandJson>,
    #[serde(default)]
    languages: Vec<LanguageJson>,
    #[serde(default)]
    grammars: Vec<GrammarJson>,
    #[serde(default)]
    themes: Vec<ThemeJson>,
    #[serde(default)]
    keybindings: Vec<KeybindingJson>,
    #[serde(default)]
    views: Option<serde_json::Value>,
    #[serde(default)]
    menus: Option<serde_json::Value>,
    #[serde(default)]
    configuration: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct CommandJson {
    command: Option<String>,
    title: Option<String>,
    category: Option<String>,
}

#[derive(Deserialize)]
struct LanguageJson {
    id: Option<String>,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Deserialize)]
struct GrammarJson {
    language: Option<String>,
    #[serde(rename = "scopeName")]
    scope_name: Option<String>,
    path: Option<String>,
}

#[derive(Deserialize)]
struct ThemeJson {
    label: Option<String>,
    #[serde(rename = "uiTheme")]
    ui_theme: Option<String>,
    path: Option<String>,
}

#[derive(Deserialize)]
struct KeybindingJson {
    command: Option<String>,
    key: Option<String>,
    when: Option<String>,
}

impl ExtensionDescription {
    /// Parse an [`ExtensionDescription`] from the raw JSON text of a
    /// `package.json` file.
    pub fn from_package_json(json: &str, location: VsUri) -> Result<Self, String> {
        let pkg: PackageJson =
            serde_json::from_str(json).map_err(|e| format!("invalid package.json: {e}"))?;

        let name = pkg.name.unwrap_or_default();
        let publisher = pkg.publisher.unwrap_or_default();
        let id = if publisher.is_empty() {
            name.clone()
        } else {
            format!("{publisher}.{name}")
        };

        let extension_kind = match &pkg.extension_kind {
            Some(serde_json::Value::String(s)) => match s.as_str() {
                "ui" => ExtensionKind::UI,
                "workspace" => ExtensionKind::Workspace,
                _ => ExtensionKind::Both,
            },
            Some(serde_json::Value::Array(arr)) => {
                let has_ui = arr.iter().any(|v| v.as_str() == Some("ui"));
                let has_ws = arr.iter().any(|v| v.as_str() == Some("workspace"));
                match (has_ui, has_ws) {
                    (true, true) => ExtensionKind::Both,
                    (true, false) => ExtensionKind::UI,
                    (false, true) => ExtensionKind::Workspace,
                    _ => ExtensionKind::Both,
                }
            }
            _ => ExtensionKind::Both,
        };

        let contributes = match pkg.contributes {
            Some(c) => ExtensionContributions {
                commands: c
                    .commands
                    .into_iter()
                    .filter_map(|cmd| {
                        Some(ContributedCommand {
                            command: cmd.command?,
                            title: cmd.title?,
                            category: cmd.category,
                        })
                    })
                    .collect(),
                languages: c
                    .languages
                    .into_iter()
                    .filter_map(|l| {
                        Some(ContributedLanguage {
                            id: l.id?,
                            extensions: l.extensions,
                            aliases: l.aliases,
                        })
                    })
                    .collect(),
                grammars: c
                    .grammars
                    .into_iter()
                    .filter_map(|g| {
                        Some(ContributedGrammar {
                            language: g.language?,
                            scope_name: g.scope_name?,
                            path: g.path?,
                        })
                    })
                    .collect(),
                themes: c
                    .themes
                    .into_iter()
                    .filter_map(|t| {
                        Some(ContributedTheme {
                            label: t.label?,
                            ui_theme: t.ui_theme?,
                            path: t.path?,
                        })
                    })
                    .collect(),
                keybindings: c
                    .keybindings
                    .into_iter()
                    .filter_map(|k| {
                        Some(ContributedKeybinding {
                            command: k.command?,
                            key: k.key?,
                            when: k.when,
                        })
                    })
                    .collect(),
                views: match c.views {
                    Some(serde_json::Value::Object(map)) => {
                        map.into_values().collect()
                    }
                    Some(v) => vec![v],
                    None => Vec::new(),
                },
                menus: c
                    .menus
                    .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new())),
                configuration: match c.configuration {
                    Some(serde_json::Value::Array(arr)) => arr,
                    Some(v) => vec![v],
                    None => Vec::new(),
                },
            },
            None => ExtensionContributions::default(),
        };

        Ok(Self {
            id,
            display_name: pkg.display_name.unwrap_or_else(|| name.clone()),
            name,
            version: pkg.version.unwrap_or_default(),
            publisher,
            main: pkg.main,
            activation_events: pkg.activation_events,
            contributes,
            extension_kind,
            is_builtin: false,
            location,
        })
    }
}

// ---------------------------------------------------------------------------
// ExtensionHostState
// ---------------------------------------------------------------------------

/// Lifecycle state of the extension host process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionHostState {
    Starting,
    Running,
    Stopped,
    Error(String),
}

// ---------------------------------------------------------------------------
// ExtensionHostManager
// ---------------------------------------------------------------------------

/// Manages registered extensions and their activation state.
pub struct ExtensionHostManager {
    extensions: Vec<ExtensionDescription>,
    state: ExtensionHostState,
    activated: Vec<String>,
    on_did_change_state: Emitter<ExtensionHostState>,
}

impl ExtensionHostManager {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            state: ExtensionHostState::Stopped,
            activated: Vec::new(),
            on_did_change_state: Emitter::new(),
        }
    }

    /// Register a new extension with the manager.
    pub fn register_extension(&mut self, ext: ExtensionDescription) {
        self.extensions.push(ext);
    }

    /// Look up an extension by its unique identifier.
    pub fn get_extension(&self, id: &str) -> Option<&ExtensionDescription> {
        self.extensions.iter().find(|e| e.id == id)
    }

    /// Return all registered extensions.
    pub fn get_all_extensions(&self) -> &[ExtensionDescription] {
        &self.extensions
    }

    /// Return every extension whose `activation_events` match the given
    /// `event` string. The wildcard `"*"` matches any event.
    pub fn should_activate(&self, event: &str) -> Vec<&ExtensionDescription> {
        self.extensions
            .iter()
            .filter(|ext| {
                ext.activation_events
                    .iter()
                    .any(|ae| ae == "*" || ae == event)
            })
            .collect()
    }

    /// Mark an extension as activated.
    pub fn mark_activated(&mut self, id: &str) {
        if !self.activated.contains(&id.to_string()) {
            self.activated.push(id.to_string());
        }
    }

    /// Check whether an extension has been activated.
    pub fn is_activated(&self, id: &str) -> bool {
        self.activated.iter().any(|a| a == id)
    }

    /// Current lifecycle state.
    pub fn state(&self) -> &ExtensionHostState {
        &self.state
    }

    /// Transition to a new lifecycle state.
    pub fn set_state(&mut self, state: ExtensionHostState) {
        self.state = state.clone();
        self.on_did_change_state.fire(&state);
    }

    /// Subscribe to state-change events.
    pub fn on_did_change_state(&self) -> Event<ExtensionHostState> {
        self.on_did_change_state.event()
    }
}

impl Default for ExtensionHostManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_package_json() -> &'static str {
        r#"{
            "name": "rust-lang",
            "displayName": "Rust Language Support",
            "version": "1.0.0",
            "publisher": "rust-lang",
            "main": "./out/extension.js",
            "activationEvents": ["onLanguage:rust", "onCommand:rust.build"],
            "extensionKind": "workspace",
            "contributes": {
                "commands": [
                    { "command": "rust.build", "title": "Build", "category": "Rust" }
                ],
                "languages": [
                    { "id": "rust", "extensions": [".rs"], "aliases": ["Rust"] }
                ],
                "grammars": [
                    { "language": "rust", "scopeName": "source.rust", "path": "./syntaxes/rust.tmLanguage.json" }
                ],
                "themes": [
                    { "label": "Rusty Dark", "uiTheme": "vs-dark", "path": "./themes/dark.json" }
                ],
                "keybindings": [
                    { "command": "rust.build", "key": "ctrl+shift+b", "when": "editorLangId == rust" }
                ]
            }
        }"#
    }

    #[test]
    fn parse_package_json() {
        let loc = VsUri::file("/extensions/rust-lang");
        let ext = ExtensionDescription::from_package_json(sample_package_json(), loc).unwrap();

        assert_eq!(ext.id, "rust-lang.rust-lang");
        assert_eq!(ext.name, "rust-lang");
        assert_eq!(ext.display_name, "Rust Language Support");
        assert_eq!(ext.version, "1.0.0");
        assert_eq!(ext.publisher, "rust-lang");
        assert_eq!(ext.main.as_deref(), Some("./out/extension.js"));
        assert_eq!(ext.extension_kind, ExtensionKind::Workspace);
        assert!(!ext.is_builtin);

        assert_eq!(ext.contributes.commands.len(), 1);
        assert_eq!(ext.contributes.commands[0].command, "rust.build");
        assert_eq!(
            ext.contributes.commands[0].category.as_deref(),
            Some("Rust")
        );

        assert_eq!(ext.contributes.languages.len(), 1);
        assert_eq!(ext.contributes.languages[0].id, "rust");
        assert_eq!(ext.contributes.languages[0].extensions, vec![".rs"]);

        assert_eq!(ext.contributes.grammars.len(), 1);
        assert_eq!(ext.contributes.grammars[0].scope_name, "source.rust");

        assert_eq!(ext.contributes.themes.len(), 1);
        assert_eq!(ext.contributes.themes[0].label, "Rusty Dark");

        assert_eq!(ext.contributes.keybindings.len(), 1);
        assert_eq!(ext.contributes.keybindings[0].key, "ctrl+shift+b");
    }

    #[test]
    fn activation_event_matching() {
        let mut mgr = ExtensionHostManager::new();

        let loc = VsUri::file("/ext/a");
        let mut ext_a =
            ExtensionDescription::from_package_json(sample_package_json(), loc).unwrap();
        ext_a.id = "ext-a".into();

        let loc = VsUri::file("/ext/b");
        let ext_b = ExtensionDescription {
            id: "ext-b".into(),
            name: "b".into(),
            display_name: "B".into(),
            version: "0.1.0".into(),
            publisher: "test".into(),
            main: None,
            activation_events: vec!["*".into()],
            contributes: ExtensionContributions::default(),
            extension_kind: ExtensionKind::Both,
            is_builtin: false,
            location: loc,
        };

        mgr.register_extension(ext_a);
        mgr.register_extension(ext_b);

        // onLanguage:rust matches ext-a, wildcard matches ext-b
        let activated = mgr.should_activate("onLanguage:rust");
        assert_eq!(activated.len(), 2);

        // onCommand:rust.build matches ext-a, wildcard matches ext-b
        let activated = mgr.should_activate("onCommand:rust.build");
        assert_eq!(activated.len(), 2);

        // Unknown event only matches the wildcard ext-b
        let activated = mgr.should_activate("onLanguage:python");
        assert_eq!(activated.len(), 1);
        assert_eq!(activated[0].id, "ext-b");
    }

    #[test]
    fn extension_lookup() {
        let mut mgr = ExtensionHostManager::new();

        let loc = VsUri::file("/ext/rust");
        let ext =
            ExtensionDescription::from_package_json(sample_package_json(), loc).unwrap();
        let id = ext.id.clone();

        mgr.register_extension(ext);

        assert!(mgr.get_extension(&id).is_some());
        assert!(mgr.get_extension("nonexistent").is_none());
        assert_eq!(mgr.get_all_extensions().len(), 1);
    }

    #[test]
    fn activation_tracking() {
        let mut mgr = ExtensionHostManager::new();
        assert!(!mgr.is_activated("foo"));

        mgr.mark_activated("foo");
        assert!(mgr.is_activated("foo"));

        // Duplicate mark is idempotent
        mgr.mark_activated("foo");
        assert!(mgr.is_activated("foo"));
    }

    #[test]
    fn state_management() {
        let mut mgr = ExtensionHostManager::new();
        assert_eq!(*mgr.state(), ExtensionHostState::Stopped);

        let states = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let s = states.clone();
        let _handle = mgr.on_did_change_state().on(move |state: &ExtensionHostState| {
            s.lock().unwrap().push(state.clone());
        });

        mgr.set_state(ExtensionHostState::Starting);
        mgr.set_state(ExtensionHostState::Running);
        mgr.set_state(ExtensionHostState::Error("crash".into()));
        mgr.set_state(ExtensionHostState::Stopped);

        assert_eq!(*mgr.state(), ExtensionHostState::Stopped);

        let collected = states.lock().unwrap();
        assert_eq!(
            *collected,
            vec![
                ExtensionHostState::Starting,
                ExtensionHostState::Running,
                ExtensionHostState::Error("crash".into()),
                ExtensionHostState::Stopped,
            ]
        );
    }

    #[test]
    fn parse_minimal_package_json() {
        let json = r#"{ "name": "minimal" }"#;
        let ext = ExtensionDescription::from_package_json(json, VsUri::file("/ext")).unwrap();
        assert_eq!(ext.id, "minimal");
        assert_eq!(ext.name, "minimal");
        assert_eq!(ext.display_name, "minimal");
        assert!(ext.main.is_none());
        assert!(ext.activation_events.is_empty());
        assert_eq!(ext.extension_kind, ExtensionKind::Both);
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let result = ExtensionDescription::from_package_json("not json", VsUri::file("/x"));
        assert!(result.is_err());
    }
}
