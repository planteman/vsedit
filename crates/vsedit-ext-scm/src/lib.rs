//! Ext API: Source control.
//!
//! RPC bridge between the extension host and the main thread for SCM.

pub mod git;

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

/// Accumulated statistics for ext-scm operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtScmStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtScmStats {
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
    pub fn merge(&mut self, other: &ExtScmStats) {
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

impl Default for ExtScmStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtScmStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtScmStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-scm.
#[derive(Debug, Clone)]
pub struct ExtScmValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtScmValidator {
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

impl Default for ExtScmValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SCM history
// ---------------------------------------------------------------------------

/// A single item in a file's SCM history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScmHistoryItem {
    /// Commit or changeset ID.
    pub id: String,
    /// Short summary / commit message first line.
    pub message: String,
    /// Author name.
    pub author: String,
    /// Unix timestamp of the commit.
    pub timestamp: u64,
    /// File path at the time of this commit (may differ from current due to renames).
    pub path: String,
}

impl ScmHistoryItem {
    pub fn new(
        id: impl Into<String>,
        message: impl Into<String>,
        author: impl Into<String>,
        timestamp: u64,
        path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            message: message.into(),
            author: author.into(),
            timestamp,
            path: path.into(),
        }
    }

    /// Return a short form of the ID (first 7 chars, like git short SHA).
    pub fn short_id(&self) -> &str {
        if self.id.len() > 7 { &self.id[..7] } else { &self.id }
    }

    /// Return the first line of the commit message.
    pub fn subject(&self) -> &str {
        self.message.lines().next().unwrap_or("")
    }
}

impl fmt::Display for ScmHistoryItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} ({})", self.short_id(), self.subject(), self.author)
    }
}

/// A list of history items with helper methods.
#[derive(Debug, Clone, Default)]
pub struct ScmHistory {
    items: Vec<ScmHistoryItem>,
}

impl ScmHistory {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, item: ScmHistoryItem) {
        self.items.push(item);
    }

    /// Return items sorted by timestamp, newest first.
    pub fn newest_first(&self) -> Vec<&ScmHistoryItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        sorted
    }

    /// Filter history to a specific author.
    pub fn by_author(&self, author: &str) -> Vec<&ScmHistoryItem> {
        self.items.iter().filter(|i| i.author == author).collect()
    }

    /// Return the most recent item.
    pub fn latest(&self) -> Option<&ScmHistoryItem> {
        self.items.iter().max_by_key(|i| i.timestamp)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ScmBridge additional helpers
// ---------------------------------------------------------------------------

impl ScmBridge {
    /// Collect all resources across all providers and groups.
    pub fn all_resources(&self) -> Vec<&ScmResource> {
        self.providers
            .iter()
            .flat_map(|p| &p.groups)
            .flat_map(|g| &g.resources)
            .collect()
    }

    /// Return providers that have at least one resource in any group.
    pub fn providers_with_changes(&self) -> Vec<&SourceControl> {
        self.providers
            .iter()
            .filter(|p| p.groups.iter().any(|g| !g.resources.is_empty()))
            .collect()
    }

    /// Return a summary string listing all providers and their resource counts.
    pub fn summary(&self) -> String {
        let parts: Vec<String> = self
            .providers
            .iter()
            .map(|p| {
                let total: usize = p.groups.iter().map(|g| g.resources.len()).sum();
                format!("{}({} resources)", p.label, total)
            })
            .collect();
        if parts.is_empty() {
            "ScmBridge: no providers".to_string()
        } else {
            format!("ScmBridge: {}", parts.join(", "))
        }
    }
}

impl fmt::Display for ScmBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ---------------------------------------------------------------------------
// SourceControl helpers
// ---------------------------------------------------------------------------

impl SourceControl {
    /// Number of resource groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Total number of resources across all groups.
    pub fn total_resources(&self) -> usize {
        self.groups.iter().map(|g| g.resources.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// SourceControlGroup helpers
// ---------------------------------------------------------------------------

impl SourceControlGroup {
    /// Whether this group contains no resources.
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ScmResource helpers
// ---------------------------------------------------------------------------

impl ScmResource {
    /// Extract the file extension from the URI (e.g. "rs" from "file:///a.rs").
    pub fn extension(&self) -> Option<&str> {
        let name = self.file_name()?;
        let dot = name.rfind('.')?;
        if dot + 1 < name.len() {
            Some(&name[dot + 1..])
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Change grouping by directory
// ---------------------------------------------------------------------------

/// Groups SCM resources by their parent directory path.
///
/// The directory is extracted from the URI by stripping the last path segment.
pub fn group_resources_by_directory(resources: &[ScmResource]) -> HashMap<String, Vec<&ScmResource>> {
    let mut map: HashMap<String, Vec<&ScmResource>> = HashMap::new();
    for res in resources {
        let dir = directory_from_uri(&res.uri);
        map.entry(dir).or_default().push(res);
    }
    map
}

use std::collections::HashMap;

/// Extract the directory portion from a URI string.
fn directory_from_uri(uri: &str) -> String {
    match uri.rfind('/') {
        Some(pos) if pos > 0 => uri[..pos].to_string(),
        _ => "/".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Diff statistics
// ---------------------------------------------------------------------------

/// Statistics about a textual diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffStats {
    /// Number of lines added.
    pub additions: usize,
    /// Number of lines removed.
    pub deletions: usize,
    /// Number of files changed.
    pub files_changed: usize,
}

impl DiffStats {
    pub fn new(additions: usize, deletions: usize, files_changed: usize) -> Self {
        Self { additions, deletions, files_changed }
    }

    /// Total number of lines changed (additions + deletions).
    pub fn total_changes(&self) -> usize {
        self.additions + self.deletions
    }

    /// Whether no changes were recorded.
    pub fn is_empty(&self) -> bool {
        self.additions == 0 && self.deletions == 0 && self.files_changed == 0
    }

    /// Compute a diff summary from a unified diff string.
    ///
    /// Counts lines beginning with `+` (excluding `+++`) as additions
    /// and lines beginning with `-` (excluding `---`) as deletions.
    pub fn from_unified_diff(diff: &str) -> Self {
        let mut additions = 0;
        let mut deletions = 0;
        let mut files = std::collections::HashSet::new();
        for line in diff.lines() {
            if line.starts_with("--- ") {
                // header, skip
            } else if line.starts_with("+++ ") {
                if let Some(path) = line.strip_prefix("+++ ") {
                    files.insert(path.trim().to_string());
                }
            } else if line.starts_with('+') {
                additions += 1;
            } else if line.starts_with('-') {
                deletions += 1;
            }
        }
        Self { additions, deletions, files_changed: files.len().max(if additions > 0 || deletions > 0 { 1 } else { 0 }) }
    }
}

impl fmt::Display for DiffStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} file(s) changed, {} insertion(s), {} deletion(s)",
            self.files_changed, self.additions, self.deletions
        )
    }
}

// ---------------------------------------------------------------------------
// Commit message validation
// ---------------------------------------------------------------------------

/// Validates a commit message according to common conventions.
#[derive(Debug, Clone)]
pub struct CommitMessageValidator {
    /// Max length for the subject (first line).
    pub max_subject_length: usize,
    /// Max length for any body line.
    pub max_body_line_length: usize,
}

impl CommitMessageValidator {
    pub fn new() -> Self {
        Self {
            max_subject_length: 72,
            max_body_line_length: 100,
        }
    }

    /// Validate a commit message, returning a list of warnings.
    pub fn validate(&self, message: &str) -> Vec<String> {
        let mut warnings = Vec::new();
        if message.trim().is_empty() {
            warnings.push("commit message must not be empty".into());
            return warnings;
        }
        let mut lines = message.lines();
        if let Some(subject) = lines.next() {
            if subject.len() > self.max_subject_length {
                warnings.push(format!(
                    "subject line is {} chars (max {})",
                    subject.len(),
                    self.max_subject_length
                ));
            }
            if subject.ends_with('.') {
                warnings.push("subject line should not end with a period".into());
            }
        }
        // Check if second line is blank (conventional)
        let second = lines.next();
        if let Some(line) = second {
            if !line.trim().is_empty() {
                warnings.push("second line should be blank to separate subject from body".into());
            }
        }
        for line in lines {
            if line.len() > self.max_body_line_length {
                warnings.push(format!(
                    "body line exceeds {} chars: \"{}...\"",
                    self.max_body_line_length,
                    &line[..40.min(line.len())]
                ));
                break; // report only first offending line
            }
        }
        warnings
    }
}

impl Default for CommitMessageValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Branch name validation
// ---------------------------------------------------------------------------

/// Validates branch names according to Git conventions.
pub fn validate_branch_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("branch name must not be empty".into());
    }
    if name.starts_with('-') {
        return Err("branch name must not start with a hyphen".into());
    }
    if name.contains("..") {
        return Err("branch name must not contain '..'".into());
    }
    if name.contains(' ') || name.contains('~') || name.contains('^') || name.contains(':') {
        return Err("branch name contains invalid characters".into());
    }
    if name.ends_with('/') || name.ends_with('.') {
        return Err("branch name must not end with '/' or '.'".into());
    }
    if name.contains("@{") {
        return Err("branch name must not contain '@{'".into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// ScmHistory — additional methods
// ---------------------------------------------------------------------------

impl ScmHistory {
    /// Filter history to items within a time range `[from, to]`.
    pub fn in_time_range(&self, from: u64, to: u64) -> Vec<&ScmHistoryItem> {
        self.items
            .iter()
            .filter(|i| i.timestamp >= from && i.timestamp <= to)
            .collect()
    }

    /// Returns all unique authors in the history.
    pub fn authors(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for item in &self.items {
            if seen.insert(item.author.as_str()) {
                result.push(item.author.as_str());
            }
        }
        result.sort();
        result
    }

    /// Returns the oldest item.
    pub fn oldest(&self) -> Option<&ScmHistoryItem> {
        self.items.iter().min_by_key(|i| i.timestamp)
    }

    /// Returns items as a slice.
    pub fn items(&self) -> &[ScmHistoryItem] {
        &self.items
    }

    /// Clears all history items.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

// ---------------------------------------------------------------------------
// SourceControl — additional methods
// ---------------------------------------------------------------------------

impl SourceControl {
    /// Returns all resources across all groups as a flat list.
    pub fn all_resources(&self) -> Vec<&ScmResource> {
        self.groups
            .iter()
            .flat_map(|g| &g.resources)
            .collect()
    }

    /// Finds a group by id.
    pub fn get_group(&self, group_id: &str) -> Option<&SourceControlGroup> {
        self.groups.iter().find(|g| g.id == group_id)
    }

    /// Returns group ids.
    pub fn group_ids(&self) -> Vec<&str> {
        self.groups.iter().map(|g| g.id.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// DiffStats — additional methods
// ---------------------------------------------------------------------------

impl DiffStats {
    /// Merge two DiffStats instances.
    pub fn merge(&self, other: &DiffStats) -> DiffStats {
        DiffStats {
            additions: self.additions + other.additions,
            deletions: self.deletions + other.deletions,
            files_changed: self.files_changed + other.files_changed,
        }
    }

    /// Returns the ratio of additions to total changes, or 0.0 if no changes.
    pub fn addition_ratio(&self) -> f64 {
        let total = self.total_changes();
        if total == 0 {
            0.0
        } else {
            self.additions as f64 / total as f64
        }
    }
}

// ---------------------------------------------------------------------------
// CommitMessageValidator — additional methods
// ---------------------------------------------------------------------------

impl CommitMessageValidator {
    /// Set the maximum subject line length.
    pub fn with_max_subject(mut self, max: usize) -> Self {
        self.max_subject_length = max;
        self
    }

    /// Set the maximum body line length.
    pub fn with_max_body_line(mut self, max: usize) -> Self {
        self.max_body_line_length = max;
        self
    }

    /// Returns `true` if the message has no validation warnings.
    pub fn is_valid(&self, message: &str) -> bool {
        self.validate(message).is_empty()
    }
}

// ---------------------------------------------------------------------------
// Merge conflict detection helpers
// ---------------------------------------------------------------------------

/// Markers used by typical three-way merge tools.
const CONFLICT_START: &str = "<<<<<<<";
const CONFLICT_MID: &str = "=======";
const CONFLICT_END: &str = ">>>>>>>";

/// A single merge conflict region found in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    /// 1-based line number where the conflict starts (`<<<<<<<`).
    pub start_line: usize,
    /// 1-based line number of the separator (`=======`).
    pub separator_line: usize,
    /// 1-based line number where the conflict ends (`>>>>>>>`).
    pub end_line: usize,
    /// The "ours" side of the conflict (lines between start and separator).
    pub ours: Vec<String>,
    /// The "theirs" side of the conflict (lines between separator and end).
    pub theirs: Vec<String>,
}

impl MergeConflict {
    /// Total number of conflicting lines (ours + theirs).
    pub fn total_lines(&self) -> usize {
        self.ours.len() + self.theirs.len()
    }

    /// Whether either side of the conflict is empty (trivial conflict).
    pub fn is_trivial(&self) -> bool {
        self.ours.is_empty() || self.theirs.is_empty()
    }
}

impl fmt::Display for MergeConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "conflict at lines {}-{} ({} ours, {} theirs)",
            self.start_line,
            self.end_line,
            self.ours.len(),
            self.theirs.len()
        )
    }
}

/// Detect merge conflict regions in file content.
///
/// Returns all conflict blocks found, each with line numbers and the two sides.
pub fn detect_merge_conflicts(content: &str) -> Vec<MergeConflict> {
    let mut conflicts = Vec::new();
    let mut start_line: Option<usize> = None;
    let mut separator_line: Option<usize> = None;
    let mut ours: Vec<String> = Vec::new();
    let mut theirs: Vec<String> = Vec::new();
    let mut in_ours = false;
    let mut in_theirs = false;

    for (idx, line) in content.lines().enumerate() {
        let lineno = idx + 1;
        let trimmed = line.trim();

        if trimmed.starts_with(CONFLICT_START) {
            start_line = Some(lineno);
            ours.clear();
            theirs.clear();
            in_ours = true;
            in_theirs = false;
        } else if trimmed.starts_with(CONFLICT_MID) && in_ours {
            separator_line = Some(lineno);
            in_ours = false;
            in_theirs = true;
        } else if trimmed.starts_with(CONFLICT_END) && in_theirs {
            if let (Some(sl), Some(sep)) = (start_line, separator_line) {
                conflicts.push(MergeConflict {
                    start_line: sl,
                    separator_line: sep,
                    end_line: lineno,
                    ours: ours.clone(),
                    theirs: theirs.clone(),
                });
            }
            start_line = None;
            separator_line = None;
            ours.clear();
            theirs.clear();
            in_ours = false;
            in_theirs = false;
        } else if in_ours {
            ours.push(line.to_string());
        } else if in_theirs {
            theirs.push(line.to_string());
        }
    }
    conflicts
}

/// Returns `true` if the content contains any merge conflict markers.
pub fn has_merge_conflicts(content: &str) -> bool {
    content.lines().any(|l| {
        let t = l.trim();
        t.starts_with(CONFLICT_START) || t.starts_with(CONFLICT_END)
    })
}

// ---------------------------------------------------------------------------
// Staged / unstaged change tracking
// ---------------------------------------------------------------------------

/// Tracks staged and unstaged resources for a provider, enabling move
/// operations between the two sets.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChangeTracker {
    staged: Vec<ScmResource>,
    unstaged: Vec<ScmResource>,
}

impl ChangeTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a resource to the unstaged set.
    pub fn add_unstaged(&mut self, resource: ScmResource) {
        if !self.unstaged.iter().any(|r| r.uri == resource.uri) {
            self.unstaged.push(resource);
        }
    }

    /// Stage a resource by URI, moving it from unstaged to staged.
    /// Returns `true` if the resource was found and staged.
    pub fn stage(&mut self, uri: &str) -> bool {
        if let Some(pos) = self.unstaged.iter().position(|r| r.uri == uri) {
            let resource = self.unstaged.remove(pos);
            if !self.staged.iter().any(|r| r.uri == uri) {
                self.staged.push(resource);
            }
            true
        } else {
            false
        }
    }

    /// Unstage a resource by URI, moving it from staged back to unstaged.
    /// Returns `true` if the resource was found and unstaged.
    pub fn unstage(&mut self, uri: &str) -> bool {
        if let Some(pos) = self.staged.iter().position(|r| r.uri == uri) {
            let resource = self.staged.remove(pos);
            if !self.unstaged.iter().any(|r| r.uri == uri) {
                self.unstaged.push(resource);
            }
            true
        } else {
            false
        }
    }

    /// Stage all unstaged resources.
    pub fn stage_all(&mut self) {
        let moved: Vec<ScmResource> = self.unstaged.drain(..).collect();
        for r in moved {
            if !self.staged.iter().any(|s| s.uri == r.uri) {
                self.staged.push(r);
            }
        }
    }

    /// Unstage all staged resources.
    pub fn unstage_all(&mut self) {
        let moved: Vec<ScmResource> = self.staged.drain(..).collect();
        for r in moved {
            if !self.unstaged.iter().any(|u| u.uri == r.uri) {
                self.unstaged.push(r);
            }
        }
    }

    /// Discard a resource from unstaged changes by URI.
    pub fn discard(&mut self, uri: &str) -> bool {
        let before = self.unstaged.len();
        self.unstaged.retain(|r| r.uri != uri);
        self.unstaged.len() < before
    }

    pub fn staged(&self) -> &[ScmResource] {
        &self.staged
    }

    pub fn unstaged(&self) -> &[ScmResource] {
        &self.unstaged
    }

    pub fn staged_count(&self) -> usize {
        self.staged.len()
    }

    pub fn unstaged_count(&self) -> usize {
        self.unstaged.len()
    }

    pub fn total_count(&self) -> usize {
        self.staged.len() + self.unstaged.len()
    }

    pub fn is_clean(&self) -> bool {
        self.staged.is_empty() && self.unstaged.is_empty()
    }

    /// Produce `SourceControlGroup` entries for staged and unstaged.
    pub fn to_groups(&self) -> Vec<SourceControlGroup> {
        vec![
            SourceControlGroup {
                id: "staged".to_string(),
                label: "Staged Changes".to_string(),
                resources: self.staged.clone(),
            },
            SourceControlGroup {
                id: "changes".to_string(),
                label: "Changes".to_string(),
                resources: self.unstaged.clone(),
            },
        ]
    }
}

impl fmt::Display for ChangeTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} staged, {} unstaged",
            self.staged.len(),
            self.unstaged.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Commit message templates
// ---------------------------------------------------------------------------

/// A reusable commit message template with placeholder substitution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommitTemplate {
    /// Human-readable name of the template.
    pub name: String,
    /// Template body, may contain `{placeholders}`.
    pub body: String,
}

impl CommitTemplate {
    pub fn new(name: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            body: body.into(),
        }
    }

    /// Substitute `{key}` placeholders with provided values.
    pub fn render(&self, vars: &HashMap<String, String>) -> String {
        let mut result = self.body.clone();
        for (key, value) in vars {
            let placeholder = format!("{{{key}}}");
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// Return the set of placeholder names found in the template body.
    pub fn placeholders(&self) -> Vec<String> {
        let mut out = Vec::new();
        let bytes = self.body.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'{' {
                if let Some(end) = self.body[i + 1..].find('}') {
                    let name = &self.body[i + 1..i + 1 + end];
                    if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        let s = name.to_string();
                        if !out.contains(&s) {
                            out.push(s);
                        }
                    }
                    i += end + 2;
                    continue;
                }
            }
            i += 1;
        }
        out
    }
}

impl fmt::Display for CommitTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.name, self.body)
    }
}

/// A collection of commit templates.
#[derive(Debug, Clone, Default)]
pub struct TemplateRegistry {
    templates: Vec<CommitTemplate>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a template. Replaces any existing template with the same name.
    pub fn register(&mut self, template: CommitTemplate) {
        self.templates.retain(|t| t.name != template.name);
        self.templates.push(template);
    }

    pub fn get(&self, name: &str) -> Option<&CommitTemplate> {
        self.templates.iter().find(|t| t.name == name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.templates.iter().map(|t| t.name.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.templates.len();
        self.templates.retain(|t| t.name != name);
        self.templates.len() < before
    }
}

// ---------------------------------------------------------------------------
// SCM input-box validation
// ---------------------------------------------------------------------------

/// Result of validating an SCM input-box value (e.g. commit message draft).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputBoxSeverity {
    Info,
    Warning,
    Error,
}

/// A single validation diagnostic for the input box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputBoxDiagnostic {
    pub severity: InputBoxSeverity,
    pub message: String,
}

/// Validate an SCM input-box value, returning diagnostics.
///
/// Checks: non-empty, max length, subject-line length.
pub fn validate_input_box(value: &str, max_length: usize) -> Vec<InputBoxDiagnostic> {
    let mut diags = Vec::new();
    if value.is_empty() {
        return diags; // empty is fine—not yet typed
    }
    if value.len() > max_length {
        diags.push(InputBoxDiagnostic {
            severity: InputBoxSeverity::Error,
            message: format!("message exceeds {} characters", max_length),
        });
    }
    if let Some(subject) = value.lines().next() {
        if subject.len() > 72 {
            diags.push(InputBoxDiagnostic {
                severity: InputBoxSeverity::Warning,
                message: format!(
                    "subject line is {} chars; convention is ≤72",
                    subject.len()
                ),
            });
        }
    }
    // Warn on trailing whitespace in any line.
    for (idx, line) in value.lines().enumerate() {
        if line != line.trim_end() {
            diags.push(InputBoxDiagnostic {
                severity: InputBoxSeverity::Info,
                message: format!("trailing whitespace on line {}", idx + 1),
            });
            break; // report once
        }
    }
    diags
}

// ---------------------------------------------------------------------------
// Provider registry snapshot
// ---------------------------------------------------------------------------

/// A point-in-time snapshot of the SCM provider registry, useful for diffing
/// state between ticks or serialising to the extension host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistrySnapshot {
    pub providers: Vec<SourceControl>,
    /// Monotonic sequence number at the time of capture.
    pub seq: u64,
}

impl ScmBridge {
    /// Capture a snapshot of the current provider state.
    pub fn snapshot(&self, seq: u64) -> RegistrySnapshot {
        RegistrySnapshot {
            providers: self.providers.clone(),
            seq,
        }
    }

    /// Compute which provider IDs were added or removed between two snapshots.
    pub fn diff_snapshots(
        old: &RegistrySnapshot,
        new: &RegistrySnapshot,
    ) -> (Vec<String>, Vec<String>) {
        let old_ids: std::collections::HashSet<&str> =
            old.providers.iter().map(|p| p.id.as_str()).collect();
        let new_ids: std::collections::HashSet<&str> =
            new.providers.iter().map(|p| p.id.as_str()).collect();

        let added: Vec<String> = new_ids
            .difference(&old_ids)
            .map(|s| s.to_string())
            .collect();
        let removed: Vec<String> = old_ids
            .difference(&new_ids)
            .map(|s| s.to_string())
            .collect();
        (added, removed)
    }

    /// Compute resource-level changes between two snapshots for a given
    /// provider, returning `(added_uris, removed_uris)`.
    pub fn diff_provider_resources(
        old: &RegistrySnapshot,
        new: &RegistrySnapshot,
        provider_id: &str,
    ) -> (Vec<String>, Vec<String>) {
        let collect_uris = |snap: &RegistrySnapshot| -> std::collections::HashSet<String> {
            snap.providers
                .iter()
                .filter(|p| p.id == provider_id)
                .flat_map(|p| &p.groups)
                .flat_map(|g| &g.resources)
                .map(|r| r.uri.clone())
                .collect()
        };
        let old_uris = collect_uris(old);
        let new_uris = collect_uris(new);

        let added: Vec<String> = new_uris.difference(&old_uris).cloned().collect();
        let removed: Vec<String> = old_uris.difference(&new_uris).cloned().collect();
        (added, removed)
    }
}

// ── ScmDiffStat ─────────────────────────────────────────────────────────

/// Statistics about a set of source control changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScmDiffStat {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

impl ScmDiffStat {
    pub fn new(files_changed: usize, insertions: usize, deletions: usize) -> Self {
        Self { files_changed, insertions, deletions }
    }

    pub fn total_changes(&self) -> usize { self.insertions + self.deletions }

    /// Ratio of insertions to total changes.
    pub fn change_ratio(&self) -> f64 {
        let total = self.total_changes();
        if total == 0 { return 0.0; }
        self.insertions as f64 / total as f64
    }

    /// Returns true if total changes exceed the threshold.
    pub fn is_large_diff(&self, threshold: usize) -> bool {
        self.total_changes() > threshold
    }

    pub fn is_empty(&self) -> bool { self.files_changed == 0 && self.insertions == 0 && self.deletions == 0 }
}

impl fmt::Display for ScmDiffStat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} files, +{} -{}", self.files_changed, self.insertions, self.deletions)
    }
}

// ── ScmBranchTracker ────────────────────────────────────────────────────

/// Tracks the current branch and available branches.
#[derive(Debug, Clone)]
pub struct ScmBranchTracker {
    current: String,
    branches: Vec<String>,
}

impl ScmBranchTracker {
    pub fn new(current: &str) -> Self {
        Self { current: current.to_string(), branches: vec![current.to_string()] }
    }

    pub fn current_branch(&self) -> &str { &self.current }

    pub fn switch_branch(&mut self, branch: &str) -> bool {
        if self.branches.iter().any(|b| b == branch) {
            self.current = branch.to_string();
            true
        } else {
            false
        }
    }

    pub fn add_branch(&mut self, branch: &str) {
        if !self.has_branch(branch) { self.branches.push(branch.to_string()); }
    }

    pub fn remove_branch(&mut self, branch: &str) -> bool {
        if branch == self.current { return false; }
        if let Some(pos) = self.branches.iter().position(|b| b == branch) {
            self.branches.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn has_branch(&self, branch: &str) -> bool { self.branches.iter().any(|b| b == branch) }
    pub fn branch_count(&self) -> usize { self.branches.len() }
    pub fn all_branches(&self) -> &[String] { &self.branches }
}

// ── ScmConflictDetector ─────────────────────────────────────────────────

/// Detects and manages merge conflict resolution.
#[derive(Debug, Clone)]
pub struct ScmConflictDetector {
    conflicting: Vec<String>,
    resolved: Vec<String>,
}

impl ScmConflictDetector {
    pub fn new() -> Self { Self { conflicting: Vec::new(), resolved: Vec::new() } }

    /// Detect conflicts from a list of (file, has_conflict) pairs.
    pub fn detect_from_status(&mut self, statuses: &[(&str, bool)]) {
        for &(file, conflict) in statuses {
            if conflict && !self.conflicting.contains(&file.to_string()) {
                self.conflicting.push(file.to_string());
            }
        }
    }

    pub fn conflict_count(&self) -> usize { self.conflicting.len() }

    pub fn conflicting_files(&self) -> &[String] { &self.conflicting }

    pub fn all_resolved(&self) -> bool { self.conflicting.is_empty() }

    pub fn mark_resolved(&mut self, file: &str) -> bool {
        if let Some(pos) = self.conflicting.iter().position(|f| f == file) {
            let removed = self.conflicting.remove(pos);
            self.resolved.push(removed);
            true
        } else {
            false
        }
    }

    pub fn resolved_count(&self) -> usize { self.resolved.len() }
}


/// Source control configuration manager.
#[derive(Debug, Clone)]
pub struct ExtScmConfig {
    entries: Vec<ExtScmEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single source control entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtScmEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl ExtScmEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl ExtScmConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: ExtScmEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&ExtScmEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ExtScmEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&ExtScmEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&ExtScmEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&ExtScmEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<ExtScmEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Source control provider API — extended utilities (yz)
// ---------------------------------------------------------------------------

/// Metric accumulator for ext_scm operations.
#[derive(Debug, Clone)]
pub struct YzMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YzMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for ext_scm.
#[derive(Debug, Clone)]
pub struct YzRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YzRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for ext_scm lookups.
#[derive(Debug, Clone)]
pub struct YzLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YzLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for ext_scm
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtScmRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtScmRingBuf {
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
pub struct XaExtScmCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtScmCounter {
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

impl Default for XaExtScmCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 69
// ---------------------------------------------------------------------------

/// Generic object pool `Xc69Pool<T>`.
pub struct Xc69Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc69Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc69PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc69Pool<T> {
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
    pub fn stats(&self) -> Xc69PoolStats {
        Xc69PoolStats {
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

impl<T> Default for Xc69Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc69Scheduler`.
pub struct Xc69Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc69Scheduler {
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

impl Default for Xc69Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_69 hash for the given byte slice.
pub fn xc_69_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_69 convention.
pub fn xc_69_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_114 deepening: state machine + event bus ---

/// States for the Xd114 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd114State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd114State {
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
pub struct Xd114Transition {
    pub from: Xd114State,
    pub to: Xd114State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd114StateMachine {
    current: Xd114State,
    history: Vec<Xd114Transition>,
    step_counter: usize,
}

impl Xd114StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd114State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd114State {
        self.current
    }

    pub fn history(&self) -> &[Xd114Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd114State) -> Result<Xd114State, String> {
        let allowed = match (self.current, target) {
            (Xd114State::Idle, Xd114State::Running) => true,
            (Xd114State::Running, Xd114State::Paused) => true,
            (Xd114State::Running, Xd114State::Done) => true,
            (Xd114State::Paused, Xd114State::Running) => true,
            (Xd114State::Paused, Xd114State::Done) => true,
            (Xd114State::Done, Xd114State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_114: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd114Transition {
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
            "Xd114SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd114State> {
        let prefix = "Xd114SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd114State::Idle),
            "Running" => Some(Xd114State::Running),
            "Paused" => Some(Xd114State::Paused),
            "Done" => Some(Xd114State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd114State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd114 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd114Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd114Event {
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

type Xd114HandlerFn = Box<dyn Fn(&Xd114Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd114EventBus {
    handlers: Vec<(usize, Option<String>, Xd114HandlerFn)>,
    next_id: usize,
    published: Vec<Xd114Event>,
}

impl Xd114EventBus {
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
        F: Fn(&Xd114Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd114Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd114Event) {
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

    pub fn published_events(&self) -> &[Xd114Event] {
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

    #[test]
    fn ext_scm_stats_new_defaults() {
        let stats = ExtScmStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_scm_stats_record_success() {
        let mut stats = ExtScmStats::new();
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
    fn ext_scm_stats_record_failure() {
        let mut stats = ExtScmStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_scm_stats_reset() {
        let mut stats = ExtScmStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_scm_stats_merge() {
        let mut a = ExtScmStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtScmStats::new();
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
    fn ext_scm_stats_display() {
        let mut stats = ExtScmStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_scm_stats_default() {
        let stats = ExtScmStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_scm_validator_accepts_valid_name() {
        let v = ExtScmValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_scm_validator_rejects_empty() {
        let v = ExtScmValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_scm_validator_rejects_too_long() {
        let v = ExtScmValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_scm_validator_forbidden_prefix() {
        let v = ExtScmValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_scm_validator_allowed_chars() {
        let v = ExtScmValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_scm_validator_range() {
        let v = ExtScmValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_scm_sanitize_removes_control() {
        let result = ExtScmValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_scm_truncate_short_string() {
        assert_eq!(ExtScmValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_scm_truncate_long_string() {
        let result = ExtScmValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_scm_is_ascii_printable() {
        assert!(ExtScmValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtScmValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- ScmHistoryItem --

    #[test]
    fn history_item_short_id() {
        let item = ScmHistoryItem::new("abc1234567890", "Fix bug", "Alice", 1000, "src/main.rs");
        assert_eq!(item.short_id(), "abc1234");
    }

    #[test]
    fn history_item_short_id_already_short() {
        let item = ScmHistoryItem::new("abc", "Fix", "Bob", 100, "f.rs");
        assert_eq!(item.short_id(), "abc");
    }

    #[test]
    fn history_item_subject() {
        let item = ScmHistoryItem::new("abc", "First line\nSecond line", "A", 100, "f.rs");
        assert_eq!(item.subject(), "First line");
    }

    #[test]
    fn history_item_display() {
        let item = ScmHistoryItem::new("abc1234567890", "Fix bug", "Alice", 1000, "src/main.rs");
        let s = format!("{item}");
        assert!(s.contains("abc1234"));
        assert!(s.contains("Fix bug"));
        assert!(s.contains("Alice"));
    }

    #[test]
    fn history_newest_first() {
        let mut h = ScmHistory::new();
        h.push(ScmHistoryItem::new("a", "old", "A", 100, "f.rs"));
        h.push(ScmHistoryItem::new("b", "new", "A", 300, "f.rs"));
        h.push(ScmHistoryItem::new("c", "mid", "A", 200, "f.rs"));
        let sorted = h.newest_first();
        assert_eq!(sorted[0].id, "b");
        assert_eq!(sorted[1].id, "c");
        assert_eq!(sorted[2].id, "a");
    }

    #[test]
    fn history_by_author() {
        let mut h = ScmHistory::new();
        h.push(ScmHistoryItem::new("a", "m", "Alice", 100, "f.rs"));
        h.push(ScmHistoryItem::new("b", "m", "Bob", 200, "f.rs"));
        h.push(ScmHistoryItem::new("c", "m", "Alice", 300, "f.rs"));
        assert_eq!(h.by_author("Alice").len(), 2);
        assert_eq!(h.by_author("Bob").len(), 1);
    }

    #[test]
    fn history_latest() {
        let mut h = ScmHistory::new();
        h.push(ScmHistoryItem::new("a", "old", "A", 100, "f.rs"));
        h.push(ScmHistoryItem::new("b", "new", "A", 500, "f.rs"));
        assert_eq!(h.latest().unwrap().id, "b");
    }

    // -- ScmBridge helpers -------------------------------------------------

    #[test]
    fn bridge_all_resources() {
        let mut bridge = ScmBridge::new();
        bridge.register_provider("git", "Git", None);
        bridge.create_group("git", "changes", "Changes");
        bridge.handle_message(&ScmMessage::UpdateResources {
            provider_id: "git".into(),
            group_id: "changes".into(),
            resources: vec![
                ScmResource::plain("file:///a.rs"),
                ScmResource::plain("file:///b.rs"),
            ],
        });
        let all = bridge.all_resources();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn bridge_providers_with_changes() {
        let mut bridge = ScmBridge::new();
        bridge.register_provider("git", "Git", None);
        bridge.register_provider("svn", "SVN", None);
        bridge.create_group("git", "changes", "Changes");
        bridge.handle_message(&ScmMessage::UpdateResources {
            provider_id: "git".into(),
            group_id: "changes".into(),
            resources: vec![ScmResource::plain("file:///a.rs")],
        });
        let with_changes = bridge.providers_with_changes();
        assert_eq!(with_changes.len(), 1);
        assert_eq!(with_changes[0].id, "git");
    }

    #[test]
    fn bridge_summary_display() {
        let bridge = ScmBridge::new();
        let s = format!("{}", bridge);
        assert!(s.contains("no providers"));

        let mut bridge2 = ScmBridge::new();
        bridge2.register_provider("git", "Git", None);
        let s2 = bridge2.summary();
        assert!(s2.contains("Git"));
    }

    #[test]
    fn source_control_group_count_and_total() {
        let mut bridge = ScmBridge::new();
        bridge.register_provider("git", "Git", None);
        bridge.create_group("git", "staged", "Staged");
        bridge.create_group("git", "changes", "Changes");
        let p = bridge.get_provider("git").unwrap();
        assert_eq!(p.group_count(), 2);
        assert_eq!(p.total_resources(), 0);
    }

    #[test]
    fn source_control_group_is_empty() {
        let g = SourceControlGroup {
            id: "test".into(),
            label: "Test".into(),
            resources: Vec::new(),
        };
        assert!(g.is_empty());
    }

    #[test]
    fn scm_resource_extension() {
        let r = ScmResource::plain("file:///src/main.rs");
        assert_eq!(r.extension(), Some("rs"));

        let no_ext = ScmResource::plain("file:///Makefile");
        assert!(no_ext.extension().is_none());
    }

    #[test]
    fn scm_resource_extension_multi_dot() {
        let r = ScmResource::plain("file:///archive.tar.gz");
        assert_eq!(r.extension(), Some("gz"));
    }

    #[test]
    fn group_resources_by_directory_groups_correctly() {
        let resources = vec![
            ScmResource::plain("file:///src/main.rs"),
            ScmResource::plain("file:///src/lib.rs"),
            ScmResource::plain("file:///tests/test1.rs"),
        ];
        let groups = group_resources_by_directory(&resources);
        assert_eq!(groups.get("file:///src").unwrap().len(), 2);
        assert_eq!(groups.get("file:///tests").unwrap().len(), 1);
    }

    #[test]
    fn diff_stats_from_unified_diff() {
        let diff = "\
--- a/file.rs
+++ b/file.rs
@@ -1,3 +1,4 @@
 line1
-old_line
+new_line
+added_line
 line3
";
        let stats = DiffStats::from_unified_diff(diff);
        assert_eq!(stats.additions, 2);
        assert_eq!(stats.deletions, 1);
        assert_eq!(stats.total_changes(), 3);
        assert!(!stats.is_empty());
    }

    #[test]
    fn diff_stats_empty_diff() {
        let stats = DiffStats::from_unified_diff("");
        assert!(stats.is_empty());
    }

    #[test]
    fn commit_message_validator_valid_message() {
        let v = CommitMessageValidator::new();
        let warnings = v.validate("Fix a bug\n\nThis fixes issue #42.");
        assert!(warnings.is_empty());
    }

    #[test]
    fn commit_message_validator_long_subject() {
        let v = CommitMessageValidator::new();
        let long_subject = "x".repeat(80);
        let warnings = v.validate(&long_subject);
        assert!(warnings.iter().any(|w| w.contains("subject line")));
    }

    #[test]
    fn commit_message_validator_trailing_period() {
        let v = CommitMessageValidator::new();
        let warnings = v.validate("Fix a bug.");
        assert!(warnings.iter().any(|w| w.contains("period")));
    }

    #[test]
    fn validate_branch_name_valid() {
        assert!(validate_branch_name("feature/my-branch").is_ok());
        assert!(validate_branch_name("main").is_ok());
    }

    #[test]
    fn validate_branch_name_invalid() {
        assert!(validate_branch_name("").is_err());
        assert!(validate_branch_name("-bad").is_err());
        assert!(validate_branch_name("a..b").is_err());
        assert!(validate_branch_name("a b").is_err());
        assert!(validate_branch_name("foo/").is_err());
        assert!(validate_branch_name("ref@{1}").is_err());
    }

    // -- ScmHistory additional methods --------------------------------------

    #[test]
    fn history_in_time_range() {
        let mut h = ScmHistory::new();
        h.push(ScmHistoryItem::new("a", "m", "A", 100, "f.rs"));
        h.push(ScmHistoryItem::new("b", "m", "B", 200, "f.rs"));
        h.push(ScmHistoryItem::new("c", "m", "C", 300, "f.rs"));
        let range = h.in_time_range(150, 250);
        assert_eq!(range.len(), 1);
        assert_eq!(range[0].id, "b");
    }

    #[test]
    fn history_authors() {
        let mut h = ScmHistory::new();
        h.push(ScmHistoryItem::new("a", "m", "Alice", 100, "f.rs"));
        h.push(ScmHistoryItem::new("b", "m", "Bob", 200, "f.rs"));
        h.push(ScmHistoryItem::new("c", "m", "Alice", 300, "f.rs"));
        let authors = h.authors();
        assert_eq!(authors, vec!["Alice", "Bob"]);
    }

    #[test]
    fn history_oldest_and_clear() {
        let mut h = ScmHistory::new();
        h.push(ScmHistoryItem::new("a", "m", "A", 300, "f.rs"));
        h.push(ScmHistoryItem::new("b", "m", "A", 100, "f.rs"));
        assert_eq!(h.oldest().unwrap().id, "b");
        h.clear();
        assert!(h.is_empty());
    }

    // -- SourceControl additional methods -----------------------------------

    #[test]
    fn source_control_all_resources_and_groups() {
        let mut bridge = ScmBridge::new();
        bridge.register_provider("git", "Git", None);
        bridge.create_group("git", "changes", "Changes");
        bridge.create_group("git", "staged", "Staged");
        bridge.handle_message(&ScmMessage::UpdateResources {
            provider_id: "git".into(),
            group_id: "changes".into(),
            resources: vec![ScmResource::plain("file:///a.rs")],
        });
        let p = bridge.get_provider("git").unwrap();
        assert_eq!(p.all_resources().len(), 1);
        assert!(p.get_group("changes").is_some());
        assert!(p.get_group("nonexistent").is_none());
        let ids = p.group_ids();
        assert!(ids.contains(&"changes"));
        assert!(ids.contains(&"staged"));
    }

    // -- DiffStats additional methods ---------------------------------------

    #[test]
    fn diff_stats_merge_and_ratio() {
        let a = DiffStats::new(10, 5, 1);
        let b = DiffStats::new(3, 2, 1);
        let merged = a.merge(&b);
        assert_eq!(merged.additions, 13);
        assert_eq!(merged.deletions, 7);
        assert_eq!(merged.files_changed, 2);
        let ratio = a.addition_ratio();
        assert!((ratio - 10.0 / 15.0).abs() < 0.001);
    }

    #[test]
    fn diff_stats_addition_ratio_empty() {
        let empty = DiffStats::new(0, 0, 0);
        assert!((empty.addition_ratio() - 0.0).abs() < f64::EPSILON);
    }

    // -- CommitMessageValidator additional methods --------------------------

    #[test]
    fn commit_message_validator_is_valid() {
        let v = CommitMessageValidator::new();
        assert!(v.is_valid("Fix a bug\n\nDetails here"));
        assert!(!v.is_valid("Fix a bug."));
    }

    #[test]
    fn commit_message_validator_custom_limits() {
        let v = CommitMessageValidator::new()
            .with_max_subject(10)
            .with_max_body_line(20);
        let warnings = v.validate("Short\n\nOk body");
        assert!(warnings.is_empty());
        let warnings = v.validate("This subject is way too long for the limit");
        assert!(!warnings.is_empty());
    }

    // -- Merge conflict detection ------------------------------------------

    #[test]
    fn detect_merge_conflicts_finds_single_conflict() {
        let content = "\
before
<<<<<<< HEAD
our line
=======
their line
>>>>>>> branch
after
";
        let conflicts = detect_merge_conflicts(content);
        assert_eq!(conflicts.len(), 1);
        let c = &conflicts[0];
        assert_eq!(c.start_line, 2);
        assert_eq!(c.separator_line, 4);
        assert_eq!(c.end_line, 6);
        assert_eq!(c.ours, vec!["our line"]);
        assert_eq!(c.theirs, vec!["their line"]);
        assert_eq!(c.total_lines(), 2);
        assert!(!c.is_trivial());
        // Display
        let s = format!("{c}");
        assert!(s.contains("conflict at lines 2-6"));
    }

    #[test]
    fn detect_merge_conflicts_multiple() {
        let content = "\
<<<<<<< HEAD
a
=======
b
>>>>>>> x
middle
<<<<<<< HEAD
c
d
=======
e
>>>>>>> y
";
        let conflicts = detect_merge_conflicts(content);
        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].ours, vec!["a"]);
        assert_eq!(conflicts[1].ours, vec!["c", "d"]);
        assert_eq!(conflicts[1].theirs, vec!["e"]);
    }

    #[test]
    fn detect_merge_conflicts_none() {
        assert!(detect_merge_conflicts("normal file\nno conflicts\n").is_empty());
    }

    #[test]
    fn has_merge_conflicts_flag() {
        assert!(has_merge_conflicts("<<<<<<< HEAD\nfoo\n=======\nbar\n>>>>>>> b\n"));
        assert!(!has_merge_conflicts("clean file"));
    }

    #[test]
    fn merge_conflict_trivial() {
        let content = "\
<<<<<<< HEAD
=======
their stuff
>>>>>>> b
";
        let conflicts = detect_merge_conflicts(content);
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].is_trivial()); // ours is empty
    }

    // -- Change tracker (staged/unstaged) ----------------------------------

    #[test]
    fn change_tracker_stage_unstage_lifecycle() {
        let mut ct = ChangeTracker::new();
        assert!(ct.is_clean());

        ct.add_unstaged(ScmResource::plain("file:///a.rs"));
        ct.add_unstaged(ScmResource::plain("file:///b.rs"));
        assert_eq!(ct.unstaged_count(), 2);
        assert_eq!(ct.staged_count(), 0);
        assert_eq!(ct.total_count(), 2);

        // Stage one
        assert!(ct.stage("file:///a.rs"));
        assert_eq!(ct.staged_count(), 1);
        assert_eq!(ct.unstaged_count(), 1);

        // Unstage it back
        assert!(ct.unstage("file:///a.rs"));
        assert_eq!(ct.staged_count(), 0);
        assert_eq!(ct.unstaged_count(), 2);

        // Stage all
        ct.stage_all();
        assert_eq!(ct.staged_count(), 2);
        assert_eq!(ct.unstaged_count(), 0);

        // Unstage all
        ct.unstage_all();
        assert_eq!(ct.staged_count(), 0);
        assert_eq!(ct.unstaged_count(), 2);
    }

    #[test]
    fn change_tracker_discard() {
        let mut ct = ChangeTracker::new();
        ct.add_unstaged(ScmResource::plain("file:///a.rs"));
        assert!(ct.discard("file:///a.rs"));
        assert!(ct.is_clean());
        assert!(!ct.discard("file:///nonexistent"));
    }

    #[test]
    fn change_tracker_to_groups() {
        let mut ct = ChangeTracker::new();
        ct.add_unstaged(ScmResource::plain("file:///a.rs"));
        ct.stage("file:///a.rs");
        ct.add_unstaged(ScmResource::plain("file:///b.rs"));
        let groups = ct.to_groups();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].id, "staged");
        assert_eq!(groups[0].resources.len(), 1);
        assert_eq!(groups[1].id, "changes");
        assert_eq!(groups[1].resources.len(), 1);
    }

    #[test]
    fn change_tracker_display() {
        let mut ct = ChangeTracker::new();
        ct.add_unstaged(ScmResource::plain("file:///a.rs"));
        ct.stage("file:///a.rs");
        let s = format!("{ct}");
        assert!(s.contains("1 staged"));
    }

    #[test]
    fn change_tracker_no_duplicate_add() {
        let mut ct = ChangeTracker::new();
        ct.add_unstaged(ScmResource::plain("file:///a.rs"));
        ct.add_unstaged(ScmResource::plain("file:///a.rs"));
        assert_eq!(ct.unstaged_count(), 1);
    }

    // -- Commit templates --------------------------------------------------

    #[test]
    fn commit_template_render_and_placeholders() {
        let tpl = CommitTemplate::new("feat", "feat({scope}): {description}");
        let placeholders = tpl.placeholders();
        assert_eq!(placeholders, vec!["scope", "description"]);

        let mut vars = HashMap::new();
        vars.insert("scope".to_string(), "auth".to_string());
        vars.insert("description".to_string(), "add login".to_string());
        let rendered = tpl.render(&vars);
        assert_eq!(rendered, "feat(auth): add login");
    }

    #[test]
    fn commit_template_no_placeholders() {
        let tpl = CommitTemplate::new("simple", "just a message");
        assert!(tpl.placeholders().is_empty());
        let rendered = tpl.render(&HashMap::new());
        assert_eq!(rendered, "just a message");
    }

    #[test]
    fn commit_template_display() {
        let tpl = CommitTemplate::new("fix", "fix: {msg}");
        let s = format!("{tpl}");
        assert!(s.contains("[fix]"));
    }

    #[test]
    fn template_registry_operations() {
        let mut reg = TemplateRegistry::new();
        assert!(reg.is_empty());

        reg.register(CommitTemplate::new("feat", "feat: {msg}"));
        reg.register(CommitTemplate::new("fix", "fix: {msg}"));
        assert_eq!(reg.len(), 2);
        assert!(reg.names().contains(&"feat"));
        assert!(reg.get("feat").is_some());

        // Replace existing
        reg.register(CommitTemplate::new("feat", "feat({scope}): {msg}"));
        assert_eq!(reg.len(), 2);
        assert!(reg.get("feat").unwrap().body.contains("{scope}"));

        assert!(reg.remove("fix"));
        assert_eq!(reg.len(), 1);
        assert!(!reg.remove("nonexistent"));
    }

    // -- Input-box validation ----------------------------------------------

    #[test]
    fn validate_input_box_empty_ok() {
        let diags = validate_input_box("", 1000);
        assert!(diags.is_empty());
    }

    #[test]
    fn validate_input_box_too_long() {
        let diags = validate_input_box(&"x".repeat(200), 100);
        assert!(diags.iter().any(|d| d.severity == InputBoxSeverity::Error));
    }

    #[test]
    fn validate_input_box_long_subject() {
        let long_subject = "a".repeat(80);
        let diags = validate_input_box(&long_subject, 1000);
        assert!(diags
            .iter()
            .any(|d| d.severity == InputBoxSeverity::Warning));
    }

    #[test]
    fn validate_input_box_trailing_whitespace() {
        let diags = validate_input_box("hello   \nworld", 1000);
        assert!(diags
            .iter()
            .any(|d| d.severity == InputBoxSeverity::Info));
    }

    // -- Registry snapshot -------------------------------------------------

    #[test]
    fn snapshot_and_diff() {
        let mut bridge = ScmBridge::new();
        bridge.register_provider("git", "Git", None);
        let snap1 = bridge.snapshot(1);

        bridge.register_provider("svn", "SVN", None);
        bridge.unregister_provider("git");
        let snap2 = bridge.snapshot(2);

        let (added, removed) = ScmBridge::diff_snapshots(&snap1, &snap2);
        assert_eq!(added, vec!["svn"]);
        assert_eq!(removed, vec!["git"]);
        assert_eq!(snap1.seq, 1);
        assert_eq!(snap2.seq, 2);
    }

    #[test]
    fn diff_provider_resources_between_snapshots() {
        let mut bridge = ScmBridge::new();
        bridge.register_provider("git", "Git", None);
        bridge.create_group("git", "changes", "Changes");
        bridge.handle_message(&ScmMessage::UpdateResources {
            provider_id: "git".into(),
            group_id: "changes".into(),
            resources: vec![
                ScmResource::plain("file:///a.rs"),
                ScmResource::plain("file:///b.rs"),
            ],
        });
        let snap1 = bridge.snapshot(1);

        bridge.handle_message(&ScmMessage::UpdateResources {
            provider_id: "git".into(),
            group_id: "changes".into(),
            resources: vec![
                ScmResource::plain("file:///b.rs"),
                ScmResource::plain("file:///c.rs"),
            ],
        });
        let snap2 = bridge.snapshot(2);

        let (added, removed) =
            ScmBridge::diff_provider_resources(&snap1, &snap2, "git");
        assert_eq!(added, vec!["file:///c.rs"]);
        assert_eq!(removed, vec!["file:///a.rs"]);
    }

    // ── ScmDiffStat tests ──

    #[test]
    fn diff_stat_total_changes() {
        let s = ScmDiffStat::new(3, 20, 10);
        assert_eq!(s.total_changes(), 30);
        assert!(!s.is_empty());
    }

    #[test]
    fn diff_stat_change_ratio() {
        let s = ScmDiffStat::new(1, 75, 25);
        assert!((s.change_ratio() - 0.75).abs() < 0.01);
    }

    #[test]
    fn diff_stat_is_large() {
        let s = ScmDiffStat::new(1, 500, 500);
        assert!(s.is_large_diff(999));
        assert!(!s.is_large_diff(1000));
    }

    #[test]
    fn diff_stat_display() {
        let s = ScmDiffStat::new(2, 10, 5);
        assert_eq!(format!("{}", s), "2 files, +10 -5");
    }

    // ── ScmBranchTracker tests ──

    #[test]
    fn branch_tracker_current() {
        let t = ScmBranchTracker::new("main");
        assert_eq!(t.current_branch(), "main");
        assert_eq!(t.branch_count(), 1);
    }

    #[test]
    fn branch_tracker_switch() {
        let mut t = ScmBranchTracker::new("main");
        t.add_branch("feature");
        assert!(t.switch_branch("feature"));
        assert_eq!(t.current_branch(), "feature");
        assert!(!t.switch_branch("nonexist"));
    }

    #[test]
    fn branch_tracker_remove() {
        let mut t = ScmBranchTracker::new("main");
        t.add_branch("feature");
        assert!(t.remove_branch("feature"));
        assert!(!t.remove_branch("main")); // can't remove current
    }

    #[test]
    fn branch_tracker_add_dedup() {
        let mut t = ScmBranchTracker::new("main");
        t.add_branch("main");
        assert_eq!(t.branch_count(), 1);
    }

    // ── ScmConflictDetector tests ──

    #[test]
    fn conflict_detect_and_resolve() {
        let mut d = ScmConflictDetector::new();
        d.detect_from_status(&[("a.rs", true), ("b.rs", false), ("c.rs", true)]);
        assert_eq!(d.conflict_count(), 2);
        assert!(!d.all_resolved());
        assert!(d.mark_resolved("a.rs"));
        assert!(d.mark_resolved("c.rs"));
        assert!(d.all_resolved());
        assert_eq!(d.resolved_count(), 2);
    }

    #[test]
    fn conflict_mark_resolved_unknown() {
        let mut d = ScmConflictDetector::new();
        assert!(!d.mark_resolved("nonexist.rs"));
    }

    #[test]
    fn conflict_detect_no_duplicates() {
        let mut d = ScmConflictDetector::new();
        d.detect_from_status(&[("a.rs", true)]);
        d.detect_from_status(&[("a.rs", true)]);
        assert_eq!(d.conflict_count(), 1);
    }

    #[test]
    fn ext_scm_entry_creation() {
        let e = ExtScmEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn ext_scm_entry_with_priority() {
        let e = ExtScmEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn ext_scm_entry_metadata() {
        let e = ExtScmEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn ext_scm_entry_remove_meta() {
        let mut e = ExtScmEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn ext_scm_entry_activate_deactivate() {
        let mut e = ExtScmEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn ext_scm_config_add_sorted() {
        let mut c = ExtScmConfig::new(10);
        c.add(ExtScmEntry::new("lo", "Lo").with_priority(1));
        c.add(ExtScmEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn ext_scm_config_capacity() {
        let mut c = ExtScmConfig::new(1);
        assert!(c.add(ExtScmEntry::new("a", "A")));
        assert!(!c.add(ExtScmEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn ext_scm_config_remove() {
        let mut c = ExtScmConfig::new(10);
        c.add(ExtScmEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn ext_scm_config_get() {
        let mut c = ExtScmConfig::new(10);
        c.add(ExtScmEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn ext_scm_config_active_entries() {
        let mut c = ExtScmConfig::new(10);
        c.add(ExtScmEntry::new("a", "A"));
        c.add(ExtScmEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn ext_scm_config_enable_disable() {
        let mut c = ExtScmConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn ext_scm_config_clear() {
        let mut c = ExtScmConfig::new(10);
        c.add(ExtScmEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn ext_scm_config_find_by_label() {
        let mut c = ExtScmConfig::new(10);
        c.add(ExtScmEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn ext_scm_config_top_n() {
        let mut c = ExtScmConfig::new(10);
        c.add(ExtScmEntry::new("a", "A").with_priority(1));
        c.add(ExtScmEntry::new("b", "B").with_priority(2));
        c.add(ExtScmEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn ext_scm_config_deactivate_activate_all() {
        let mut c = ExtScmConfig::new(10);
        c.add(ExtScmEntry::new("a", "A"));
        c.add(ExtScmEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn ext_scm_config_highest_priority() {
        let mut c = ExtScmConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(ExtScmEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn ext_scm_config_contains() {
        let mut c = ExtScmConfig::new(10);
        c.add(ExtScmEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn ext_scm_config_labels() {
        let mut c = ExtScmConfig::new(10);
        c.add(ExtScmEntry::new("a", "Alpha"));
        c.add(ExtScmEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn ext_scm_config_drain_inactive() {
        let mut c = ExtScmConfig::new(10);
        c.add(ExtScmEntry::new("a", "A"));
        c.add(ExtScmEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn yz_metrics_empty() {
        let m = YzMetrics::new("ext_scm");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yz_metrics_record_and_mean() {
        let mut m = YzMetrics::new("ext_scm");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yz_metrics_min_max() {
        let mut m = YzMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yz_metrics_variance_and_std() {
        let mut m = YzMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn yz_metrics_percentile() {
        let mut m = YzMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yz_metrics_merge() {
        let mut a = YzMetrics::new("a");
        a.record(1.0);
        let mut b = YzMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yz_metrics_reset() {
        let mut m = YzMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yz_rate_window_empty() {
        let rw = YzRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yz_rate_window_tick_and_rate() {
        let mut rw = YzRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yz_lru_cache_basic() {
        let mut c = YzLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yz_lru_cache_contains_and_keys() {
        let mut c = YzLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yz_lru_cache_remove() {
        let mut c = YzLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yz_metrics_sum() {
        let mut m = YzMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yz_metrics_label() {
        let m = YzMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yz_lru_cache_clear() {
        let mut c = YzLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for ext_scm
    #[test]
    fn xa_ext_scm_ring_new() {
        let rb = super::XaExtScmRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_scm_ring_push_len() {
        let mut rb = super::XaExtScmRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_scm_ring_wrap() {
        let mut rb = super::XaExtScmRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_scm_ring_mean_empty() {
        let rb = super::XaExtScmRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_scm_ring_mean_values() {
        let mut rb = super::XaExtScmRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_scm_ring_min_max() {
        let mut rb = super::XaExtScmRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_scm_ring_iter() {
        let mut rb = super::XaExtScmRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_scm_counter_new() {
        let c = super::XaExtScmCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_scm_counter_inc() {
        let mut c = super::XaExtScmCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_scm_counter_inc_by() {
        let mut c = super::XaExtScmCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_scm_counter_reset() {
        let mut c = super::XaExtScmCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_scm_counter_clear() {
        let mut c = super::XaExtScmCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_scm_counter_default() {
        let c = super::XaExtScmCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 69 ----

    #[test]
    fn xc_69_pool_new_empty() {
        let pool: super::Xc69Pool<i32> = super::Xc69Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_69_pool_release_acquire() {
        let mut pool = super::Xc69Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_69_pool_acquire_empty() {
        let mut pool: super::Xc69Pool<i32> = super::Xc69Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_69_pool_full() {
        let mut pool = super::Xc69Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_69_pool_drain() {
        let mut pool = super::Xc69Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_69_pool_stats() {
        let mut pool = super::Xc69Pool::new(8);
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
    fn xc_69_pool_clear() {
        let mut pool = super::Xc69Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_69_pool_shrink() {
        let mut pool = super::Xc69Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_69_pool_default() {
        let pool: super::Xc69Pool<String> = super::Xc69Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_69_pool_extend() {
        let mut pool = super::Xc69Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_69_pool_retain() {
        let mut pool = super::Xc69Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_69_scheduler_round_robin() {
        let mut sched = super::Xc69Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_69_scheduler_empty() {
        let mut sched = super::Xc69Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_69_scheduler_reset() {
        let mut sched = super::Xc69Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_69_scheduler_add_remove() {
        let mut sched = super::Xc69Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_69_scheduler_targets() {
        let sched = super::Xc69Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_69_hash_empty() {
        assert_eq!(super::xc_69_hash(b""), 5381);
    }

    #[test]
    fn xc_69_hash_data() {
        let h = super::xc_69_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_69_hash(b"hello"), h);
    }

    #[test]
    fn xc_69_reverse_str() {
        assert_eq!(super::xc_69_reverse("abc"), "cba");
        assert_eq!(super::xc_69_reverse(""), "");
    }


    // --- xd_114 deepening tests ---

    #[test]
    fn xd_114_sm_initial_state() {
        let sm = Xd114StateMachine::new();
        assert_eq!(sm.current_state(), Xd114State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_114_sm_valid_idle_to_running() {
        let mut sm = Xd114StateMachine::new();
        assert!(sm.transition(Xd114State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd114State::Running);
    }

    #[test]
    fn xd_114_sm_valid_running_to_paused() {
        let mut sm = Xd114StateMachine::new();
        sm.transition(Xd114State::Running).unwrap();
        assert!(sm.transition(Xd114State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd114State::Paused);
    }

    #[test]
    fn xd_114_sm_valid_running_to_done() {
        let mut sm = Xd114StateMachine::new();
        sm.transition(Xd114State::Running).unwrap();
        assert!(sm.transition(Xd114State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd114State::Done);
    }

    #[test]
    fn xd_114_sm_valid_paused_to_running() {
        let mut sm = Xd114StateMachine::new();
        sm.transition(Xd114State::Running).unwrap();
        sm.transition(Xd114State::Paused).unwrap();
        assert!(sm.transition(Xd114State::Running).is_ok());
    }

    #[test]
    fn xd_114_sm_valid_done_to_idle() {
        let mut sm = Xd114StateMachine::new();
        sm.transition(Xd114State::Running).unwrap();
        sm.transition(Xd114State::Done).unwrap();
        assert!(sm.transition(Xd114State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd114State::Idle);
    }

    #[test]
    fn xd_114_sm_invalid_idle_to_done() {
        let mut sm = Xd114StateMachine::new();
        assert!(sm.transition(Xd114State::Done).is_err());
    }

    #[test]
    fn xd_114_sm_invalid_idle_to_paused() {
        let mut sm = Xd114StateMachine::new();
        assert!(sm.transition(Xd114State::Paused).is_err());
    }

    #[test]
    fn xd_114_sm_history_tracking() {
        let mut sm = Xd114StateMachine::new();
        sm.transition(Xd114State::Running).unwrap();
        sm.transition(Xd114State::Paused).unwrap();
        sm.transition(Xd114State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd114State::Idle);
        assert_eq!(sm.history()[0].to, Xd114State::Running);
        assert_eq!(sm.history()[1].from, Xd114State::Running);
        assert_eq!(sm.history()[2].to, Xd114State::Done);
    }

    #[test]
    fn xd_114_sm_serialize_deserialize() {
        let mut sm = Xd114StateMachine::new();
        sm.transition(Xd114State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd114StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd114State::Running));
    }

    #[test]
    fn xd_114_sm_deserialize_invalid() {
        assert_eq!(Xd114StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_114_sm_reset() {
        let mut sm = Xd114StateMachine::new();
        sm.transition(Xd114State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd114State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_114_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd114EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd114Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_114_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd114EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd114Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd114Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_114_bus_unsubscribe() {
        let mut bus = Xd114EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_114_event_kind_and_payload() {
        let e = Xd114Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd114Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_114_bus_clear_history() {
        let mut bus = Xd114EventBus::new();
        bus.publish(Xd114Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_114_sm_step_counter_increments() {
        let mut sm = Xd114StateMachine::new();
        sm.transition(Xd114State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd114State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }

}
