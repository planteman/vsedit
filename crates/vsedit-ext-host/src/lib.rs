//! Extension host process management
//!
//! Manages extension descriptions parsed from `package.json` and tracks
//! extension host lifecycle state and activation. Provides child-process
//! spawning with a `Content-Length`-framed JSON-RPC transport for
//! communicating with VS Code extension host processes.

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
    fn ext_host_validator_accepts_valid_name() {
        let v = ExtHostValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_host_validator_rejects_empty() {
        let v = ExtHostValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_host_validator_rejects_too_long() {
        let v = ExtHostValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_host_validator_forbidden_prefix() {
        let v = ExtHostValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_host_validator_allowed_chars() {
        let v = ExtHostValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_host_validator_range() {
        let v = ExtHostValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_host_sanitize_removes_control() {
        let result = ExtHostValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_host_truncate_short_string() {
        assert_eq!(ExtHostValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_host_truncate_long_string() {
        let result = ExtHostValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_host_is_ascii_printable() {
        assert!(ExtHostValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtHostValidator::is_ascii_printable("Hello\x00World"));
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
}
