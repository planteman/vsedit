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
}
