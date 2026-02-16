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
}
