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

// ---------------------------------------------------------------------------
// ActivationGroup – logical grouping of related extensions
// ---------------------------------------------------------------------------

/// Groups related extensions together for batch activation management.
#[derive(Debug, Clone)]
pub struct ActivationGroup {
    pub name: String,
    pub extension_ids: Vec<String>,
    pub policy: ActivationPolicy,
}

impl ActivationGroup {
    /// Create a new activation group with a lazy policy.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            extension_ids: Vec::new(),
            policy: ActivationPolicy::Lazy,
        }
    }

    /// Add an extension to this group.
    pub fn add(&mut self, ext_id: impl Into<String>) {
        let id = ext_id.into();
        if !self.extension_ids.contains(&id) {
            self.extension_ids.push(id);
        }
    }

    /// Remove an extension from this group.
    pub fn remove(&mut self, ext_id: &str) -> bool {
        let before = self.extension_ids.len();
        self.extension_ids.retain(|id| id != ext_id);
        self.extension_ids.len() < before
    }

    /// Set the activation policy for this group.
    pub fn with_policy(mut self, policy: ActivationPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Number of extensions in the group.
    pub fn len(&self) -> usize {
        self.extension_ids.len()
    }

    /// Whether the group is empty.
    pub fn is_empty(&self) -> bool {
        self.extension_ids.is_empty()
    }

    /// Check if an extension belongs to this group.
    pub fn contains(&self, ext_id: &str) -> bool {
        self.extension_ids.iter().any(|id| id == ext_id)
    }

    /// Filter extension IDs to only those present in a provided set.
    pub fn intersect(&self, available: &HashSet<String>) -> Vec<String> {
        self.extension_ids
            .iter()
            .filter(|id| available.contains(id.as_str()))
            .cloned()
            .collect()
    }
}

impl fmt::Display for ActivationGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ActivationGroup(\"{}\", {} exts, policy={})",
            self.name,
            self.extension_ids.len(),
            self.policy,
        )
    }
}

// ---------------------------------------------------------------------------
// ActivationSummary – aggregate report on activation state
// ---------------------------------------------------------------------------

/// Summarises the current activation state across all extensions.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivationSummary {
    pub total_registered: usize,
    pub total_activated: usize,
    pub total_pending: usize,
    pub events_by_kind: HashMap<String, usize>,
}

impl ActivationSummary {
    /// Build a summary from an [`ExtensionActivationQueue`].
    ///
    /// Because the queue's internal registry is private, this constructor
    /// captures only the high-level counts available through the public API.
    pub fn from_queue(queue: &ExtensionActivationQueue) -> Self {
        let total_registered = queue.registered_count();
        let total_activated = queue.activated_count();
        let total_pending = queue.pending_count();

        Self {
            total_registered,
            total_activated,
            total_pending,
            events_by_kind: HashMap::new(),
        }
    }

    /// Build a summary with explicit event counts (for callers that
    /// have access to the individual event lists).
    pub fn with_events(
        total_registered: usize,
        total_activated: usize,
        total_pending: usize,
        events: &[ActivationEvent],
    ) -> Self {
        let mut events_by_kind: HashMap<String, usize> = HashMap::new();
        for event in events {
            let key = match event {
                ActivationEvent::Star => "Star",
                ActivationEvent::OnLanguage(_) => "OnLanguage",
                ActivationEvent::OnCommand(_) => "OnCommand",
                ActivationEvent::OnFileSystem(_) => "OnFileSystem",
                ActivationEvent::OnView(_) => "OnView",
                ActivationEvent::OnUri(_) => "OnUri",
                ActivationEvent::WorkspaceContains(_) => "WorkspaceContains",
                ActivationEvent::OnDebug => "OnDebug",
                ActivationEvent::OnAuthenticationRequest(_) => "OnAuthenticationRequest",
                ActivationEvent::OnStartupFinished => "OnStartupFinished",
            };
            *events_by_kind.entry(key.to_string()).or_insert(0) += 1;
        }
        Self {
            total_registered,
            total_activated,
            total_pending,
            events_by_kind,
        }
    }

    /// The fraction of registered extensions that have been activated.
    pub fn activation_ratio(&self) -> f64 {
        if self.total_registered == 0 {
            return 0.0;
        }
        self.total_activated as f64 / self.total_registered as f64
    }

    /// Whether all registered extensions have been activated.
    pub fn all_activated(&self) -> bool {
        self.total_pending == 0 && self.total_registered == self.total_activated
    }
}

impl fmt::Display for ActivationSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ActivationSummary(registered={}, activated={}, pending={}, ratio={:.1}%)",
            self.total_registered,
            self.total_activated,
            self.total_pending,
            self.activation_ratio() * 100.0,
        )
    }
}

/// Parses and categorizes activation events with error collection.
#[derive(Debug, Clone, Default)]
pub struct ActivationEventParser;

impl ActivationEventParser {
    /// Parse all events, collecting errors for invalid ones.
    pub fn validate_and_parse(events: &[&str]) -> Result<Vec<ActivationEvent>, Vec<String>> {
        let mut parsed = Vec::new();
        let mut errors = Vec::new();
        for &e in events {
            match validate_activation_event(e) {
                Ok(ev) => parsed.push(ev),
                Err(msg) => errors.push(msg),
            }
        }
        if errors.is_empty() {
            Ok(parsed)
        } else {
            Err(errors)
        }
    }

    /// Group events by their type name.
    pub fn categorize(events: &[ActivationEvent]) -> HashMap<String, Vec<ActivationEvent>> {
        let mut map: HashMap<String, Vec<ActivationEvent>> = HashMap::new();
        for ev in events {
            let key = Self::event_type_name(ev);
            map.entry(key).or_default().push(ev.clone());
        }
        map
    }

    /// Returns true if any event is `*`.
    pub fn has_star(events: &[ActivationEvent]) -> bool {
        events.iter().any(|e| matches!(e, ActivationEvent::Star))
    }

    /// Returns unique sorted type names present in the event list.
    pub fn event_types(events: &[ActivationEvent]) -> Vec<String> {
        let mut types: Vec<String> = events
            .iter()
            .map(|e| Self::event_type_name(e))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        types.sort();
        types
    }

    fn event_type_name(event: &ActivationEvent) -> String {
        match event {
            ActivationEvent::Star => "star".into(),
            ActivationEvent::OnLanguage(_) => "language".into(),
            ActivationEvent::OnCommand(_) => "command".into(),
            ActivationEvent::OnView(_) => "view".into(),
            ActivationEvent::OnUri(_) => "uri".into(),
            ActivationEvent::OnFileSystem(_) => "fileSystem".into(),
            ActivationEvent::WorkspaceContains(_) => "workspaceContains".into(),
            ActivationEvent::OnDebug => "debug".into(),
            ActivationEvent::OnAuthenticationRequest(_) => "authenticationRequest".into(),
            ActivationEvent::OnStartupFinished => "startupFinished".into(),
        }
    }
}

/// A single timing record for an extension activation.
#[derive(Debug, Clone)]
pub struct ActivationTimingRecord {
    pub extension_id: String,
    pub event: String,
    pub duration_ms: u64,
    pub timestamp: u64,
}

/// Profiles activation timing across extensions.
#[derive(Debug, Clone, Default)]
pub struct ActivationTimingProfiler {
    records: Vec<ActivationTimingRecord>,
}

impl ActivationTimingProfiler {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    pub fn record(&mut self, ext_id: &str, event: &str, duration_ms: u64, timestamp: u64) {
        self.records.push(ActivationTimingRecord {
            extension_id: ext_id.to_string(),
            event: event.to_string(),
            duration_ms,
            timestamp,
        });
    }

    pub fn total_startup_time(&self) -> u64 {
        self.records.iter().map(|r| r.duration_ms).sum()
    }

    pub fn slowest(&self) -> Option<&ActivationTimingRecord> {
        self.records.iter().max_by_key(|r| r.duration_ms)
    }

    pub fn fastest(&self) -> Option<&ActivationTimingRecord> {
        self.records.iter().min_by_key(|r| r.duration_ms)
    }

    pub fn average_ms(&self) -> f64 {
        if self.records.is_empty() {
            return 0.0;
        }
        self.total_startup_time() as f64 / self.records.len() as f64
    }

    pub fn by_extension(&self, ext_id: &str) -> Vec<&ActivationTimingRecord> {
        self.records.iter().filter(|r| r.extension_id == ext_id).collect()
    }

    pub fn count(&self) -> usize {
        self.records.len()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }

    pub fn top_n_slowest(&self, n: usize) -> Vec<&ActivationTimingRecord> {
        let mut sorted: Vec<&ActivationTimingRecord> = self.records.iter().collect();
        sorted.sort_by(|a, b| b.duration_ms.cmp(&a.duration_ms));
        sorted.truncate(n);
        sorted
    }
}

/// Schedules lazy activation of extensions, tracking pending vs activated state.
#[derive(Debug, Clone, Default)]
pub struct LazyActivationScheduler {
    scheduled: HashMap<String, ActivationEvent>,
    activated: HashSet<String>,
}

impl LazyActivationScheduler {
    pub fn new() -> Self {
        Self {
            scheduled: HashMap::new(),
            activated: HashSet::new(),
        }
    }

    pub fn schedule(&mut self, ext_id: &str, event: ActivationEvent) {
        if !self.activated.contains(ext_id) {
            self.scheduled.insert(ext_id.to_string(), event);
        }
    }

    /// Move an extension from scheduled to activated, returning its event.
    pub fn activate(&mut self, ext_id: &str) -> Option<ActivationEvent> {
        let event = self.scheduled.remove(ext_id)?;
        self.activated.insert(ext_id.to_string());
        Some(event)
    }

    pub fn is_scheduled(&self, ext_id: &str) -> bool {
        self.scheduled.contains_key(ext_id)
    }

    pub fn is_activated(&self, ext_id: &str) -> bool {
        self.activated.contains(ext_id)
    }

    pub fn pending_count(&self) -> usize {
        self.scheduled.len()
    }

    pub fn activated_count(&self) -> usize {
        self.activated.len()
    }

    pub fn cancel(&mut self, ext_id: &str) {
        self.scheduled.remove(ext_id);
    }
}

/// Resolves activation dependencies between extensions via topological ordering.
#[derive(Debug, Clone, Default)]
pub struct ActivationDependencyResolver {
    deps: HashMap<String, Vec<String>>,
}

impl ActivationDependencyResolver {
    pub fn new() -> Self {
        Self { deps: HashMap::new() }
    }

    pub fn add_dependency(&mut self, ext_id: &str, depends_on: &str) {
        self.deps
            .entry(ext_id.to_string())
            .or_default()
            .push(depends_on.to_string());
    }

    /// Returns the topological activation order needed to activate `ext_id` (BFS).
    /// The result lists dependencies first, ending with `ext_id` itself.
    pub fn resolve_order(&self, ext_id: &str) -> Vec<String> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut order = Vec::new();
        queue.push_back(ext_id.to_string());
        visited.insert(ext_id.to_string());
        while let Some(current) = queue.pop_front() {
            if let Some(dep_list) = self.deps.get(&current) {
                for dep in dep_list {
                    if visited.insert(dep.clone()) {
                        queue.push_back(dep.clone());
                    }
                }
            }
            order.push(current);
        }
        order.reverse();
        order
    }

    /// Detects whether activating `ext_id` would encounter a circular dependency.
    pub fn has_circular(&self, ext_id: &str) -> bool {
        let mut visited = HashSet::new();
        let mut stack = vec![ext_id.to_string()];
        while let Some(current) = stack.pop() {
            if !visited.insert(current.clone()) {
                return true;
            }
            if let Some(dep_list) = self.deps.get(&current) {
                for dep in dep_list {
                    stack.push(dep.clone());
                }
            }
        }
        false
    }

    pub fn direct_deps(&self, ext_id: &str) -> Vec<String> {
        self.deps.get(ext_id).cloned().unwrap_or_default()
    }

    /// Returns all extension IDs that directly depend on `ext_id`.
    pub fn all_dependents(&self, ext_id: &str) -> Vec<String> {
        self.deps
            .iter()
            .filter(|(_, dep_list)| dep_list.contains(&ext_id.to_string()))
            .map(|(id, _)| id.clone())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ActivationEventValidator - activation event validator
// ---------------------------------------------------------------------------

/// Severity level for activation event validator issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActivationEventValidatorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ActivationEventValidatorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [ActivationEventValidator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationEventValidatorEntry {
    pub id: String,
    pub label: String,
    pub severity: ActivationEventValidatorSeverity,
    pub detail: Option<String>,
    pub event_count: usize,
    enabled: bool,
}

impl ActivationEventValidatorEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: ActivationEventValidatorSeverity::Low,
            detail: None,
            event_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: ActivationEventValidatorSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_event_count(mut self, val: usize) -> Self {
        self.event_count = val;
        self
    }

    pub fn is_valid_event(&self) -> bool {
        self.enabled && self.severity >= ActivationEventValidatorSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.event_count, det)
    }
}

impl fmt::Display for ActivationEventValidatorEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [ActivationEventValidatorEntry] items.
#[derive(Debug, Clone)]
pub struct ActivationEventValidator {
    entries: Vec<ActivationEventValidatorEntry>,
    name: String,
    capacity: usize,
}

impl ActivationEventValidator {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: ActivationEventValidatorEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<ActivationEventValidatorEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&ActivationEventValidatorEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn event_count(&self) -> usize { self.entries.len() }

    pub fn is_valid_event(&self) -> bool {
        self.entries.iter().any(|e| e.is_valid_event())
    }

    pub fn entries_by_severity(&self, severity: ActivationEventValidatorSeverity) -> Vec<&ActivationEventValidatorEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= ActivationEventValidatorSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&ActivationEventValidatorEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&ActivationEventValidatorEntry> {
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
// ActivationStartupProfiler - activation startup profiler
// ---------------------------------------------------------------------------

/// Configuration for [ActivationStartupProfiler].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationStartupProfilerConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub startup_ms: usize,
}

impl ActivationStartupProfilerConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, startup_ms: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_startup_ms(mut self, val: usize) -> Self { self.startup_ms = val; self }
}

impl Default for ActivationStartupProfilerConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ActivationStartupProfiler].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationStartupProfilerItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ActivationStartupProfilerItem {
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

    pub fn is_slow_startup(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ActivationStartupProfilerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ActivationStartupProfilerItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ActivationStartupProfiler {
    config: ActivationStartupProfilerConfig,
    items: Vec<ActivationStartupProfilerItem>,
}

impl ActivationStartupProfiler {
    pub fn new(config: ActivationStartupProfilerConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ActivationStartupProfilerItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ActivationStartupProfilerItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ActivationStartupProfilerItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn startup_ms(&self) -> usize { self.items.len() }

    pub fn is_slow_startup(&self) -> bool {
        self.items.iter().any(|i| i.is_slow_startup())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ActivationStartupProfilerItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ActivationStartupProfilerItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ActivationStartupProfilerConfig {
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
// vsedit-ext-activation: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtActivationXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl ExtActivationXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for ExtActivationXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct ExtActivationXRegistry {
    entries: Vec<ExtActivationXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl ExtActivationXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: ExtActivationXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&ExtActivationXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut ExtActivationXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<ExtActivationXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&ExtActivationXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&ExtActivationXConfig> {
        let mut sorted: Vec<&ExtActivationXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&ExtActivationXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> ExtActivationXIterator<'_> {
        ExtActivationXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct ExtActivationXIterator<'a> {
    inner: std::slice::Iter<'a, ExtActivationXConfig>,
}

impl<'a> Iterator for ExtActivationXIterator<'a> {
    type Item = &'a ExtActivationXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct ExtActivationXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl ExtActivationXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct ExtActivationXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl ExtActivationXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &ExtActivationXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &ExtActivationXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &ExtActivationXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for ExtActivationXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct ExtActivationXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl ExtActivationXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &ExtActivationXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &ExtActivationXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for ExtActivationXValidator {
    fn default() -> Self {
        Self::new()
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
// xb_ utilities – batch 79
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer79 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer79 {
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
pub fn xb_fnv1a_79(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_79<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_79<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_79(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_79(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 48
// ---------------------------------------------------------------------------

/// Generic object pool `Xc48Pool<T>`.
pub struct Xc48Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc48Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc48PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc48Pool<T> {
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
    pub fn stats(&self) -> Xc48PoolStats {
        Xc48PoolStats {
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

impl<T> Default for Xc48Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc48Scheduler`.
pub struct Xc48Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc48Scheduler {
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

impl Default for Xc48Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_48 hash for the given byte slice.
pub fn xc_48_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_48 convention.
pub fn xc_48_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe92 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe92Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe92PipelineError {
    pub stage: Xe92Stage,
    pub message: String,
}

impl std::fmt::Display for Xe92PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe92Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe92Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe92PipelineError>>>,
    stage_names: Vec<Xe92Stage>,
}

impl Xe92Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe92PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe92Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe92PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe92Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe92PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe92Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe92PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe92Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe92PipelineError> {
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

    pub fn compose(mut self, other: Xe92Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe92CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe92CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe92Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe92CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe92CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe92Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe92CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_92_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe92CacheEntry {
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

    fn xe_92_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe92CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_92_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe92PipelineError> {
    Ok(data)
}

pub fn xe_92_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe92PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_92_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe92PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_92_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe92PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_92_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe92PipelineError> {
    Err(Xe92PipelineError {
        stage: Xe92Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_90: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg90Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg90Graph {
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

impl Default for Xg90Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_90: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg90Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg90Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg90Heap<T>) {
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

impl<T: Ord> Default for Xg90Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 47).
pub struct Xh47SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh47SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 89 as u64,
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

/// A compact bit set supporting boolean operations (variant 47).
pub struct Xh47BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh47BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 47).
pub struct Xi47Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi47Deque<T> {
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
pub struct Xi47Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi47Interval {
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

/// A simple interval tree (variant 47).
pub struct Xi47IntervalTree {
    xi_intervals: Vec<Xi47Interval>,
}

impl Xi47IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi47Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi47Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi47Interval) -> Vec<&Xi47Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi47Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi47Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi47Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi47Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi47Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi47Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 47) ---

/// Disjoint set / union-find for crate 47.
pub struct Xj47UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj47UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ47_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 47.
pub struct Xj47BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj47BTreeNode<K, V>>>,
    len: usize,
}

struct Xj47BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj47BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj47BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ47_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ47_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj47BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj47BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj47BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj47BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_47 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk47SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk47SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk47DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk47DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_47).
#[derive(Debug, Clone)]
pub struct Xl47Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl47Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_47).
#[derive(Debug, Clone)]
pub struct Xl47SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl47SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm47MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm47MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm47Tokenizer {
    text: String,
}

impl Xm47Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
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

    #[test]
    fn activation_group_add_remove() {
        let mut group = ActivationGroup::new("python-tools");
        group.add("pylint");
        group.add("pyright");
        group.add("pylint"); // duplicate
        assert_eq!(group.len(), 2);
        assert!(group.contains("pylint"));
        assert!(group.remove("pylint"));
        assert_eq!(group.len(), 1);
        assert!(!group.contains("pylint"));
        assert!(!group.remove("nonexistent"));
    }

    #[test]
    fn activation_group_intersect() {
        let mut group = ActivationGroup::new("web");
        group.add("eslint");
        group.add("prettier");
        group.add("stylelint");
        let available: HashSet<String> =
            ["eslint", "stylelint"].iter().map(|s| s.to_string()).collect();
        let result = group.intersect(&available);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"eslint".to_string()));
        assert!(!result.contains(&"prettier".to_string()));
    }

    #[test]
    fn activation_group_display_and_policy() {
        let group = ActivationGroup::new("test-group")
            .with_policy(ActivationPolicy::Eager);
        assert!(group.policy.is_eager());
        let display = format!("{group}");
        assert!(display.contains("test-group"));
        assert!(display.contains("eager"));
    }

    #[test]
    fn activation_summary_from_queue() {
        let mut queue = ExtensionActivationQueue::new();
        queue.register(
            "ext-a".into(),
            vec![ActivationEvent::Star],
        );
        queue.register(
            "ext-b".into(),
            vec![ActivationEvent::OnLanguage("rust".into())],
        );
        let summary = ActivationSummary::from_queue(&queue);
        assert_eq!(summary.total_registered, 2);
        assert_eq!(summary.total_activated, 0);
        assert_eq!(summary.total_pending, 0);
        assert!(!summary.all_activated());
    }

    #[test]
    fn activation_summary_with_events() {
        let events = vec![
            ActivationEvent::Star,
            ActivationEvent::OnLanguage("rust".into()),
            ActivationEvent::OnLanguage("python".into()),
        ];
        let summary = ActivationSummary::with_events(3, 1, 0, &events);
        assert_eq!(*summary.events_by_kind.get("Star").unwrap(), 1);
        assert_eq!(*summary.events_by_kind.get("OnLanguage").unwrap(), 2);
    }

    #[test]
    fn activation_summary_ratio_and_display() {
        let mut queue = ExtensionActivationQueue::new();
        queue.register("ext-a".into(), vec![ActivationEvent::Star]);
        queue.register("ext-b".into(), vec![ActivationEvent::Star]);
        let mut matcher = ActivationEventMatcher::new();
        matcher.finish_startup();
        let _ = queue.evaluate(&matcher);
        let _ = queue.pop_pending();
        let summary = ActivationSummary::from_queue(&queue);
        assert!(summary.activation_ratio() > 0.0);
        let display = format!("{summary}");
        assert!(display.contains("ActivationSummary"));
    }

    #[test]
    fn activation_group_empty() {
        let group = ActivationGroup::new("empty");
        assert!(group.is_empty());
        assert_eq!(group.len(), 0);
    }

    #[test]
    fn parser_validate_and_parse_ok() {
        let events = vec!["*", "onLanguage:rust", "onCommand:doThing"];
        let result = ActivationEventParser::validate_and_parse(&events);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], ActivationEvent::Star);
    }

    #[test]
    fn parser_validate_and_parse_errors() {
        let events = vec!["*", "bogus:event", "alsoInvalid"];
        let result = ActivationEventParser::validate_and_parse(&events);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn parser_categorize_events() {
        let events = vec![
            ActivationEvent::Star,
            ActivationEvent::OnLanguage("rust".into()),
            ActivationEvent::OnLanguage("python".into()),
            ActivationEvent::OnCommand("save".into()),
        ];
        let cat = ActivationEventParser::categorize(&events);
        assert_eq!(cat.get("language").unwrap().len(), 2);
        assert_eq!(cat.get("command").unwrap().len(), 1);
        assert_eq!(cat.get("star").unwrap().len(), 1);
    }

    #[test]
    fn parser_has_star_and_event_types() {
        let events = vec![
            ActivationEvent::OnLanguage("rust".into()),
            ActivationEvent::Star,
        ];
        assert!(ActivationEventParser::has_star(&events));
        assert!(!ActivationEventParser::has_star(&[ActivationEvent::OnDebug]));
        let types = ActivationEventParser::event_types(&events);
        assert_eq!(types, vec!["language", "star"]);
    }

    #[test]
    fn timing_profiler_basic() {
        let mut profiler = ActivationTimingProfiler::new();
        profiler.record("ext-a", "onLanguage:rust", 50, 100);
        profiler.record("ext-b", "onCommand:save", 120, 200);
        profiler.record("ext-a", "*", 30, 300);
        assert_eq!(profiler.count(), 3);
        assert_eq!(profiler.total_startup_time(), 200);
        assert!((profiler.average_ms() - 66.666).abs() < 1.0);
    }

    #[test]
    fn timing_profiler_slowest_fastest() {
        let mut profiler = ActivationTimingProfiler::new();
        profiler.record("ext-a", "ev1", 10, 0);
        profiler.record("ext-b", "ev2", 90, 1);
        profiler.record("ext-c", "ev3", 50, 2);
        assert_eq!(profiler.slowest().unwrap().extension_id, "ext-b");
        assert_eq!(profiler.fastest().unwrap().extension_id, "ext-a");
        let top2 = profiler.top_n_slowest(2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0].duration_ms, 90);
        assert_eq!(top2[1].duration_ms, 50);
    }

    #[test]
    fn timing_profiler_by_extension_and_clear() {
        let mut profiler = ActivationTimingProfiler::new();
        profiler.record("ext-a", "ev1", 10, 0);
        profiler.record("ext-b", "ev2", 20, 1);
        profiler.record("ext-a", "ev3", 30, 2);
        assert_eq!(profiler.by_extension("ext-a").len(), 2);
        assert_eq!(profiler.by_extension("ext-c").len(), 0);
        profiler.clear();
        assert_eq!(profiler.count(), 0);
        assert_eq!(profiler.average_ms(), 0.0);
    }

    #[test]
    fn lazy_scheduler_lifecycle() {
        let mut sched = LazyActivationScheduler::new();
        sched.schedule("ext-a", ActivationEvent::OnLanguage("rust".into()));
        sched.schedule("ext-b", ActivationEvent::Star);
        assert!(sched.is_scheduled("ext-a"));
        assert!(!sched.is_activated("ext-a"));
        assert_eq!(sched.pending_count(), 2);
        let ev = sched.activate("ext-a");
        assert_eq!(ev, Some(ActivationEvent::OnLanguage("rust".into())));
        assert!(sched.is_activated("ext-a"));
        assert!(!sched.is_scheduled("ext-a"));
        assert_eq!(sched.activated_count(), 1);
        assert_eq!(sched.pending_count(), 1);
    }

    #[test]
    fn lazy_scheduler_cancel_and_no_double_schedule() {
        let mut sched = LazyActivationScheduler::new();
        sched.schedule("ext-a", ActivationEvent::Star);
        sched.cancel("ext-a");
        assert!(!sched.is_scheduled("ext-a"));
        assert_eq!(sched.pending_count(), 0);
        // already-activated extension cannot be re-scheduled
        sched.schedule("ext-b", ActivationEvent::OnDebug);
        sched.activate("ext-b");
        sched.schedule("ext-b", ActivationEvent::Star);
        assert!(!sched.is_scheduled("ext-b"));
        assert!(sched.is_activated("ext-b"));
    }

    #[test]
    fn dependency_resolver_order() {
        let mut resolver = ActivationDependencyResolver::new();
        resolver.add_dependency("app", "lib-a");
        resolver.add_dependency("app", "lib-b");
        resolver.add_dependency("lib-a", "core");
        let order = resolver.resolve_order("app");
        let app_pos = order.iter().position(|x| x == "app").unwrap();
        let lib_a_pos = order.iter().position(|x| x == "lib-a").unwrap();
        let core_pos = order.iter().position(|x| x == "core").unwrap();
        assert!(core_pos < lib_a_pos);
        assert!(lib_a_pos < app_pos);
    }

    #[test]
    fn dependency_resolver_circular() {
        let mut resolver = ActivationDependencyResolver::new();
        resolver.add_dependency("a", "b");
        resolver.add_dependency("b", "a");
        assert!(resolver.has_circular("a"));
        let mut resolver2 = ActivationDependencyResolver::new();
        resolver2.add_dependency("x", "y");
        assert!(!resolver2.has_circular("x"));
    }

    #[test]
    fn dependency_resolver_direct_and_dependents() {
        let mut resolver = ActivationDependencyResolver::new();
        resolver.add_dependency("app", "lib-a");
        resolver.add_dependency("app", "lib-b");
        resolver.add_dependency("svc", "lib-a");
        let direct = resolver.direct_deps("app");
        assert_eq!(direct.len(), 2);
        assert!(direct.contains(&"lib-a".to_string()));
        let dependents = resolver.all_dependents("lib-a");
        assert_eq!(dependents.len(), 2);
        assert!(dependents.contains(&"app".to_string()));
        assert!(dependents.contains(&"svc".to_string()));
        assert!(resolver.direct_deps("unknown").is_empty());
    }

#[test]
    fn activationeventvalidator_severity_ordering() {
        assert!(ActivationEventValidatorSeverity::Critical > ActivationEventValidatorSeverity::High);
        assert!(ActivationEventValidatorSeverity::High > ActivationEventValidatorSeverity::Medium);
        assert!(ActivationEventValidatorSeverity::Medium > ActivationEventValidatorSeverity::Low);
    }

    #[test]
    fn activationeventvalidator_severity_display() {
        assert_eq!(ActivationEventValidatorSeverity::Low.to_string(), "low");
        assert_eq!(ActivationEventValidatorSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn activationeventvalidator_entry_creation() {
        let e = ActivationEventValidatorEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, ActivationEventValidatorSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn activationeventvalidator_entry_builder() {
        let e = ActivationEventValidatorEntry::new("e2", "Entry 2")
            .with_severity(ActivationEventValidatorSeverity::High)
            .with_detail("some detail")
            .with_event_count(42);
        assert_eq!(e.severity, ActivationEventValidatorSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.event_count, 42);
    }

    #[test]
    fn activationeventvalidator_entry_enable_disable() {
        let mut e = ActivationEventValidatorEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn activationeventvalidator_add_and_count() {
        let mut mgr = ActivationEventValidator::new("test");
        mgr.add(ActivationEventValidatorEntry::new("a", "A"));
        mgr.add(ActivationEventValidatorEntry::new("b", "B").with_severity(ActivationEventValidatorSeverity::High));
        assert_eq!(mgr.event_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn activationeventvalidator_remove() {
        let mut mgr = ActivationEventValidator::new("test");
        mgr.add(ActivationEventValidatorEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn activationeventvalidator_capacity() {
        let mut mgr = ActivationEventValidator::new("test").with_capacity(1);
        assert!(mgr.add(ActivationEventValidatorEntry::new("a", "A")));
        assert!(!mgr.add(ActivationEventValidatorEntry::new("b", "B")));
    }

    #[test]
    fn activationeventvalidator_sorted_by_severity() {
        let mut mgr = ActivationEventValidator::new("test");
        mgr.add(ActivationEventValidatorEntry::new("lo", "Low"));
        mgr.add(ActivationEventValidatorEntry::new("hi", "High").with_severity(ActivationEventValidatorSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, ActivationEventValidatorSeverity::Critical);
    }

    #[test]
    fn activationeventvalidator_summary() {
        let mgr = ActivationEventValidator::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn activationstartupprofiler_config_defaults() {
        let cfg = ActivationStartupProfilerConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn activationstartupprofiler_item_creation() {
        let item = ActivationStartupProfilerItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn activationstartupprofiler_add_and_get() {
        let mut mgr = ActivationStartupProfiler::new(ActivationStartupProfilerConfig::new("test"));
        mgr.add(ActivationStartupProfilerItem::new("k1", "v1"));
        assert_eq!(mgr.startup_ms(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn activationstartupprofiler_remove_item() {
        let mut mgr = ActivationStartupProfiler::new(ActivationStartupProfilerConfig::new("test"));
        mgr.add(ActivationStartupProfilerItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn activationstartupprofiler_sorted_by_priority() {
        let mut mgr = ActivationStartupProfiler::new(ActivationStartupProfilerConfig::new("test"));
        mgr.add(ActivationStartupProfilerItem::new("lo", "low").with_priority(1));
        mgr.add(ActivationStartupProfilerItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn activationstartupprofiler_items_with_tag() {
        let mut mgr = ActivationStartupProfiler::new(ActivationStartupProfilerConfig::new("test"));
        mgr.add(ActivationStartupProfilerItem::new("a", "1").with_tag("x"));
        mgr.add(ActivationStartupProfilerItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn activationstartupprofiler_report() {
        let mgr = ActivationStartupProfiler::new(ActivationStartupProfilerConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn extActivation_x_config_new() {
        let c = ExtActivationXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn extActivation_x_config_builder() {
        let c = ExtActivationXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn extActivation_x_config_display() {
        let c = ExtActivationXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn extActivation_x_registry_insert_get() {
        let mut reg = ExtActivationXRegistry::new();
        reg.insert(ExtActivationXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn extActivation_x_registry_duplicate() {
        let mut reg = ExtActivationXRegistry::new();
        reg.insert(ExtActivationXConfig::new("a")).unwrap();
        assert!(reg.insert(ExtActivationXConfig::new("a")).is_err());
    }

    #[test]
    fn extActivation_x_registry_remove() {
        let mut reg = ExtActivationXRegistry::new();
        reg.insert(ExtActivationXConfig::new("a")).unwrap();
        reg.insert(ExtActivationXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn extActivation_x_registry_active_entries() {
        let mut reg = ExtActivationXRegistry::new();
        reg.insert(ExtActivationXConfig::new("a")).unwrap();
        reg.insert(ExtActivationXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn extActivation_x_registry_by_weight() {
        let mut reg = ExtActivationXRegistry::new();
        reg.insert(ExtActivationXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(ExtActivationXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn extActivation_x_registry_tags() {
        let mut reg = ExtActivationXRegistry::new();
        reg.insert(ExtActivationXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(ExtActivationXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn extActivation_x_registry_total_weight() {
        let mut reg = ExtActivationXRegistry::new();
        reg.insert(ExtActivationXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(ExtActivationXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn extActivation_x_registry_iterator() {
        let mut reg = ExtActivationXRegistry::new();
        reg.insert(ExtActivationXConfig::new("a")).unwrap();
        reg.insert(ExtActivationXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn extActivation_x_cache_put_get() {
        let mut cache = ExtActivationXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn extActivation_x_cache_eviction() {
        let mut cache = ExtActivationXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn extActivation_x_cache_lru_order() {
        let mut cache = ExtActivationXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn extActivation_x_cache_most_least_recent() {
        let mut cache = ExtActivationXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn extActivation_x_formatter_entry() {
        let e = ExtActivationXConfig::new("k").with_value("v");
        let fmt = ExtActivationXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn extActivation_x_formatter_summary() {
        let mut reg = ExtActivationXRegistry::new();
        reg.insert(ExtActivationXConfig::new("a").with_weight(5)).unwrap();
        let fmt = ExtActivationXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn extActivation_x_validator_valid() {
        let v = ExtActivationXValidator::new();
        let c = ExtActivationXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn extActivation_x_validator_empty_key() {
        let v = ExtActivationXValidator::new();
        let c = ExtActivationXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extActivation_x_validator_require_value() {
        let v = ExtActivationXValidator::new().require_value(true);
        let c = ExtActivationXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extActivation_x_validator_allowed_tags() {
        let v = ExtActivationXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = ExtActivationXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extActivation_x_validator_validate_all() {
        let v = ExtActivationXValidator::new();
        let mut reg = ExtActivationXRegistry::new();
        reg.insert(ExtActivationXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
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


    #[test]
    fn xb_ring_buffer_79_push_and_len() {
        let mut rb = super::XbRingBuffer79::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_79_overwrite() {
        let mut rb = super::XbRingBuffer79::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_79_get_out_of_bounds() {
        let rb = super::XbRingBuffer79::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_79_drain_all() {
        let mut rb = super::XbRingBuffer79::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_79_peek_front_back() {
        let mut rb = super::XbRingBuffer79::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_79_clear() {
        let mut rb = super::XbRingBuffer79::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_79_capacity() {
        let rb = super::XbRingBuffer79::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_79_basic() {
        let h = super::xb_fnv1a_79(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_79(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_79_different_inputs() {
        let h1 = super::xb_fnv1a_79(b"abc");
        let h2 = super::xb_fnv1a_79(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_79_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_79(&data);
        let dec = super::xb_rle_decode_79(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_79_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_79(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_79(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_79_values() {
        assert!((super::xb_clamp_79(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_79(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_79(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_79_values() {
        assert!((super::xb_lerp_79(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_79(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_79(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_79_wrap_around_twice() {
        let mut rb = super::XbRingBuffer79::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 48 ----

    #[test]
    fn xc_48_pool_new_empty() {
        let pool: super::Xc48Pool<i32> = super::Xc48Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_48_pool_release_acquire() {
        let mut pool = super::Xc48Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_48_pool_acquire_empty() {
        let mut pool: super::Xc48Pool<i32> = super::Xc48Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_48_pool_full() {
        let mut pool = super::Xc48Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_48_pool_drain() {
        let mut pool = super::Xc48Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_48_pool_stats() {
        let mut pool = super::Xc48Pool::new(8);
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
    fn xc_48_pool_clear() {
        let mut pool = super::Xc48Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_48_pool_shrink() {
        let mut pool = super::Xc48Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_48_pool_default() {
        let pool: super::Xc48Pool<String> = super::Xc48Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_48_pool_extend() {
        let mut pool = super::Xc48Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_48_pool_retain() {
        let mut pool = super::Xc48Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_48_scheduler_round_robin() {
        let mut sched = super::Xc48Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_48_scheduler_empty() {
        let mut sched = super::Xc48Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_48_scheduler_reset() {
        let mut sched = super::Xc48Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_48_scheduler_add_remove() {
        let mut sched = super::Xc48Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_48_scheduler_targets() {
        let sched = super::Xc48Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_48_hash_empty() {
        assert_eq!(super::xc_48_hash(b""), 5381);
    }

    #[test]
    fn xc_48_hash_data() {
        let h = super::xc_48_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_48_hash(b"hello"), h);
    }

    #[test]
    fn xc_48_reverse_str() {
        assert_eq!(super::xc_48_reverse("abc"), "cba");
        assert_eq!(super::xc_48_reverse(""), "");
    }


    #[test]
    fn xe_92_pipeline_empty() {
        let p = super::Xe92Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_92_pipeline_parse_stage() {
        let p = super::Xe92Pipeline::new()
            .add_parse(super::xe_92_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_92_pipeline_transform_double() {
        let p = super::Xe92Pipeline::new()
            .add_transform(super::xe_92_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_92_pipeline_validate_reverse() {
        let p = super::Xe92Pipeline::new()
            .add_validate(super::xe_92_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_92_pipeline_emit_filter() {
        let p = super::Xe92Pipeline::new()
            .add_emit(super::xe_92_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_92_pipeline_multi_stage() {
        let p = super::Xe92Pipeline::new()
            .add_parse(super::xe_92_pipeline_identity)
            .add_transform(super::xe_92_pipeline_double)
            .add_validate(super::xe_92_pipeline_reverse)
            .add_emit(super::xe_92_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_92_pipeline_error_propagation() {
        let p = super::Xe92Pipeline::new()
            .add_parse(super::xe_92_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe92Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_92_pipeline_compose() {
        let p1 = super::Xe92Pipeline::new()
            .add_parse(super::xe_92_pipeline_identity);
        let p2 = super::Xe92Pipeline::new()
            .add_transform(super::xe_92_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_92_pipeline_error_display() {
        let e = super::Xe92PipelineError {
            stage: super::Xe92Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_92_cache_put_get() {
        let mut c = super::Xe92Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_92_cache_miss() {
        let mut c: super::Xe92Cache<&str, i32> = super::Xe92Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_92_cache_ttl_expiry() {
        let mut c = super::Xe92Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_92_cache_evict() {
        let mut c = super::Xe92Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_92_cache_capacity() {
        let mut c = super::Xe92Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_92_cache_stats() {
        let mut c = super::Xe92Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_92_cache_clear() {
        let mut c = super::Xe92Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_90 graph tests ------------------------------------------------

    #[test]
    fn xg_90_graph_empty() {
        let g = super::Xg90Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_90_graph_add_node() {
        let mut g = super::Xg90Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_90_graph_add_edge() {
        let mut g = super::Xg90Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_90_graph_neighbors() {
        let mut g = super::Xg90Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_90_graph_has_path() {
        let mut g = super::Xg90Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_90_graph_self_path() {
        let g = super::Xg90Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_90_graph_topo_sort() {
        let mut g = super::Xg90Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_90_graph_cycle_detect_false() {
        let mut g = super::Xg90Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_90_graph_cycle_detect_true() {
        let mut g = super::Xg90Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_90 heap tests -------------------------------------------------

    #[test]
    fn xg_90_heap_empty() {
        let h: super::Xg90Heap<i32> = super::Xg90Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_90_heap_push_pop() {
        let mut h = super::Xg90Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_90_heap_peek() {
        let mut h = super::Xg90Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_90_heap_drain_sorted() {
        let mut h = super::Xg90Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_90_heap_merge() {
        let mut a = super::Xg90Heap::new();
        let mut b = super::Xg90Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_90_heap_default() {
        let h: super::Xg90Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_90_graph_default() {
        let g: super::Xg90Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh47_skip_insert_contains() {
        let mut sl = super::Xh47SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh47_skip_remove() {
        let mut sl = super::Xh47SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh47_skip_len() {
        let mut sl = super::Xh47SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh47_skip_range_query() {
        let mut sl = super::Xh47SkipList::xh_new(4);
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
    fn xh47_skip_floor_ceiling() {
        let mut sl = super::Xh47SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh47_skip_rank() {
        let mut sl = super::Xh47SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh47_skip_empty() {
        let sl = super::Xh47SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh47_skip_duplicates() {
        let mut sl = super::Xh47SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh47_bitset_set_test() {
        let mut bs = super::Xh47BitSet::xh_new(256);
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
    fn xh47_bitset_clear_count() {
        let mut bs = super::Xh47BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh47_bitset_and_or_xor() {
        let mut a = super::Xh47BitSet::xh_new(128);
        let mut b = super::Xh47BitSet::xh_new(128);
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
    fn xh47_bitset_iter_ones() {
        let mut bs = super::Xh47BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh47_bitset_first_last() {
        let mut bs = super::Xh47BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh47_bitset_empty() {
        let bs = super::Xh47BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi47_deque_push_pop_back() {
        let mut dq = super::Xi47Deque::xi_new(4);
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
    fn xi47_deque_push_pop_front() {
        let mut dq = super::Xi47Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi47_deque_mixed_ops() {
        let mut dq = super::Xi47Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi47_deque_get_and_split() {
        let mut dq = super::Xi47Deque::xi_new(8);
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
    fn xi47_deque_rotate_left() {
        let mut dq = super::Xi47Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi47_deque_rotate_right() {
        let mut dq = super::Xi47Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi47_deque_grow() {
        let mut dq = super::Xi47Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi47_deque_empty() {
        let dq = super::Xi47Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi47_interval_tree_insert_query() {
        let mut tree = super::Xi47IntervalTree::xi_new();
        tree.xi_insert(super::Xi47Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi47Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi47Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi47_interval_tree_overlap() {
        let mut tree = super::Xi47IntervalTree::xi_new();
        tree.xi_insert(super::Xi47Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi47Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi47Interval::xi_new(12, 20));
        let q = super::Xi47Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi47_interval_tree_remove() {
        let mut tree = super::Xi47IntervalTree::xi_new();
        tree.xi_insert(super::Xi47Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi47Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi47_interval_tree_gaps() {
        let mut tree = super::Xi47IntervalTree::xi_new();
        tree.xi_insert(super::Xi47Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi47Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi47Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi47Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi47Interval::xi_new(8, 10));
    }

    #[test]
    fn xi47_interval_tree_merge() {
        let mut tree = super::Xi47IntervalTree::xi_new();
        tree.xi_insert(super::Xi47Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi47Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi47Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi47Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi47Interval::xi_new(10, 15));
    }

    #[test]
    fn xi47_interval_tree_all() {
        let mut tree = super::Xi47IntervalTree::xi_new();
        tree.xi_insert(super::Xi47Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi47Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi47_interval_tree_empty() {
        let tree = super::Xi47IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi47_interval_tree_contains_point() {
        let iv = super::Xi47Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 47) ---

    #[test]
    fn xj_47_uf_make_and_find() {
        let mut uf = super::Xj47UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_47_uf_union_connected() {
        let mut uf = super::Xj47UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_47_uf_component_count() {
        let mut uf = super::Xj47UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_47_uf_component_size() {
        let mut uf = super::Xj47UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_47_uf_largest_component() {
        let mut uf = super::Xj47UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_47_uf_many_elements() {
        let mut uf = super::Xj47UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_47_uf_separate_components() {
        let mut uf = super::Xj47UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_47_uf_path_compression() {
        let mut uf = super::Xj47UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_47_bt_insert_get() {
        let mut bt = super::Xj47BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_47_bt_contains_len() {
        let mut bt = super::Xj47BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_47_bt_replace() {
        let mut bt = super::Xj47BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_47_bt_remove() {
        let mut bt = super::Xj47BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_47_bt_keys_values() {
        let mut bt = super::Xj47BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_47_bt_range() {
        let mut bt = super::Xj47BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_47_bt_min_max() {
        let mut bt = super::Xj47BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_47_bt_many_inserts() {
        let mut bt = super::Xj47BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_47 segment tree tests ---

    #[test]
    fn xk_47_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk47SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_47_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk47SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_47_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk47SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_47_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk47SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_47_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk47SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_47_st_single_element() {
        let data = vec![42];
        let st = super::Xk47SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_47_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk47SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_47_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk47SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_47 disjoint intervals tests ---

    #[test]
    fn xk_47_di_add_and_count() {
        let mut di = super::Xk47DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_47_di_merge_overlap() {
        let mut di = super::Xk47DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_47_di_contains() {
        let mut di = super::Xk47DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_47_di_remove() {
        let mut di = super::Xk47DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_47_di_covered_length() {
        let mut di = super::Xk47DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_47_di_gaps() {
        let mut di = super::Xk47DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_47_di_merge_adjacent() {
        let mut di = super::Xk47DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_47_di_empty() {
        let di = super::Xk47DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_47_rope_new_empty() {
        let rope = super::Xl47Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_47_rope_from_str() {
        let rope = super::Xl47Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_47_rope_insert_at() {
        let mut rope = super::Xl47Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_47_rope_delete_range() {
        let mut rope = super::Xl47Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_47_rope_char_at() {
        let rope = super::Xl47Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_47_rope_split_concat() {
        let rope = super::Xl47Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_47_rope_line_count() {
        let rope = super::Xl47Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_47_rope_line_at() {
        let rope = super::Xl47Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_47_sa_build_and_search() {
        let sa = super::Xl47SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_47_sa_count() {
        let sa = super::Xl47SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_47_sa_longest_repeated() {
        let sa = super::Xl47SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_47_sa_all_positions() {
        let sa = super::Xl47SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_47_sa_len() {
        let sa = super::Xl47SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_47_sa_empty() {
        let sa = super::Xl47SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_47_rope_slice() {
        let rope = super::Xl47Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_47_sa_search_start() {
        let sa = super::Xl47SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_47_sparse_set_get() {
        let mut m = super::Xm47MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_47_sparse_row_col() {
        let mut m = super::Xm47MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_47_sparse_transpose() {
        let mut m = super::Xm47MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_47_sparse_multiply_vec() {
        let mut m = super::Xm47MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_47_sparse_nnz_density() {
        let mut m = super::Xm47MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_47_sparse_clear() {
        let mut m = super::Xm47MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_47_sparse_overwrite_zero() {
        let mut m = super::Xm47MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_47_tokenizer_basic() {
        let t = super::Xm47Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_47_tokenizer_count() {
        let t = super::Xm47Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_47_tokenizer_unique() {
        let t = super::Xm47Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_47_tokenizer_frequency() {
        let t = super::Xm47Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_47_tokenizer_delimiter() {
        let t = super::Xm47Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_47_tokenizer_whitespace() {
        let t = super::Xm47Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_47_tokenizer_empty() {
        let t = super::Xm47Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }

}
