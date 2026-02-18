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


/// Configuration manager for userdatasync functionality.
pub struct UserdatasyncConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl UserdatasyncConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &UserdatasyncConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for userdatasync operations.
pub struct UserdatasyncRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl UserdatasyncRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for userdatasync.
pub struct UserdatasyncValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl UserdatasyncValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &UserdatasyncValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}



// ---------------------------------------------------------------------------
// userdatasync – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for user data synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YUserdatasyncSyncConflictAction {
    AcceptLocal,
    AcceptRemote,
    Merge,
    Skip,
}

impl YUserdatasyncSyncConflictAction {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::AcceptLocal => 0,
            Self::AcceptRemote => 1,
            Self::Merge => 2,
            Self::Skip => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::AcceptLocal => "AcceptLocal",
            Self::AcceptRemote => "AcceptRemote",
            Self::Merge => "Merge",
            Self::Skip => "Skip",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YUserdatasyncSyncConflictAction] {
        &[
            YUserdatasyncSyncConflictAction::AcceptLocal,
            YUserdatasyncSyncConflictAction::AcceptRemote,
            YUserdatasyncSyncConflictAction::Merge,
            YUserdatasyncSyncConflictAction::Skip,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YUserdatasyncSyncConflictAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks sync state data.
#[derive(Debug, Clone)]
pub struct YUserdatasyncSyncState {
    pub last_sync_ms: u64,
    pub pending_changes: usize,
    pub version: u64,
}

impl YUserdatasyncSyncState {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            last_sync_ms: 0,
            pending_changes: 0,
            version: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YUserdatasyncSyncState({}: {:?})", "last_sync_ms", self.last_sync_ms)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_userdatasync_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_userdatasync_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_userdatasync_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_userdatasync_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_userdatasync_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_userdatasync_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_userdatasync_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_userdatasync_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// userdatasync – Extended sync merge log helpers
// ---------------------------------------------------------------------------

/// Priority levels for sync merge log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZUserdatasyncPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZUserdatasyncPriority {
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
    pub fn all_asc() -> [ZUserdatasyncPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZUserdatasyncPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks sync merge log data.
#[derive(Debug, Clone)]
pub struct ZUserdatasyncSyncMergeLog {
    pub merge_records: Vec<(String, String)>,
    pub conflicts_resolved: usize,
    pub last_merge_ms: u64,
}

impl ZUserdatasyncSyncMergeLog {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            merge_records: Vec::new(),
            conflicts_resolved: 0,
            last_merge_ms: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.merge_records.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.merge_records.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.merge_records.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZUserdatasyncSyncMergeLog[conflicts_resolved={:?}, last_merge_ms={:?}]", self.conflicts_resolved, self.last_merge_ms)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for sync merge log.
pub fn z_userdatasync_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_userdatasync_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_userdatasync_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_userdatasync_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_userdatasync_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_userdatasync_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_userdatasync_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 195
// ---------------------------------------------------------------------------

/// Generic object pool `Xc195Pool<T>`.
pub struct Xc195Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc195Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc195PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc195Pool<T> {
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
    pub fn stats(&self) -> Xc195PoolStats {
        Xc195PoolStats {
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

impl<T> Default for Xc195Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc195Scheduler`.
pub struct Xc195Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc195Scheduler {
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

impl Default for Xc195Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_195 hash for the given byte slice.
pub fn xc_195_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_195 convention.
pub fn xc_195_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_25 deepening: state machine + event bus ---

/// States for the Xd25 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd25State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd25State {
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
pub struct Xd25Transition {
    pub from: Xd25State,
    pub to: Xd25State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd25StateMachine {
    current: Xd25State,
    history: Vec<Xd25Transition>,
    step_counter: usize,
}

impl Xd25StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd25State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd25State {
        self.current
    }

    pub fn history(&self) -> &[Xd25Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd25State) -> Result<Xd25State, String> {
        let allowed = match (self.current, target) {
            (Xd25State::Idle, Xd25State::Running) => true,
            (Xd25State::Running, Xd25State::Paused) => true,
            (Xd25State::Running, Xd25State::Done) => true,
            (Xd25State::Paused, Xd25State::Running) => true,
            (Xd25State::Paused, Xd25State::Done) => true,
            (Xd25State::Done, Xd25State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_25: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd25Transition {
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
            "Xd25SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd25State> {
        let prefix = "Xd25SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd25State::Idle),
            "Running" => Some(Xd25State::Running),
            "Paused" => Some(Xd25State::Paused),
            "Done" => Some(Xd25State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd25State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd25 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd25Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd25Event {
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

type Xd25HandlerFn = Box<dyn Fn(&Xd25Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd25EventBus {
    handlers: Vec<(usize, Option<String>, Xd25HandlerFn)>,
    next_id: usize,
    published: Vec<Xd25Event>,
}

impl Xd25EventBus {
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
        F: Fn(&Xd25Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd25Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd25Event) {
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

    pub fn published_events(&self) -> &[Xd25Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #23
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf23Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf23TrieNode {
    children: std::collections::HashMap<char, Xf23TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf23Trie {
    root: Xf23TrieNode,
    count: usize,
}

impl Xf23Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf23TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf23TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf23TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf23BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf23BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
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
    fn userdatasync_validator_accepts_and_rejects() {
        let mut v = UserdatasyncValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn userdatasync_validator_warnings() {
        let mut v = UserdatasyncValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn userdatasync_validator_clear_and_merge() {
        let mut v = UserdatasyncValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = UserdatasyncValidationCollector::new();
        a.add_error("a_err");
        let mut b = UserdatasyncValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
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


    #[test]
    fn userdatasync_config_new() {
        let cfg = UserdatasyncConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn userdatasync_config_set_get() {
        let mut cfg = UserdatasyncConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn userdatasync_config_remove() {
        let mut cfg = UserdatasyncConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn userdatasync_config_keys_sorted() {
        let mut cfg = UserdatasyncConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn userdatasync_config_bump_version() {
        let mut cfg = UserdatasyncConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn userdatasync_config_clear() {
        let mut cfg = UserdatasyncConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn userdatasync_config_merge() {
        let mut cfg1 = UserdatasyncConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = UserdatasyncConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn userdatasync_config_disable() {
        let mut cfg = UserdatasyncConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn userdatasync_rate_tracker_empty() {
        let rt = UserdatasyncRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn userdatasync_rate_tracker_record() {
        let mut rt = UserdatasyncRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn userdatasync_rate_tracker_prune() {
        let mut rt = UserdatasyncRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn userdatasync_validator_valid() {
        let v = UserdatasyncValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn userdatasync_validator_errors() {
        let mut v = UserdatasyncValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn userdatasync_validator_clear() {
        let mut v = UserdatasyncValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn userdatasync_validator_merge() {
        let mut v1 = UserdatasyncValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = UserdatasyncValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn userdatasync_rate_tracker_clear() {
        let mut rt = UserdatasyncRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    // -- userdatasync extended domain tests ----------------------------------------

    #[test]
    fn y_userdatasync_enum_index() {
        assert_eq!(YUserdatasyncSyncConflictAction::AcceptLocal.index(), 0);
        assert_eq!(YUserdatasyncSyncConflictAction::AcceptRemote.index(), 1);
        assert_eq!(YUserdatasyncSyncConflictAction::Merge.index(), 2);
        assert_eq!(YUserdatasyncSyncConflictAction::Skip.index(), 3);
    }

    #[test]
    fn y_userdatasync_enum_label() {
        assert_eq!(YUserdatasyncSyncConflictAction::AcceptLocal.label(), "AcceptLocal");
        assert_eq!(YUserdatasyncSyncConflictAction::AcceptRemote.label(), "AcceptRemote");
        assert_eq!(YUserdatasyncSyncConflictAction::Merge.label(), "Merge");
        assert_eq!(YUserdatasyncSyncConflictAction::Skip.label(), "Skip");
    }

    #[test]
    fn y_userdatasync_enum_all() {
        let all = YUserdatasyncSyncConflictAction::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_userdatasync_enum_is_default() {
        assert!(YUserdatasyncSyncConflictAction::AcceptLocal.is_default());
        assert!(!YUserdatasyncSyncConflictAction::Skip.is_default());
    }

    #[test]
    fn y_userdatasync_enum_display() {
        assert_eq!(format!("{}", YUserdatasyncSyncConflictAction::AcceptLocal), "AcceptLocal");
    }

    #[test]
    fn y_userdatasync_struct_new() {
        let s = YUserdatasyncSyncState::new();
        let _ = s.summary();
    }

    #[test]
    fn y_userdatasync_fingerprint_deterministic() {
        let h1 = y_userdatasync_fingerprint("hello");
        let h2 = y_userdatasync_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_userdatasync_fingerprint("a"), y_userdatasync_fingerprint("b"));
    }

    #[test]
    fn y_userdatasync_truncate_short() {
        assert_eq!(y_userdatasync_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_userdatasync_truncate_long() {
        let r = y_userdatasync_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_userdatasync_normalize_key_basic() {
        assert_eq!(y_userdatasync_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_userdatasync_split_path_basic() {
        let parts = y_userdatasync_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_userdatasync_count_occurrences_basic() {
        assert_eq!(y_userdatasync_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_userdatasync_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_userdatasync_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_userdatasync_in_range_basic() {
        assert!(y_userdatasync_in_range(5, 1, 10));
        assert!(y_userdatasync_in_range(1, 1, 10));
        assert!(y_userdatasync_in_range(10, 1, 10));
        assert!(!y_userdatasync_in_range(0, 1, 10));
        assert!(!y_userdatasync_in_range(11, 1, 10));
    }

    #[test]
    fn y_userdatasync_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_userdatasync_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_userdatasync_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_userdatasync_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- userdatasync Z-extended tests -----------------------------------------------

    #[test]
    fn z_userdatasync_priority_weight() {
        assert_eq!(ZUserdatasyncPriority::Idle.weight(), 0);
        assert_eq!(ZUserdatasyncPriority::Normal.weight(), 2);
        assert_eq!(ZUserdatasyncPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_userdatasync_priority_label() {
        assert_eq!(ZUserdatasyncPriority::Low.label(), "low");
        assert_eq!(ZUserdatasyncPriority::High.label(), "high");
    }

    #[test]
    fn z_userdatasync_priority_is_elevated() {
        assert!(!ZUserdatasyncPriority::Normal.is_elevated());
        assert!(ZUserdatasyncPriority::High.is_elevated());
        assert!(ZUserdatasyncPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_userdatasync_priority_display() {
        assert_eq!(format!("{}", ZUserdatasyncPriority::Idle), "idle");
    }

    #[test]
    fn z_userdatasync_priority_all_asc() {
        let all = ZUserdatasyncPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZUserdatasyncPriority::Idle);
        assert_eq!(all[4], ZUserdatasyncPriority::Realtime);
    }

    #[test]
    fn z_userdatasync_struct_new() {
        let s = ZUserdatasyncSyncMergeLog::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_userdatasync_struct_toggled_clone() {
        let s = ZUserdatasyncSyncMergeLog::new();
        let t = s.toggled_clone();
        let _ = t.last_merge_ms;
    }

    #[test]
    fn z_userdatasync_rolling_hash_deterministic() {
        let h1 = z_userdatasync_rolling_hash(b"test");
        let h2 = z_userdatasync_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_userdatasync_rolling_hash(b"a"), z_userdatasync_rolling_hash(b"b"));
    }

    #[test]
    fn z_userdatasync_pad_to_basic() {
        assert_eq!(z_userdatasync_pad_to("hi", 5), "hi   ");
        assert_eq!(z_userdatasync_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_userdatasync_is_identifier_basic() {
        assert!(z_userdatasync_is_identifier("foo_bar"));
        assert!(z_userdatasync_is_identifier("abc123"));
        assert!(!z_userdatasync_is_identifier(""));
        assert!(!z_userdatasync_is_identifier("has space"));
    }

    #[test]
    fn z_userdatasync_levenshtein_basic() {
        assert_eq!(z_userdatasync_levenshtein("", ""), 0);
        assert_eq!(z_userdatasync_levenshtein("abc", "abc"), 0);
        assert_eq!(z_userdatasync_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_userdatasync_unique_words_basic() {
        let w = z_userdatasync_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_userdatasync_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_userdatasync_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_userdatasync_common_prefix_basic() {
        assert_eq!(z_userdatasync_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_userdatasync_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_userdatasync_struct_clear() {
        let mut s = ZUserdatasyncSyncMergeLog::new();
        s.merge_records.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_userdatasync_rolling_hash_empty() {
        let h = z_userdatasync_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 195 ----

    #[test]
    fn xc_195_pool_new_empty() {
        let pool: super::Xc195Pool<i32> = super::Xc195Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_195_pool_release_acquire() {
        let mut pool = super::Xc195Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_195_pool_acquire_empty() {
        let mut pool: super::Xc195Pool<i32> = super::Xc195Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_195_pool_full() {
        let mut pool = super::Xc195Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_195_pool_drain() {
        let mut pool = super::Xc195Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_195_pool_stats() {
        let mut pool = super::Xc195Pool::new(8);
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
    fn xc_195_pool_clear() {
        let mut pool = super::Xc195Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_195_pool_shrink() {
        let mut pool = super::Xc195Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_195_pool_default() {
        let pool: super::Xc195Pool<String> = super::Xc195Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_195_pool_extend() {
        let mut pool = super::Xc195Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_195_pool_retain() {
        let mut pool = super::Xc195Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_195_scheduler_round_robin() {
        let mut sched = super::Xc195Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_195_scheduler_empty() {
        let mut sched = super::Xc195Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_195_scheduler_reset() {
        let mut sched = super::Xc195Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_195_scheduler_add_remove() {
        let mut sched = super::Xc195Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_195_scheduler_targets() {
        let sched = super::Xc195Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_195_hash_empty() {
        assert_eq!(super::xc_195_hash(b""), 5381);
    }

    #[test]
    fn xc_195_hash_data() {
        let h = super::xc_195_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_195_hash(b"hello"), h);
    }

    #[test]
    fn xc_195_reverse_str() {
        assert_eq!(super::xc_195_reverse("abc"), "cba");
        assert_eq!(super::xc_195_reverse(""), "");
    }


    // --- xd_25 deepening tests ---

    #[test]
    fn xd_25_sm_initial_state() {
        let sm = Xd25StateMachine::new();
        assert_eq!(sm.current_state(), Xd25State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_25_sm_valid_idle_to_running() {
        let mut sm = Xd25StateMachine::new();
        assert!(sm.transition(Xd25State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd25State::Running);
    }

    #[test]
    fn xd_25_sm_valid_running_to_paused() {
        let mut sm = Xd25StateMachine::new();
        sm.transition(Xd25State::Running).unwrap();
        assert!(sm.transition(Xd25State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd25State::Paused);
    }

    #[test]
    fn xd_25_sm_valid_running_to_done() {
        let mut sm = Xd25StateMachine::new();
        sm.transition(Xd25State::Running).unwrap();
        assert!(sm.transition(Xd25State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd25State::Done);
    }

    #[test]
    fn xd_25_sm_valid_paused_to_running() {
        let mut sm = Xd25StateMachine::new();
        sm.transition(Xd25State::Running).unwrap();
        sm.transition(Xd25State::Paused).unwrap();
        assert!(sm.transition(Xd25State::Running).is_ok());
    }

    #[test]
    fn xd_25_sm_valid_done_to_idle() {
        let mut sm = Xd25StateMachine::new();
        sm.transition(Xd25State::Running).unwrap();
        sm.transition(Xd25State::Done).unwrap();
        assert!(sm.transition(Xd25State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd25State::Idle);
    }

    #[test]
    fn xd_25_sm_invalid_idle_to_done() {
        let mut sm = Xd25StateMachine::new();
        assert!(sm.transition(Xd25State::Done).is_err());
    }

    #[test]
    fn xd_25_sm_invalid_idle_to_paused() {
        let mut sm = Xd25StateMachine::new();
        assert!(sm.transition(Xd25State::Paused).is_err());
    }

    #[test]
    fn xd_25_sm_history_tracking() {
        let mut sm = Xd25StateMachine::new();
        sm.transition(Xd25State::Running).unwrap();
        sm.transition(Xd25State::Paused).unwrap();
        sm.transition(Xd25State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd25State::Idle);
        assert_eq!(sm.history()[0].to, Xd25State::Running);
        assert_eq!(sm.history()[1].from, Xd25State::Running);
        assert_eq!(sm.history()[2].to, Xd25State::Done);
    }

    #[test]
    fn xd_25_sm_serialize_deserialize() {
        let mut sm = Xd25StateMachine::new();
        sm.transition(Xd25State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd25StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd25State::Running));
    }

    #[test]
    fn xd_25_sm_deserialize_invalid() {
        assert_eq!(Xd25StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_25_sm_reset() {
        let mut sm = Xd25StateMachine::new();
        sm.transition(Xd25State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd25State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_25_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd25EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd25Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_25_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd25EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd25Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd25Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_25_bus_unsubscribe() {
        let mut bus = Xd25EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_25_event_kind_and_payload() {
        let e = Xd25Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd25Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_25_bus_clear_history() {
        let mut bus = Xd25EventBus::new();
        bus.publish(Xd25Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_25_sm_step_counter_increments() {
        let mut sm = Xd25StateMachine::new();
        sm.transition(Xd25State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd25State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #23 --

    #[test]
    fn xf23_trie_insert_search() {
        let mut t = Xf23Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf23_trie_starts_with() {
        let mut t = Xf23Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf23_trie_remove() {
        let mut t = Xf23Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf23_trie_word_count() {
        let mut t = Xf23Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf23_trie_longest_prefix() {
        let mut t = Xf23Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf23_trie_all_words() {
        let mut t = Xf23Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf23_trie_autocomplete() {
        let mut t = Xf23Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf23_trie_empty_search() {
        let t = Xf23Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf23_bloom_add_contains() {
        let mut bf = Xf23BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf23_bloom_probably_absent() {
        let bf = Xf23BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf23_bloom_false_positive_rate() {
        let mut bf = Xf23BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf23_bloom_clear() {
        let mut bf = Xf23BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf23_bloom_union() {
        let mut a = Xf23BloomFilter::xf_new(512, 2);
        let mut b = Xf23BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf23_bloom_intersection_estimate() {
        let mut a = Xf23BloomFilter::xf_new(512, 2);
        let mut b = Xf23BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf23_bloom_union_size_mismatch() {
        let a = Xf23BloomFilter::xf_new(256, 2);
        let b = Xf23BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }

}
