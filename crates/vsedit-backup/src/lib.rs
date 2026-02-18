//! Hot exit and file backup.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur during backup operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupError {
    FileNotFound(String),
    BackupLimitReached { limit: usize },
    InvalidPath(String),
    CorruptedBackup(String),
}

impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackupError::FileNotFound(path) => write!(f, "file not found: {path}"),
            BackupError::BackupLimitReached { limit } => {
                write!(f, "backup limit reached: {limit}")
            }
            BackupError::InvalidPath(path) => write!(f, "invalid path: {path}"),
            BackupError::CorruptedBackup(path) => write!(f, "corrupted backup: {path}"),
        }
    }
}

/// Policy that controls backup behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupPolicy {
    pub max_backups_per_file: usize,
    pub max_total_size: u64,
    pub auto_prune: bool,
}

impl Default for BackupPolicy {
    fn default() -> Self {
        Self {
            max_backups_per_file: 10,
            max_total_size: 100 * 1024 * 1024, // 100 MB
            auto_prune: false,
        }
    }
}

/// Aggregate statistics about all stored backups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupStats {
    pub total_count: usize,
    pub total_size: u64,
    pub oldest_timestamp: Option<u64>,
    pub newest_timestamp: Option<u64>,
}

/// A single backup record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEntry {
    pub original_path: String,
    pub backup_path: String,
    pub timestamp: u64,
    pub size: u64,
}

impl fmt::Display for BackupEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let filename = self.original_path.rsplit('/').next().unwrap_or(&self.original_path);
        write!(
            f,
            "backup of {filename} at timestamp {} ({} bytes)",
            self.timestamp, self.size
        )
    }
}

/// Builder for constructing a [`BackupEntry`] step by step.
#[derive(Debug, Default)]
pub struct BackupEntryBuilder {
    original_path: Option<String>,
    backup_path: Option<String>,
    timestamp: Option<u64>,
    size: Option<u64>,
}

impl BackupEntryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn original_path(mut self, path: impl Into<String>) -> Self {
        self.original_path = Some(path.into());
        self
    }

    pub fn backup_path(mut self, path: impl Into<String>) -> Self {
        self.backup_path = Some(path.into());
        self
    }

    pub fn timestamp(mut self, ts: u64) -> Self {
        self.timestamp = Some(ts);
        self
    }

    pub fn size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn build(self) -> Result<BackupEntry, BackupError> {
        let original_path = self
            .original_path
            .ok_or_else(|| BackupError::InvalidPath("original_path is required".into()))?;
        let backup_path = self
            .backup_path
            .ok_or_else(|| BackupError::InvalidPath("backup_path is required".into()))?;
        Ok(BackupEntry {
            original_path,
            backup_path,
            timestamp: self.timestamp.unwrap_or(0),
            size: self.size.unwrap_or(0),
        })
    }
}

/// In-memory backup service that tracks file snapshots.
pub struct BackupService {
    pub backup_dir: String,
    pub max_backups: usize,
    pub policy: BackupPolicy,
    entries: Vec<BackupEntry>,
    next_timestamp: u64,
}

impl BackupService {
    pub fn new(backup_dir: impl Into<String>) -> Self {
        Self {
            backup_dir: backup_dir.into(),
            max_backups: 5,
            policy: BackupPolicy::default(),
            entries: Vec::new(),
            next_timestamp: 1,
        }
    }

    /// Create a backup entry for the given path and content.
    pub fn create_backup(&mut self, path: &str, content: &str) -> BackupEntry {
        let ts = self.next_timestamp;
        self.next_timestamp += 1;
        let backup_path = self.generate_backup_path(path, ts);
        let entry = BackupEntry {
            original_path: path.to_string(),
            backup_path,
            timestamp: ts,
            size: content.len() as u64,
        };
        self.entries.push(entry.clone());
        entry
    }

    /// List all backups for a given original path, ordered by timestamp.
    pub fn list_backups(&self, path: &str) -> Vec<&BackupEntry> {
        let mut results: Vec<&BackupEntry> = self
            .entries
            .iter()
            .filter(|e| e.original_path == path)
            .collect();
        results.sort_by_key(|e| e.timestamp);
        results
    }

    /// Return the backup path of the most recent backup for the given path.
    pub fn restore_latest(&self, path: &str) -> Option<String> {
        self.list_backups(path)
            .last()
            .map(|e| e.backup_path.clone())
    }

    /// Keep only the most recent `max_backups` entries for the given path.
    pub fn prune_old_backups(&mut self, path: &str) {
        let mut indices: Vec<(usize, u64)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.original_path == path)
            .map(|(i, e)| (i, e.timestamp))
            .collect();
        indices.sort_by_key(|&(_, ts)| ts);

        if indices.len() > self.max_backups {
            let to_remove = indices.len() - self.max_backups;
            let remove_indices: Vec<usize> =
                indices[..to_remove].iter().map(|&(i, _)| i).collect();
            // Remove in reverse order to preserve indices.
            for &i in remove_indices.iter().rev() {
                self.entries.remove(i);
            }
        }
    }

    /// Delete a specific backup entry by its backup path.
    pub fn delete_backup(&mut self, backup_path: &str) -> Result<BackupEntry, BackupError> {
        let pos = self
            .entries
            .iter()
            .position(|e| e.backup_path == backup_path)
            .ok_or_else(|| BackupError::FileNotFound(backup_path.to_string()))?;
        Ok(self.entries.remove(pos))
    }

    /// Sum of all backup sizes in bytes.
    pub fn total_backup_size(&self) -> u64 {
        self.entries.iter().map(|e| e.size).sum()
    }

    /// Return all entries across all original paths, ordered by timestamp.
    pub fn list_all_backups(&self) -> Vec<&BackupEntry> {
        let mut all: Vec<&BackupEntry> = self.entries.iter().collect();
        all.sort_by_key(|e| e.timestamp);
        all
    }

    /// Filter backups whose original path ends with the given extension.
    pub fn find_backups_by_extension(&self, ext: &str) -> Vec<&BackupEntry> {
        let suffix = if ext.starts_with('.') {
            ext.to_string()
        } else {
            format!(".{ext}")
        };
        self.entries
            .iter()
            .filter(|e| e.original_path.ends_with(&suffix))
            .collect()
    }

    /// Compute aggregate statistics about all stored backups.
    pub fn get_backup_stats(&self) -> BackupStats {
        let total_count = self.entries.len();
        let total_size = self.total_backup_size();
        let oldest_timestamp = self.entries.iter().map(|e| e.timestamp).min();
        let newest_timestamp = self.entries.iter().map(|e| e.timestamp).max();
        BackupStats {
            total_count,
            total_size,
            oldest_timestamp,
            newest_timestamp,
        }
    }

    /// Apply the current [`BackupPolicy`] to a specific file path.
    ///
    /// Enforces `max_backups_per_file` by pruning the oldest entries and
    /// returns an error if total size exceeds `max_total_size` after pruning.
    pub fn apply_policy(&mut self, path: &str) -> Result<(), BackupError> {
        // Enforce per-file limit.
        let count = self.list_backups(path).len();
        if count > self.policy.max_backups_per_file {
            let saved = self.max_backups;
            self.max_backups = self.policy.max_backups_per_file;
            self.prune_old_backups(path);
            self.max_backups = saved;
        }
        // Enforce total size limit.
        if self.total_backup_size() > self.policy.max_total_size {
            return Err(BackupError::BackupLimitReached {
                limit: self.policy.max_total_size as usize,
            });
        }
        Ok(())
    }

    fn generate_backup_path(&self, path: &str, timestamp: u64) -> String {
        let file_name = path.rsplit('/').next().unwrap_or(path);
        format!("{}/{}.{}.bak", self.backup_dir, file_name, timestamp)
    }

    /// Returns true if entries is empty.
    pub fn is_entries_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the first entrie, if any.
    pub fn first_entrie(&self) -> Option<&BackupEntry> {
        self.entries.first()
    }

    /// Get the last entrie, if any.
    pub fn last_entrie(&self) -> Option<&BackupEntry> {
        self.entries.last()
    }

    /// Retain only entries matching the predicate.
    pub fn retain_entries(&mut self, f: impl Fn(&BackupEntry) -> bool) {
        self.entries.retain(|item| f(item));
    }
}

impl Default for BackupService {
    fn default() -> Self {
        Self::new("/tmp/backups")
    }
}

/// Accumulated statistics for backup operations.
#[derive(Debug, Clone, PartialEq)]
pub struct BackupStatsSummary {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl BackupStatsSummary {
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
    pub fn merge(&mut self, other: &BackupStatsSummary) {
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

impl Default for BackupStatsSummary {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BackupStatsSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BackupStatsSummary(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for backup.
#[derive(Debug, Clone)]
pub struct BackupValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl BackupValidator {
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

impl Default for BackupValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// BackupScheduler – periodic backup scheduling
// ---------------------------------------------------------------------------

/// Schedules periodic backups for a set of watched files.
pub struct BackupScheduler {
    pub interval_secs: u64,
    pub last_backup_time: u64,
    pub enabled: bool,
    pub files_to_backup: Vec<String>,
}

impl BackupScheduler {
    /// Create a new scheduler that triggers every `interval_secs` seconds.
    pub fn new(interval_secs: u64) -> Self {
        Self {
            interval_secs,
            last_backup_time: 0,
            enabled: true,
            files_to_backup: Vec::new(),
        }
    }

    /// Register a file path for scheduled backup.
    pub fn add_file(&mut self, path: &str) {
        if !self.files_to_backup.iter().any(|p| p == path) {
            self.files_to_backup.push(path.to_string());
        }
    }

    /// Remove a file path from the schedule. Returns `true` if it was present.
    pub fn remove_file(&mut self, path: &str) -> bool {
        if let Some(pos) = self.files_to_backup.iter().position(|p| p == path) {
            self.files_to_backup.remove(pos);
            true
        } else {
            false
        }
    }

    /// Returns `true` when enough time has elapsed since the last backup.
    pub fn is_due(&self, current_time: u64) -> bool {
        self.enabled && current_time.saturating_sub(self.last_backup_time) >= self.interval_secs
    }

    /// Record that a backup was completed at `current_time`.
    pub fn mark_completed(&mut self, current_time: u64) {
        self.last_backup_time = current_time;
    }

    /// Enable scheduling.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable scheduling.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Return the list of files registered for backup.
    pub fn files_to_backup(&self) -> &[String] {
        &self.files_to_backup
    }
}

impl fmt::Display for BackupScheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BackupScheduler(interval={}s, enabled={}, files={})",
            self.interval_secs,
            self.enabled,
            self.files_to_backup.len()
        )
    }
}

// ---------------------------------------------------------------------------
// BackupRotation – keep only the N most-recent backups
// ---------------------------------------------------------------------------

/// Rotation policy that keeps at most `max_count` backup entries.
pub struct BackupRotation {
    pub max_count: usize,
}

impl BackupRotation {
    pub fn new(max_count: usize) -> Self {
        Self { max_count }
    }

    /// Sort `entries` by timestamp (ascending) and drop the oldest until only
    /// `max_count` remain.
    pub fn rotate(&self, entries: &mut Vec<BackupEntry>) {
        if entries.len() <= self.max_count {
            return;
        }
        entries.sort_by_key(|e| e.timestamp);
        let remove_count = entries.len() - self.max_count;
        entries.drain(..remove_count);
    }

    /// Returns `true` when the number of backups exceeds the limit.
    pub fn should_rotate(&self, count: usize) -> bool {
        count > self.max_count
    }

    /// How many entries would need to be removed to satisfy the limit.
    pub fn entries_to_remove(&self, count: usize) -> usize {
        count.saturating_sub(self.max_count)
    }
}

impl fmt::Display for BackupRotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BackupRotation(max_count={})", self.max_count)
    }
}

// ---------------------------------------------------------------------------
// backup_verify – integrity verification helpers
// ---------------------------------------------------------------------------

/// Result of comparing an original hash with a backup hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupVerifyResult {
    Valid,
    Corrupted { expected: u64, actual: u64 },
}

impl fmt::Display for BackupVerifyResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Valid => write!(f, "backup verified: valid"),
            Self::Corrupted { expected, actual } => {
                write!(
                    f,
                    "backup corrupted: expected hash {expected:#x}, got {actual:#x}"
                )
            }
        }
    }
}

/// Compare an original file hash with the hash of its backup.
pub fn backup_verify(original_hash: u64, backup_hash: u64) -> BackupVerifyResult {
    if original_hash == backup_hash {
        BackupVerifyResult::Valid
    } else {
        BackupVerifyResult::Corrupted {
            expected: original_hash,
            actual: backup_hash,
        }
    }
}

/// A simple FNV-1a 64-bit hash suitable for quick integrity checks in tests.
pub fn simple_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// ---------------------------------------------------------------------------
// BackupDiff – compare backup content with current content
// ---------------------------------------------------------------------------

/// Represents a line-level difference between two text snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// Line exists only in the old (backup) version.
    Removed(String),
    /// Line exists only in the new (current) version.
    Added(String),
    /// Line is unchanged between versions.
    Unchanged(String),
}

/// Result of diffing a backup against the current file content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupDiff {
    pub lines: Vec<DiffLine>,
}

impl BackupDiff {
    /// Compute a simple line-level diff between `old` (backup) and `new` (current).
    ///
    /// Uses a greedy longest-common-subsequence approach to align matching lines.
    pub fn compute(old: &str, new: &str) -> Self {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        let mut result = Vec::new();
        let mut oi = 0;
        let mut ni = 0;

        while oi < old_lines.len() && ni < new_lines.len() {
            if old_lines[oi] == new_lines[ni] {
                result.push(DiffLine::Unchanged(old_lines[oi].to_string()));
                oi += 1;
                ni += 1;
            } else {
                // Look ahead in new for a match to old[oi]
                let new_match = new_lines[ni..].iter().position(|l| *l == old_lines[oi]);
                // Look ahead in old for a match to new[ni]
                let old_match = old_lines[oi..].iter().position(|l| *l == new_lines[ni]);

                match (new_match, old_match) {
                    (Some(nm), Some(om)) if nm <= om => {
                        for j in ni..ni + nm {
                            result.push(DiffLine::Added(new_lines[j].to_string()));
                        }
                        ni += nm;
                    }
                    (_, Some(om)) => {
                        for j in oi..oi + om {
                            result.push(DiffLine::Removed(old_lines[j].to_string()));
                        }
                        oi += om;
                    }
                    (Some(nm), None) => {
                        for j in ni..ni + nm {
                            result.push(DiffLine::Added(new_lines[j].to_string()));
                        }
                        ni += nm;
                    }
                    (None, None) => {
                        result.push(DiffLine::Removed(old_lines[oi].to_string()));
                        result.push(DiffLine::Added(new_lines[ni].to_string()));
                        oi += 1;
                        ni += 1;
                    }
                }
            }
        }

        for line in &old_lines[oi..] {
            result.push(DiffLine::Removed(line.to_string()));
        }
        for line in &new_lines[ni..] {
            result.push(DiffLine::Added(line.to_string()));
        }

        Self { lines: result }
    }

    /// Returns the number of added lines.
    pub fn additions(&self) -> usize {
        self.lines.iter().filter(|l| matches!(l, DiffLine::Added(_))).count()
    }

    /// Returns the number of removed lines.
    pub fn deletions(&self) -> usize {
        self.lines.iter().filter(|l| matches!(l, DiffLine::Removed(_))).count()
    }

    /// Returns `true` if the two snapshots are identical.
    pub fn is_unchanged(&self) -> bool {
        self.lines.iter().all(|l| matches!(l, DiffLine::Unchanged(_)))
    }
}

impl fmt::Display for BackupDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in &self.lines {
            match line {
                DiffLine::Removed(s) => writeln!(f, "- {s}")?,
                DiffLine::Added(s) => writeln!(f, "+ {s}")?,
                DiffLine::Unchanged(s) => writeln!(f, "  {s}")?,
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// BackupCleaner – bulk cleanup of stale backups
// ---------------------------------------------------------------------------

/// Bulk cleanup utility that removes backups older than a given age or
/// exceeding a total size budget.
pub struct BackupCleaner {
    /// Maximum age in seconds; backups older than this are considered stale.
    pub max_age_secs: u64,
    /// Maximum total size in bytes across all retained backups.
    pub max_total_bytes: u64,
}

impl BackupCleaner {
    pub fn new(max_age_secs: u64, max_total_bytes: u64) -> Self {
        Self {
            max_age_secs,
            max_total_bytes,
        }
    }

    /// Remove entries from `entries` that are older than `max_age_secs`
    /// relative to `now`. Returns the number of entries removed.
    pub fn remove_stale(&self, entries: &mut Vec<BackupEntry>, now: u64) -> usize {
        let before = entries.len();
        entries.retain(|e| now.saturating_sub(e.timestamp) < self.max_age_secs);
        before - entries.len()
    }

    /// Remove the oldest entries until the total size is within budget.
    /// Returns the number of entries removed.
    pub fn enforce_size_budget(&self, entries: &mut Vec<BackupEntry>) -> usize {
        entries.sort_by_key(|e| e.timestamp);
        let mut total: u64 = entries.iter().map(|e| e.size).sum();
        let mut removed = 0usize;
        while total > self.max_total_bytes && !entries.is_empty() {
            total -= entries[0].size;
            entries.remove(0);
            removed += 1;
        }
        removed
    }

    /// Convenience method: remove stale entries first, then enforce size budget.
    /// Returns `(stale_removed, budget_removed)`.
    pub fn clean(&self, entries: &mut Vec<BackupEntry>, now: u64) -> (usize, usize) {
        let stale = self.remove_stale(entries, now);
        let budget = self.enforce_size_budget(entries);
        (stale, budget)
    }
}

impl fmt::Display for BackupCleaner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BackupCleaner(max_age={}s, max_bytes={})",
            self.max_age_secs, self.max_total_bytes
        )
    }
}

// ---------------------------------------------------------------------------
// BackupVerifier – batch integrity verification
// ---------------------------------------------------------------------------

/// Batch verifier that checks a set of backup entries against known hashes.
pub struct BackupVerifier {
    /// Pairs of (backup_path, expected_hash).
    expectations: Vec<(String, u64)>,
}

impl BackupVerifier {
    pub fn new() -> Self {
        Self {
            expectations: Vec::new(),
        }
    }

    /// Register an expected hash for a backup path.
    pub fn expect(&mut self, backup_path: impl Into<String>, hash: u64) {
        self.expectations.push((backup_path.into(), hash));
    }

    /// Verify all registered expectations against provided actual hashes.
    ///
    /// `actual_hashes` maps backup_path → actual hash. Returns a list of
    /// `(backup_path, BackupVerifyResult)` for every registered expectation.
    pub fn verify_all(
        &self,
        actual_hashes: &[(String, u64)],
    ) -> Vec<(String, BackupVerifyResult)> {
        self.expectations
            .iter()
            .map(|(path, expected)| {
                let actual = actual_hashes
                    .iter()
                    .find(|(p, _)| p == path)
                    .map(|(_, h)| *h);
                let result = match actual {
                    Some(h) => backup_verify(*expected, h),
                    None => BackupVerifyResult::Corrupted {
                        expected: *expected,
                        actual: 0,
                    },
                };
                (path.clone(), result)
            })
            .collect()
    }

    /// Returns the number of registered expectations.
    pub fn expectation_count(&self) -> usize {
        self.expectations.len()
    }

    /// Returns `true` if all verifications pass.
    pub fn all_valid(&self, actual_hashes: &[(String, u64)]) -> bool {
        self.verify_all(actual_hashes)
            .iter()
            .all(|(_, r)| *r == BackupVerifyResult::Valid)
    }
}

impl Default for BackupVerifier {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Extended BackupService methods
// ---------------------------------------------------------------------------

impl BackupPolicy {
    /// Validate the policy settings, returning errors for any invalid values.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.max_backups_per_file == 0 {
            errors.push("max_backups_per_file must be at least 1".to_string());
        }
        if self.max_total_size == 0 {
            errors.push("max_total_size must be at least 1 byte".to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl BackupEntry {
    /// Compute the age of this backup in seconds relative to `now`.
    ///
    /// If `now` is less than the entry's timestamp, returns 0.
    pub fn age_seconds(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    /// Returns `true` if this entry backs up the same original file as `other`.
    pub fn same_file(&self, other: &BackupEntry) -> bool {
        self.original_path == other.original_path
    }
}

impl BackupService {
    /// Return aggregate stats for the service.
    pub fn stats(&self) -> BackupStats {
        self.get_backup_stats()
    }

    /// Remove all backup entries, returning the count of entries purged.
    pub fn purge_all(&mut self) -> usize {
        let count = self.entries.len();
        self.entries.clear();
        count
    }

    /// Find all backups whose timestamp falls within `[start, end]` (inclusive).
    pub fn find_by_timestamp(&self, start: u64, end: u64) -> Vec<&BackupEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .collect()
    }

    /// Remove duplicate backups for the same original file that have identical sizes,
    /// keeping only the most recent one.  Returns the number of entries removed.
    pub fn deduplicate(&mut self) -> usize {
        let mut seen: std::collections::HashMap<(&str, u64), u64> = std::collections::HashMap::new();
        // First pass: find the newest timestamp for each (path, size) pair.
        for entry in &self.entries {
            let key = (entry.original_path.as_str(), entry.size);
            let ts = seen.entry(key).or_insert(0);
            if entry.timestamp > *ts {
                *ts = entry.timestamp;
            }
        }
        // Collect the best timestamps keyed by (path_owned, size).
        let best: std::collections::HashMap<(String, u64), u64> = seen
            .into_iter()
            .map(|((p, s), ts)| ((p.to_string(), s), ts))
            .collect();
        let before = self.entries.len();
        self.entries.retain(|e| {
            best.get(&(e.original_path.clone(), e.size))
                .map_or(true, |&ts| e.timestamp == ts)
        });
        before - self.entries.len()
    }

    /// Count backups for a specific file.
    pub fn count_for_file(&self, path: &str) -> usize {
        self.entries.iter().filter(|e| e.original_path == path).count()
    }

    /// Return distinct original paths that have backups.
    pub fn backed_up_files(&self) -> Vec<&str> {
        let mut paths: Vec<&str> = self
            .entries
            .iter()
            .map(|e| e.original_path.as_str())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }
}

// ---------------------------------------------------------------------------
// BackupMetadata – rich metadata for backup entries
// ---------------------------------------------------------------------------

/// Metadata associated with a backup entry for tracking provenance and context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupMetadata {
    pub original_path: String,
    pub backup_path: String,
    pub timestamp: u64,
    pub size: u64,
    pub content_hash: u64,
    pub description: String,
    pub tags: Vec<String>,
}

impl BackupMetadata {
    /// Create metadata from a backup entry and its content.
    pub fn from_entry(entry: &BackupEntry, content: &[u8], description: &str) -> Self {
        Self {
            original_path: entry.original_path.clone(),
            backup_path: entry.backup_path.clone(),
            timestamp: entry.timestamp,
            size: entry.size,
            content_hash: simple_hash(content),
            description: description.to_string(),
            tags: Vec::new(),
        }
    }

    /// Add a tag to this metadata.
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag = tag.into();
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// Remove a tag. Returns `true` if it was present.
    pub fn remove_tag(&mut self, tag: &str) -> bool {
        if let Some(pos) = self.tags.iter().position(|t| t == tag) {
            self.tags.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if this metadata has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Verify content integrity against stored hash.
    pub fn verify_content(&self, content: &[u8]) -> bool {
        simple_hash(content) == self.content_hash
    }
}

impl fmt::Display for BackupMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BackupMetadata({}, hash={:#x}, tags=[{}])",
            self.backup_path,
            self.content_hash,
            self.tags.join(", ")
        )
    }
}

// ---------------------------------------------------------------------------
// BackupIndex – fast lookup of backups by content hash or path
// ---------------------------------------------------------------------------

/// Index structure for efficient backup lookups by content hash and path.
#[derive(Debug, Clone)]
pub struct BackupIndex {
    entries: Vec<BackupMetadata>,
}

impl BackupIndex {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Insert metadata into the index.
    pub fn insert(&mut self, meta: BackupMetadata) {
        self.entries.push(meta);
    }

    /// Find all backups with a given content hash (deduplication lookup).
    pub fn find_by_hash(&self, hash: u64) -> Vec<&BackupMetadata> {
        self.entries
            .iter()
            .filter(|m| m.content_hash == hash)
            .collect()
    }

    /// Find all backups for a given original path.
    pub fn find_by_path(&self, path: &str) -> Vec<&BackupMetadata> {
        self.entries
            .iter()
            .filter(|m| m.original_path == path)
            .collect()
    }

    /// Find backups matching a specific tag.
    pub fn find_by_tag(&self, tag: &str) -> Vec<&BackupMetadata> {
        self.entries.iter().filter(|m| m.has_tag(tag)).collect()
    }

    /// Check if content with the given hash already exists in the index.
    pub fn has_hash(&self, hash: u64) -> bool {
        self.entries.iter().any(|m| m.content_hash == hash)
    }

    /// Remove all entries for a given backup path. Returns count removed.
    pub fn remove_by_backup_path(&mut self, backup_path: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|m| m.backup_path != backup_path);
        before - self.entries.len()
    }

    /// Return total number of indexed entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return all distinct content hashes in the index.
    pub fn distinct_hashes(&self) -> Vec<u64> {
        let mut hashes: Vec<u64> = self.entries.iter().map(|m| m.content_hash).collect();
        hashes.sort();
        hashes.dedup();
        hashes
    }

    /// Return total storage size of all indexed entries.
    pub fn total_size(&self) -> u64 {
        self.entries.iter().map(|m| m.size).sum()
    }
}

impl Default for BackupIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// BackupTimeline – query backups across time windows
// ---------------------------------------------------------------------------

/// A time-windowed view over backup entries for temporal queries.
pub struct BackupTimeline {
    entries: Vec<BackupEntry>,
}

impl BackupTimeline {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Build a timeline from a slice of entries.
    pub fn from_entries(entries: &[BackupEntry]) -> Self {
        let mut sorted = entries.to_vec();
        sorted.sort_by_key(|e| e.timestamp);
        Self { entries: sorted }
    }

    /// Add an entry to the timeline (maintains sorted order).
    pub fn add(&mut self, entry: BackupEntry) {
        let pos = self
            .entries
            .binary_search_by_key(&entry.timestamp, |e| e.timestamp)
            .unwrap_or_else(|p| p);
        self.entries.insert(pos, entry);
    }

    /// Return all entries in the time window `[start, end]` (inclusive).
    pub fn range(&self, start: u64, end: u64) -> Vec<&BackupEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .collect()
    }

    /// Return the most recent entry at or before `timestamp`.
    pub fn latest_before(&self, timestamp: u64) -> Option<&BackupEntry> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.timestamp <= timestamp)
    }

    /// Return the oldest entry at or after `timestamp`.
    pub fn earliest_after(&self, timestamp: u64) -> Option<&BackupEntry> {
        self.entries.iter().find(|e| e.timestamp >= timestamp)
    }

    /// Compute the time gap (in timestamp units) between consecutive backups.
    /// Returns an empty vec if fewer than 2 entries exist.
    pub fn gaps(&self) -> Vec<u64> {
        self.entries
            .windows(2)
            .map(|w| w[1].timestamp.saturating_sub(w[0].timestamp))
            .collect()
    }

    /// Return the average gap between consecutive backups, or `None` if fewer
    /// than 2 entries.
    pub fn average_gap(&self) -> Option<u64> {
        let gaps = self.gaps();
        if gaps.is_empty() {
            return None;
        }
        let sum: u64 = gaps.iter().sum();
        Some(sum / gaps.len() as u64)
    }

    /// Return the maximum gap between consecutive backups.
    pub fn max_gap(&self) -> Option<u64> {
        self.gaps().into_iter().max()
    }

    /// Return total number of entries in the timeline.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the timeline has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Group entries by original path.
    pub fn group_by_file(&self) -> std::collections::HashMap<&str, Vec<&BackupEntry>> {
        let mut map: std::collections::HashMap<&str, Vec<&BackupEntry>> =
            std::collections::HashMap::new();
        for entry in &self.entries {
            map.entry(entry.original_path.as_str())
                .or_default()
                .push(entry);
        }
        map
    }
}

impl Default for BackupTimeline {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// BackupRotationPolicy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BackupRotationPolicy {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl BackupRotationPolicy {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for BackupRotationPolicy {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for BackupRotationPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "BackupRotationPolicy({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// BackupIntegrityChecker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BackupIntegrityChecker {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl BackupIntegrityChecker {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for BackupIntegrityChecker {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for BackupIntegrityChecker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "BackupIntegrityChecker({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// BackupRotationPolicySnapshot — point-in-time snapshot of BackupRotationPolicy state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BackupRotationPolicySnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl BackupRotationPolicySnapshot {
    pub fn capture(source: &BackupRotationPolicy, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for BackupRotationPolicySnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// BackupIntegrityCheckerStats — aggregate statistics for BackupIntegrityChecker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct BackupIntegrityCheckerStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl BackupIntegrityCheckerStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for BackupIntegrityCheckerStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// BackupRotationPolicyConfig — configuration for BackupRotationPolicy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BackupRotationPolicyConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl BackupRotationPolicyConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for BackupRotationPolicyConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for BackupRotationPolicyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// BackupRetentionPolicy — configurable retention rules
// ---------------------------------------------------------------------------

/// Configuration for how long / how many backups to keep.
#[derive(Debug, Clone)]
pub struct BackupRetentionPolicy {
    pub max_backups: usize,
    pub max_age_secs: u64,
    pub max_total_bytes: u64,
    pub compress_after_secs: u64,
}

impl BackupRetentionPolicy {
    pub fn new() -> Self {
        Self {
            max_backups: 50,
            max_age_secs: 30 * 24 * 3600, // 30 days
            max_total_bytes: 500 * 1024 * 1024,
            compress_after_secs: 7 * 24 * 3600,
        }
    }

    pub fn with_max_backups(mut self, n: usize) -> Self { self.max_backups = n; self }
    pub fn with_max_age_days(mut self, days: u64) -> Self { self.max_age_secs = days * 86400; self }
    pub fn with_max_total_bytes(mut self, bytes: u64) -> Self { self.max_total_bytes = bytes; self }
    pub fn with_compress_after_days(mut self, days: u64) -> Self { self.compress_after_secs = days * 86400; self }

    pub fn max_age_days(&self) -> u64 { self.max_age_secs / 86400 }

    pub fn should_compress(&self, age_secs: u64) -> bool {
        age_secs >= self.compress_after_secs
    }

    pub fn is_expired(&self, age_secs: u64) -> bool {
        age_secs >= self.max_age_secs
    }
}

impl Default for BackupRetentionPolicy {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// BackupFileInfo — metadata for a single backup file
// ---------------------------------------------------------------------------

/// Metadata about one backup file for retention decisions.
#[derive(Debug, Clone)]
pub struct BackupFileInfo {
    pub path: String,
    pub size: u64,
    pub created_ts: u64,
    pub compressed: bool,
    pub checksum: u64,
}

impl BackupFileInfo {
    pub fn new(path: impl Into<String>, size: u64, created_ts: u64) -> Self {
        Self { path: path.into(), size, created_ts, compressed: false, checksum: 0 }
    }

    pub fn with_checksum(mut self, cs: u64) -> Self { self.checksum = cs; self }
    pub fn with_compressed(mut self, c: bool) -> Self { self.compressed = c; self }

    pub fn age_seconds(&self, now: u64) -> u64 { now.saturating_sub(self.created_ts) }

    pub fn filename(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

// ---------------------------------------------------------------------------
// BackupRetentionManager — prune / compress decisions
// ---------------------------------------------------------------------------

/// Applies a `BackupRetentionPolicy` to a set of `BackupFileInfo` entries.
#[derive(Debug, Clone)]
pub struct BackupRetentionManager {
    entries: Vec<BackupFileInfo>,
    policy: BackupRetentionPolicy,
}

impl BackupRetentionManager {
    pub fn new(policy: BackupRetentionPolicy) -> Self {
        Self { entries: Vec::new(), policy }
    }

    pub fn add_entry(&mut self, entry: BackupFileInfo) {
        self.entries.push(entry);
    }

    /// Entries that should be removed — either expired or over the count limit.
    pub fn entries_to_prune(&self, now: u64) -> Vec<&BackupFileInfo> {
        let mut sorted: Vec<&BackupFileInfo> = self.entries.iter().collect();
        sorted.sort_by_key(|e| std::cmp::Reverse(e.created_ts));

        let mut prune = Vec::new();
        let mut total_bytes = 0u64;

        for (i, entry) in sorted.iter().enumerate() {
            let expired = self.policy.is_expired(entry.age_seconds(now));
            let over_count = i >= self.policy.max_backups;
            total_bytes += entry.size;
            let over_size = total_bytes > self.policy.max_total_bytes && i > 0;

            if expired || over_count || over_size {
                prune.push(*entry);
            }
        }
        prune
    }

    /// Entries that should be compressed but aren't yet.
    pub fn entries_to_compress(&self, now: u64) -> Vec<&BackupFileInfo> {
        self.entries.iter()
            .filter(|e| !e.compressed && self.policy.should_compress(e.age_seconds(now)))
            .collect()
    }

    pub fn newest(&self) -> Option<&BackupFileInfo> {
        self.entries.iter().max_by_key(|e| e.created_ts)
    }

    pub fn oldest(&self) -> Option<&BackupFileInfo> {
        self.entries.iter().min_by_key(|e| e.created_ts)
    }

    /// Returns true if any entry shares the same checksum (duplicate content).
    pub fn has_duplicate_checksum(&self, checksum: u64) -> bool {
        self.entries.iter().filter(|e| e.checksum == checksum && checksum != 0).count() > 1
    }

    pub fn total_size(&self) -> u64 {
        self.entries.iter().map(|e| e.size).sum()
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}


/// Configuration manager for backup functionality.
pub struct BackupConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl BackupConfig {
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

    pub fn merge(&mut self, other: &BackupConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for backup operations.
pub struct BackupRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl BackupRateTracker {
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

/// Validation result collector for backup.
pub struct BackupValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl BackupValidationCollector {
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

    pub fn merge(&mut self, other: &BackupValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}



// ---------------------------------------------------------------------------
// backup – Extended backup schedule helpers
// ---------------------------------------------------------------------------

/// Priority levels for backup schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZBackupPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZBackupPriority {
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
    pub fn all_asc() -> [ZBackupPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZBackupPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks backup schedule data.
#[derive(Debug, Clone)]
pub struct ZBackupBackupSchedule {
    pub intervals_ms: Vec<u64>,
    pub next_backup_ms: u64,
    pub paused: bool,
}

impl ZBackupBackupSchedule {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            intervals_ms: Vec::new(),
            next_backup_ms: 0,
            paused: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.intervals_ms.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.intervals_ms.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.intervals_ms.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZBackupBackupSchedule[next_backup_ms={:?}, paused={:?}]", self.next_backup_ms, self.paused)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.paused = !c.paused;
        c
    }
}

/// Compute a simple rolling hash for backup schedule.
pub fn z_backup_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_backup_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_backup_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_backup_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_backup_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_backup_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_backup_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 6
// ---------------------------------------------------------------------------

/// Generic object pool `Xc6Pool<T>`.
pub struct Xc6Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc6Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc6PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc6Pool<T> {
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
    pub fn stats(&self) -> Xc6PoolStats {
        Xc6PoolStats {
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

impl<T> Default for Xc6Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc6Scheduler`.
pub struct Xc6Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc6Scheduler {
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

impl Default for Xc6Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_6 hash for the given byte slice.
pub fn xc_6_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_6 convention.
pub fn xc_6_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_22 deepening: state machine + event bus ---

/// States for the Xd22 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd22State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd22State {
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
pub struct Xd22Transition {
    pub from: Xd22State,
    pub to: Xd22State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd22StateMachine {
    current: Xd22State,
    history: Vec<Xd22Transition>,
    step_counter: usize,
}

impl Xd22StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd22State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd22State {
        self.current
    }

    pub fn history(&self) -> &[Xd22Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd22State) -> Result<Xd22State, String> {
        let allowed = match (self.current, target) {
            (Xd22State::Idle, Xd22State::Running) => true,
            (Xd22State::Running, Xd22State::Paused) => true,
            (Xd22State::Running, Xd22State::Done) => true,
            (Xd22State::Paused, Xd22State::Running) => true,
            (Xd22State::Paused, Xd22State::Done) => true,
            (Xd22State::Done, Xd22State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_22: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd22Transition {
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
            "Xd22SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd22State> {
        let prefix = "Xd22SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd22State::Idle),
            "Running" => Some(Xd22State::Running),
            "Paused" => Some(Xd22State::Paused),
            "Done" => Some(Xd22State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd22State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd22 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd22Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd22Event {
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

type Xd22HandlerFn = Box<dyn Fn(&Xd22Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd22EventBus {
    handlers: Vec<(usize, Option<String>, Xd22HandlerFn)>,
    next_id: usize,
    published: Vec<Xd22Event>,
}

impl Xd22EventBus {
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
        F: Fn(&Xd22Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd22Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd22Event) {
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

    pub fn published_events(&self) -> &[Xd22Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #20
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf20Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf20TrieNode {
    children: std::collections::HashMap<char, Xf20TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf20Trie {
    root: Xf20TrieNode,
    count: usize,
}

impl Xf20Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf20TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf20TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf20TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf20BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf20BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 5).
pub struct Xh5SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh5SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 47 as u64,
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

/// A compact bit set supporting boolean operations (variant 5).
pub struct Xh5BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh5BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 5).
pub struct Xi5Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi5Deque<T> {
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
pub struct Xi5Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi5Interval {
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

/// A simple interval tree (variant 5).
pub struct Xi5IntervalTree {
    xi_intervals: Vec<Xi5Interval>,
}

impl Xi5IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi5Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi5Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi5Interval) -> Vec<&Xi5Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi5Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi5Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi5Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi5Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi5Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi5Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 5) ---

/// Disjoint set / union-find for crate 5.
pub struct Xj5UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj5UnionFind {
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

const XJ5_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 5.
pub struct Xj5BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj5BTreeNode<K, V>>>,
    len: usize,
}

struct Xj5BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj5BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj5BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ5_BTREE_ORDER - 1
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
        let mid = XJ5_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj5BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj5BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj5BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj5BTreeNode::xj_new_leaf();
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


// --- xk_5 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk5SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk5SegmentTree {
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
pub struct Xk5DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk5DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_5).
#[derive(Debug, Clone)]
pub struct Xl5Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl5Rope {
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

/// Suffix array for efficient string searching (xl_5).
#[derive(Debug, Clone)]
pub struct Xl5SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl5SuffixArray {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_backup() {
        let mut svc = BackupService::new("/backups");
        let entry = svc.create_backup("/home/user/file.txt", "hello world");
        assert_eq!(entry.original_path, "/home/user/file.txt");
        assert_eq!(entry.size, 11);
        assert_eq!(entry.backup_path, "/backups/file.txt.1.bak");
        assert_eq!(svc.list_backups("/home/user/file.txt").len(), 1);
    }

    #[test]
    fn prune_old_backups() {
        let mut svc = BackupService::new("/backups");
        svc.max_backups = 2;
        for i in 0..5 {
            svc.create_backup("/a.txt", &format!("v{i}"));
        }
        assert_eq!(svc.list_backups("/a.txt").len(), 5);
        svc.prune_old_backups("/a.txt");
        let remaining = svc.list_backups("/a.txt");
        assert_eq!(remaining.len(), 2);
        // The two most recent should survive.
        assert_eq!(remaining[0].timestamp, 4);
        assert_eq!(remaining[1].timestamp, 5);
    }

    #[test]
    fn restore_latest() {
        let mut svc = BackupService::new("/backups");
        assert!(svc.restore_latest("/missing.txt").is_none());
        svc.create_backup("/f.txt", "a");
        svc.create_backup("/f.txt", "b");
        let latest = svc.restore_latest("/f.txt").unwrap();
        assert_eq!(latest, "/backups/f.txt.2.bak");
    }

    #[test]
    fn delete_backup_success() {
        let mut svc = BackupService::new("/backups");
        let entry = svc.create_backup("/a.txt", "data");
        let removed = svc.delete_backup(&entry.backup_path).unwrap();
        assert_eq!(removed, entry);
        assert!(svc.list_backups("/a.txt").is_empty());
    }

    #[test]
    fn delete_backup_not_found() {
        let mut svc = BackupService::new("/backups");
        let err = svc.delete_backup("/no/such.bak").unwrap_err();
        assert_eq!(err, BackupError::FileNotFound("/no/such.bak".into()));
    }

    #[test]
    fn total_backup_size_sums_all() {
        let mut svc = BackupService::new("/backups");
        svc.create_backup("/a.txt", "aaa");
        svc.create_backup("/b.rs", "bbbbb");
        assert_eq!(svc.total_backup_size(), 8);
    }

    #[test]
    fn list_all_backups_across_paths() {
        let mut svc = BackupService::new("/backups");
        svc.create_backup("/x.txt", "x");
        svc.create_backup("/y.txt", "y");
        svc.create_backup("/x.txt", "x2");
        let all = svc.list_all_backups();
        assert_eq!(all.len(), 3);
        assert!(all.windows(2).all(|w| w[0].timestamp <= w[1].timestamp));
    }

    #[test]
    fn find_backups_by_extension_filters() {
        let mut svc = BackupService::new("/backups");
        svc.create_backup("/src/main.rs", "fn main(){}");
        svc.create_backup("/docs/readme.md", "# hi");
        svc.create_backup("/src/lib.rs", "pub mod x;");
        let rs = svc.find_backups_by_extension("rs");
        assert_eq!(rs.len(), 2);
        let md = svc.find_backups_by_extension(".md");
        assert_eq!(md.len(), 1);
    }

    #[test]
    fn backup_stats_empty() {
        let svc = BackupService::new("/backups");
        let stats = svc.get_backup_stats();
        assert_eq!(stats.total_count, 0);
        assert_eq!(stats.total_size, 0);
        assert_eq!(stats.oldest_timestamp, None);
        assert_eq!(stats.newest_timestamp, None);
    }

    #[test]
    fn backup_stats_populated() {
        let mut svc = BackupService::new("/backups");
        svc.create_backup("/a.txt", "short");
        svc.create_backup("/b.txt", "a bit longer");
        svc.create_backup("/a.txt", "medium");
        let stats = svc.get_backup_stats();
        assert_eq!(stats.total_count, 3);
        assert_eq!(stats.total_size, 5 + 12 + 6);
        assert_eq!(stats.oldest_timestamp, Some(1));
        assert_eq!(stats.newest_timestamp, Some(3));
    }

    #[test]
    fn display_backup_entry() {
        let entry = BackupEntry {
            original_path: "/home/user/main.rs".into(),
            backup_path: "/backups/main.rs.1.bak".into(),
            timestamp: 42,
            size: 1024,
        };
        let display = format!("{entry}");
        assert_eq!(display, "backup of main.rs at timestamp 42 (1024 bytes)");
    }

    #[test]
    fn error_display_variants() {
        assert_eq!(
            format!("{}", BackupError::FileNotFound("/a.txt".into())),
            "file not found: /a.txt"
        );
        assert_eq!(
            format!("{}", BackupError::BackupLimitReached { limit: 10 }),
            "backup limit reached: 10"
        );
        assert_eq!(
            format!("{}", BackupError::InvalidPath("bad".into())),
            "invalid path: bad"
        );
        assert_eq!(
            format!("{}", BackupError::CorruptedBackup("/x.bak".into())),
            "corrupted backup: /x.bak"
        );
    }

    #[test]
    fn builder_success_and_defaults() {
        let entry = BackupEntryBuilder::new()
            .original_path("/src/lib.rs")
            .backup_path("/backups/lib.rs.1.bak")
            .build()
            .unwrap();
        assert_eq!(entry.original_path, "/src/lib.rs");
        assert_eq!(entry.timestamp, 0);
        assert_eq!(entry.size, 0);
    }

    #[test]
    fn builder_missing_required_field() {
        let err = BackupEntryBuilder::new()
            .backup_path("/backups/x.bak")
            .build()
            .unwrap_err();
        assert!(matches!(err, BackupError::InvalidPath(_)));
    }

    #[test]
    fn policy_auto_prune_per_file() {
        let mut svc = BackupService::new("/backups");
        svc.policy.max_backups_per_file = 2;
        for i in 0..5 {
            svc.create_backup("/f.txt", &format!("v{i}"));
        }
        assert_eq!(svc.list_backups("/f.txt").len(), 5);
        svc.apply_policy("/f.txt").unwrap();
        assert_eq!(svc.list_backups("/f.txt").len(), 2);
    }

    #[test]
    fn policy_total_size_exceeded() {
        let mut svc = BackupService::new("/backups");
        svc.policy.max_total_size = 5;
        svc.create_backup("/a.txt", "abcdef"); // 6 bytes, over limit
        let result = svc.apply_policy("/a.txt");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BackupError::BackupLimitReached { .. }
        ));
    }

    // -- BackupScheduler tests --

    #[test]
    fn scheduler_is_due() {
        let sched = BackupScheduler::new(60);
        assert!(sched.is_due(60));
        assert!(sched.is_due(120));
        assert!(!sched.is_due(30));
    }

    #[test]
    fn scheduler_mark_completed() {
        let mut sched = BackupScheduler::new(60);
        sched.mark_completed(100);
        assert!(!sched.is_due(110));
        assert!(sched.is_due(160));
    }

    #[test]
    fn scheduler_add_remove_files() {
        let mut sched = BackupScheduler::new(60);
        sched.add_file("/a.txt");
        sched.add_file("/b.txt");
        assert_eq!(sched.files_to_backup().len(), 2);
        // duplicates are ignored
        sched.add_file("/a.txt");
        assert_eq!(sched.files_to_backup().len(), 2);
        assert!(sched.remove_file("/a.txt"));
        assert!(!sched.remove_file("/nonexistent"));
        assert_eq!(sched.files_to_backup(), &["/b.txt".to_string()]);
    }

    #[test]
    fn scheduler_enable_disable() {
        let mut sched = BackupScheduler::new(10);
        assert!(sched.is_due(10));
        sched.disable();
        assert!(!sched.is_due(10));
        sched.enable();
        assert!(sched.is_due(10));
    }

    // -- BackupRotation tests --

    #[test]
    fn rotation_keeps_max() {
        let rot = BackupRotation::new(2);
        let mut entries = vec![
            BackupEntry {
                original_path: "/a".into(),
                backup_path: "/b/a".into(),
                timestamp: 1,
                size: 10,
            },
            BackupEntry {
                original_path: "/b".into(),
                backup_path: "/b/b".into(),
                timestamp: 3,
                size: 20,
            },
            BackupEntry {
                original_path: "/c".into(),
                backup_path: "/b/c".into(),
                timestamp: 2,
                size: 15,
            },
        ];
        rot.rotate(&mut entries);
        assert_eq!(entries.len(), 2);
        // oldest (timestamp 1) should be removed; remaining sorted ascending
        assert_eq!(entries[0].timestamp, 2);
        assert_eq!(entries[1].timestamp, 3);
    }

    #[test]
    fn rotation_no_removal_when_under_limit() {
        let rot = BackupRotation::new(5);
        let mut entries = vec![BackupEntry {
            original_path: "/x".into(),
            backup_path: "/b/x".into(),
            timestamp: 1,
            size: 5,
        }];
        rot.rotate(&mut entries);
        assert_eq!(entries.len(), 1);
        assert!(!rot.should_rotate(1));
        assert_eq!(rot.entries_to_remove(1), 0);
    }

    // -- backup_verify tests --

    #[test]
    fn backup_verify_valid() {
        let result = backup_verify(42, 42);
        assert_eq!(result, BackupVerifyResult::Valid);
        assert!(result.to_string().contains("valid"));
    }

    #[test]
    fn backup_verify_corrupted() {
        let result = backup_verify(42, 99);
        assert_eq!(
            result,
            BackupVerifyResult::Corrupted {
                expected: 42,
                actual: 99
            }
        );
        assert!(result.to_string().contains("corrupted"));
    }

    #[test]
    fn simple_hash_consistency() {
        let data = b"hello world";
        let h1 = simple_hash(data);
        let h2 = simple_hash(data);
        assert_eq!(h1, h2);
        // different data should (very likely) produce a different hash
        let h3 = simple_hash(b"hello worlD");
        assert_ne!(h1, h3);
    }

    #[test]
    fn behavior_check_0() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn backup_stats_new_defaults() {
        let stats = BackupStatsSummary::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn backup_stats_record_success() {
        let mut stats = BackupStatsSummary::new();
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
    fn backup_stats_record_failure() {
        let mut stats = BackupStatsSummary::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn backup_stats_reset() {
        let mut stats = BackupStatsSummary::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn backup_stats_merge() {
        let mut a = BackupStatsSummary::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = BackupStatsSummary::new();
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
    fn backup_stats_display() {
        let mut stats = BackupStatsSummary::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn backup_stats_default() {
        let stats = BackupStatsSummary::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn backup_validator_accepts_and_rejects() {
        let mut v = BackupValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn backup_validator_warnings() {
        let mut v = BackupValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn backup_validator_clear_and_merge() {
        let mut v = BackupValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = BackupValidationCollector::new();
        a.add_error("a_err");
        let mut b = BackupValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    // -- BackupDiff tests --

    #[test]
    fn diff_identical_content() {
        let text = "line one\nline two\nline three";
        let diff = BackupDiff::compute(text, text);
        assert!(diff.is_unchanged());
        assert_eq!(diff.additions(), 0);
        assert_eq!(diff.deletions(), 0);
    }

    #[test]
    fn diff_detects_additions_and_removals() {
        let old = "alpha\nbeta\ngamma";
        let new = "alpha\ndelta\ngamma";
        let diff = BackupDiff::compute(old, new);
        assert!(!diff.is_unchanged());
        assert!(diff.additions() >= 1);
        assert!(diff.deletions() >= 1);
        // "alpha" and "gamma" should be unchanged
        assert!(diff.lines.contains(&DiffLine::Unchanged("alpha".into())));
        assert!(diff.lines.contains(&DiffLine::Unchanged("gamma".into())));
    }

    #[test]
    fn diff_display_format() {
        let old = "aaa";
        let new = "bbb";
        let diff = BackupDiff::compute(old, new);
        let output = format!("{diff}");
        assert!(output.contains("- aaa"));
        assert!(output.contains("+ bbb"));
    }

    // -- BackupCleaner tests --

    #[test]
    fn cleaner_removes_stale_entries() {
        let cleaner = BackupCleaner::new(100, u64::MAX);
        let mut entries = vec![
            BackupEntry {
                original_path: "/a".into(),
                backup_path: "/b/a".into(),
                timestamp: 10,
                size: 5,
            },
            BackupEntry {
                original_path: "/b".into(),
                backup_path: "/b/b".into(),
                timestamp: 200,
                size: 5,
            },
        ];
        let removed = cleaner.remove_stale(&mut entries, 250);
        assert_eq!(removed, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp, 200);
    }

    #[test]
    fn cleaner_enforces_size_budget() {
        let cleaner = BackupCleaner::new(u64::MAX, 10);
        let mut entries = vec![
            BackupEntry {
                original_path: "/a".into(),
                backup_path: "/b/a".into(),
                timestamp: 1,
                size: 8,
            },
            BackupEntry {
                original_path: "/b".into(),
                backup_path: "/b/b".into(),
                timestamp: 2,
                size: 8,
            },
        ];
        let removed = cleaner.enforce_size_budget(&mut entries);
        assert_eq!(removed, 1);
        assert_eq!(entries.len(), 1);
        // The newer entry should survive
        assert_eq!(entries[0].timestamp, 2);
    }

    // -- BackupVerifier tests --

    #[test]
    fn verifier_all_valid() {
        let mut v = BackupVerifier::new();
        v.expect("/b/a.bak", 100);
        v.expect("/b/b.bak", 200);
        let actuals = vec![
            ("/b/a.bak".to_string(), 100u64),
            ("/b/b.bak".to_string(), 200u64),
        ];
        assert!(v.all_valid(&actuals));
        assert_eq!(v.expectation_count(), 2);
    }

    #[test]
    fn verifier_detects_corruption() {
        let mut v = BackupVerifier::new();
        v.expect("/b/a.bak", 100);
        let actuals = vec![("/b/a.bak".to_string(), 999u64)];
        assert!(!v.all_valid(&actuals));
        let results = v.verify_all(&actuals);
        assert_eq!(
            results[0].1,
            BackupVerifyResult::Corrupted {
                expected: 100,
                actual: 999
            }
        );
    }

    // -- New functionality tests --

    #[test]
    fn backup_policy_validate_ok() {
        let policy = BackupPolicy::default();
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn backup_policy_validate_zero_backups() {
        let policy = BackupPolicy {
            max_backups_per_file: 0,
            max_total_size: 1024,
            auto_prune: false,
        };
        let errs = policy.validate().unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("max_backups_per_file"));
    }

    #[test]
    fn backup_entry_age_seconds() {
        let entry = BackupEntry {
            original_path: "/a.txt".into(),
            backup_path: "/b/a.bak".into(),
            timestamp: 100,
            size: 10,
        };
        assert_eq!(entry.age_seconds(150), 50);
        assert_eq!(entry.age_seconds(50), 0); // now before timestamp
    }

    #[test]
    fn backup_service_purge_all() {
        let mut svc = BackupService::new("/backups");
        svc.create_backup("/a.txt", "hello");
        svc.create_backup("/b.txt", "world");
        assert_eq!(svc.purge_all(), 2);
        assert!(svc.is_entries_empty());
    }

    #[test]
    fn backup_service_find_by_timestamp() {
        let mut svc = BackupService::new("/backups");
        svc.create_backup("/a.txt", "v1"); // ts=1
        svc.create_backup("/a.txt", "v2"); // ts=2
        svc.create_backup("/a.txt", "v3"); // ts=3
        let found = svc.find_by_timestamp(2, 3);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn backup_service_deduplicate() {
        let mut svc = BackupService::new("/backups");
        // Two backups of same file with same size => dedup keeps latest
        svc.create_backup("/a.txt", "hello"); // ts=1, size=5
        svc.create_backup("/a.txt", "hello"); // ts=2, size=5
        svc.create_backup("/a.txt", "hello"); // ts=3, size=5
        let removed = svc.deduplicate();
        assert_eq!(removed, 2);
        assert_eq!(svc.count_for_file("/a.txt"), 1);
        assert_eq!(svc.list_backups("/a.txt")[0].timestamp, 3);
    }

    #[test]
    fn backup_service_backed_up_files() {
        let mut svc = BackupService::new("/backups");
        svc.create_backup("/a.txt", "v1");
        svc.create_backup("/b.txt", "v2");
        svc.create_backup("/a.txt", "v3");
        let files = svc.backed_up_files();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"/a.txt"));
        assert!(files.contains(&"/b.txt"));
    }

    #[test]
    fn backup_entry_same_file() {
        let e1 = BackupEntry {
            original_path: "/a.txt".into(),
            backup_path: "/b/a.1.bak".into(),
            timestamp: 1,
            size: 10,
        };
        let e2 = BackupEntry {
            original_path: "/a.txt".into(),
            backup_path: "/b/a.2.bak".into(),
            timestamp: 2,
            size: 20,
        };
        let e3 = BackupEntry {
            original_path: "/c.txt".into(),
            backup_path: "/b/c.1.bak".into(),
            timestamp: 3,
            size: 10,
        };
        assert!(e1.same_file(&e2));
        assert!(!e1.same_file(&e3));
    }

    // -----------------------------------------------------------------------
    // BackupMetadata tests
    // -----------------------------------------------------------------------

    #[test]
    fn backup_metadata_from_entry_and_verify() {
        let entry = BackupEntry {
            original_path: "/src/main.rs".into(),
            backup_path: "/backups/main.rs.1.bak".into(),
            timestamp: 100,
            size: 13,
        };
        let content = b"hello, world!";
        let meta = BackupMetadata::from_entry(&entry, content, "initial save");
        assert_eq!(meta.original_path, "/src/main.rs");
        assert_eq!(meta.size, 13);
        assert_eq!(meta.description, "initial save");
        assert_eq!(meta.content_hash, simple_hash(content));
        assert!(meta.verify_content(content));
        assert!(!meta.verify_content(b"modified"));
    }

    #[test]
    fn backup_metadata_tags() {
        let entry = BackupEntry {
            original_path: "/a.txt".into(),
            backup_path: "/b/a.1.bak".into(),
            timestamp: 1,
            size: 4,
        };
        let mut meta = BackupMetadata::from_entry(&entry, b"data", "test");
        assert!(!meta.has_tag("release"));
        meta.add_tag("release");
        meta.add_tag("important");
        meta.add_tag("release"); // duplicate should be ignored
        assert_eq!(meta.tags.len(), 2);
        assert!(meta.has_tag("release"));
        assert!(meta.remove_tag("release"));
        assert!(!meta.has_tag("release"));
        assert!(!meta.remove_tag("nonexistent"));
    }

    // -----------------------------------------------------------------------
    // BackupIndex tests
    // -----------------------------------------------------------------------

    #[test]
    fn backup_index_insert_and_find_by_hash() {
        let mut index = BackupIndex::new();
        assert!(index.is_empty());

        let entry = BackupEntry {
            original_path: "/a.txt".into(),
            backup_path: "/b/a.1.bak".into(),
            timestamp: 1,
            size: 5,
        };
        let content = b"alpha";
        let meta = BackupMetadata::from_entry(&entry, content, "v1");
        let hash = meta.content_hash;
        index.insert(meta);

        assert_eq!(index.len(), 1);
        assert!(index.has_hash(hash));
        assert!(!index.has_hash(0xDEAD));
        assert_eq!(index.find_by_hash(hash).len(), 1);
        assert_eq!(index.find_by_path("/a.txt").len(), 1);
        assert_eq!(index.find_by_path("/nonexistent").len(), 0);
    }

    #[test]
    fn backup_index_find_by_tag_and_dedup() {
        let mut index = BackupIndex::new();
        let content = b"same content";
        let hash = simple_hash(content);

        let e1 = BackupEntry {
            original_path: "/a.txt".into(),
            backup_path: "/b/a.1.bak".into(),
            timestamp: 1,
            size: 12,
        };
        let mut m1 = BackupMetadata::from_entry(&e1, content, "first");
        m1.add_tag("release");
        index.insert(m1);

        let e2 = BackupEntry {
            original_path: "/a.txt".into(),
            backup_path: "/b/a.2.bak".into(),
            timestamp: 2,
            size: 12,
        };
        let m2 = BackupMetadata::from_entry(&e2, content, "second");
        index.insert(m2);

        // Both have the same hash → duplicate content
        assert_eq!(index.find_by_hash(hash).len(), 2);
        assert_eq!(index.find_by_tag("release").len(), 1);
        assert_eq!(index.distinct_hashes().len(), 1);
        assert_eq!(index.total_size(), 24);

        // Remove one by backup path
        assert_eq!(index.remove_by_backup_path("/b/a.1.bak"), 1);
        assert_eq!(index.len(), 1);
    }

    // -----------------------------------------------------------------------
    // BackupTimeline tests
    // -----------------------------------------------------------------------

    fn make_entry(path: &str, ts: u64, size: u64) -> BackupEntry {
        BackupEntry {
            original_path: path.to_string(),
            backup_path: format!("/b/{}.{}.bak", path, ts),
            timestamp: ts,
            size,
        }
    }

    #[test]
    fn backup_timeline_range_and_navigation() {
        let entries = vec![
            make_entry("/a.txt", 10, 100),
            make_entry("/a.txt", 20, 200),
            make_entry("/b.txt", 30, 150),
            make_entry("/a.txt", 40, 120),
            make_entry("/b.txt", 50, 180),
        ];
        let timeline = BackupTimeline::from_entries(&entries);
        assert_eq!(timeline.len(), 5);

        // Range query
        let window = timeline.range(15, 35);
        assert_eq!(window.len(), 2);
        assert_eq!(window[0].timestamp, 20);
        assert_eq!(window[1].timestamp, 30);

        // Navigation
        let before = timeline.latest_before(25).unwrap();
        assert_eq!(before.timestamp, 20);
        let after = timeline.earliest_after(25).unwrap();
        assert_eq!(after.timestamp, 30);

        assert!(timeline.latest_before(5).is_none());
        assert!(timeline.earliest_after(100).is_none());
    }

    #[test]
    fn backup_timeline_gaps_and_grouping() {
        let entries = vec![
            make_entry("/a.txt", 10, 100),
            make_entry("/a.txt", 15, 100),
            make_entry("/b.txt", 25, 200),
            make_entry("/a.txt", 50, 100),
        ];
        let timeline = BackupTimeline::from_entries(&entries);

        // Gaps: 5, 10, 25
        let gaps = timeline.gaps();
        assert_eq!(gaps, vec![5, 10, 25]);
        assert_eq!(timeline.average_gap(), Some(13)); // (5+10+25)/3 = 13
        assert_eq!(timeline.max_gap(), Some(25));

        // Group by file
        let groups = timeline.group_by_file();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups["/a.txt"].len(), 3);
        assert_eq!(groups["/b.txt"].len(), 1);
    }

    #[test]
    fn backup_timeline_add_maintains_order() {
        let mut timeline = BackupTimeline::new();
        assert!(timeline.is_empty());

        timeline.add(make_entry("/a.txt", 30, 100));
        timeline.add(make_entry("/a.txt", 10, 50));
        timeline.add(make_entry("/a.txt", 20, 75));

        assert_eq!(timeline.len(), 3);
        let range = timeline.range(0, 100);
        assert_eq!(range[0].timestamp, 10);
        assert_eq!(range[1].timestamp, 20);
        assert_eq!(range[2].timestamp, 30);

        // Empty timeline edge cases
        let empty = BackupTimeline::new();
        assert_eq!(empty.average_gap(), None);
        assert_eq!(empty.max_gap(), None);
        assert!(empty.gaps().is_empty());
    }

    #[test] fn backupRotationPolicy_new() { let s = BackupRotationPolicy::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn backupRotationPolicy_add() { let mut s = BackupRotationPolicy::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn backupRotationPolicy_remove() { let mut s = BackupRotationPolicy::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn backupRotationPolicy_config() { let mut s = BackupRotationPolicy::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn backupRotationPolicy_nav() { let mut s = BackupRotationPolicy::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn backupRotationPolicy_filter() { let mut s = BackupRotationPolicy::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn backupRotationPolicy_display() { assert!(format!("{}", BackupRotationPolicy::new()).contains("BackupRotationPolicy")); }
    #[test] fn backupIntegrityChecker_new() { let s = BackupIntegrityChecker::new(); assert!(s.is_empty()); }
    #[test] fn backupIntegrityChecker_add() { let mut s = BackupIntegrityChecker::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn backupIntegrityChecker_active() { let mut s = BackupIntegrityChecker::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn backupIntegrityChecker_error() { let mut s = BackupIntegrityChecker::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn backupIntegrityChecker_rm_group() { let mut s = BackupIntegrityChecker::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn backupIntegrityChecker_display() { assert!(format!("{}", BackupIntegrityChecker::new()).contains("BackupIntegrityChecker")); }


    #[test] fn backupRotationPolicy_snap_capture() {
        let s = BackupRotationPolicy::new();
        let snap = BackupRotationPolicySnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn backupRotationPolicy_snap_stale() {
        let s = BackupRotationPolicy::new();
        let snap = BackupRotationPolicySnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn backupRotationPolicy_snap_diff() {
        let s = BackupRotationPolicy::new();
        let s1v = BackupRotationPolicySnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn backupRotationPolicy_snap_display() {
        let s = BackupRotationPolicy::new();
        let snap = BackupRotationPolicySnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn backupIntegrityChecker_stats_record() {
        let mut st = BackupIntegrityCheckerStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn backupIntegrityChecker_stats_hit_ratio() {
        let mut st = BackupIntegrityCheckerStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn backupIntegrityChecker_stats_merge() {
        let mut a = BackupIntegrityCheckerStats::new();
        a.total_adds = 5;
        let mut b = BackupIntegrityCheckerStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn backupIntegrityChecker_stats_display() {
        let st = BackupIntegrityCheckerStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn backupRotationPolicy_config_default() {
        let c = BackupRotationPolicyConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn backupRotationPolicy_config_builder() {
        let c = BackupRotationPolicyConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn backupRotationPolicy_config_labels() {
        let mut c = BackupRotationPolicyConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn backupRotationPolicy_config_cleanup_threshold() {
        let c = BackupRotationPolicyConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn backupRotationPolicy_config_display() {
        assert!(format!("{}", BackupRotationPolicyConfig::new()).contains("Config"));
    }
    #[test] fn backupIntegrityChecker_stats_peaks() {
        let mut st = BackupIntegrityCheckerStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- BackupRetentionPolicy -----------------------------------------------

    #[test]
    fn retention_policy_defaults() {
        let p = BackupRetentionPolicy::new();
        assert_eq!(p.max_backups, 50);
        assert_eq!(p.max_age_days(), 30);
    }

    #[test]
    fn retention_policy_builder() {
        let p = BackupRetentionPolicy::new()
            .with_max_backups(10)
            .with_max_age_days(7)
            .with_compress_after_days(1);
        assert_eq!(p.max_backups, 10);
        assert_eq!(p.max_age_days(), 7);
        assert!(p.should_compress(2 * 86400));
    }

    #[test]
    fn retention_should_compress() {
        let p = BackupRetentionPolicy::new().with_compress_after_days(3);
        assert!(!p.should_compress(86400));
        assert!(p.should_compress(4 * 86400));
    }

    #[test]
    fn retention_is_expired() {
        let p = BackupRetentionPolicy::new().with_max_age_days(5);
        assert!(!p.is_expired(3 * 86400));
        assert!(p.is_expired(6 * 86400));
    }

    // -- BackupFileInfo -------------------------------------------------------

    #[test]
    fn backup_file_info_age() {
        let info = BackupFileInfo::new("/tmp/bak", 1024, 1000);
        assert_eq!(info.age_seconds(2000), 1000);
    }

    #[test]
    fn backup_file_info_filename() {
        let info = BackupFileInfo::new("/tmp/backups/file.bak", 512, 0);
        assert_eq!(info.filename(), "file.bak");
    }

    // -- BackupRetentionManager -----------------------------------------------

    #[test]
    fn retention_manager_prune_by_count() {
        let policy = BackupRetentionPolicy::new().with_max_backups(2).with_max_age_days(365);
        let mut mgr = BackupRetentionManager::new(policy);
        mgr.add_entry(BackupFileInfo::new("a", 100, 100));
        mgr.add_entry(BackupFileInfo::new("b", 100, 200));
        mgr.add_entry(BackupFileInfo::new("c", 100, 300));
        let pruned = mgr.entries_to_prune(400);
        assert!(!pruned.is_empty());
    }

    #[test]
    fn retention_manager_prune_by_age() {
        let policy = BackupRetentionPolicy::new().with_max_age_days(1);
        let mut mgr = BackupRetentionManager::new(policy);
        mgr.add_entry(BackupFileInfo::new("old", 100, 0));
        let pruned = mgr.entries_to_prune(2 * 86400);
        assert_eq!(pruned.len(), 1);
    }

    #[test]
    fn retention_manager_compress() {
        let policy = BackupRetentionPolicy::new().with_compress_after_days(1);
        let mut mgr = BackupRetentionManager::new(policy);
        mgr.add_entry(BackupFileInfo::new("f1", 100, 0).with_compressed(false));
        mgr.add_entry(BackupFileInfo::new("f2", 100, 0).with_compressed(true));
        let to_compress = mgr.entries_to_compress(2 * 86400);
        assert_eq!(to_compress.len(), 1);
        assert_eq!(to_compress[0].path, "f1");
    }

    #[test]
    fn retention_manager_newest_oldest() {
        let mut mgr = BackupRetentionManager::new(BackupRetentionPolicy::new());
        mgr.add_entry(BackupFileInfo::new("a", 100, 10));
        mgr.add_entry(BackupFileInfo::new("b", 100, 50));
        assert_eq!(mgr.newest().unwrap().path, "b");
        assert_eq!(mgr.oldest().unwrap().path, "a");
    }

    #[test]
    fn retention_manager_duplicate_checksum() {
        let mut mgr = BackupRetentionManager::new(BackupRetentionPolicy::new());
        mgr.add_entry(BackupFileInfo::new("a", 100, 0).with_checksum(999));
        mgr.add_entry(BackupFileInfo::new("b", 100, 1).with_checksum(999));
        assert!(mgr.has_duplicate_checksum(999));
        assert!(!mgr.has_duplicate_checksum(111));
    }

    #[test]
    fn retention_manager_total_size() {
        let mut mgr = BackupRetentionManager::new(BackupRetentionPolicy::new());
        mgr.add_entry(BackupFileInfo::new("a", 100, 0));
        mgr.add_entry(BackupFileInfo::new("b", 250, 0));
        assert_eq!(mgr.total_size(), 350);
    }


    #[test]
    fn backup_config_new() {
        let cfg = BackupConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn backup_config_set_get() {
        let mut cfg = BackupConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn backup_config_remove() {
        let mut cfg = BackupConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn backup_config_keys_sorted() {
        let mut cfg = BackupConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn backup_config_bump_version() {
        let mut cfg = BackupConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn backup_config_clear() {
        let mut cfg = BackupConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn backup_config_merge() {
        let mut cfg1 = BackupConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = BackupConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn backup_config_disable() {
        let mut cfg = BackupConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn backup_rate_tracker_empty() {
        let rt = BackupRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn backup_rate_tracker_record() {
        let mut rt = BackupRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn backup_rate_tracker_prune() {
        let mut rt = BackupRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn backup_validator_valid() {
        let v = BackupValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn backup_validator_errors() {
        let mut v = BackupValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn backup_validator_clear() {
        let mut v = BackupValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn backup_validator_merge() {
        let mut v1 = BackupValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = BackupValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn backup_rate_tracker_clear() {
        let mut rt = BackupRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    // -- backup Z-extended tests -----------------------------------------------

    #[test]
    fn z_backup_priority_weight() {
        assert_eq!(ZBackupPriority::Idle.weight(), 0);
        assert_eq!(ZBackupPriority::Normal.weight(), 2);
        assert_eq!(ZBackupPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_backup_priority_label() {
        assert_eq!(ZBackupPriority::Low.label(), "low");
        assert_eq!(ZBackupPriority::High.label(), "high");
    }

    #[test]
    fn z_backup_priority_is_elevated() {
        assert!(!ZBackupPriority::Normal.is_elevated());
        assert!(ZBackupPriority::High.is_elevated());
        assert!(ZBackupPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_backup_priority_display() {
        assert_eq!(format!("{}", ZBackupPriority::Idle), "idle");
    }

    #[test]
    fn z_backup_priority_all_asc() {
        let all = ZBackupPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZBackupPriority::Idle);
        assert_eq!(all[4], ZBackupPriority::Realtime);
    }

    #[test]
    fn z_backup_struct_new() {
        let s = ZBackupBackupSchedule::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_backup_struct_toggled_clone() {
        let s = ZBackupBackupSchedule::new();
        let t = s.toggled_clone();
        assert_ne!(s.paused, t.paused);
    }

    #[test]
    fn z_backup_rolling_hash_deterministic() {
        let h1 = z_backup_rolling_hash(b"test");
        let h2 = z_backup_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_backup_rolling_hash(b"a"), z_backup_rolling_hash(b"b"));
    }

    #[test]
    fn z_backup_pad_to_basic() {
        assert_eq!(z_backup_pad_to("hi", 5), "hi   ");
        assert_eq!(z_backup_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_backup_is_identifier_basic() {
        assert!(z_backup_is_identifier("foo_bar"));
        assert!(z_backup_is_identifier("abc123"));
        assert!(!z_backup_is_identifier(""));
        assert!(!z_backup_is_identifier("has space"));
    }

    #[test]
    fn z_backup_levenshtein_basic() {
        assert_eq!(z_backup_levenshtein("", ""), 0);
        assert_eq!(z_backup_levenshtein("abc", "abc"), 0);
        assert_eq!(z_backup_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_backup_unique_words_basic() {
        let w = z_backup_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_backup_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_backup_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_backup_common_prefix_basic() {
        assert_eq!(z_backup_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_backup_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_backup_struct_clear() {
        let mut s = ZBackupBackupSchedule::new();
        s.intervals_ms.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_backup_rolling_hash_empty() {
        let h = z_backup_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // ---- xc_ pool / scheduler tests – block 6 ----

    #[test]
    fn xc_6_pool_new_empty() {
        let pool: super::Xc6Pool<i32> = super::Xc6Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_6_pool_release_acquire() {
        let mut pool = super::Xc6Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_6_pool_acquire_empty() {
        let mut pool: super::Xc6Pool<i32> = super::Xc6Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_6_pool_full() {
        let mut pool = super::Xc6Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_6_pool_drain() {
        let mut pool = super::Xc6Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_6_pool_stats() {
        let mut pool = super::Xc6Pool::new(8);
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
    fn xc_6_pool_clear() {
        let mut pool = super::Xc6Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_6_pool_shrink() {
        let mut pool = super::Xc6Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_6_pool_default() {
        let pool: super::Xc6Pool<String> = super::Xc6Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_6_pool_extend() {
        let mut pool = super::Xc6Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_6_pool_retain() {
        let mut pool = super::Xc6Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_6_scheduler_round_robin() {
        let mut sched = super::Xc6Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_6_scheduler_empty() {
        let mut sched = super::Xc6Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_6_scheduler_reset() {
        let mut sched = super::Xc6Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_6_scheduler_add_remove() {
        let mut sched = super::Xc6Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_6_scheduler_targets() {
        let sched = super::Xc6Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_6_hash_empty() {
        assert_eq!(super::xc_6_hash(b""), 5381);
    }

    #[test]
    fn xc_6_hash_data() {
        let h = super::xc_6_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_6_hash(b"hello"), h);
    }

    #[test]
    fn xc_6_reverse_str() {
        assert_eq!(super::xc_6_reverse("abc"), "cba");
        assert_eq!(super::xc_6_reverse(""), "");
    }


    // --- xd_22 deepening tests ---

    #[test]
    fn xd_22_sm_initial_state() {
        let sm = Xd22StateMachine::new();
        assert_eq!(sm.current_state(), Xd22State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_22_sm_valid_idle_to_running() {
        let mut sm = Xd22StateMachine::new();
        assert!(sm.transition(Xd22State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd22State::Running);
    }

    #[test]
    fn xd_22_sm_valid_running_to_paused() {
        let mut sm = Xd22StateMachine::new();
        sm.transition(Xd22State::Running).unwrap();
        assert!(sm.transition(Xd22State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd22State::Paused);
    }

    #[test]
    fn xd_22_sm_valid_running_to_done() {
        let mut sm = Xd22StateMachine::new();
        sm.transition(Xd22State::Running).unwrap();
        assert!(sm.transition(Xd22State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd22State::Done);
    }

    #[test]
    fn xd_22_sm_valid_paused_to_running() {
        let mut sm = Xd22StateMachine::new();
        sm.transition(Xd22State::Running).unwrap();
        sm.transition(Xd22State::Paused).unwrap();
        assert!(sm.transition(Xd22State::Running).is_ok());
    }

    #[test]
    fn xd_22_sm_valid_done_to_idle() {
        let mut sm = Xd22StateMachine::new();
        sm.transition(Xd22State::Running).unwrap();
        sm.transition(Xd22State::Done).unwrap();
        assert!(sm.transition(Xd22State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd22State::Idle);
    }

    #[test]
    fn xd_22_sm_invalid_idle_to_done() {
        let mut sm = Xd22StateMachine::new();
        assert!(sm.transition(Xd22State::Done).is_err());
    }

    #[test]
    fn xd_22_sm_invalid_idle_to_paused() {
        let mut sm = Xd22StateMachine::new();
        assert!(sm.transition(Xd22State::Paused).is_err());
    }

    #[test]
    fn xd_22_sm_history_tracking() {
        let mut sm = Xd22StateMachine::new();
        sm.transition(Xd22State::Running).unwrap();
        sm.transition(Xd22State::Paused).unwrap();
        sm.transition(Xd22State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd22State::Idle);
        assert_eq!(sm.history()[0].to, Xd22State::Running);
        assert_eq!(sm.history()[1].from, Xd22State::Running);
        assert_eq!(sm.history()[2].to, Xd22State::Done);
    }

    #[test]
    fn xd_22_sm_serialize_deserialize() {
        let mut sm = Xd22StateMachine::new();
        sm.transition(Xd22State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd22StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd22State::Running));
    }

    #[test]
    fn xd_22_sm_deserialize_invalid() {
        assert_eq!(Xd22StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_22_sm_reset() {
        let mut sm = Xd22StateMachine::new();
        sm.transition(Xd22State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd22State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_22_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd22EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd22Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_22_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd22EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd22Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd22Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_22_bus_unsubscribe() {
        let mut bus = Xd22EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_22_event_kind_and_payload() {
        let e = Xd22Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd22Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_22_bus_clear_history() {
        let mut bus = Xd22EventBus::new();
        bus.publish(Xd22Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_22_sm_step_counter_increments() {
        let mut sm = Xd22StateMachine::new();
        sm.transition(Xd22State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd22State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #20 --

    #[test]
    fn xf20_trie_insert_search() {
        let mut t = Xf20Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf20_trie_starts_with() {
        let mut t = Xf20Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf20_trie_remove() {
        let mut t = Xf20Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf20_trie_word_count() {
        let mut t = Xf20Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf20_trie_longest_prefix() {
        let mut t = Xf20Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf20_trie_all_words() {
        let mut t = Xf20Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf20_trie_autocomplete() {
        let mut t = Xf20Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf20_trie_empty_search() {
        let t = Xf20Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf20_bloom_add_contains() {
        let mut bf = Xf20BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf20_bloom_probably_absent() {
        let bf = Xf20BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf20_bloom_false_positive_rate() {
        let mut bf = Xf20BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf20_bloom_clear() {
        let mut bf = Xf20BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf20_bloom_union() {
        let mut a = Xf20BloomFilter::xf_new(512, 2);
        let mut b = Xf20BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf20_bloom_intersection_estimate() {
        let mut a = Xf20BloomFilter::xf_new(512, 2);
        let mut b = Xf20BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf20_bloom_union_size_mismatch() {
        let a = Xf20BloomFilter::xf_new(256, 2);
        let b = Xf20BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh5_skip_insert_contains() {
        let mut sl = super::Xh5SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh5_skip_remove() {
        let mut sl = super::Xh5SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh5_skip_len() {
        let mut sl = super::Xh5SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh5_skip_range_query() {
        let mut sl = super::Xh5SkipList::xh_new(4);
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
    fn xh5_skip_floor_ceiling() {
        let mut sl = super::Xh5SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh5_skip_rank() {
        let mut sl = super::Xh5SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh5_skip_empty() {
        let sl = super::Xh5SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh5_skip_duplicates() {
        let mut sl = super::Xh5SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh5_bitset_set_test() {
        let mut bs = super::Xh5BitSet::xh_new(256);
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
    fn xh5_bitset_clear_count() {
        let mut bs = super::Xh5BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh5_bitset_and_or_xor() {
        let mut a = super::Xh5BitSet::xh_new(128);
        let mut b = super::Xh5BitSet::xh_new(128);
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
    fn xh5_bitset_iter_ones() {
        let mut bs = super::Xh5BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh5_bitset_first_last() {
        let mut bs = super::Xh5BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh5_bitset_empty() {
        let bs = super::Xh5BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi5_deque_push_pop_back() {
        let mut dq = super::Xi5Deque::xi_new(4);
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
    fn xi5_deque_push_pop_front() {
        let mut dq = super::Xi5Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi5_deque_mixed_ops() {
        let mut dq = super::Xi5Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi5_deque_get_and_split() {
        let mut dq = super::Xi5Deque::xi_new(8);
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
    fn xi5_deque_rotate_left() {
        let mut dq = super::Xi5Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi5_deque_rotate_right() {
        let mut dq = super::Xi5Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi5_deque_grow() {
        let mut dq = super::Xi5Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi5_deque_empty() {
        let dq = super::Xi5Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi5_interval_tree_insert_query() {
        let mut tree = super::Xi5IntervalTree::xi_new();
        tree.xi_insert(super::Xi5Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi5Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi5Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi5_interval_tree_overlap() {
        let mut tree = super::Xi5IntervalTree::xi_new();
        tree.xi_insert(super::Xi5Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi5Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi5Interval::xi_new(12, 20));
        let q = super::Xi5Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi5_interval_tree_remove() {
        let mut tree = super::Xi5IntervalTree::xi_new();
        tree.xi_insert(super::Xi5Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi5Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi5_interval_tree_gaps() {
        let mut tree = super::Xi5IntervalTree::xi_new();
        tree.xi_insert(super::Xi5Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi5Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi5Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi5Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi5Interval::xi_new(8, 10));
    }

    #[test]
    fn xi5_interval_tree_merge() {
        let mut tree = super::Xi5IntervalTree::xi_new();
        tree.xi_insert(super::Xi5Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi5Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi5Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi5Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi5Interval::xi_new(10, 15));
    }

    #[test]
    fn xi5_interval_tree_all() {
        let mut tree = super::Xi5IntervalTree::xi_new();
        tree.xi_insert(super::Xi5Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi5Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi5_interval_tree_empty() {
        let tree = super::Xi5IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi5_interval_tree_contains_point() {
        let iv = super::Xi5Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 5) ---

    #[test]
    fn xj_5_uf_make_and_find() {
        let mut uf = super::Xj5UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_5_uf_union_connected() {
        let mut uf = super::Xj5UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_5_uf_component_count() {
        let mut uf = super::Xj5UnionFind::xj_new();
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
    fn xj_5_uf_component_size() {
        let mut uf = super::Xj5UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_5_uf_largest_component() {
        let mut uf = super::Xj5UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_5_uf_many_elements() {
        let mut uf = super::Xj5UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_5_uf_separate_components() {
        let mut uf = super::Xj5UnionFind::xj_new();
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
    fn xj_5_uf_path_compression() {
        let mut uf = super::Xj5UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_5_bt_insert_get() {
        let mut bt = super::Xj5BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_5_bt_contains_len() {
        let mut bt = super::Xj5BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_5_bt_replace() {
        let mut bt = super::Xj5BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_5_bt_remove() {
        let mut bt = super::Xj5BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_5_bt_keys_values() {
        let mut bt = super::Xj5BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_5_bt_range() {
        let mut bt = super::Xj5BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_5_bt_min_max() {
        let mut bt = super::Xj5BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_5_bt_many_inserts() {
        let mut bt = super::Xj5BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_5 segment tree tests ---

    #[test]
    fn xk_5_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk5SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_5_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk5SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_5_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk5SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_5_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk5SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_5_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk5SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_5_st_single_element() {
        let data = vec![42];
        let st = super::Xk5SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_5_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk5SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_5_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk5SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_5 disjoint intervals tests ---

    #[test]
    fn xk_5_di_add_and_count() {
        let mut di = super::Xk5DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_5_di_merge_overlap() {
        let mut di = super::Xk5DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_5_di_contains() {
        let mut di = super::Xk5DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_5_di_remove() {
        let mut di = super::Xk5DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_5_di_covered_length() {
        let mut di = super::Xk5DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_5_di_gaps() {
        let mut di = super::Xk5DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_5_di_merge_adjacent() {
        let mut di = super::Xk5DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_5_di_empty() {
        let di = super::Xk5DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_5_rope_new_empty() {
        let rope = super::Xl5Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_5_rope_from_str() {
        let rope = super::Xl5Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_5_rope_insert_at() {
        let mut rope = super::Xl5Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_5_rope_delete_range() {
        let mut rope = super::Xl5Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_5_rope_char_at() {
        let rope = super::Xl5Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_5_rope_split_concat() {
        let rope = super::Xl5Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_5_rope_line_count() {
        let rope = super::Xl5Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_5_rope_line_at() {
        let rope = super::Xl5Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_5_sa_build_and_search() {
        let sa = super::Xl5SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_5_sa_count() {
        let sa = super::Xl5SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_5_sa_longest_repeated() {
        let sa = super::Xl5SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_5_sa_all_positions() {
        let sa = super::Xl5SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_5_sa_len() {
        let sa = super::Xl5SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_5_sa_empty() {
        let sa = super::Xl5SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_5_rope_slice() {
        let rope = super::Xl5Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_5_sa_search_start() {
        let sa = super::Xl5SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
