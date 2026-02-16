//! Extension activation event handling.

use std::fmt;
use std::collections::{HashMap, HashSet, VecDeque};

/// Activation events that trigger extension loading.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActivationEvent {
    /// Activate on startup (always).
    Star,
    /// Activate on language.
    OnLanguage(String),
    /// Activate on command.
    OnCommand(String),
    /// Activate when a file with pattern is opened.
    OnFileSystem(String),
    /// Activate on view.
    OnView(String),
    /// Activate on URI scheme.
    OnUri(String),
    /// Activate on workspace contains.
    WorkspaceContains(String),
    /// Activate on debug.
    OnDebug,
    /// Activate on authentication.
    OnAuthenticationRequest(String),
    /// Activate on start finished.
    OnStartupFinished,
}

/// Parse activation event strings from package.json.
pub fn parse_activation_event(event: &str) -> Option<ActivationEvent> {
    if event == "*" { return Some(ActivationEvent::Star); }
    if event == "onStartupFinished" { return Some(ActivationEvent::OnStartupFinished); }
    if event == "onDebug" { return Some(ActivationEvent::OnDebug); }
    if let Some(lang) = event.strip_prefix("onLanguage:") {
        return Some(ActivationEvent::OnLanguage(lang.to_string()));
    }
    if let Some(cmd) = event.strip_prefix("onCommand:") {
        return Some(ActivationEvent::OnCommand(cmd.to_string()));
    }
    if let Some(fs) = event.strip_prefix("onFileSystem:") {
        return Some(ActivationEvent::OnFileSystem(fs.to_string()));
    }
    if let Some(view) = event.strip_prefix("onView:") {
        return Some(ActivationEvent::OnView(view.to_string()));
    }
    if let Some(uri) = event.strip_prefix("onUri:") {
        return Some(ActivationEvent::OnUri(uri.to_string()));
    }
    if let Some(glob) = event.strip_prefix("workspaceContains:") {
        return Some(ActivationEvent::WorkspaceContains(glob.to_string()));
    }
    if let Some(provider) = event.strip_prefix("onAuthenticationRequest:") {
        return Some(ActivationEvent::OnAuthenticationRequest(provider.to_string()));
    }
    None
}

/// Check if an activation event matches a trigger.
pub fn matches_trigger(event: &ActivationEvent, trigger: &str, value: &str) -> bool {
    match (event, trigger) {
        (ActivationEvent::Star, _) => true,
        (ActivationEvent::OnLanguage(lang), "onLanguage") => lang == value,
        (ActivationEvent::OnCommand(cmd), "onCommand") => cmd == value,
        (ActivationEvent::OnView(view), "onView") => view == value,
        (ActivationEvent::OnStartupFinished, "onStartupFinished") => true,
        _ => false,
    }
}

/// Checks whether a set of conditions satisfy an activation event.
pub struct ActivationEventMatcher {
    /// Currently open languages.
    pub open_languages: HashSet<String>,
    /// Whether startup has finished.
    pub startup_finished: bool,
    /// Currently open file URI schemes.
    pub open_schemes: HashSet<String>,
    /// Files present in the workspace root.
    pub workspace_files: HashSet<String>,
}

impl ActivationEventMatcher {
    pub fn new() -> Self {
        Self {
            open_languages: HashSet::new(),
            startup_finished: false,
            open_schemes: HashSet::new(),
            workspace_files: HashSet::new(),
        }
    }

    /// Check whether the given activation event's conditions are currently met.
    pub fn should_activate(&self, event: &ActivationEvent) -> bool {
        match event {
            ActivationEvent::Star => true,
            ActivationEvent::OnLanguage(lang) => self.open_languages.contains(lang),
            ActivationEvent::OnStartupFinished => self.startup_finished,
            ActivationEvent::OnFileSystem(scheme) => self.open_schemes.contains(scheme),
            ActivationEvent::WorkspaceContains(pattern) => {
                self.workspace_files.iter().any(|f| f.contains(pattern.as_str()))
            }
            ActivationEvent::OnDebug => false,
            ActivationEvent::OnCommand(_) => false,
            ActivationEvent::OnView(_) => false,
            ActivationEvent::OnUri(_) => false,
            ActivationEvent::OnAuthenticationRequest(_) => false,
        }
    }
}

impl Default for ActivationEventMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks pending extension activations.
pub struct ExtensionActivationQueue {
    /// Extension ID → list of activation events.
    registry: HashMap<String, Vec<ActivationEvent>>,
    /// Extensions already activated.
    activated: HashSet<String>,
    /// Queue of extension IDs pending activation.
    pending: VecDeque<String>,
}

impl ExtensionActivationQueue {
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
            activated: HashSet::new(),
            pending: VecDeque::new(),
        }
    }

    /// Register an extension with its activation events.
    pub fn register(&mut self, extension_id: String, events: Vec<ActivationEvent>) {
        self.registry.insert(extension_id, events);
    }

    /// Evaluate all registered extensions against the current matcher state.
    /// Returns newly queued extension IDs.
    pub fn evaluate(&mut self, matcher: &ActivationEventMatcher) -> Vec<String> {
        let mut newly_queued = Vec::new();
        for (ext_id, events) in &self.registry {
            if self.activated.contains(ext_id) {
                continue;
            }
            if events.iter().any(|e| matcher.should_activate(e)) {
                if !self.pending.contains(ext_id) {
                    self.pending.push_back(ext_id.clone());
                    newly_queued.push(ext_id.clone());
                }
            }
        }
        newly_queued
    }

    /// Pop the next extension to activate.
    pub fn pop_pending(&mut self) -> Option<String> {
        if let Some(ext_id) = self.pending.pop_front() {
            self.activated.insert(ext_id.clone());
            Some(ext_id)
        } else {
            None
        }
    }

    pub fn is_activated(&self, extension_id: &str) -> bool {
        self.activated.contains(extension_id)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for ExtensionActivationQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Additional activation utilities
// ---------------------------------------------------------------------------

impl std::fmt::Display for ActivationEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Star => write!(f, "*"),
            Self::OnLanguage(l) => write!(f, "onLanguage:{l}"),
            Self::OnCommand(c) => write!(f, "onCommand:{c}"),
            Self::OnFileSystem(s) => write!(f, "onFileSystem:{s}"),
            Self::OnView(v) => write!(f, "onView:{v}"),
            Self::OnUri(u) => write!(f, "onUri:{u}"),
            Self::WorkspaceContains(g) => write!(f, "workspaceContains:{g}"),
            Self::OnDebug => write!(f, "onDebug"),
            Self::OnAuthenticationRequest(p) => write!(f, "onAuthenticationRequest:{p}"),
            Self::OnStartupFinished => write!(f, "onStartupFinished"),
        }
    }
}

/// Serialize an activation event back to the string form used in package.json.
pub fn activation_event_to_string(event: &ActivationEvent) -> String {
    format!("{event}")
}

/// Parse a list of activation event strings, skipping any unrecognized ones.
pub fn parse_activation_events(events: &[&str]) -> Vec<ActivationEvent> {
    events.iter().filter_map(|e| parse_activation_event(e)).collect()
}

/// Validate that an activation event string is well-formed.
pub fn validate_activation_event(event: &str) -> Result<ActivationEvent, String> {
    parse_activation_event(event).ok_or_else(|| format!("unknown activation event: {event}"))
}

/// An activation dependency graph: extensions can depend on other extensions being activated first.
#[derive(Debug, Clone, Default)]
pub struct ActivationDependencyGraph {
    /// Extension ID → set of extension IDs it depends on.
    deps: HashMap<String, HashSet<String>>,
}

impl ActivationDependencyGraph {
    pub fn new() -> Self {
        Self { deps: HashMap::new() }
    }

    /// Add a dependency: `ext_id` depends on `depends_on` being activated first.
    pub fn add_dependency(&mut self, ext_id: impl Into<String>, depends_on: impl Into<String>) {
        self.deps.entry(ext_id.into()).or_default().insert(depends_on.into());
    }

    /// Get the set of dependencies for an extension.
    pub fn dependencies_of(&self, ext_id: &str) -> HashSet<String> {
        self.deps.get(ext_id).cloned().unwrap_or_default()
    }

    /// Check whether all dependencies of `ext_id` are in the `activated` set.
    pub fn can_activate(&self, ext_id: &str, activated: &HashSet<String>) -> bool {
        match self.deps.get(ext_id) {
            None => true,
            Some(deps) => deps.iter().all(|d| activated.contains(d)),
        }
    }

    /// Return all extensions that have no unsatisfied dependencies given the activated set.
    pub fn ready_to_activate(&self, all_ids: &[String], activated: &HashSet<String>) -> Vec<String> {
        all_ids
            .iter()
            .filter(|id| !activated.contains(id.as_str()) && self.can_activate(id, activated))
            .cloned()
            .collect()
    }

    /// Produce a topological ordering of all extensions, or return an error if there is a cycle.
    pub fn topological_sort(&self, all_ids: &[String]) -> Result<Vec<String>, String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for id in all_ids {
            in_degree.entry(id.clone()).or_insert(0);
        }
        for (id, deps) in &self.deps {
            for dep in deps {
                if all_ids.contains(dep) {
                    *in_degree.entry(id.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(id, _)| id.clone())
            .collect();
        queue.make_contiguous().sort();

        let mut result = Vec::new();
        while let Some(id) = queue.pop_front() {
            result.push(id.clone());
            for (ext_id, deps) in &self.deps {
                if deps.contains(&id) {
                    if let Some(deg) = in_degree.get_mut(ext_id) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push_back(ext_id.clone());
                        }
                    }
                }
            }
        }

        if result.len() == all_ids.len() {
            Ok(result)
        } else {
            Err("cycle detected in activation dependencies".to_string())
        }
    }

    /// Number of registered dependency relationships.
    pub fn total_edges(&self) -> usize {
        self.deps.values().map(|s| s.len()).sum()
    }
}

impl ActivationEventMatcher {
    /// Register that a language file was opened.
    pub fn open_language(&mut self, lang: impl Into<String>) {
        self.open_languages.insert(lang.into());
    }

    /// Register that a URI scheme is available.
    pub fn add_scheme(&mut self, scheme: impl Into<String>) {
        self.open_schemes.insert(scheme.into());
    }

    /// Register that a file exists in the workspace.
    pub fn add_workspace_file(&mut self, file: impl Into<String>) {
        self.workspace_files.insert(file.into());
    }

    /// Mark startup as finished.
    pub fn finish_startup(&mut self) {
        self.startup_finished = true;
    }

    /// Collect all activation events that currently match.
    pub fn matching_events(&self, events: &[ActivationEvent]) -> Vec<ActivationEvent> {
        events.iter().filter(|e| self.should_activate(e)).cloned().collect()
    }
}

impl ExtensionActivationQueue {
    /// Register multiple extensions at once.
    pub fn register_many(&mut self, entries: Vec<(String, Vec<ActivationEvent>)>) {
        for (id, events) in entries {
            self.register(id, events);
        }
    }

    /// Drain all pending activations, returning them in order.
    pub fn drain_pending(&mut self) -> Vec<String> {
        let mut result = Vec::new();
        while let Some(id) = self.pop_pending() {
            result.push(id);
        }
        result
    }

    /// Number of activated extensions.
    pub fn activated_count(&self) -> usize {
        self.activated.len()
    }

    /// Number of registered extensions.
    pub fn registered_count(&self) -> usize {
        self.registry.len()
    }

    /// Get the activation events registered for an extension.
    pub fn events_for(&self, ext_id: &str) -> Option<&Vec<ActivationEvent>> {
        self.registry.get(ext_id)
    }

    /// Reset the queue: clear activated and pending, but keep the registry.
    pub fn reset(&mut self) {
        self.activated.clear();
        self.pending.clear();
    }
}

/// Accumulated statistics for ext-activation operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtActivationStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtActivationStats {
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
    pub fn merge(&mut self, other: &ExtActivationStats) {
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

impl Default for ExtActivationStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtActivationStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtActivationStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-activation.
#[derive(Debug, Clone)]
pub struct ExtActivationValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtActivationValidator {
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

impl Default for ExtActivationValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// A record of a single extension activation's performance.
#[derive(Debug, Clone)]
pub struct ActivationPerformanceRecord {
    /// The extension that was activated.
    pub extension_id: String,
    /// Timestamp (ms) when activation started.
    pub activation_start_ms: u64,
    /// Timestamp (ms) when activation ended, if finished.
    pub activation_end_ms: Option<u64>,
    /// The event that triggered activation.
    pub event: ActivationEvent,
}

impl ActivationPerformanceRecord {
    /// Create a new performance record for an activation that has just started.
    pub fn new(ext_id: &str, event: ActivationEvent, start_ms: u64) -> Self {
        Self {
            extension_id: ext_id.to_string(),
            activation_start_ms: start_ms,
            activation_end_ms: None,
            event,
        }
    }

    /// Mark the activation as finished at the given timestamp.
    pub fn finish(&mut self, end_ms: u64) {
        self.activation_end_ms = Some(end_ms);
    }

    /// Duration in milliseconds, or `None` if activation has not finished.
    pub fn duration_ms(&self) -> Option<u64> {
        self.activation_end_ms.map(|end| end.saturating_sub(self.activation_start_ms))
    }

    /// Returns `true` if the activation duration exceeds `threshold_ms`.
    pub fn is_slow(&self, threshold_ms: u64) -> bool {
        self.duration_ms().map_or(false, |d| d > threshold_ms)
    }
}

impl fmt::Display for ActivationPerformanceRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.duration_ms() {
            Some(ms) => write!(f, "{} activated in {}ms", self.extension_id, ms),
            None => write!(f, "{} activation in progress", self.extension_id),
        }
    }
}

/// Convenience constructor for [`ActivationPerformanceRecord`].
pub fn activation_performance_record(
    ext_id: &str,
    event: ActivationEvent,
    start_ms: u64,
) -> ActivationPerformanceRecord {
    ActivationPerformanceRecord::new(ext_id, event, start_ms)
}

/// Tracks activation performance for multiple extensions.
#[derive(Debug, Default)]
pub struct ActivationPerformanceTracker {
    records: Vec<ActivationPerformanceRecord>,
}

impl ActivationPerformanceTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    /// Begin tracking an activation and return the record index.
    pub fn start_activation(&mut self, ext_id: &str, event: ActivationEvent, start_ms: u64) -> usize {
        let idx = self.records.len();
        self.records.push(ActivationPerformanceRecord::new(ext_id, event, start_ms));
        idx
    }

    /// Mark a previously started activation as finished.
    pub fn end_activation(&mut self, index: usize, end_ms: u64) {
        if let Some(record) = self.records.get_mut(index) {
            record.finish(end_ms);
        }
    }

    /// Return references to all records whose activation duration exceeds `threshold_ms`.
    pub fn slow_activations(&self, threshold_ms: u64) -> Vec<&ActivationPerformanceRecord> {
        self.records.iter().filter(|r| r.is_slow(threshold_ms)).collect()
    }

    /// Average activation time in milliseconds across all finished records.
    pub fn average_activation_ms(&self) -> Option<f64> {
        let finished: Vec<u64> = self.records.iter().filter_map(|r| r.duration_ms()).collect();
        if finished.is_empty() {
            return None;
        }
        let total: u64 = finished.iter().sum();
        Some(total as f64 / finished.len() as f64)
    }

    /// Total number of tracked activations (finished or not).
    pub fn total_count(&self) -> usize {
        self.records.len()
    }
}

impl ActivationDependencyGraph {
    /// Check whether the dependency graph contains a cycle among the given extension IDs.
    pub fn has_cycle(&self, all_ids: &[String]) -> bool {
        self.topological_sort(all_ids).is_err()
    }

    /// Reverse lookup: return the IDs of extensions that depend on `ext_id`.
    pub fn dependents_of(&self, ext_id: &str) -> Vec<String> {
        self.deps
            .iter()
            .filter(|(_, deps)| deps.contains(ext_id))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Return extension IDs that have no dependencies at all.
    pub fn independent_extensions(&self, all_ids: &[String]) -> Vec<String> {
        all_ids
            .iter()
            .filter(|id| self.deps.get(id.as_str()).map_or(true, HashSet::is_empty))
            .cloned()
            .collect()
    }
}

/// A filter for selecting activation events by type.
#[derive(Debug, Default)]
pub struct ActivationEventFilter {
    languages: HashSet<String>,
    commands: HashSet<String>,
}

impl ActivationEventFilter {
    /// Create a new empty filter (matches nothing).
    pub fn new() -> Self {
        Self::default()
    }

    /// Include events that activate on the given language.
    pub fn include_language(mut self, lang: &str) -> Self {
        self.languages.insert(lang.to_string());
        self
    }

    /// Include events that activate on the given command.
    pub fn include_command(mut self, cmd: &str) -> Self {
        self.commands.insert(cmd.to_string());
        self
    }

    /// Check whether a single event matches the filter.
    pub fn matches(&self, event: &ActivationEvent) -> bool {
        match event {
            ActivationEvent::OnLanguage(lang) => self.languages.contains(lang),
            ActivationEvent::OnCommand(cmd) => self.commands.contains(cmd),
            _ => false,
        }
    }

    /// Return only the events that match this filter.
    pub fn filter<'a>(&self, events: &'a [ActivationEvent]) -> Vec<&'a ActivationEvent> {
        events.iter().filter(|e| self.matches(e)).collect()
    }
}

// ---------------------------------------------------------------------------
// ActivationPolicy
// ---------------------------------------------------------------------------

/// Defines when and how an extension should be activated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationPolicy {
    /// Activate immediately on startup.
    Eager,
    /// Activate only when explicitly needed (default).
    Lazy,
    /// Activate on-demand when a specific event fires.
    OnDemand(ActivationEvent),
    /// Never activate automatically; requires manual activation.
    Manual,
}

impl ActivationPolicy {
    /// Returns `true` if this policy activates eagerly on startup.
    pub fn is_eager(&self) -> bool {
        matches!(self, Self::Eager)
    }

    /// Returns `true` if activation requires an explicit trigger.
    pub fn requires_trigger(&self) -> bool {
        matches!(self, Self::OnDemand(_) | Self::Manual)
    }

    /// Returns the triggering event, if this policy is on-demand.
    pub fn trigger_event(&self) -> Option<&ActivationEvent> {
        match self {
            Self::OnDemand(event) => Some(event),
            _ => None,
        }
    }
}

impl fmt::Display for ActivationPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eager => write!(f, "eager"),
            Self::Lazy => write!(f, "lazy"),
            Self::OnDemand(event) => write!(f, "on-demand({event})"),
            Self::Manual => write!(f, "manual"),
        }
    }
}

// ---------------------------------------------------------------------------
// ActivationPriorityQueue
// ---------------------------------------------------------------------------

/// A priority-based activation queue. Lower priority values are activated first.
#[derive(Debug)]
pub struct ActivationPriorityQueue {
    entries: Vec<(u32, String, ActivationEvent)>,
}

impl ActivationPriorityQueue {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Enqueue an extension for activation with the given priority.
    pub fn enqueue(&mut self, priority: u32, ext_id: &str, event: ActivationEvent) {
        self.entries.push((priority, ext_id.to_string(), event));
        self.entries.sort_by_key(|(p, _, _)| *p);
    }

    /// Dequeue the highest-priority (lowest value) extension.
    pub fn dequeue(&mut self) -> Option<(u32, String, ActivationEvent)> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.entries.remove(0))
        }
    }

    /// Peek at the next extension without removing it.
    pub fn peek(&self) -> Option<&(u32, String, ActivationEvent)> {
        self.entries.first()
    }

    /// Number of pending activations.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove all entries for a given extension.
    pub fn remove_extension(&mut self, ext_id: &str) {
        self.entries.retain(|(_, id, _)| id != ext_id);
    }

    /// Drain all entries, returning them in priority order.
    pub fn drain_all(&mut self) -> Vec<(u32, String, ActivationEvent)> {
        std::mem::take(&mut self.entries)
    }
}

impl Default for ActivationPriorityQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ActivationCondition
// ---------------------------------------------------------------------------

/// A compound condition that must be satisfied before activation proceeds.
#[derive(Debug, Clone)]
pub struct ActivationCondition {
    required_events: Vec<ActivationEvent>,
    forbidden_events: Vec<ActivationEvent>,
    min_delay_ms: Option<u64>,
}

impl ActivationCondition {
    pub fn new() -> Self {
        Self {
            required_events: Vec::new(),
            forbidden_events: Vec::new(),
            min_delay_ms: None,
        }
    }

    /// Add a required event that must have fired before activation.
    pub fn require(mut self, event: ActivationEvent) -> Self {
        self.required_events.push(event);
        self
    }

    /// Add an event that must NOT have fired for activation to proceed.
    pub fn forbid(mut self, event: ActivationEvent) -> Self {
        self.forbidden_events.push(event);
        self
    }

    /// Set a minimum delay (ms) since the first required event fired.
    pub fn min_delay(mut self, ms: u64) -> Self {
        self.min_delay_ms = Some(ms);
        self
    }

    /// Evaluate the condition against a set of already-fired events and elapsed time.
    pub fn is_satisfied(&self, fired: &[ActivationEvent], elapsed_ms: u64) -> bool {
        for req in &self.required_events {
            if !fired.contains(req) {
                return false;
            }
        }
        for forbidden in &self.forbidden_events {
            if fired.contains(forbidden) {
                return false;
            }
        }
        if let Some(min) = self.min_delay_ms {
            if elapsed_ms < min {
                return false;
            }
        }
        true
    }

    /// Number of required events.
    pub fn required_count(&self) -> usize {
        self.required_events.len()
    }

    /// Number of forbidden events.
    pub fn forbidden_count(&self) -> usize {
        self.forbidden_events.len()
    }
}

impl Default for ActivationCondition {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ActivationPerformanceTracker percentile extension
// ---------------------------------------------------------------------------

impl ActivationPerformanceTracker {
    /// Compute the p-th percentile (0..=100) of activation durations.
    /// Returns `None` if there are no finished records.
    pub fn percentile_ms(&self, p: u8) -> Option<f64> {
        let p = p.min(100);
        let mut durations: Vec<u64> = self.records.iter().filter_map(|r| r.duration_ms()).collect();
        if durations.is_empty() {
            return None;
        }
        durations.sort_unstable();
        let rank = (p as f64 / 100.0) * (durations.len() as f64 - 1.0);
        let lower = rank.floor() as usize;
        let upper = rank.ceil() as usize;
        if lower == upper {
            Some(durations[lower] as f64)
        } else {
            let frac = rank - lower as f64;
            Some(durations[lower] as f64 * (1.0 - frac) + durations[upper] as f64 * frac)
        }
    }

    /// Median activation time in milliseconds.
    pub fn median_ms(&self) -> Option<f64> {
        self.percentile_ms(50)
    }

    /// 95th percentile activation time.
    pub fn p95_ms(&self) -> Option<f64> {
        self.percentile_ms(95)
    }

    /// Return references to all records.
    pub fn all_records(&self) -> &[ActivationPerformanceRecord] {
        &self.records
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_events() {
        assert_eq!(parse_activation_event("*"), Some(ActivationEvent::Star));
        assert_eq!(
            parse_activation_event("onLanguage:rust"),
            Some(ActivationEvent::OnLanguage("rust".into()))
        );
        assert_eq!(
            parse_activation_event("onCommand:workbench.action.files.save"),
            Some(ActivationEvent::OnCommand("workbench.action.files.save".into()))
        );
        assert_eq!(parse_activation_event("onStartupFinished"), Some(ActivationEvent::OnStartupFinished));
    }

    #[test]
    fn trigger_matching() {
        let event = ActivationEvent::OnLanguage("rust".into());
        assert!(matches_trigger(&event, "onLanguage", "rust"));
        assert!(!matches_trigger(&event, "onLanguage", "python"));
    }

    #[test]
    fn star_matches_everything() {
        let event = ActivationEvent::Star;
        assert!(matches_trigger(&event, "onLanguage", "anything"));
    }

    #[test]
    fn parse_all_event_types() {
        assert_eq!(parse_activation_event("onDebug"), Some(ActivationEvent::OnDebug));
        assert_eq!(
            parse_activation_event("onFileSystem:ftp"),
            Some(ActivationEvent::OnFileSystem("ftp".into()))
        );
        assert_eq!(
            parse_activation_event("onView:explorer"),
            Some(ActivationEvent::OnView("explorer".into()))
        );
        assert_eq!(
            parse_activation_event("onUri:vscode"),
            Some(ActivationEvent::OnUri("vscode".into()))
        );
        assert_eq!(
            parse_activation_event("workspaceContains:*.rs"),
            Some(ActivationEvent::WorkspaceContains("*.rs".into()))
        );
        assert_eq!(
            parse_activation_event("onAuthenticationRequest:github"),
            Some(ActivationEvent::OnAuthenticationRequest("github".into()))
        );
        assert_eq!(parse_activation_event("unknownEvent"), None);
    }

    #[test]
    fn matcher_language() {
        let mut m = ActivationEventMatcher::new();
        let event = ActivationEvent::OnLanguage("rust".into());
        assert!(!m.should_activate(&event));
        m.open_languages.insert("rust".into());
        assert!(m.should_activate(&event));
    }

    #[test]
    fn matcher_startup_finished() {
        let mut m = ActivationEventMatcher::new();
        let event = ActivationEvent::OnStartupFinished;
        assert!(!m.should_activate(&event));
        m.startup_finished = true;
        assert!(m.should_activate(&event));
    }

    #[test]
    fn activation_queue_basic() {
        let mut queue = ExtensionActivationQueue::new();
        queue.register("ext-a".into(), vec![ActivationEvent::Star]);
        queue.register(
            "ext-b".into(),
            vec![ActivationEvent::OnLanguage("rust".into())],
        );

        let matcher = ActivationEventMatcher::new();
        let queued = queue.evaluate(&matcher);
        // Star should activate immediately
        assert!(queued.contains(&"ext-a".to_string()));
        assert!(!queued.contains(&"ext-b".to_string()));

        let popped = queue.pop_pending().unwrap();
        assert_eq!(popped, "ext-a");
        assert!(queue.is_activated("ext-a"));
        assert!(!queue.is_activated("ext-b"));
    }

    #[test]
    fn activation_queue_no_double_activation() {
        let mut queue = ExtensionActivationQueue::new();
        queue.register("ext-a".into(), vec![ActivationEvent::Star]);

        let matcher = ActivationEventMatcher::new();
        queue.evaluate(&matcher);
        queue.pop_pending();

        // Re-evaluate should not re-queue
        let queued = queue.evaluate(&matcher);
        assert!(queued.is_empty());
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn matcher_workspace_contains() {
        let mut m = ActivationEventMatcher::new();
        m.workspace_files.insert("Cargo.toml".into());
        m.workspace_files.insert("src/main.rs".into());

        let event = ActivationEvent::WorkspaceContains("Cargo".into());
        assert!(m.should_activate(&event));

        let no_match = ActivationEvent::WorkspaceContains("package.json".into());
        assert!(!m.should_activate(&no_match));
    }

    #[test]
    fn activation_event_display_roundtrip() {
        let events = vec![
            ActivationEvent::Star,
            ActivationEvent::OnLanguage("rust".into()),
            ActivationEvent::OnCommand("save".into()),
            ActivationEvent::OnFileSystem("ftp".into()),
            ActivationEvent::OnView("explorer".into()),
            ActivationEvent::OnUri("vscode".into()),
            ActivationEvent::WorkspaceContains("*.rs".into()),
            ActivationEvent::OnDebug,
            ActivationEvent::OnAuthenticationRequest("github".into()),
            ActivationEvent::OnStartupFinished,
        ];
        for event in &events {
            let s = activation_event_to_string(event);
            let parsed = parse_activation_event(&s).unwrap();
            assert_eq!(&parsed, event);
        }
    }

    #[test]
    fn parse_activation_events_batch() {
        let input = vec!["onLanguage:rust", "bad", "onCommand:run", "*"];
        let result = parse_activation_events(&input);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn validate_activation_event_ok() {
        assert!(validate_activation_event("onLanguage:rust").is_ok());
    }

    #[test]
    fn validate_activation_event_err() {
        let err = validate_activation_event("nonsense").unwrap_err();
        assert!(err.contains("unknown"));
    }

    #[test]
    fn dependency_graph_basic() {
        let mut g = ActivationDependencyGraph::new();
        g.add_dependency("ext-b", "ext-a");
        let activated: HashSet<String> = HashSet::new();
        assert!(!g.can_activate("ext-b", &activated));
        let mut activated2: HashSet<String> = HashSet::new();
        activated2.insert("ext-a".into());
        assert!(g.can_activate("ext-b", &activated2));
    }

    #[test]
    fn dependency_graph_topological_sort() {
        let mut g = ActivationDependencyGraph::new();
        g.add_dependency("c", "b");
        g.add_dependency("b", "a");
        let ids = vec!["a".into(), "b".into(), "c".into()];
        let sorted = g.topological_sort(&ids).unwrap();
        assert_eq!(sorted, vec!["a", "b", "c"]);
    }

    #[test]
    fn dependency_graph_ready_to_activate() {
        let mut g = ActivationDependencyGraph::new();
        g.add_dependency("b", "a");
        g.add_dependency("c", "a");
        let ids = vec!["a".into(), "b".into(), "c".into()];
        let activated = HashSet::new();
        let ready = g.ready_to_activate(&ids, &activated);
        assert_eq!(ready, vec!["a".to_string()]);
    }

    #[test]
    fn dependency_graph_total_edges() {
        let mut g = ActivationDependencyGraph::new();
        g.add_dependency("b", "a");
        g.add_dependency("c", "a");
        g.add_dependency("c", "b");
        assert_eq!(g.total_edges(), 3);
    }

    #[test]
    fn matcher_convenience_methods() {
        let mut m = ActivationEventMatcher::new();
        m.open_language("python");
        m.add_scheme("file");
        m.add_workspace_file("Makefile");
        m.finish_startup();
        assert!(m.should_activate(&ActivationEvent::OnLanguage("python".into())));
        assert!(m.should_activate(&ActivationEvent::OnFileSystem("file".into())));
        assert!(m.should_activate(&ActivationEvent::WorkspaceContains("Makefile".into())));
        assert!(m.should_activate(&ActivationEvent::OnStartupFinished));
    }

    #[test]
    fn matcher_matching_events_filter() {
        let mut m = ActivationEventMatcher::new();
        m.open_language("rust");
        let events = vec![
            ActivationEvent::OnLanguage("rust".into()),
            ActivationEvent::OnLanguage("python".into()),
            ActivationEvent::Star,
        ];
        let matched = m.matching_events(&events);
        assert_eq!(matched.len(), 2);
    }

    #[test]
    fn queue_register_many() {
        let mut q = ExtensionActivationQueue::new();
        q.register_many(vec![
            ("a".into(), vec![ActivationEvent::Star]),
            ("b".into(), vec![ActivationEvent::OnDebug]),
        ]);
        assert_eq!(q.registered_count(), 2);
    }

    #[test]
    fn queue_drain_pending() {
        let mut q = ExtensionActivationQueue::new();
        q.register("a".into(), vec![ActivationEvent::Star]);
        q.register("b".into(), vec![ActivationEvent::Star]);
        let m = ActivationEventMatcher::new();
        q.evaluate(&m);
        let drained = q.drain_pending();
        assert_eq!(drained.len(), 2);
        assert_eq!(q.activated_count(), 2);
        assert_eq!(q.pending_count(), 0);
    }

    #[test]
    fn queue_reset() {
        let mut q = ExtensionActivationQueue::new();
        q.register("a".into(), vec![ActivationEvent::Star]);
        let m = ActivationEventMatcher::new();
        q.evaluate(&m);
        q.drain_pending();
        assert_eq!(q.activated_count(), 1);
        q.reset();
        assert_eq!(q.activated_count(), 0);
        assert_eq!(q.pending_count(), 0);
    }

    #[test]
    fn queue_events_for() {
        let mut q = ExtensionActivationQueue::new();
        q.register("x".into(), vec![ActivationEvent::OnDebug]);
        let events = q.events_for("x").unwrap();
        assert_eq!(events.len(), 1);
        assert!(q.events_for("y").is_none());
    }

    #[test]
    fn ext_activation_stats_new_defaults() {
        let stats = ExtActivationStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_activation_stats_record_success() {
        let mut stats = ExtActivationStats::new();
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
    fn ext_activation_stats_record_failure() {
        let mut stats = ExtActivationStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_activation_stats_reset() {
        let mut stats = ExtActivationStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_activation_stats_merge() {
        let mut a = ExtActivationStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtActivationStats::new();
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
    fn ext_activation_stats_display() {
        let mut stats = ExtActivationStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_activation_stats_default() {
        let stats = ExtActivationStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_activation_validator_accepts_valid_name() {
        let v = ExtActivationValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_activation_validator_rejects_empty() {
        let v = ExtActivationValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_activation_validator_rejects_too_long() {
        let v = ExtActivationValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_activation_validator_forbidden_prefix() {
        let v = ExtActivationValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_activation_validator_allowed_chars() {
        let v = ExtActivationValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_activation_validator_range() {
        let v = ExtActivationValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_activation_sanitize_removes_control() {
        let result = ExtActivationValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_activation_truncate_short_string() {
        assert_eq!(ExtActivationValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_activation_truncate_long_string() {
        let result = ExtActivationValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_activation_is_ascii_printable() {
        assert!(ExtActivationValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtActivationValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn perf_record_creation_and_duration() {
        let mut rec = ActivationPerformanceRecord::new(
            "ext.foo",
            ActivationEvent::OnLanguage("rust".into()),
            100,
        );
        assert_eq!(rec.duration_ms(), None);
        rec.finish(250);
        assert_eq!(rec.duration_ms(), Some(150));
        assert_eq!(rec.extension_id, "ext.foo");
    }

    #[test]
    fn perf_record_is_slow() {
        let mut rec = activation_performance_record(
            "ext.slow",
            ActivationEvent::Star,
            0,
        );
        rec.finish(500);
        assert!(rec.is_slow(100));
        assert!(!rec.is_slow(500));
        assert!(!rec.is_slow(600));
    }

    #[test]
    fn perf_record_display() {
        let mut rec = ActivationPerformanceRecord::new("ext.a", ActivationEvent::Star, 10);
        assert_eq!(format!("{rec}"), "ext.a activation in progress");
        rec.finish(30);
        assert_eq!(format!("{rec}"), "ext.a activated in 20ms");
    }

    #[test]
    fn perf_tracker_tracks_multiple() {
        let mut tracker = ActivationPerformanceTracker::new();
        let i0 = tracker.start_activation("a", ActivationEvent::Star, 0);
        let i1 = tracker.start_activation("b", ActivationEvent::OnCommand("cmd".into()), 10);
        assert_eq!(tracker.total_count(), 2);
        tracker.end_activation(i0, 50);
        tracker.end_activation(i1, 110);
        assert_eq!(tracker.average_activation_ms(), Some(75.0));
    }

    #[test]
    fn perf_tracker_slow_activations_filter() {
        let mut tracker = ActivationPerformanceTracker::new();
        let i0 = tracker.start_activation("fast", ActivationEvent::Star, 0);
        let i1 = tracker.start_activation("slow", ActivationEvent::Star, 0);
        tracker.end_activation(i0, 10);
        tracker.end_activation(i1, 500);
        let slow = tracker.slow_activations(100);
        assert_eq!(slow.len(), 1);
        assert_eq!(slow[0].extension_id, "slow");
    }

    #[test]
    fn dep_graph_has_cycle_detects_cycle() {
        let mut g = ActivationDependencyGraph::new();
        g.add_dependency("a", "b");
        g.add_dependency("b", "a");
        let ids = vec!["a".to_string(), "b".to_string()];
        assert!(g.has_cycle(&ids));
    }

    #[test]
    fn dep_graph_has_cycle_no_cycle() {
        let mut g = ActivationDependencyGraph::new();
        g.add_dependency("a", "b");
        let ids = vec!["a".to_string(), "b".to_string()];
        assert!(!g.has_cycle(&ids));
    }

    #[test]
    fn dep_graph_dependents_of_reverse() {
        let mut g = ActivationDependencyGraph::new();
        g.add_dependency("x", "base");
        g.add_dependency("y", "base");
        g.add_dependency("z", "other");
        let mut deps = g.dependents_of("base");
        deps.sort();
        assert_eq!(deps, vec!["x", "y"]);
        assert!(g.dependents_of("nonexistent").is_empty());
    }

    #[test]
    fn dep_graph_independent_extensions() {
        let mut g = ActivationDependencyGraph::new();
        g.add_dependency("a", "b");
        let ids = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let indep = g.independent_extensions(&ids);
        assert_eq!(indep, vec!["b".to_string(), "c".to_string()]);
    }

    #[test]
    fn event_filter_matches_language() {
        let filter = ActivationEventFilter::new()
            .include_language("rust")
            .include_language("python");
        assert!(filter.matches(&ActivationEvent::OnLanguage("rust".into())));
        assert!(filter.matches(&ActivationEvent::OnLanguage("python".into())));
        assert!(!filter.matches(&ActivationEvent::OnLanguage("go".into())));
        assert!(!filter.matches(&ActivationEvent::Star));
    }

    #[test]
    fn event_filter_returns_subset() {
        let filter = ActivationEventFilter::new()
            .include_language("rust")
            .include_command("myCmd");
        let events = vec![
            ActivationEvent::OnLanguage("rust".into()),
            ActivationEvent::OnLanguage("go".into()),
            ActivationEvent::OnCommand("myCmd".into()),
            ActivationEvent::Star,
        ];
        let matched = filter.filter(&events);
        assert_eq!(matched.len(), 2);
        assert_eq!(matched[0], &ActivationEvent::OnLanguage("rust".into()));
        assert_eq!(matched[1], &ActivationEvent::OnCommand("myCmd".into()));
    }

    #[test]
    fn activation_policy_properties() {
        let eager = ActivationPolicy::Eager;
        assert!(eager.is_eager());
        assert!(!eager.requires_trigger());
        assert!(eager.trigger_event().is_none());

        let lazy = ActivationPolicy::Lazy;
        assert!(!lazy.is_eager());

        let on_demand = ActivationPolicy::OnDemand(ActivationEvent::OnLanguage("rust".into()));
        assert!(on_demand.requires_trigger());
        assert_eq!(
            on_demand.trigger_event(),
            Some(&ActivationEvent::OnLanguage("rust".into()))
        );

        let manual = ActivationPolicy::Manual;
        assert!(manual.requires_trigger());
        assert!(manual.trigger_event().is_none());
    }

    #[test]
    fn activation_policy_display() {
        assert_eq!(ActivationPolicy::Eager.to_string(), "eager");
        assert_eq!(ActivationPolicy::Lazy.to_string(), "lazy");
        assert_eq!(ActivationPolicy::Manual.to_string(), "manual");
    }

    #[test]
    fn priority_queue_ordering() {
        let mut q = ActivationPriorityQueue::new();
        q.enqueue(10, "ext-c", ActivationEvent::Star);
        q.enqueue(1, "ext-a", ActivationEvent::OnDebug);
        q.enqueue(5, "ext-b", ActivationEvent::OnStartupFinished);

        assert_eq!(q.len(), 3);
        assert!(!q.is_empty());

        let (p, id, _) = q.dequeue().unwrap();
        assert_eq!(p, 1);
        assert_eq!(id, "ext-a");

        let (p, id, _) = q.dequeue().unwrap();
        assert_eq!(p, 5);
        assert_eq!(id, "ext-b");
    }

    #[test]
    fn priority_queue_remove_extension() {
        let mut q = ActivationPriorityQueue::new();
        q.enqueue(1, "ext-a", ActivationEvent::Star);
        q.enqueue(2, "ext-b", ActivationEvent::OnDebug);
        q.enqueue(3, "ext-a", ActivationEvent::OnStartupFinished);
        assert_eq!(q.len(), 3);

        q.remove_extension("ext-a");
        assert_eq!(q.len(), 1);
        assert_eq!(q.peek().unwrap().1, "ext-b");
    }

    #[test]
    fn activation_condition_evaluation() {
        let cond = ActivationCondition::new()
            .require(ActivationEvent::OnLanguage("rust".into()))
            .forbid(ActivationEvent::OnDebug)
            .min_delay(100);

        let fired = vec![ActivationEvent::OnLanguage("rust".into())];
        assert!(!cond.is_satisfied(&fired, 50)); // too early
        assert!(cond.is_satisfied(&fired, 100)); // ok

        let fired_with_debug = vec![
            ActivationEvent::OnLanguage("rust".into()),
            ActivationEvent::OnDebug,
        ];
        assert!(!cond.is_satisfied(&fired_with_debug, 200)); // forbidden

        assert_eq!(cond.required_count(), 1);
        assert_eq!(cond.forbidden_count(), 1);
    }

    #[test]
    fn activation_condition_empty_satisfied() {
        let cond = ActivationCondition::new();
        assert!(cond.is_satisfied(&[], 0));
    }

    #[test]
    fn performance_tracker_percentiles() {
        let mut tracker = ActivationPerformanceTracker::new();
        for i in 0..10 {
            let idx = tracker.start_activation(
                &format!("ext-{i}"),
                ActivationEvent::Star,
                i * 100,
            );
            tracker.end_activation(idx, i * 100 + (i + 1) * 10);
        }

        let median = tracker.median_ms().unwrap();
        assert!(median > 0.0);

        let p95 = tracker.p95_ms().unwrap();
        assert!(p95 >= median);

        let p0 = tracker.percentile_ms(0).unwrap();
        let p100 = tracker.percentile_ms(100).unwrap();
        assert!(p0 <= p100);
    }
}
