//! Ext API: Source control.
//!
//! RPC bridge between the extension host and the main thread for SCM.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_scm";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ScmMessage {
    RegisterProvider {
        id: String,
        label: String,
        root_uri: Option<String>,
    },
    UnregisterProvider {
        id: String,
    },
    CreateResourceGroup {
        provider_id: String,
        group_id: String,
        label: String,
    },
    UpdateResources {
        provider_id: String,
        group_id: String,
        resources: Vec<ScmResource>,
    },
    SetInputBoxValue {
        provider_id: String,
        value: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceControl {
    pub id: String,
    pub label: String,
    pub root_uri: Option<String>,
    pub input_box_value: String,
    pub groups: Vec<SourceControlGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceControlGroup {
    pub id: String,
    pub label: String,
    pub resources: Vec<ScmResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScmResource {
    pub uri: String,
    pub decorations: Option<ScmResourceDecorations>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScmResourceDecorations {
    pub icon_path: Option<String>,
    pub tooltip: Option<String>,
    pub strikethrough: bool,
    pub faded: bool,
}

// ── Bridge ──

pub struct ScmBridge {
    providers: Vec<SourceControl>,
}

impl ScmBridge {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register_provider(&mut self, id: &str, label: &str, root_uri: Option<String>) {
        if !self.providers.iter().any(|p| p.id == id) {
            self.providers.push(SourceControl {
                id: id.to_string(),
                label: label.to_string(),
                root_uri,
                input_box_value: String::new(),
                groups: Vec::new(),
            });
        }
    }

    pub fn unregister_provider(&mut self, id: &str) {
        self.providers.retain(|p| p.id != id);
    }

    pub fn get_provider(&self, id: &str) -> Option<&SourceControl> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn create_group(&mut self, provider_id: &str, group_id: &str, label: &str) {
        if let Some(p) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            p.groups.push(SourceControlGroup {
                id: group_id.to_string(),
                label: label.to_string(),
                resources: Vec::new(),
            });
        }
    }

    pub fn handle_message(&mut self, msg: &ScmMessage) -> serde_json::Value {
        match msg {
            ScmMessage::RegisterProvider {
                id,
                label,
                root_uri,
            } => {
                self.register_provider(id, label, root_uri.clone());
                serde_json::json!({"registered": true})
            }
            ScmMessage::UnregisterProvider { id } => {
                self.unregister_provider(id);
                serde_json::json!({"unregistered": true})
            }
            ScmMessage::CreateResourceGroup {
                provider_id,
                group_id,
                label,
            } => {
                self.create_group(provider_id, group_id, label);
                serde_json::json!({"created": true})
            }
            ScmMessage::UpdateResources {
                provider_id,
                group_id,
                resources,
            } => {
                if let Some(p) = self.providers.iter_mut().find(|p| p.id == *provider_id) {
                    if let Some(g) = p.groups.iter_mut().find(|g| g.id == *group_id) {
                        g.resources = resources.clone();
                        return serde_json::json!({"updated": resources.len()});
                    }
                }
                serde_json::json!({"error": "not found"})
            }
            ScmMessage::SetInputBoxValue { provider_id, value } => {
                if let Some(p) = self.providers.iter_mut().find(|p| p.id == *provider_id) {
                    p.input_box_value = value.clone();
                    serde_json::json!({"set": true})
                } else {
                    serde_json::json!({"error": "not found"})
                }
            }
        }
    }
}

impl Default for ScmBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ── Error Types ──

/// Errors that can occur during SCM bridge operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ScmError {
    /// The requested provider was not found.
    ProviderNotFound(String),
    /// The requested resource group was not found.
    GroupNotFound { provider_id: String, group_id: String },
    /// A provider with this ID already exists.
    DuplicateProvider(String),
    /// A resource group with this ID already exists in the provider.
    DuplicateGroup { provider_id: String, group_id: String },
    /// Validation failed for the given field.
    ValidationError(String),
}

impl fmt::Display for ScmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScmError::ProviderNotFound(id) => write!(f, "provider not found: {id}"),
            ScmError::GroupNotFound {
                provider_id,
                group_id,
            } => write!(f, "group '{group_id}' not found in provider '{provider_id}'"),
            ScmError::DuplicateProvider(id) => write!(f, "provider already registered: {id}"),
            ScmError::DuplicateGroup {
                provider_id,
                group_id,
            } => write!(
                f,
                "group '{group_id}' already exists in provider '{provider_id}'"
            ),
            ScmError::ValidationError(msg) => write!(f, "validation error: {msg}"),
        }
    }
}

impl std::error::Error for ScmError {}

// ── Display Implementations ──

impl fmt::Display for ScmResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.decorations {
            Some(d) => write!(
                f,
                "{} ({})",
                self.uri,
                d.tooltip.as_deref().unwrap_or("no tooltip")
            ),
            None => write!(f, "{}", self.uri),
        }
    }
}

impl fmt::Display for SourceControlGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} resources)", self.label, self.resources.len())
    }
}

impl fmt::Display for SourceControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}] ({} groups)",
            self.label,
            self.id,
            self.groups.len()
        )
    }
}

// ── Builder: SourceControlBuilder ──

/// Builder for constructing a [`SourceControl`] instance with validation.
pub struct SourceControlBuilder {
    id: Option<String>,
    label: Option<String>,
    root_uri: Option<String>,
    input_box_value: String,
    groups: Vec<SourceControlGroup>,
}

impl SourceControlBuilder {
    pub fn new() -> Self {
        Self {
            id: None,
            label: None,
            root_uri: None,
            input_box_value: String::new(),
            groups: Vec::new(),
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn root_uri(mut self, uri: impl Into<String>) -> Self {
        self.root_uri = Some(uri.into());
        self
    }

    pub fn input_box_value(mut self, value: impl Into<String>) -> Self {
        self.input_box_value = value.into();
        self
    }

    pub fn group(mut self, group: SourceControlGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Build the [`SourceControl`], returning an error if required fields are missing.
    pub fn build(self) -> Result<SourceControl, ScmError> {
        let id = self
            .id
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ScmError::ValidationError("id is required".into()))?;
        let label = self
            .label
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ScmError::ValidationError("label is required".into()))?;
        Ok(SourceControl {
            id,
            label,
            root_uri: self.root_uri,
            input_box_value: self.input_box_value,
            groups: self.groups,
        })
    }
}

impl Default for SourceControlBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ── ScmBridge Helpers ──

impl ScmBridge {
    /// List all registered provider IDs.
    pub fn provider_ids(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.id.as_str()).collect()
    }

    /// Return the total number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Return a mutable reference to a provider.
    pub fn get_provider_mut(&mut self, id: &str) -> Option<&mut SourceControl> {
        self.providers.iter_mut().find(|p| p.id == id)
    }

    /// Register a provider, returning an error on duplicate.
    pub fn try_register_provider(
        &mut self,
        id: &str,
        label: &str,
        root_uri: Option<String>,
    ) -> Result<(), ScmError> {
        if self.providers.iter().any(|p| p.id == id) {
            return Err(ScmError::DuplicateProvider(id.to_string()));
        }
        self.providers.push(SourceControl {
            id: id.to_string(),
            label: label.to_string(),
            root_uri,
            input_box_value: String::new(),
            groups: Vec::new(),
        });
        Ok(())
    }

    /// Create a group, returning an error if the provider doesn't exist or group is duplicate.
    pub fn try_create_group(
        &mut self,
        provider_id: &str,
        group_id: &str,
        label: &str,
    ) -> Result<(), ScmError> {
        let provider = self
            .providers
            .iter_mut()
            .find(|p| p.id == provider_id)
            .ok_or_else(|| ScmError::ProviderNotFound(provider_id.to_string()))?;
        if provider.groups.iter().any(|g| g.id == group_id) {
            return Err(ScmError::DuplicateGroup {
                provider_id: provider_id.to_string(),
                group_id: group_id.to_string(),
            });
        }
        provider.groups.push(SourceControlGroup {
            id: group_id.to_string(),
            label: label.to_string(),
            resources: Vec::new(),
        });
        Ok(())
    }

    /// Count total resources across all providers and groups.
    pub fn total_resource_count(&self) -> usize {
        self.providers
            .iter()
            .flat_map(|p| &p.groups)
            .map(|g| g.resources.len())
            .sum()
    }

    /// Find all resources whose URI contains the given substring.
    pub fn find_resources_by_uri(&self, pattern: &str) -> Vec<&ScmResource> {
        self.providers
            .iter()
            .flat_map(|p| &p.groups)
            .flat_map(|g| &g.resources)
            .filter(|r| r.uri.contains(pattern))
            .collect()
    }
}

// ── ScmResource helpers ──

impl ScmResource {
    /// Create a resource with no decorations.
    pub fn plain(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            decorations: None,
        }
    }

    /// Returns `true` if this resource has strikethrough decoration.
    pub fn is_deleted(&self) -> bool {
        self.decorations
            .as_ref()
            .map_or(false, |d| d.strikethrough)
    }

    /// Returns `true` if this resource is faded.
    pub fn is_faded(&self) -> bool {
        self.decorations.as_ref().map_or(false, |d| d.faded)
    }

    /// Extract the file name portion from the URI, if any.
    pub fn file_name(&self) -> Option<&str> {
        self.uri.rsplit('/').next()
    }
}

/// Initialize the scm extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = ScmMessage::RegisterProvider {
            id: "git".into(),
            label: "Git".into(),
            root_uri: Some("file:///repo".into()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ScmMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn resource_serialization() {
        let r = ScmResource {
            uri: "file:///a.rs".into(),
            decorations: Some(ScmResourceDecorations {
                icon_path: None,
                tooltip: Some("modified".into()),
                strikethrough: false,
                faded: false,
            }),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: ScmResource = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn bridge_provider_lifecycle() {
        let mut bridge = ScmBridge::new();
        bridge.register_provider("git", "Git", None);
        assert!(bridge.get_provider("git").is_some());
        bridge.unregister_provider("git");
        assert!(bridge.get_provider("git").is_none());
    }

    #[test]
    fn bridge_create_group() {
        let mut bridge = ScmBridge::new();
        bridge.register_provider("git", "Git", None);
        bridge.create_group("git", "changes", "Changes");
        let p = bridge.get_provider("git").unwrap();
        assert_eq!(p.groups.len(), 1);
        assert_eq!(p.groups[0].label, "Changes");
    }

    #[test]
    fn bridge_set_input_box() {
        let mut bridge = ScmBridge::new();
        bridge.register_provider("git", "Git", None);
        let msg = ScmMessage::SetInputBoxValue {
            provider_id: "git".into(),
            value: "fix: bug".into(),
        };
        bridge.handle_message(&msg);
        assert_eq!(bridge.get_provider("git").unwrap().input_box_value, "fix: bug");
    }

    // ── Error type tests ──

    #[test]
    fn error_display_provider_not_found() {
        let err = ScmError::ProviderNotFound("svn".into());
        assert_eq!(err.to_string(), "provider not found: svn");
    }

    #[test]
    fn error_display_group_not_found() {
        let err = ScmError::GroupNotFound {
            provider_id: "git".into(),
            group_id: "staged".into(),
        };
        assert_eq!(err.to_string(), "group 'staged' not found in provider 'git'");
    }

    #[test]
    fn error_display_duplicate_provider() {
        let err = ScmError::DuplicateProvider("git".into());
        assert_eq!(err.to_string(), "provider already registered: git");
    }

    // ── Builder tests ──

    #[test]
    fn builder_success() {
        let sc = SourceControlBuilder::new()
            .id("git")
            .label("Git")
            .root_uri("file:///repo")
            .build()
            .unwrap();
        assert_eq!(sc.id, "git");
        assert_eq!(sc.label, "Git");
        assert_eq!(sc.root_uri.as_deref(), Some("file:///repo"));
        assert!(sc.groups.is_empty());
    }

    #[test]
    fn builder_missing_id_errors() {
        let result = SourceControlBuilder::new().label("Git").build();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ScmError::ValidationError("id is required".into())
        );
    }

    #[test]
    fn builder_empty_label_errors() {
        let result = SourceControlBuilder::new().id("git").label("").build();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ScmError::ValidationError("label is required".into())
        );
    }

    // ── Try-methods with error handling ──

    #[test]
    fn try_register_duplicate_provider() {
        let mut bridge = ScmBridge::new();
        bridge.try_register_provider("git", "Git", None).unwrap();
        let err = bridge.try_register_provider("git", "Git2", None).unwrap_err();
        assert_eq!(err, ScmError::DuplicateProvider("git".into()));
    }

    #[test]
    fn try_create_group_missing_provider() {
        let mut bridge = ScmBridge::new();
        let err = bridge.try_create_group("git", "changes", "Changes").unwrap_err();
        assert_eq!(err, ScmError::ProviderNotFound("git".into()));
    }

    #[test]
    fn try_create_duplicate_group() {
        let mut bridge = ScmBridge::new();
        bridge.try_register_provider("git", "Git", None).unwrap();
        bridge.try_create_group("git", "changes", "Changes").unwrap();
        let err = bridge.try_create_group("git", "changes", "Changes").unwrap_err();
        assert_eq!(
            err,
            ScmError::DuplicateGroup {
                provider_id: "git".into(),
                group_id: "changes".into(),
            }
        );
    }

    // ── Resource helper tests ──

    #[test]
    fn resource_plain_and_file_name() {
        let r = ScmResource::plain("file:///src/main.rs");
        assert!(r.decorations.is_none());
        assert_eq!(r.file_name(), Some("main.rs"));
    }

    #[test]
    fn resource_deleted_and_faded() {
        let r = ScmResource {
            uri: "file:///old.rs".into(),
            decorations: Some(ScmResourceDecorations {
                icon_path: None,
                tooltip: Some("deleted".into()),
                strikethrough: true,
                faded: false,
            }),
        };
        assert!(r.is_deleted());
        assert!(!r.is_faded());
    }

    // ── Bridge computation tests ──

    #[test]
    fn total_resource_count_and_find() {
        let mut bridge = ScmBridge::new();
        bridge.register_provider("git", "Git", None);
        bridge.create_group("git", "changes", "Changes");
        bridge.create_group("git", "staged", "Staged");

        let msg = ScmMessage::UpdateResources {
            provider_id: "git".into(),
            group_id: "changes".into(),
            resources: vec![
                ScmResource::plain("file:///a.rs"),
                ScmResource::plain("file:///b.ts"),
            ],
        };
        bridge.handle_message(&msg);

        let msg2 = ScmMessage::UpdateResources {
            provider_id: "git".into(),
            group_id: "staged".into(),
            resources: vec![ScmResource::plain("file:///c.rs")],
        };
        bridge.handle_message(&msg2);

        assert_eq!(bridge.total_resource_count(), 3);
        assert_eq!(bridge.find_resources_by_uri(".rs").len(), 2);
        assert_eq!(bridge.find_resources_by_uri(".ts").len(), 1);
        assert_eq!(bridge.find_resources_by_uri(".py").len(), 0);
    }

    #[test]
    fn provider_ids_and_count() {
        let mut bridge = ScmBridge::new();
        assert_eq!(bridge.provider_count(), 0);
        bridge.register_provider("git", "Git", None);
        bridge.register_provider("svn", "SVN", None);
        assert_eq!(bridge.provider_count(), 2);
        let ids = bridge.provider_ids();
        assert!(ids.contains(&"git"));
        assert!(ids.contains(&"svn"));
    }

    // ── Display tests ──

    #[test]
    fn display_source_control() {
        let sc = SourceControlBuilder::new()
            .id("git")
            .label("Git")
            .build()
            .unwrap();
        assert_eq!(format!("{sc}"), "Git [git] (0 groups)");
    }

    #[test]
    fn display_resource_with_tooltip() {
        let r = ScmResource {
            uri: "file:///a.rs".into(),
            decorations: Some(ScmResourceDecorations {
                icon_path: None,
                tooltip: Some("modified".into()),
                strikethrough: false,
                faded: false,
            }),
        };
        assert_eq!(format!("{r}"), "file:///a.rs (modified)");
    }

    #[test]
    fn display_resource_without_decorations() {
        let r = ScmResource::plain("file:///b.rs");
        assert_eq!(format!("{r}"), "file:///b.rs");
    }
}
