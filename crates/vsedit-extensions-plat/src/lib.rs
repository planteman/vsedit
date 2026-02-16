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
}
