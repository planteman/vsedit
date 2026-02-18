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

}