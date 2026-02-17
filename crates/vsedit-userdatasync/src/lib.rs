//! Settings sync across devices.

pub mod extensions;
pub mod keybindings;
pub mod merge;
pub mod profile;
pub mod service;
pub mod snippets;
pub mod state;

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyncResource {
    Settings,
    Keybindings,
    Snippets,
    Extensions,
    GlobalState,
    Profiles,
}

impl fmt::Display for SyncResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncResource::Settings => write!(f, "Settings"),
            SyncResource::Keybindings => write!(f, "Keybindings"),
            SyncResource::Snippets => write!(f, "Snippets"),
            SyncResource::Extensions => write!(f, "Extensions"),
            SyncResource::GlobalState => write!(f, "Global State"),
            SyncResource::Profiles => write!(f, "Profiles"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    Idle,
    Syncing,
    Conflict,
    Error(String),
    UpToDate,
}

impl fmt::Display for SyncStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncStatus::Idle => write!(f, "Idle"),
            SyncStatus::Syncing => write!(f, "Syncing"),
            SyncStatus::Conflict => write!(f, "Conflict"),
            SyncStatus::Error(msg) => write!(f, "Error: {msg}"),
            SyncStatus::UpToDate => write!(f, "Up to Date"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncError {
    ResourceNotFound(String),
    AlreadyExists(String),
    SyncInProgress,
    NotEnabled,
    ConflictDetected,
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncError::ResourceNotFound(name) => write!(f, "resource not found: {name}"),
            SyncError::AlreadyExists(name) => write!(f, "resource already exists: {name}"),
            SyncError::SyncInProgress => write!(f, "sync already in progress"),
            SyncError::NotEnabled => write!(f, "sync is not enabled"),
            SyncError::ConflictDetected => write!(f, "conflict detected during sync"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SyncEntry {
    pub resource: SyncResource,
    pub local_version: u64,
    pub remote_version: Option<u64>,
    pub status: SyncStatus,
}

impl SyncEntry {
    /// Returns true if local and remote versions match.
    pub fn is_in_sync(&self) -> bool {
        self.remote_version == Some(self.local_version)
    }
}

pub struct SyncService {
    entries: Vec<SyncEntry>,
    enabled: bool,
}

impl SyncService {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            enabled: false,
        }
    }

    pub fn add_resource(&mut self, resource: SyncResource) {
        if !self.entries.iter().any(|e| e.resource == resource) {
            self.entries.push(SyncEntry {
                resource,
                local_version: 0,
                remote_version: None,
                status: SyncStatus::Idle,
            });
        }
    }

    pub fn remove_resource(&mut self, resource: &SyncResource) -> Result<(), SyncError> {
        let idx = self
            .entries
            .iter()
            .position(|e| &e.resource == resource)
            .ok_or_else(|| SyncError::ResourceNotFound(resource.to_string()))?;
        self.entries.remove(idx);
        Ok(())
    }

    pub fn set_status(&mut self, resource: &SyncResource, status: SyncStatus) {
        if let Some(entry) = self.entries.iter_mut().find(|e| &e.resource == resource) {
            entry.status = status;
        }
    }

    pub fn get_status(&self, resource: &SyncResource) -> Option<&SyncStatus> {
        self.entries
            .iter()
            .find(|e| &e.resource == resource)
            .map(|e| &e.status)
    }

    pub fn has_conflicts(&self) -> bool {
        self.entries.iter().any(|e| e.status == SyncStatus::Conflict)
    }

    pub fn is_syncing(&self) -> bool {
        self.entries.iter().any(|e| e.status == SyncStatus::Syncing)
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

    /// Begin syncing a resource. Fails if sync is disabled, the resource is
    /// not found, or the resource is already syncing.
    pub fn try_sync(&mut self, resource: &SyncResource) -> Result<(), SyncError> {
        if !self.enabled {
            return Err(SyncError::NotEnabled);
        }
        let entry = self
            .entries
            .iter_mut()
            .find(|e| &e.resource == resource)
            .ok_or_else(|| SyncError::ResourceNotFound(resource.to_string()))?;
        if entry.status == SyncStatus::Syncing {
            return Err(SyncError::SyncInProgress);
        }
        entry.status = SyncStatus::Syncing;
        Ok(())
    }

    /// Resolve a conflict by setting the resource status to UpToDate.
    pub fn resolve_conflict(&mut self, resource: &SyncResource) -> Result<(), SyncError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| &e.resource == resource)
            .ok_or_else(|| SyncError::ResourceNotFound(resource.to_string()))?;
        entry.status = SyncStatus::UpToDate;
        Ok(())
    }

    pub fn get_all_entries(&self) -> &[SyncEntry] {
        &self.entries
    }

    /// Update the local and/or remote version of a resource.
    pub fn update_version(
        &mut self,
        resource: &SyncResource,
        local: Option<u64>,
        remote: Option<u64>,
    ) -> Result<(), SyncError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| &e.resource == resource)
            .ok_or_else(|| SyncError::ResourceNotFound(resource.to_string()))?;
        if let Some(v) = local {
            entry.local_version = v;
        }
        if let Some(v) = remote {
            entry.remote_version = Some(v);
        }
        Ok(())
    }

    /// Returns true if any resource has a local version greater than its
    /// remote version.
    pub fn needs_sync(&self) -> bool {
        self.entries.iter().any(|e| match e.remote_version {
            Some(rv) => e.local_version > rv,
            None => true,
        })
    }

    pub fn resource_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if entries is empty.
    pub fn is_entries_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the first entrie, if any.
    pub fn first_entrie(&self) -> Option<&SyncEntry> {
        self.entries.first()
    }

    /// Get the last entrie, if any.
    pub fn last_entrie(&self) -> Option<&SyncEntry> {
        self.entries.last()
    }

    /// Retain only entries matching the predicate.
    pub fn retain_entries(&mut self, f: impl Fn(&SyncEntry) -> bool) {
        self.entries.retain(|item| f(item));
    }
}

impl Default for SyncService {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for userdatasync operations.
#[derive(Debug, Clone, PartialEq)]
pub struct UserdatasyncStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl UserdatasyncStats {
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
    pub fn merge(&mut self, other: &UserdatasyncStats) {
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

impl Default for UserdatasyncStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UserdatasyncStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UserdatasyncStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for userdatasync.
#[derive(Debug, Clone)]
pub struct UserdatasyncValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl UserdatasyncValidator {
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

impl Default for UserdatasyncValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SyncConflictResolver
// ---------------------------------------------------------------------------

/// Strategy for resolving sync conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy {
    UseLocal,
    UseRemote,
    Merge,
    Manual,
}

/// Represents a sync conflict between local and remote values.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncConflict {
    pub resource: SyncResource,
    pub local_value: String,
    pub remote_value: String,
    pub timestamp_local: u64,
    pub timestamp_remote: u64,
}

/// Resolves sync conflicts according to a chosen strategy.
pub struct SyncConflictResolver {
    default_strategy: ConflictStrategy,
    per_resource_strategy: HashMap<SyncResource, ConflictStrategy>,
    resolved_conflicts: Vec<(SyncConflict, ConflictStrategy)>,
}

impl SyncConflictResolver {
    pub fn new(default_strategy: ConflictStrategy) -> Self {
        Self {
            default_strategy,
            per_resource_strategy: HashMap::new(),
            resolved_conflicts: Vec::new(),
        }
    }

    pub fn set_strategy_for(&mut self, resource: SyncResource, strategy: ConflictStrategy) {
        self.per_resource_strategy.insert(resource, strategy);
    }

    pub fn get_strategy(&self, resource: &SyncResource) -> ConflictStrategy {
        self.per_resource_strategy
            .get(resource)
            .copied()
            .unwrap_or(self.default_strategy)
    }

    /// Resolve a conflict and return the chosen value.
    ///
    /// * `UseLocal` — returns the local value.
    /// * `UseRemote` — returns the remote value.
    /// * `Merge` — concatenates local and remote with a separator.
    /// * `Manual` — falls back to the local value.
    pub fn resolve(&mut self, conflict: SyncConflict) -> String {
        let strategy = self.get_strategy(&conflict.resource);
        let result = match strategy {
            ConflictStrategy::UseLocal | ConflictStrategy::Manual => {
                conflict.local_value.clone()
            }
            ConflictStrategy::UseRemote => conflict.remote_value.clone(),
            ConflictStrategy::Merge => {
                format!("{}<<<>>>{}",conflict.local_value, conflict.remote_value)
            }
        };
        self.resolved_conflicts.push((conflict, strategy));
        result
    }

    pub fn resolved_count(&self) -> usize {
        self.resolved_conflicts.len()
    }

    pub fn clear_resolved(&mut self) {
        self.resolved_conflicts.clear();
    }
}

// ---------------------------------------------------------------------------
// sync_diff
// ---------------------------------------------------------------------------

/// Result of diffing local and remote sync state.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncDiff {
    pub local_only: Vec<String>,
    pub remote_only: Vec<String>,
    pub modified: Vec<String>,
    pub unchanged: Vec<String>,
}

impl SyncDiff {
    /// Returns `true` when there are no differences at all.
    pub fn is_in_sync(&self) -> bool {
        self.local_only.is_empty()
            && self.remote_only.is_empty()
            && self.modified.is_empty()
    }

    /// Total number of changed keys (local-only + remote-only + modified).
    pub fn total_changes(&self) -> usize {
        self.local_only.len() + self.remote_only.len() + self.modified.len()
    }

    /// Returns `true` if there are modified keys (potential conflicts).
    pub fn has_conflicts(&self) -> bool {
        !self.modified.is_empty()
    }
}

/// Compare local and remote key-value maps and produce a diff.
pub fn sync_diff(
    local: &HashMap<String, String>,
    remote: &HashMap<String, String>,
) -> SyncDiff {
    let mut local_only = Vec::new();
    let mut modified = Vec::new();
    let mut unchanged = Vec::new();

    for (key, local_val) in local {
        match remote.get(key) {
            Some(remote_val) if remote_val == local_val => {
                unchanged.push(key.clone());
            }
            Some(_) => {
                modified.push(key.clone());
            }
            None => {
                local_only.push(key.clone());
            }
        }
    }

    let mut remote_only: Vec<String> = remote
        .keys()
        .filter(|k| !local.contains_key(*k))
        .cloned()
        .collect();

    // Sort for deterministic output.
    local_only.sort();
    remote_only.sort();
    modified.sort();
    unchanged.sort();

    SyncDiff {
        local_only,
        remote_only,
        modified,
        unchanged,
    }
}

// ---------------------------------------------------------------------------
// SyncLog
// ---------------------------------------------------------------------------

/// A single sync operation log entry.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncLogEntry {
    pub timestamp: u64,
    pub resource: SyncResource,
    pub operation: SyncOperation,
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOperation {
    Push,
    Pull,
    Merge,
    Resolve,
}

pub struct SyncLog {
    entries: Vec<SyncLogEntry>,
    max_entries: usize,
}

impl SyncLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Append a log entry, dropping the oldest if the log is at capacity.
    pub fn log(&mut self, entry: SyncLogEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[SyncLogEntry] {
        &self.entries
    }

    pub fn entries_for_resource(&self, resource: &SyncResource) -> Vec<&SyncLogEntry> {
        self.entries
            .iter()
            .filter(|e| &e.resource == resource)
            .collect()
    }

    pub fn last_entry(&self) -> Option<&SyncLogEntry> {
        self.entries.last()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Count of entries where `success` is `false`.
    pub fn error_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.success).count()
    }

    /// Fraction of successful entries in `[0.0, 1.0]`, or `1.0` when empty.
    pub fn success_rate(&self) -> f64 {
        if self.entries.is_empty() {
            return 1.0;
        }
        let ok = self.entries.iter().filter(|e| e.success).count();
        ok as f64 / self.entries.len() as f64
    }
}

// ---------------------------------------------------------------------------
// ResourceVersion
// ---------------------------------------------------------------------------

/// Versioning metadata for a single sync resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceVersion {
    pub resource: SyncResource,
    pub version: u64,
    pub hash: String,
    pub timestamp: u64,
}

impl ResourceVersion {
    pub fn new(resource: SyncResource, version: u64, hash: impl Into<String>, timestamp: u64) -> Self {
        Self {
            resource,
            version,
            hash: hash.into(),
            timestamp,
        }
    }

    /// Returns `true` when both version number and content hash match.
    pub fn matches(&self, other: &ResourceVersion) -> bool {
        self.version == other.version && self.hash == other.hash
    }

    /// Returns `true` if `self` is strictly newer than `other` by version number.
    pub fn is_newer_than(&self, other: &ResourceVersion) -> bool {
        self.version > other.version
    }

    /// Returns `true` if the content hash differs from `other` regardless of version.
    pub fn content_differs(&self, other: &ResourceVersion) -> bool {
        self.hash != other.hash
    }
}

impl fmt::Display for ResourceVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@v{} ({})", self.resource, self.version, &self.hash)
    }
}

impl From<(SyncResource, u64, &str, u64)> for ResourceVersion {
    fn from((resource, version, hash, timestamp): (SyncResource, u64, &str, u64)) -> Self {
        Self::new(resource, version, hash, timestamp)
    }
}

// ---------------------------------------------------------------------------
// SyncPlan
// ---------------------------------------------------------------------------

/// The direction of a planned resource transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    Upload,
    Download,
}

impl fmt::Display for SyncDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncDirection::Upload => write!(f, "Upload"),
            SyncDirection::Download => write!(f, "Download"),
        }
    }
}

/// A single item within a [`SyncPlan`].
#[derive(Debug, Clone, PartialEq)]
pub struct SyncPlanItem {
    pub resource: SyncResource,
    pub direction: SyncDirection,
    pub estimated_bytes: u64,
    pub has_conflict: bool,
}

/// A planned sync operation describing what will be transferred.
#[derive(Debug, Clone)]
pub struct SyncPlan {
    items: Vec<SyncPlanItem>,
    conflict_strategy: ConflictStrategy,
}

impl SyncPlan {
    pub fn new(conflict_strategy: ConflictStrategy) -> Self {
        Self {
            items: Vec::new(),
            conflict_strategy,
        }
    }

    pub fn add_item(&mut self, item: SyncPlanItem) {
        self.items.push(item);
    }

    pub fn items(&self) -> &[SyncPlanItem] {
        &self.items
    }

    /// Total estimated transfer size in bytes.
    pub fn estimated_total_bytes(&self) -> u64 {
        self.items.iter().map(|i| i.estimated_bytes).sum()
    }

    /// Number of items that have a conflict.
    pub fn conflict_count(&self) -> usize {
        self.items.iter().filter(|i| i.has_conflict).count()
    }

    /// Returns `true` if the plan contains any conflicts.
    pub fn has_conflicts(&self) -> bool {
        self.conflict_count() > 0
    }

    /// The strategy that will be used for conflicts in this plan.
    pub fn conflict_strategy(&self) -> ConflictStrategy {
        self.conflict_strategy
    }

    /// Returns `true` when the plan has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Number of items in the plan.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Build a plan by comparing local and remote [`ResourceVersion`] lists.
    pub fn from_versions(
        local: &[ResourceVersion],
        remote: &[ResourceVersion],
        conflict_strategy: ConflictStrategy,
    ) -> Self {
        let mut plan = SyncPlan::new(conflict_strategy);

        let remote_map: HashMap<&SyncResource, &ResourceVersion> =
            remote.iter().map(|r| (&r.resource, r)).collect();

        for lv in local {
            if let Some(rv) = remote_map.get(&lv.resource) {
                if lv.content_differs(rv) {
                    let (direction, has_conflict) = if lv.is_newer_than(rv) {
                        (SyncDirection::Upload, false)
                    } else if rv.is_newer_than(lv) {
                        (SyncDirection::Download, false)
                    } else {
                        // Same version number but different hashes — conflict.
                        (SyncDirection::Upload, true)
                    };
                    plan.add_item(SyncPlanItem {
                        resource: lv.resource.clone(),
                        direction,
                        estimated_bytes: 0,
                        has_conflict,
                    });
                }
            } else {
                // Exists locally but not remotely — upload.
                plan.add_item(SyncPlanItem {
                    resource: lv.resource.clone(),
                    direction: SyncDirection::Upload,
                    estimated_bytes: 0,
                    has_conflict: false,
                });
            }
        }

        let local_map: HashMap<&SyncResource, &ResourceVersion> =
            local.iter().map(|r| (&r.resource, r)).collect();
        for rv in remote {
            if !local_map.contains_key(&rv.resource) {
                plan.add_item(SyncPlanItem {
                    resource: rv.resource.clone(),
                    direction: SyncDirection::Download,
                    estimated_bytes: 0,
                    has_conflict: false,
                });
            }
        }

        plan
    }
}

impl fmt::Display for SyncPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SyncPlan({} items, {} conflicts, ~{} bytes)",
            self.len(),
            self.conflict_count(),
            self.estimated_total_bytes()
        )
    }
}

// ---------------------------------------------------------------------------
// SyncHistory
// ---------------------------------------------------------------------------

/// Outcome of a completed sync operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOutcome {
    Success,
    PartialSuccess,
    Failed,
}

impl fmt::Display for SyncOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncOutcome::Success => write!(f, "Success"),
            SyncOutcome::PartialSuccess => write!(f, "Partial Success"),
            SyncOutcome::Failed => write!(f, "Failed"),
        }
    }
}

/// Record of a single completed sync operation.
#[derive(Debug, Clone)]
pub struct SyncHistoryEntry {
    pub timestamp: u64,
    pub resources: Vec<SyncResource>,
    pub outcome: SyncOutcome,
    pub duration_ms: u64,
    pub bytes_transferred: u64,
    pub error_message: Option<String>,
}

impl fmt::Display for SyncHistoryEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[t={}] {} ({} resources, {}ms, {} bytes)",
            self.timestamp,
            self.outcome,
            self.resources.len(),
            self.duration_ms,
            self.bytes_transferred,
        )
    }
}

/// Stores a bounded history of past sync operations.
pub struct SyncHistory {
    entries: Vec<SyncHistoryEntry>,
    max_entries: usize,
}

impl SyncHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Record a completed sync operation.
    pub fn record(&mut self, entry: SyncHistoryEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[SyncHistoryEntry] {
        &self.entries
    }

    pub fn last_entry(&self) -> Option<&SyncHistoryEntry> {
        self.entries.last()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Average duration of all recorded syncs in milliseconds.
    pub fn average_duration_ms(&self) -> u64 {
        if self.entries.is_empty() {
            return 0;
        }
        let total: u64 = self.entries.iter().map(|e| e.duration_ms).sum();
        total / self.entries.len() as u64
    }

    /// Total bytes transferred across all recorded syncs.
    pub fn total_bytes_transferred(&self) -> u64 {
        self.entries.iter().map(|e| e.bytes_transferred).sum()
    }

    /// Number of entries with a given outcome.
    pub fn count_by_outcome(&self, outcome: SyncOutcome) -> usize {
        self.entries.iter().filter(|e| e.outcome == outcome).count()
    }

    /// Return entries that involved a particular resource.
    pub fn entries_for_resource(&self, resource: &SyncResource) -> Vec<&SyncHistoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.resources.contains(resource))
            .collect()
    }
}

// -- Content conflict resolution with merge policies -------------------------

/// Policy for resolving content-level sync conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergePolicy {
    AcceptLocal,
    AcceptRemote,
    Manual,
}

impl fmt::Display for MergePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergePolicy::AcceptLocal => f.write_str("Accept Local"),
            MergePolicy::AcceptRemote => f.write_str("Accept Remote"),
            MergePolicy::Manual => f.write_str("Manual"),
        }
    }
}

/// A detected conflict between local and remote content payloads.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncContentConflict {
    pub resource: SyncResource,
    pub local_content: String,
    pub remote_content: String,
    pub detected_at: u64,
}

impl SyncContentConflict {
    /// Resolve the conflict using the given policy.
    pub fn resolve(&self, policy: MergePolicy) -> String {
        match policy {
            MergePolicy::AcceptLocal => self.local_content.clone(),
            MergePolicy::AcceptRemote => self.remote_content.clone(),
            MergePolicy::Manual => String::new(),
        }
    }

    /// Check if local and remote are identical (false conflict).
    pub fn is_false_conflict(&self) -> bool {
        self.local_content == self.remote_content
    }
}

impl fmt::Display for SyncContentConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Conflict({}, local={}B, remote={}B)",
            self.resource, self.local_content.len(), self.remote_content.len())
    }
}

/// Resolver that applies merge policies to content conflicts.
#[derive(Debug)]
pub struct ContentConflictResolver {
    default_policy: MergePolicy,
    resource_policies: HashMap<SyncResource, MergePolicy>,
}

impl ContentConflictResolver {
    pub fn new(default: MergePolicy) -> Self {
        Self {
            default_policy: default,
            resource_policies: HashMap::new(),
        }
    }

    pub fn set_policy(&mut self, resource: SyncResource, policy: MergePolicy) {
        self.resource_policies.insert(resource, policy);
    }

    pub fn policy_for(&self, resource: &SyncResource) -> MergePolicy {
        self.resource_policies.get(resource).copied().unwrap_or(self.default_policy)
    }

    pub fn resolve(&self, conflict: &SyncContentConflict) -> String {
        let policy = self.policy_for(&conflict.resource);
        conflict.resolve(policy)
    }
}

// -- SyncActivityLog with timestamps -----------------------------------------

/// An entry in the sync activity log.
#[derive(Debug, Clone)]
pub struct SyncActivityEntry {
    pub timestamp: u64,
    pub resource: SyncResource,
    pub action: String,
    pub detail: Option<String>,
}

impl fmt::Display for SyncActivityEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} {}", self.timestamp, self.action, self.resource)?;
        if let Some(detail) = &self.detail {
            write!(f, ": {detail}")?;
        }
        Ok(())
    }
}

/// Log of sync activities.
#[derive(Debug, Default)]
pub struct SyncActivityLog {
    entries: Vec<SyncActivityEntry>,
    max_entries: usize,
}

impl SyncActivityLog {
    pub fn new(max_entries: usize) -> Self {
        Self { entries: Vec::new(), max_entries }
    }

    pub fn log(&mut self, timestamp: u64, resource: SyncResource, action: &str, detail: Option<&str>) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(SyncActivityEntry {
            timestamp,
            resource,
            action: action.to_string(),
            detail: detail.map(|s| s.to_string()),
        });
    }

    pub fn entries(&self) -> &[SyncActivityEntry] {
        &self.entries
    }

    pub fn entries_for_resource(&self, resource: &SyncResource) -> Vec<&SyncActivityEntry> {
        self.entries.iter().filter(|e| &e.resource == resource).collect()
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

    /// Get entries after a timestamp.
    pub fn since(&self, timestamp: u64) -> Vec<&SyncActivityEntry> {
        self.entries.iter().filter(|e| e.timestamp > timestamp).collect()
    }
}

impl fmt::Display for SyncActivityLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ActivityLog({} entries)", self.entries.len())
    }
}

// -- SyncQuotaManager for storage limits ------------------------------------

/// Manages storage quotas for sync.
#[derive(Debug)]
pub struct SyncQuotaManager {
    max_bytes: u64,
    used_bytes: HashMap<SyncResource, u64>,
}

impl SyncQuotaManager {
    pub fn new(max_bytes: u64) -> Self {
        Self { max_bytes, used_bytes: HashMap::new() }
    }

    pub fn record_usage(&mut self, resource: SyncResource, bytes: u64) {
        self.used_bytes.insert(resource, bytes);
    }

    pub fn total_used(&self) -> u64 {
        self.used_bytes.values().sum()
    }

    pub fn remaining(&self) -> u64 {
        let used = self.total_used();
        if used >= self.max_bytes { 0 } else { self.max_bytes - used }
    }

    pub fn is_over_quota(&self) -> bool {
        self.total_used() > self.max_bytes
    }

    pub fn usage_percentage(&self) -> f64 {
        if self.max_bytes == 0 { return 100.0; }
        (self.total_used() as f64 / self.max_bytes as f64) * 100.0
    }

    pub fn usage_for(&self, resource: &SyncResource) -> u64 {
        self.used_bytes.get(resource).copied().unwrap_or(0)
    }

    /// Check if adding bytes would exceed quota.
    pub fn can_add(&self, bytes: u64) -> bool {
        self.total_used() + bytes <= self.max_bytes
    }
}

impl fmt::Display for SyncQuotaManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Quota({}/{}B, {:.1}%)", self.total_used(), self.max_bytes, self.usage_percentage())
    }
}

// -- Sync profile switching --------------------------------------------------

/// A sync profile with enabled/disabled resources.
#[derive(Debug, Clone)]
pub struct SyncProfile {
    pub name: String,
    pub enabled_resources: Vec<SyncResource>,
    pub active: bool,
}

impl SyncProfile {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            enabled_resources: Vec::new(),
            active: false,
        }
    }

    pub fn enable_resource(&mut self, resource: SyncResource) {
        if !self.enabled_resources.contains(&resource) {
            self.enabled_resources.push(resource);
        }
    }

    pub fn disable_resource(&mut self, resource: &SyncResource) {
        self.enabled_resources.retain(|r| r != resource);
    }

    pub fn is_resource_enabled(&self, resource: &SyncResource) -> bool {
        self.enabled_resources.contains(resource)
    }

    pub fn resource_count(&self) -> usize {
        self.enabled_resources.len()
    }
}

impl fmt::Display for SyncProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.active { "active" } else { "inactive" };
        write!(f, "SyncProfile({}, {}, {} resources)", self.name, status, self.enabled_resources.len())
    }
}


// === Sync Conflict Merger ===

/// Sync Conflict Merger implementation.
#[derive(Debug, Clone)]
pub struct SyncConflictMerger {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: SyncConflictMergerStats,
}

/// Statistics for SyncConflictMerger.
#[derive(Debug, Clone, Default)]
pub struct SyncConflictMergerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl SyncConflictMergerStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl SyncConflictMerger {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: SyncConflictMergerStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &SyncConflictMergerStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for SyncConflictMerger {
    fn default() -> Self {
        Self::new()
    }
}

// === Sync Progress Tracker ===

/// Priority level for SyncProgressTracker items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SyncProgressTrackerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl SyncProgressTrackerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for SyncProgressTrackerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Sync Progress Tracker implementation.
#[derive(Debug, Clone)]
pub struct SyncProgressTracker {
    items: Vec<SyncProgressTrackerItem>,
    max_items: usize,
    default_priority: SyncProgressTrackerPriority,
}

/// A single item in SyncProgressTracker.
#[derive(Debug, Clone)]
pub struct SyncProgressTrackerItem {
    pub id: String,
    pub label: String,
    pub priority: SyncProgressTrackerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl SyncProgressTrackerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: SyncProgressTrackerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: SyncProgressTrackerPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl SyncProgressTracker {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: SyncProgressTrackerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: SyncProgressTrackerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<SyncProgressTrackerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&SyncProgressTrackerItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: SyncProgressTrackerPriority) -> Vec<&SyncProgressTrackerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&SyncProgressTrackerItem> {
        let mut sorted: Vec<&SyncProgressTrackerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&SyncProgressTrackerItem> {
        let mut sorted: Vec<&SyncProgressTrackerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&SyncProgressTrackerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: SyncProgressTrackerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> SyncProgressTrackerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &SyncProgressTrackerItem> {
        self.items.iter()
    }
}

impl Default for SyncProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_status() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::Settings);
        assert_eq!(
            svc.get_status(&SyncResource::Settings),
            Some(&SyncStatus::Idle)
        );
        svc.set_status(&SyncResource::Settings, SyncStatus::UpToDate);
        assert_eq!(
            svc.get_status(&SyncResource::Settings),
            Some(&SyncStatus::UpToDate)
        );
    }

    #[test]
    fn conflicts_and_syncing() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::Keybindings);
        svc.add_resource(SyncResource::Snippets);
        assert!(!svc.has_conflicts());
        svc.set_status(&SyncResource::Keybindings, SyncStatus::Conflict);
        assert!(svc.has_conflicts());
        svc.set_status(&SyncResource::Snippets, SyncStatus::Syncing);
        assert!(svc.is_syncing());
    }

    #[test]
    fn enable_disable() {
        let mut svc = SyncService::new();
        assert!(!svc.is_enabled());
        svc.enable();
        assert!(svc.is_enabled());
        svc.disable();
        assert!(!svc.is_enabled());
    }

    #[test]
    fn sync_error_display() {
        assert_eq!(
            SyncError::ResourceNotFound("Settings".into()).to_string(),
            "resource not found: Settings"
        );
        assert_eq!(
            SyncError::AlreadyExists("Settings".into()).to_string(),
            "resource already exists: Settings"
        );
        assert_eq!(
            SyncError::SyncInProgress.to_string(),
            "sync already in progress"
        );
        assert_eq!(SyncError::NotEnabled.to_string(), "sync is not enabled");
    }

    #[test]
    fn resource_and_status_display() {
        assert_eq!(SyncResource::Settings.to_string(), "Settings");
        assert_eq!(SyncResource::GlobalState.to_string(), "Global State");
        assert_eq!(SyncStatus::Idle.to_string(), "Idle");
        assert_eq!(SyncStatus::UpToDate.to_string(), "Up to Date");
        assert_eq!(
            SyncStatus::Error("oops".into()).to_string(),
            "Error: oops"
        );
    }

    #[test]
    fn remove_resource_success_and_not_found() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::Extensions);
        assert_eq!(svc.resource_count(), 1);
        assert!(svc.remove_resource(&SyncResource::Extensions).is_ok());
        assert_eq!(svc.resource_count(), 0);
        assert_eq!(
            svc.remove_resource(&SyncResource::Extensions),
            Err(SyncError::ResourceNotFound("Extensions".into()))
        );
    }

    #[test]
    fn try_sync_requires_enabled() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::Settings);
        assert_eq!(
            svc.try_sync(&SyncResource::Settings),
            Err(SyncError::NotEnabled)
        );
        svc.enable();
        assert!(svc.try_sync(&SyncResource::Settings).is_ok());
        assert_eq!(
            svc.get_status(&SyncResource::Settings),
            Some(&SyncStatus::Syncing)
        );
    }

    #[test]
    fn try_sync_prevents_double_sync() {
        let mut svc = SyncService::new();
        svc.enable();
        svc.add_resource(SyncResource::Snippets);
        svc.try_sync(&SyncResource::Snippets).unwrap();
        assert_eq!(
            svc.try_sync(&SyncResource::Snippets),
            Err(SyncError::SyncInProgress)
        );
    }

    #[test]
    fn try_sync_resource_not_found() {
        let mut svc = SyncService::new();
        svc.enable();
        assert_eq!(
            svc.try_sync(&SyncResource::GlobalState),
            Err(SyncError::ResourceNotFound("Global State".into()))
        );
    }

    #[test]
    fn resolve_conflict() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::Keybindings);
        svc.set_status(&SyncResource::Keybindings, SyncStatus::Conflict);
        assert!(svc.has_conflicts());
        svc.resolve_conflict(&SyncResource::Keybindings).unwrap();
        assert!(!svc.has_conflicts());
        assert_eq!(
            svc.get_status(&SyncResource::Keybindings),
            Some(&SyncStatus::UpToDate)
        );
    }

    #[test]
    fn get_all_entries_and_resource_count() {
        let mut svc = SyncService::new();
        assert_eq!(svc.get_all_entries().len(), 0);
        svc.add_resource(SyncResource::Settings);
        svc.add_resource(SyncResource::Extensions);
        svc.add_resource(SyncResource::Snippets);
        assert_eq!(svc.resource_count(), 3);
        assert_eq!(svc.get_all_entries().len(), 3);
    }

    #[test]
    fn update_version_and_needs_sync() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::Settings);
        // No remote version yet — needs sync
        assert!(svc.needs_sync());
        svc.update_version(&SyncResource::Settings, Some(1), Some(1))
            .unwrap();
        assert!(!svc.needs_sync());
        // Bump local ahead of remote
        svc.update_version(&SyncResource::Settings, Some(2), None)
            .unwrap();
        assert!(svc.needs_sync());
    }

    #[test]
    fn entry_is_in_sync() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::GlobalState);
        let entry = &svc.get_all_entries()[0];
        assert!(!entry.is_in_sync()); // remote is None
        svc.update_version(&SyncResource::GlobalState, Some(5), Some(5))
            .unwrap();
        assert!(svc.get_all_entries()[0].is_in_sync());
        svc.update_version(&SyncResource::GlobalState, Some(6), None)
            .unwrap();
        assert!(!svc.get_all_entries()[0].is_in_sync());
    }

    #[test]
    fn default_trait() {
        let svc = SyncService::default();
        assert!(!svc.is_enabled());
        assert_eq!(svc.resource_count(), 0);
    }

    #[test]
    fn eq_syncresource_same() {
        assert_eq!(SyncResource::Settings, SyncResource::Settings);
    }

    #[test]
    fn ne_syncresource_diff() {
        assert_ne!(SyncResource::Settings, SyncResource::Keybindings);
    }

    #[test]
    fn eq_syncstatus_same() {
        assert_eq!(SyncStatus::Idle, SyncStatus::Idle);
    }

    #[test]
    fn ne_syncstatus_diff() {
        assert_ne!(SyncStatus::Idle, SyncStatus::Syncing);
    }

    #[test]
    fn eq_syncerror_same() {
        assert_eq!(SyncError::SyncInProgress, SyncError::SyncInProgress);
    }

    #[test]
    fn ne_syncerror_diff() {
        assert_ne!(SyncError::SyncInProgress, SyncError::NotEnabled);
    }

    #[test]
    fn display_syncresource_variants() {
        assert!(!SyncResource::Settings.to_string().is_empty());
        assert!(!SyncResource::Keybindings.to_string().is_empty());
        assert!(!SyncResource::Snippets.to_string().is_empty());
        assert!(!SyncResource::Extensions.to_string().is_empty());
        assert!(!SyncResource::GlobalState.to_string().is_empty());
        assert!(!SyncResource::Profiles.to_string().is_empty());
    }

    #[test]
    fn display_syncstatus_variants() {
        assert!(!SyncStatus::Idle.to_string().is_empty());
        assert!(!SyncStatus::Syncing.to_string().is_empty());
        assert!(!SyncStatus::Conflict.to_string().is_empty());
        assert!(!SyncStatus::UpToDate.to_string().is_empty());
    }

    #[test]
    fn display_syncerror_variants() {
        assert!(!SyncError::SyncInProgress.to_string().is_empty());
        assert!(!SyncError::NotEnabled.to_string().is_empty());
    }

    // -----------------------------------------------------------------------
    // SyncConflictResolver tests
    // -----------------------------------------------------------------------

    #[test]
    fn conflict_resolver_default_strategy() {
        let resolver = SyncConflictResolver::new(ConflictStrategy::UseLocal);
        assert_eq!(
            resolver.get_strategy(&SyncResource::Settings),
            ConflictStrategy::UseLocal
        );
    }

    #[test]
    fn conflict_resolver_per_resource_strategy() {
        let mut resolver = SyncConflictResolver::new(ConflictStrategy::UseLocal);
        resolver.set_strategy_for(SyncResource::Keybindings, ConflictStrategy::UseRemote);
        assert_eq!(
            resolver.get_strategy(&SyncResource::Keybindings),
            ConflictStrategy::UseRemote
        );
        // Other resources still use the default.
        assert_eq!(
            resolver.get_strategy(&SyncResource::Settings),
            ConflictStrategy::UseLocal
        );
    }

    #[test]
    fn conflict_resolver_resolve_use_local() {
        let mut resolver = SyncConflictResolver::new(ConflictStrategy::UseLocal);
        let conflict = SyncConflict {
            resource: SyncResource::Settings,
            local_value: "local".into(),
            remote_value: "remote".into(),
            timestamp_local: 1,
            timestamp_remote: 2,
        };
        assert_eq!(resolver.resolve(conflict), "local");
    }

    #[test]
    fn conflict_resolver_resolve_use_remote() {
        let mut resolver = SyncConflictResolver::new(ConflictStrategy::UseRemote);
        let conflict = SyncConflict {
            resource: SyncResource::Extensions,
            local_value: "local".into(),
            remote_value: "remote".into(),
            timestamp_local: 1,
            timestamp_remote: 2,
        };
        assert_eq!(resolver.resolve(conflict), "remote");
    }

    #[test]
    fn conflict_resolver_resolve_merge() {
        let mut resolver = SyncConflictResolver::new(ConflictStrategy::Merge);
        let conflict = SyncConflict {
            resource: SyncResource::Snippets,
            local_value: "AAA".into(),
            remote_value: "BBB".into(),
            timestamp_local: 10,
            timestamp_remote: 20,
        };
        assert_eq!(resolver.resolve(conflict), "AAA<<<>>>BBB");
    }

    #[test]
    fn conflict_resolver_resolve_manual_returns_local() {
        let mut resolver = SyncConflictResolver::new(ConflictStrategy::Manual);
        let conflict = SyncConflict {
            resource: SyncResource::Profiles,
            local_value: "mine".into(),
            remote_value: "theirs".into(),
            timestamp_local: 5,
            timestamp_remote: 6,
        };
        assert_eq!(resolver.resolve(conflict), "mine");
    }

    #[test]
    fn conflict_resolver_resolved_count() {
        let mut resolver = SyncConflictResolver::new(ConflictStrategy::UseLocal);
        assert_eq!(resolver.resolved_count(), 0);
        let conflict = SyncConflict {
            resource: SyncResource::Settings,
            local_value: "a".into(),
            remote_value: "b".into(),
            timestamp_local: 0,
            timestamp_remote: 0,
        };
        resolver.resolve(conflict);
        assert_eq!(resolver.resolved_count(), 1);
    }

    #[test]
    fn conflict_resolver_clear_resolved() {
        let mut resolver = SyncConflictResolver::new(ConflictStrategy::UseLocal);
        let conflict = SyncConflict {
            resource: SyncResource::Settings,
            local_value: "a".into(),
            remote_value: "b".into(),
            timestamp_local: 0,
            timestamp_remote: 0,
        };
        resolver.resolve(conflict);
        assert_eq!(resolver.resolved_count(), 1);
        resolver.clear_resolved();
        assert_eq!(resolver.resolved_count(), 0);
    }

    #[test]
    fn conflict_resolver_per_resource_overrides_default() {
        let mut resolver = SyncConflictResolver::new(ConflictStrategy::UseLocal);
        resolver.set_strategy_for(SyncResource::GlobalState, ConflictStrategy::Merge);
        let conflict = SyncConflict {
            resource: SyncResource::GlobalState,
            local_value: "L".into(),
            remote_value: "R".into(),
            timestamp_local: 0,
            timestamp_remote: 0,
        };
        assert_eq!(resolver.resolve(conflict), "L<<<>>>R");
    }

    // -----------------------------------------------------------------------
    // sync_diff tests
    // -----------------------------------------------------------------------

    #[test]
    fn sync_diff_identical_maps() {
        let mut local = HashMap::new();
        local.insert("a".into(), "1".into());
        let diff = sync_diff(&local, &local);
        assert!(diff.is_in_sync());
        assert_eq!(diff.total_changes(), 0);
        assert!(!diff.has_conflicts());
    }

    #[test]
    fn sync_diff_local_only_keys() {
        let mut local = HashMap::new();
        local.insert("x".into(), "1".into());
        let remote = HashMap::new();
        let diff = sync_diff(&local, &remote);
        assert_eq!(diff.local_only, vec!["x".to_string()]);
        assert!(diff.remote_only.is_empty());
        assert!(!diff.is_in_sync());
    }

    #[test]
    fn sync_diff_remote_only_keys() {
        let local = HashMap::new();
        let mut remote = HashMap::new();
        remote.insert("y".into(), "2".into());
        let diff = sync_diff(&local, &remote);
        assert_eq!(diff.remote_only, vec!["y".to_string()]);
        assert!(diff.local_only.is_empty());
    }

    #[test]
    fn sync_diff_modified_keys() {
        let mut local = HashMap::new();
        local.insert("k".into(), "old".into());
        let mut remote = HashMap::new();
        remote.insert("k".into(), "new".into());
        let diff = sync_diff(&local, &remote);
        assert_eq!(diff.modified, vec!["k".to_string()]);
        assert!(diff.has_conflicts());
    }

    #[test]
    fn sync_diff_unchanged_keys() {
        let mut local = HashMap::new();
        local.insert("same".into(), "v".into());
        let mut remote = HashMap::new();
        remote.insert("same".into(), "v".into());
        let diff = sync_diff(&local, &remote);
        assert_eq!(diff.unchanged, vec!["same".to_string()]);
        assert!(diff.is_in_sync());
    }

    #[test]
    fn sync_diff_total_changes() {
        let mut local = HashMap::new();
        local.insert("a".into(), "1".into());
        local.insert("b".into(), "old".into());
        let mut remote = HashMap::new();
        remote.insert("b".into(), "new".into());
        remote.insert("c".into(), "3".into());
        let diff = sync_diff(&local, &remote);
        // a is local-only, b is modified, c is remote-only
        assert_eq!(diff.total_changes(), 3);
    }

    #[test]
    fn sync_diff_empty_maps() {
        let diff = sync_diff(&HashMap::new(), &HashMap::new());
        assert!(diff.is_in_sync());
        assert_eq!(diff.total_changes(), 0);
    }

    #[test]
    fn sync_diff_mixed_scenario() {
        let mut local = HashMap::new();
        local.insert("keep".into(), "same".into());
        local.insert("changed".into(), "v1".into());
        local.insert("only_local".into(), "x".into());

        let mut remote = HashMap::new();
        remote.insert("keep".into(), "same".into());
        remote.insert("changed".into(), "v2".into());
        remote.insert("only_remote".into(), "y".into());

        let diff = sync_diff(&local, &remote);
        assert_eq!(diff.unchanged, vec!["keep".to_string()]);
        assert_eq!(diff.modified, vec!["changed".to_string()]);
        assert_eq!(diff.local_only, vec!["only_local".to_string()]);
        assert_eq!(diff.remote_only, vec!["only_remote".to_string()]);
        assert!(!diff.is_in_sync());
    }

    // -----------------------------------------------------------------------
    // SyncLog tests
    // -----------------------------------------------------------------------

    fn make_entry(
        ts: u64,
        resource: SyncResource,
        op: SyncOperation,
        success: bool,
    ) -> SyncLogEntry {
        SyncLogEntry {
            timestamp: ts,
            resource,
            operation: op,
            success,
            message: None,
        }
    }

    #[test]
    fn sync_log_new_is_empty() {
        let log = SyncLog::new(10);
        assert!(log.entries().is_empty());
        assert_eq!(log.last_entry(), None);
    }

    #[test]
    fn sync_log_log_and_entries() {
        let mut log = SyncLog::new(10);
        log.log(make_entry(1, SyncResource::Settings, SyncOperation::Push, true));
        assert_eq!(log.entries().len(), 1);
    }

    #[test]
    fn sync_log_drops_oldest_at_capacity() {
        let mut log = SyncLog::new(2);
        log.log(make_entry(1, SyncResource::Settings, SyncOperation::Push, true));
        log.log(make_entry(2, SyncResource::Settings, SyncOperation::Pull, true));
        log.log(make_entry(3, SyncResource::Settings, SyncOperation::Merge, true));
        assert_eq!(log.entries().len(), 2);
        assert_eq!(log.entries()[0].timestamp, 2);
    }

    #[test]
    fn sync_log_entries_for_resource() {
        let mut log = SyncLog::new(10);
        log.log(make_entry(1, SyncResource::Settings, SyncOperation::Push, true));
        log.log(make_entry(2, SyncResource::Keybindings, SyncOperation::Pull, true));
        log.log(make_entry(3, SyncResource::Settings, SyncOperation::Merge, false));
        let settings = log.entries_for_resource(&SyncResource::Settings);
        assert_eq!(settings.len(), 2);
    }

    #[test]
    fn sync_log_last_entry() {
        let mut log = SyncLog::new(10);
        log.log(make_entry(1, SyncResource::Snippets, SyncOperation::Push, true));
        log.log(make_entry(2, SyncResource::Extensions, SyncOperation::Resolve, false));
        assert_eq!(log.last_entry().unwrap().timestamp, 2);
    }

    #[test]
    fn sync_log_clear() {
        let mut log = SyncLog::new(10);
        log.log(make_entry(1, SyncResource::Settings, SyncOperation::Push, true));
        log.clear();
        assert!(log.entries().is_empty());
    }

    #[test]
    fn sync_log_error_count() {
        let mut log = SyncLog::new(10);
        log.log(make_entry(1, SyncResource::Settings, SyncOperation::Push, true));
        log.log(make_entry(2, SyncResource::Settings, SyncOperation::Pull, false));
        log.log(make_entry(3, SyncResource::Settings, SyncOperation::Merge, false));
        assert_eq!(log.error_count(), 2);
    }

    #[test]
    fn sync_log_success_rate() {
        let mut log = SyncLog::new(10);
        log.log(make_entry(1, SyncResource::Settings, SyncOperation::Push, true));
        log.log(make_entry(2, SyncResource::Settings, SyncOperation::Pull, false));
        assert!((log.success_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn sync_log_success_rate_empty() {
        let log = SyncLog::new(10);
        assert!((log.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn userdatasync_stats_new_defaults() {
        let stats = UserdatasyncStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn userdatasync_stats_record_success() {
        let mut stats = UserdatasyncStats::new();
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
    fn userdatasync_stats_record_failure() {
        let mut stats = UserdatasyncStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn userdatasync_stats_reset() {
        let mut stats = UserdatasyncStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn userdatasync_stats_merge() {
        let mut a = UserdatasyncStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = UserdatasyncStats::new();
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
    fn userdatasync_stats_display() {
        let mut stats = UserdatasyncStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn userdatasync_stats_default() {
        let stats = UserdatasyncStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn userdatasync_validator_accepts_valid_name() {
        let v = UserdatasyncValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn userdatasync_validator_rejects_empty() {
        let v = UserdatasyncValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn userdatasync_validator_rejects_too_long() {
        let v = UserdatasyncValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn userdatasync_validator_forbidden_prefix() {
        let v = UserdatasyncValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn userdatasync_validator_allowed_chars() {
        let v = UserdatasyncValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn userdatasync_validator_range() {
        let v = UserdatasyncValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn userdatasync_sanitize_removes_control() {
        let result = UserdatasyncValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn userdatasync_truncate_short_string() {
        assert_eq!(UserdatasyncValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn userdatasync_truncate_long_string() {
        let result = UserdatasyncValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn userdatasync_is_ascii_printable() {
        assert!(UserdatasyncValidator::is_ascii_printable("Hello World 123"));
        assert!(!UserdatasyncValidator::is_ascii_printable("Hello\x00World"));
    }

    // -----------------------------------------------------------------------
    // ResourceVersion tests
    // -----------------------------------------------------------------------

    #[test]
    fn resource_version_matches_identical() {
        let a = ResourceVersion::new(SyncResource::Settings, 1, "abc123", 1000);
        let b = ResourceVersion::new(SyncResource::Settings, 1, "abc123", 2000);
        assert!(a.matches(&b));
    }

    #[test]
    fn resource_version_differs_by_hash() {
        let a = ResourceVersion::new(SyncResource::Settings, 1, "abc123", 1000);
        let b = ResourceVersion::new(SyncResource::Settings, 1, "def456", 1000);
        assert!(!a.matches(&b));
        assert!(a.content_differs(&b));
    }

    #[test]
    fn resource_version_is_newer_than() {
        let a = ResourceVersion::new(SyncResource::Keybindings, 3, "h1", 100);
        let b = ResourceVersion::new(SyncResource::Keybindings, 1, "h2", 200);
        assert!(a.is_newer_than(&b));
        assert!(!b.is_newer_than(&a));
    }

    #[test]
    fn resource_version_display() {
        let v = ResourceVersion::new(SyncResource::Snippets, 5, "deadbeef", 0);
        let s = format!("{v}");
        assert!(s.contains("Snippets"));
        assert!(s.contains("v5"));
        assert!(s.contains("deadbeef"));
    }

    #[test]
    fn resource_version_from_tuple() {
        let v: ResourceVersion = (SyncResource::Extensions, 2, "hash", 99).into();
        assert_eq!(v.resource, SyncResource::Extensions);
        assert_eq!(v.version, 2);
        assert_eq!(v.hash, "hash");
        assert_eq!(v.timestamp, 99);
    }

    // -----------------------------------------------------------------------
    // SyncPlan tests
    // -----------------------------------------------------------------------

    #[test]
    fn sync_plan_empty() {
        let plan = SyncPlan::new(ConflictStrategy::UseLocal);
        assert!(plan.is_empty());
        assert_eq!(plan.len(), 0);
        assert_eq!(plan.estimated_total_bytes(), 0);
        assert!(!plan.has_conflicts());
    }

    #[test]
    fn sync_plan_add_items_and_totals() {
        let mut plan = SyncPlan::new(ConflictStrategy::UseRemote);
        plan.add_item(SyncPlanItem {
            resource: SyncResource::Settings,
            direction: SyncDirection::Upload,
            estimated_bytes: 1024,
            has_conflict: false,
        });
        plan.add_item(SyncPlanItem {
            resource: SyncResource::Keybindings,
            direction: SyncDirection::Download,
            estimated_bytes: 512,
            has_conflict: true,
        });
        assert_eq!(plan.len(), 2);
        assert_eq!(plan.estimated_total_bytes(), 1536);
        assert_eq!(plan.conflict_count(), 1);
        assert!(plan.has_conflicts());
        assert_eq!(plan.conflict_strategy(), ConflictStrategy::UseRemote);
    }

    #[test]
    fn sync_plan_from_versions_upload_newer_local() {
        let local = vec![ResourceVersion::new(SyncResource::Settings, 3, "aaa", 100)];
        let remote = vec![ResourceVersion::new(SyncResource::Settings, 1, "bbb", 50)];
        let plan = SyncPlan::from_versions(&local, &remote, ConflictStrategy::UseLocal);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan.items()[0].direction, SyncDirection::Upload);
        assert!(!plan.items()[0].has_conflict);
    }

    #[test]
    fn sync_plan_from_versions_download_newer_remote() {
        let local = vec![ResourceVersion::new(SyncResource::Snippets, 1, "old", 10)];
        let remote = vec![ResourceVersion::new(SyncResource::Snippets, 5, "new", 90)];
        let plan = SyncPlan::from_versions(&local, &remote, ConflictStrategy::Merge);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan.items()[0].direction, SyncDirection::Download);
    }

    #[test]
    fn sync_plan_from_versions_conflict_same_version_different_hash() {
        let local = vec![ResourceVersion::new(SyncResource::Extensions, 2, "hashA", 50)];
        let remote = vec![ResourceVersion::new(SyncResource::Extensions, 2, "hashB", 60)];
        let plan = SyncPlan::from_versions(&local, &remote, ConflictStrategy::UseLocal);
        assert_eq!(plan.len(), 1);
        assert!(plan.items()[0].has_conflict);
    }

    #[test]
    fn sync_plan_from_versions_local_only_and_remote_only() {
        let local = vec![ResourceVersion::new(SyncResource::Settings, 1, "h1", 10)];
        let remote = vec![ResourceVersion::new(SyncResource::Keybindings, 1, "h2", 20)];
        let plan = SyncPlan::from_versions(&local, &remote, ConflictStrategy::UseLocal);
        assert_eq!(plan.len(), 2);
        let upload = plan.items().iter().find(|i| i.direction == SyncDirection::Upload).unwrap();
        assert_eq!(upload.resource, SyncResource::Settings);
        let download = plan.items().iter().find(|i| i.direction == SyncDirection::Download).unwrap();
        assert_eq!(download.resource, SyncResource::Keybindings);
    }

    #[test]
    fn sync_plan_from_versions_in_sync() {
        let local = vec![ResourceVersion::new(SyncResource::Profiles, 4, "same", 100)];
        let remote = vec![ResourceVersion::new(SyncResource::Profiles, 4, "same", 200)];
        let plan = SyncPlan::from_versions(&local, &remote, ConflictStrategy::Merge);
        assert!(plan.is_empty());
    }

    #[test]
    fn sync_plan_display() {
        let mut plan = SyncPlan::new(ConflictStrategy::UseLocal);
        plan.add_item(SyncPlanItem {
            resource: SyncResource::Settings,
            direction: SyncDirection::Upload,
            estimated_bytes: 256,
            has_conflict: false,
        });
        let s = format!("{plan}");
        assert!(s.contains("1 items"));
        assert!(s.contains("0 conflicts"));
        assert!(s.contains("256 bytes"));
    }

    // -----------------------------------------------------------------------
    // SyncHistory tests
    // -----------------------------------------------------------------------

    fn make_history_entry(
        ts: u64,
        resources: Vec<SyncResource>,
        outcome: SyncOutcome,
        duration_ms: u64,
        bytes: u64,
    ) -> SyncHistoryEntry {
        SyncHistoryEntry {
            timestamp: ts,
            resources,
            outcome,
            duration_ms,
            bytes_transferred: bytes,
            error_message: None,
        }
    }

    #[test]
    fn sync_history_empty() {
        let history = SyncHistory::new(10);
        assert!(history.entries().is_empty());
        assert!(history.last_entry().is_none());
        assert_eq!(history.average_duration_ms(), 0);
        assert_eq!(history.total_bytes_transferred(), 0);
    }

    #[test]
    fn sync_history_record_and_query() {
        let mut history = SyncHistory::new(10);
        history.record(make_history_entry(
            100,
            vec![SyncResource::Settings, SyncResource::Keybindings],
            SyncOutcome::Success,
            250,
            4096,
        ));
        history.record(make_history_entry(
            200,
            vec![SyncResource::Settings],
            SyncOutcome::Failed,
            100,
            0,
        ));
        assert_eq!(history.entries().len(), 2);
        assert_eq!(history.last_entry().unwrap().timestamp, 200);
        assert_eq!(history.average_duration_ms(), 175);
        assert_eq!(history.total_bytes_transferred(), 4096);
        assert_eq!(history.count_by_outcome(SyncOutcome::Success), 1);
        assert_eq!(history.count_by_outcome(SyncOutcome::Failed), 1);
        assert_eq!(history.count_by_outcome(SyncOutcome::PartialSuccess), 0);
    }

    #[test]
    fn sync_history_drops_oldest() {
        let mut history = SyncHistory::new(2);
        history.record(make_history_entry(1, vec![SyncResource::Settings], SyncOutcome::Success, 10, 0));
        history.record(make_history_entry(2, vec![SyncResource::Settings], SyncOutcome::Success, 20, 0));
        history.record(make_history_entry(3, vec![SyncResource::Settings], SyncOutcome::Success, 30, 0));
        assert_eq!(history.entries().len(), 2);
        assert_eq!(history.entries()[0].timestamp, 2);
    }

    #[test]
    fn sync_history_entries_for_resource() {
        let mut history = SyncHistory::new(10);
        history.record(make_history_entry(1, vec![SyncResource::Settings], SyncOutcome::Success, 10, 100));
        history.record(make_history_entry(2, vec![SyncResource::Keybindings], SyncOutcome::Success, 20, 200));
        history.record(make_history_entry(3, vec![SyncResource::Settings, SyncResource::Snippets], SyncOutcome::PartialSuccess, 30, 300));
        let settings = history.entries_for_resource(&SyncResource::Settings);
        assert_eq!(settings.len(), 2);
        let keybindings = history.entries_for_resource(&SyncResource::Keybindings);
        assert_eq!(keybindings.len(), 1);
    }

    #[test]
    fn sync_history_clear() {
        let mut history = SyncHistory::new(10);
        history.record(make_history_entry(1, vec![SyncResource::Settings], SyncOutcome::Success, 10, 0));
        history.clear();
        assert!(history.entries().is_empty());
    }

    #[test]
    fn sync_history_entry_display() {
        let entry = make_history_entry(
            42,
            vec![SyncResource::Settings, SyncResource::Extensions],
            SyncOutcome::PartialSuccess,
            350,
            8192,
        );
        let s = format!("{entry}");
        assert!(s.contains("Partial Success"));
        assert!(s.contains("2 resources"));
        assert!(s.contains("350ms"));
    }

    #[test]
    fn sync_direction_display() {
        assert_eq!(SyncDirection::Upload.to_string(), "Upload");
        assert_eq!(SyncDirection::Download.to_string(), "Download");
    }

    #[test]
    fn sync_outcome_display() {
        assert_eq!(SyncOutcome::Success.to_string(), "Success");
        assert_eq!(SyncOutcome::PartialSuccess.to_string(), "Partial Success");
        assert_eq!(SyncOutcome::Failed.to_string(), "Failed");
    }

    // -- ContentConflictResolver tests ----------------------------------------

    #[test]
    fn content_conflict_resolve_accept_local() {
        let conflict = SyncContentConflict {
            resource: SyncResource::Settings,
            local_content: "local".into(),
            remote_content: "remote".into(),
            detected_at: 100,
        };
        assert_eq!(conflict.resolve(MergePolicy::AcceptLocal), "local");
    }

    #[test]
    fn content_conflict_resolve_accept_remote() {
        let conflict = SyncContentConflict {
            resource: SyncResource::Settings,
            local_content: "local".into(),
            remote_content: "remote".into(),
            detected_at: 100,
        };
        assert_eq!(conflict.resolve(MergePolicy::AcceptRemote), "remote");
    }

    #[test]
    fn content_conflict_false_conflict() {
        let conflict = SyncContentConflict {
            resource: SyncResource::Settings,
            local_content: "same".into(),
            remote_content: "same".into(),
            detected_at: 100,
        };
        assert!(conflict.is_false_conflict());
    }

    #[test]
    fn content_resolver_policy_per_resource() {
        let mut resolver = ContentConflictResolver::new(MergePolicy::AcceptLocal);
        resolver.set_policy(SyncResource::Settings, MergePolicy::AcceptRemote);

        assert_eq!(resolver.policy_for(&SyncResource::Settings), MergePolicy::AcceptRemote);
        assert_eq!(resolver.policy_for(&SyncResource::Keybindings), MergePolicy::AcceptLocal);
    }

    #[test]
    fn content_conflict_display() {
        let conflict = SyncContentConflict {
            resource: SyncResource::Settings,
            local_content: "abc".into(),
            remote_content: "def".into(),
            detected_at: 100,
        };
        let s = conflict.to_string();
        assert!(s.contains("Settings"));
    }

    // -- SyncActivityLog tests ------------------------------------------------

    #[test]
    fn activity_log_record() {
        let mut log = SyncActivityLog::new(100);
        log.log(1000, SyncResource::Settings, "upload", Some("completed"));
        log.log(1001, SyncResource::Keybindings, "download", None);
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn activity_log_for_resource() {
        let mut log = SyncActivityLog::new(100);
        log.log(1, SyncResource::Settings, "upload", None);
        log.log(2, SyncResource::Keybindings, "upload", None);
        log.log(3, SyncResource::Settings, "download", None);
        let settings = log.entries_for_resource(&SyncResource::Settings);
        assert_eq!(settings.len(), 2);
    }

    #[test]
    fn activity_log_since() {
        let mut log = SyncActivityLog::new(100);
        log.log(100, SyncResource::Settings, "upload", None);
        log.log(200, SyncResource::Settings, "download", None);
        let recent = log.since(150);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn activity_log_evicts() {
        let mut log = SyncActivityLog::new(2);
        log.log(1, SyncResource::Settings, "a", None);
        log.log(2, SyncResource::Settings, "b", None);
        log.log(3, SyncResource::Settings, "c", None);
        assert_eq!(log.len(), 2);
        assert_eq!(log.entries()[0].action, "b");
    }

    // -- SyncQuotaManager tests -----------------------------------------------

    #[test]
    fn quota_tracking() {
        let mut quota = SyncQuotaManager::new(1000);
        quota.record_usage(SyncResource::Settings, 300);
        quota.record_usage(SyncResource::Keybindings, 200);
        assert_eq!(quota.total_used(), 500);
        assert_eq!(quota.remaining(), 500);
        assert!(!quota.is_over_quota());
    }

    #[test]
    fn quota_over_limit() {
        let mut quota = SyncQuotaManager::new(100);
        quota.record_usage(SyncResource::Settings, 200);
        assert!(quota.is_over_quota());
        assert_eq!(quota.remaining(), 0);
    }

    #[test]
    fn quota_can_add() {
        let mut quota = SyncQuotaManager::new(1000);
        quota.record_usage(SyncResource::Settings, 900);
        assert!(quota.can_add(100));
        assert!(!quota.can_add(101));
    }

    #[test]
    fn quota_display() {
        let quota = SyncQuotaManager::new(1000);
        let s = quota.to_string();
        assert!(s.contains("0/1000B"));
    }

    // -- SyncProfile tests ----------------------------------------------------

    #[test]
    fn profile_enable_disable_resource() {
        let mut profile = SyncProfile::new("work");
        profile.enable_resource(SyncResource::Settings);
        profile.enable_resource(SyncResource::Keybindings);
        assert_eq!(profile.resource_count(), 2);
        profile.disable_resource(&SyncResource::Settings);
        assert_eq!(profile.resource_count(), 1);
        assert!(!profile.is_resource_enabled(&SyncResource::Settings));
    }

    #[test]
    fn profile_no_duplicates() {
        let mut profile = SyncProfile::new("test");
        profile.enable_resource(SyncResource::Settings);
        profile.enable_resource(SyncResource::Settings);
        assert_eq!(profile.resource_count(), 1);
    }

    #[test]
    fn profile_display() {
        let profile = SyncProfile::new("default");
        let s = profile.to_string();
        assert!(s.contains("default"));
        assert!(s.contains("inactive"));
    }

    #[test]
    fn merge_policy_display() {
        assert_eq!(MergePolicy::AcceptLocal.to_string(), "Accept Local");
        assert_eq!(MergePolicy::Manual.to_string(), "Manual");
    }

    #[test]
    fn syncConflictMerger_new() {
        let s = SyncConflictMerger::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn syncConflictMerger_add_contains() {
        let mut s = SyncConflictMerger::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn syncConflictMerger_add_duplicate() {
        let mut s = SyncConflictMerger::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn syncConflictMerger_remove() {
        let mut s = SyncConflictMerger::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn syncConflictMerger_capacity() {
        let s = SyncConflictMerger::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn syncConflictMerger_search() {
        let mut s = SyncConflictMerger::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn syncConflictMerger_stats() {
        let mut s = SyncConflictMerger::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn syncProgressTracker_new() {
        let m = SyncProgressTracker::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn syncProgressTracker_add_find() {
        let mut m = SyncProgressTracker::new();
        m.add(SyncProgressTrackerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn syncProgressTracker_priority_filter() {
        let mut m = SyncProgressTracker::new();
        m.add(SyncProgressTrackerItem::new("a", "A").with_priority(SyncProgressTrackerPriority::High));
        m.add(SyncProgressTrackerItem::new("b", "B").with_priority(SyncProgressTrackerPriority::Low));
        m.add(SyncProgressTrackerItem::new("c", "C").with_priority(SyncProgressTrackerPriority::High));
        assert_eq!(m.by_priority(SyncProgressTrackerPriority::High).len(), 2);
    }

    #[test]
    fn syncProgressTracker_remove() {
        let mut m = SyncProgressTracker::new();
        m.add(SyncProgressTrackerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn syncProgressTracker_search() {
        let mut m = SyncProgressTracker::new();
        m.add(SyncProgressTrackerItem::new("id1", "Hello World"));
        m.add(SyncProgressTrackerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn syncProgressTracker_total_weight() {
        let mut m = SyncProgressTracker::new();
        m.add(SyncProgressTrackerItem::new("a", "A").with_priority(SyncProgressTrackerPriority::Critical));
        m.add(SyncProgressTrackerItem::new("b", "B").with_priority(SyncProgressTrackerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn syncProgressTracker_capacity_limit() {
        let mut m = SyncProgressTracker::new().with_max_items(2);
        m.add(SyncProgressTrackerItem::new("1", "one"));
        m.add(SyncProgressTrackerItem::new("2", "two"));
        assert!(!m.add(SyncProgressTrackerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn syncProgressTracker_sorted_by_priority() {
        let mut m = SyncProgressTracker::new();
        m.add(SyncProgressTrackerItem::new("lo", "Low").with_priority(SyncProgressTrackerPriority::Low));
        m.add(SyncProgressTrackerItem::new("hi", "High").with_priority(SyncProgressTrackerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn syncProgressTracker_item_metadata() {
        let mut item = SyncProgressTrackerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn syncConflictMerger_enabled_toggle() {
        let mut s = SyncConflictMerger::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn syncProgressTracker_priority_display() {
        assert_eq!(format!("{}", SyncProgressTrackerPriority::High), "high");
        assert_eq!(format!("{}", SyncProgressTrackerPriority::Low), "low");
    }

}
