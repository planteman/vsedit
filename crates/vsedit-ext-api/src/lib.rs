//! Extension API surface (vscode.* namespace bridging).
//!
//! This crate defines the complete `vscode.*` API surface exposed to extensions.
//! It maps each namespace to its corresponding bridge crate and provides the
//! central API registry for the extension host.

use std::fmt;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Namespace identifiers
// ---------------------------------------------------------------------------

/// VS Code API namespace identifiers.
pub mod namespaces {
    pub const COMMANDS: &str = "commands";
    pub const WINDOW: &str = "window";
    pub const WORKSPACE: &str = "workspace";
    pub const LANGUAGES: &str = "languages";
    pub const DEBUG: &str = "debug";
    pub const EXTENSIONS: &str = "extensions";
    pub const ENV: &str = "env";
    pub const TASKS: &str = "tasks";
    pub const SCM: &str = "scm";
    pub const COMMENTS: &str = "comments";
    pub const AUTHENTICATION: &str = "authentication";
    pub const NOTEBOOKS: &str = "notebooks";
    pub const TESTS: &str = "tests";
    pub const CHAT: &str = "chat";
    pub const LM: &str = "lm";
}

/// All supported namespaces.
pub fn all_namespaces() -> Vec<&'static str> {
    vec![
        namespaces::COMMANDS,
        namespaces::WINDOW,
        namespaces::WORKSPACE,
        namespaces::LANGUAGES,
        namespaces::DEBUG,
        namespaces::EXTENSIONS,
        namespaces::ENV,
        namespaces::TASKS,
        namespaces::SCM,
        namespaces::COMMENTS,
        namespaces::AUTHENTICATION,
        namespaces::NOTEBOOKS,
        namespaces::TESTS,
        namespaces::CHAT,
        namespaces::LM,
    ]
}

// ---------------------------------------------------------------------------
// API Version
// ---------------------------------------------------------------------------

/// The VS Code API version we are compatible with.
pub const API_VERSION: &str = "1.110.0";

/// Minimum engine version extensions should declare.
pub const MIN_ENGINE_VERSION: &str = "1.70.0";

// ---------------------------------------------------------------------------
// Activation events
// ---------------------------------------------------------------------------

/// Extension activation event kinds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActivationEvent {
    /// Activate on startup.
    Star,
    /// Activate when a language is opened.
    OnLanguage(String),
    /// Activate when a command is executed.
    OnCommand(String),
    /// Activate when a debug type is used.
    OnDebug(String),
    /// Activate when a file system scheme is accessed.
    OnFileSystem(String),
    /// Activate when a view is opened.
    OnView(String),
    /// Activate when a custom URI is handled.
    OnUri,
    /// Activate when a workspace contains a file matching a glob.
    WorkspaceContains(String),
    /// Activate when a webview panel is resolved.
    OnWebviewPanel(String),
    /// Activate when authentication is requested.
    OnAuthenticationRequest(String),
    /// Activate when a notebook type is opened.
    OnNotebook(String),
    /// Activate when a terminal profile is requested.
    OnTerminalProfile(String),
    /// Activate when the startup is finished.
    OnStartupFinished,
}

impl ActivationEvent {
    /// Parse an activation event string like `"onLanguage:rust"`.
    pub fn parse(s: &str) -> Option<Self> {
        if s == "*" {
            return Some(Self::Star);
        }
        if s == "onUri" {
            return Some(Self::OnUri);
        }
        if s == "onStartupFinished" {
            return Some(Self::OnStartupFinished);
        }
        let (prefix, value) = s.split_once(':')?;
        let value = value.to_string();
        match prefix {
            "onLanguage" => Some(Self::OnLanguage(value)),
            "onCommand" => Some(Self::OnCommand(value)),
            "onDebug" | "onDebugResolve" | "onDebugInitialConfigurations" => {
                Some(Self::OnDebug(value))
            }
            "onFileSystem" => Some(Self::OnFileSystem(value)),
            "onView" => Some(Self::OnView(value)),
            "workspaceContains" => Some(Self::WorkspaceContains(value)),
            "onWebviewPanel" => Some(Self::OnWebviewPanel(value)),
            "onAuthenticationRequest" => Some(Self::OnAuthenticationRequest(value)),
            "onNotebook" => Some(Self::OnNotebook(value)),
            "onTerminalProfile" => Some(Self::OnTerminalProfile(value)),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Extension contribution points
// ---------------------------------------------------------------------------

/// Known VS Code extension contribution point identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ContributionPoint {
    Commands,
    Menus,
    Keybindings,
    Languages,
    Grammars,
    Themes,
    IconThemes,
    ProductIconThemes,
    Snippets,
    Views,
    ViewsContainers,
    Configuration,
    ConfigurationDefaults,
    TaskDefinitions,
    DebugAdapters,
    Breakpoints,
    Terminal,
    Colors,
    CustomEditors,
    Walkthroughs,
    Notebooks,
    NotebookRenderers,
    Authentication,
    Localizations,
    ChatParticipants,
    LanguageModels,
    Other(String),
}

impl ContributionPoint {
    /// Parse a contribution point from its JSON key.
    pub fn from_key(key: &str) -> Self {
        match key {
            "commands" => Self::Commands,
            "menus" => Self::Menus,
            "keybindings" => Self::Keybindings,
            "languages" => Self::Languages,
            "grammars" => Self::Grammars,
            "themes" => Self::Themes,
            "iconThemes" => Self::IconThemes,
            "productIconThemes" => Self::ProductIconThemes,
            "snippets" => Self::Snippets,
            "views" => Self::Views,
            "viewsContainers" => Self::ViewsContainers,
            "configuration" => Self::Configuration,
            "configurationDefaults" => Self::ConfigurationDefaults,
            "taskDefinitions" => Self::TaskDefinitions,
            "debuggers" => Self::DebugAdapters,
            "breakpoints" => Self::Breakpoints,
            "terminal" => Self::Terminal,
            "colors" => Self::Colors,
            "customEditors" => Self::CustomEditors,
            "walkthroughs" => Self::Walkthroughs,
            "notebooks" => Self::Notebooks,
            "notebookRenderer" => Self::NotebookRenderers,
            "authentication" => Self::Authentication,
            "localizations" => Self::Localizations,
            "chatParticipants" => Self::ChatParticipants,
            "languageModels" | "languageModelTools" => Self::LanguageModels,
            other => Self::Other(other.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// API capability flags
// ---------------------------------------------------------------------------

/// Flags indicating which API capabilities are supported in the current context.
#[derive(Debug, Clone)]
pub struct ApiCapabilities {
    pub supports_proposed_api: bool,
    pub supports_webview: bool,
    pub supports_terminal: bool,
    pub supports_debug: bool,
    pub supports_notebook: bool,
    pub supports_chat: bool,
    pub supports_language_models: bool,
    pub supports_testing: bool,
    pub supports_authentication: bool,
    pub supports_custom_editors: bool,
}

impl Default for ApiCapabilities {
    fn default() -> Self {
        Self {
            supports_proposed_api: false,
            supports_webview: false,
            supports_terminal: true,
            supports_debug: true,
            supports_notebook: false,
            supports_chat: false,
            supports_language_models: false,
            supports_testing: true,
            supports_authentication: true,
            supports_custom_editors: false,
        }
    }
}

// ---------------------------------------------------------------------------
// API registry
// ---------------------------------------------------------------------------

/// Tracks which API namespaces and proxy identifiers are registered.
pub struct ApiRegistry {
    /// Namespace → proxy identifier mapping.
    namespace_proxies: HashMap<String, u32>,
    /// All registered contribution points from loaded extensions.
    contribution_points: Vec<ContributionPoint>,
    /// Capabilities of the current host.
    capabilities: ApiCapabilities,
}

impl ApiRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            namespace_proxies: HashMap::new(),
            contribution_points: Vec::new(),
            capabilities: ApiCapabilities::default(),
        }
    }

    /// Create a registry with default namespace proxies registered.
    pub fn with_defaults() -> Self {
        let mut reg = Self::new();
        for (i, ns) in all_namespaces().iter().enumerate() {
            reg.register_namespace(ns, (i + 1) as u32);
        }
        reg
    }

    /// Register a namespace with its proxy identifier.
    pub fn register_namespace(&mut self, namespace: &str, proxy_id: u32) {
        self.namespace_proxies.insert(namespace.to_string(), proxy_id);
    }

    /// Look up the proxy identifier for a namespace.
    pub fn get_proxy_id(&self, namespace: &str) -> Option<u32> {
        self.namespace_proxies.get(namespace).copied()
    }

    /// Check if a namespace is registered.
    pub fn has_namespace(&self, namespace: &str) -> bool {
        self.namespace_proxies.contains_key(namespace)
    }

    /// Get all registered namespaces.
    pub fn registered_namespaces(&self) -> Vec<&str> {
        self.namespace_proxies.keys().map(|s| s.as_str()).collect()
    }

    /// Register a contribution point.
    pub fn register_contribution(&mut self, point: ContributionPoint) {
        if !self.contribution_points.contains(&point) {
            self.contribution_points.push(point);
        }
    }

    /// Get all registered contribution points.
    pub fn contributions(&self) -> &[ContributionPoint] {
        &self.contribution_points
    }

    /// Get the current capabilities.
    pub fn capabilities(&self) -> &ApiCapabilities {
        &self.capabilities
    }

    /// Update capabilities.
    pub fn set_capabilities(&mut self, caps: ApiCapabilities) {
        self.capabilities = caps;
    }

    /// Total number of registered namespaces.
    pub fn namespace_count(&self) -> usize {
        self.namespace_proxies.len()
    }

    /// Returns true if contribution_points is empty.
    pub fn is_contribution_points_empty(&self) -> bool {
        self.contribution_points.is_empty()
    }

    /// Get the first contribution_point, if any.
    pub fn first_contribution_point(&self) -> Option<&ContributionPoint> {
        self.contribution_points.first()
    }

    /// Get the last contribution_point, if any.
    pub fn last_contribution_point(&self) -> Option<&ContributionPoint> {
        self.contribution_points.last()
    }

    /// Retain only contribution_points matching the predicate.
    pub fn retain_contribution_points(&mut self, f: impl Fn(&ContributionPoint) -> bool) {
        self.contribution_points.retain(|item| f(item));
    }
}

impl Default for ApiRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for ext-api operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtApiStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtApiStats {
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
    pub fn merge(&mut self, other: &ExtApiStats) {
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

impl Default for ExtApiStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtApiStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtApiStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-api.
#[derive(Debug, Clone)]
pub struct ExtApiValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtApiValidator {
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

impl Default for ExtApiValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Version compatibility checking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ApiVersionCheck {
    pub current_version: String,
}

impl ApiVersionCheck {
    pub fn new(current: impl Into<String>) -> Self {
        Self { current_version: current.into() }
    }

    /// Parse a semver string "major.minor.patch" into (major, minor, patch).
    fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 { return None; }
        Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
    }

    /// Check if `required` version is satisfied by the current version.
    /// Returns true if current >= required.
    pub fn is_compatible(&self, required: &str) -> bool {
        let Some(current) = Self::parse_semver(&self.current_version) else { return false };
        let required = required.trim_start_matches('^').trim_start_matches('~').trim_start_matches(">=");
        let Some(req) = Self::parse_semver(required) else { return false };
        current >= req
    }

    /// Check if two versions share the same major version.
    pub fn same_major(&self, other: &str) -> bool {
        let Some(current) = Self::parse_semver(&self.current_version) else { return false };
        let Some(other) = Self::parse_semver(other) else { return false };
        current.0 == other.0
    }
}

// ---------------------------------------------------------------------------
// Deprecation tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ApiDeprecationWarning {
    pub api_name: String,
    pub deprecated_since: String,
    pub replacement: Option<String>,
    pub message: String,
}

impl ApiDeprecationWarning {
    pub fn new(api_name: impl Into<String>, since: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            api_name: api_name.into(),
            deprecated_since: since.into(),
            replacement: None,
            message: message.into(),
        }
    }

    pub fn with_replacement(mut self, replacement: impl Into<String>) -> Self {
        self.replacement = Some(replacement.into());
        self
    }

    /// Format a human-readable deprecation message.
    pub fn format_warning(&self) -> String {
        match &self.replacement {
            Some(r) => format!("'{}' is deprecated since {}. Use '{}' instead. {}", self.api_name, self.deprecated_since, r, self.message),
            None => format!("'{}' is deprecated since {}. {}", self.api_name, self.deprecated_since, self.message),
        }
    }
}

impl fmt::Display for ApiDeprecationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_warning())
    }
}

/// Registry of deprecation warnings.
#[derive(Debug, Clone, Default)]
pub struct DeprecationRegistry {
    warnings: Vec<ApiDeprecationWarning>,
}

impl DeprecationRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, warning: ApiDeprecationWarning) {
        if !self.warnings.iter().any(|w| w.api_name == warning.api_name) {
            self.warnings.push(warning);
        }
    }

    pub fn is_deprecated(&self, api_name: &str) -> bool {
        self.warnings.iter().any(|w| w.api_name == api_name)
    }

    pub fn get_warning(&self, api_name: &str) -> Option<&ApiDeprecationWarning> {
        self.warnings.iter().find(|w| w.api_name == api_name)
    }

    pub fn all_warnings(&self) -> &[ApiDeprecationWarning] {
        &self.warnings
    }

    pub fn count(&self) -> usize {
        self.warnings.len()
    }

    /// Get warnings introduced since a specific version.
    pub fn warnings_since(&self, version: &str) -> Vec<&ApiDeprecationWarning> {
        let check = ApiVersionCheck::new(version);
        self.warnings.iter().filter(|w| {
            // Warning is "since" a version; include it if `version >= deprecated_since`
            check.is_compatible(&w.deprecated_since)
        }).collect()
    }
}

// ---------------------------------------------------------------------------
// Capability querying helpers
// ---------------------------------------------------------------------------

/// Check whether a specific capability is supported.
pub fn api_capability_check(caps: &ApiCapabilities, feature: &str) -> bool {
    match feature {
        "proposedApi" => caps.supports_proposed_api,
        "webview" => caps.supports_webview,
        "terminal" => caps.supports_terminal,
        "debug" => caps.supports_debug,
        "notebook" => caps.supports_notebook,
        "chat" => caps.supports_chat,
        "languageModels" => caps.supports_language_models,
        "testing" => caps.supports_testing,
        "authentication" => caps.supports_authentication,
        "customEditors" => caps.supports_custom_editors,
        _ => false,
    }
}

/// List all supported capability names for the given capabilities struct.
pub fn api_supported_features(caps: &ApiCapabilities) -> Vec<&'static str> {
    let mut features = Vec::new();
    let checks = [
        ("proposedApi", caps.supports_proposed_api),
        ("webview", caps.supports_webview),
        ("terminal", caps.supports_terminal),
        ("debug", caps.supports_debug),
        ("notebook", caps.supports_notebook),
        ("chat", caps.supports_chat),
        ("languageModels", caps.supports_language_models),
        ("testing", caps.supports_testing),
        ("authentication", caps.supports_authentication),
        ("customEditors", caps.supports_custom_editors),
    ];
    for (name, supported) in checks {
        if supported { features.push(name); }
    }
    features
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_count() {
        assert_eq!(all_namespaces().len(), 15);
    }

    #[test]
    fn api_version() {
        assert!(API_VERSION.starts_with("1."));
    }

    #[test]
    fn parse_activation_events() {
        assert_eq!(ActivationEvent::parse("*"), Some(ActivationEvent::Star));
        assert_eq!(
            ActivationEvent::parse("onLanguage:rust"),
            Some(ActivationEvent::OnLanguage("rust".into()))
        );
        assert_eq!(
            ActivationEvent::parse("onCommand:editor.action.formatDocument"),
            Some(ActivationEvent::OnCommand(
                "editor.action.formatDocument".into()
            ))
        );
        assert_eq!(ActivationEvent::parse("onUri"), Some(ActivationEvent::OnUri));
        assert_eq!(
            ActivationEvent::parse("onStartupFinished"),
            Some(ActivationEvent::OnStartupFinished)
        );
        assert_eq!(
            ActivationEvent::parse("workspaceContains:**/*.rs"),
            Some(ActivationEvent::WorkspaceContains("**/*.rs".into()))
        );
        assert!(ActivationEvent::parse("invalidEvent").is_none());
    }

    #[test]
    fn contribution_point_parsing() {
        assert_eq!(ContributionPoint::from_key("commands"), ContributionPoint::Commands);
        assert_eq!(ContributionPoint::from_key("grammars"), ContributionPoint::Grammars);
        assert_eq!(
            ContributionPoint::from_key("unknown"),
            ContributionPoint::Other("unknown".into())
        );
    }

    #[test]
    fn api_registry_defaults() {
        let reg = ApiRegistry::with_defaults();
        assert_eq!(reg.namespace_count(), 15);
        assert!(reg.has_namespace("commands"));
        assert!(reg.has_namespace("window"));
        assert!(reg.has_namespace("lm"));
        assert!(!reg.has_namespace("nonexistent"));
    }

    #[test]
    fn api_registry_proxy_ids() {
        let reg = ApiRegistry::with_defaults();
        assert_eq!(reg.get_proxy_id("commands"), Some(1));
        assert_eq!(reg.get_proxy_id("window"), Some(2));
        assert_eq!(reg.get_proxy_id("nonexistent"), None);
    }

    #[test]
    fn api_registry_contributions() {
        let mut reg = ApiRegistry::new();
        assert!(reg.contributions().is_empty());
        reg.register_contribution(ContributionPoint::Commands);
        reg.register_contribution(ContributionPoint::Themes);
        reg.register_contribution(ContributionPoint::Commands); // duplicate
        assert_eq!(reg.contributions().len(), 2);
    }

    #[test]
    fn capabilities_defaults() {
        let caps = ApiCapabilities::default();
        assert!(!caps.supports_proposed_api);
        assert!(caps.supports_terminal);
        assert!(caps.supports_debug);
        assert!(caps.supports_testing);
    }

    #[test]
    fn eq_activationevent_same() {
        assert_eq!(ActivationEvent::Star, ActivationEvent::Star);
    }

    #[test]
    fn ne_activationevent_diff() {
        assert_ne!(ActivationEvent::Star, ActivationEvent::OnUri);
    }

    #[test]
    fn eq_contributionpoint_same() {
        assert_eq!(ContributionPoint::Commands, ContributionPoint::Commands);
    }

    #[test]
    fn ne_contributionpoint_diff() {
        assert_ne!(ContributionPoint::Commands, ContributionPoint::Menus);
    }

    #[test]
    fn version_check_compatible() {
        let check = ApiVersionCheck::new("1.110.0");
        assert!(check.is_compatible("1.70.0"));
        assert!(check.is_compatible("1.110.0"));
        assert!(!check.is_compatible("1.111.0"));
        assert!(!check.is_compatible("2.0.0"));
    }

    #[test]
    fn version_check_caret_prefix() {
        let check = ApiVersionCheck::new("1.110.0");
        assert!(check.is_compatible("^1.70.0"));
        assert!(check.is_compatible(">=1.100.0"));
    }

    #[test]
    fn version_check_same_major() {
        let check = ApiVersionCheck::new("1.110.0");
        assert!(check.same_major("1.0.0"));
        assert!(!check.same_major("2.0.0"));
    }

    #[test]
    fn version_check_invalid_semver() {
        let check = ApiVersionCheck::new("1.110.0");
        assert!(!check.is_compatible("not-a-version"));
        assert!(!check.is_compatible("1.2"));
    }

    #[test]
    fn deprecation_warning_format_with_replacement() {
        let w = ApiDeprecationWarning::new("window.showInputBox", "1.100.0", "Consider alternatives.")
            .with_replacement("window.createInputBox");
        let msg = w.format_warning();
        assert!(msg.contains("window.showInputBox"));
        assert!(msg.contains("1.100.0"));
        assert!(msg.contains("window.createInputBox"));
    }

    #[test]
    fn deprecation_warning_format_without_replacement() {
        let w = ApiDeprecationWarning::new("env.openExternal", "1.90.0", "Will be removed.");
        let msg = w.format_warning();
        assert!(msg.contains("env.openExternal"));
        assert!(!msg.contains("Use '"));
    }

    #[test]
    fn deprecation_registry_tracks_warnings() {
        let mut reg = DeprecationRegistry::new();
        reg.register(ApiDeprecationWarning::new("api.old", "1.50.0", "gone"));
        reg.register(ApiDeprecationWarning::new("api.old", "1.50.0", "duplicate"));
        assert_eq!(reg.count(), 1);
        assert!(reg.is_deprecated("api.old"));
        assert!(!reg.is_deprecated("api.new"));
    }

    #[test]
    fn deprecation_registry_get_warning() {
        let mut reg = DeprecationRegistry::new();
        reg.register(ApiDeprecationWarning::new("api.foo", "1.80.0", "removed"));
        let w = reg.get_warning("api.foo").unwrap();
        assert_eq!(w.deprecated_since, "1.80.0");
        assert!(reg.get_warning("api.bar").is_none());
    }

    #[test]
    fn capability_check_known_features() {
        let caps = ApiCapabilities::default();
        assert!(api_capability_check(&caps, "terminal"));
        assert!(api_capability_check(&caps, "debug"));
        assert!(!api_capability_check(&caps, "webview"));
        assert!(!api_capability_check(&caps, "unknown_feature"));
    }

    #[test]
    fn supported_features_list() {
        let caps = ApiCapabilities::default();
        let features = api_supported_features(&caps);
        assert!(features.contains(&"terminal"));
        assert!(features.contains(&"debug"));
        assert!(features.contains(&"testing"));
        assert!(!features.contains(&"webview"));
    }

    #[test]
    fn deprecation_warning_display() {
        let w = ApiDeprecationWarning::new("old.api", "1.0.0", "removed")
            .with_replacement("new.api");
        let s = format!("{}", w);
        assert!(s.contains("old.api"));
        assert!(s.contains("new.api"));
    }

    #[test]
    fn version_check_parse_semver_valid() {
        let check = ApiVersionCheck::new("0.0.1");
        assert!(check.is_compatible("0.0.0"));
        assert!(check.is_compatible("0.0.1"));
        assert!(!check.is_compatible("0.0.2"));
    }

    #[test]
    fn ext_api_stats_new_defaults() {
        let stats = ExtApiStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_api_stats_record_success() {
        let mut stats = ExtApiStats::new();
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
    fn ext_api_stats_record_failure() {
        let mut stats = ExtApiStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_api_stats_reset() {
        let mut stats = ExtApiStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_api_stats_merge() {
        let mut a = ExtApiStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtApiStats::new();
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
    fn ext_api_stats_display() {
        let mut stats = ExtApiStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_api_stats_default() {
        let stats = ExtApiStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_api_validator_accepts_valid_name() {
        let v = ExtApiValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_api_validator_rejects_empty() {
        let v = ExtApiValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_api_validator_rejects_too_long() {
        let v = ExtApiValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_api_validator_forbidden_prefix() {
        let v = ExtApiValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_api_validator_allowed_chars() {
        let v = ExtApiValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_api_validator_range() {
        let v = ExtApiValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_api_sanitize_removes_control() {
        let result = ExtApiValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_api_truncate_short_string() {
        assert_eq!(ExtApiValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_api_truncate_long_string() {
        let result = ExtApiValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_api_is_ascii_printable() {
        assert!(ExtApiValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtApiValidator::is_ascii_printable("Hello\x00World"));
    }
}
