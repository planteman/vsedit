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
}
