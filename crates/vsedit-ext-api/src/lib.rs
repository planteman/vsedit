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

}
