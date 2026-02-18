//! Extension host process management
//!
//! Manages extension descriptions parsed from `package.json` and tracks
//! extension host lifecycle state and activation. Provides child-process
//! spawning with a `Content-Length`-framed JSON-RPC transport for
//! communicating with VS Code extension host processes.

use std::collections::HashMap;
pub mod handlers;
pub mod process;
pub mod scanner;
pub mod transport;

use std::fmt;
use std::io;

use serde::Deserialize;
use vsedit_events::{Emitter, Event};
use vsedit_uri::VsUri;

pub use handlers::MainThreadHandlers;
pub use handlers::get_output_lines;
pub use process::{ExtensionHostConfig, ExtensionHostProcess, ExtensionRuntime};
pub use scanner::scan_extensions;
pub use transport::RpcTransport;

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
///
/// Optionally owns an [`ExtensionHostProcess`] that is spawned via
/// [`start_host`](Self::start_host) and torn down via
/// [`stop_host`](Self::stop_host).
pub struct ExtensionHostManager {
    extensions: Vec<ExtensionDescription>,
    state: ExtensionHostState,
    activated: Vec<String>,
    on_did_change_state: Emitter<ExtensionHostState>,
    process: Option<ExtensionHostProcess>,
    config: ExtensionHostConfig,
}

impl ExtensionHostManager {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            state: ExtensionHostState::Stopped,
            activated: Vec::new(),
            on_did_change_state: Emitter::new(),
            process: None,
            config: ExtensionHostConfig::default(),
        }
    }

    /// Create a manager with a specific host configuration.
    pub fn with_config(config: ExtensionHostConfig) -> Self {
        Self {
            extensions: Vec::new(),
            state: ExtensionHostState::Stopped,
            activated: Vec::new(),
            on_did_change_state: Emitter::new(),
            process: None,
            config,
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

    // -- Process management -------------------------------------------------

    /// Spawn the extension host child process.
    ///
    /// Transitions the state to `Starting` and then `Running` on success, or
    /// `Error` on failure.
    pub fn start_host(&mut self) -> io::Result<()> {
        if self.process.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "extension host is already running",
            ));
        }
        self.set_state(ExtensionHostState::Starting);

        match ExtensionHostProcess::spawn(&self.config) {
            Ok(proc) => {
                self.process = Some(proc);
                self.set_state(ExtensionHostState::Running);
                Ok(())
            }
            Err(e) => {
                self.set_state(ExtensionHostState::Error(e.to_string()));
                Err(e)
            }
        }
    }

    /// Stop the extension host child process (if running).
    pub fn stop_host(&mut self) {
        if let Some(mut proc) = self.process.take() {
            proc.kill();
        }
        self.set_state(ExtensionHostState::Stopped);
    }

    /// Check whether the extension host process is currently running.
    pub fn is_host_running(&mut self) -> bool {
        match &mut self.process {
            Some(proc) => proc.is_alive(),
            None => false,
        }
    }

    /// Access the underlying host process (if running) for sending/receiving
    /// messages.
    pub fn process_mut(&mut self) -> Option<&mut ExtensionHostProcess> {
        self.process.as_mut()
    }
}

impl Default for ExtensionHostManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Additional impl blocks – utility helpers, predicates, conversions
// ---------------------------------------------------------------------------

impl ContributedCommand {
    /// Return the qualified command string: `"category: title"` when a category
    /// is present, or just the title otherwise.
    pub fn qualified_title(&self) -> String {
        match &self.category {
            Some(cat) => format!("{}: {}", cat, self.title),
            None => self.title.clone(),
        }
    }

    /// Whether this command belongs to the given category (case-insensitive).
    pub fn is_in_category(&self, category: &str) -> bool {
        self.category
            .as_deref()
            .map(|c| c.eq_ignore_ascii_case(category))
            .unwrap_or(false)
    }
}

impl ContributedLanguage {
    /// Check whether a file extension (e.g. `".rs"`) is associated with this
    /// language.
    pub fn matches_extension(&self, ext: &str) -> bool {
        self.extensions.iter().any(|e| e == ext)
    }

    /// Check whether `name` matches any of this language's aliases
    /// (case-insensitive).
    pub fn has_alias(&self, name: &str) -> bool {
        self.aliases
            .iter()
            .any(|a| a.eq_ignore_ascii_case(name))
    }
}

impl ContributedGrammar {
    /// Whether this grammar targets the given language id.
    pub fn is_for_language(&self, lang: &str) -> bool {
        self.language == lang
    }
}

impl ContributedTheme {
    /// Whether this is a dark theme (ui_theme contains "dark").
    pub fn is_dark(&self) -> bool {
        self.ui_theme.contains("dark")
    }

    /// Whether this is a light theme (ui_theme contains "light").
    pub fn is_light(&self) -> bool {
        self.ui_theme.contains("light")
    }

    /// Whether this is a high-contrast theme.
    pub fn is_high_contrast(&self) -> bool {
        self.ui_theme.contains("hc")
    }
}

impl ContributedKeybinding {
    /// Whether this keybinding is conditional (has a `when` clause).
    pub fn is_conditional(&self) -> bool {
        self.when.is_some()
    }

    /// Whether the key chord contains the given modifier (case-insensitive),
    /// e.g. `"ctrl"`, `"shift"`, `"alt"`, `"meta"`.
    pub fn has_modifier(&self, modifier: &str) -> bool {
        self.key
            .to_ascii_lowercase()
            .contains(&modifier.to_ascii_lowercase())
    }
}

impl ExtensionContributions {
    /// Return `true` when the extension contributes nothing at all.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
            && self.languages.is_empty()
            && self.grammars.is_empty()
            && self.themes.is_empty()
            && self.keybindings.is_empty()
            && self.views.is_empty()
            && self.configuration.is_empty()
            && self.menus.as_object().map_or(true, |m| m.is_empty())
    }

    /// Total number of contribution items across all categories.
    pub fn total_count(&self) -> usize {
        self.commands.len()
            + self.languages.len()
            + self.grammars.len()
            + self.themes.len()
            + self.keybindings.len()
            + self.views.len()
            + self.configuration.len()
    }

    /// Find a command contribution by its command id.
    pub fn find_command(&self, command_id: &str) -> Option<&ContributedCommand> {
        self.commands.iter().find(|c| c.command == command_id)
    }

    /// Find a language contribution by file extension (e.g. `".rs"`).
    pub fn language_for_extension(&self, ext: &str) -> Option<&ContributedLanguage> {
        self.languages.iter().find(|l| l.matches_extension(ext))
    }
}

impl ExtensionKind {
    /// Whether this kind includes workspace functionality.
    pub fn includes_workspace(&self) -> bool {
        matches!(self, ExtensionKind::Workspace | ExtensionKind::Both)
    }

    /// Whether this kind includes UI functionality.
    pub fn includes_ui(&self) -> bool {
        matches!(self, ExtensionKind::UI | ExtensionKind::Both)
    }
}

impl ExtensionDescription {
    /// Whether this extension has a runnable entry-point (`main` field).
    pub fn is_runnable(&self) -> bool {
        self.main.is_some()
    }

    /// Whether this extension activates eagerly (has `"*"` activation event).
    pub fn is_eager(&self) -> bool {
        self.activation_events.iter().any(|e| e == "*")
    }

    /// Return parsed activation events.
    pub fn parsed_activation_events(&self) -> Vec<ActivationEvent> {
        self.activation_events
            .iter()
            .map(|raw| ActivationEvent::parse(raw))
            .collect()
    }

    /// Whether this extension responds to the given language activation.
    pub fn activates_on_language(&self, lang: &str) -> bool {
        let needle = format!("onLanguage:{lang}");
        self.activation_events.iter().any(|e| e == &needle || e == "*")
    }
}

impl ExtensionHostState {
    /// Whether the host is in an operational state (Starting or Running).
    pub fn is_alive(&self) -> bool {
        matches!(self, ExtensionHostState::Starting | ExtensionHostState::Running)
    }

    /// Whether the host terminated with an error.
    pub fn is_error(&self) -> bool {
        matches!(self, ExtensionHostState::Error(_))
    }

    /// Extract the error message, if any.
    pub fn error_message(&self) -> Option<&str> {
        match self {
            ExtensionHostState::Error(msg) => Some(msg.as_str()),
            _ => None,
        }
    }
}

impl ExtensionHostManager {
    /// Return the number of registered extensions.
    pub fn extension_count(&self) -> usize {
        self.extensions.len()
    }

    /// Return the number of activated extensions.
    pub fn activated_count(&self) -> usize {
        self.activated.len()
    }

    /// Remove all registered extensions and reset activation state.
    pub fn clear_extensions(&mut self) {
        self.extensions.clear();
        self.activated.clear();
    }

    /// Return ids of all activated extensions.
    pub fn activated_ids(&self) -> &[String] {
        &self.activated
    }

    /// Find all extensions that contribute a given command id.
    pub fn extensions_for_command(&self, command_id: &str) -> Vec<&ExtensionDescription> {
        self.extensions
            .iter()
            .filter(|ext| ext.contributes.find_command(command_id).is_some())
            .collect()
    }
}

impl ActivationEvent {
    /// Whether this is a wildcard event.
    pub fn is_star(&self) -> bool {
        matches!(self, ActivationEvent::Star)
    }

    /// Extract the language id if this is an `OnLanguage` event.
    pub fn language(&self) -> Option<&str> {
        match self {
            ActivationEvent::OnLanguage(lang) => Some(lang.as_str()),
            _ => None,
        }
    }

    /// Extract the command id if this is an `OnCommand` event.
    pub fn command(&self) -> Option<&str> {
        match self {
            ActivationEvent::OnCommand(cmd) => Some(cmd.as_str()),
            _ => None,
        }
    }

    /// Serialize back to the raw activation event string.
    pub fn to_raw(&self) -> String {
        match self {
            ActivationEvent::Star => "*".to_string(),
            ActivationEvent::OnStartupFinished => "onStartupFinished".to_string(),
            ActivationEvent::OnLanguage(l) => format!("onLanguage:{l}"),
            ActivationEvent::OnCommand(c) => format!("onCommand:{c}"),
            ActivationEvent::WorkspaceContains(p) => format!("workspaceContains:{p}"),
            ActivationEvent::Unknown(s) => s.clone(),
        }
    }
}

impl ContributionPointRegistry {
    /// Look up a command by its command id across all extensions.
    pub fn find_command(&self, command_id: &str) -> Option<&CommandContribution> {
        self.commands.iter().find(|c| c.command == command_id)
    }

    /// Look up a language by its id.
    pub fn find_language(&self, lang_id: &str) -> Option<&LanguageContribution> {
        self.languages.iter().find(|l| l.id == lang_id)
    }

    /// Find the language contribution that matches a file extension.
    pub fn language_for_file_extension(&self, ext: &str) -> Option<&LanguageContribution> {
        self.languages
            .iter()
            .find(|l| l.extensions.iter().any(|e| e == ext))
    }

    /// Return all commands contributed by a specific extension.
    pub fn commands_by_extension(&self, ext_id: &str) -> Vec<&CommandContribution> {
        self.commands
            .iter()
            .filter(|c| c.extension_id == ext_id)
            .collect()
    }

    /// Total number of contribution items across all categories.
    pub fn total_count(&self) -> usize {
        self.commands.len()
            + self.languages.len()
            + self.themes.len()
            + self.snippets.len()
            + self.grammars.len()
            + self.debuggers.len()
            + self.views.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for ext-host operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtHostStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtHostStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &ExtHostStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for ExtHostStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtHostStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtHostStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-host.
#[derive(Debug, Clone)]
pub struct ExtHostValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtHostValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for ExtHostValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ExtensionScanner
// ---------------------------------------------------------------------------

/// Scans a directory tree for extensions by locating `package.json` files.
///
/// Unlike the low-level [`scan_extensions`] function this struct retains the
/// root path and provides a higher-level API for repeated scans.
pub struct ExtensionScanner {
    root: std::path::PathBuf,
}

impl ExtensionScanner {
    /// Create a scanner rooted at `path`.
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { root: path.into() }
    }

    /// Scan the root directory for extensions.
    pub fn scan_directory(&self) -> Vec<ExtensionDescription> {
        scanner::scan_extensions(&self.root)
    }

    /// Return the root path being scanned.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }
}

// ---------------------------------------------------------------------------
// ActivationEventHandler
// ---------------------------------------------------------------------------

/// Activation event kinds understood by the extension host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationEvent {
    /// `onLanguage:X`
    OnLanguage(String),
    /// `onCommand:X`
    OnCommand(String),
    /// `workspaceContains:X`
    WorkspaceContains(String),
    /// `onStartupFinished`
    OnStartupFinished,
    /// `*` — always activate
    Star,
    /// An unrecognised activation event string.
    Unknown(String),
}

impl ActivationEvent {
    /// Parse a raw activation event string.
    pub fn parse(raw: &str) -> Self {
        if raw == "*" {
            return Self::Star;
        }
        if raw == "onStartupFinished" {
            return Self::OnStartupFinished;
        }
        if let Some(lang) = raw.strip_prefix("onLanguage:") {
            return Self::OnLanguage(lang.to_string());
        }
        if let Some(cmd) = raw.strip_prefix("onCommand:") {
            return Self::OnCommand(cmd.to_string());
        }
        if let Some(pat) = raw.strip_prefix("workspaceContains:") {
            return Self::WorkspaceContains(pat.to_string());
        }
        Self::Unknown(raw.to_string())
    }
}

/// Decides which extensions should activate in response to events.
pub struct ActivationEventHandler {
    extensions: Vec<ExtensionDescription>,
    activated: std::collections::HashSet<String>,
}

impl ActivationEventHandler {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            activated: std::collections::HashSet::new(),
        }
    }

    /// Register an extension for activation tracking.
    pub fn register(&mut self, ext: ExtensionDescription) {
        self.extensions.push(ext);
    }

    /// Check whether a specific extension should activate for the given event.
    pub fn should_activate(&self, event: &ActivationEvent, ext: &ExtensionDescription) -> bool {
        ext.activation_events.iter().any(|raw| {
            let parsed = ActivationEvent::parse(raw);
            parsed == ActivationEvent::Star || parsed == *event
        })
    }

    /// Return all registered extensions that are waiting to be activated
    /// (i.e. not yet marked as activated).
    pub fn pending_activations(&self) -> Vec<&ExtensionDescription> {
        self.extensions
            .iter()
            .filter(|ext| !self.activated.contains(&ext.id))
            .collect()
    }

    /// Mark an extension as activated.
    pub fn mark_activated(&mut self, id: &str) {
        self.activated.insert(id.to_string());
    }

    /// Return all extensions that should activate for the given event and
    /// have not yet been activated.
    pub fn extensions_to_activate(&self, event: &ActivationEvent) -> Vec<&ExtensionDescription> {
        self.extensions
            .iter()
            .filter(|ext| {
                !self.activated.contains(&ext.id) && self.should_activate(event, ext)
            })
            .collect()
    }
}

impl Default for ActivationEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ContributionPointRegistry
// ---------------------------------------------------------------------------

/// A command contribution entry.
#[derive(Debug, Clone)]
pub struct CommandContribution {
    pub extension_id: String,
    pub command: String,
    pub title: String,
    pub category: Option<String>,
}

/// A language contribution entry.
#[derive(Debug, Clone)]
pub struct LanguageContribution {
    pub extension_id: String,
    pub id: String,
    pub extensions: Vec<String>,
    pub aliases: Vec<String>,
}

/// A snippet contribution entry.
#[derive(Debug, Clone)]
pub struct SnippetContribution {
    pub extension_id: String,
    pub language: String,
    pub path: String,
}

/// A debugger contribution entry.
#[derive(Debug, Clone)]
pub struct DebuggerContribution {
    pub extension_id: String,
    pub debugger_type: String,
    pub label: String,
}

/// A view contribution entry.
#[derive(Debug, Clone)]
pub struct ViewContribution {
    pub extension_id: String,
    pub id: String,
    pub name: String,
}

/// Tracks what extensions contribute across all registered extensions.
#[derive(Debug, Default)]
pub struct ContributionPointRegistry {
    commands: Vec<CommandContribution>,
    languages: Vec<LanguageContribution>,
    themes: Vec<ContributedTheme>,
    snippets: Vec<SnippetContribution>,
    grammars: Vec<ContributedGrammar>,
    debuggers: Vec<DebuggerContribution>,
    views: Vec<ViewContribution>,
}

impl ContributionPointRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register all contributions from an extension's `contributes` JSON
    /// section. Falls back gracefully when fields are missing.
    pub fn register_contributions(&mut self, ext_id: &str, contributes: &serde_json::Value) {
        if let Some(cmds) = contributes.get("commands").and_then(|v| v.as_array()) {
            for cmd in cmds {
                if let (Some(command), Some(title)) =
                    (cmd.get("command").and_then(|v| v.as_str()),
                     cmd.get("title").and_then(|v| v.as_str()))
                {
                    self.commands.push(CommandContribution {
                        extension_id: ext_id.to_string(),
                        command: command.to_string(),
                        title: title.to_string(),
                        category: cmd.get("category").and_then(|v| v.as_str()).map(String::from),
                    });
                }
            }
        }
        if let Some(langs) = contributes.get("languages").and_then(|v| v.as_array()) {
            for lang in langs {
                if let Some(id) = lang.get("id").and_then(|v| v.as_str()) {
                    let extensions = lang.get("extensions")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    let aliases = lang.get("aliases")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    self.languages.push(LanguageContribution {
                        extension_id: ext_id.to_string(),
                        id: id.to_string(),
                        extensions,
                        aliases,
                    });
                }
            }
        }
        if let Some(themes) = contributes.get("themes").and_then(|v| v.as_array()) {
            for t in themes {
                if let (Some(label), Some(ui_theme), Some(path)) = (
                    t.get("label").and_then(|v| v.as_str()),
                    t.get("uiTheme").and_then(|v| v.as_str()),
                    t.get("path").and_then(|v| v.as_str()),
                ) {
                    self.themes.push(ContributedTheme {
                        label: label.to_string(),
                        ui_theme: ui_theme.to_string(),
                        path: path.to_string(),
                    });
                }
            }
        }
        if let Some(snippets) = contributes.get("snippets").and_then(|v| v.as_array()) {
            for s in snippets {
                if let (Some(language), Some(path)) = (
                    s.get("language").and_then(|v| v.as_str()),
                    s.get("path").and_then(|v| v.as_str()),
                ) {
                    self.snippets.push(SnippetContribution {
                        extension_id: ext_id.to_string(),
                        language: language.to_string(),
                        path: path.to_string(),
                    });
                }
            }
        }
        if let Some(grams) = contributes.get("grammars").and_then(|v| v.as_array()) {
            for g in grams {
                if let (Some(language), Some(scope_name), Some(path)) = (
                    g.get("language").and_then(|v| v.as_str()),
                    g.get("scopeName").and_then(|v| v.as_str()),
                    g.get("path").and_then(|v| v.as_str()),
                ) {
                    self.grammars.push(ContributedGrammar {
                        language: language.to_string(),
                        scope_name: scope_name.to_string(),
                        path: path.to_string(),
                    });
                }
            }
        }
        if let Some(debuggers) = contributes.get("debuggers").and_then(|v| v.as_array()) {
            for d in debuggers {
                if let (Some(dtype), Some(label)) = (
                    d.get("type").and_then(|v| v.as_str()),
                    d.get("label").and_then(|v| v.as_str()),
                ) {
                    self.debuggers.push(DebuggerContribution {
                        extension_id: ext_id.to_string(),
                        debugger_type: dtype.to_string(),
                        label: label.to_string(),
                    });
                }
            }
        }
        if let Some(views_obj) = contributes.get("views").and_then(|v| v.as_object()) {
            for (_container, entries) in views_obj {
                if let Some(arr) = entries.as_array() {
                    for v in arr {
                        if let (Some(id), Some(name)) = (
                            v.get("id").and_then(|v| v.as_str()),
                            v.get("name").and_then(|v| v.as_str()),
                        ) {
                            self.views.push(ViewContribution {
                                extension_id: ext_id.to_string(),
                                id: id.to_string(),
                                name: name.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    pub fn get_commands(&self) -> &[CommandContribution] {
        &self.commands
    }

    pub fn get_languages(&self) -> &[LanguageContribution] {
        &self.languages
    }

    pub fn get_themes(&self) -> &[ContributedTheme] {
        &self.themes
    }

    pub fn get_snippets(&self) -> &[SnippetContribution] {
        &self.snippets
    }

    pub fn get_grammars(&self) -> &[ContributedGrammar] {
        &self.grammars
    }

    pub fn get_debuggers(&self) -> &[DebuggerContribution] {
        &self.debuggers
    }

    pub fn get_views(&self) -> &[ViewContribution] {
        &self.views
    }
}


// === Extension Host Restart Handler ===

/// Extension Host Restart Handler implementation.
#[derive(Debug, Clone)]
pub struct ExtensionHostRestartHandler {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: ExtensionHostRestartHandlerStats,
}

/// Statistics for ExtensionHostRestartHandler.
#[derive(Debug, Clone, Default)]
pub struct ExtensionHostRestartHandlerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl ExtensionHostRestartHandlerStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl ExtensionHostRestartHandler {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: ExtensionHostRestartHandlerStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &ExtensionHostRestartHandlerStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for ExtensionHostRestartHandler {
    fn default() -> Self {
        Self::new()
    }
}

// === Extension Host Memory Monitor ===

/// Priority level for ExtensionHostMemoryMonitor items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExtensionHostMemoryMonitorPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl ExtensionHostMemoryMonitorPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for ExtensionHostMemoryMonitorPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Extension Host Memory Monitor implementation.
#[derive(Debug, Clone)]
pub struct ExtensionHostMemoryMonitor {
    items: Vec<ExtensionHostMemoryMonitorItem>,
    max_items: usize,
    default_priority: ExtensionHostMemoryMonitorPriority,
}

/// A single item in ExtensionHostMemoryMonitor.
#[derive(Debug, Clone)]
pub struct ExtensionHostMemoryMonitorItem {
    pub id: String,
    pub label: String,
    pub priority: ExtensionHostMemoryMonitorPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl ExtensionHostMemoryMonitorItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: ExtensionHostMemoryMonitorPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: ExtensionHostMemoryMonitorPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl ExtensionHostMemoryMonitor {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: ExtensionHostMemoryMonitorPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: ExtensionHostMemoryMonitorItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<ExtensionHostMemoryMonitorItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&ExtensionHostMemoryMonitorItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: ExtensionHostMemoryMonitorPriority) -> Vec<&ExtensionHostMemoryMonitorItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ExtensionHostMemoryMonitorItem> {
        let mut sorted: Vec<&ExtensionHostMemoryMonitorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&ExtensionHostMemoryMonitorItem> {
        let mut sorted: Vec<&ExtensionHostMemoryMonitorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&ExtensionHostMemoryMonitorItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: ExtensionHostMemoryMonitorPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> ExtensionHostMemoryMonitorPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &ExtensionHostMemoryMonitorItem> {
        self.items.iter()
    }
}

impl Default for ExtensionHostMemoryMonitor {
    fn default() -> Self {
        Self::new()
    }
}


/// Configuration manager for ext_host functionality.
pub struct ExtHostConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ExtHostConfig {
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

    pub fn merge(&mut self, other: &ExtHostConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for ext_host operations.
pub struct ExtHostRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ExtHostRateTracker {
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

/// Validation result collector for ext_host.
pub struct ExtHostValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ExtHostValidationCollector {
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

    pub fn merge(&mut self, other: &ExtHostValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Extension host lifecycle management — extended utilities (zu)
// ---------------------------------------------------------------------------

/// Metric accumulator for ext_host operations.
#[derive(Debug, Clone)]
pub struct ZuMetrics {
    samples: Vec<f64>,
    label: String,
}

impl ZuMetrics {
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

/// Sliding-window rate counter for ext_host.
#[derive(Debug, Clone)]
pub struct ZuRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl ZuRateWindow {
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

/// A small LRU-style cache for ext_host lookups.
#[derive(Debug, Clone)]
pub struct ZuLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZuLruCache {
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
// xa_ extended helpers for ext_host
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtHostRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtHostRingBuf {
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
pub struct XaExtHostCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtHostCounter {
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

impl Default for XaExtHostCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 60
// ---------------------------------------------------------------------------

/// Generic object pool `Xc60Pool<T>`.
pub struct Xc60Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc60Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc60PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc60Pool<T> {
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
    pub fn stats(&self) -> Xc60PoolStats {
        Xc60PoolStats {
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

impl<T> Default for Xc60Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc60Scheduler`.
pub struct Xc60Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc60Scheduler {
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

impl Default for Xc60Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_60 hash for the given byte slice.
pub fn xc_60_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_60 convention.
pub fn xc_60_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_33 deepening: state machine + event bus ---

/// States for the Xd33 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd33State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd33State {
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
pub struct Xd33Transition {
    pub from: Xd33State,
    pub to: Xd33State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd33StateMachine {
    current: Xd33State,
    history: Vec<Xd33Transition>,
    step_counter: usize,
}

impl Xd33StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd33State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd33State {
        self.current
    }

    pub fn history(&self) -> &[Xd33Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd33State) -> Result<Xd33State, String> {
        let allowed = match (self.current, target) {
            (Xd33State::Idle, Xd33State::Running) => true,
            (Xd33State::Running, Xd33State::Paused) => true,
            (Xd33State::Running, Xd33State::Done) => true,
            (Xd33State::Paused, Xd33State::Running) => true,
            (Xd33State::Paused, Xd33State::Done) => true,
            (Xd33State::Done, Xd33State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_33: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd33Transition {
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
            "Xd33SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd33State> {
        let prefix = "Xd33SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd33State::Idle),
            "Running" => Some(Xd33State::Running),
            "Paused" => Some(Xd33State::Paused),
            "Done" => Some(Xd33State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd33State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd33 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd33Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd33Event {
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

type Xd33HandlerFn = Box<dyn Fn(&Xd33Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd33EventBus {
    handlers: Vec<(usize, Option<String>, Xd33HandlerFn)>,
    next_id: usize,
    published: Vec<Xd33Event>,
}

impl Xd33EventBus {
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
        F: Fn(&Xd33Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd33Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd33Event) {
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

    pub fn published_events(&self) -> &[Xd33Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #31
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf31Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf31TrieNode {
    children: std::collections::HashMap<char, Xf31TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf31Trie {
    root: Xf31TrieNode,
    count: usize,
}

impl Xf31Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf31TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf31TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf31TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf31BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf31BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 59).
pub struct Xh59SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh59SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 101 as u64,
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

/// A compact bit set supporting boolean operations (variant 59).
pub struct Xh59BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh59BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 59).
pub struct Xi59Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi59Deque<T> {
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
pub struct Xi59Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi59Interval {
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

/// A simple interval tree (variant 59).
pub struct Xi59IntervalTree {
    xi_intervals: Vec<Xi59Interval>,
}

impl Xi59IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi59Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi59Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi59Interval) -> Vec<&Xi59Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi59Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi59Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi59Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi59Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi59Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi59Interval> = Vec::new();
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

    #[test]
    fn ext_host_stats_new_defaults() {
        let stats = ExtHostStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_host_stats_record_success() {
        let mut stats = ExtHostStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_host_stats_record_failure() {
        let mut stats = ExtHostStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_host_stats_reset() {
        let mut stats = ExtHostStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_host_stats_merge() {
        let mut a = ExtHostStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtHostStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn ext_host_stats_display() {
        let mut stats = ExtHostStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_host_stats_default() {
        let stats = ExtHostStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_host_validator_accepts_and_rejects() {
        let mut v = ExtHostValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad extension");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad extension"));
    }

    #[test]
    fn ext_host_validator_warnings() {
        let mut v = ExtHostValidationCollector::new();
        v.add_warning("deprecated ext");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn ext_host_validator_clear_and_merge() {
        let mut v = ExtHostValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = ExtHostValidationCollector::new();
        a.add_error("a_err");
        let mut b = ExtHostValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    // -- ExtensionScanner tests ------------------------------------------------

    #[test]
    fn scanner_empty_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let scanner = ExtensionScanner::new(tmp.path());
        assert!(scanner.scan_directory().is_empty());
        assert_eq!(scanner.root(), tmp.path());
    }

    #[test]
    fn scanner_finds_extension() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ext_dir = tmp.path().join("my-ext");
        std::fs::create_dir(&ext_dir).unwrap();
        std::fs::write(
            ext_dir.join("package.json"),
            r#"{"name":"my-ext","publisher":"test","version":"1.0.0"}"#,
        ).unwrap();
        let scanner = ExtensionScanner::new(tmp.path());
        let exts = scanner.scan_directory();
        assert_eq!(exts.len(), 1);
        assert_eq!(exts[0].id, "test.my-ext");
    }

    #[test]
    fn scanner_nonexistent_dir() {
        let scanner = ExtensionScanner::new("/tmp/vsedit-scanner-does-not-exist-999");
        assert!(scanner.scan_directory().is_empty());
    }

    // -- ActivationEvent tests -------------------------------------------------

    #[test]
    fn parse_activation_event_on_language() {
        assert_eq!(
            ActivationEvent::parse("onLanguage:rust"),
            ActivationEvent::OnLanguage("rust".to_string())
        );
    }

    #[test]
    fn parse_activation_event_on_command() {
        assert_eq!(
            ActivationEvent::parse("onCommand:editor.action.format"),
            ActivationEvent::OnCommand("editor.action.format".to_string())
        );
    }

    #[test]
    fn parse_activation_event_workspace_contains() {
        assert_eq!(
            ActivationEvent::parse("workspaceContains:Cargo.toml"),
            ActivationEvent::WorkspaceContains("Cargo.toml".to_string())
        );
    }

    #[test]
    fn parse_activation_event_star() {
        assert_eq!(ActivationEvent::parse("*"), ActivationEvent::Star);
    }

    #[test]
    fn parse_activation_event_startup_finished() {
        assert_eq!(
            ActivationEvent::parse("onStartupFinished"),
            ActivationEvent::OnStartupFinished
        );
    }

    #[test]
    fn parse_activation_event_unknown() {
        assert_eq!(
            ActivationEvent::parse("onSomethingElse"),
            ActivationEvent::Unknown("onSomethingElse".to_string())
        );
    }

    #[test]
    fn activation_handler_should_activate_language() {
        let handler = ActivationEventHandler::new();
        let ext = ExtensionDescription {
            id: "ext-a".into(),
            name: "a".into(),
            display_name: "A".into(),
            version: "0.1.0".into(),
            publisher: "test".into(),
            main: None,
            activation_events: vec!["onLanguage:rust".into()],
            contributes: ExtensionContributions::default(),
            extension_kind: ExtensionKind::Both,
            is_builtin: false,
            location: VsUri::file("/ext/a"),
        };
        let event = ActivationEvent::OnLanguage("rust".into());
        assert!(handler.should_activate(&event, &ext));
        let wrong = ActivationEvent::OnLanguage("python".into());
        assert!(!handler.should_activate(&wrong, &ext));
    }

    #[test]
    fn activation_handler_star_matches_all() {
        let handler = ActivationEventHandler::new();
        let ext = ExtensionDescription {
            id: "ext-star".into(),
            name: "star".into(),
            display_name: "Star".into(),
            version: "0.1.0".into(),
            publisher: "test".into(),
            main: None,
            activation_events: vec!["*".into()],
            contributes: ExtensionContributions::default(),
            extension_kind: ExtensionKind::Both,
            is_builtin: false,
            location: VsUri::file("/ext/star"),
        };
        assert!(handler.should_activate(&ActivationEvent::OnLanguage("anything".into()), &ext));
        assert!(handler.should_activate(&ActivationEvent::OnStartupFinished, &ext));
    }

    #[test]
    fn activation_handler_pending_and_mark() {
        let mut handler = ActivationEventHandler::new();
        handler.register(ExtensionDescription {
            id: "ext-1".into(),
            name: "one".into(),
            display_name: "One".into(),
            version: "0.1.0".into(),
            publisher: "test".into(),
            main: None,
            activation_events: vec!["onLanguage:rust".into()],
            contributes: ExtensionContributions::default(),
            extension_kind: ExtensionKind::Both,
            is_builtin: false,
            location: VsUri::file("/ext/1"),
        });
        assert_eq!(handler.pending_activations().len(), 1);
        handler.mark_activated("ext-1");
        assert_eq!(handler.pending_activations().len(), 0);
    }

    #[test]
    fn activation_handler_extensions_to_activate() {
        let mut handler = ActivationEventHandler::new();
        handler.register(ExtensionDescription {
            id: "ext-rust".into(),
            name: "rust".into(),
            display_name: "Rust".into(),
            version: "0.1.0".into(),
            publisher: "test".into(),
            main: None,
            activation_events: vec!["onLanguage:rust".into()],
            contributes: ExtensionContributions::default(),
            extension_kind: ExtensionKind::Both,
            is_builtin: false,
            location: VsUri::file("/ext/rust"),
        });
        handler.register(ExtensionDescription {
            id: "ext-py".into(),
            name: "py".into(),
            display_name: "Python".into(),
            version: "0.1.0".into(),
            publisher: "test".into(),
            main: None,
            activation_events: vec!["onLanguage:python".into()],
            contributes: ExtensionContributions::default(),
            extension_kind: ExtensionKind::Both,
            is_builtin: false,
            location: VsUri::file("/ext/py"),
        });
        let event = ActivationEvent::OnLanguage("rust".into());
        let to_activate = handler.extensions_to_activate(&event);
        assert_eq!(to_activate.len(), 1);
        assert_eq!(to_activate[0].id, "ext-rust");
    }

    // -- ContributionPointRegistry tests ---------------------------------------

    #[test]
    fn registry_register_commands() {
        let mut reg = ContributionPointRegistry::new();
        let json: serde_json::Value = serde_json::from_str(r#"{
            "commands": [
                {"command": "ext.hello", "title": "Hello", "category": "Greet"},
                {"command": "ext.bye", "title": "Goodbye"}
            ]
        }"#).unwrap();
        reg.register_contributions("test-ext", &json);
        let cmds = reg.get_commands();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].command, "ext.hello");
        assert_eq!(cmds[0].title, "Hello");
        assert_eq!(cmds[0].category.as_deref(), Some("Greet"));
        assert_eq!(cmds[1].category, None);
        assert_eq!(cmds[0].extension_id, "test-ext");
    }

    #[test]
    fn registry_register_languages() {
        let mut reg = ContributionPointRegistry::new();
        let json: serde_json::Value = serde_json::from_str(r#"{
            "languages": [
                {"id": "rust", "extensions": [".rs"], "aliases": ["Rust"]}
            ]
        }"#).unwrap();
        reg.register_contributions("rust-ext", &json);
        let langs = reg.get_languages();
        assert_eq!(langs.len(), 1);
        assert_eq!(langs[0].id, "rust");
        assert_eq!(langs[0].extensions, vec![".rs"]);
        assert_eq!(langs[0].aliases, vec!["Rust"]);
    }

    #[test]
    fn registry_register_themes() {
        let mut reg = ContributionPointRegistry::new();
        let json: serde_json::Value = serde_json::from_str(r#"{
            "themes": [
                {"label": "Dark", "uiTheme": "vs-dark", "path": "./dark.json"}
            ]
        }"#).unwrap();
        reg.register_contributions("theme-ext", &json);
        assert_eq!(reg.get_themes().len(), 1);
        assert_eq!(reg.get_themes()[0].label, "Dark");
    }

    #[test]
    fn registry_register_grammars() {
        let mut reg = ContributionPointRegistry::new();
        let json: serde_json::Value = serde_json::from_str(r#"{
            "grammars": [
                {"language": "rust", "scopeName": "source.rust", "path": "./rust.json"}
            ]
        }"#).unwrap();
        reg.register_contributions("gram-ext", &json);
        assert_eq!(reg.get_grammars().len(), 1);
        assert_eq!(reg.get_grammars()[0].scope_name, "source.rust");
    }

    #[test]
    fn registry_register_snippets() {
        let mut reg = ContributionPointRegistry::new();
        let json: serde_json::Value = serde_json::from_str(r#"{
            "snippets": [
                {"language": "rust", "path": "./snippets/rust.json"}
            ]
        }"#).unwrap();
        reg.register_contributions("snip-ext", &json);
        assert_eq!(reg.get_snippets().len(), 1);
        assert_eq!(reg.get_snippets()[0].language, "rust");
    }

    #[test]
    fn registry_register_debuggers() {
        let mut reg = ContributionPointRegistry::new();
        let json: serde_json::Value = serde_json::from_str(r#"{
            "debuggers": [
                {"type": "lldb", "label": "LLDB Debugger"}
            ]
        }"#).unwrap();
        reg.register_contributions("dbg-ext", &json);
        assert_eq!(reg.get_debuggers().len(), 1);
        assert_eq!(reg.get_debuggers()[0].debugger_type, "lldb");
    }

    #[test]
    fn registry_register_views() {
        let mut reg = ContributionPointRegistry::new();
        let json: serde_json::Value = serde_json::from_str(r#"{
            "views": {
                "explorer": [
                    {"id": "myView", "name": "My View"}
                ]
            }
        }"#).unwrap();
        reg.register_contributions("view-ext", &json);
        assert_eq!(reg.get_views().len(), 1);
        assert_eq!(reg.get_views()[0].id, "myView");
        assert_eq!(reg.get_views()[0].name, "My View");
    }

    #[test]
    fn registry_empty_contributes() {
        let mut reg = ContributionPointRegistry::new();
        let json: serde_json::Value = serde_json::from_str(r#"{}"#).unwrap();
        reg.register_contributions("empty-ext", &json);
        assert!(reg.get_commands().is_empty());
        assert!(reg.get_languages().is_empty());
        assert!(reg.get_themes().is_empty());
    }

    #[test]
    fn registry_multiple_extensions() {
        let mut reg = ContributionPointRegistry::new();
        let json1: serde_json::Value = serde_json::from_str(r#"{
            "commands": [{"command": "a.cmd", "title": "A"}]
        }"#).unwrap();
        let json2: serde_json::Value = serde_json::from_str(r#"{
            "commands": [{"command": "b.cmd", "title": "B"}]
        }"#).unwrap();
        reg.register_contributions("ext-a", &json1);
        reg.register_contributions("ext-b", &json2);
        assert_eq!(reg.get_commands().len(), 2);
        assert_eq!(reg.get_commands()[0].extension_id, "ext-a");
        assert_eq!(reg.get_commands()[1].extension_id, "ext-b");
    }

    // -- ContributedCommand helpers -------------------------------------------

    #[test]
    fn contributed_command_qualified_title_with_category() {
        let cmd = ContributedCommand {
            command: "rust.build".into(),
            title: "Build".into(),
            category: Some("Rust".into()),
        };
        assert_eq!(cmd.qualified_title(), "Rust: Build");
    }

    #[test]
    fn contributed_command_qualified_title_without_category() {
        let cmd = ContributedCommand {
            command: "editor.format".into(),
            title: "Format Document".into(),
            category: None,
        };
        assert_eq!(cmd.qualified_title(), "Format Document");
    }

    #[test]
    fn contributed_command_is_in_category() {
        let cmd = ContributedCommand {
            command: "rust.build".into(),
            title: "Build".into(),
            category: Some("Rust".into()),
        };
        assert!(cmd.is_in_category("rust"));
        assert!(cmd.is_in_category("RUST"));
        assert!(!cmd.is_in_category("Go"));

        let uncategorized = ContributedCommand {
            command: "x".into(),
            title: "X".into(),
            category: None,
        };
        assert!(!uncategorized.is_in_category("anything"));
    }

    // -- ContributedLanguage helpers ------------------------------------------

    #[test]
    fn contributed_language_matches_extension() {
        let lang = ContributedLanguage {
            id: "rust".into(),
            extensions: vec![".rs".into(), ".rlib".into()],
            aliases: vec!["Rust".into()],
        };
        assert!(lang.matches_extension(".rs"));
        assert!(lang.matches_extension(".rlib"));
        assert!(!lang.matches_extension(".py"));
    }

    #[test]
    fn contributed_language_has_alias() {
        let lang = ContributedLanguage {
            id: "rust".into(),
            extensions: vec![".rs".into()],
            aliases: vec!["Rust".into(), "rs".into()],
        };
        assert!(lang.has_alias("rust"));
        assert!(lang.has_alias("RS"));
        assert!(!lang.has_alias("python"));
    }

    // -- ContributedGrammar helpers -------------------------------------------

    #[test]
    fn contributed_grammar_is_for_language() {
        let gram = ContributedGrammar {
            language: "rust".into(),
            scope_name: "source.rust".into(),
            path: "./rust.json".into(),
        };
        assert!(gram.is_for_language("rust"));
        assert!(!gram.is_for_language("python"));
    }

    // -- ContributedTheme helpers ---------------------------------------------

    #[test]
    fn contributed_theme_dark_light_hc() {
        let dark = ContributedTheme {
            label: "My Dark".into(),
            ui_theme: "vs-dark".into(),
            path: "./dark.json".into(),
        };
        assert!(dark.is_dark());
        assert!(!dark.is_light());
        assert!(!dark.is_high_contrast());

        let light = ContributedTheme {
            label: "My Light".into(),
            ui_theme: "vs-light".into(),
            path: "./light.json".into(),
        };
        assert!(light.is_light());
        assert!(!light.is_dark());

        let hc = ContributedTheme {
            label: "High Contrast".into(),
            ui_theme: "hc-black".into(),
            path: "./hc.json".into(),
        };
        assert!(hc.is_high_contrast());
    }

    // -- ContributedKeybinding helpers ----------------------------------------

    #[test]
    fn contributed_keybinding_is_conditional() {
        let conditional = ContributedKeybinding {
            command: "rust.build".into(),
            key: "ctrl+shift+b".into(),
            when: Some("editorLangId == rust".into()),
        };
        assert!(conditional.is_conditional());

        let unconditional = ContributedKeybinding {
            command: "editor.save".into(),
            key: "ctrl+s".into(),
            when: None,
        };
        assert!(!unconditional.is_conditional());
    }

    #[test]
    fn contributed_keybinding_has_modifier() {
        let kb = ContributedKeybinding {
            command: "x".into(),
            key: "Ctrl+Shift+B".into(),
            when: None,
        };
        assert!(kb.has_modifier("ctrl"));
        assert!(kb.has_modifier("shift"));
        assert!(!kb.has_modifier("alt"));
    }

    // -- ExtensionContributions helpers ---------------------------------------

    #[test]
    fn extension_contributions_is_empty() {
        let empty = ExtensionContributions::default();
        assert!(empty.is_empty());
        assert_eq!(empty.total_count(), 0);
    }

    #[test]
    fn extension_contributions_total_count() {
        let loc = VsUri::file("/ext/rust");
        let ext = ExtensionDescription::from_package_json(sample_package_json(), loc).unwrap();
        // 1 command + 1 language + 1 grammar + 1 theme + 1 keybinding = 5
        assert_eq!(ext.contributes.total_count(), 5);
        assert!(!ext.contributes.is_empty());
    }

    #[test]
    fn extension_contributions_find_command() {
        let loc = VsUri::file("/ext/rust");
        let ext = ExtensionDescription::from_package_json(sample_package_json(), loc).unwrap();
        assert!(ext.contributes.find_command("rust.build").is_some());
        assert!(ext.contributes.find_command("nonexistent").is_none());
    }

    #[test]
    fn extension_contributions_language_for_extension() {
        let loc = VsUri::file("/ext/rust");
        let ext = ExtensionDescription::from_package_json(sample_package_json(), loc).unwrap();
        let lang = ext.contributes.language_for_extension(".rs");
        assert!(lang.is_some());
        assert_eq!(lang.unwrap().id, "rust");
        assert!(ext.contributes.language_for_extension(".py").is_none());
    }

    // -- ExtensionKind helpers ------------------------------------------------

    #[test]
    fn extension_kind_includes() {
        assert!(ExtensionKind::Both.includes_workspace());
        assert!(ExtensionKind::Both.includes_ui());
        assert!(ExtensionKind::UI.includes_ui());
        assert!(!ExtensionKind::UI.includes_workspace());
        assert!(ExtensionKind::Workspace.includes_workspace());
        assert!(!ExtensionKind::Workspace.includes_ui());
    }

    // -- ExtensionDescription helpers -----------------------------------------

    #[test]
    fn extension_description_is_runnable() {
        let loc = VsUri::file("/ext/rust");
        let ext = ExtensionDescription::from_package_json(sample_package_json(), loc).unwrap();
        assert!(ext.is_runnable());

        let minimal = ExtensionDescription::from_package_json(
            r#"{"name":"no-main"}"#,
            VsUri::file("/x"),
        ).unwrap();
        assert!(!minimal.is_runnable());
    }

    #[test]
    fn extension_description_is_eager() {
        let eager = ExtensionDescription {
            id: "eager".into(),
            name: "eager".into(),
            display_name: "Eager".into(),
            version: "1.0.0".into(),
            publisher: "test".into(),
            main: None,
            activation_events: vec!["*".into()],
            contributes: ExtensionContributions::default(),
            extension_kind: ExtensionKind::Both,
            is_builtin: false,
            location: VsUri::file("/ext/eager"),
        };
        assert!(eager.is_eager());

        let lazy = ExtensionDescription {
            id: "lazy".into(),
            name: "lazy".into(),
            display_name: "Lazy".into(),
            version: "1.0.0".into(),
            publisher: "test".into(),
            main: None,
            activation_events: vec!["onLanguage:rust".into()],
            contributes: ExtensionContributions::default(),
            extension_kind: ExtensionKind::Both,
            is_builtin: false,
            location: VsUri::file("/ext/lazy"),
        };
        assert!(!lazy.is_eager());
    }

    #[test]
    fn extension_description_parsed_activation_events() {
        let loc = VsUri::file("/ext/rust");
        let ext = ExtensionDescription::from_package_json(sample_package_json(), loc).unwrap();
        let events = ext.parsed_activation_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], ActivationEvent::OnLanguage("rust".into()));
        assert_eq!(events[1], ActivationEvent::OnCommand("rust.build".into()));
    }

    #[test]
    fn extension_description_activates_on_language() {
        let loc = VsUri::file("/ext/rust");
        let ext = ExtensionDescription::from_package_json(sample_package_json(), loc).unwrap();
        assert!(ext.activates_on_language("rust"));
        assert!(!ext.activates_on_language("python"));
    }

    // -- ExtensionHostState helpers -------------------------------------------

    #[test]
    fn extension_host_state_predicates() {
        assert!(ExtensionHostState::Starting.is_alive());
        assert!(ExtensionHostState::Running.is_alive());
        assert!(!ExtensionHostState::Stopped.is_alive());
        assert!(!ExtensionHostState::Error("x".into()).is_alive());

        assert!(!ExtensionHostState::Running.is_error());
        assert!(ExtensionHostState::Error("boom".into()).is_error());

        assert_eq!(ExtensionHostState::Running.error_message(), None);
        assert_eq!(
            ExtensionHostState::Error("boom".into()).error_message(),
            Some("boom")
        );
    }

    // -- ExtensionHostManager helpers -----------------------------------------

    #[test]
    fn manager_extension_count_and_clear() {
        let mut mgr = ExtensionHostManager::new();
        assert_eq!(mgr.extension_count(), 0);
        assert_eq!(mgr.activated_count(), 0);

        let loc = VsUri::file("/ext/rust");
        let ext = ExtensionDescription::from_package_json(sample_package_json(), loc).unwrap();
        let id = ext.id.clone();
        mgr.register_extension(ext);
        mgr.mark_activated(&id);

        assert_eq!(mgr.extension_count(), 1);
        assert_eq!(mgr.activated_count(), 1);
        assert_eq!(mgr.activated_ids(), &[id]);

        mgr.clear_extensions();
        assert_eq!(mgr.extension_count(), 0);
        assert_eq!(mgr.activated_count(), 0);
    }

    #[test]
    fn manager_extensions_for_command() {
        let mut mgr = ExtensionHostManager::new();
        let loc = VsUri::file("/ext/rust");
        let ext = ExtensionDescription::from_package_json(sample_package_json(), loc).unwrap();
        mgr.register_extension(ext);

        let found = mgr.extensions_for_command("rust.build");
        assert_eq!(found.len(), 1);
        assert!(mgr.extensions_for_command("nonexistent").is_empty());
    }

    // -- ActivationEvent helpers ----------------------------------------------

    #[test]
    fn activation_event_accessors() {
        assert!(ActivationEvent::Star.is_star());
        assert!(!ActivationEvent::OnStartupFinished.is_star());

        assert_eq!(
            ActivationEvent::OnLanguage("rust".into()).language(),
            Some("rust")
        );
        assert_eq!(ActivationEvent::Star.language(), None);

        assert_eq!(
            ActivationEvent::OnCommand("x.y".into()).command(),
            Some("x.y")
        );
        assert_eq!(ActivationEvent::Star.command(), None);
    }

    #[test]
    fn activation_event_roundtrip_to_raw() {
        let cases = vec![
            ("*", ActivationEvent::Star),
            ("onStartupFinished", ActivationEvent::OnStartupFinished),
            ("onLanguage:rust", ActivationEvent::OnLanguage("rust".into())),
            ("onCommand:x.y", ActivationEvent::OnCommand("x.y".into())),
            (
                "workspaceContains:Cargo.toml",
                ActivationEvent::WorkspaceContains("Cargo.toml".into()),
            ),
            ("onFoo", ActivationEvent::Unknown("onFoo".into())),
        ];
        for (raw, event) in &cases {
            assert_eq!(ActivationEvent::parse(raw), *event);
            assert_eq!(event.to_raw(), *raw);
        }
    }

    // -- ContributionPointRegistry helpers ------------------------------------

    #[test]
    fn registry_find_command() {
        let mut reg = ContributionPointRegistry::new();
        let json: serde_json::Value = serde_json::from_str(r#"{
            "commands": [
                {"command": "ext.hello", "title": "Hello"}
            ]
        }"#).unwrap();
        reg.register_contributions("test-ext", &json);
        assert!(reg.find_command("ext.hello").is_some());
        assert!(reg.find_command("missing").is_none());
    }

    #[test]
    fn registry_find_language() {
        let mut reg = ContributionPointRegistry::new();
        let json: serde_json::Value = serde_json::from_str(r#"{
            "languages": [{"id": "rust", "extensions": [".rs"]}]
        }"#).unwrap();
        reg.register_contributions("r", &json);
        assert!(reg.find_language("rust").is_some());
        assert!(reg.find_language("python").is_none());
    }

    #[test]
    fn registry_language_for_file_extension() {
        let mut reg = ContributionPointRegistry::new();
        let json: serde_json::Value = serde_json::from_str(r#"{
            "languages": [
                {"id": "rust", "extensions": [".rs"]},
                {"id": "python", "extensions": [".py", ".pyw"]}
            ]
        }"#).unwrap();
        reg.register_contributions("multi", &json);
        let lang = reg.language_for_file_extension(".py");
        assert_eq!(lang.unwrap().id, "python");
        assert!(reg.language_for_file_extension(".java").is_none());
    }

    #[test]
    fn registry_commands_by_extension() {
        let mut reg = ContributionPointRegistry::new();
        let json1: serde_json::Value = serde_json::from_str(r#"{
            "commands": [
                {"command": "a.one", "title": "One"},
                {"command": "a.two", "title": "Two"}
            ]
        }"#).unwrap();
        let json2: serde_json::Value = serde_json::from_str(r#"{
            "commands": [{"command": "b.one", "title": "B1"}]
        }"#).unwrap();
        reg.register_contributions("ext-a", &json1);
        reg.register_contributions("ext-b", &json2);
        assert_eq!(reg.commands_by_extension("ext-a").len(), 2);
        assert_eq!(reg.commands_by_extension("ext-b").len(), 1);
        assert!(reg.commands_by_extension("ext-c").is_empty());
    }

    #[test]
    fn registry_total_count() {
        let mut reg = ContributionPointRegistry::new();
        let json: serde_json::Value = serde_json::from_str(r#"{
            "commands": [{"command": "c", "title": "C"}],
            "languages": [{"id": "l"}],
            "themes": [{"label": "T", "uiTheme": "vs-dark", "path": "t.json"}]
        }"#).unwrap();
        reg.register_contributions("ext", &json);
        assert_eq!(reg.total_count(), 3);
    }

    #[test]
    fn extensionHostRestartHandler_new() {
        let s = ExtensionHostRestartHandler::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn extensionHostRestartHandler_add_contains() {
        let mut s = ExtensionHostRestartHandler::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn extensionHostRestartHandler_add_duplicate() {
        let mut s = ExtensionHostRestartHandler::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn extensionHostRestartHandler_remove() {
        let mut s = ExtensionHostRestartHandler::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn extensionHostRestartHandler_capacity() {
        let s = ExtensionHostRestartHandler::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn extensionHostRestartHandler_search() {
        let mut s = ExtensionHostRestartHandler::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn extensionHostRestartHandler_stats() {
        let mut s = ExtensionHostRestartHandler::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn extensionHostMemoryMonitor_new() {
        let m = ExtensionHostMemoryMonitor::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn extensionHostMemoryMonitor_add_find() {
        let mut m = ExtensionHostMemoryMonitor::new();
        m.add(ExtensionHostMemoryMonitorItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn extensionHostMemoryMonitor_priority_filter() {
        let mut m = ExtensionHostMemoryMonitor::new();
        m.add(ExtensionHostMemoryMonitorItem::new("a", "A").with_priority(ExtensionHostMemoryMonitorPriority::High));
        m.add(ExtensionHostMemoryMonitorItem::new("b", "B").with_priority(ExtensionHostMemoryMonitorPriority::Low));
        m.add(ExtensionHostMemoryMonitorItem::new("c", "C").with_priority(ExtensionHostMemoryMonitorPriority::High));
        assert_eq!(m.by_priority(ExtensionHostMemoryMonitorPriority::High).len(), 2);
    }

    #[test]
    fn extensionHostMemoryMonitor_remove() {
        let mut m = ExtensionHostMemoryMonitor::new();
        m.add(ExtensionHostMemoryMonitorItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn extensionHostMemoryMonitor_search() {
        let mut m = ExtensionHostMemoryMonitor::new();
        m.add(ExtensionHostMemoryMonitorItem::new("id1", "Hello World"));
        m.add(ExtensionHostMemoryMonitorItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn extensionHostMemoryMonitor_total_weight() {
        let mut m = ExtensionHostMemoryMonitor::new();
        m.add(ExtensionHostMemoryMonitorItem::new("a", "A").with_priority(ExtensionHostMemoryMonitorPriority::Critical));
        m.add(ExtensionHostMemoryMonitorItem::new("b", "B").with_priority(ExtensionHostMemoryMonitorPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn extensionHostMemoryMonitor_capacity_limit() {
        let mut m = ExtensionHostMemoryMonitor::new().with_max_items(2);
        m.add(ExtensionHostMemoryMonitorItem::new("1", "one"));
        m.add(ExtensionHostMemoryMonitorItem::new("2", "two"));
        assert!(!m.add(ExtensionHostMemoryMonitorItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn extensionHostMemoryMonitor_sorted_by_priority() {
        let mut m = ExtensionHostMemoryMonitor::new();
        m.add(ExtensionHostMemoryMonitorItem::new("lo", "Low").with_priority(ExtensionHostMemoryMonitorPriority::Low));
        m.add(ExtensionHostMemoryMonitorItem::new("hi", "High").with_priority(ExtensionHostMemoryMonitorPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn extensionHostMemoryMonitor_item_metadata() {
        let mut item = ExtensionHostMemoryMonitorItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn extensionHostRestartHandler_enabled_toggle() {
        let mut s = ExtensionHostRestartHandler::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn extensionHostMemoryMonitor_priority_display() {
        assert_eq!(format!("{}", ExtensionHostMemoryMonitorPriority::High), "high");
        assert_eq!(format!("{}", ExtensionHostMemoryMonitorPriority::Low), "low");
    }


    #[test]
    fn ext_host_config_new() {
        let cfg = ExtHostConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn ext_host_config_set_get() {
        let mut cfg = ExtHostConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn ext_host_config_remove() {
        let mut cfg = ExtHostConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn ext_host_config_keys_sorted() {
        let mut cfg = ExtHostConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn ext_host_config_bump_version() {
        let mut cfg = ExtHostConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn ext_host_config_clear() {
        let mut cfg = ExtHostConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn ext_host_config_merge() {
        let mut cfg1 = ExtHostConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ExtHostConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn ext_host_config_disable() {
        let mut cfg = ExtHostConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn ext_host_rate_tracker_empty() {
        let rt = ExtHostRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn ext_host_rate_tracker_record() {
        let mut rt = ExtHostRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn ext_host_rate_tracker_prune() {
        let mut rt = ExtHostRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn ext_host_validator_valid() {
        let v = ExtHostValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn ext_host_validator_errors() {
        let mut v = ExtHostValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn ext_host_validator_clear() {
        let mut v = ExtHostValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn ext_host_validator_merge() {
        let mut v1 = ExtHostValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = ExtHostValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn ext_host_rate_tracker_clear() {
        let mut rt = ExtHostRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zu_metrics_empty() {
        let m = ZuMetrics::new("ext_host");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zu_metrics_record_and_mean() {
        let mut m = ZuMetrics::new("ext_host");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zu_metrics_min_max() {
        let mut m = ZuMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zu_metrics_variance_and_std() {
        let mut m = ZuMetrics::new("v");
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
    fn zu_metrics_percentile() {
        let mut m = ZuMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn zu_metrics_merge() {
        let mut a = ZuMetrics::new("a");
        a.record(1.0);
        let mut b = ZuMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn zu_metrics_reset() {
        let mut m = ZuMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn zu_rate_window_empty() {
        let rw = ZuRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn zu_rate_window_tick_and_rate() {
        let mut rw = ZuRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn zu_lru_cache_basic() {
        let mut c = ZuLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn zu_lru_cache_contains_and_keys() {
        let mut c = ZuLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn zu_lru_cache_remove() {
        let mut c = ZuLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn zu_metrics_sum() {
        let mut m = ZuMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zu_metrics_label() {
        let m = ZuMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn zu_lru_cache_clear() {
        let mut c = ZuLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for ext_host
    #[test]
    fn xa_ext_host_ring_new() {
        let rb = super::XaExtHostRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_host_ring_push_len() {
        let mut rb = super::XaExtHostRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_host_ring_wrap() {
        let mut rb = super::XaExtHostRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_host_ring_mean_empty() {
        let rb = super::XaExtHostRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_host_ring_mean_values() {
        let mut rb = super::XaExtHostRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_host_ring_min_max() {
        let mut rb = super::XaExtHostRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_host_ring_iter() {
        let mut rb = super::XaExtHostRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_host_counter_new() {
        let c = super::XaExtHostCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_host_counter_inc() {
        let mut c = super::XaExtHostCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_host_counter_inc_by() {
        let mut c = super::XaExtHostCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_host_counter_reset() {
        let mut c = super::XaExtHostCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_host_counter_clear() {
        let mut c = super::XaExtHostCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_host_counter_default() {
        let c = super::XaExtHostCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 60 ----

    #[test]
    fn xc_60_pool_new_empty() {
        let pool: super::Xc60Pool<i32> = super::Xc60Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_60_pool_release_acquire() {
        let mut pool = super::Xc60Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_60_pool_acquire_empty() {
        let mut pool: super::Xc60Pool<i32> = super::Xc60Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_60_pool_full() {
        let mut pool = super::Xc60Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_60_pool_drain() {
        let mut pool = super::Xc60Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_60_pool_stats() {
        let mut pool = super::Xc60Pool::new(8);
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
    fn xc_60_pool_clear() {
        let mut pool = super::Xc60Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_60_pool_shrink() {
        let mut pool = super::Xc60Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_60_pool_default() {
        let pool: super::Xc60Pool<String> = super::Xc60Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_60_pool_extend() {
        let mut pool = super::Xc60Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_60_pool_retain() {
        let mut pool = super::Xc60Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_60_scheduler_round_robin() {
        let mut sched = super::Xc60Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_60_scheduler_empty() {
        let mut sched = super::Xc60Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_60_scheduler_reset() {
        let mut sched = super::Xc60Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_60_scheduler_add_remove() {
        let mut sched = super::Xc60Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_60_scheduler_targets() {
        let sched = super::Xc60Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_60_hash_empty() {
        assert_eq!(super::xc_60_hash(b""), 5381);
    }

    #[test]
    fn xc_60_hash_data() {
        let h = super::xc_60_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_60_hash(b"hello"), h);
    }

    #[test]
    fn xc_60_reverse_str() {
        assert_eq!(super::xc_60_reverse("abc"), "cba");
        assert_eq!(super::xc_60_reverse(""), "");
    }


    // --- xd_33 deepening tests ---

    #[test]
    fn xd_33_sm_initial_state() {
        let sm = Xd33StateMachine::new();
        assert_eq!(sm.current_state(), Xd33State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_33_sm_valid_idle_to_running() {
        let mut sm = Xd33StateMachine::new();
        assert!(sm.transition(Xd33State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd33State::Running);
    }

    #[test]
    fn xd_33_sm_valid_running_to_paused() {
        let mut sm = Xd33StateMachine::new();
        sm.transition(Xd33State::Running).unwrap();
        assert!(sm.transition(Xd33State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd33State::Paused);
    }

    #[test]
    fn xd_33_sm_valid_running_to_done() {
        let mut sm = Xd33StateMachine::new();
        sm.transition(Xd33State::Running).unwrap();
        assert!(sm.transition(Xd33State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd33State::Done);
    }

    #[test]
    fn xd_33_sm_valid_paused_to_running() {
        let mut sm = Xd33StateMachine::new();
        sm.transition(Xd33State::Running).unwrap();
        sm.transition(Xd33State::Paused).unwrap();
        assert!(sm.transition(Xd33State::Running).is_ok());
    }

    #[test]
    fn xd_33_sm_valid_done_to_idle() {
        let mut sm = Xd33StateMachine::new();
        sm.transition(Xd33State::Running).unwrap();
        sm.transition(Xd33State::Done).unwrap();
        assert!(sm.transition(Xd33State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd33State::Idle);
    }

    #[test]
    fn xd_33_sm_invalid_idle_to_done() {
        let mut sm = Xd33StateMachine::new();
        assert!(sm.transition(Xd33State::Done).is_err());
    }

    #[test]
    fn xd_33_sm_invalid_idle_to_paused() {
        let mut sm = Xd33StateMachine::new();
        assert!(sm.transition(Xd33State::Paused).is_err());
    }

    #[test]
    fn xd_33_sm_history_tracking() {
        let mut sm = Xd33StateMachine::new();
        sm.transition(Xd33State::Running).unwrap();
        sm.transition(Xd33State::Paused).unwrap();
        sm.transition(Xd33State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd33State::Idle);
        assert_eq!(sm.history()[0].to, Xd33State::Running);
        assert_eq!(sm.history()[1].from, Xd33State::Running);
        assert_eq!(sm.history()[2].to, Xd33State::Done);
    }

    #[test]
    fn xd_33_sm_serialize_deserialize() {
        let mut sm = Xd33StateMachine::new();
        sm.transition(Xd33State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd33StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd33State::Running));
    }

    #[test]
    fn xd_33_sm_deserialize_invalid() {
        assert_eq!(Xd33StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_33_sm_reset() {
        let mut sm = Xd33StateMachine::new();
        sm.transition(Xd33State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd33State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_33_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd33EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd33Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_33_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd33EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd33Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd33Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_33_bus_unsubscribe() {
        let mut bus = Xd33EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_33_event_kind_and_payload() {
        let e = Xd33Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd33Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_33_bus_clear_history() {
        let mut bus = Xd33EventBus::new();
        bus.publish(Xd33Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_33_sm_step_counter_increments() {
        let mut sm = Xd33StateMachine::new();
        sm.transition(Xd33State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd33State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #31 --

    #[test]
    fn xf31_trie_insert_search() {
        let mut t = Xf31Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf31_trie_starts_with() {
        let mut t = Xf31Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf31_trie_remove() {
        let mut t = Xf31Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf31_trie_word_count() {
        let mut t = Xf31Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf31_trie_longest_prefix() {
        let mut t = Xf31Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf31_trie_all_words() {
        let mut t = Xf31Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf31_trie_autocomplete() {
        let mut t = Xf31Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf31_trie_empty_search() {
        let t = Xf31Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf31_bloom_add_contains() {
        let mut bf = Xf31BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf31_bloom_probably_absent() {
        let bf = Xf31BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf31_bloom_false_positive_rate() {
        let mut bf = Xf31BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf31_bloom_clear() {
        let mut bf = Xf31BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf31_bloom_union() {
        let mut a = Xf31BloomFilter::xf_new(512, 2);
        let mut b = Xf31BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf31_bloom_intersection_estimate() {
        let mut a = Xf31BloomFilter::xf_new(512, 2);
        let mut b = Xf31BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf31_bloom_union_size_mismatch() {
        let a = Xf31BloomFilter::xf_new(256, 2);
        let b = Xf31BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh59_skip_insert_contains() {
        let mut sl = super::Xh59SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh59_skip_remove() {
        let mut sl = super::Xh59SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh59_skip_len() {
        let mut sl = super::Xh59SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh59_skip_range_query() {
        let mut sl = super::Xh59SkipList::xh_new(4);
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
    fn xh59_skip_floor_ceiling() {
        let mut sl = super::Xh59SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh59_skip_rank() {
        let mut sl = super::Xh59SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh59_skip_empty() {
        let sl = super::Xh59SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh59_skip_duplicates() {
        let mut sl = super::Xh59SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh59_bitset_set_test() {
        let mut bs = super::Xh59BitSet::xh_new(256);
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
    fn xh59_bitset_clear_count() {
        let mut bs = super::Xh59BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh59_bitset_and_or_xor() {
        let mut a = super::Xh59BitSet::xh_new(128);
        let mut b = super::Xh59BitSet::xh_new(128);
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
    fn xh59_bitset_iter_ones() {
        let mut bs = super::Xh59BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh59_bitset_first_last() {
        let mut bs = super::Xh59BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh59_bitset_empty() {
        let bs = super::Xh59BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi59_deque_push_pop_back() {
        let mut dq = super::Xi59Deque::xi_new(4);
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
    fn xi59_deque_push_pop_front() {
        let mut dq = super::Xi59Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi59_deque_mixed_ops() {
        let mut dq = super::Xi59Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi59_deque_get_and_split() {
        let mut dq = super::Xi59Deque::xi_new(8);
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
    fn xi59_deque_rotate_left() {
        let mut dq = super::Xi59Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi59_deque_rotate_right() {
        let mut dq = super::Xi59Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi59_deque_grow() {
        let mut dq = super::Xi59Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi59_deque_empty() {
        let dq = super::Xi59Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi59_interval_tree_insert_query() {
        let mut tree = super::Xi59IntervalTree::xi_new();
        tree.xi_insert(super::Xi59Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi59Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi59Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi59_interval_tree_overlap() {
        let mut tree = super::Xi59IntervalTree::xi_new();
        tree.xi_insert(super::Xi59Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi59Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi59Interval::xi_new(12, 20));
        let q = super::Xi59Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi59_interval_tree_remove() {
        let mut tree = super::Xi59IntervalTree::xi_new();
        tree.xi_insert(super::Xi59Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi59Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi59_interval_tree_gaps() {
        let mut tree = super::Xi59IntervalTree::xi_new();
        tree.xi_insert(super::Xi59Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi59Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi59Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi59Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi59Interval::xi_new(8, 10));
    }

    #[test]
    fn xi59_interval_tree_merge() {
        let mut tree = super::Xi59IntervalTree::xi_new();
        tree.xi_insert(super::Xi59Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi59Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi59Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi59Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi59Interval::xi_new(10, 15));
    }

    #[test]
    fn xi59_interval_tree_all() {
        let mut tree = super::Xi59IntervalTree::xi_new();
        tree.xi_insert(super::Xi59Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi59Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi59_interval_tree_empty() {
        let tree = super::Xi59IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi59_interval_tree_contains_point() {
        let iv = super::Xi59Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
