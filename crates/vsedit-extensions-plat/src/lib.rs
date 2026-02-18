//! Extension manifest and schema.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionError {
    NotFound(String),
    AlreadyInstalled(String),
    InvalidManifest(String),
    DependencyMissing(String),
}

impl fmt::Display for ExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtensionError::NotFound(id) => write!(f, "extension not found: {id}"),
            ExtensionError::AlreadyInstalled(id) => write!(f, "extension already installed: {id}"),
            ExtensionError::InvalidManifest(msg) => write!(f, "invalid manifest: {msg}"),
            ExtensionError::DependencyMissing(dep) => write!(f, "missing dependency: {dep}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionKind {
    UI,
    Workspace,
    Web,
}

impl fmt::Display for ExtensionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtensionKind::UI => write!(f, "UI"),
            ExtensionKind::Workspace => write!(f, "Workspace"),
            ExtensionKind::Web => write!(f, "Web"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionIdentifier {
    pub id: String,
    pub version: String,
}

impl fmt::Display for ExtensionIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.id, self.version)
    }
}

impl ExtensionIdentifier {
    /// Parse an "id@version" string into an ExtensionIdentifier.
    pub fn parse(s: &str) -> Result<Self, ExtensionError> {
        let parts: Vec<&str> = s.splitn(2, '@').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(ExtensionError::InvalidManifest(format!(
                "expected 'id@version', got '{s}'"
            )));
        }
        Ok(Self {
            id: parts[0].to_string(),
            version: parts[1].to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ExtensionManifest {
    pub identifier: ExtensionIdentifier,
    pub name: String,
    pub publisher: String,
    pub description: Option<String>,
    pub kind: ExtensionKind,
    pub activation_events: Vec<String>,
    pub contributes: Vec<String>,
}

impl ExtensionManifest {
    /// Returns the full identifier as "publisher.id".
    pub fn full_id(&self) -> String {
        format!("{}.{}", self.publisher, self.identifier.id)
    }

    /// Checks whether the manifest lists a specific activation event.
    pub fn has_activation_event(&self, event: &str) -> bool {
        self.activation_events.iter().any(|e| e == event)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionStatus {
    Installed,
    Enabled,
    Disabled,
    Uninstalled,
}

impl fmt::Display for ExtensionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtensionStatus::Installed => write!(f, "Installed"),
            ExtensionStatus::Enabled => write!(f, "Enabled"),
            ExtensionStatus::Disabled => write!(f, "Disabled"),
            ExtensionStatus::Uninstalled => write!(f, "Uninstalled"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtensionEntry {
    pub manifest: ExtensionManifest,
    pub status: ExtensionStatus,
}

pub struct ExtensionService {
    extensions: Vec<ExtensionEntry>,
}

impl ExtensionService {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
        }
    }

    pub fn install(&mut self, manifest: ExtensionManifest) {
        self.extensions.push(ExtensionEntry {
            manifest,
            status: ExtensionStatus::Installed,
        });
    }

    pub fn enable(&mut self, id: &str) {
        if let Some(ext) = self.find_by_id_mut(id) {
            ext.status = ExtensionStatus::Enabled;
        }
    }

    pub fn disable(&mut self, id: &str) {
        if let Some(ext) = self.find_by_id_mut(id) {
            ext.status = ExtensionStatus::Disabled;
        }
    }

    pub fn uninstall(&mut self, id: &str) {
        if let Some(ext) = self.find_by_id_mut(id) {
            ext.status = ExtensionStatus::Uninstalled;
        }
    }

    pub fn get_enabled(&self) -> Vec<&ExtensionEntry> {
        self.extensions
            .iter()
            .filter(|e| e.status == ExtensionStatus::Enabled)
            .collect()
    }

    pub fn find_by_id(&self, id: &str) -> Option<&ExtensionEntry> {
        self.extensions
            .iter()
            .find(|e| e.manifest.identifier.id == id)
    }

    fn find_by_id_mut(&mut self, id: &str) -> Option<&mut ExtensionEntry> {
        self.extensions
            .iter_mut()
            .find(|e| e.manifest.identifier.id == id)
    }

    /// Install an extension, returning an error if it is already installed.
    pub fn try_install(&mut self, manifest: ExtensionManifest) -> Result<(), ExtensionError> {
        if self.find_by_id(&manifest.identifier.id).is_some() {
            return Err(ExtensionError::AlreadyInstalled(
                manifest.identifier.id.clone(),
            ));
        }
        self.install(manifest);
        Ok(())
    }

    /// Return all extensions matching the given status.
    pub fn get_by_status(&self, status: ExtensionStatus) -> Vec<&ExtensionEntry> {
        self.extensions
            .iter()
            .filter(|e| e.status == status)
            .collect()
    }

    /// Total number of extensions tracked by the service.
    pub fn count(&self) -> usize {
        self.extensions.len()
    }

    /// Search for extensions whose name contains the given substring (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&ExtensionEntry> {
        let q = query.to_lowercase();
        self.extensions
            .iter()
            .filter(|e| e.manifest.name.to_lowercase().contains(&q))
            .collect()
    }

    /// Return all extensions from the given publisher.
    pub fn get_by_publisher(&self, publisher: &str) -> Vec<&ExtensionEntry> {
        self.extensions
            .iter()
            .filter(|e| e.manifest.publisher == publisher)
            .collect()
    }
}

impl Default for ExtensionService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Additional extension platform utilities
// ---------------------------------------------------------------------------

/// Semantic version comparison result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionCompare {
    Older,
    Same,
    Newer,
}

/// Parse a semver-like "major.minor.patch" string into a tuple.
pub fn parse_version(v: &str) -> Result<(u32, u32, u32), ExtensionError> {
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 3 {
        return Err(ExtensionError::InvalidManifest(format!("invalid version: {v}")));
    }
    let major = parts[0].parse::<u32>().map_err(|_| ExtensionError::InvalidManifest(format!("bad major: {v}")))?;
    let minor = parts[1].parse::<u32>().map_err(|_| ExtensionError::InvalidManifest(format!("bad minor: {v}")))?;
    let patch = parts[2].parse::<u32>().map_err(|_| ExtensionError::InvalidManifest(format!("bad patch: {v}")))?;
    Ok((major, minor, patch))
}

/// Compare two semver strings.
pub fn compare_versions(a: &str, b: &str) -> Result<VersionCompare, ExtensionError> {
    let va = parse_version(a)?;
    let vb = parse_version(b)?;
    Ok(match va.cmp(&vb) {
        std::cmp::Ordering::Less => VersionCompare::Older,
        std::cmp::Ordering::Equal => VersionCompare::Same,
        std::cmp::Ordering::Greater => VersionCompare::Newer,
    })
}

impl ExtensionIdentifier {
    /// Check whether this version is compatible with (>=) a minimum version.
    pub fn is_compatible_with(&self, min_version: &str) -> Result<bool, ExtensionError> {
        let cmp = compare_versions(&self.version, min_version)?;
        Ok(matches!(cmp, VersionCompare::Same | VersionCompare::Newer))
    }
}

impl ExtensionManifest {
    /// Validate the manifest: id and name must be non-empty, version must parse.
    pub fn validate(&self) -> Result<(), ExtensionError> {
        if self.identifier.id.is_empty() {
            return Err(ExtensionError::InvalidManifest("id is empty".into()));
        }
        if self.name.is_empty() {
            return Err(ExtensionError::InvalidManifest("name is empty".into()));
        }
        if self.publisher.is_empty() {
            return Err(ExtensionError::InvalidManifest("publisher is empty".into()));
        }
        parse_version(&self.identifier.version)?;
        Ok(())
    }

    /// Return the number of contributions registered.
    pub fn contribution_count(&self) -> usize {
        self.contributes.len()
    }

    /// Whether the extension activates eagerly (has "*" activation event).
    pub fn is_eager(&self) -> bool {
        self.activation_events.iter().any(|e| e == "*")
    }
}

impl ExtensionService {
    /// Update an extension to a new version. Returns error if not found.
    pub fn update_version(&mut self, id: &str, new_version: &str) -> Result<(), ExtensionError> {
        let ext = self.find_by_id_mut(id).ok_or_else(|| ExtensionError::NotFound(id.into()))?;
        parse_version(new_version)?;
        ext.manifest.identifier.version = new_version.to_string();
        Ok(())
    }

    /// Return all extensions sorted by name (case-insensitive).
    pub fn sorted_by_name(&self) -> Vec<&ExtensionEntry> {
        let mut result: Vec<&ExtensionEntry> = self.extensions.iter().collect();
        result.sort_by(|a, b| {
            a.manifest.name.to_lowercase().cmp(&b.manifest.name.to_lowercase())
        });
        result
    }

    /// Return all extensions of a given kind.
    pub fn get_by_kind(&self, kind: ExtensionKind) -> Vec<&ExtensionEntry> {
        self.extensions.iter().filter(|e| e.manifest.kind == kind).collect()
    }

    /// Check whether any installed extension has the given activation event.
    pub fn has_any_with_event(&self, event: &str) -> bool {
        self.extensions.iter().any(|e| e.manifest.has_activation_event(event))
    }

    /// Remove all extensions with Uninstalled status, returning the count removed.
    pub fn purge_uninstalled(&mut self) -> usize {
        let before = self.extensions.len();
        self.extensions.retain(|e| e.status != ExtensionStatus::Uninstalled);
        before - self.extensions.len()
    }
}

impl fmt::Display for ExtensionEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}] ({})", self.manifest.full_id(), self.manifest.identifier.version, self.status)
    }
}

/// Check if a version string matches the X.Y.Z pattern where each component is a number.
pub fn validate_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Validator for extension manifests, returning all validation errors at once.
pub struct ExtensionManifestValidator;

impl ExtensionManifestValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate(&self, manifest: &ExtensionManifest) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if manifest.identifier.id.is_empty() {
            errors.push("id must not be empty".to_string());
        } else if !manifest.identifier.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            errors.push("id must contain only lowercase alphanumeric characters and hyphens".to_string());
        }
        if manifest.name.is_empty() {
            errors.push("name must not be empty".to_string());
        }
        if manifest.publisher.is_empty() {
            errors.push("publisher must not be empty".to_string());
        }
        if !validate_semver(&manifest.identifier.version) {
            errors.push("version must match X.Y.Z pattern".to_string());
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}

impl Default for ExtensionManifestValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationEvent {
    OnLanguage(String),
    OnCommand(String),
    OnFileSystem(String),
    OnStartupFinished,
    Star,
    OnView(String),
    WorkspaceContains(String),
}

impl ActivationEvent {
    pub fn parse(s: &str) -> Option<Self> {
        if s == "*" {
            return Some(ActivationEvent::Star);
        }
        if s == "onStartupFinished" {
            return Some(ActivationEvent::OnStartupFinished);
        }
        if let Some(rest) = s.strip_prefix("onLanguage:") {
            return Some(ActivationEvent::OnLanguage(rest.to_string()));
        }
        if let Some(rest) = s.strip_prefix("onCommand:") {
            return Some(ActivationEvent::OnCommand(rest.to_string()));
        }
        if let Some(rest) = s.strip_prefix("onFileSystem:") {
            return Some(ActivationEvent::OnFileSystem(rest.to_string()));
        }
        if let Some(rest) = s.strip_prefix("onView:") {
            return Some(ActivationEvent::OnView(rest.to_string()));
        }
        if let Some(rest) = s.strip_prefix("workspaceContains:") {
            return Some(ActivationEvent::WorkspaceContains(rest.to_string()));
        }
        None
    }
}

/// Check if a manifest's activation_events list matches the given event.
pub fn extension_activate_by_event(manifest: &ExtensionManifest, event: &ActivationEvent) -> bool {
    manifest.activation_events.iter().any(|raw| {
        ActivationEvent::parse(raw).as_ref() == Some(event)
    })
}

/// Resolves extension dependency ordering via topological sort.
pub struct ExtensionDependencyResolver {
    extensions: Vec<(String, Vec<String>)>,
}

impl ExtensionDependencyResolver {
    pub fn new() -> Self {
        Self { extensions: Vec::new() }
    }

    pub fn add_extension(&mut self, id: impl Into<String>, deps: Vec<String>) {
        self.extensions.push((id.into(), deps));
    }

    pub fn has_extension(&self, id: &str) -> bool {
        self.extensions.iter().any(|(eid, _)| eid == id)
    }

    pub fn resolve_order(&self) -> Result<Vec<String>, ExtensionError> {
        use std::collections::{HashMap, HashSet, VecDeque};

        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
        let known: HashSet<&str> = self.extensions.iter().map(|(id, _)| id.as_str()).collect();

        for (id, _) in &self.extensions {
            in_degree.entry(id.as_str()).or_insert(0);
        }

        for (id, deps) in &self.extensions {
            for dep in deps {
                if !known.contains(dep.as_str()) {
                    return Err(ExtensionError::DependencyMissing(dep.clone()));
                }
                dependents.entry(dep.as_str()).or_default().push(id.as_str());
                *in_degree.entry(id.as_str()).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<&str> = in_degree.iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(&id, _)| id)
            .collect();
        let mut result = Vec::new();

        while let Some(id) = queue.pop_front() {
            result.push(id.to_string());
            if let Some(deps) = dependents.get(id) {
                for &dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep);
                        }
                    }
                }
            }
        }

        if result.len() != self.extensions.len() {
            return Err(ExtensionError::DependencyMissing("circular dependency detected".to_string()));
        }

        Ok(result)
    }
}

impl Default for ExtensionDependencyResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for extensions-plat operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionsPlatStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtensionsPlatStats {
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
    pub fn merge(&mut self, other: &ExtensionsPlatStats) {
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

impl Default for ExtensionsPlatStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtensionsPlatStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtensionsPlatStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for extensions-plat.
#[derive(Debug, Clone)]
pub struct ExtensionsPlatValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtensionsPlatValidator {
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

impl Default for ExtensionsPlatValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ExtensionDependencyGraph – dependency tracking & topological sort
// ---------------------------------------------------------------------------

/// Directed graph tracking dependencies between extensions.
#[derive(Debug, Clone)]
pub struct ExtensionDependencyGraph {
    /// adjacency list: extension id -> set of ids it depends on
    deps: std::collections::HashMap<String, Vec<String>>,
}

impl ExtensionDependencyGraph {
    pub fn new() -> Self {
        Self {
            deps: std::collections::HashMap::new(),
        }
    }

    /// Register an extension (no dependencies yet).
    pub fn add_extension(&mut self, id: impl Into<String>) {
        self.deps.entry(id.into()).or_default();
    }

    /// Declare that `ext` depends on `dependency`.
    pub fn add_dependency(&mut self, ext: impl Into<String>, dependency: impl Into<String>) {
        let ext = ext.into();
        let dep = dependency.into();
        self.deps.entry(dep.clone()).or_default();
        self.deps.entry(ext.clone()).or_default().push(dep);
    }

    /// All direct dependencies for `ext`.
    pub fn direct_deps(&self, ext: &str) -> Vec<&str> {
        self.deps
            .get(ext)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Transitive (recursive) dependencies for `ext`.
    pub fn transitive_deps(&self, ext: &str) -> Vec<String> {
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![ext.to_string()];
        while let Some(current) = stack.pop() {
            if let Some(neighbors) = self.deps.get(&current) {
                for n in neighbors {
                    if visited.insert(n.clone()) {
                        stack.push(n.clone());
                    }
                }
            }
        }
        let mut result: Vec<String> = visited.into_iter().collect();
        result.sort();
        result
    }

    /// Number of registered extensions.
    pub fn extension_count(&self) -> usize {
        self.deps.len()
    }

    /// Check if adding `dependency` to `ext` would create a cycle.
    pub fn would_create_cycle(&self, ext: &str, dependency: &str) -> bool {
        if ext == dependency {
            return true;
        }
        // Does `dependency` transitively depend on `ext`?
        self.transitive_deps(dependency).contains(&ext.to_string())
    }

    /// Topological sort (Kahn's algorithm). Returns `Err` with a cycle participant
    /// if the graph has a cycle.
    pub fn topological_sort(&self) -> Result<Vec<String>, ExtensionError> {
        let mut in_degree: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (node, neighbors) in &self.deps {
            in_degree.entry(node.as_str()).or_insert(0);
            for n in neighbors {
                *in_degree.entry(n.as_str()).or_insert(0) += 1;
            }
        }

        // Note: in this graph, edges go FROM dependent TO dependency.
        // "in_degree" here actually counts how many extensions depend on a node.
        // For load ordering we want dependencies first, so we reverse the edge direction.
        let mut in_deg: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (node, _) in &self.deps {
            in_deg.entry(node.as_str()).or_insert(0);
        }
        for (_, neighbors) in &self.deps {
            for n in neighbors {
                // n is a dependency – it has an outgoing reverse-edge to the dependent
                in_deg.entry(n.as_str()).or_insert(0);
            }
        }
        // Reverse direction: for each ext->dep edge, dep must come first.
        // So in reverse graph dep->ext and in_degree of ext increases.
        let mut reverse_in: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        let mut reverse_adj: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
        for (node, _) in &self.deps {
            reverse_in.entry(node.as_str()).or_insert(0);
            reverse_adj.entry(node.as_str()).or_default();
        }
        for (ext, neighbors) in &self.deps {
            for dep in neighbors {
                reverse_adj.entry(dep.as_str()).or_default().push(ext.as_str());
                *reverse_in.entry(ext.as_str()).or_insert(0) += 1;
            }
        }

        let mut queue: std::collections::VecDeque<&str> = std::collections::VecDeque::new();
        for (&node, &deg) in &reverse_in {
            if deg == 0 {
                queue.push_back(node);
            }
        }

        let mut order: Vec<String> = Vec::new();
        while let Some(node) = queue.pop_front() {
            order.push(node.to_string());
            if let Some(neighbors) = reverse_adj.get(node) {
                for &n in neighbors {
                    let d = reverse_in.get_mut(n).unwrap();
                    *d -= 1;
                    if *d == 0 {
                        queue.push_back(n);
                    }
                }
            }
        }

        if order.len() != self.deps.len() {
            return Err(ExtensionError::DependencyMissing(
                "cycle detected in extension dependencies".to_string(),
            ));
        }
        Ok(order)
    }

    /// Check whether all declared dependencies actually exist in the graph.
    pub fn check_missing(&self) -> Vec<String> {
        let known: std::collections::HashSet<&str> =
            self.deps.keys().map(|s| s.as_str()).collect();
        let mut missing = Vec::new();
        for neighbors in self.deps.values() {
            for n in neighbors {
                if !known.contains(n.as_str()) {
                    missing.push(n.clone());
                }
            }
        }
        missing.sort();
        missing.dedup();
        missing
    }

    /// Extensions that nothing else depends on (leaf nodes).
    pub fn leaf_extensions(&self) -> Vec<&str> {
        let mut depended_on: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for neighbors in self.deps.values() {
            for n in neighbors {
                depended_on.insert(n.as_str());
            }
        }
        let mut leaves: Vec<&str> = self
            .deps
            .keys()
            .map(|s| s.as_str())
            .filter(|s| !depended_on.contains(s))
            .collect();
        leaves.sort();
        leaves
    }

    /// Extensions that have no dependencies themselves (root nodes).
    pub fn root_extensions(&self) -> Vec<&str> {
        let mut roots: Vec<&str> = self
            .deps
            .iter()
            .filter(|(_, v)| v.is_empty())
            .map(|(k, _)| k.as_str())
            .collect();
        roots.sort();
        roots
    }
}

impl Default for ExtensionDependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtensionDependencyGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExtensionDependencyGraph({} extensions)", self.deps.len())
    }
}

// ---------------------------------------------------------------------------
// Extension compatibility checking
// ---------------------------------------------------------------------------

/// Semantic version range for compatibility checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRange {
    pub min_major: u32,
    pub min_minor: u32,
    pub min_patch: u32,
    pub max_major: Option<u32>,
}

impl VersionRange {
    pub fn at_least(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            min_major: major,
            min_minor: minor,
            min_patch: patch,
            max_major: None,
        }
    }

    pub fn compatible_major(major: u32) -> Self {
        Self {
            min_major: major,
            min_minor: 0,
            min_patch: 0,
            max_major: Some(major),
        }
    }

    /// Does the given version string satisfy this range?
    pub fn satisfies(&self, version: &str) -> Result<bool, ExtensionError> {
        let (maj, min, pat) = parse_version(version)?;
        if let Some(max) = self.max_major {
            if maj != max {
                return Ok(false);
            }
        }
        if maj < self.min_major {
            return Ok(false);
        }
        if maj == self.min_major && min < self.min_minor {
            return Ok(false);
        }
        if maj == self.min_major && min == self.min_minor && pat < self.min_patch {
            return Ok(false);
        }
        Ok(true)
    }
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ">={}.{}.{}", self.min_major, self.min_minor, self.min_patch)?;
        if let Some(max) = self.max_major {
            write!(f, ", <{}", max + 1)?;
        }
        Ok(())
    }
}

/// Check compatibility between an extension and the host engine.
#[derive(Debug, Clone)]
pub struct CompatibilityChecker {
    engine_version: String,
}

impl CompatibilityChecker {
    pub fn new(engine_version: impl Into<String>) -> Self {
        Self {
            engine_version: engine_version.into(),
        }
    }

    pub fn engine_version(&self) -> &str {
        &self.engine_version
    }

    /// Check if a manifest is compatible with the current engine.
    pub fn is_compatible(&self, manifest: &ExtensionManifest) -> Result<bool, ExtensionError> {
        manifest.identifier.is_compatible_with(&self.engine_version)
    }

    /// Filter a list of manifests to only compatible ones.
    pub fn filter_compatible<'a>(
        &self,
        manifests: &[&'a ExtensionManifest],
    ) -> Vec<&'a ExtensionManifest> {
        manifests
            .iter()
            .filter(|m| self.is_compatible(m).unwrap_or(false))
            .copied()
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Extension recommendation engine
// ---------------------------------------------------------------------------

/// A recommendation for an extension based on file types in the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionRecommendation {
    pub extension_id: String,
    pub reason: String,
    pub file_pattern: String,
    pub priority: u8,
}

impl ExtensionRecommendation {
    pub fn new(
        extension_id: impl Into<String>,
        reason: impl Into<String>,
        file_pattern: impl Into<String>,
        priority: u8,
    ) -> Self {
        Self {
            extension_id: extension_id.into(),
            reason: reason.into(),
            file_pattern: file_pattern.into(),
            priority,
        }
    }
}

impl fmt::Display for ExtensionRecommendation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (priority {}) – {}",
            self.extension_id, self.priority, self.reason
        )
    }
}

/// Recommends extensions based on file patterns present in the workspace.
#[derive(Debug, Clone, Default)]
pub struct ExtensionRecommender {
    rules: Vec<(String, ExtensionRecommendation)>,
}

impl ExtensionRecommender {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a recommendation rule: when a file matching `file_ext`
    /// is present, suggest the given extension.
    pub fn add_rule(&mut self, file_ext: impl Into<String>, rec: ExtensionRecommendation) {
        self.rules.push((file_ext.into(), rec));
    }

    /// Given a list of file extensions present in the workspace, return
    /// matching recommendations sorted by priority (highest first).
    pub fn recommend(&self, present_extensions: &[&str]) -> Vec<&ExtensionRecommendation> {
        let mut recs: Vec<&ExtensionRecommendation> = self
            .rules
            .iter()
            .filter(|(ext, _)| present_extensions.contains(&ext.as_str()))
            .map(|(_, rec)| rec)
            .collect();
        recs.sort_by(|a, b| b.priority.cmp(&a.priority));
        recs
    }

    /// Return all unique extension IDs that would be recommended.
    pub fn recommended_ids(&self, present_extensions: &[&str]) -> Vec<String> {
        let mut ids: Vec<String> = self
            .recommend(present_extensions)
            .iter()
            .map(|r| r.extension_id.clone())
            .collect();
        ids.dedup();
        ids
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl fmt::Display for ExtensionRecommender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExtensionRecommender({} rules)", self.rules.len())
    }
}

/// Summarizes the status of all extensions in the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionStatusSummary {
    pub installed: usize,
    pub enabled: usize,
    pub disabled: usize,
    pub uninstalled: usize,
}

impl ExtensionStatusSummary {
    /// Build a summary from an `ExtensionService`.
    pub fn from_service(service: &ExtensionService) -> Self {
        Self {
            installed: service.get_by_status(ExtensionStatus::Installed).len(),
            enabled: service.get_by_status(ExtensionStatus::Enabled).len(),
            disabled: service.get_by_status(ExtensionStatus::Disabled).len(),
            uninstalled: service.get_by_status(ExtensionStatus::Uninstalled).len(),
        }
    }

    pub fn total(&self) -> usize {
        self.installed + self.enabled + self.disabled + self.uninstalled
    }

    pub fn active(&self) -> usize {
        self.installed + self.enabled
    }
}

impl fmt::Display for ExtensionStatusSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "installed={}, enabled={}, disabled={}, uninstalled={}",
            self.installed, self.enabled, self.disabled, self.uninstalled
        )
    }
}

// ---------------------------------------------------------------------------
// ExtensionValidateScan – checking extension manifests
// ---------------------------------------------------------------------------

/// Result of scanning/validating an extension manifest.
#[derive(Debug, Clone)]
pub struct ManifestScanResult {
    pub extension_id: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ManifestScanResult {
    /// Whether the manifest is valid (no errors).
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Total number of issues (errors + warnings).
    pub fn issue_count(&self) -> usize {
        self.errors.len() + self.warnings.len()
    }
}

/// Scans and validates extension manifests against platform rules.
pub struct ExtensionValidateScan {
    required_fields: Vec<String>,
    max_name_length: usize,
}

impl ExtensionValidateScan {
    /// Create a scanner with default rules.
    pub fn new() -> Self {
        Self {
            required_fields: vec![
                "name".into(), "publisher".into(), "version".into(),
            ],
            max_name_length: 214,
        }
    }

    /// Scan a manifest and return validation results.
    pub fn scan(&self, manifest: &ExtensionManifest) -> ManifestScanResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if manifest.name.is_empty() {
            errors.push("name must not be empty".into());
        }
        if manifest.publisher.is_empty() {
            errors.push("publisher must not be empty".into());
        }
        if manifest.identifier.version.is_empty() {
            errors.push("version must not be empty".into());
        }
        if manifest.name.len() > self.max_name_length {
            errors.push(format!("name exceeds max length of {}", self.max_name_length));
        }
        if !validate_semver(&manifest.identifier.version) {
            warnings.push("version is not valid semver".into());
        }
        if manifest.activation_events.is_empty() {
            warnings.push("no activation events declared".into());
        }

        ManifestScanResult {
            extension_id: manifest.full_id(),
            errors,
            warnings,
        }
    }

    /// Scan multiple manifests.
    pub fn scan_all(&self, manifests: &[ExtensionManifest]) -> Vec<ManifestScanResult> {
        manifests.iter().map(|m| self.scan(m)).collect()
    }

    /// Set max name length.
    pub fn set_max_name_length(&mut self, max: usize) {
        self.max_name_length = max;
    }
}

// ---------------------------------------------------------------------------
// ExtensionStorageQuota – per-extension storage management
// ---------------------------------------------------------------------------

/// Manages per-extension storage quotas.
pub struct ExtensionStorageQuota {
    quotas: std::collections::HashMap<String, (u64, u64)>, // (used, max)
    default_max: u64,
}

impl ExtensionStorageQuota {
    /// Create a quota manager with a default max per extension (in bytes).
    pub fn new(default_max_bytes: u64) -> Self {
        Self {
            quotas: std::collections::HashMap::new(),
            default_max: default_max_bytes,
        }
    }

    /// Record storage usage for an extension.
    pub fn set_usage(&mut self, ext_id: &str, used_bytes: u64) {
        let entry = self.quotas.entry(ext_id.to_string()).or_insert((0, self.default_max));
        entry.0 = used_bytes;
    }

    /// Set a custom quota for an extension.
    pub fn set_quota(&mut self, ext_id: &str, max_bytes: u64) {
        let entry = self.quotas.entry(ext_id.to_string()).or_insert((0, self.default_max));
        entry.1 = max_bytes;
    }

    /// Check if an extension has exceeded its quota.
    pub fn is_over_quota(&self, ext_id: &str) -> bool {
        self.quotas.get(ext_id).map(|(used, max)| used > max).unwrap_or(false)
    }

    /// Get remaining bytes for an extension.
    pub fn remaining_bytes(&self, ext_id: &str) -> u64 {
        self.quotas.get(ext_id)
            .map(|(used, max)| max.saturating_sub(*used))
            .unwrap_or(self.default_max)
    }

    /// Get usage percentage for an extension.
    pub fn usage_percent(&self, ext_id: &str) -> f64 {
        self.quotas.get(ext_id)
            .map(|(used, max)| if *max == 0 { 100.0 } else { (*used as f64 / *max as f64) * 100.0 })
            .unwrap_or(0.0)
    }

    /// List extensions over a given usage percentage.
    pub fn extensions_over_percent(&self, threshold: f64) -> Vec<&str> {
        self.quotas.iter()
            .filter(|(_, (used, max))| {
                if *max == 0 { return true; }
                (*used as f64 / *max as f64) * 100.0 > threshold
            })
            .map(|(id, _)| id.as_str())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ExtensionToggle – enable/disable toggle with reason tracking
// ---------------------------------------------------------------------------

/// Reason why an extension was disabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisableReason {
    /// User manually disabled.
    User,
    /// Disabled due to workspace trust.
    WorkspaceTrust,
    /// Disabled due to compatibility issue.
    Compatibility,
    /// Disabled due to dependency missing.
    DependencyMissing,
}

/// Tracks enable/disable state with reason for each extension.
pub struct ExtensionToggle {
    disabled: std::collections::HashMap<String, DisableReason>,
}

impl ExtensionToggle {
    /// Create a new toggle tracker.
    pub fn new() -> Self {
        Self { disabled: std::collections::HashMap::new() }
    }

    /// Disable an extension with a reason.
    pub fn disable(&mut self, ext_id: impl Into<String>, reason: DisableReason) {
        self.disabled.insert(ext_id.into(), reason);
    }

    /// Enable an extension (remove disable record).
    pub fn enable(&mut self, ext_id: &str) -> bool {
        self.disabled.remove(ext_id).is_some()
    }

    /// Check if an extension is disabled.
    pub fn is_disabled(&self, ext_id: &str) -> bool {
        self.disabled.contains_key(ext_id)
    }

    /// Get the disable reason for an extension.
    pub fn disable_reason(&self, ext_id: &str) -> Option<&DisableReason> {
        self.disabled.get(ext_id)
    }

    /// Count of disabled extensions.
    pub fn disabled_count(&self) -> usize {
        self.disabled.len()
    }

    /// List all extensions disabled for a specific reason.
    pub fn disabled_by_reason(&self, reason: &DisableReason) -> Vec<&str> {
        self.disabled.iter()
            .filter(|(_, r)| *r == reason)
            .map(|(id, _)| id.as_str())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ExtensionStartupTracker – profiles extension activation times
// ---------------------------------------------------------------------------

/// Tracks activation duration for each extension to identify slow activations.
pub struct ExtensionStartupTracker {
    activations: std::collections::HashMap<String, u64>,
}

impl ExtensionStartupTracker {
    pub fn new() -> Self {
        Self {
            activations: std::collections::HashMap::new(),
        }
    }

    /// Record activation time in milliseconds for an extension.
    pub fn record_activation(&mut self, ext_id: &str, duration_ms: u64) {
        self.activations.insert(ext_id.to_string(), duration_ms);
    }

    /// Get activation time for a specific extension.
    pub fn activation_time(&self, ext_id: &str) -> Option<u64> {
        self.activations.get(ext_id).copied()
    }

    /// Return the `n` slowest extensions sorted by descending activation time.
    pub fn slowest(&self, n: usize) -> Vec<(String, u64)> {
        let mut entries: Vec<(String, u64)> = self
            .activations
            .iter()
            .map(|(id, &ms)| (id.clone(), ms))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(n);
        entries
    }

    /// Sum of all recorded activation times.
    pub fn total_activation_time(&self) -> u64 {
        self.activations.values().sum()
    }

    /// Average activation time across all recorded extensions.
    pub fn average_activation_time(&self) -> f64 {
        if self.activations.is_empty() {
            return 0.0;
        }
        self.total_activation_time() as f64 / self.activations.len() as f64
    }

    /// Number of extensions with recorded activation times.
    pub fn extension_count(&self) -> usize {
        self.activations.len()
    }
}

// ---------------------------------------------------------------------------
// ExtensionResourceLoader – tracks bundled assets for extensions
// ---------------------------------------------------------------------------

/// Metadata for a single bundled resource file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceInfo {
    pub path: String,
    pub size_bytes: u64,
}

/// Manages the set of bundled resources belonging to an extension.
pub struct ExtensionResourceLoader {
    ext_id: String,
    resources: std::collections::HashMap<String, ResourceInfo>,
}

impl ExtensionResourceLoader {
    pub fn new(ext_id: &str) -> Self {
        Self {
            ext_id: ext_id.to_string(),
            resources: std::collections::HashMap::new(),
        }
    }

    /// Register a resource by its relative path and size.
    pub fn register_resource(&mut self, relative_path: &str, size_bytes: u64) {
        self.resources.insert(
            relative_path.to_string(),
            ResourceInfo {
                path: relative_path.to_string(),
                size_bytes,
            },
        );
    }

    /// Look up a resource by relative path.
    pub fn get_resource(&self, relative_path: &str) -> Option<&ResourceInfo> {
        self.resources.get(relative_path)
    }

    /// Total size in bytes of all registered resources.
    pub fn total_size(&self) -> u64 {
        self.resources.values().map(|r| r.size_bytes).sum()
    }

    /// Number of registered resources.
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// List relative paths of all registered resources.
    pub fn resources(&self) -> Vec<&str> {
        self.resources.keys().map(|k| k.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// ExtensionConfigDefault – manages extension configuration contributions
// ---------------------------------------------------------------------------

struct ConfigEntry {
    key: String,
    value: String,
    description: String,
}

/// Stores default configuration values contributed by an extension.
pub struct ExtensionConfigDefault {
    ext_id: String,
    entries: Vec<ConfigEntry>,
}

impl ExtensionConfigDefault {
    pub fn new(ext_id: &str) -> Self {
        Self {
            ext_id: ext_id.to_string(),
            entries: Vec::new(),
        }
    }

    /// Add a default configuration entry. If the key already exists it is overwritten.
    pub fn add_default(&mut self, key: &str, value: &str, description: &str) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.key == key) {
            existing.value = value.to_string();
            existing.description = description.to_string();
        } else {
            self.entries.push(ConfigEntry {
                key: key.to_string(),
                value: value.to_string(),
                description: description.to_string(),
            });
        }
    }

    /// Get the default value for a configuration key.
    pub fn get_default(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.key == key)
            .map(|e| e.value.as_str())
    }

    /// Return all defaults as `(key, value, description)` tuples.
    pub fn defaults(&self) -> Vec<(&str, &str, &str)> {
        self.entries
            .iter()
            .map(|e| (e.key.as_str(), e.value.as_str(), e.description.as_str()))
            .collect()
    }

    /// Number of registered defaults.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Check whether a key is registered.
    pub fn has_key(&self, key: &str) -> bool {
        self.entries.iter().any(|e| e.key == key)
    }
}

// ---------------------------------------------------------------------------
// ExtensionPackExpander – expands extension packs to individual extensions
// ---------------------------------------------------------------------------

/// Represents an extension pack and its member extensions.
pub struct ExtensionPackExpander {
    pack_id: String,
    members: Vec<String>,
}

impl ExtensionPackExpander {
    pub fn new(pack_id: &str) -> Self {
        Self {
            pack_id: pack_id.to_string(),
            members: Vec::new(),
        }
    }

    /// Add a member extension to the pack.
    pub fn add_member(&mut self, ext_id: &str) {
        if !self.members.iter().any(|m| m == ext_id) {
            self.members.push(ext_id.to_string());
        }
    }

    /// Ordered list of member extension IDs.
    pub fn members(&self) -> &[String] {
        &self.members
    }

    /// Check whether the pack contains a specific extension.
    pub fn contains(&self, ext_id: &str) -> bool {
        self.members.iter().any(|m| m == ext_id)
    }

    /// Number of member extensions.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// The pack's own identifier.
    pub fn pack_id(&self) -> &str {
        &self.pack_id
    }
}

impl fmt::Display for ExtensionPackExpander {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pack {} ({} members: {})",
            self.pack_id,
            self.members.len(),
            self.members.join(", ")
        )
    }
}


// ─── ExtPBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for extension events.
#[derive(Debug, Clone)]
pub struct ExtPBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> ExtPBufRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for ExtPBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExtPBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── ExtPFmt Formatter ───────────────────────────────────────

/// Formatting options for extension platform output.
#[derive(Debug, Clone)]
pub struct ExtPFmtFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for ExtPFmtFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl ExtPFmtFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for extension platform data.
pub struct ExtPFmtFmt {
    options: ExtPFmtFmtOpts,
}

impl ExtPFmtFmt {
    pub fn new(options: ExtPFmtFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: ExtPFmtFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}


// ---------------------------------------------------------------------------
// extensions_plat – Platform service helpers
// ---------------------------------------------------------------------------

/// Capability flags for platform feature detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XExtensionsPlatCapabilities {
    flags: std::collections::HashSet<String>,
}

impl XExtensionsPlatCapabilities {
    pub fn new() -> Self {
        Self { flags: std::collections::HashSet::new() }
    }

    pub fn register(&mut self, cap: impl Into<String>) {
        self.flags.insert(cap.into());
    }

    pub fn has(&self, cap: &str) -> bool {
        self.flags.contains(cap)
    }

    pub fn len(&self) -> usize {
        self.flags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flags.is_empty()
    }

    /// Return the intersection with another capability set.
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            flags: self.flags.intersection(&other.flags).cloned().collect(),
        }
    }

    /// Return capabilities present here but not in `other`.
    pub fn diff(&self, other: &Self) -> Self {
        Self {
            flags: self.flags.difference(&other.flags).cloned().collect(),
        }
    }

    pub fn all(&self) -> Vec<&str> {
        self.flags.iter().map(|s| s.as_str()).collect()
    }
}

impl Default for XExtensionsPlatCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple service registry keyed by name.
#[derive(Debug, Default)]
pub struct XExtensionsPlatServiceRegistry {
    services: std::collections::HashMap<String, String>,
}

impl XExtensionsPlatServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a service. Returns the previous value if the key was already present.
    pub fn register(&mut self, name: impl Into<String>, descriptor: impl Into<String>) -> Option<String> {
        self.services.insert(name.into(), descriptor.into())
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.services.get(name).map(|s| s.as_str())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.services.contains_key(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<String> {
        self.services.remove(name)
    }

    pub fn len(&self) -> usize {
        self.services.len()
    }

    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.services.keys().map(|s| s.as_str()).collect()
    }
}

/// Sanitize a path-like string by collapsing repeated separators and removing trailing ones.
pub fn x_extensions_plat_sanitize_path(p: &str) -> String {
    let mut result = String::with_capacity(p.len());
    let mut last_was_sep = false;
    for ch in p.chars() {
        if ch == '/' || ch == '\\' {
            if !last_was_sep {
                result.push('/');
            }
            last_was_sep = true;
        } else {
            result.push(ch);
            last_was_sep = false;
        }
    }
    if result.len() > 1 && result.ends_with('/') {
        result.pop();
    }
    result
}



// ---------------------------------------------------------------------------
// extensions_plat – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for extensions platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YExtensionsPlatExtensionActivation {
    OnLoad,
    OnCommand,
    OnLanguage,
    OnView,
}

impl YExtensionsPlatExtensionActivation {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::OnLoad => 0,
            Self::OnCommand => 1,
            Self::OnLanguage => 2,
            Self::OnView => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::OnLoad => "OnLoad",
            Self::OnCommand => "OnCommand",
            Self::OnLanguage => "OnLanguage",
            Self::OnView => "OnView",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YExtensionsPlatExtensionActivation] {
        &[
            YExtensionsPlatExtensionActivation::OnLoad,
            YExtensionsPlatExtensionActivation::OnCommand,
            YExtensionsPlatExtensionActivation::OnLanguage,
            YExtensionsPlatExtensionActivation::OnView,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YExtensionsPlatExtensionActivation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks extension manifest data.
#[derive(Debug, Clone)]
pub struct YExtensionsPlatExtensionManifest {
    pub id: String,
    pub version: String,
    pub dependencies: Vec<String>,
}

impl YExtensionsPlatExtensionManifest {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            id: String::new(),
            version: String::new(),
            dependencies: Vec::new(),
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.dependencies.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.dependencies.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.dependencies.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YExtensionsPlatExtensionManifest({}: {:?})", "id", self.id)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_extensions_plat_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_extensions_plat_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_extensions_plat_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_extensions_plat_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_extensions_plat_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_extensions_plat_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_extensions_plat_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_extensions_plat_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// extensions_plat – Extended extension sandbox helpers
// ---------------------------------------------------------------------------

/// Priority levels for extension sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZExtensionsPlatPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZExtensionsPlatPriority {
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
    pub fn all_asc() -> [ZExtensionsPlatPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZExtensionsPlatPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks extension sandbox data.
#[derive(Debug, Clone)]
pub struct ZExtensionsPlatExtensionSandbox {
    pub permissions: Vec<String>,
    pub memory_limit_mb: u32,
    pub isolated: bool,
}

impl ZExtensionsPlatExtensionSandbox {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            permissions: Vec::new(),
            memory_limit_mb: 0,
            isolated: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.permissions.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.permissions.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.permissions.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZExtensionsPlatExtensionSandbox[memory_limit_mb={:?}, isolated={:?}]", self.memory_limit_mb, self.isolated)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.isolated = !c.isolated;
        c
    }
}

/// Compute a simple rolling hash for extension sandbox.
pub fn z_extensions_plat_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_extensions_plat_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_extensions_plat_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_extensions_plat_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_extensions_plat_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_extensions_plat_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_extensions_plat_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 89
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer89 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer89 {
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
pub fn xb_fnv1a_89(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_89<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_89<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_89(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_89(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 80
// ---------------------------------------------------------------------------

/// Generic object pool `Xc80Pool<T>`.
pub struct Xc80Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc80Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc80PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc80Pool<T> {
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
    pub fn stats(&self) -> Xc80PoolStats {
        Xc80PoolStats {
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

impl<T> Default for Xc80Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc80Scheduler`.
pub struct Xc80Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc80Scheduler {
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

impl Default for Xc80Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_80 hash for the given byte slice.
pub fn xc_80_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_80 convention.
pub fn xc_80_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe102 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe102Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe102PipelineError {
    pub stage: Xe102Stage,
    pub message: String,
}

impl std::fmt::Display for Xe102PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe102Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe102Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe102PipelineError>>>,
    stage_names: Vec<Xe102Stage>,
}

impl Xe102Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe102PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe102Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe102PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe102Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe102PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe102Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe102PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe102Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe102PipelineError> {
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

    pub fn compose(mut self, other: Xe102Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe102CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe102CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe102Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe102CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe102CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe102Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe102CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_102_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe102CacheEntry {
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

    fn xe_102_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe102CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_102_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe102PipelineError> {
    Ok(data)
}

pub fn xe_102_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe102PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_102_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe102PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_102_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe102PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_102_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe102PipelineError> {
    Err(Xe102PipelineError {
        stage: Xe102Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_100: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg100Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg100Graph {
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

impl Default for Xg100Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_100: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg100Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg100Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg100Heap<T>) {
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

impl<T: Ord> Default for Xg100Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 79).
pub struct Xh79SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh79SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 121 as u64,
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

/// A compact bit set supporting boolean operations (variant 79).
pub struct Xh79BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh79BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 79).
pub struct Xi79Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi79Deque<T> {
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
pub struct Xi79Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi79Interval {
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

/// A simple interval tree (variant 79).
pub struct Xi79IntervalTree {
    xi_intervals: Vec<Xi79Interval>,
}

impl Xi79IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi79Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi79Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi79Interval) -> Vec<&Xi79Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi79Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi79Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi79Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi79Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi79Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi79Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 58) ---

/// Disjoint set / union-find for crate 58.
pub struct Xj58UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj58UnionFind {
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

const XJ58_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 58.
pub struct Xj58BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj58BTreeNode<K, V>>>,
    len: usize,
}

struct Xj58BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj58BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj58BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ58_BTREE_ORDER - 1
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
        let mid = XJ58_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj58BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj58BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj58BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj58BTreeNode::xj_new_leaf();
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


// --- xk_79 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk79SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk79SegmentTree {
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
pub struct Xk79DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk79DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_58).
#[derive(Debug, Clone)]
pub struct Xl58Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl58Rope {
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

/// Suffix array for efficient string searching (xl_58).
#[derive(Debug, Clone)]
pub struct Xl58SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl58SuffixArray {
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
pub struct Xm58MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm58MatrixSparse {
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
pub struct Xm58Tokenizer {
    text: String,
}

impl Xm58Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 79.
pub struct Xn79Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn79Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 79 -----

#[derive(Debug, Clone)]
struct Xn79AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn79AvlNode<K, V>>>,
    right: Option<Box<Xn79AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 79.
#[derive(Debug, Clone)]
pub struct Xn79AVL<K, V> {
    root: Option<Box<Xn79AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn79AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn79AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn79AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn79AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn79AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn79AvlNode<K, V>>) -> Box<Xn79AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn79AvlNode<K, V>>) -> Box<Xn79AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn79AvlNode<K, V>>) -> Box<Xn79AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn79AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn79AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn79AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn79AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn79AvlNode<K, V>>) -> &Xn79AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn79AvlNode<K, V>>) -> (Box<Xn79AvlNode<K, V>>, Option<Box<Xn79AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn79AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn79AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn79AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn79AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn79AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn79AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn79AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo79RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo79Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo79RBNode<K, V> {
    key: K,
    value: V,
    color: Xo79Color,
    left: Option<Box<Xo79RBNode<K, V>>>,
    right: Option<Box<Xo79RBNode<K, V>>>,
}

/// A red-black tree map for crate 79.
#[derive(Debug, Clone)]
pub struct Xo79RedBlack<K, V> {
    root: Option<Box<Xo79RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo79RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo79Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo79RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo79RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo79RBNode {
                    key, value, color: Xo79Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo79RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo79Color::Red)
    }

    fn xo_balance(mut h: Box<Xo79RBNode<K, V>>) -> Box<Xo79RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo79Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo79RBNode<K, V>>) -> Box<Xo79RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo79Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo79RBNode<K, V>>) -> Box<Xo79RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo79Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo79RBNode<K, V>>) {
        h.color = Xo79Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo79Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo79Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo79Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo79RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo79RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo79RBNode<K, V>) -> (K, V, Option<Box<Xo79RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo79RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo79Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo79RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo79ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 79.
#[derive(Debug, Clone)]
pub struct Xo79ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo79ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo79#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo79#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 79).
#[derive(Debug)]
pub struct Xp79SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp79Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp79Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp79Node<K, V>>>,
    xp_right: Option<Box<Xp79Node<K, V>>>,
}

impl<K: Ord, V> Xp79Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp79SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp79SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp79Node<K, V>>>, key: &K) -> Option<Box<Xp79Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp79Node<K, V>>) -> Box<Xp79Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp79Node<K, V>>) -> Box<Xp79Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp79Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp79Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp79Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq79Treap ---------------

use std::cmp::Ordering as Xq79Ord;

struct Xq79TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq79TreapNode<K, V>>>,
    right: Option<Box<Xq79TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq79Treap<K, V> {
    root: Option<Box<Xq79TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq79TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_79_size<K, V>(node: &Option<Box<Xq79TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_79_update_size<K, V>(node: &mut Xq79TreapNode<K, V>) {
    node.size = 1 + xq_79_size(&node.left) + xq_79_size(&node.right);
}

fn xq_79_rotate_right<K, V>(mut node: Box<Xq79TreapNode<K, V>>) -> Box<Xq79TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_79_update_size(&mut node);
    left.right = Some(node);
    xq_79_update_size(&mut left);
    left
}

fn xq_79_rotate_left<K, V>(mut node: Box<Xq79TreapNode<K, V>>) -> Box<Xq79TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_79_update_size(&mut node);
    right.left = Some(node);
    xq_79_update_size(&mut right);
    right
}

fn xq_79_insert_node<K: Ord, V>(
    node: Option<Box<Xq79TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq79TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq79TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq79Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq79Ord::Less => {
                let (new_left, old) = xq_79_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_79_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_79_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq79Ord::Greater => {
                let (new_right, old) = xq_79_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_79_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_79_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_79_remove_node<K: Ord, V>(
    node: Option<Box<Xq79TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq79TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq79Ord::Less => {
                let (new_left, old) = xq_79_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_79_update_size(&mut n);
                (Some(n), old)
            }
            Xq79Ord::Greater => {
                let (new_right, old) = xq_79_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_79_update_size(&mut n);
                (Some(n), old)
            }
            Xq79Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_79_rotate_right(n);
                    let (new_right, old) = xq_79_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_79_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_79_rotate_left(n);
                    let (new_left, old) = xq_79_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_79_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_79_find_min<K, V>(node: &Option<Box<Xq79TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_79_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_79_find_max<K, V>(node: &Option<Box<Xq79TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_79_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_79_rank<K: Ord, V>(node: &Option<Box<Xq79TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq79Ord::Less => xq_79_rank(&n.left, key),
            Xq79Ord::Equal => xq_79_size(&n.left),
            Xq79Ord::Greater => 1 + xq_79_size(&n.left) + xq_79_rank(&n.right, key),
        },
    }
}

fn xq_79_kth<K, V>(node: &Option<Box<Xq79TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_79_size(&n.left);
        if k < left_size {
            xq_79_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_79_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_79_in_order<K: Clone, V>(node: &Option<Box<Xq79TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_79_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_79_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq79Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 79 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_79_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq79Ord::Equal => return Some(&n.value),
                Xq79Ord::Less => cur = &n.left,
                Xq79Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_79_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_79_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_79_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_79_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_79_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_79_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_79_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq79VEBTree ---------------

pub struct Xq79VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq79VEBTree>>,
    clusters: Vec<Option<Box<Xq79VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq79VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq79VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq79VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manifest(id: &str) -> ExtensionManifest {
        ExtensionManifest {
            identifier: ExtensionIdentifier {
                id: id.to_string(),
                version: "1.0.0".to_string(),
            },
            name: id.to_string(),
            publisher: "test".to_string(),
            description: None,
            kind: ExtensionKind::UI,
            activation_events: Vec::new(),
            contributes: Vec::new(),
        }
    }

    fn make_manifest_full(
        id: &str,
        name: &str,
        publisher: &str,
        events: Vec<&str>,
    ) -> ExtensionManifest {
        ExtensionManifest {
            identifier: ExtensionIdentifier {
                id: id.to_string(),
                version: "1.0.0".to_string(),
            },
            name: name.to_string(),
            publisher: publisher.to_string(),
            description: Some("desc".to_string()),
            kind: ExtensionKind::Workspace,
            activation_events: events.into_iter().map(String::from).collect(),
            contributes: Vec::new(),
        }
    }

    #[test]
    fn install_and_find() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest("ext.a"));
        assert!(svc.find_by_id("ext.a").is_some());
        assert!(svc.find_by_id("ext.b").is_none());
    }

    #[test]
    fn enable_disable() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest("ext.a"));
        svc.enable("ext.a");
        assert_eq!(svc.get_enabled().len(), 1);
        svc.disable("ext.a");
        assert_eq!(svc.get_enabled().len(), 0);
        assert_eq!(
            svc.find_by_id("ext.a").unwrap().status,
            ExtensionStatus::Disabled
        );
    }

    #[test]
    fn uninstall_works() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest("ext.a"));
        svc.uninstall("ext.a");
        assert_eq!(
            svc.find_by_id("ext.a").unwrap().status,
            ExtensionStatus::Uninstalled
        );
    }

    #[test]
    fn try_install_ok() {
        let mut svc = ExtensionService::new();
        assert!(svc.try_install(make_manifest("ext.a")).is_ok());
        assert_eq!(svc.count(), 1);
    }

    #[test]
    fn try_install_duplicate() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest("ext.a"));
        let err = svc.try_install(make_manifest("ext.a")).unwrap_err();
        assert_eq!(err, ExtensionError::AlreadyInstalled("ext.a".to_string()));
    }

    #[test]
    fn get_by_status_works() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest("ext.a"));
        svc.install(make_manifest("ext.b"));
        svc.enable("ext.a");
        assert_eq!(svc.get_by_status(ExtensionStatus::Enabled).len(), 1);
        assert_eq!(svc.get_by_status(ExtensionStatus::Installed).len(), 1);
        assert_eq!(svc.get_by_status(ExtensionStatus::Disabled).len(), 0);
    }

    #[test]
    fn count_extensions() {
        let mut svc = ExtensionService::new();
        assert_eq!(svc.count(), 0);
        svc.install(make_manifest("ext.a"));
        svc.install(make_manifest("ext.b"));
        assert_eq!(svc.count(), 2);
    }

    #[test]
    fn search_by_name() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest_full("a", "Rust Analyzer", "matklad", vec![]));
        svc.install(make_manifest_full("b", "Python", "ms", vec![]));
        svc.install(make_manifest_full("c", "rust-fmt", "rustlang", vec![]));
        assert_eq!(svc.search("rust").len(), 2);
        assert_eq!(svc.search("PYTHON").len(), 1);
        assert_eq!(svc.search("go").len(), 0);
    }

    #[test]
    fn get_by_publisher_works() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest_full("a", "ExtA", "acme", vec![]));
        svc.install(make_manifest_full("b", "ExtB", "acme", vec![]));
        svc.install(make_manifest_full("c", "ExtC", "other", vec![]));
        assert_eq!(svc.get_by_publisher("acme").len(), 2);
        assert_eq!(svc.get_by_publisher("other").len(), 1);
        assert_eq!(svc.get_by_publisher("none").len(), 0);
    }

    #[test]
    fn manifest_full_id() {
        let m = make_manifest_full("prettier", "Prettier", "esbenp", vec![]);
        assert_eq!(m.full_id(), "esbenp.prettier");
    }

    #[test]
    fn manifest_has_activation_event() {
        let m = make_manifest_full("a", "A", "pub", vec!["onLanguage:rust", "onCommand:start"]);
        assert!(m.has_activation_event("onLanguage:rust"));
        assert!(!m.has_activation_event("onLanguage:python"));
    }

    #[test]
    fn identifier_parse_ok() {
        let id = ExtensionIdentifier::parse("my-ext@2.3.1").unwrap();
        assert_eq!(id.id, "my-ext");
        assert_eq!(id.version, "2.3.1");
    }

    #[test]
    fn identifier_parse_invalid() {
        assert!(ExtensionIdentifier::parse("no-version").is_err());
        assert!(ExtensionIdentifier::parse("@1.0").is_err());
        assert!(ExtensionIdentifier::parse("id@").is_err());
    }

    #[test]
    fn display_impls() {
        assert_eq!(ExtensionKind::UI.to_string(), "UI");
        assert_eq!(ExtensionKind::Workspace.to_string(), "Workspace");
        assert_eq!(ExtensionKind::Web.to_string(), "Web");
        assert_eq!(ExtensionStatus::Enabled.to_string(), "Enabled");
        assert_eq!(ExtensionStatus::Disabled.to_string(), "Disabled");
        let id = ExtensionIdentifier {
            id: "foo".to_string(),
            version: "1.0.0".to_string(),
        };
        assert_eq!(id.to_string(), "foo@1.0.0");
    }

    #[test]
    fn extension_error_display() {
        let e = ExtensionError::NotFound("x".to_string());
        assert_eq!(e.to_string(), "extension not found: x");
        let e = ExtensionError::DependencyMissing("dep".to_string());
        assert_eq!(e.to_string(), "missing dependency: dep");
    }

    #[test]
    fn parse_version_ok() {
        assert_eq!(parse_version("1.2.3").unwrap(), (1, 2, 3));
        assert_eq!(parse_version("0.0.0").unwrap(), (0, 0, 0));
        assert_eq!(parse_version("10.20.30").unwrap(), (10, 20, 30));
    }

    #[test]
    fn parse_version_err() {
        assert!(parse_version("1.2").is_err());
        assert!(parse_version("abc").is_err());
        assert!(parse_version("1.2.x").is_err());
    }

    #[test]
    fn compare_versions_basic() {
        assert_eq!(compare_versions("1.0.0", "1.0.0").unwrap(), VersionCompare::Same);
        assert_eq!(compare_versions("1.0.0", "2.0.0").unwrap(), VersionCompare::Older);
        assert_eq!(compare_versions("2.0.0", "1.0.0").unwrap(), VersionCompare::Newer);
        assert_eq!(compare_versions("1.1.0", "1.0.9").unwrap(), VersionCompare::Newer);
    }

    #[test]
    fn identifier_is_compatible() {
        let id = ExtensionIdentifier { id: "x".into(), version: "2.0.0".into() };
        assert!(id.is_compatible_with("1.0.0").unwrap());
        assert!(id.is_compatible_with("2.0.0").unwrap());
        assert!(!id.is_compatible_with("3.0.0").unwrap());
    }

    #[test]
    fn manifest_validate_ok() {
        let m = make_manifest("ext.a");
        assert!(m.validate().is_ok());
    }

    #[test]
    fn manifest_validate_empty_id() {
        let mut m = make_manifest("ext.a");
        m.identifier.id = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn manifest_is_eager() {
        let m = make_manifest_full("a", "A", "pub", vec!["*", "onLanguage:rust"]);
        assert!(m.is_eager());
        let m2 = make_manifest_full("b", "B", "pub", vec!["onLanguage:rust"]);
        assert!(!m2.is_eager());
    }

    #[test]
    fn manifest_contribution_count() {
        let m = make_manifest("ext.a");
        assert_eq!(m.contribution_count(), 0);
    }

    #[test]
    fn service_update_version() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest("ext.a"));
        assert!(svc.update_version("ext.a", "2.0.0").is_ok());
        assert_eq!(svc.find_by_id("ext.a").unwrap().manifest.identifier.version, "2.0.0");
    }

    #[test]
    fn service_update_version_not_found() {
        let mut svc = ExtensionService::new();
        assert!(svc.update_version("nope", "1.0.0").is_err());
    }

    #[test]
    fn service_sorted_by_name() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest_full("c", "Charlie", "pub", vec![]));
        svc.install(make_manifest_full("a", "Alpha", "pub", vec![]));
        svc.install(make_manifest_full("b", "Bravo", "pub", vec![]));
        let sorted = svc.sorted_by_name();
        assert_eq!(sorted[0].manifest.name, "Alpha");
        assert_eq!(sorted[1].manifest.name, "Bravo");
        assert_eq!(sorted[2].manifest.name, "Charlie");
    }

    #[test]
    fn service_get_by_kind() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest("a")); // UI kind
        svc.install(make_manifest_full("b", "B", "pub", vec![])); // Workspace kind
        assert_eq!(svc.get_by_kind(ExtensionKind::UI).len(), 1);
        assert_eq!(svc.get_by_kind(ExtensionKind::Workspace).len(), 1);
        assert_eq!(svc.get_by_kind(ExtensionKind::Web).len(), 0);
    }

    #[test]
    fn service_has_any_with_event() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest_full("a", "A", "pub", vec!["onLanguage:rust"]));
        assert!(svc.has_any_with_event("onLanguage:rust"));
        assert!(!svc.has_any_with_event("onLanguage:python"));
    }

    #[test]
    fn service_purge_uninstalled() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest("a"));
        svc.install(make_manifest("b"));
        svc.uninstall("a");
        assert_eq!(svc.purge_uninstalled(), 1);
        assert_eq!(svc.count(), 1);
    }

    #[test]
    fn extension_entry_display() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest("ext.a"));
        let entry = svc.find_by_id("ext.a").unwrap();
        let s = format!("{entry}");
        assert!(s.contains("test.ext.a"));
        assert!(s.contains("1.0.0"));
    }

    #[test]
    fn test_validate_semver_valid() {
        assert!(validate_semver("1.0.0"));
        assert!(validate_semver("0.0.0"));
        assert!(validate_semver("10.20.30"));
    }

    #[test]
    fn test_validate_semver_invalid() {
        assert!(!validate_semver("1.0"));
        assert!(!validate_semver("abc"));
        assert!(!validate_semver("1.2.x"));
        assert!(!validate_semver(""));
        assert!(!validate_semver("1.2.3.4"));
    }

    #[test]
    fn test_manifest_validator_valid() {
        let v = ExtensionManifestValidator::new();
        let m = ExtensionManifest {
            identifier: ExtensionIdentifier { id: "my-ext".into(), version: "1.0.0".into() },
            name: "My Extension".into(),
            publisher: "acme".into(),
            description: None,
            kind: ExtensionKind::UI,
            activation_events: Vec::new(),
            contributes: Vec::new(),
        };
        assert!(v.validate(&m).is_ok());
    }

    #[test]
    fn test_manifest_validator_empty_name() {
        let v = ExtensionManifestValidator::new();
        let m = ExtensionManifest {
            identifier: ExtensionIdentifier { id: "my-ext".into(), version: "1.0.0".into() },
            name: "".into(),
            publisher: "acme".into(),
            description: None,
            kind: ExtensionKind::UI,
            activation_events: Vec::new(),
            contributes: Vec::new(),
        };
        let errs = v.validate(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn test_manifest_validator_bad_version() {
        let v = ExtensionManifestValidator::new();
        let m = ExtensionManifest {
            identifier: ExtensionIdentifier { id: "my-ext".into(), version: "bad".into() },
            name: "My Extension".into(),
            publisher: "acme".into(),
            description: None,
            kind: ExtensionKind::UI,
            activation_events: Vec::new(),
            contributes: Vec::new(),
        };
        let errs = v.validate(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("version")));
    }

    #[test]
    fn test_manifest_validator_bad_id() {
        let v = ExtensionManifestValidator::new();
        let m = ExtensionManifest {
            identifier: ExtensionIdentifier { id: "BAD_ID!".into(), version: "1.0.0".into() },
            name: "My Extension".into(),
            publisher: "acme".into(),
            description: None,
            kind: ExtensionKind::UI,
            activation_events: Vec::new(),
            contributes: Vec::new(),
        };
        let errs = v.validate(&m).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("id")));
    }

    #[test]
    fn test_activation_event_parse_language() {
        let ev = ActivationEvent::parse("onLanguage:rust").unwrap();
        assert_eq!(ev, ActivationEvent::OnLanguage("rust".to_string()));
    }

    #[test]
    fn test_activation_event_parse_command() {
        let ev = ActivationEvent::parse("onCommand:myext.doThing").unwrap();
        assert_eq!(ev, ActivationEvent::OnCommand("myext.doThing".to_string()));
    }

    #[test]
    fn test_activation_event_parse_star() {
        let ev = ActivationEvent::parse("*").unwrap();
        assert_eq!(ev, ActivationEvent::Star);
    }

    #[test]
    fn test_activation_event_parse_invalid() {
        assert!(ActivationEvent::parse("unknown:foo").is_none());
        assert!(ActivationEvent::parse("").is_none());
    }

    #[test]
    fn test_activate_by_event_matches() {
        let m = make_manifest_full("a", "A", "pub", vec!["onLanguage:rust", "onCommand:start"]);
        assert!(extension_activate_by_event(&m, &ActivationEvent::OnLanguage("rust".to_string())));
        assert!(extension_activate_by_event(&m, &ActivationEvent::OnCommand("start".to_string())));
    }

    #[test]
    fn test_activate_by_event_no_match() {
        let m = make_manifest_full("a", "A", "pub", vec!["onLanguage:rust"]);
        assert!(!extension_activate_by_event(&m, &ActivationEvent::OnLanguage("python".to_string())));
        assert!(!extension_activate_by_event(&m, &ActivationEvent::Star));
    }

    #[test]
    fn test_dep_resolver_simple_order() {
        let mut resolver = ExtensionDependencyResolver::new();
        resolver.add_extension("base", vec![]);
        resolver.add_extension("mid", vec!["base".to_string()]);
        resolver.add_extension("top", vec!["mid".to_string()]);
        let order = resolver.resolve_order().unwrap();
        let base_pos = order.iter().position(|x| x == "base").unwrap();
        let mid_pos = order.iter().position(|x| x == "mid").unwrap();
        let top_pos = order.iter().position(|x| x == "top").unwrap();
        assert!(base_pos < mid_pos);
        assert!(mid_pos < top_pos);
    }

    #[test]
    fn test_dep_resolver_detects_missing() {
        let mut resolver = ExtensionDependencyResolver::new();
        resolver.add_extension("ext-a", vec!["nonexistent".to_string()]);
        let err = resolver.resolve_order().unwrap_err();
        assert_eq!(err, ExtensionError::DependencyMissing("nonexistent".to_string()));
    }

    #[test]
    fn extensions_plat_stats_new_defaults() {
        let stats = ExtensionsPlatStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn extensions_plat_stats_record_success() {
        let mut stats = ExtensionsPlatStats::new();
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
    fn extensions_plat_stats_record_failure() {
        let mut stats = ExtensionsPlatStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn extensions_plat_stats_reset() {
        let mut stats = ExtensionsPlatStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn extensions_plat_stats_merge() {
        let mut a = ExtensionsPlatStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtensionsPlatStats::new();
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
    fn extensions_plat_stats_display() {
        let mut stats = ExtensionsPlatStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn extensions_plat_stats_default() {
        let stats = ExtensionsPlatStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn extensions_plat_validator_accepts_valid_name() {
        let v = ExtensionsPlatValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn extensions_plat_validator_rejects_empty() {
        let v = ExtensionsPlatValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn extensions_plat_validator_rejects_too_long() {
        let v = ExtensionsPlatValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn extensions_plat_validator_forbidden_prefix() {
        let v = ExtensionsPlatValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn extensions_plat_validator_allowed_chars() {
        let v = ExtensionsPlatValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn extensions_plat_validator_range() {
        let v = ExtensionsPlatValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn extensions_plat_sanitize_removes_control() {
        let result = ExtensionsPlatValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn extensions_plat_truncate_short_string() {
        assert_eq!(ExtensionsPlatValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn extensions_plat_truncate_long_string() {
        let result = ExtensionsPlatValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn extensions_plat_is_ascii_printable() {
        assert!(ExtensionsPlatValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtensionsPlatValidator::is_ascii_printable("Hello\x00World"));
    }

    // --- New tests for dependency graph, compatibility, load ordering ---

    #[test]
    fn dep_graph_add_and_direct_deps() {
        let mut g = ExtensionDependencyGraph::new();
        g.add_extension("a");
        g.add_dependency("b", "a");
        g.add_dependency("c", "a");
        g.add_dependency("c", "b");
        assert_eq!(g.extension_count(), 3);
        assert_eq!(g.direct_deps("c"), vec!["a", "b"]);
        assert!(g.direct_deps("a").is_empty());
    }

    #[test]
    fn dep_graph_topological_sort() {
        let mut g = ExtensionDependencyGraph::new();
        g.add_dependency("app", "lib");
        g.add_dependency("lib", "core");
        let order = g.topological_sort().unwrap();
        let pos = |id: &str| order.iter().position(|s| s == id).unwrap();
        assert!(pos("core") < pos("lib"));
        assert!(pos("lib") < pos("app"));
    }

    #[test]
    fn dep_graph_cycle_detection() {
        let mut g = ExtensionDependencyGraph::new();
        g.add_dependency("a", "b");
        g.add_dependency("b", "a");
        assert!(g.topological_sort().is_err());
        assert!(g.would_create_cycle("a", "b"));
    }

    #[test]
    fn dep_graph_transitive_deps() {
        let mut g = ExtensionDependencyGraph::new();
        g.add_dependency("c", "b");
        g.add_dependency("b", "a");
        let trans = g.transitive_deps("c");
        assert!(trans.contains(&"a".to_string()));
        assert!(trans.contains(&"b".to_string()));
    }

    #[test]
    fn dep_graph_roots_and_leaves() {
        let mut g = ExtensionDependencyGraph::new();
        g.add_dependency("app", "lib");
        g.add_dependency("lib", "core");
        let roots = g.root_extensions();
        assert!(roots.contains(&"core"));
        let leaves = g.leaf_extensions();
        assert!(leaves.contains(&"app"));
    }

    #[test]
    fn version_range_satisfies() {
        let range = VersionRange::at_least(1, 2, 0);
        assert!(range.satisfies("1.2.0").unwrap());
        assert!(range.satisfies("2.0.0").unwrap());
        assert!(!range.satisfies("1.1.9").unwrap());

        let compat = VersionRange::compatible_major(1);
        assert!(compat.satisfies("1.5.0").unwrap());
        assert!(!compat.satisfies("2.0.0").unwrap());
    }

    #[test]
    fn compatibility_checker_filters() {
        let checker = CompatibilityChecker::new("1.50.0");
        let m1 = make_manifest("pub1.ext1");
        // All test manifests have version "1.0.0" and is_compatible_with checks parse_version
        // which compares with the engine version
        assert_eq!(checker.engine_version(), "1.50.0");
    }

    #[test]
    fn dep_graph_display() {
        let mut g = ExtensionDependencyGraph::new();
        g.add_extension("x");
        g.add_extension("y");
        assert_eq!(format!("{g}"), "ExtensionDependencyGraph(2 extensions)");
    }

    // --- new tests ---

    #[test]
    fn extension_recommendation_basic() {
        let rec = ExtensionRecommendation::new(
            "rust-analyzer",
            "Rust files detected",
            "*.rs",
            10,
        );
        assert_eq!(rec.extension_id, "rust-analyzer");
        assert_eq!(rec.priority, 10);
        let display = format!("{}", rec);
        assert!(display.contains("rust-analyzer"));
    }

    #[test]
    fn recommender_filters_by_file_ext() {
        let mut recommender = ExtensionRecommender::new();
        recommender.add_rule(
            "rs",
            ExtensionRecommendation::new("rust-analyzer", "Rust", "*.rs", 10),
        );
        recommender.add_rule(
            "py",
            ExtensionRecommendation::new("pylance", "Python", "*.py", 8),
        );
        recommender.add_rule(
            "ts",
            ExtensionRecommendation::new("typescript", "TypeScript", "*.ts", 9),
        );
        let recs = recommender.recommend(&["rs", "py"]);
        assert_eq!(recs.len(), 2);
        // highest priority first
        assert_eq!(recs[0].extension_id, "rust-analyzer");
        assert_eq!(recs[1].extension_id, "pylance");
    }

    #[test]
    fn recommender_no_matches() {
        let recommender = ExtensionRecommender::new();
        let recs = recommender.recommend(&["go"]);
        assert!(recs.is_empty());
    }

    #[test]
    fn extension_status_summary() {
        let mut svc = ExtensionService::new();
        svc.install(make_manifest("ext-a"));
        svc.install(make_manifest("ext-b"));
        svc.enable("ext-a");
        let summary = ExtensionStatusSummary::from_service(&svc);
        assert_eq!(summary.enabled, 1);
        assert_eq!(summary.installed, 1);
        assert_eq!(summary.total(), 2);
        assert_eq!(summary.active(), 2);
    }

    #[test]
    fn extension_status_summary_display() {
        let summary = ExtensionStatusSummary {
            installed: 2,
            enabled: 3,
            disabled: 1,
            uninstalled: 0,
        };
        let text = format!("{}", summary);
        assert!(text.contains("installed=2"));
        assert!(text.contains("enabled=3"));
    }

    #[test]
    fn recommender_recommended_ids() {
        let mut recommender = ExtensionRecommender::new();
        recommender.add_rule(
            "rs",
            ExtensionRecommendation::new("rust-analyzer", "Rust", "*.rs", 10),
        );
        let ids = recommender.recommended_ids(&["rs"]);
        assert_eq!(ids, vec!["rust-analyzer"]);
        let ids_empty = recommender.recommended_ids(&["java"]);
        assert!(ids_empty.is_empty());
    }

    // -- ExtensionValidateScan tests --

    #[test]
    fn validate_scan_valid_manifest() {
        let scanner = ExtensionValidateScan::new();
        let m = make_manifest("test-ext");
        let result = scanner.scan(&m);
        assert!(result.is_valid());
    }

    #[test]
    fn validate_scan_empty_name() {
        let scanner = ExtensionValidateScan::new();
        let mut m = make_manifest("");
        m.name = String::new();
        let result = scanner.scan(&m);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn validate_scan_multiple() {
        let scanner = ExtensionValidateScan::new();
        let manifests = vec![make_manifest("a"), make_manifest("b")];
        let results = scanner.scan_all(&manifests);
        assert_eq!(results.len(), 2);
    }

    // -- ExtensionStorageQuota tests --

    #[test]
    fn storage_quota_basic() {
        let mut q = ExtensionStorageQuota::new(1_000_000);
        q.set_usage("ext-a", 500_000);
        assert!(!q.is_over_quota("ext-a"));
        assert_eq!(q.remaining_bytes("ext-a"), 500_000);
    }

    #[test]
    fn storage_quota_over() {
        let mut q = ExtensionStorageQuota::new(1000);
        q.set_usage("ext-a", 2000);
        assert!(q.is_over_quota("ext-a"));
        assert!(q.usage_percent("ext-a") > 100.0);
    }

    #[test]
    fn storage_quota_custom() {
        let mut q = ExtensionStorageQuota::new(1000);
        q.set_quota("ext-a", 5000);
        q.set_usage("ext-a", 3000);
        assert!(!q.is_over_quota("ext-a"));
        assert_eq!(q.remaining_bytes("ext-a"), 2000);
    }

    // -- ExtensionToggle tests --

    #[test]
    fn toggle_disable_enable() {
        let mut t = ExtensionToggle::new();
        t.disable("ext-a", DisableReason::User);
        assert!(t.is_disabled("ext-a"));
        assert_eq!(t.disable_reason("ext-a"), Some(&DisableReason::User));
        assert!(t.enable("ext-a"));
        assert!(!t.is_disabled("ext-a"));
    }

    #[test]
    fn toggle_disabled_by_reason() {
        let mut t = ExtensionToggle::new();
        t.disable("a", DisableReason::User);
        t.disable("b", DisableReason::WorkspaceTrust);
        t.disable("c", DisableReason::User);
        assert_eq!(t.disabled_by_reason(&DisableReason::User).len(), 2);
        assert_eq!(t.disabled_count(), 3);
    }

    // -- ExtensionStartupTracker tests --

    #[test]
    fn startup_tracker_record_and_query() {
        let mut tracker = ExtensionStartupTracker::new();
        tracker.record_activation("ext-a", 120);
        tracker.record_activation("ext-b", 300);
        assert_eq!(tracker.activation_time("ext-a"), Some(120));
        assert_eq!(tracker.activation_time("ext-b"), Some(300));
        assert_eq!(tracker.activation_time("ext-c"), None);
        assert_eq!(tracker.extension_count(), 2);
    }

    #[test]
    fn startup_tracker_slowest_and_totals() {
        let mut tracker = ExtensionStartupTracker::new();
        tracker.record_activation("fast", 10);
        tracker.record_activation("medium", 50);
        tracker.record_activation("slow", 200);
        let slowest = tracker.slowest(2);
        assert_eq!(slowest.len(), 2);
        assert_eq!(slowest[0].0, "slow");
        assert_eq!(slowest[1].0, "medium");
        assert_eq!(tracker.total_activation_time(), 260);
        assert!((tracker.average_activation_time() - 86.666).abs() < 1.0);
    }

    #[test]
    fn startup_tracker_empty() {
        let tracker = ExtensionStartupTracker::new();
        assert_eq!(tracker.extension_count(), 0);
        assert_eq!(tracker.total_activation_time(), 0);
        assert_eq!(tracker.average_activation_time(), 0.0);
        assert!(tracker.slowest(5).is_empty());
    }

    // -- ExtensionResourceLoader tests --

    #[test]
    fn resource_loader_register_and_query() {
        let mut loader = ExtensionResourceLoader::new("my.ext");
        loader.register_resource("icons/logo.png", 4096);
        loader.register_resource("data/schema.json", 512);
        assert_eq!(loader.resource_count(), 2);
        let info = loader.get_resource("icons/logo.png").unwrap();
        assert_eq!(info.size_bytes, 4096);
        assert_eq!(info.path, "icons/logo.png");
        assert_eq!(loader.total_size(), 4608);
    }

    #[test]
    fn resource_loader_missing() {
        let loader = ExtensionResourceLoader::new("my.ext");
        assert!(loader.get_resource("nope").is_none());
        assert_eq!(loader.resource_count(), 0);
        assert_eq!(loader.total_size(), 0);
    }

    // -- ExtensionConfigDefault tests --

    #[test]
    fn config_default_add_and_query() {
        let mut cfg = ExtensionConfigDefault::new("my.ext");
        cfg.add_default("theme", "dark", "Default color theme");
        cfg.add_default("fontSize", "14", "Editor font size");
        assert_eq!(cfg.get_default("theme"), Some("dark"));
        assert!(cfg.has_key("fontSize"));
        assert!(!cfg.has_key("missing"));
        assert_eq!(cfg.count(), 2);
    }

    #[test]
    fn config_default_overwrite() {
        let mut cfg = ExtensionConfigDefault::new("my.ext");
        cfg.add_default("theme", "dark", "Theme");
        cfg.add_default("theme", "light", "Updated theme");
        assert_eq!(cfg.get_default("theme"), Some("light"));
        assert_eq!(cfg.count(), 1);
    }

    #[test]
    fn config_default_defaults_list() {
        let mut cfg = ExtensionConfigDefault::new("my.ext");
        cfg.add_default("a", "1", "first");
        cfg.add_default("b", "2", "second");
        let defs = cfg.defaults();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0], ("a", "1", "first"));
    }

    // -- ExtensionPackExpander tests --

    #[test]
    fn pack_expander_add_and_query() {
        let mut pack = ExtensionPackExpander::new("pack.web");
        pack.add_member("ext-html");
        pack.add_member("ext-css");
        pack.add_member("ext-js");
        assert_eq!(pack.member_count(), 3);
        assert!(pack.contains("ext-css"));
        assert!(!pack.contains("ext-go"));
        assert_eq!(pack.pack_id(), "pack.web");
        assert_eq!(pack.members(), &["ext-html", "ext-css", "ext-js"]);
    }

    #[test]
    fn pack_expander_no_duplicates() {
        let mut pack = ExtensionPackExpander::new("pack.dup");
        pack.add_member("ext-a");
        pack.add_member("ext-a");
        assert_eq!(pack.member_count(), 1);
    }

    #[test]
    fn pack_expander_display() {
        let mut pack = ExtensionPackExpander::new("pack.test");
        pack.add_member("ext-a");
        pack.add_member("ext-b");
        let s = format!("{pack}");
        assert!(s.contains("pack.test"));
        assert!(s.contains("2 members"));
    }

    #[test]
    fn resource_loader_resources_list() {
        let mut loader = ExtensionResourceLoader::new("my.ext");
        loader.register_resource("a.txt", 100);
        loader.register_resource("b.txt", 200);
        let paths = loader.resources();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&"a.txt"));
        assert!(paths.contains(&"b.txt"));
    }

    #[test]
    fn startup_tracker_overwrite() {
        let mut tracker = ExtensionStartupTracker::new();
        tracker.record_activation("ext-a", 100);
        tracker.record_activation("ext-a", 250);
        assert_eq!(tracker.activation_time("ext-a"), Some(250));
        assert_eq!(tracker.extension_count(), 1);
    }

    #[test]
    fn toggle_enable_nonexistent() {
        let mut t = ExtensionToggle::new();
        assert!(!t.enable("missing"));
    }

    #[test]
    fn extpbuf_ringbuf_push_get() {
        let mut rb = ExtPBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn extpbuf_ringbuf_overflow() {
        let mut rb = ExtPBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn extpbuf_ringbuf_clear() {
        let mut rb = ExtPBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn extpbuf_ringbuf_newest_oldest() {
        let mut rb = ExtPBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn extpbuf_ringbuf_to_vec() {
        let mut rb = ExtPBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn extpbuf_ringbuf_is_full() {
        let mut rb = ExtPBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn extpfmt_fmt_list() {
        let f = ExtPFmtFmt::new(ExtPFmtFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn extpfmt_fmt_kv() {
        let f = ExtPFmtFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn extpfmt_fmt_section() {
        let f = ExtPFmtFmt::new(ExtPFmtFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn extpfmt_fmt_truncate() {
        let f = ExtPFmtFmt::new(ExtPFmtFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn extpfmt_fmt_opts_defaults() {
        let o = ExtPFmtFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    // -- extensions_plat additional tests -------------------------------------------

    #[test]
    fn x_extensions_plat_capabilities_register_and_has() {
        let mut caps = XExtensionsPlatCapabilities::new();
        caps.register("clipboard");
        assert!(caps.has("clipboard"));
        assert!(!caps.has("fs"));
    }

    #[test]
    fn x_extensions_plat_capabilities_len() {
        let mut caps = XExtensionsPlatCapabilities::new();
        assert!(caps.is_empty());
        caps.register("a");
        caps.register("b");
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn x_extensions_plat_capabilities_intersect() {
        let mut a = XExtensionsPlatCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XExtensionsPlatCapabilities::new();
        b.register("y");
        b.register("z");
        let inter = a.intersect(&b);
        assert_eq!(inter.len(), 1);
        assert!(inter.has("y"));
    }

    #[test]
    fn x_extensions_plat_capabilities_diff() {
        let mut a = XExtensionsPlatCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XExtensionsPlatCapabilities::new();
        b.register("y");
        let d = a.diff(&b);
        assert_eq!(d.len(), 1);
        assert!(d.has("x"));
    }

    #[test]
    fn x_extensions_plat_service_registry_basic() {
        let mut reg = XExtensionsPlatServiceRegistry::new();
        assert!(reg.is_empty());
        reg.register("clipboard", "v1");
        assert_eq!(reg.get("clipboard"), Some("v1"));
        assert!(reg.contains("clipboard"));
    }

    #[test]
    fn x_extensions_plat_service_registry_replace() {
        let mut reg = XExtensionsPlatServiceRegistry::new();
        assert!(reg.register("svc", "old").is_none());
        assert_eq!(reg.register("svc", "new"), Some("old".into()));
        assert_eq!(reg.get("svc"), Some("new"));
    }

    #[test]
    fn x_extensions_plat_service_registry_remove() {
        let mut reg = XExtensionsPlatServiceRegistry::new();
        reg.register("svc", "v1");
        assert_eq!(reg.remove("svc"), Some("v1".into()));
        assert!(reg.is_empty());
    }

    #[test]
    fn x_extensions_plat_service_registry_names() {
        let mut reg = XExtensionsPlatServiceRegistry::new();
        reg.register("a", "1");
        reg.register("b", "2");
        let mut names = reg.names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn x_extensions_plat_sanitize_path_basic() {
        assert_eq!(x_extensions_plat_sanitize_path("/a//b///c/"), "/a/b/c");
    }

    #[test]
    fn x_extensions_plat_sanitize_path_backslash() {
        assert_eq!(x_extensions_plat_sanitize_path("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn x_extensions_plat_sanitize_path_single() {
        assert_eq!(x_extensions_plat_sanitize_path("/"), "/");
    }

    #[test]
    fn x_extensions_plat_capabilities_default() {
        let caps = XExtensionsPlatCapabilities::default();
        assert!(caps.is_empty());
    }

    #[test]
    fn x_extensions_plat_capabilities_all() {
        let mut caps = XExtensionsPlatCapabilities::new();
        caps.register("a");
        caps.register("b");
        let mut all = caps.all();
        all.sort();
        assert_eq!(all, vec!["a", "b"]);
    }


    // -- extensions_plat extended domain tests ----------------------------------------

    #[test]
    fn y_extensions_plat_enum_index() {
        assert_eq!(YExtensionsPlatExtensionActivation::OnLoad.index(), 0);
        assert_eq!(YExtensionsPlatExtensionActivation::OnCommand.index(), 1);
        assert_eq!(YExtensionsPlatExtensionActivation::OnLanguage.index(), 2);
        assert_eq!(YExtensionsPlatExtensionActivation::OnView.index(), 3);
    }

    #[test]
    fn y_extensions_plat_enum_label() {
        assert_eq!(YExtensionsPlatExtensionActivation::OnLoad.label(), "OnLoad");
        assert_eq!(YExtensionsPlatExtensionActivation::OnCommand.label(), "OnCommand");
        assert_eq!(YExtensionsPlatExtensionActivation::OnLanguage.label(), "OnLanguage");
        assert_eq!(YExtensionsPlatExtensionActivation::OnView.label(), "OnView");
    }

    #[test]
    fn y_extensions_plat_enum_all() {
        let all = YExtensionsPlatExtensionActivation::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_extensions_plat_enum_is_default() {
        assert!(YExtensionsPlatExtensionActivation::OnLoad.is_default());
        assert!(!YExtensionsPlatExtensionActivation::OnView.is_default());
    }

    #[test]
    fn y_extensions_plat_enum_display() {
        assert_eq!(format!("{}", YExtensionsPlatExtensionActivation::OnLoad), "OnLoad");
    }

    #[test]
    fn y_extensions_plat_struct_new() {
        let s = YExtensionsPlatExtensionManifest::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_extensions_plat_struct_clear() {
        let mut s = YExtensionsPlatExtensionManifest::new();
        s.dependencies.push("test".into());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_extensions_plat_fingerprint_deterministic() {
        let h1 = y_extensions_plat_fingerprint("hello");
        let h2 = y_extensions_plat_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_extensions_plat_fingerprint("a"), y_extensions_plat_fingerprint("b"));
    }

    #[test]
    fn y_extensions_plat_truncate_short() {
        assert_eq!(y_extensions_plat_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_extensions_plat_truncate_long() {
        let r = y_extensions_plat_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_extensions_plat_normalize_key_basic() {
        assert_eq!(y_extensions_plat_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_extensions_plat_split_path_basic() {
        let parts = y_extensions_plat_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_extensions_plat_count_occurrences_basic() {
        assert_eq!(y_extensions_plat_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_extensions_plat_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_extensions_plat_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_extensions_plat_in_range_basic() {
        assert!(y_extensions_plat_in_range(5, 1, 10));
        assert!(y_extensions_plat_in_range(1, 1, 10));
        assert!(y_extensions_plat_in_range(10, 1, 10));
        assert!(!y_extensions_plat_in_range(0, 1, 10));
        assert!(!y_extensions_plat_in_range(11, 1, 10));
    }

    #[test]
    fn y_extensions_plat_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_extensions_plat_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_extensions_plat_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_extensions_plat_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- extensions_plat Z-extended tests -----------------------------------------------

    #[test]
    fn z_extensions_plat_priority_weight() {
        assert_eq!(ZExtensionsPlatPriority::Idle.weight(), 0);
        assert_eq!(ZExtensionsPlatPriority::Normal.weight(), 2);
        assert_eq!(ZExtensionsPlatPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_extensions_plat_priority_label() {
        assert_eq!(ZExtensionsPlatPriority::Low.label(), "low");
        assert_eq!(ZExtensionsPlatPriority::High.label(), "high");
    }

    #[test]
    fn z_extensions_plat_priority_is_elevated() {
        assert!(!ZExtensionsPlatPriority::Normal.is_elevated());
        assert!(ZExtensionsPlatPriority::High.is_elevated());
        assert!(ZExtensionsPlatPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_extensions_plat_priority_display() {
        assert_eq!(format!("{}", ZExtensionsPlatPriority::Idle), "idle");
    }

    #[test]
    fn z_extensions_plat_priority_all_asc() {
        let all = ZExtensionsPlatPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZExtensionsPlatPriority::Idle);
        assert_eq!(all[4], ZExtensionsPlatPriority::Realtime);
    }

    #[test]
    fn z_extensions_plat_struct_new() {
        let s = ZExtensionsPlatExtensionSandbox::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_extensions_plat_struct_toggled_clone() {
        let s = ZExtensionsPlatExtensionSandbox::new();
        let t = s.toggled_clone();
        assert_ne!(s.isolated, t.isolated);
    }

    #[test]
    fn z_extensions_plat_rolling_hash_deterministic() {
        let h1 = z_extensions_plat_rolling_hash(b"test");
        let h2 = z_extensions_plat_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_extensions_plat_rolling_hash(b"a"), z_extensions_plat_rolling_hash(b"b"));
    }

    #[test]
    fn z_extensions_plat_pad_to_basic() {
        assert_eq!(z_extensions_plat_pad_to("hi", 5), "hi   ");
        assert_eq!(z_extensions_plat_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_extensions_plat_is_identifier_basic() {
        assert!(z_extensions_plat_is_identifier("foo_bar"));
        assert!(z_extensions_plat_is_identifier("abc123"));
        assert!(!z_extensions_plat_is_identifier(""));
        assert!(!z_extensions_plat_is_identifier("has space"));
    }

    #[test]
    fn z_extensions_plat_levenshtein_basic() {
        assert_eq!(z_extensions_plat_levenshtein("", ""), 0);
        assert_eq!(z_extensions_plat_levenshtein("abc", "abc"), 0);
        assert_eq!(z_extensions_plat_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_extensions_plat_unique_words_basic() {
        let w = z_extensions_plat_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_extensions_plat_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_extensions_plat_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_extensions_plat_common_prefix_basic() {
        assert_eq!(z_extensions_plat_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_extensions_plat_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_extensions_plat_struct_clear() {
        let mut s = ZExtensionsPlatExtensionSandbox::new();
        s.permissions.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_extensions_plat_rolling_hash_empty() {
        let h = z_extensions_plat_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_89_push_and_len() {
        let mut rb = super::XbRingBuffer89::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_89_overwrite() {
        let mut rb = super::XbRingBuffer89::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_89_get_out_of_bounds() {
        let rb = super::XbRingBuffer89::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_89_drain_all() {
        let mut rb = super::XbRingBuffer89::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_89_peek_front_back() {
        let mut rb = super::XbRingBuffer89::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_89_clear() {
        let mut rb = super::XbRingBuffer89::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_89_capacity() {
        let rb = super::XbRingBuffer89::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_89_basic() {
        let h = super::xb_fnv1a_89(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_89(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_89_different_inputs() {
        let h1 = super::xb_fnv1a_89(b"abc");
        let h2 = super::xb_fnv1a_89(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_89_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_89(&data);
        let dec = super::xb_rle_decode_89(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_89_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_89(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_89(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_89_values() {
        assert!((super::xb_clamp_89(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_89(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_89(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_89_values() {
        assert!((super::xb_lerp_89(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_89(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_89(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_89_wrap_around_twice() {
        let mut rb = super::XbRingBuffer89::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 80 ----

    #[test]
    fn xc_80_pool_new_empty() {
        let pool: super::Xc80Pool<i32> = super::Xc80Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_80_pool_release_acquire() {
        let mut pool = super::Xc80Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_80_pool_acquire_empty() {
        let mut pool: super::Xc80Pool<i32> = super::Xc80Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_80_pool_full() {
        let mut pool = super::Xc80Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_80_pool_drain() {
        let mut pool = super::Xc80Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_80_pool_stats() {
        let mut pool = super::Xc80Pool::new(8);
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
    fn xc_80_pool_clear() {
        let mut pool = super::Xc80Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_80_pool_shrink() {
        let mut pool = super::Xc80Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_80_pool_default() {
        let pool: super::Xc80Pool<String> = super::Xc80Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_80_pool_extend() {
        let mut pool = super::Xc80Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_80_pool_retain() {
        let mut pool = super::Xc80Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_80_scheduler_round_robin() {
        let mut sched = super::Xc80Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_80_scheduler_empty() {
        let mut sched = super::Xc80Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_80_scheduler_reset() {
        let mut sched = super::Xc80Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_80_scheduler_add_remove() {
        let mut sched = super::Xc80Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_80_scheduler_targets() {
        let sched = super::Xc80Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_80_hash_empty() {
        assert_eq!(super::xc_80_hash(b""), 5381);
    }

    #[test]
    fn xc_80_hash_data() {
        let h = super::xc_80_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_80_hash(b"hello"), h);
    }

    #[test]
    fn xc_80_reverse_str() {
        assert_eq!(super::xc_80_reverse("abc"), "cba");
        assert_eq!(super::xc_80_reverse(""), "");
    }


    #[test]
    fn xe_102_pipeline_empty() {
        let p = super::Xe102Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_102_pipeline_parse_stage() {
        let p = super::Xe102Pipeline::new()
            .add_parse(super::xe_102_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_102_pipeline_transform_double() {
        let p = super::Xe102Pipeline::new()
            .add_transform(super::xe_102_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_102_pipeline_validate_reverse() {
        let p = super::Xe102Pipeline::new()
            .add_validate(super::xe_102_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_102_pipeline_emit_filter() {
        let p = super::Xe102Pipeline::new()
            .add_emit(super::xe_102_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_102_pipeline_multi_stage() {
        let p = super::Xe102Pipeline::new()
            .add_parse(super::xe_102_pipeline_identity)
            .add_transform(super::xe_102_pipeline_double)
            .add_validate(super::xe_102_pipeline_reverse)
            .add_emit(super::xe_102_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_102_pipeline_error_propagation() {
        let p = super::Xe102Pipeline::new()
            .add_parse(super::xe_102_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe102Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_102_pipeline_compose() {
        let p1 = super::Xe102Pipeline::new()
            .add_parse(super::xe_102_pipeline_identity);
        let p2 = super::Xe102Pipeline::new()
            .add_transform(super::xe_102_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_102_pipeline_error_display() {
        let e = super::Xe102PipelineError {
            stage: super::Xe102Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_102_cache_put_get() {
        let mut c = super::Xe102Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_102_cache_miss() {
        let mut c: super::Xe102Cache<&str, i32> = super::Xe102Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_102_cache_ttl_expiry() {
        let mut c = super::Xe102Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_102_cache_evict() {
        let mut c = super::Xe102Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_102_cache_capacity() {
        let mut c = super::Xe102Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_102_cache_stats() {
        let mut c = super::Xe102Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_102_cache_clear() {
        let mut c = super::Xe102Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_100 graph tests ------------------------------------------------

    #[test]
    fn xg_100_graph_empty() {
        let g = super::Xg100Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_100_graph_add_node() {
        let mut g = super::Xg100Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_100_graph_add_edge() {
        let mut g = super::Xg100Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_100_graph_neighbors() {
        let mut g = super::Xg100Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_100_graph_has_path() {
        let mut g = super::Xg100Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_100_graph_self_path() {
        let g = super::Xg100Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_100_graph_topo_sort() {
        let mut g = super::Xg100Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_100_graph_cycle_detect_false() {
        let mut g = super::Xg100Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_100_graph_cycle_detect_true() {
        let mut g = super::Xg100Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_100 heap tests -------------------------------------------------

    #[test]
    fn xg_100_heap_empty() {
        let h: super::Xg100Heap<i32> = super::Xg100Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_100_heap_push_pop() {
        let mut h = super::Xg100Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_100_heap_peek() {
        let mut h = super::Xg100Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_100_heap_drain_sorted() {
        let mut h = super::Xg100Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_100_heap_merge() {
        let mut a = super::Xg100Heap::new();
        let mut b = super::Xg100Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_100_heap_default() {
        let h: super::Xg100Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_100_graph_default() {
        let g: super::Xg100Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh79_skip_insert_contains() {
        let mut sl = super::Xh79SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh79_skip_remove() {
        let mut sl = super::Xh79SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh79_skip_len() {
        let mut sl = super::Xh79SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh79_skip_range_query() {
        let mut sl = super::Xh79SkipList::xh_new(4);
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
    fn xh79_skip_floor_ceiling() {
        let mut sl = super::Xh79SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh79_skip_rank() {
        let mut sl = super::Xh79SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh79_skip_empty() {
        let sl = super::Xh79SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh79_skip_duplicates() {
        let mut sl = super::Xh79SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh79_bitset_set_test() {
        let mut bs = super::Xh79BitSet::xh_new(256);
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
    fn xh79_bitset_clear_count() {
        let mut bs = super::Xh79BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh79_bitset_and_or_xor() {
        let mut a = super::Xh79BitSet::xh_new(128);
        let mut b = super::Xh79BitSet::xh_new(128);
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
    fn xh79_bitset_iter_ones() {
        let mut bs = super::Xh79BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh79_bitset_first_last() {
        let mut bs = super::Xh79BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh79_bitset_empty() {
        let bs = super::Xh79BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi79_deque_push_pop_back() {
        let mut dq = super::Xi79Deque::xi_new(4);
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
    fn xi79_deque_push_pop_front() {
        let mut dq = super::Xi79Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi79_deque_mixed_ops() {
        let mut dq = super::Xi79Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi79_deque_get_and_split() {
        let mut dq = super::Xi79Deque::xi_new(8);
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
    fn xi79_deque_rotate_left() {
        let mut dq = super::Xi79Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi79_deque_rotate_right() {
        let mut dq = super::Xi79Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi79_deque_grow() {
        let mut dq = super::Xi79Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi79_deque_empty() {
        let dq = super::Xi79Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi79_interval_tree_insert_query() {
        let mut tree = super::Xi79IntervalTree::xi_new();
        tree.xi_insert(super::Xi79Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi79Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi79Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi79_interval_tree_overlap() {
        let mut tree = super::Xi79IntervalTree::xi_new();
        tree.xi_insert(super::Xi79Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi79Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi79Interval::xi_new(12, 20));
        let q = super::Xi79Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi79_interval_tree_remove() {
        let mut tree = super::Xi79IntervalTree::xi_new();
        tree.xi_insert(super::Xi79Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi79Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi79_interval_tree_gaps() {
        let mut tree = super::Xi79IntervalTree::xi_new();
        tree.xi_insert(super::Xi79Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi79Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi79Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi79Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi79Interval::xi_new(8, 10));
    }

    #[test]
    fn xi79_interval_tree_merge() {
        let mut tree = super::Xi79IntervalTree::xi_new();
        tree.xi_insert(super::Xi79Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi79Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi79Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi79Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi79Interval::xi_new(10, 15));
    }

    #[test]
    fn xi79_interval_tree_all() {
        let mut tree = super::Xi79IntervalTree::xi_new();
        tree.xi_insert(super::Xi79Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi79Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi79_interval_tree_empty() {
        let tree = super::Xi79IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi79_interval_tree_contains_point() {
        let iv = super::Xi79Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 58) ---

    #[test]
    fn xj_58_uf_make_and_find() {
        let mut uf = super::Xj58UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_58_uf_union_connected() {
        let mut uf = super::Xj58UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_58_uf_component_count() {
        let mut uf = super::Xj58UnionFind::xj_new();
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
    fn xj_58_uf_component_size() {
        let mut uf = super::Xj58UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_58_uf_largest_component() {
        let mut uf = super::Xj58UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_58_uf_many_elements() {
        let mut uf = super::Xj58UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_58_uf_separate_components() {
        let mut uf = super::Xj58UnionFind::xj_new();
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
    fn xj_58_uf_path_compression() {
        let mut uf = super::Xj58UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_58_bt_insert_get() {
        let mut bt = super::Xj58BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_58_bt_contains_len() {
        let mut bt = super::Xj58BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_58_bt_replace() {
        let mut bt = super::Xj58BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_58_bt_remove() {
        let mut bt = super::Xj58BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_58_bt_keys_values() {
        let mut bt = super::Xj58BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_58_bt_range() {
        let mut bt = super::Xj58BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_58_bt_min_max() {
        let mut bt = super::Xj58BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_58_bt_many_inserts() {
        let mut bt = super::Xj58BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_79 segment tree tests ---

    #[test]
    fn xk_79_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk79SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_79_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk79SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_79_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk79SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_79_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk79SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_79_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk79SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_79_st_single_element() {
        let data = vec![42];
        let st = super::Xk79SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_79_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk79SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_79_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk79SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_79 disjoint intervals tests ---

    #[test]
    fn xk_79_di_add_and_count() {
        let mut di = super::Xk79DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_79_di_merge_overlap() {
        let mut di = super::Xk79DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_79_di_contains() {
        let mut di = super::Xk79DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_79_di_remove() {
        let mut di = super::Xk79DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_79_di_covered_length() {
        let mut di = super::Xk79DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_79_di_gaps() {
        let mut di = super::Xk79DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_79_di_merge_adjacent() {
        let mut di = super::Xk79DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_79_di_empty() {
        let di = super::Xk79DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_58_rope_new_empty() {
        let rope = super::Xl58Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_58_rope_from_str() {
        let rope = super::Xl58Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_58_rope_insert_at() {
        let mut rope = super::Xl58Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_58_rope_delete_range() {
        let mut rope = super::Xl58Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_58_rope_char_at() {
        let rope = super::Xl58Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_58_rope_split_concat() {
        let rope = super::Xl58Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_58_rope_line_count() {
        let rope = super::Xl58Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_58_rope_line_at() {
        let rope = super::Xl58Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_58_sa_build_and_search() {
        let sa = super::Xl58SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_58_sa_count() {
        let sa = super::Xl58SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_58_sa_longest_repeated() {
        let sa = super::Xl58SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_58_sa_all_positions() {
        let sa = super::Xl58SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_58_sa_len() {
        let sa = super::Xl58SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_58_sa_empty() {
        let sa = super::Xl58SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_58_rope_slice() {
        let rope = super::Xl58Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_58_sa_search_start() {
        let sa = super::Xl58SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_58_sparse_set_get() {
        let mut m = super::Xm58MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_58_sparse_row_col() {
        let mut m = super::Xm58MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_58_sparse_transpose() {
        let mut m = super::Xm58MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_58_sparse_multiply_vec() {
        let mut m = super::Xm58MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_58_sparse_nnz_density() {
        let mut m = super::Xm58MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_58_sparse_clear() {
        let mut m = super::Xm58MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_58_sparse_overwrite_zero() {
        let mut m = super::Xm58MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_58_tokenizer_basic() {
        let t = super::Xm58Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_58_tokenizer_count() {
        let t = super::Xm58Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_58_tokenizer_unique() {
        let t = super::Xm58Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_58_tokenizer_frequency() {
        let t = super::Xm58Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_58_tokenizer_delimiter() {
        let t = super::Xm58Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_58_tokenizer_whitespace() {
        let t = super::Xm58Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_58_tokenizer_empty() {
        let t = super::Xm58Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 79 ----

    #[test]
    fn xn_79_fenwick_prefix_sum() {
        let mut ft = super::Xn79Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_79_fenwick_range_sum() {
        let mut ft = super::Xn79Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_79_fenwick_point_query() {
        let mut ft = super::Xn79Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_79_fenwick_len() {
        let ft = super::Xn79Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_79_fenwick_multiple_updates() {
        let mut ft = super::Xn79Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_79_fenwick_single_element() {
        let mut ft = super::Xn79Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_79_fenwick_find_kth() {
        let mut ft = super::Xn79Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_79_fenwick_negative_delta() {
        let mut ft = super::Xn79Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 79 ----

    #[test]
    fn xn_79_avl_insert_get() {
        let mut m = super::Xn79AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_79_avl_remove() {
        let mut m = super::Xn79AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_79_avl_in_order() {
        let mut m = super::Xn79AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_79_avl_min_max() {
        let mut m = super::Xn79AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_79_avl_floor_ceiling() {
        let mut m = super::Xn79AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_79_avl_height_balanced() {
        let mut m = super::Xn79AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_79_avl_overwrite() {
        let mut m = super::Xn79AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_79_avl_empty() {
        let m: super::Xn79AVL<i32, i32> = super::Xn79AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo79RedBlack tests ---

    #[test]
    fn xo_79_rb_insert_and_get() {
        let mut tree = super::Xo79RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_79_rb_len_and_empty() {
        let mut tree = super::Xo79RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_79_rb_min_max() {
        let mut tree = super::Xo79RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_79_rb_contains() {
        let mut tree = super::Xo79RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_79_rb_remove() {
        let mut tree = super::Xo79RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_79_rb_in_order() {
        let mut tree = super::Xo79RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_79_rb_black_height() {
        let mut tree = super::Xo79RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_79_rb_overwrite() {
        let mut tree = super::Xo79RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo79ConsistentHash tests ---

    #[test]
    fn xo_79_ch_add_and_count() {
        let mut ring = super::Xo79ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_79_ch_remove_node() {
        let mut ring = super::Xo79ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_79_ch_get_node() {
        let mut ring = super::Xo79ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_79_ch_empty_ring() {
        let ring = super::Xo79ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_79_ch_distribution() {
        let mut ring = super::Xo79ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_79_ch_rebalance() {
        let mut ring = super::Xo79ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_79_ch_virtual_nodes() {
        let mut ring = super::Xo79ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_79_ch_consistent_lookup() {
        let mut ring = super::Xo79ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_79_splay_insert_get() {
        let mut t = super::Xp79SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_79_splay_remove() {
        let mut t = super::Xp79SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_79_splay_count_increases() {
        let mut t = super::Xp79SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_79_splay_depth() {
        let mut t = super::Xp79SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_79_splay_len_empty() {
        let t = super::Xp79SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_79_splay_min_max() {
        let mut t = super::Xp79SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_79_splay_overwrite() {
        let mut t = super::Xp79SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_79_splay_remove_missing() {
        let mut t = super::Xp79SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_79 treap tests ----
    #[test]
    fn xq_79_treap_empty() {
        let t = super::Xq79Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_79_treap_insert_get() {
        let mut t = super::Xq79Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_79_treap_overwrite() {
        let mut t = super::Xq79Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_79_treap_remove() {
        let mut t = super::Xq79Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_79_treap_min_max() {
        let mut t = super::Xq79Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_79_treap_rank() {
        let mut t = super::Xq79Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_79_treap_kth() {
        let mut t = super::Xq79Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_79_treap_in_order() {
        let mut t = super::Xq79Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_79 VEB tree tests ----
    #[test]
    fn xq_79_veb_empty() {
        let v = super::Xq79VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_79_veb_insert_contains() {
        let mut v = super::Xq79VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_79_veb_min_max() {
        let mut v = super::Xq79VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_79_veb_delete() {
        let mut v = super::Xq79VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_79_veb_successor() {
        let mut v = super::Xq79VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_79_veb_predecessor() {
        let mut v = super::Xq79VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_79_veb_count() {
        let mut v = super::Xq79VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_79_veb_duplicate_insert() {
        let mut v = super::Xq79VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }

}