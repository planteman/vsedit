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
    fn uninstall() {
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
    fn get_by_status() {
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
    fn get_by_publisher() {
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
}
