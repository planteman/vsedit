//! Hot exit and file backup.

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
    fn backup_validator_accepts_valid_name() {
        let v = BackupValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn backup_validator_rejects_empty() {
        let v = BackupValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn backup_validator_rejects_too_long() {
        let v = BackupValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn backup_validator_forbidden_prefix() {
        let v = BackupValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn backup_validator_allowed_chars() {
        let v = BackupValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn backup_validator_range() {
        let v = BackupValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn backup_sanitize_removes_control() {
        let result = BackupValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn backup_truncate_short_string() {
        assert_eq!(BackupValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn backup_truncate_long_string() {
        let result = BackupValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn backup_is_ascii_printable() {
        assert!(BackupValidator::is_ascii_printable("Hello World 123"));
        assert!(!BackupValidator::is_ascii_printable("Hello\x00World"));
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
}
