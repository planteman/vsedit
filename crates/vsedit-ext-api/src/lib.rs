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

// ---------------------------------------------------------------------------
// api_version_compare — semver comparison
// ---------------------------------------------------------------------------

/// Result of comparing two API versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionOrdering {
    /// The first version is older than the second.
    Older,
    /// The versions are the same.
    Same,
    /// The first version is newer than the second.
    Newer,
}

/// Parse a semver string into (major, minor, patch). Returns None if invalid.
pub fn parse_semver(version: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

/// Compare two semver version strings.
/// Returns None if either version string is invalid.
pub fn api_version_compare(a: &str, b: &str) -> Option<VersionOrdering> {
    let (a_maj, a_min, a_pat) = parse_semver(a)?;
    let (b_maj, b_min, b_pat) = parse_semver(b)?;

    let result = (a_maj, a_min, a_pat).cmp(&(b_maj, b_min, b_pat));
    Some(match result {
        std::cmp::Ordering::Less => VersionOrdering::Older,
        std::cmp::Ordering::Equal => VersionOrdering::Same,
        std::cmp::Ordering::Greater => VersionOrdering::Newer,
    })
}

/// Check whether a required API version is satisfied by the current API_VERSION.
pub fn api_version_satisfies(required: &str) -> Option<bool> {
    match api_version_compare(API_VERSION, required)? {
        VersionOrdering::Older => Some(false),
        _ => Some(true),
    }
}

/// Return the newer of two version strings. Returns None if either is invalid.
pub fn api_version_max<'a>(a: &'a str, b: &'a str) -> Option<&'a str> {
    match api_version_compare(a, b)? {
        VersionOrdering::Older => Some(b),
        _ => Some(a),
    }
}

/// Check if a version string is a valid semver triple.
pub fn is_valid_semver(version: &str) -> bool {
    parse_semver(version).is_some()
}

// ---------------------------------------------------------------------------
// API registry helpers & iteration
// ---------------------------------------------------------------------------

impl ApiRegistry {
    /// Returns the number of contribution points registered.
    pub fn contribution_count(&self) -> usize {
        self.contribution_points.len()
    }

    /// Check if a specific contribution point type is registered.
    pub fn has_contribution(&self, point: &ContributionPoint) -> bool {
        self.contribution_points.contains(point)
    }

    /// Returns a summary of the registry state.
    pub fn summary(&self) -> String {
        format!(
            "ApiRegistry: {} namespaces, {} contributions",
            self.namespace_count(),
            self.contribution_count(),
        )
    }

    /// Clear all registered namespaces and contribution points.
    pub fn clear(&mut self) {
        self.namespace_proxies.clear();
        self.contribution_points.clear();
    }
}

impl fmt::Display for ApiRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let features = api_supported_features(&self.capabilities);
        write!(
            f,
            "ApiRegistry({} ns, {} contrib, features={})",
            self.namespace_count(),
            self.contribution_count(),
            if features.is_empty() { "none".to_string() } else { features.join("+") },
        )
    }
}

impl fmt::Display for ApiCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let features = api_supported_features(self);
        if features.is_empty() {
            write!(f, "none")
        } else {
            write!(f, "{}", features.join("+"))
        }
    }
}

impl ActivationEvent {
    /// Returns a canonical string representation for serialization.
    pub fn to_key(&self) -> String {
        match self {
            ActivationEvent::OnLanguage(l) => format!("onLanguage:{}", l),
            ActivationEvent::OnCommand(c) => format!("onCommand:{}", c),
            ActivationEvent::OnView(v) => format!("onView:{}", v),
            ActivationEvent::OnUri => "onUri".to_string(),
            ActivationEvent::OnStartupFinished => "onStartupFinished".to_string(),
            ActivationEvent::Star => "*".to_string(),
            ActivationEvent::OnDebug(d) => format!("onDebug:{}", d),
            ActivationEvent::OnFileSystem(fs) => format!("onFileSystem:{}", fs),
            ActivationEvent::OnWebviewPanel(e) => format!("onWebviewPanel:{}", e),
            ActivationEvent::OnNotebook(n) => format!("onNotebook:{}", n),
            ActivationEvent::OnAuthenticationRequest(a) => format!("onAuthenticationRequest:{}", a),
            ActivationEvent::OnTerminalProfile(t) => format!("onTerminalProfile:{}", t),
            ActivationEvent::WorkspaceContains(p) => format!("workspaceContains:{}", p),
        }
    }

    /// Returns true if this is a wildcard activation (activates on startup).
    pub fn is_eager(&self) -> bool {
        matches!(self, ActivationEvent::Star)
    }
}

// ---------------------------------------------------------------------------
// API versioning helpers
// ---------------------------------------------------------------------------

/// Parsed semver version for API compatibility checks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    /// Parse a "major.minor.patch" string.
    pub fn parse(s: &str) -> Option<Self> {
        let (major, minor, patch) = parse_semver(s)?;
        Some(Self { major, minor, patch })
    }

    /// Check if `self` satisfies a `^required` constraint (same major, >= required).
    pub fn satisfies_caret(&self, required: &SemVer) -> bool {
        if self.major != required.major {
            return false;
        }
        (self.minor, self.patch) >= (required.minor, required.patch)
    }

    /// Format as a version string.
    pub fn to_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

// ---------------------------------------------------------------------------
// Capability enumeration
// ---------------------------------------------------------------------------

/// Enumerate all capability names and their current values.
pub fn enumerate_capabilities(caps: &ApiCapabilities) -> Vec<(&'static str, bool)> {
    vec![
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
    ]
}

/// Count the number of enabled capabilities.
pub fn count_enabled_capabilities(caps: &ApiCapabilities) -> usize {
    enumerate_capabilities(caps)
        .iter()
        .filter(|(_, v)| *v)
        .count()
}

// ---------------------------------------------------------------------------
// Extension metadata validation
// ---------------------------------------------------------------------------

/// Errors that can occur when validating extension metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionMetadataError {
    /// The extension ID is empty or invalid.
    InvalidId(String),
    /// The display name is empty.
    EmptyDisplayName,
    /// The engine version constraint is not satisfied.
    IncompatibleEngine { required: String, current: String },
    /// An activation event string is not recognized.
    InvalidActivationEvent(String),
}

impl fmt::Display for ExtensionMetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(id) => write!(f, "invalid extension id: '{id}'"),
            Self::EmptyDisplayName => write!(f, "display name must not be empty"),
            Self::IncompatibleEngine { required, current } =>
                write!(f, "engine {current} does not satisfy {required}"),
            Self::InvalidActivationEvent(ev) =>
                write!(f, "unrecognized activation event: '{ev}'"),
        }
    }
}

/// Metadata for a VS Code extension (subset of package.json fields).
#[derive(Debug, Clone)]
pub struct ExtensionMetadata {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub engine_version: String,
    pub activation_events: Vec<String>,
}

/// Validate extension metadata, returning all errors found.
pub fn validate_extension_metadata(meta: &ExtensionMetadata) -> Vec<ExtensionMetadataError> {
    let mut errors = Vec::new();

    // ID must be "publisher.name" format
    if meta.id.is_empty() || !meta.id.contains('.') {
        errors.push(ExtensionMetadataError::InvalidId(meta.id.clone()));
    }

    if meta.display_name.trim().is_empty() {
        errors.push(ExtensionMetadataError::EmptyDisplayName);
    }

    // Check engine compatibility
    let check = ApiVersionCheck::new(API_VERSION);
    if !check.is_compatible(&meta.engine_version) {
        errors.push(ExtensionMetadataError::IncompatibleEngine {
            required: meta.engine_version.clone(),
            current: API_VERSION.to_string(),
        });
    }

    // Validate activation events
    for ev in &meta.activation_events {
        if ActivationEvent::parse(ev).is_none() {
            errors.push(ExtensionMetadataError::InvalidActivationEvent(ev.clone()));
        }
    }

    errors
}

// ---------------------------------------------------------------------------
// API permission model
// ---------------------------------------------------------------------------

/// Permissions that can be granted to an extension for API access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiPermission {
    /// Read-only access to workspace files.
    FileSystemRead,
    /// Write access to workspace files.
    FileSystemWrite,
    /// Execute commands in the editor.
    CommandExecution,
    /// Access to terminal APIs.
    TerminalAccess,
    /// Access to debug APIs.
    DebugAccess,
    /// Network requests (outbound HTTP, WebSocket).
    NetworkAccess,
    /// Access to clipboard read/write.
    ClipboardAccess,
    /// Access to environment variables and shell.
    EnvironmentAccess,
    /// Access to authentication providers.
    AuthenticationAccess,
    /// Access to language model / AI APIs.
    LanguageModelAccess,
}

/// A set of permissions granted to a specific extension.
#[derive(Debug, Clone)]
pub struct ExtensionPermissions {
    extension_id: String,
    granted: std::collections::HashSet<ApiPermission>,
    denied_log: Vec<(ApiPermission, String)>,
}

impl ExtensionPermissions {
    /// Create a new permission set for an extension with no permissions granted.
    pub fn new(extension_id: impl Into<String>) -> Self {
        Self {
            extension_id: extension_id.into(),
            granted: std::collections::HashSet::new(),
            denied_log: Vec::new(),
        }
    }

    /// Grant a permission.
    pub fn grant(&mut self, perm: ApiPermission) {
        self.granted.insert(perm);
    }

    /// Revoke a previously granted permission.
    pub fn revoke(&mut self, perm: ApiPermission) {
        self.granted.remove(&perm);
    }

    /// Check whether a permission is currently granted.
    pub fn has(&self, perm: ApiPermission) -> bool {
        self.granted.contains(&perm)
    }

    /// Attempt to use a permission. Returns `Ok(())` if granted, or `Err` with
    /// a description and records the denial.
    pub fn check(&mut self, perm: ApiPermission, context: &str) -> Result<(), String> {
        if self.granted.contains(&perm) {
            Ok(())
        } else {
            let msg = format!(
                "extension '{}' denied {:?} (context: {})",
                self.extension_id, perm, context
            );
            self.denied_log.push((perm, msg.clone()));
            Err(msg)
        }
    }

    /// Return all recorded permission denials.
    pub fn denial_log(&self) -> &[(ApiPermission, String)] {
        &self.denied_log
    }

    /// Return the extension id.
    pub fn extension_id(&self) -> &str {
        &self.extension_id
    }

    /// Return the number of granted permissions.
    pub fn granted_count(&self) -> usize {
        self.granted.len()
    }
}

// ---------------------------------------------------------------------------
// API rate limiting per extension
// ---------------------------------------------------------------------------

/// Simple sliding-window rate limiter for API calls per extension.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    extension_id: String,
    /// Maximum number of calls allowed within the window.
    max_calls: u64,
    /// Window duration in milliseconds.
    window_ms: u64,
    /// Timestamps (in ms since an arbitrary epoch) of recent calls.
    timestamps: Vec<u64>,
}

impl RateLimiter {
    /// Create a new rate limiter.
    pub fn new(extension_id: impl Into<String>, max_calls: u64, window_ms: u64) -> Self {
        Self {
            extension_id: extension_id.into(),
            max_calls,
            window_ms,
            timestamps: Vec::new(),
        }
    }

    /// Attempt to record a call at the given timestamp (ms).
    /// Returns `Ok(remaining)` with the number of remaining calls in the window,
    /// or `Err` if the rate limit is exceeded.
    pub fn try_acquire(&mut self, now_ms: u64) -> Result<u64, String> {
        self.prune(now_ms);
        if self.timestamps.len() as u64 >= self.max_calls {
            return Err(format!(
                "rate limit exceeded for '{}': {} calls in {}ms window",
                self.extension_id, self.max_calls, self.window_ms
            ));
        }
        self.timestamps.push(now_ms);
        Ok(self.max_calls - self.timestamps.len() as u64)
    }

    /// Remove timestamps outside the current window.
    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&ts| ts > cutoff);
    }

    /// Return how many calls have been made in the current window.
    pub fn current_count(&self, now_ms: u64) -> u64 {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.iter().filter(|&&ts| ts > cutoff).count() as u64
    }

    /// Reset the limiter, clearing all recorded timestamps.
    pub fn reset(&mut self) {
        self.timestamps.clear();
    }

    /// Return the extension id this limiter is associated with.
    pub fn extension_id(&self) -> &str {
        &self.extension_id
    }
}

// ---------------------------------------------------------------------------
// Extension capability declaration
// ---------------------------------------------------------------------------

/// Capabilities that an extension declares it requires from the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionCapabilityDeclaration {
    pub extension_id: String,
    pub required_namespaces: Vec<String>,
    pub required_permissions: Vec<ApiPermission>,
    pub min_api_version: Option<String>,
}

impl ExtensionCapabilityDeclaration {
    /// Create a new capability declaration for an extension.
    pub fn new(extension_id: impl Into<String>) -> Self {
        Self {
            extension_id: extension_id.into(),
            required_namespaces: Vec::new(),
            required_permissions: Vec::new(),
            min_api_version: None,
        }
    }

    /// Declare that a namespace is required.
    pub fn require_namespace(mut self, ns: impl Into<String>) -> Self {
        self.required_namespaces.push(ns.into());
        self
    }

    /// Declare that a permission is required.
    pub fn require_permission(mut self, perm: ApiPermission) -> Self {
        self.required_permissions.push(perm);
        self
    }

    /// Declare the minimum API version required.
    pub fn require_api_version(mut self, version: impl Into<String>) -> Self {
        self.min_api_version = Some(version.into());
        self
    }

    /// Validate the declaration against the current registry and API version.
    /// Returns a list of unsatisfied requirements as human-readable strings.
    pub fn validate_against(&self, registry: &ApiRegistry) -> Vec<String> {
        let mut issues = Vec::new();

        for ns in &self.required_namespaces {
            if !registry.has_namespace(ns) {
                issues.push(format!("required namespace '{}' is not registered", ns));
            }
        }

        if let Some(ref min_ver) = self.min_api_version {
            let check = ApiVersionCheck::new(API_VERSION);
            if !check.is_compatible(min_ver) {
                issues.push(format!(
                    "requires API version {} but host provides {}",
                    min_ver, API_VERSION
                ));
            }
        }

        issues
    }
}

// ---------------------------------------------------------------------------
// ApiVersionNegotiator – select compatible API version
// ---------------------------------------------------------------------------

/// Negotiates the highest compatible API version between host and extension.
pub struct ApiVersionNegotiator {
    host_version: (u32, u32, u32),
}

impl ApiVersionNegotiator {
    /// Create a negotiator from the host's current API version string.
    pub fn new(host_version: &str) -> Option<Self> {
        let parts: Vec<&str> = host_version.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts[2].parse().ok()?;
        Some(Self { host_version: (major, minor, patch) })
    }

    /// Check if an extension's minimum required version is compatible.
    pub fn is_compatible(&self, min_version: &str) -> bool {
        if let Some(req) = Self::parse_version(min_version) {
            // Same major, host minor >= required minor
            self.host_version.0 == req.0 && self.host_version.1 >= req.1
        } else {
            false
        }
    }

    /// Select the best version from a list of supported versions.
    pub fn select_best<'a>(&self, candidates: &[&'a str]) -> Option<&'a str> {
        candidates
            .iter()
            .filter(|v| self.is_compatible(v))
            .max_by(|a, b| {
                let va = Self::parse_version(a).unwrap_or((0, 0, 0));
                let vb = Self::parse_version(b).unwrap_or((0, 0, 0));
                va.cmp(&vb)
            })
            .copied()
    }

    fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() != 3 { return None; }
        Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
    }

    /// Return the host version as a tuple.
    pub fn host_version(&self) -> (u32, u32, u32) {
        self.host_version
    }
}

// ---------------------------------------------------------------------------
// ApiDeprecationWarner – tracks deprecated API calls
// ---------------------------------------------------------------------------

/// Record of a deprecated API call.
#[derive(Debug, Clone)]
pub struct DeprecationRecord {
    pub api_name: String,
    pub replacement: Option<String>,
    pub call_count: u64,
}

/// Tracks deprecated API usage and emits warnings.
pub struct ApiDeprecationWarner {
    records: HashMap<String, DeprecationRecord>,
}

impl ApiDeprecationWarner {
    /// Create a new empty warner.
    pub fn new() -> Self {
        Self { records: HashMap::new() }
    }

    /// Register a deprecated API and its replacement.
    pub fn register(&mut self, api_name: impl Into<String>, replacement: Option<String>) {
        let name = api_name.into();
        self.records.entry(name.clone()).or_insert(DeprecationRecord {
            api_name: name,
            replacement,
            call_count: 0,
        });
    }

    /// Record a call to a deprecated API. Returns `true` if the API is deprecated.
    pub fn record_call(&mut self, api_name: &str) -> bool {
        if let Some(rec) = self.records.get_mut(api_name) {
            rec.call_count += 1;
            true
        } else {
            false
        }
    }

    /// Check if an API is deprecated.
    pub fn is_deprecated(&self, api_name: &str) -> bool {
        self.records.contains_key(api_name)
    }

    /// Get all deprecation records with at least one call.
    pub fn active_warnings(&self) -> Vec<&DeprecationRecord> {
        self.records.values().filter(|r| r.call_count > 0).collect()
    }

    /// Generate a warning message for a deprecated API.
    pub fn warning_message(&self, api_name: &str) -> Option<String> {
        self.records.get(api_name).map(|r| {
            match &r.replacement {
                Some(repl) => format!("'{}' is deprecated, use '{}' instead", r.api_name, repl),
                None => format!("'{}' is deprecated with no replacement", r.api_name),
            }
        })
    }

    /// Number of registered deprecated APIs.
    pub fn registered_count(&self) -> usize {
        self.records.len()
    }
}

// ---------------------------------------------------------------------------
// ApiCallThrottler – rate limiting for API calls
// ---------------------------------------------------------------------------

/// Simple token-bucket style rate limiter for API calls.
pub struct ApiCallThrottler {
    calls: HashMap<String, Vec<u64>>,
    max_calls_per_window: usize,
    window_ms: u64,
}

impl ApiCallThrottler {
    /// Create a throttler allowing `max_calls` within `window_ms` milliseconds.
    pub fn new(max_calls: usize, window_ms: u64) -> Self {
        Self {
            calls: HashMap::new(),
            max_calls_per_window: max_calls,
            window_ms,
        }
    }

    /// Try to record a call at the given timestamp. Returns `true` if allowed.
    pub fn try_call(&mut self, api_name: &str, now_ms: u64) -> bool {
        let entry = self.calls.entry(api_name.to_string()).or_default();
        let cutoff = now_ms.saturating_sub(self.window_ms);
        entry.retain(|&ts| ts > cutoff);
        if entry.len() < self.max_calls_per_window {
            entry.push(now_ms);
            true
        } else {
            false
        }
    }

    /// Number of calls remaining in the current window.
    pub fn remaining(&self, api_name: &str, now_ms: u64) -> usize {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        let used = self.calls.get(api_name)
            .map(|ts| ts.iter().filter(|&&t| t > cutoff).count())
            .unwrap_or(0);
        self.max_calls_per_window.saturating_sub(used)
    }

    /// Check if an API is currently throttled.
    pub fn is_throttled(&self, api_name: &str, now_ms: u64) -> bool {
        self.remaining(api_name, now_ms) == 0
    }
}

// ---------------------------------------------------------------------------
// Extension capability probing
// ---------------------------------------------------------------------------

/// Probes whether an extension has specific capabilities based on its declarations.
pub struct ExtensionCapabilityProbe {
    supported_namespaces: Vec<String>,
    supported_events: Vec<String>,
}

impl ExtensionCapabilityProbe {
    /// Create a probe from lists of supported namespaces and activation events.
    pub fn new(namespaces: Vec<String>, events: Vec<String>) -> Self {
        Self { supported_namespaces: namespaces, supported_events: events }
    }

    /// Check if the extension supports a given namespace.
    pub fn supports_namespace(&self, ns: &str) -> bool {
        self.supported_namespaces.iter().any(|n| n == ns)
    }

    /// Check if the extension responds to a given activation event.
    pub fn responds_to_event(&self, event: &str) -> bool {
        self.supported_events.iter().any(|e| e == event)
    }

    /// List all supported namespaces.
    pub fn namespaces(&self) -> &[String] {
        &self.supported_namespaces
    }

    /// Return the number of supported capabilities.
    pub fn capability_count(&self) -> usize {
        self.supported_namespaces.len() + self.supported_events.len()
    }
}


// ---------------------------------------------------------------------------
// ApiMockProvider
// ---------------------------------------------------------------------------

/// A mock API provider for testing extension API interactions.
///
/// Records all namespace calls and can return pre-configured responses.
#[derive(Debug, Clone)]
pub struct ApiMockProvider {
    call_log: Vec<ApiMockCall>,
    responses: HashMap<String, Vec<String>>,
}

/// A recorded API call made through the mock provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiMockCall {
    pub namespace: String,
    pub method: String,
    pub args: Vec<String>,
}

impl ApiMockProvider {
    /// Create a new empty mock provider.
    pub fn new() -> Self {
        Self {
            call_log: Vec::new(),
            responses: HashMap::new(),
        }
    }

    /// Register a canned response for a `namespace.method` key.
    pub fn register_response(&mut self, key: impl Into<String>, values: Vec<String>) {
        self.responses.insert(key.into(), values);
    }

    /// Record a call and return any registered response.
    pub fn call(
        &mut self,
        namespace: &str,
        method: &str,
        args: Vec<String>,
    ) -> Option<Vec<String>> {
        self.call_log.push(ApiMockCall {
            namespace: namespace.to_string(),
            method: method.to_string(),
            args,
        });
        let key = format!("{namespace}.{method}");
        self.responses.get(&key).cloned()
    }

    /// Return all recorded calls.
    pub fn calls(&self) -> &[ApiMockCall] {
        &self.call_log
    }

    /// Return calls filtered by namespace.
    pub fn calls_for_namespace(&self, ns: &str) -> Vec<&ApiMockCall> {
        self.call_log.iter().filter(|c| c.namespace == ns).collect()
    }

    /// Return calls filtered by method name.
    pub fn calls_for_method(&self, method: &str) -> Vec<&ApiMockCall> {
        self.call_log.iter().filter(|c| c.method == method).collect()
    }

    /// Number of calls recorded so far.
    pub fn call_count(&self) -> usize {
        self.call_log.len()
    }

    /// Clear the call log.
    pub fn reset(&mut self) {
        self.call_log.clear();
    }

    /// Check if a specific method was ever called.
    pub fn was_called(&self, namespace: &str, method: &str) -> bool {
        self.call_log.iter().any(|c| c.namespace == namespace && c.method == method)
    }
}

// ---------------------------------------------------------------------------
// ApiEventBus
// ---------------------------------------------------------------------------

/// Typed event that can be dispatched through the event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiEvent {
    /// The namespace that fired this event (e.g. "workspace").
    pub namespace: String,
    /// Event name (e.g. "onDidChangeConfiguration").
    pub name: String,
    /// Serialised event data.
    pub data: Option<String>,
}

impl ApiEvent {
    /// Create a new event.
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
            data: None,
        }
    }

    /// Attach data to the event.
    pub fn with_data(mut self, data: impl Into<String>) -> Self {
        self.data = Some(data.into());
        self
    }
}

impl fmt::Display for ApiEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.namespace, self.name)?;
        if let Some(d) = &self.data {
            write!(f, " ({d})")?;
        }
        Ok(())
    }
}

/// A simple in-process event bus for API events.
///
/// Listeners are stored as `(namespace_filter, event_name_filter)` pairs.
/// A `None` filter matches all values.
#[derive(Debug, Clone)]
pub struct ApiEventBus {
    history: Vec<ApiEvent>,
    listeners: Vec<(Option<String>, Option<String>)>,
}

impl ApiEventBus {
    /// Create a new empty event bus.
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            listeners: Vec::new(),
        }
    }

    /// Register a listener with optional namespace and event name filters.
    pub fn on(
        &mut self,
        namespace: Option<String>,
        event_name: Option<String>,
    ) -> usize {
        let id = self.listeners.len();
        self.listeners.push((namespace, event_name));
        id
    }

    /// Fire an event, recording it in history and returning matching listener IDs.
    pub fn emit(&mut self, event: ApiEvent) -> Vec<usize> {
        let matching: Vec<usize> = self
            .listeners
            .iter()
            .enumerate()
            .filter(|(_, (ns_filter, name_filter))| {
                let ns_ok = ns_filter.as_ref().map_or(true, |f| f == &event.namespace);
                let name_ok = name_filter.as_ref().map_or(true, |f| f == &event.name);
                ns_ok && name_ok
            })
            .map(|(id, _)| id)
            .collect();
        self.history.push(event);
        matching
    }

    /// Get the full event history.
    pub fn history(&self) -> &[ApiEvent] {
        &self.history
    }

    /// Get events for a specific namespace.
    pub fn events_for_namespace(&self, ns: &str) -> Vec<&ApiEvent> {
        self.history.iter().filter(|e| e.namespace == ns).collect()
    }

    /// Total number of events emitted.
    pub fn event_count(&self) -> usize {
        self.history.len()
    }

    /// Number of registered listeners.
    pub fn listener_count(&self) -> usize {
        self.listeners.len()
    }
}

// ---------------------------------------------------------------------------
// API request interceptor
// ---------------------------------------------------------------------------

/// Intercept decision for an API request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterceptAction {
    /// Allow the request to proceed.
    Allow,
    /// Block the request with a reason.
    Block(String),
    /// Modify the request arguments before proceeding.
    Rewrite(Vec<String>),
}

/// Rule for intercepting API requests by namespace/method.
#[derive(Debug, Clone)]
pub struct InterceptRule {
    pub namespace: String,
    pub method_pattern: String,
    pub action: InterceptAction,
}

/// Intercepts API requests based on configured rules.
#[derive(Debug, Clone)]
pub struct ApiRequestInterceptor {
    rules: Vec<InterceptRule>,
}

impl ApiRequestInterceptor {
    /// Create a new empty interceptor.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add an intercept rule.
    pub fn add_rule(&mut self, rule: InterceptRule) {
        self.rules.push(rule);
    }

    /// Evaluate a request and return the applicable action.
    /// First matching rule wins.
    pub fn evaluate(&self, namespace: &str, method: &str) -> InterceptAction {
        for rule in &self.rules {
            if rule.namespace == namespace {
                if rule.method_pattern == "*" || rule.method_pattern == method {
                    return rule.action.clone();
                }
            }
        }
        InterceptAction::Allow
    }

    /// Number of configured rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Check if any rule would block a given namespace/method combination.
    pub fn would_block(&self, namespace: &str, method: &str) -> bool {
        matches!(self.evaluate(namespace, method), InterceptAction::Block(_))
    }

    /// Remove all rules for a specific namespace.
    pub fn clear_namespace(&mut self, namespace: &str) {
        self.rules.retain(|r| r.namespace != namespace);
    }
}

// ---------------------------------------------------------------------------
// API compatibility layer
// ---------------------------------------------------------------------------

/// Represents a minimum version requirement for an API feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiVersionRequirement {
    pub feature_name: String,
    pub min_version: String,
}

/// Checks whether the current API version satisfies feature requirements.
#[derive(Debug, Clone)]
pub struct ApiCompatibilityLayer {
    current_version: String,
    requirements: Vec<ApiVersionRequirement>,
}

impl ApiCompatibilityLayer {
    /// Create a new compatibility layer for the given API version.
    pub fn new(current_version: impl Into<String>) -> Self {
        Self {
            current_version: current_version.into(),
            requirements: Vec::new(),
        }
    }

    /// Register a feature requirement.
    pub fn require(
        &mut self,
        feature: impl Into<String>,
        min_version: impl Into<String>,
    ) {
        self.requirements.push(ApiVersionRequirement {
            feature_name: feature.into(),
            min_version: min_version.into(),
        });
    }

    /// Parse a dotted version string to a comparable tuple.
    fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some((
            parts[0].parse().ok()?,
            parts[1].parse().ok()?,
            parts[2].parse().ok()?,
        ))
    }

    /// Check if the current version satisfies a minimum.
    fn version_satisfies(current: &str, min: &str) -> bool {
        match (Self::parse_version(current), Self::parse_version(min)) {
            (Some(c), Some(m)) => c >= m,
            _ => false,
        }
    }

    /// Check if a specific feature is supported.
    pub fn supports_feature(&self, feature: &str) -> bool {
        self.requirements
            .iter()
            .find(|r| r.feature_name == feature)
            .map(|r| Self::version_satisfies(&self.current_version, &r.min_version))
            .unwrap_or(true)
    }

    /// Return all unsupported features.
    pub fn unsupported_features(&self) -> Vec<&str> {
        self.requirements
            .iter()
            .filter(|r| !Self::version_satisfies(&self.current_version, &r.min_version))
            .map(|r| r.feature_name.as_str())
            .collect()
    }

    /// Return the current API version.
    pub fn current_version(&self) -> &str {
        &self.current_version
    }

    /// Total number of registered requirements.
    pub fn requirement_count(&self) -> usize {
        self.requirements.len()
    }
}




// ---------------------------------------------------------------------------
// ext_api – Extension protocol helpers
// ---------------------------------------------------------------------------

/// Activation event kinds for extension lifecycle management.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum XExtApiActivationKind {
    /// Activate on a specific language.
    Language(String),
    /// Activate on a command.
    Command(String),
    /// Activate on a workspace-contains glob.
    WorkspaceContains(String),
    /// Activate on a custom URI scheme.
    UriScheme(String),
    /// Activate on startup.
    Star,
}

impl XExtApiActivationKind {
    /// Parse an activation event string like `"onLanguage:rust"`.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw == "*" {
            return Some(Self::Star);
        }
        let (kind, value) = raw.split_once(':')?;
        match kind {
            "onLanguage" => Some(Self::Language(value.to_string())),
            "onCommand" => Some(Self::Command(value.to_string())),
            "workspaceContains" => Some(Self::WorkspaceContains(value.to_string())),
            "onUri" => Some(Self::UriScheme(value.to_string())),
            _ => None,
        }
    }

    /// Returns true if this activation kind targets a specific language.
    pub fn is_language(&self) -> bool {
        matches!(self, Self::Language(_))
    }
}

/// Message envelope for extension host RPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XExtApiRpcEnvelope {
    pub seq: u64,
    pub method: String,
    pub payload: String,
}

impl XExtApiRpcEnvelope {
    /// Create a new RPC envelope.
    pub fn new(seq: u64, method: impl Into<String>, payload: impl Into<String>) -> Self {
        Self { seq, method: method.into(), payload: payload.into() }
    }

    /// Returns true when the envelope carries a response (method starts with `$/`).
    pub fn is_response(&self) -> bool {
        self.method.starts_with("$/")
    }

    /// Compute a simple checksum of the payload (sum of bytes mod 2^32).
    pub fn payload_checksum(&self) -> u32 {
        self.payload.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32))
    }
}

/// Batch multiple RPC envelopes and return their sequence numbers.
pub fn x_ext_api_collect_sequences(envelopes: &[XExtApiRpcEnvelope]) -> Vec<u64> {
    envelopes.iter().map(|e| e.seq).collect()
}

/// Filter envelopes by method prefix.
pub fn x_ext_api_filter_by_method<'a>(
    envelopes: &'a [XExtApiRpcEnvelope],
    method_prefix: &str,
) -> Vec<&'a XExtApiRpcEnvelope> {
    envelopes.iter().filter(|e| e.method.starts_with(method_prefix)).collect()
}

/// Deduplicate envelopes by sequence number, keeping the first occurrence.
pub fn x_ext_api_dedup_by_seq(envelopes: Vec<XExtApiRpcEnvelope>) -> Vec<XExtApiRpcEnvelope> {
    let mut seen = std::collections::HashSet::new();
    envelopes.into_iter().filter(|e| seen.insert(e.seq)).collect()
}

/// Simple capability negotiation: given requested and available feature sets,
/// return the intersection.
pub fn x_ext_api_negotiate_capabilities(
    requested: &[&str],
    available: &[&str],
) -> Vec<String> {
    requested.iter()
        .filter(|r| available.contains(r))
        .map(|s| s.to_string())
        .collect()
}

/// Version tuple for extension API compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct XExtApiApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl XExtApiApiVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
    /// Check if this version satisfies a minimum requirement.
    pub fn satisfies(&self, min: &Self) -> bool {
        (self.major, self.minor, self.patch) >= (min.major, min.minor, min.patch)
    }
}

impl std::fmt::Display for XExtApiApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}


/// Configuration manager for ext_api functionality.
pub struct ExtApiConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ExtApiConfig {
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

    pub fn merge(&mut self, other: &ExtApiConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for ext_api operations.
pub struct ExtApiRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ExtApiRateTracker {
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

/// Validation result collector for ext_api.
pub struct ExtApiValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ExtApiValidationCollector {
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

    pub fn merge(&mut self, other: &ExtApiValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for ext_api
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtApiRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtApiRingBuf {
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
pub struct XaExtApiCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtApiCounter {
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

impl Default for XaExtApiCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 49
// ---------------------------------------------------------------------------

/// Generic object pool `Xc49Pool<T>`.
pub struct Xc49Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc49Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc49PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc49Pool<T> {
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
    pub fn stats(&self) -> Xc49PoolStats {
        Xc49PoolStats {
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

impl<T> Default for Xc49Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc49Scheduler`.
pub struct Xc49Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc49Scheduler {
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

impl Default for Xc49Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_49 hash for the given byte slice.
pub fn xc_49_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_49 convention.
pub fn xc_49_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_48 deepening: state machine + event bus ---

/// States for the Xd48 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd48State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd48State {
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
pub struct Xd48Transition {
    pub from: Xd48State,
    pub to: Xd48State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd48StateMachine {
    current: Xd48State,
    history: Vec<Xd48Transition>,
    step_counter: usize,
}

impl Xd48StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd48State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd48State {
        self.current
    }

    pub fn history(&self) -> &[Xd48Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd48State) -> Result<Xd48State, String> {
        let allowed = match (self.current, target) {
            (Xd48State::Idle, Xd48State::Running) => true,
            (Xd48State::Running, Xd48State::Paused) => true,
            (Xd48State::Running, Xd48State::Done) => true,
            (Xd48State::Paused, Xd48State::Running) => true,
            (Xd48State::Paused, Xd48State::Done) => true,
            (Xd48State::Done, Xd48State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_48: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd48Transition {
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
            "Xd48SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd48State> {
        let prefix = "Xd48SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd48State::Idle),
            "Running" => Some(Xd48State::Running),
            "Paused" => Some(Xd48State::Paused),
            "Done" => Some(Xd48State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd48State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd48 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd48Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd48Event {
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

type Xd48HandlerFn = Box<dyn Fn(&Xd48Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd48EventBus {
    handlers: Vec<(usize, Option<String>, Xd48HandlerFn)>,
    next_id: usize,
    published: Vec<Xd48Event>,
}

impl Xd48EventBus {
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
        F: Fn(&Xd48Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd48Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd48Event) {
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

    pub fn published_events(&self) -> &[Xd48Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_count_works() {
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
    fn extapi_validator_accepts_and_rejects() {
        let mut v = ExtApiValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn extapi_validator_warnings() {
        let mut v = ExtApiValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn extapi_validator_clear_and_merge() {
        let mut v = ExtApiValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = ExtApiValidationCollector::new();
        a.add_error("a_err");
        let mut b = ExtApiValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    // -- api_version_compare tests ------------------------------------------

    #[test]
    fn version_compare_same() {
        assert_eq!(api_version_compare("1.0.0", "1.0.0"), Some(VersionOrdering::Same));
    }

    #[test]
    fn version_compare_older_major() {
        assert_eq!(api_version_compare("1.0.0", "2.0.0"), Some(VersionOrdering::Older));
    }

    #[test]
    fn version_compare_newer_minor() {
        assert_eq!(api_version_compare("1.5.0", "1.3.0"), Some(VersionOrdering::Newer));
    }

    #[test]
    fn version_compare_patch() {
        assert_eq!(api_version_compare("1.0.1", "1.0.2"), Some(VersionOrdering::Older));
        assert_eq!(api_version_compare("1.0.3", "1.0.2"), Some(VersionOrdering::Newer));
    }

    #[test]
    fn version_compare_invalid() {
        assert_eq!(api_version_compare("1.0", "1.0.0"), None);
        assert_eq!(api_version_compare("abc", "1.0.0"), None);
    }

    #[test]
    fn version_satisfies_current() {
        assert_eq!(api_version_satisfies("1.0.0"), Some(true));
    }

    #[test]
    fn version_satisfies_too_new() {
        assert_eq!(api_version_satisfies("99.0.0"), Some(false));
    }

    #[test]
    fn version_max_picks_newer() {
        assert_eq!(api_version_max("1.2.0", "1.3.0"), Some("1.3.0"));
        assert_eq!(api_version_max("2.0.0", "1.9.9"), Some("2.0.0"));
    }

    #[test]
    fn is_valid_semver_checks() {
        assert!(is_valid_semver("1.0.0"));
        assert!(is_valid_semver("0.0.1"));
        assert!(!is_valid_semver("1.0"));
        assert!(!is_valid_semver(""));
        assert!(!is_valid_semver("a.b.c"));
    }

    #[test]
    fn parse_semver_valid() {
        assert_eq!(parse_semver("1.110.0"), Some((1, 110, 0)));
    }

    #[test]
    fn api_registry_summary() {
        let reg = ApiRegistry::new();
        let s = reg.summary();
        assert!(s.contains("0 namespaces"));
    }

    #[test]
    fn api_registry_display() {
        let reg = ApiRegistry::new();
        let s = reg.to_string();
        assert!(s.contains("ApiRegistry"));
    }

    #[test]
    fn api_capabilities_display() {
        let caps = ApiCapabilities {
            supports_proposed_api: false,
            supports_webview: false,
            supports_terminal: false,
            supports_debug: false,
            supports_notebook: false,
            supports_chat: false,
            supports_language_models: false,
            supports_testing: false,
            supports_authentication: false,
            supports_custom_editors: false,
        };
        let s = caps.to_string();
        assert_eq!(s, "none");
    }

    #[test]
    fn activation_event_to_key() {
        let e = ActivationEvent::OnLanguage("rust".to_string());
        assert_eq!(e.to_key(), "onLanguage:rust");
        assert_eq!(ActivationEvent::Star.to_key(), "*");
    }

    #[test]
    fn activation_event_is_eager() {
        assert!(ActivationEvent::Star.is_eager());
        assert!(!ActivationEvent::OnUri.is_eager());
    }

    #[test]
    fn api_registry_clear() {
        let mut reg = ApiRegistry::with_defaults();
        assert!(reg.namespace_count() > 0);
        reg.clear();
        assert_eq!(reg.namespace_count(), 0);
        assert_eq!(reg.contribution_count(), 0);
    }

    #[test]
    fn api_capabilities_display_with_flags() {
        let caps = ApiCapabilities::default();
        // default has terminal, debug, testing, authentication = true
        let s = caps.to_string();
        assert!(s.contains("terminal"));
        assert!(s.contains("debug"));
    }

    #[test]
    fn semver_parse_and_display() {
        let v = SemVer::parse("1.70.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 70);
        assert_eq!(v.patch, 3);
        assert_eq!(v.to_string(), "1.70.3");
        assert!(SemVer::parse("bad").is_none());
    }

    #[test]
    fn semver_satisfies_caret() {
        let current = SemVer::parse("1.110.0").unwrap();
        let req = SemVer::parse("1.70.0").unwrap();
        assert!(current.satisfies_caret(&req));
        let req_higher = SemVer::parse("1.120.0").unwrap();
        assert!(!current.satisfies_caret(&req_higher));
        // Different major
        let v2 = SemVer::parse("2.0.0").unwrap();
        assert!(!v2.satisfies_caret(&req));
    }

    #[test]
    fn enumerate_capabilities_returns_all() {
        let caps = ApiCapabilities::default();
        let all = enumerate_capabilities(&caps);
        assert_eq!(all.len(), 10);
        let enabled = count_enabled_capabilities(&caps);
        assert!(enabled >= 4); // terminal, debug, testing, authentication
    }

    #[test]
    fn validate_extension_metadata_valid() {
        let meta = ExtensionMetadata {
            id: "publisher.my-ext".into(),
            display_name: "My Extension".into(),
            version: "0.1.0".into(),
            engine_version: "1.70.0".into(),
            activation_events: vec!["onLanguage:rust".into(), "*".into()],
        };
        let errors = validate_extension_metadata(&meta);
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
    }

    #[test]
    fn validate_extension_metadata_invalid_id() {
        let meta = ExtensionMetadata {
            id: "no-dot".into(),
            display_name: "Name".into(),
            version: "0.1.0".into(),
            engine_version: "1.70.0".into(),
            activation_events: vec![],
        };
        let errors = validate_extension_metadata(&meta);
        assert!(errors.iter().any(|e| matches!(e, ExtensionMetadataError::InvalidId(_))));
    }

    #[test]
    fn validate_extension_metadata_bad_activation_event() {
        let meta = ExtensionMetadata {
            id: "pub.ext".into(),
            display_name: "Name".into(),
            version: "0.1.0".into(),
            engine_version: "1.70.0".into(),
            activation_events: vec!["badPrefix:value".into()],
        };
        let errors = validate_extension_metadata(&meta);
        assert!(errors.iter().any(|e| matches!(e, ExtensionMetadataError::InvalidActivationEvent(_))));
    }

    #[test]
    fn validate_extension_metadata_empty_display_name() {
        let meta = ExtensionMetadata {
            id: "pub.ext".into(),
            display_name: "   ".into(),
            version: "0.1.0".into(),
            engine_version: "1.70.0".into(),
            activation_events: vec![],
        };
        let errors = validate_extension_metadata(&meta);
        assert!(errors.iter().any(|e| matches!(e, ExtensionMetadataError::EmptyDisplayName)));
    }

    // -- permission model tests ---------------------------------------------

    #[test]
    fn permission_grant_and_check() {
        let mut perms = ExtensionPermissions::new("pub.my-ext");
        assert!(!perms.has(ApiPermission::FileSystemRead));
        perms.grant(ApiPermission::FileSystemRead);
        assert!(perms.has(ApiPermission::FileSystemRead));
        assert_eq!(perms.granted_count(), 1);

        assert!(perms.check(ApiPermission::FileSystemRead, "read file").is_ok());
        assert!(perms.check(ApiPermission::NetworkAccess, "fetch url").is_err());
        assert_eq!(perms.denial_log().len(), 1);
        assert_eq!(perms.extension_id(), "pub.my-ext");
    }

    #[test]
    fn permission_revoke() {
        let mut perms = ExtensionPermissions::new("pub.ext");
        perms.grant(ApiPermission::TerminalAccess);
        assert!(perms.has(ApiPermission::TerminalAccess));
        perms.revoke(ApiPermission::TerminalAccess);
        assert!(!perms.has(ApiPermission::TerminalAccess));
        assert_eq!(perms.granted_count(), 0);
    }

    // -- rate limiter tests -------------------------------------------------

    #[test]
    fn rate_limiter_allows_within_limit() {
        let mut rl = RateLimiter::new("pub.ext", 3, 1000);
        assert_eq!(rl.try_acquire(100), Ok(2));
        assert_eq!(rl.try_acquire(200), Ok(1));
        assert_eq!(rl.try_acquire(300), Ok(0));
        assert!(rl.try_acquire(400).is_err());
        assert_eq!(rl.current_count(400), 3);
        assert_eq!(rl.extension_id(), "pub.ext");
    }

    #[test]
    fn rate_limiter_window_expiry() {
        let mut rl = RateLimiter::new("pub.ext", 2, 1000);
        assert!(rl.try_acquire(100).is_ok());
        assert!(rl.try_acquire(200).is_ok());
        assert!(rl.try_acquire(300).is_err()); // limit hit
        // After the window expires, calls are allowed again
        assert!(rl.try_acquire(1200).is_ok());
        assert_eq!(rl.current_count(1200), 1);
    }

    #[test]
    fn rate_limiter_reset() {
        let mut rl = RateLimiter::new("pub.ext", 1, 1000);
        assert!(rl.try_acquire(100).is_ok());
        assert!(rl.try_acquire(200).is_err());
        rl.reset();
        assert!(rl.try_acquire(300).is_ok());
    }

    // -- capability declaration tests ---------------------------------------

    #[test]
    fn capability_declaration_validates_against_registry() {
        let reg = ApiRegistry::with_defaults();
        let decl = ExtensionCapabilityDeclaration::new("pub.ext")
            .require_namespace("commands")
            .require_namespace("window")
            .require_api_version("1.70.0");
        let issues = decl.validate_against(&reg);
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
    }

    #[test]
    fn capability_declaration_detects_missing_namespace() {
        let reg = ApiRegistry::new(); // empty registry
        let decl = ExtensionCapabilityDeclaration::new("pub.ext")
            .require_namespace("commands")
            .require_permission(ApiPermission::FileSystemRead);
        let issues = decl.validate_against(&reg);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("commands"));
    }

    // -- ApiVersionNegotiator tests --

    #[test]
    fn version_negotiator_compatible() {
        let neg = ApiVersionNegotiator::new("1.110.0").unwrap();
        assert!(neg.is_compatible("1.100.0"));
        assert!(neg.is_compatible("1.110.0"));
        assert!(!neg.is_compatible("1.111.0"));
        assert!(!neg.is_compatible("2.0.0"));
    }

    #[test]
    fn version_negotiator_select_best() {
        let neg = ApiVersionNegotiator::new("1.110.0").unwrap();
        let best = neg.select_best(&["1.90.0", "1.100.0", "1.110.0", "1.120.0"]);
        assert_eq!(best, Some("1.110.0"));
    }

    #[test]
    fn version_negotiator_invalid() {
        assert!(ApiVersionNegotiator::new("bad").is_none());
    }

    // -- ApiDeprecationWarner tests --

    #[test]
    fn deprecation_warner_basic() {
        let mut w = ApiDeprecationWarner::new();
        w.register("window.showModal", Some("window.showDialog".into()));
        assert!(w.is_deprecated("window.showModal"));
        assert!(!w.is_deprecated("window.showInfo"));
        assert!(w.record_call("window.showModal"));
        assert!(!w.record_call("window.showInfo"));
        assert_eq!(w.active_warnings().len(), 1);
    }

    #[test]
    fn deprecation_warner_message() {
        let mut w = ApiDeprecationWarner::new();
        w.register("old_api", Some("new_api".into()));
        let msg = w.warning_message("old_api").unwrap();
        assert!(msg.contains("deprecated"));
        assert!(msg.contains("new_api"));
    }

    #[test]
    fn deprecation_warner_no_replacement() {
        let mut w = ApiDeprecationWarner::new();
        w.register("removed_api", None);
        let msg = w.warning_message("removed_api").unwrap();
        assert!(msg.contains("no replacement"));
    }

    // -- ApiCallThrottler tests --

    #[test]
    fn throttler_allows_within_limit() {
        let mut t = ApiCallThrottler::new(3, 1000);
        assert!(t.try_call("api", 100));
        assert!(t.try_call("api", 200));
        assert!(t.try_call("api", 300));
        assert!(!t.try_call("api", 400));
    }

    #[test]
    fn throttler_window_expires() {
        let mut t = ApiCallThrottler::new(2, 1000);
        assert!(t.try_call("api", 100));
        assert!(t.try_call("api", 200));
        assert!(!t.try_call("api", 300));
        // After window expires
        assert!(t.try_call("api", 1200));
    }

    #[test]
    fn throttler_remaining() {
        let mut t = ApiCallThrottler::new(5, 1000);
        t.try_call("api", 100);
        t.try_call("api", 200);
        assert_eq!(t.remaining("api", 300), 3);
        assert!(!t.is_throttled("api", 300));
    }

    // -- ExtensionCapabilityProbe tests --

    #[test]
    fn capability_probe_checks() {
        let probe = ExtensionCapabilityProbe::new(
            vec!["commands".into(), "workspace".into()],
            vec!["onLanguage:rust".into()],
        );
        assert!(probe.supports_namespace("commands"));
        assert!(!probe.supports_namespace("debug"));
        assert!(probe.responds_to_event("onLanguage:rust"));
        assert_eq!(probe.capability_count(), 3);
    }

    #[test]
    fn capability_probe_empty() {
        let probe = ExtensionCapabilityProbe::new(vec![], vec![]);
        assert_eq!(probe.capability_count(), 0);
        assert!(!probe.supports_namespace("anything"));
    }

    // -----------------------------------------------------------------------
    // ApiMockProvider tests
    // -----------------------------------------------------------------------

    #[test]
    fn mock_provider_call_log() {
        let mut mock = ApiMockProvider::new();
        mock.call("commands", "execute", vec!["openFile".into()]);
        mock.call("workspace", "getConfig", vec![]);
        assert_eq!(mock.call_count(), 2);
        assert!(mock.was_called("commands", "execute"));
        assert!(!mock.was_called("commands", "register"));
    }

    #[test]
    fn mock_provider_responses() {
        let mut mock = ApiMockProvider::new();
        mock.register_response("workspace.getConfig", vec!["value1".into()]);
        let resp = mock.call("workspace", "getConfig", vec![]);
        assert_eq!(resp, Some(vec!["value1".into()]));
    }

    #[test]
    fn mock_provider_filter_by_namespace() {
        let mut mock = ApiMockProvider::new();
        mock.call("commands", "execute", vec![]);
        mock.call("workspace", "getConfig", vec![]);
        mock.call("commands", "register", vec![]);
        assert_eq!(mock.calls_for_namespace("commands").len(), 2);
    }

    #[test]
    fn mock_provider_reset() {
        let mut mock = ApiMockProvider::new();
        mock.call("commands", "execute", vec![]);
        assert_eq!(mock.call_count(), 1);
        mock.reset();
        assert_eq!(mock.call_count(), 0);
    }

    // -----------------------------------------------------------------------
    // ApiEventBus tests
    // -----------------------------------------------------------------------

    #[test]
    fn event_bus_emit_and_history() {
        let mut bus = ApiEventBus::new();
        bus.emit(ApiEvent::new("workspace", "didChange"));
        bus.emit(ApiEvent::new("window", "didOpen"));
        assert_eq!(bus.event_count(), 2);
        assert_eq!(bus.events_for_namespace("workspace").len(), 1);
    }

    #[test]
    fn event_bus_listeners() {
        let mut bus = ApiEventBus::new();
        let id0 = bus.on(Some("workspace".into()), None);
        let id1 = bus.on(None, Some("didChange".into()));
        let _id2 = bus.on(Some("window".into()), None);

        let matched = bus.emit(ApiEvent::new("workspace", "didChange"));
        assert!(matched.contains(&id0));
        assert!(matched.contains(&id1));
        assert_eq!(matched.len(), 2);
    }

    #[test]
    fn event_bus_wildcard_listener() {
        let mut bus = ApiEventBus::new();
        let id = bus.on(None, None);
        let matched = bus.emit(ApiEvent::new("any", "event"));
        assert!(matched.contains(&id));
    }

    #[test]
    fn event_with_data_display() {
        let event = ApiEvent::new("workspace", "didSave").with_data("file.rs");
        let s = format!("{event}");
        assert!(s.contains("workspace.didSave"));
        assert!(s.contains("file.rs"));
    }

    // -----------------------------------------------------------------------
    // ApiRequestInterceptor tests
    // -----------------------------------------------------------------------

    #[test]
    fn interceptor_default_allow() {
        let interceptor = ApiRequestInterceptor::new();
        assert_eq!(interceptor.evaluate("commands", "execute"), InterceptAction::Allow);
    }

    #[test]
    fn interceptor_block_rule() {
        let mut interceptor = ApiRequestInterceptor::new();
        interceptor.add_rule(InterceptRule {
            namespace: "debug".into(),
            method_pattern: "*".into(),
            action: InterceptAction::Block("debug disabled".into()),
        });
        assert!(interceptor.would_block("debug", "start"));
        assert!(!interceptor.would_block("commands", "execute"));
    }

    #[test]
    fn interceptor_specific_method() {
        let mut interceptor = ApiRequestInterceptor::new();
        interceptor.add_rule(InterceptRule {
            namespace: "workspace".into(),
            method_pattern: "deleteFile".into(),
            action: InterceptAction::Block("read-only".into()),
        });
        assert!(interceptor.would_block("workspace", "deleteFile"));
        assert!(!interceptor.would_block("workspace", "openFile"));
    }

    #[test]
    fn interceptor_clear_namespace() {
        let mut interceptor = ApiRequestInterceptor::new();
        interceptor.add_rule(InterceptRule {
            namespace: "debug".into(),
            method_pattern: "*".into(),
            action: InterceptAction::Block("no".into()),
        });
        assert_eq!(interceptor.rule_count(), 1);
        interceptor.clear_namespace("debug");
        assert_eq!(interceptor.rule_count(), 0);
    }

    // -----------------------------------------------------------------------
    // ApiCompatibilityLayer tests
    // -----------------------------------------------------------------------

    #[test]
    fn compat_feature_supported() {
        let mut compat = ApiCompatibilityLayer::new("1.110.0");
        compat.require("inlineChat", "1.100.0");
        assert!(compat.supports_feature("inlineChat"));
    }

    #[test]
    fn compat_feature_unsupported() {
        let mut compat = ApiCompatibilityLayer::new("1.90.0");
        compat.require("inlineChat", "1.100.0");
        assert!(!compat.supports_feature("inlineChat"));
    }

    #[test]
    fn compat_unknown_feature() {
        let compat = ApiCompatibilityLayer::new("1.110.0");
        assert!(compat.supports_feature("nonexistent"));
    }

    #[test]
    fn compat_unsupported_list() {
        let mut compat = ApiCompatibilityLayer::new("1.90.0");
        compat.require("featureA", "1.80.0");
        compat.require("featureB", "1.100.0");
        compat.require("featureC", "1.95.0");
        let unsupported = compat.unsupported_features();
        assert_eq!(unsupported.len(), 2);
        assert!(unsupported.contains(&"featureB"));
        assert!(unsupported.contains(&"featureC"));
    }



    // -- ext_api additional tests -------------------------------------------

    #[test]
    fn x_ext_api_activation_parse_language() {
        let ak = XExtApiActivationKind::parse("onLanguage:rust").unwrap();
        assert_eq!(ak, XExtApiActivationKind::Language("rust".into()));
        assert!(ak.is_language());
    }

    #[test]
    fn x_ext_api_activation_parse_command() {
        let ak = XExtApiActivationKind::parse("onCommand:editor.action.format").unwrap();
        assert_eq!(ak, XExtApiActivationKind::Command("editor.action.format".into()));
        assert!(!ak.is_language());
    }

    #[test]
    fn x_ext_api_activation_parse_star() {
        assert_eq!(XExtApiActivationKind::parse("*"), Some(XExtApiActivationKind::Star));
    }

    #[test]
    fn x_ext_api_activation_parse_unknown() {
        assert!(XExtApiActivationKind::parse("badKind:thing").is_none());
    }

    #[test]
    fn x_ext_api_activation_parse_workspace() {
        let ak = XExtApiActivationKind::parse("workspaceContains:**/Cargo.toml").unwrap();
        assert_eq!(ak, XExtApiActivationKind::WorkspaceContains("**/" .to_owned() + "Cargo.toml"));
    }

    #[test]
    fn x_ext_api_rpc_envelope_basic() {
        let env = XExtApiRpcEnvelope::new(1, "textDocument/didOpen", "{}" );
        assert_eq!(env.seq, 1);
        assert!(!env.is_response());
    }

    #[test]
    fn x_ext_api_rpc_envelope_response() {
        let env = XExtApiRpcEnvelope::new(2, "$/cancelRequest", "");
        assert!(env.is_response());
    }

    #[test]
    fn x_ext_api_rpc_payload_checksum() {
        let env = XExtApiRpcEnvelope::new(1, "m", "AB");
        assert_eq!(env.payload_checksum(), 65 + 66);
    }

    #[test]
    fn x_ext_api_collect_sequences_works() {
        let envs = vec![
            XExtApiRpcEnvelope::new(10, "a", ""),
            XExtApiRpcEnvelope::new(20, "b", ""),
        ];
        assert_eq!(x_ext_api_collect_sequences(&envs), vec![10, 20]);
    }

    #[test]
    fn x_ext_api_filter_by_method_works() {
        let envs = vec![
            XExtApiRpcEnvelope::new(1, "textDocument/open", ""),
            XExtApiRpcEnvelope::new(2, "workspace/config", ""),
            XExtApiRpcEnvelope::new(3, "textDocument/close", ""),
        ];
        let filtered = x_ext_api_filter_by_method(&envs, "textDocument/");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_ext_api_dedup_by_seq_works() {
        let envs = vec![
            XExtApiRpcEnvelope::new(1, "a", "first"),
            XExtApiRpcEnvelope::new(1, "a", "second"),
            XExtApiRpcEnvelope::new(2, "b", "third"),
        ];
        let deduped = x_ext_api_dedup_by_seq(envs);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].payload, "first");
    }

    #[test]
    fn x_ext_api_negotiate_capabilities_basic() {
        let result = x_ext_api_negotiate_capabilities(
            &["hover", "completion", "rename"],
            &["hover", "rename", "format"],
        );
        assert_eq!(result, vec!["hover", "rename"]);
    }

    #[test]
    fn x_ext_api_api_version_satisfies() {
        let v1 = XExtApiApiVersion::new(1, 80, 0);
        let min = XExtApiApiVersion::new(1, 70, 0);
        assert!(v1.satisfies(&min));
        assert!(!min.satisfies(&v1));
    }

    #[test]
    fn x_ext_api_api_version_display() {
        let v = XExtApiApiVersion::new(2, 3, 4);
        assert_eq!(v.to_string(), "2.3.4");
    }

    #[test]
    fn x_ext_api_api_version_ord() {
        let v1 = XExtApiApiVersion::new(1, 0, 0);
        let v2 = XExtApiApiVersion::new(1, 1, 0);
        assert!(v1 < v2);
    }


    #[test]
    fn ext_api_config_new() {
        let cfg = ExtApiConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn ext_api_config_set_get() {
        let mut cfg = ExtApiConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn ext_api_config_remove() {
        let mut cfg = ExtApiConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn ext_api_config_keys_sorted() {
        let mut cfg = ExtApiConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn ext_api_config_bump_version() {
        let mut cfg = ExtApiConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn ext_api_config_clear() {
        let mut cfg = ExtApiConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn ext_api_config_merge() {
        let mut cfg1 = ExtApiConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ExtApiConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn ext_api_config_disable() {
        let mut cfg = ExtApiConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn ext_api_rate_tracker_empty() {
        let rt = ExtApiRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn ext_api_rate_tracker_record() {
        let mut rt = ExtApiRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn ext_api_rate_tracker_prune() {
        let mut rt = ExtApiRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn ext_api_validator_valid() {
        let v = ExtApiValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn ext_api_validator_errors() {
        let mut v = ExtApiValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn ext_api_validator_clear() {
        let mut v = ExtApiValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn ext_api_validator_merge() {
        let mut v1 = ExtApiValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = ExtApiValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn ext_api_rate_tracker_clear() {
        let mut rt = ExtApiRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for ext_api
    #[test]
    fn xa_ext_api_ring_new() {
        let rb = super::XaExtApiRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_api_ring_push_len() {
        let mut rb = super::XaExtApiRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_api_ring_wrap() {
        let mut rb = super::XaExtApiRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_api_ring_mean_empty() {
        let rb = super::XaExtApiRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_api_ring_mean_values() {
        let mut rb = super::XaExtApiRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_api_ring_min_max() {
        let mut rb = super::XaExtApiRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_api_ring_iter() {
        let mut rb = super::XaExtApiRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_api_counter_new() {
        let c = super::XaExtApiCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_api_counter_inc() {
        let mut c = super::XaExtApiCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_api_counter_inc_by() {
        let mut c = super::XaExtApiCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_api_counter_reset() {
        let mut c = super::XaExtApiCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_api_counter_clear() {
        let mut c = super::XaExtApiCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_api_counter_default() {
        let c = super::XaExtApiCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 49 ----

    #[test]
    fn xc_49_pool_new_empty() {
        let pool: super::Xc49Pool<i32> = super::Xc49Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_49_pool_release_acquire() {
        let mut pool = super::Xc49Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_49_pool_acquire_empty() {
        let mut pool: super::Xc49Pool<i32> = super::Xc49Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_49_pool_full() {
        let mut pool = super::Xc49Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_49_pool_drain() {
        let mut pool = super::Xc49Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_49_pool_stats() {
        let mut pool = super::Xc49Pool::new(8);
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
    fn xc_49_pool_clear() {
        let mut pool = super::Xc49Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_49_pool_shrink() {
        let mut pool = super::Xc49Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_49_pool_default() {
        let pool: super::Xc49Pool<String> = super::Xc49Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_49_pool_extend() {
        let mut pool = super::Xc49Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_49_pool_retain() {
        let mut pool = super::Xc49Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_49_scheduler_round_robin() {
        let mut sched = super::Xc49Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_49_scheduler_empty() {
        let mut sched = super::Xc49Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_49_scheduler_reset() {
        let mut sched = super::Xc49Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_49_scheduler_add_remove() {
        let mut sched = super::Xc49Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_49_scheduler_targets() {
        let sched = super::Xc49Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_49_hash_empty() {
        assert_eq!(super::xc_49_hash(b""), 5381);
    }

    #[test]
    fn xc_49_hash_data() {
        let h = super::xc_49_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_49_hash(b"hello"), h);
    }

    #[test]
    fn xc_49_reverse_str() {
        assert_eq!(super::xc_49_reverse("abc"), "cba");
        assert_eq!(super::xc_49_reverse(""), "");
    }


    // --- xd_48 deepening tests ---

    #[test]
    fn xd_48_sm_initial_state() {
        let sm = Xd48StateMachine::new();
        assert_eq!(sm.current_state(), Xd48State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_48_sm_valid_idle_to_running() {
        let mut sm = Xd48StateMachine::new();
        assert!(sm.transition(Xd48State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd48State::Running);
    }

    #[test]
    fn xd_48_sm_valid_running_to_paused() {
        let mut sm = Xd48StateMachine::new();
        sm.transition(Xd48State::Running).unwrap();
        assert!(sm.transition(Xd48State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd48State::Paused);
    }

    #[test]
    fn xd_48_sm_valid_running_to_done() {
        let mut sm = Xd48StateMachine::new();
        sm.transition(Xd48State::Running).unwrap();
        assert!(sm.transition(Xd48State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd48State::Done);
    }

    #[test]
    fn xd_48_sm_valid_paused_to_running() {
        let mut sm = Xd48StateMachine::new();
        sm.transition(Xd48State::Running).unwrap();
        sm.transition(Xd48State::Paused).unwrap();
        assert!(sm.transition(Xd48State::Running).is_ok());
    }

    #[test]
    fn xd_48_sm_valid_done_to_idle() {
        let mut sm = Xd48StateMachine::new();
        sm.transition(Xd48State::Running).unwrap();
        sm.transition(Xd48State::Done).unwrap();
        assert!(sm.transition(Xd48State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd48State::Idle);
    }

    #[test]
    fn xd_48_sm_invalid_idle_to_done() {
        let mut sm = Xd48StateMachine::new();
        assert!(sm.transition(Xd48State::Done).is_err());
    }

    #[test]
    fn xd_48_sm_invalid_idle_to_paused() {
        let mut sm = Xd48StateMachine::new();
        assert!(sm.transition(Xd48State::Paused).is_err());
    }

    #[test]
    fn xd_48_sm_history_tracking() {
        let mut sm = Xd48StateMachine::new();
        sm.transition(Xd48State::Running).unwrap();
        sm.transition(Xd48State::Paused).unwrap();
        sm.transition(Xd48State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd48State::Idle);
        assert_eq!(sm.history()[0].to, Xd48State::Running);
        assert_eq!(sm.history()[1].from, Xd48State::Running);
        assert_eq!(sm.history()[2].to, Xd48State::Done);
    }

    #[test]
    fn xd_48_sm_serialize_deserialize() {
        let mut sm = Xd48StateMachine::new();
        sm.transition(Xd48State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd48StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd48State::Running));
    }

    #[test]
    fn xd_48_sm_deserialize_invalid() {
        assert_eq!(Xd48StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_48_sm_reset() {
        let mut sm = Xd48StateMachine::new();
        sm.transition(Xd48State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd48State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_48_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd48EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd48Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_48_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd48EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd48Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd48Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_48_bus_unsubscribe() {
        let mut bus = Xd48EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_48_event_kind_and_payload() {
        let e = Xd48Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd48Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_48_bus_clear_history() {
        let mut bus = Xd48EventBus::new();
        bus.publish(Xd48Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_48_sm_step_counter_increments() {
        let mut sm = Xd48StateMachine::new();
        sm.transition(Xd48State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd48State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }

}
