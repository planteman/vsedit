//! Local file history tracking.
//!
//! Records content snapshots keyed by URI + timestamp so that previous
//! versions of a file can be inspected or restored.

use std::fmt;
// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// How the history entry was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistorySource {
    /// Automatically captured on save.
    Auto,
    /// Explicitly requested by the user.
    Manual,
    /// Created from an undo operation.
    Undo,
}

/// A single history snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub uri: String,
    pub timestamp: u64,
    pub content_hash: String,
    pub label: Option<String>,
    pub source: HistorySource,
    pub content: Option<String>,
    pub size_bytes: u64,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Manages a per-file history of content snapshots.
#[derive(Debug, Clone)]
pub struct LocalHistoryService {
    pub entries: Vec<HistoryEntry>,
    pub max_entries_per_file: usize,
}

impl LocalHistoryService {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries_per_file: max_entries,
        }
    }

    /// Record a new history entry, using monotonically increasing timestamps
    /// derived from the current entry count to avoid external dependencies.
    pub fn add_entry(&mut self, uri: &str, content_hash: &str, source: HistorySource) {
        let timestamp = self
            .entries
            .iter()
            .filter(|e| e.uri == uri)
            .map(|e| e.timestamp)
            .max()
            .map_or(1, |t| t + 1);

        self.entries.push(HistoryEntry {
            uri: uri.to_string(),
            timestamp,
            content_hash: content_hash.to_string(),
            label: None,
            source,
            content: None,
            size_bytes: 0,
        });

        self.prune(uri);
    }

    /// Return history entries for `uri` in reverse-chronological order.
    pub fn get_history(&self, uri: &str) -> Vec<&HistoryEntry> {
        let mut out: Vec<&HistoryEntry> = self.entries.iter().filter(|e| e.uri == uri).collect();
        out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        out
    }

    /// Attach a user-visible label to a specific entry.
    pub fn label_entry(&mut self, uri: &str, timestamp: u64, label: &str) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.uri == uri && e.timestamp == timestamp)
        {
            entry.label = Some(label.to_string());
        }
    }

    /// Keep at most `max_entries_per_file` entries for the given URI,
    /// removing the oldest ones first.
    pub fn prune(&mut self, uri: &str) {
        let count = self.entries.iter().filter(|e| e.uri == uri).count();
        if count <= self.max_entries_per_file {
            return;
        }
        let to_remove = count - self.max_entries_per_file;
        let mut removed = 0;
        self.entries.retain(|e| {
            if e.uri == uri && removed < to_remove {
                removed += 1;
                false
            } else {
                true
            }
        });
    }

    pub fn clear_all(&mut self) {
        self.entries.clear();
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

// ---------------------------------------------------------------------------
// Extra LocalHistoryService methods
// ---------------------------------------------------------------------------

use std::collections::HashMap;

impl LocalHistoryService {
    /// Get a reference to a specific entry by URI and timestamp.
    pub fn get_entry(&self, uri: &str, timestamp: u64) -> Option<&HistoryEntry> {
        self.entries
            .iter()
            .find(|e| e.uri == uri && e.timestamp == timestamp)
    }

    /// Remove an entry by URI and timestamp. Returns `true` if removed.
    pub fn remove_entry(&mut self, uri: &str, timestamp: u64) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|e| !(e.uri == uri && e.timestamp == timestamp));
        self.entries.len() < before
    }

    /// Return the set of unique URIs across all entries.
    pub fn get_unique_uris(&self) -> Vec<String> {
        let mut uris: Vec<String> = self
            .entries
            .iter()
            .map(|e| e.uri.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        uris.sort();
        uris
    }

    /// Total size in bytes across all entries.
    pub fn total_size_bytes(&self) -> u64 {
        self.entries.iter().map(|e| e.size_bytes).sum()
    }

    /// Total size in bytes for a specific URI.
    pub fn size_for_uri(&self, uri: &str) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.uri == uri)
            .map(|e| e.size_bytes)
            .sum()
    }

    /// Remove all entries whose timestamp is older than `current_time - max_age`.
    pub fn prune_by_age(&mut self, max_age: u64, current_time: u64) {
        let cutoff = current_time.saturating_sub(max_age);
        self.entries.retain(|e| e.timestamp >= cutoff);
    }

    /// Compute aggregate statistics.
    pub fn get_stats(&self) -> HistoryStats {
        let mut entries_per_source: HashMap<String, usize> = HashMap::new();
        for entry in &self.entries {
            let key = format!("{:?}", entry.source);
            *entries_per_source.entry(key).or_insert(0) += 1;
        }
        HistoryStats {
            total_entries: self.entries.len(),
            unique_files: self.get_unique_uris().len(),
            total_size: self.total_size_bytes(),
            entries_per_source,
        }
    }
}

// ---------------------------------------------------------------------------
// HistoryStats
// ---------------------------------------------------------------------------

/// Aggregated statistics for the local history.
#[derive(Debug, Clone)]
pub struct HistoryStats {
    pub total_entries: usize,
    pub unique_files: usize,
    pub total_size: u64,
    pub entries_per_source: HashMap<String, usize>,
}

// ---------------------------------------------------------------------------
// HistoryStorageProvider trait
// ---------------------------------------------------------------------------

/// Trait for persisting history entries.
pub trait HistoryStorageProvider {
    /// Save entries. Default is a no-op.
    fn save(&self, _entries: &[HistoryEntry]) -> Result<(), String> {
        Ok(())
    }

    /// Load entries. Default returns an empty list.
    fn load(&self) -> Result<Vec<HistoryEntry>, String> {
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// HistoryEntry helper methods
// ---------------------------------------------------------------------------

impl HistoryEntry {
    /// Returns `true` if this entry was created automatically.
    pub fn is_auto(&self) -> bool {
        self.source == HistorySource::Auto
    }

    /// Returns `true` if this entry was created manually.
    pub fn is_manual(&self) -> bool {
        self.source == HistorySource::Manual
    }

    /// Returns `true` if this entry has a label set.
    pub fn has_label(&self) -> bool {
        self.label.is_some()
    }

    /// Returns `true` if this entry has content stored.
    pub fn has_content(&self) -> bool {
        self.content.is_some()
    }

    /// Returns the age of this entry relative to `current_time`.
    pub fn age(&self, current_time: u64) -> u64 {
        current_time.saturating_sub(self.timestamp)
    }
}

// ---------------------------------------------------------------------------
// Display impls
// ---------------------------------------------------------------------------

impl std::fmt::Display for HistorySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HistorySource::Auto => write!(f, "auto"),
            HistorySource::Manual => write!(f, "manual"),
            HistorySource::Undo => write!(f, "undo"),
        }
    }
}

impl std::fmt::Display for HistoryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} @{} ({})",
            self.source, self.uri, self.timestamp, self.content_hash
        )
    }
}

// ---------------------------------------------------------------------------
// PartialEq for HistoryStats
// ---------------------------------------------------------------------------

impl PartialEq for HistoryStats {
    fn eq(&self, other: &Self) -> bool {
        self.total_entries == other.total_entries
            && self.unique_files == other.unique_files
            && self.total_size == other.total_size
            && self.entries_per_source == other.entries_per_source
    }
}

impl Eq for HistoryStats {}

// ---------------------------------------------------------------------------
// HistoryDiff
// ---------------------------------------------------------------------------

/// Represents a diff between two history entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryDiff {
    pub uri: String,
    pub from_timestamp: u64,
    pub to_timestamp: u64,
    pub hash_changed: bool,
    pub size_delta: i64,
}

/// Compare two history entries and produce a [`HistoryDiff`].
pub fn compute_diff(a: &HistoryEntry, b: &HistoryEntry) -> HistoryDiff {
    HistoryDiff {
        uri: a.uri.clone(),
        from_timestamp: a.timestamp,
        to_timestamp: b.timestamp,
        hash_changed: a.content_hash != b.content_hash,
        size_delta: b.size_bytes as i64 - a.size_bytes as i64,
    }
}

// ---------------------------------------------------------------------------
// HistoryFilter
// ---------------------------------------------------------------------------

/// Optional filter criteria for querying history entries.
#[derive(Debug, Clone, Default)]
pub struct HistoryFilter {
    pub source: Option<HistorySource>,
    pub min_timestamp: Option<u64>,
    pub max_timestamp: Option<u64>,
    pub label_contains: Option<String>,
}

impl LocalHistoryService {
    /// Return entries matching the given filter.
    pub fn filter_entries(&self, filter: &HistoryFilter) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| {
                if let Some(src) = &filter.source {
                    if e.source != *src {
                        return false;
                    }
                }
                if let Some(min) = filter.min_timestamp {
                    if e.timestamp < min {
                        return false;
                    }
                }
                if let Some(max) = filter.max_timestamp {
                    if e.timestamp > max {
                        return false;
                    }
                }
                if let Some(ref substr) = filter.label_contains {
                    match &e.label {
                        Some(label) => {
                            if !label.contains(substr.as_str()) {
                                return false;
                            }
                        }
                        None => return false,
                    }
                }
                true
            })
            .collect()
    }

    /// Return the most recent entry for a given URI, or `None`.
    pub fn get_latest_entry(&self, uri: &str) -> Option<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.uri == uri)
            .max_by_key(|e| e.timestamp)
    }

    /// Return entries created by a specific source.
    pub fn get_entries_by_source(&self, source: HistorySource) -> Vec<&HistoryEntry> {
        self.entries.iter().filter(|e| e.source == source).collect()
    }

    /// Search for entries whose label contains the given substring.
    pub fn search_by_label(&self, needle: &str) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| {
                e.label
                    .as_ref()
                    .map_or(false, |l| l.contains(needle))
            })
            .collect()
    }

    /// Remove entries with duplicate content hashes per URI, keeping the
    /// most recent entry for each unique hash.
    pub fn compact(&mut self) {
        let mut seen: HashMap<(String, String), u64> = HashMap::new();
        for entry in &self.entries {
            let key = (entry.uri.clone(), entry.content_hash.clone());
            seen.entry(key)
                .and_modify(|ts| {
                    if entry.timestamp > *ts {
                        *ts = entry.timestamp;
                    }
                })
                .or_insert(entry.timestamp);
        }
        self.entries.retain(|e| {
            let key = (e.uri.clone(), e.content_hash.clone());
            seen.get(&key) == Some(&e.timestamp)
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// History pruning by age/count combined
// ---------------------------------------------------------------------------

impl LocalHistoryService {
    /// Prune entries by both age and per-file count.
    ///
    /// First removes entries older than `max_age` relative to `current_time`,
    /// then enforces the per-file count limit on remaining entries.
    pub fn prune_combined(&mut self, max_age: u64, current_time: u64) {
        self.prune_by_age(max_age, current_time);
        let uris: Vec<String> = self.get_unique_uris();
        for uri in uris {
            self.prune(&uri);
        }
    }

    /// Estimate the total storage size including content strings.
    ///
    /// Sums `size_bytes` fields plus the in-memory length of stored content.
    pub fn estimate_storage_size(&self) -> u64 {
        let mut total = self.total_size_bytes();
        for e in &self.entries {
            if let Some(ref c) = e.content {
                total += c.len() as u64;
            }
        }
        total
    }

    /// Search entries whose stored content contains the given needle.
    pub fn search_by_content(&self, needle: &str) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| {
                e.content
                    .as_ref()
                    .map_or(false, |c| c.contains(needle))
            })
            .collect()
    }

    /// Compute diffs between consecutive entries for a URI.
    ///
    /// Returns diffs ordered from oldest pair to newest pair.
    pub fn compute_consecutive_diffs(&self, uri: &str) -> Vec<HistoryDiff> {
        let mut entries: Vec<&HistoryEntry> = self.entries.iter().filter(|e| e.uri == uri).collect();
        entries.sort_by_key(|e| e.timestamp);
        entries
            .windows(2)
            .map(|pair| compute_diff(pair[0], pair[1]))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Garbage collector and line-level diff
// ---------------------------------------------------------------------------

/// Configuration for automatic garbage collection of history entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GarbageCollectorConfig {
    /// Maximum age of entries in seconds.
    pub max_age_seconds: u64,
    /// Maximum number of entries per file.
    pub max_entries_per_file: usize,
    /// Maximum total size in bytes across all entries.
    pub max_total_size_bytes: u64,
}

impl Default for GarbageCollectorConfig {
    fn default() -> Self {
        Self {
            max_age_seconds: 7 * 24 * 60 * 60, // 7 days
            max_entries_per_file: 50,
            max_total_size_bytes: 50 * 1024 * 1024, // 50 MB
        }
    }
}

/// Garbage collector for local history entries.
pub struct LocalHistoryGarbageCollector {
    pub config: GarbageCollectorConfig,
}

/// Result of a garbage collection run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GarbageCollectionResult {
    pub entries_before: usize,
    pub entries_after: usize,
    pub entries_removed: usize,
    pub bytes_freed: u64,
}

impl LocalHistoryGarbageCollector {
    pub fn new(config: GarbageCollectorConfig) -> Self {
        Self { config }
    }

    /// Run garbage collection on the given service at the specified current time.
    pub fn collect(&self, service: &mut LocalHistoryService, current_time: u64) -> GarbageCollectionResult {
        let entries_before = service.entry_count();
        let size_before = service.total_size_bytes();

        // Phase 1: Remove entries older than max_age
        service.prune_by_age(self.config.max_age_seconds, current_time);

        // Phase 2: Enforce per-file limit
        let uris = service.get_unique_uris();
        for uri in &uris {
            let count = service.entries.iter().filter(|e| e.uri == *uri).count();
            if count > self.config.max_entries_per_file {
                let old_max = service.max_entries_per_file;
                service.max_entries_per_file = self.config.max_entries_per_file;
                service.prune(uri);
                service.max_entries_per_file = old_max;
            }
        }

        // Phase 3: Enforce total size limit
        while service.total_size_bytes() > self.config.max_total_size_bytes && !service.entries.is_empty() {
            // Remove the oldest entry
            if let Some(oldest_idx) = service
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.timestamp)
                .map(|(i, _)| i)
            {
                service.entries.remove(oldest_idx);
            }
        }

        let entries_after = service.entry_count();
        let size_after = service.total_size_bytes();
        GarbageCollectionResult {
            entries_before,
            entries_after,
            entries_removed: entries_before.saturating_sub(entries_after),
            bytes_freed: size_before.saturating_sub(size_after),
        }
    }

    /// Check if garbage collection is needed based on current state.
    pub fn needs_collection(&self, service: &LocalHistoryService, current_time: u64) -> bool {
        if service.total_size_bytes() > self.config.max_total_size_bytes {
            return true;
        }
        service.entries.iter().any(|e| e.age(current_time) > self.config.max_age_seconds)
    }
}

/// A changed line between two history entry contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineDiff {
    pub line_number: usize,
    pub kind: LineDiffKind,
    pub content: String,
}

/// Kind of line difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineDiffKind {
    Added,
    Removed,
    Modified,
}

impl fmt::Display for LineDiffKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineDiffKind::Added => write!(f, "+"),
            LineDiffKind::Removed => write!(f, "-"),
            LineDiffKind::Modified => write!(f, "~"),
        }
    }
}

/// Compute line-level diffs between two content strings.
pub fn history_diff(old_content: &str, new_content: &str) -> Vec<LineDiff> {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();
    let mut diffs = Vec::new();
    let max_len = old_lines.len().max(new_lines.len());

    for i in 0..max_len {
        match (old_lines.get(i), new_lines.get(i)) {
            (Some(old), Some(new)) => {
                if old != new {
                    diffs.push(LineDiff {
                        line_number: i + 1,
                        kind: LineDiffKind::Modified,
                        content: new.to_string(),
                    });
                }
            }
            (None, Some(new)) => {
                diffs.push(LineDiff {
                    line_number: i + 1,
                    kind: LineDiffKind::Added,
                    content: new.to_string(),
                });
            }
            (Some(old), None) => {
                diffs.push(LineDiff {
                    line_number: i + 1,
                    kind: LineDiffKind::Removed,
                    content: old.to_string(),
                });
            }
            (None, None) => {}
        }
    }

    diffs
}

/// Compute the number of changed lines between two content strings.
pub fn history_diff_count(old_content: &str, new_content: &str) -> usize {
    history_diff(old_content, new_content).len()
}

// ---------------------------------------------------------------------------
// HistoryEntry additional helpers
// ---------------------------------------------------------------------------

impl HistoryEntry {
    pub fn age_secs(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_recent(&self, now: u64, threshold_secs: u64) -> bool {
        self.age_secs(now) <= threshold_secs
    }

    pub fn is_undo(&self) -> bool {
        self.source == HistorySource::Undo
    }
}

// ---------------------------------------------------------------------------
// DiffSummary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSummary {
    pub additions: usize,
    pub deletions: usize,
    pub modifications: usize,
}

impl DiffSummary {
    pub fn total(&self) -> usize {
        self.additions + self.deletions + self.modifications
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

impl fmt::Display for DiffSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "+{} -{} ~{}",
            self.additions, self.deletions, self.modifications
        )
    }
}

// ---------------------------------------------------------------------------
// HistoryDiff extensions
// ---------------------------------------------------------------------------

impl HistoryDiff {
    pub fn is_empty(&self) -> bool {
        !self.hash_changed && self.size_delta == 0
    }

    pub fn time_span(&self) -> u64 {
        self.to_timestamp.saturating_sub(self.from_timestamp)
    }
}

pub fn compute_diff_summary(old_content: &str, new_content: &str) -> DiffSummary {
    let diffs = history_diff(old_content, new_content);
    let mut additions = 0;
    let mut deletions = 0;
    let mut modifications = 0;
    for d in &diffs {
        match d.kind {
            LineDiffKind::Added => additions += 1,
            LineDiffKind::Removed => deletions += 1,
            LineDiffKind::Modified => modifications += 1,
        }
    }
    DiffSummary {
        additions,
        deletions,
        modifications,
    }
}

// ---------------------------------------------------------------------------
// HistoryFilter extensions
// ---------------------------------------------------------------------------

impl HistoryFilter {
    pub fn by_source(source: HistorySource) -> Self {
        Self {
            source: Some(source),
            ..Default::default()
        }
    }

    pub fn by_time_range(min: u64, max: u64) -> Self {
        Self {
            min_timestamp: Some(min),
            max_timestamp: Some(max),
            ..Default::default()
        }
    }

    pub fn matches(&self, entry: &HistoryEntry) -> bool {
        if let Some(src) = &self.source {
            if entry.source != *src {
                return false;
            }
        }
        if let Some(min) = self.min_timestamp {
            if entry.timestamp < min {
                return false;
            }
        }
        if let Some(max) = self.max_timestamp {
            if entry.timestamp > max {
                return false;
            }
        }
        if let Some(ref substr) = self.label_contains {
            match &entry.label {
                Some(label) => {
                    if !label.contains(substr.as_str()) {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// LocalHistoryService: entries_for_file, oldest/newest, iterator, stats
// ---------------------------------------------------------------------------

impl LocalHistoryService {
    pub fn entries_for_file(&self, uri: &str) -> Vec<&HistoryEntry> {
        self.entries.iter().filter(|e| e.uri == uri).collect()
    }

    pub fn oldest_entry(&self) -> Option<&HistoryEntry> {
        self.entries.iter().min_by_key(|e| e.timestamp)
    }

    pub fn newest_entry(&self) -> Option<&HistoryEntry> {
        self.entries.iter().max_by_key(|e| e.timestamp)
    }

    pub fn total_diffs(&self, uri: &str) -> usize {
        let count = self.entries.iter().filter(|e| e.uri == uri).count();
        count.saturating_sub(1)
    }

    pub fn average_entry_size(&self) -> f64 {
        if self.entries.is_empty() {
            return 0.0;
        }
        self.total_size_bytes() as f64 / self.entries.len() as f64
    }

    pub fn recent_entries(&self, now: u64, threshold_secs: u64) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.is_recent(now, threshold_secs))
            .collect()
    }
}

impl<'a> IntoIterator for &'a LocalHistoryService {
    type Item = &'a HistoryEntry;
    type IntoIter = std::slice::Iter<'a, HistoryEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

// ---------------------------------------------------------------------------
// GarbageCollectionResult extensions
// ---------------------------------------------------------------------------

impl GarbageCollectionResult {
    pub fn had_effect(&self) -> bool {
        self.entries_removed > 0
    }

    pub fn removal_ratio(&self) -> f64 {
        if self.entries_before == 0 {
            return 0.0;
        }
        self.entries_removed as f64 / self.entries_before as f64
    }
}

impl fmt::Display for GarbageCollectionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GC: removed {} of {} entries, freed {} bytes",
            self.entries_removed, self.entries_before, self.bytes_freed
        )
    }
}

// ---------------------------------------------------------------------------
// History export/import
// ---------------------------------------------------------------------------

/// A serializable representation of a history entry for export/import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedEntry {
    pub uri: String,
    pub timestamp: u64,
    pub content_hash: String,
    pub label: Option<String>,
    pub source: String,
    pub content: Option<String>,
    pub size_bytes: u64,
}

impl From<&HistoryEntry> for ExportedEntry {
    fn from(e: &HistoryEntry) -> Self {
        Self {
            uri: e.uri.clone(),
            timestamp: e.timestamp,
            content_hash: e.content_hash.clone(),
            label: e.label.clone(),
            source: format!("{}", e.source),
            content: e.content.clone(),
            size_bytes: e.size_bytes,
        }
    }
}

impl ExportedEntry {
    /// Parse the source string back into a `HistorySource`.
    pub fn parse_source(&self) -> Result<HistorySource, String> {
        match self.source.as_str() {
            "auto" => Ok(HistorySource::Auto),
            "manual" => Ok(HistorySource::Manual),
            "undo" => Ok(HistorySource::Undo),
            other => Err(format!("unknown source: {other}")),
        }
    }

    /// Convert back into a `HistoryEntry`.
    pub fn into_entry(self) -> Result<HistoryEntry, String> {
        let source = self.parse_source()?;
        Ok(HistoryEntry {
            uri: self.uri,
            timestamp: self.timestamp,
            content_hash: self.content_hash,
            label: self.label,
            source,
            content: self.content,
            size_bytes: self.size_bytes,
        })
    }
}

impl LocalHistoryService {
    /// Export all entries as `ExportedEntry` values.
    pub fn export_entries(&self) -> Vec<ExportedEntry> {
        self.entries.iter().map(ExportedEntry::from).collect()
    }

    /// Import entries from exported representations, skipping duplicates
    /// (same URI + timestamp already present).
    pub fn import_entries(&mut self, exported: Vec<ExportedEntry>) -> Result<usize, String> {
        let mut imported = 0usize;
        for ex in exported {
            let already_exists = self
                .entries
                .iter()
                .any(|e| e.uri == ex.uri && e.timestamp == ex.timestamp);
            if already_exists {
                continue;
            }
            let entry = ex.into_entry()?;
            self.entries.push(entry);
            imported += 1;
        }
        Ok(imported)
    }
}

// ---------------------------------------------------------------------------
// Snapshot scheduling logic
// ---------------------------------------------------------------------------

/// Configuration for automatic snapshot scheduling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotSchedule {
    /// Minimum interval between automatic snapshots in seconds.
    pub min_interval_secs: u64,
    /// Only snapshot if content hash changed since last snapshot.
    pub only_on_change: bool,
}

impl Default for SnapshotSchedule {
    fn default() -> Self {
        Self {
            min_interval_secs: 60,
            only_on_change: true,
        }
    }
}

impl SnapshotSchedule {
    /// Determine whether a new snapshot should be taken for the given URI.
    pub fn should_snapshot(
        &self,
        service: &LocalHistoryService,
        uri: &str,
        current_time: u64,
        current_hash: &str,
    ) -> bool {
        let latest = service.get_latest_entry(uri);
        match latest {
            None => true,
            Some(entry) => {
                let elapsed = current_time.saturating_sub(entry.timestamp);
                if elapsed < self.min_interval_secs {
                    return false;
                }
                if self.only_on_change && entry.content_hash == current_hash {
                    return false;
                }
                true
            }
        }
    }

    /// Compute the number of seconds until the next snapshot is allowed.
    /// Returns 0 if a snapshot can be taken now.
    pub fn time_until_next(
        &self,
        service: &LocalHistoryService,
        uri: &str,
        current_time: u64,
    ) -> u64 {
        match service.get_latest_entry(uri) {
            None => 0,
            Some(entry) => {
                let elapsed = current_time.saturating_sub(entry.timestamp);
                self.min_interval_secs.saturating_sub(elapsed)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Per-file statistics
// ---------------------------------------------------------------------------

/// Statistics for a single file's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHistoryStats {
    pub uri: String,
    pub entry_count: usize,
    pub total_size: u64,
    pub oldest_timestamp: u64,
    pub newest_timestamp: u64,
    pub unique_hashes: usize,
}

impl LocalHistoryService {
    /// Compute per-file statistics for a given URI.
    pub fn file_stats(&self, uri: &str) -> Option<FileHistoryStats> {
        let file_entries: Vec<&HistoryEntry> =
            self.entries.iter().filter(|e| e.uri == uri).collect();
        if file_entries.is_empty() {
            return None;
        }
        let total_size: u64 = file_entries.iter().map(|e| e.size_bytes).sum();
        let oldest = file_entries.iter().map(|e| e.timestamp).min().unwrap();
        let newest = file_entries.iter().map(|e| e.timestamp).max().unwrap();
        let unique_hashes: std::collections::HashSet<&str> = file_entries
            .iter()
            .map(|e| e.content_hash.as_str())
            .collect();
        Some(FileHistoryStats {
            uri: uri.to_string(),
            entry_count: file_entries.len(),
            total_size,
            oldest_timestamp: oldest,
            newest_timestamp: newest,
            unique_hashes: unique_hashes.len(),
        })
    }

    /// Compute per-file statistics for all tracked files.
    pub fn all_file_stats(&self) -> Vec<FileHistoryStats> {
        let uris = self.get_unique_uris();
        uris.iter()
            .filter_map(|uri| self.file_stats(uri))
            .collect()
    }

    /// Search entries whose URI contains the given substring (case-insensitive).
    pub fn search_by_filename(&self, needle: &str) -> Vec<&HistoryEntry> {
        let needle_lower = needle.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.uri.to_lowercase().contains(&needle_lower))
            .collect()
    }

    /// Deduplicate entries: for each URI, remove entries with the same
    /// content hash, keeping only the one with the latest timestamp.
    /// Returns the number of entries removed.
    pub fn deduplicate(&mut self) -> usize {
        let before = self.entries.len();
        self.compact();
        before - self.entries.len()
    }

    /// Compare two entries by URI and timestamps, returning a detailed comparison.
    pub fn compare_entries(
        &self,
        uri: &str,
        ts_a: u64,
        ts_b: u64,
    ) -> Option<EntryComparison> {
        let a = self.get_entry(uri, ts_a)?;
        let b = self.get_entry(uri, ts_b)?;
        let diff = compute_diff(a, b);
        let content_diff = match (&a.content, &b.content) {
            (Some(old), Some(new)) => Some(compute_diff_summary(old, new)),
            _ => None,
        };
        Some(EntryComparison {
            uri: uri.to_string(),
            from_timestamp: ts_a,
            to_timestamp: ts_b,
            hash_changed: diff.hash_changed,
            size_delta: diff.size_delta,
            content_diff,
        })
    }
}

/// Detailed comparison of two history entries.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryComparison {
    pub uri: String,
    pub from_timestamp: u64,
    pub to_timestamp: u64,
    pub hash_changed: bool,
    pub size_delta: i64,
    pub content_diff: Option<DiffSummary>,
}

// ---------------------------------------------------------------------------
// Diff – line-level comparison of two content strings
// ---------------------------------------------------------------------------

/// Line-level diff between two content strings.
#[derive(Debug, Clone)]
pub struct LocalHistoryDiff {
    old_lines: Vec<String>,
    new_lines: Vec<String>,
}

impl LocalHistoryDiff {
    pub fn new(old: &str, new: &str) -> Self {
        Self {
            old_lines: old.lines().map(String::from).collect(),
            new_lines: new.lines().map(String::from).collect(),
        }
    }

    pub fn has_changes(&self) -> bool {
        self.old_lines != self.new_lines
    }

    /// Number of lines present in `new` but not at the corresponding position
    /// in `old` (pure additions beyond old length + changed lines).
    pub fn added_lines(&self) -> usize {
        let max = self.old_lines.len().max(self.new_lines.len());
        let mut count = 0usize;
        for i in 0..max {
            let old = self.old_lines.get(i);
            let new = self.new_lines.get(i);
            match (old, new) {
                (None, Some(_)) => count += 1,
                (Some(o), Some(n)) if o != n => count += 1,
                _ => {}
            }
        }
        count
    }

    /// Number of lines present in `old` but missing or changed in `new`.
    pub fn removed_lines(&self) -> usize {
        let max = self.old_lines.len().max(self.new_lines.len());
        let mut count = 0usize;
        for i in 0..max {
            let old = self.old_lines.get(i);
            let new = self.new_lines.get(i);
            match (old, new) {
                (Some(_), None) => count += 1,
                (Some(o), Some(n)) if o != n => count += 1,
                _ => {}
            }
        }
        count
    }

    /// Returns 1-based line numbers where the two texts differ.
    pub fn changed_line_numbers(&self) -> Vec<usize> {
        let max = self.old_lines.len().max(self.new_lines.len());
        let mut nums = Vec::new();
        for i in 0..max {
            let old = self.old_lines.get(i);
            let new = self.new_lines.get(i);
            if old != new {
                nums.push(i + 1);
            }
        }
        nums
    }

    pub fn summary(&self) -> String {
        let added = self.added_lines();
        let removed = self.removed_lines();
        if !self.has_changes() {
            return "No changes".to_string();
        }
        format!(
            "{} line(s) added, {} line(s) removed",
            added, removed
        )
    }

    /// Simple unified-diff-style output with `+`/`-` prefixes.
    pub fn as_unified_diff(&self) -> String {
        let max = self.old_lines.len().max(self.new_lines.len());
        let mut out = String::new();
        for i in 0..max {
            let old = self.old_lines.get(i);
            let new = self.new_lines.get(i);
            match (old, new) {
                (Some(o), Some(n)) if o == n => {
                    out.push(' ');
                    out.push_str(o);
                    out.push('\n');
                }
                (Some(o), Some(n)) => {
                    out.push('-');
                    out.push_str(o);
                    out.push('\n');
                    out.push('+');
                    out.push_str(n);
                    out.push('\n');
                }
                (Some(o), None) => {
                    out.push('-');
                    out.push_str(o);
                    out.push('\n');
                }
                (None, Some(n)) => {
                    out.push('+');
                    out.push_str(n);
                    out.push('\n');
                }
                (None, None) => {}
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Prune – strategies for trimming history entries
// ---------------------------------------------------------------------------

/// Pruning strategies for history entries.
pub struct LocalHistoryPrune;

impl LocalHistoryPrune {
    pub fn new() -> Self {
        Self
    }

    /// Returns indices of entries to *remove* so that at most `max_count`
    /// remain.  Keeps the entries with the highest timestamps (newest).
    pub fn prune_by_count(entries: &[HistoryEntry], max_count: usize) -> Vec<usize> {
        if entries.len() <= max_count {
            return Vec::new();
        }
        let mut indexed: Vec<(usize, u64)> =
            entries.iter().enumerate().map(|(i, e)| (i, e.timestamp)).collect();
        // Sort newest-first by timestamp.
        indexed.sort_by(|a, b| b.1.cmp(&a.1));
        // Indices beyond max_count are to be removed.
        let mut remove: Vec<usize> = indexed[max_count..].iter().map(|(i, _)| *i).collect();
        remove.sort();
        remove
    }

    /// Returns indices of entries older than `max_age_secs` relative to `now`.
    pub fn prune_by_age(entries: &[HistoryEntry], max_age_secs: u64, now: u64) -> Vec<usize> {
        let cutoff = now.saturating_sub(max_age_secs);
        entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.timestamp < cutoff)
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns indices to remove (oldest first) until total size is within
    /// `max_total_bytes`.
    pub fn prune_by_size(entries: &[HistoryEntry], max_total_bytes: u64) -> Vec<usize> {
        let total: u64 = entries.iter().map(|e| e.size_bytes).sum();
        if total <= max_total_bytes {
            return Vec::new();
        }

        // Sort by timestamp ascending (oldest first) to remove oldest.
        let mut indexed: Vec<(usize, u64, u64)> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (i, e.timestamp, e.size_bytes))
            .collect();
        indexed.sort_by_key(|&(_, ts, _)| ts);

        let mut excess = total - max_total_bytes;
        let mut remove = Vec::new();
        for (i, _, sz) in &indexed {
            if excess == 0 {
                break;
            }
            remove.push(*i);
            excess = excess.saturating_sub(*sz);
        }
        remove.sort();
        remove
    }

    /// Returns indices of entries to *keep* – the intersection of count and
    /// age constraints.
    pub fn entries_to_keep(
        entries: &[HistoryEntry],
        max_count: usize,
        max_age_secs: u64,
        now: u64,
    ) -> Vec<usize> {
        let remove_count: std::collections::HashSet<usize> =
            Self::prune_by_count(entries, max_count).into_iter().collect();
        let remove_age: std::collections::HashSet<usize> =
            Self::prune_by_age(entries, max_age_secs, now).into_iter().collect();
        (0..entries.len())
            .filter(|i| !remove_count.contains(i) && !remove_age.contains(i))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Restore – helpers for locating entries to restore
// ---------------------------------------------------------------------------

/// Helpers for locating entries suitable for restore operations.
pub struct LocalHistoryRestore;

impl LocalHistoryRestore {
    pub fn new() -> Self {
        Self
    }

    /// Find the entry whose timestamp is closest to `target_ts` without
    /// exceeding it (at-or-before).
    pub fn find_entry_at_timestamp(entries: &[HistoryEntry], target_ts: u64) -> Option<usize> {
        entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.timestamp <= target_ts)
            .max_by_key(|(_, e)| e.timestamp)
            .map(|(i, _)| i)
    }

    /// Find the first entry whose `content_hash` matches `hash`.
    pub fn find_entry_by_hash(entries: &[HistoryEntry], hash: &str) -> Option<usize> {
        entries
            .iter()
            .enumerate()
            .find(|(_, e)| e.content_hash == hash)
            .map(|(i, _)| i)
    }

    /// Find the entry whose timestamp is absolutely closest to `target_ts`.
    pub fn find_nearest(entries: &[HistoryEntry], target_ts: u64) -> Option<usize> {
        entries
            .iter()
            .enumerate()
            .min_by_key(|(_, e)| {
                if e.timestamp >= target_ts {
                    e.timestamp - target_ts
                } else {
                    target_ts - e.timestamp
                }
            })
            .map(|(i, _)| i)
    }
}

// ---------------------------------------------------------------------------
// Size tracker
// ---------------------------------------------------------------------------

/// Tracks cumulative file sizes across the history store.
pub struct HistorySizeTracker {
    pub sizes: HashMap<String, u64>,
}

impl HistorySizeTracker {
    pub fn new() -> Self {
        Self {
            sizes: HashMap::new(),
        }
    }

    pub fn record(&mut self, uri: &str, size_bytes: u64) {
        self.sizes.insert(uri.to_string(), size_bytes);
    }

    pub fn total_size(&self) -> u64 {
        self.sizes.values().sum()
    }

    pub fn largest_file(&self) -> Option<(&str, u64)> {
        self.sizes
            .iter()
            .max_by_key(|&(_, sz)| sz)
            .map(|(k, &v)| (k.as_str(), v))
    }

    pub fn files_over_threshold(&self, threshold: u64) -> Vec<(&str, u64)> {
        let mut result: Vec<(&str, u64)> = self
            .sizes
            .iter()
            .filter(|&(_, &sz)| sz > threshold)
            .map(|(k, &v)| (k.as_str(), v))
            .collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    pub fn file_count(&self) -> usize {
        self.sizes.len()
    }

    pub fn average_size(&self) -> u64 {
        if self.sizes.is_empty() {
            return 0;
        }
        self.total_size() / self.sizes.len() as u64
    }
}

// ---------------------------------------------------------------------------
// LocalHistoryEntryFormatter – formats history entries for display
// ---------------------------------------------------------------------------

/// Display format style for history entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryDisplayStyle {
    /// Short one-line summary.
    Compact,
    /// Multi-line detail view.
    Detailed,
    /// Machine-readable key=value pairs.
    KeyValue,
}

/// Formats history entries for display in the UI.
#[derive(Debug)]
pub struct LocalHistoryEntryFormatter {
    style: HistoryDisplayStyle,
    show_content_hash: bool,
    max_label_width: usize,
    time_format_24h: bool,
}

impl LocalHistoryEntryFormatter {
    pub fn new(style: HistoryDisplayStyle) -> Self {
        Self {
            style,
            show_content_hash: false,
            max_label_width: 40,
            time_format_24h: true,
        }
    }

    pub fn with_content_hash(mut self, show: bool) -> Self {
        self.show_content_hash = show;
        self
    }

    pub fn with_max_label_width(mut self, w: usize) -> Self {
        self.max_label_width = w;
        self
    }

    pub fn with_24h_format(mut self, v: bool) -> Self {
        self.time_format_24h = v;
        self
    }

    /// Format a single history entry.
    pub fn format_entry(&self, entry: &HistoryEntry) -> String {
        match self.style {
            HistoryDisplayStyle::Compact => self.format_compact(entry),
            HistoryDisplayStyle::Detailed => self.format_detailed(entry),
            HistoryDisplayStyle::KeyValue => self.format_kv(entry),
        }
    }

    fn format_compact(&self, entry: &HistoryEntry) -> String {
        let label = entry.label.as_deref().unwrap_or("(no label)");
        let label = self.truncate_label(label);
        let source_char = match entry.source {
            HistorySource::Auto => 'A',
            HistorySource::Manual => 'M',
            HistorySource::Undo => 'U',
        };
        format!("[{}] {} t={}", source_char, label, entry.timestamp)
    }

    fn format_detailed(&self, entry: &HistoryEntry) -> String {
        let mut lines = Vec::new();
        lines.push(format!("URI:    {}", entry.uri));
        lines.push(format!("Time:   {}", self.format_timestamp(entry.timestamp)));
        lines.push(format!("Source: {:?}", entry.source));
        if let Some(ref label) = entry.label {
            lines.push(format!("Label:  {}", self.truncate_label(label)));
        }
        if self.show_content_hash {
            lines.push(format!("Hash:   {}", entry.content_hash));
        }
        if let Some(ref content) = entry.content {
            let preview_len = content.len().min(80);
            lines.push(format!("Preview: {}…", &content[..preview_len]));
        }
        lines.join("\n")
    }

    fn format_kv(&self, entry: &HistoryEntry) -> String {
        let mut pairs = vec![
            format!("uri={}", entry.uri),
            format!("timestamp={}", entry.timestamp),
            format!("source={:?}", entry.source),
            format!("hash={}", entry.content_hash),
        ];
        if let Some(ref label) = entry.label {
            pairs.push(format!("label={}", label));
        }
        pairs.join(" ")
    }

    fn truncate_label<'a>(&self, label: &'a str) -> &'a str {
        if label.len() <= self.max_label_width {
            label
        } else {
            &label[..self.max_label_width]
        }
    }

    fn format_timestamp(&self, ts: u64) -> String {
        let hours = (ts / 3600) % 24;
        let minutes = (ts / 60) % 60;
        let seconds = ts % 60;
        if self.time_format_24h {
            format!("{hours:02}:{minutes:02}:{seconds:02}")
        } else {
            let period = if hours < 12 { "AM" } else { "PM" };
            let h12 = if hours == 0 { 12 } else if hours > 12 { hours - 12 } else { hours };
            format!("{h12}:{minutes:02}:{seconds:02} {period}")
        }
    }

    pub fn style(&self) -> HistoryDisplayStyle {
        self.style
    }

    /// Format multiple entries, separated by a blank line.
    pub fn format_entries(&self, entries: &[HistoryEntry]) -> String {
        entries
            .iter()
            .map(|e| self.format_entry(e))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

// ---------------------------------------------------------------------------
// LocalHistoryMergeTool – merges history entries from multiple sources
// ---------------------------------------------------------------------------

/// Strategy used when entries conflict during merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeConflictStrategy {
    /// Keep the newer entry based on timestamp.
    PreferNewer,
    /// Keep the older entry.
    PreferOlder,
    /// Keep both entries.
    KeepBoth,
}

/// Merges history entries from multiple sources into a unified timeline.
#[derive(Debug)]
pub struct LocalHistoryMergeTool {
    strategy: MergeConflictStrategy,
    dedup_by_hash: bool,
}

impl LocalHistoryMergeTool {
    pub fn new(strategy: MergeConflictStrategy) -> Self {
        Self { strategy, dedup_by_hash: true }
    }

    pub fn with_dedup(mut self, dedup: bool) -> Self {
        self.dedup_by_hash = dedup;
        self
    }

    /// Merge two sorted (by timestamp) entry lists into a single sorted list.
    pub fn merge(&self, a: &[HistoryEntry], b: &[HistoryEntry]) -> Vec<HistoryEntry> {
        let mut combined: Vec<HistoryEntry> = a.iter().chain(b.iter()).cloned().collect();
        combined.sort_by_key(|e| e.timestamp);

        if self.dedup_by_hash {
            self.dedup_entries(&mut combined);
        }
        combined
    }

    fn dedup_entries(&self, entries: &mut Vec<HistoryEntry>) {
        let mut seen = std::collections::HashSet::new();
        entries.retain(|e| {
            let key = format!("{}:{}", e.uri, e.content_hash);
            if seen.contains(&key) {
                match self.strategy {
                    MergeConflictStrategy::KeepBoth => true,
                    MergeConflictStrategy::PreferNewer => false,
                    MergeConflictStrategy::PreferOlder => false,
                }
            } else {
                seen.insert(key);
                true
            }
        });
    }

    /// Merge multiple sources at once.
    pub fn merge_all(&self, sources: &[Vec<HistoryEntry>]) -> Vec<HistoryEntry> {
        let mut result = Vec::new();
        for source in sources {
            result = self.merge(&result, source);
        }
        result
    }

    /// Count how many duplicates would be removed.
    pub fn count_duplicates(&self, entries: &[HistoryEntry]) -> usize {
        let mut seen = std::collections::HashSet::new();
        let mut dupes = 0;
        for e in entries {
            let key = format!("{}:{}", e.uri, e.content_hash);
            if !seen.insert(key) {
                dupes += 1;
            }
        }
        dupes
    }

    pub fn strategy(&self) -> MergeConflictStrategy {
        self.strategy
    }
}



// ─── LHist Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for history entries.
#[derive(Debug, Clone)]
pub struct LHistRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> LHistRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for LHistRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LHistRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── LHist Builder & Validator ─────────────────────────────

/// Builder for constructing history configurations.
#[derive(Debug, Clone)]
pub struct LHistBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl LHistBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<LHistCfg, LHistBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(LHistBuildErr { errors }); }
        Ok(LHistCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated history configuration.
#[derive(Debug, Clone)]
pub struct LHistCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl LHistCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &LHistCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for LHistCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LHistCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct LHistBuildErr { pub errors: Vec<String> }

impl fmt::Display for LHistBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LHistBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for LHistBuildErr {}



// ---------------------------------------------------------------------------
// local_history – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for local file history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YLocalHistoryHistoryEntryKind {
    Edit,
    Save,
    Revert,
    Create,
}

impl YLocalHistoryHistoryEntryKind {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Edit => 0,
            Self::Save => 1,
            Self::Revert => 2,
            Self::Create => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Edit => "Edit",
            Self::Save => "Save",
            Self::Revert => "Revert",
            Self::Create => "Create",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YLocalHistoryHistoryEntryKind] {
        &[
            YLocalHistoryHistoryEntryKind::Edit,
            YLocalHistoryHistoryEntryKind::Save,
            YLocalHistoryHistoryEntryKind::Revert,
            YLocalHistoryHistoryEntryKind::Create,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YLocalHistoryHistoryEntryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks history timeline data.
#[derive(Debug, Clone)]
pub struct YLocalHistoryHistoryTimeline {
    pub entries: Vec<(u64, String)>,
    pub max_entries: usize,
    pub file_path: String,
}

impl YLocalHistoryHistoryTimeline {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 0,
            file_path: String::new(),
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YLocalHistoryHistoryTimeline({}: {:?})", "entries", self.entries)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_local_history_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_local_history_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_local_history_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_local_history_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_local_history_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_local_history_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_local_history_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_local_history_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// local_history – Extended history compactor helpers
// ---------------------------------------------------------------------------

/// Priority levels for history compactor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZLocalHistoryPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZLocalHistoryPriority {
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
    pub fn all_asc() -> [ZLocalHistoryPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZLocalHistoryPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks history compactor data.
#[derive(Debug, Clone)]
pub struct ZLocalHistoryHistoryCompactor {
    pub versions: Vec<(u64, usize)>,
    pub max_versions: usize,
    pub bytes_saved: u64,
}

impl ZLocalHistoryHistoryCompactor {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            versions: Vec::new(),
            max_versions: 0,
            bytes_saved: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.versions.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.versions.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.versions.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZLocalHistoryHistoryCompactor[max_versions={:?}, bytes_saved={:?}]", self.max_versions, self.bytes_saved)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for history compactor.
pub fn z_local_history_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_local_history_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_local_history_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_local_history_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_local_history_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_local_history_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_local_history_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 116
// ---------------------------------------------------------------------------

/// Generic object pool `Xc116Pool<T>`.
pub struct Xc116Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc116Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc116PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc116Pool<T> {
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
    pub fn stats(&self) -> Xc116PoolStats {
        Xc116PoolStats {
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

impl<T> Default for Xc116Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc116Scheduler`.
pub struct Xc116Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc116Scheduler {
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

impl Default for Xc116Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_116 hash for the given byte slice.
pub fn xc_116_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_116 convention.
pub fn xc_116_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_20 deepening: state machine + event bus ---

/// States for the Xd20 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd20State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd20State {
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
pub struct Xd20Transition {
    pub from: Xd20State,
    pub to: Xd20State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd20StateMachine {
    current: Xd20State,
    history: Vec<Xd20Transition>,
    step_counter: usize,
}

impl Xd20StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd20State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd20State {
        self.current
    }

    pub fn history(&self) -> &[Xd20Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd20State) -> Result<Xd20State, String> {
        let allowed = match (self.current, target) {
            (Xd20State::Idle, Xd20State::Running) => true,
            (Xd20State::Running, Xd20State::Paused) => true,
            (Xd20State::Running, Xd20State::Done) => true,
            (Xd20State::Paused, Xd20State::Running) => true,
            (Xd20State::Paused, Xd20State::Done) => true,
            (Xd20State::Done, Xd20State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_20: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd20Transition {
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
            "Xd20SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd20State> {
        let prefix = "Xd20SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd20State::Idle),
            "Running" => Some(Xd20State::Running),
            "Paused" => Some(Xd20State::Paused),
            "Done" => Some(Xd20State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd20State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd20 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd20Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd20Event {
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

type Xd20HandlerFn = Box<dyn Fn(&Xd20Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd20EventBus {
    handlers: Vec<(usize, Option<String>, Xd20HandlerFn)>,
    next_id: usize,
    published: Vec<Xd20Event>,
}

impl Xd20EventBus {
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
        F: Fn(&Xd20Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd20Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd20Event) {
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

    pub fn published_events(&self) -> &[Xd20Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #18
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf18Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf18TrieNode {
    children: std::collections::HashMap<char, Xf18TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf18Trie {
    root: Xf18TrieNode,
    count: usize,
}

impl Xf18Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf18TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf18TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf18TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf18BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf18BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 115).
pub struct Xh115SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh115SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 157 as u64,
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

/// A compact bit set supporting boolean operations (variant 115).
pub struct Xh115BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh115BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 115).
pub struct Xi115Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi115Deque<T> {
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
pub struct Xi115Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi115Interval {
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

/// A simple interval tree (variant 115).
pub struct Xi115IntervalTree {
    xi_intervals: Vec<Xi115Interval>,
}

impl Xi115IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi115Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi115Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi115Interval) -> Vec<&Xi115Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi115Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi115Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi115Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi115Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi115Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi115Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 115) ---

/// Disjoint set / union-find for crate 115.
pub struct Xj115UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj115UnionFind {
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

const XJ115_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 115.
pub struct Xj115BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj115BTreeNode<K, V>>>,
    len: usize,
}

struct Xj115BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj115BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj115BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ115_BTREE_ORDER - 1
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
        let mid = XJ115_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj115BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj115BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj115BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj115BTreeNode::xj_new_leaf();
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


// --- xk_115 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk115SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk115SegmentTree {
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
pub struct Xk115DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk115DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_115).
#[derive(Debug, Clone)]
pub struct Xl115Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl115Rope {
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

/// Suffix array for efficient string searching (xl_115).
#[derive(Debug, Clone)]
pub struct Xl115SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl115SuffixArray {
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


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm115MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm115MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm115Tokenizer {
    text: String,
}

impl Xm115Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_retrieve_history() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "abc123", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "def456", HistorySource::Manual);
        let history = svc.get_history("file:///a.rs");
        assert_eq!(history.len(), 2);
        // Most recent first.
        assert_eq!(history[0].content_hash, "def456");
    }

    #[test]
    fn prune_limits_entries() {
        let mut svc = LocalHistoryService::new(2);
        svc.add_entry("file:///b.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///b.rs", "h2", HistorySource::Auto);
        svc.add_entry("file:///b.rs", "h3", HistorySource::Auto);
        assert_eq!(svc.get_history("file:///b.rs").len(), 2);
        // Oldest (h1) should have been pruned.
        assert!(svc.entries.iter().all(|e| e.content_hash != "h1"));
    }

    #[test]
    fn label_entry_sets_label() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///c.rs", "hash", HistorySource::Undo);
        svc.label_entry("file:///c.rs", 1, "before refactor");
        let history = svc.get_history("file:///c.rs");
        assert_eq!(history[0].label.as_deref(), Some("before refactor"));
    }

    #[test]
    fn get_entry_by_uri_and_timestamp() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        assert!(svc.get_entry("file:///a.rs", 1).is_some());
        assert!(svc.get_entry("file:///a.rs", 99).is_none());
    }

    #[test]
    fn remove_entry_by_uri_and_timestamp() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        assert!(svc.remove_entry("file:///a.rs", 1));
        assert_eq!(svc.entry_count(), 0);
        assert!(!svc.remove_entry("file:///a.rs", 1));
    }

    #[test]
    fn get_unique_uris() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///b.rs", "h2", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h3", HistorySource::Manual);
        let uris = svc.get_unique_uris();
        assert_eq!(uris.len(), 2);
    }

    #[test]
    fn total_size_and_size_for_uri() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///b.rs", "h2", HistorySource::Auto);
        // Manually set sizes for testing
        svc.entries[0].size_bytes = 100;
        svc.entries[1].size_bytes = 200;
        assert_eq!(svc.total_size_bytes(), 300);
        assert_eq!(svc.size_for_uri("file:///a.rs"), 100);
    }

    #[test]
    fn prune_by_age() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Auto);
        // Entries have timestamps 1 and 2
        svc.prune_by_age(1, 3); // cutoff = 2, keep timestamp >= 2
        assert_eq!(svc.entry_count(), 1);
        assert_eq!(svc.entries[0].content_hash, "h2");
    }

    #[test]
    fn get_stats() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Manual);
        svc.add_entry("file:///b.rs", "h3", HistorySource::Auto);
        let stats = svc.get_stats();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.unique_files, 2);
        assert_eq!(*stats.entries_per_source.get("Auto").unwrap(), 2);
        assert_eq!(*stats.entries_per_source.get("Manual").unwrap(), 1);
    }

    #[test]
    fn history_entry_content_field() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.entries[0].content = Some("fn main() {}".to_string());
        svc.entries[0].size_bytes = 12;
        let entry = svc.get_entry("file:///a.rs", 1).unwrap();
        assert_eq!(entry.content.as_deref(), Some("fn main() {}"));
        assert_eq!(entry.size_bytes, 12);
    }

    #[test]
    fn default_storage_provider() {
        struct TestStorage;
        impl HistoryStorageProvider for TestStorage {}
        let s = TestStorage;
        assert!(s.save(&[]).is_ok());
        assert!(s.load().unwrap().is_empty());
    }

    #[test]
    fn custom_storage_provider() {
        struct MemStorage;
        impl HistoryStorageProvider for MemStorage {
            fn load(&self) -> Result<Vec<HistoryEntry>, String> {
                Ok(vec![HistoryEntry {
                    uri: "file:///x.rs".to_string(),
                    timestamp: 1,
                    content_hash: "abc".to_string(),
                    label: None,
                    source: HistorySource::Auto,
                    content: None,
                    size_bytes: 0,
                }])
            }
        }
        let s = MemStorage;
        assert_eq!(s.load().unwrap().len(), 1);
    }

    #[test]
    fn prune_by_age_keeps_all_when_young() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Auto);
        svc.prune_by_age(100, 3); // cutoff = 0, all kept
        assert_eq!(svc.entry_count(), 2);
    }

    #[test]
    fn clear_all_empties_entries() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///d.rs", "x", HistorySource::Auto);
        svc.clear_all();
        assert_eq!(svc.entry_count(), 0);
    }

    #[test]
    fn entry_is_auto_and_is_manual() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Manual);
        assert!(svc.entries[0].is_auto());
        assert!(!svc.entries[0].is_manual());
        assert!(svc.entries[1].is_manual());
        assert!(!svc.entries[1].is_auto());
    }

    #[test]
    fn entry_has_label_and_has_content() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        assert!(!svc.entries[0].has_label());
        assert!(!svc.entries[0].has_content());
        svc.entries[0].label = Some("snapshot".to_string());
        svc.entries[0].content = Some("data".to_string());
        assert!(svc.entries[0].has_label());
        assert!(svc.entries[0].has_content());
    }

    #[test]
    fn entry_age() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        // timestamp is 1
        assert_eq!(svc.entries[0].age(10), 9);
        assert_eq!(svc.entries[0].age(1), 0);
        // saturating: current_time < timestamp
        assert_eq!(svc.entries[0].age(0), 0);
    }

    #[test]
    fn history_source_display() {
        assert_eq!(format!("{}", HistorySource::Auto), "auto");
        assert_eq!(format!("{}", HistorySource::Manual), "manual");
        assert_eq!(format!("{}", HistorySource::Undo), "undo");
    }

    #[test]
    fn history_entry_display() {
        let entry = HistoryEntry {
            uri: "file:///x.rs".to_string(),
            timestamp: 42,
            content_hash: "abc".to_string(),
            label: None,
            source: HistorySource::Manual,
            content: None,
            size_bytes: 0,
        };
        assert_eq!(format!("{}", entry), "[manual] file:///x.rs @42 (abc)");
    }

    #[test]
    fn compute_diff_detects_changes() {
        let a = HistoryEntry {
            uri: "file:///a.rs".to_string(),
            timestamp: 1,
            content_hash: "aaa".to_string(),
            label: None,
            source: HistorySource::Auto,
            content: None,
            size_bytes: 100,
        };
        let b = HistoryEntry {
            uri: "file:///a.rs".to_string(),
            timestamp: 2,
            content_hash: "bbb".to_string(),
            label: None,
            source: HistorySource::Auto,
            content: None,
            size_bytes: 150,
        };
        let diff = compute_diff(&a, &b);
        assert!(diff.hash_changed);
        assert_eq!(diff.size_delta, 50);
        assert_eq!(diff.from_timestamp, 1);
        assert_eq!(diff.to_timestamp, 2);
    }

    #[test]
    fn compute_diff_no_change() {
        let a = HistoryEntry {
            uri: "file:///a.rs".to_string(),
            timestamp: 1,
            content_hash: "same".to_string(),
            label: None,
            source: HistorySource::Auto,
            content: None,
            size_bytes: 100,
        };
        let b = HistoryEntry {
            uri: "file:///a.rs".to_string(),
            timestamp: 2,
            content_hash: "same".to_string(),
            label: None,
            source: HistorySource::Auto,
            content: None,
            size_bytes: 100,
        };
        let diff = compute_diff(&a, &b);
        assert!(!diff.hash_changed);
        assert_eq!(diff.size_delta, 0);
    }

    #[test]
    fn get_latest_entry_returns_most_recent() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Manual);
        svc.add_entry("file:///b.rs", "h3", HistorySource::Auto);
        let latest = svc.get_latest_entry("file:///a.rs").unwrap();
        assert_eq!(latest.content_hash, "h2");
        assert!(svc.get_latest_entry("file:///missing.rs").is_none());
    }

    #[test]
    fn get_entries_by_source() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Manual);
        svc.add_entry("file:///b.rs", "h3", HistorySource::Auto);
        let autos = svc.get_entries_by_source(HistorySource::Auto);
        assert_eq!(autos.len(), 2);
        let manuals = svc.get_entries_by_source(HistorySource::Manual);
        assert_eq!(manuals.len(), 1);
        let undos = svc.get_entries_by_source(HistorySource::Undo);
        assert!(undos.is_empty());
    }

    #[test]
    fn search_by_label_finds_matching() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Manual);
        svc.label_entry("file:///a.rs", 1, "before refactor");
        svc.label_entry("file:///a.rs", 2, "after refactor");
        let results = svc.search_by_label("refactor");
        assert_eq!(results.len(), 2);
        let results = svc.search_by_label("before");
        assert_eq!(results.len(), 1);
        let results = svc.search_by_label("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn compact_removes_duplicate_hashes() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "same_hash", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "same_hash", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "different", HistorySource::Auto);
        assert_eq!(svc.entry_count(), 3);
        svc.compact();
        assert_eq!(svc.entry_count(), 2);
        // The kept entry for "same_hash" should be the one with the higher timestamp
        let kept = svc
            .entries
            .iter()
            .find(|e| e.content_hash == "same_hash")
            .unwrap();
        assert_eq!(kept.timestamp, 2);
    }

    #[test]
    fn filter_entries_by_source() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Manual);
        let filter = HistoryFilter {
            source: Some(HistorySource::Auto),
            ..Default::default()
        };
        let results = svc.filter_entries(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content_hash, "h1");
    }

    #[test]
    fn filter_entries_by_timestamp_range() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h3", HistorySource::Auto);
        // timestamps are 1, 2, 3
        let filter = HistoryFilter {
            min_timestamp: Some(2),
            max_timestamp: Some(2),
            ..Default::default()
        };
        let results = svc.filter_entries(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content_hash, "h2");
    }

    #[test]
    fn filter_entries_by_label() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Auto);
        svc.label_entry("file:///a.rs", 1, "important save");
        let filter = HistoryFilter {
            label_contains: Some("important".to_string()),
            ..Default::default()
        };
        let results = svc.filter_entries(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content_hash, "h1");
    }

    #[test]
    fn history_stats_partial_eq() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        let stats1 = svc.get_stats();
        let stats2 = svc.get_stats();
        assert_eq!(stats1, stats2);
    }

    #[test]
    fn prune_combined_removes_old_then_limits() {
        let mut svc = LocalHistoryService::new(2);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h3", HistorySource::Auto);
        // timestamps 1, 2, 3; max_entries_per_file=2 already pruned h1
        svc.add_entry("file:///b.rs", "b1", HistorySource::Auto);
        svc.prune_combined(1, 4); // cutoff=3, keeps timestamp >= 3
        let a_history = svc.get_history("file:///a.rs");
        assert_eq!(a_history.len(), 1);
        assert_eq!(a_history[0].content_hash, "h3");
    }

    #[test]
    fn estimate_storage_size_includes_content() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.entries[0].size_bytes = 50;
        svc.entries[0].content = Some("hello world".to_string());
        let size = svc.estimate_storage_size();
        assert_eq!(size, 50 + 11); // 11 bytes for "hello world"
    }

    #[test]
    fn search_by_content_finds_matching() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Auto);
        svc.entries[0].content = Some("fn main() {}".to_string());
        svc.entries[1].content = Some("fn helper() {}".to_string());
        let results = svc.search_by_content("main");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content_hash, "h1");
    }

    #[test]
    fn search_by_content_no_content() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        let results = svc.search_by_content("anything");
        assert!(results.is_empty());
    }

    #[test]
    fn compute_consecutive_diffs_for_uri() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h3", HistorySource::Auto);
        svc.entries[0].size_bytes = 100;
        svc.entries[1].size_bytes = 150;
        svc.entries[2].size_bytes = 120;
        let diffs = svc.compute_consecutive_diffs("file:///a.rs");
        assert_eq!(diffs.len(), 2);
        assert!(diffs[0].hash_changed);
        assert_eq!(diffs[0].size_delta, 50);
        assert_eq!(diffs[1].size_delta, -30);
    }

    #[test]
    fn compute_consecutive_diffs_single_entry() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        let diffs = svc.compute_consecutive_diffs("file:///a.rs");
        assert!(diffs.is_empty());
    }

    #[test]
    fn gc_config_defaults() {
        let cfg = GarbageCollectorConfig::default();
        assert_eq!(cfg.max_age_seconds, 7 * 24 * 60 * 60);
        assert_eq!(cfg.max_entries_per_file, 50);
        assert_eq!(cfg.max_total_size_bytes, 50 * 1024 * 1024);
    }

    #[test]
    fn gc_prunes_old_entries() {
        let mut svc = LocalHistoryService::new(100);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h3", HistorySource::Auto);
        // timestamps: 1, 2, 3
        let gc = LocalHistoryGarbageCollector::new(GarbageCollectorConfig {
            max_age_seconds: 1,
            max_entries_per_file: 100,
            max_total_size_bytes: u64::MAX,
        });
        let result = gc.collect(&mut svc, 4); // cutoff = 3, keeps >= 3
        assert_eq!(result.entries_removed, 2);
        assert_eq!(result.entries_after, 1);
    }

    #[test]
    fn gc_enforces_per_file_limit() {
        let mut svc = LocalHistoryService::new(100);
        for i in 0..10 {
            svc.add_entry("file:///a.rs", &format!("h{i}"), HistorySource::Auto);
        }
        let gc = LocalHistoryGarbageCollector::new(GarbageCollectorConfig {
            max_age_seconds: u64::MAX,
            max_entries_per_file: 3,
            max_total_size_bytes: u64::MAX,
        });
        let result = gc.collect(&mut svc, 100);
        assert_eq!(result.entries_after, 3);
    }

    #[test]
    fn gc_enforces_total_size() {
        let mut svc = LocalHistoryService::new(100);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Auto);
        svc.entries[0].size_bytes = 500;
        svc.entries[1].size_bytes = 500;
        let gc = LocalHistoryGarbageCollector::new(GarbageCollectorConfig {
            max_age_seconds: u64::MAX,
            max_entries_per_file: 100,
            max_total_size_bytes: 600,
        });
        let result = gc.collect(&mut svc, 100);
        assert!(svc.total_size_bytes() <= 600);
        assert!(result.entries_removed > 0);
    }

    #[test]
    fn gc_needs_collection() {
        let mut svc = LocalHistoryService::new(100);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        let gc = LocalHistoryGarbageCollector::new(GarbageCollectorConfig {
            max_age_seconds: 5,
            max_entries_per_file: 100,
            max_total_size_bytes: u64::MAX,
        });
        assert!(!gc.needs_collection(&svc, 3)); // age=2, max=5
        assert!(gc.needs_collection(&svc, 10)); // age=9, max=5
    }

    #[test]
    fn history_diff_detects_modifications() {
        let old = "line1\nline2\nline3";
        let new = "line1\nmodified\nline3";
        let diffs = history_diff(old, new);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].line_number, 2);
        assert_eq!(diffs[0].kind, LineDiffKind::Modified);
        assert_eq!(diffs[0].content, "modified");
    }

    #[test]
    fn history_diff_detects_additions() {
        let old = "line1";
        let new = "line1\nnew_line";
        let diffs = history_diff(old, new);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].kind, LineDiffKind::Added);
    }

    #[test]
    fn history_diff_detects_removals() {
        let old = "line1\nline2";
        let new = "line1";
        let diffs = history_diff(old, new);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].kind, LineDiffKind::Removed);
    }

    #[test]
    fn history_diff_identical() {
        let content = "same\ncontent";
        assert!(history_diff(content, content).is_empty());
    }

    #[test]
    fn history_diff_count_works() {
        let old = "a\nb\nc";
        let new = "a\nmodified\nc\nnew";
        assert_eq!(history_diff_count(old, new), 2); // modified + added
    }

    #[test]
    fn line_diff_kind_display() {
        assert_eq!(format!("{}", LineDiffKind::Added), "+");
        assert_eq!(format!("{}", LineDiffKind::Removed), "-");
        assert_eq!(format!("{}", LineDiffKind::Modified), "~");
    }

    #[test]
    fn entry_age_secs_and_is_recent() {
        let entry = HistoryEntry {
            uri: "file:///a.rs".to_string(),
            timestamp: 100,
            content_hash: "abc".to_string(),
            label: None,
            source: HistorySource::Auto,
            content: None,
            size_bytes: 0,
        };
        assert_eq!(entry.age_secs(150), 50);
        assert_eq!(entry.age_secs(50), 0);
        assert!(entry.is_recent(150, 50));
        assert!(!entry.is_recent(200, 50));
        assert!(entry.is_recent(100, 0));
    }

    #[test]
    fn diff_summary_display_and_total() {
        let summary = DiffSummary {
            additions: 3,
            deletions: 1,
            modifications: 2,
        };
        assert_eq!(summary.total(), 6);
        assert!(!summary.is_empty());
        assert_eq!(format!("{}", summary), "+3 -1 ~2");

        let empty = DiffSummary {
            additions: 0,
            deletions: 0,
            modifications: 0,
        };
        assert!(empty.is_empty());
    }

    #[test]
    fn compute_diff_summary_counts() {
        let old = "line1\nline2\nline3";
        let new = "line1\nchanged\nline3\nnew_line";
        let summary = compute_diff_summary(old, new);
        assert_eq!(summary.modifications, 1);
        assert_eq!(summary.additions, 1);
        assert_eq!(summary.deletions, 0);
        assert_eq!(summary.total(), 2);
    }

    #[test]
    fn history_diff_is_empty_and_time_span() {
        let diff = HistoryDiff {
            uri: "file:///a.rs".to_string(),
            from_timestamp: 10,
            to_timestamp: 20,
            hash_changed: false,
            size_delta: 0,
        };
        assert!(diff.is_empty());
        assert_eq!(diff.time_span(), 10);

        let diff2 = HistoryDiff {
            uri: "file:///a.rs".to_string(),
            from_timestamp: 5,
            to_timestamp: 15,
            hash_changed: true,
            size_delta: 42,
        };
        assert!(!diff2.is_empty());
        assert_eq!(diff2.time_span(), 10);
    }

    #[test]
    fn history_filter_constructors_and_matches() {
        let filter = HistoryFilter::by_source(HistorySource::Manual);
        assert_eq!(filter.source, Some(HistorySource::Manual));
        assert!(filter.min_timestamp.is_none());

        let range = HistoryFilter::by_time_range(5, 10);
        assert_eq!(range.min_timestamp, Some(5));
        assert_eq!(range.max_timestamp, Some(10));

        let entry = HistoryEntry {
            uri: "file:///a.rs".to_string(),
            timestamp: 7,
            content_hash: "h".to_string(),
            label: None,
            source: HistorySource::Auto,
            content: None,
            size_bytes: 0,
        };
        assert!(range.matches(&entry));
        assert!(!filter.matches(&entry));
    }

    #[test]
    fn service_oldest_newest_and_entries_for_file() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///b.rs", "h2", HistorySource::Manual);
        svc.add_entry("file:///a.rs", "h3", HistorySource::Undo);

        assert_eq!(svc.oldest_entry().unwrap().content_hash, "h1");
        assert_eq!(svc.newest_entry().unwrap().content_hash, "h3");
        assert_eq!(svc.entries_for_file("file:///a.rs").len(), 2);
        assert_eq!(svc.entries_for_file("file:///b.rs").len(), 1);
        assert!(svc.entries_for_file("file:///missing.rs").is_empty());

        assert!(LocalHistoryService::new(10).oldest_entry().is_none());
        assert!(LocalHistoryService::new(10).newest_entry().is_none());
    }

    #[test]
    fn service_stats_and_iterator() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h3", HistorySource::Manual);
        svc.entries[0].size_bytes = 100;
        svc.entries[1].size_bytes = 200;
        svc.entries[2].size_bytes = 300;

        assert_eq!(svc.total_diffs("file:///a.rs"), 2);
        assert_eq!(svc.total_diffs("file:///missing.rs"), 0);
        assert!((svc.average_entry_size() - 200.0).abs() < f64::EPSILON);
        assert_eq!(LocalHistoryService::new(5).average_entry_size(), 0.0);

        let collected: Vec<&HistoryEntry> = (&svc).into_iter().collect();
        assert_eq!(collected.len(), 3);
    }

    #[test]
    fn gc_result_extensions_and_display() {
        let result = GarbageCollectionResult {
            entries_before: 10,
            entries_after: 6,
            entries_removed: 4,
            bytes_freed: 2048,
        };
        assert!(result.had_effect());
        assert!((result.removal_ratio() - 0.4).abs() < f64::EPSILON);
        assert_eq!(
            format!("{}", result),
            "GC: removed 4 of 10 entries, freed 2048 bytes"
        );

        let no_effect = GarbageCollectionResult {
            entries_before: 5,
            entries_after: 5,
            entries_removed: 0,
            bytes_freed: 0,
        };
        assert!(!no_effect.had_effect());
        assert_eq!(no_effect.removal_ratio(), 0.0);

        let empty = GarbageCollectionResult {
            entries_before: 0,
            entries_after: 0,
            entries_removed: 0,
            bytes_freed: 0,
        };
        assert_eq!(empty.removal_ratio(), 0.0);
    }

    #[test]
    fn recent_entries_filters_by_threshold() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h3", HistorySource::Auto);
        let recent = svc.recent_entries(4, 1);
        assert_eq!(recent.len(), 1);
        let recent_all = svc.recent_entries(4, 10);
        assert_eq!(recent_all.len(), 3);
    }

    // -----------------------------------------------------------------------
    // New tests for export/import, scheduling, file stats, dedup, compare
    // -----------------------------------------------------------------------

    #[test]
    fn export_and_import_round_trip() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "h2", HistorySource::Manual);
        svc.add_entry("file:///b.rs", "h3", HistorySource::Undo);
        let exported = svc.export_entries();
        assert_eq!(exported.len(), 3);
        assert_eq!(exported[0].source, "auto");
        assert_eq!(exported[1].source, "manual");
        assert_eq!(exported[2].source, "undo");

        // Import into a fresh service
        let mut svc2 = LocalHistoryService::new(10);
        let count = svc2.import_entries(exported).unwrap();
        assert_eq!(count, 3);
        assert_eq!(svc2.entry_count(), 3);

        // Re-importing the same entries yields 0 new imports (dedup by uri+ts)
        let exported2 = svc.export_entries();
        let count2 = svc2.import_entries(exported2).unwrap();
        assert_eq!(count2, 0);
    }

    #[test]
    fn exported_entry_parse_source_invalid() {
        let bad = ExportedEntry {
            uri: "file:///x.rs".to_string(),
            timestamp: 1,
            content_hash: "h".to_string(),
            label: None,
            source: "bogus".to_string(),
            content: None,
            size_bytes: 0,
        };
        assert!(bad.parse_source().is_err());
        assert!(bad.into_entry().is_err());
    }

    #[test]
    fn snapshot_schedule_should_snapshot() {
        let svc = LocalHistoryService::new(10);
        let schedule = SnapshotSchedule {
            min_interval_secs: 60,
            only_on_change: true,
        };

        // No entries yet — always snapshot
        assert!(schedule.should_snapshot(&svc, "file:///a.rs", 100, "hash1"));
        assert_eq!(schedule.time_until_next(&svc, "file:///a.rs", 100), 0);
    }

    #[test]
    fn snapshot_schedule_respects_interval_and_change() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "hash1", HistorySource::Auto);

        let schedule = SnapshotSchedule {
            min_interval_secs: 60,
            only_on_change: true,
        };

        // Entry has timestamp=1. At current_time=30, elapsed=29 < 60
        assert!(!schedule.should_snapshot(&svc, "file:///a.rs", 30, "hash2"));
        assert_eq!(schedule.time_until_next(&svc, "file:///a.rs", 30), 31);

        // Enough time but same hash — no snapshot (only_on_change=true)
        assert!(!schedule.should_snapshot(&svc, "file:///a.rs", 100, "hash1"));

        // Enough time and different hash — snapshot
        assert!(schedule.should_snapshot(&svc, "file:///a.rs", 100, "hash2"));

        // only_on_change=false: same hash but enough time — snapshot
        let schedule2 = SnapshotSchedule {
            min_interval_secs: 60,
            only_on_change: false,
        };
        assert!(schedule2.should_snapshot(&svc, "file:///a.rs", 100, "hash1"));
    }

    #[test]
    fn file_stats_and_all_file_stats() {
        let mut svc = LocalHistoryService::new(10);
        svc.entries.push(HistoryEntry {
            uri: "file:///a.rs".to_string(),
            timestamp: 10,
            content_hash: "h1".to_string(),
            label: None,
            source: HistorySource::Auto,
            content: None,
            size_bytes: 100,
        });
        svc.entries.push(HistoryEntry {
            uri: "file:///a.rs".to_string(),
            timestamp: 20,
            content_hash: "h2".to_string(),
            label: None,
            source: HistorySource::Manual,
            content: None,
            size_bytes: 200,
        });
        svc.entries.push(HistoryEntry {
            uri: "file:///b.rs".to_string(),
            timestamp: 15,
            content_hash: "h1".to_string(),
            label: None,
            source: HistorySource::Auto,
            content: None,
            size_bytes: 50,
        });

        let stats_a = svc.file_stats("file:///a.rs").unwrap();
        assert_eq!(stats_a.entry_count, 2);
        assert_eq!(stats_a.total_size, 300);
        assert_eq!(stats_a.oldest_timestamp, 10);
        assert_eq!(stats_a.newest_timestamp, 20);
        assert_eq!(stats_a.unique_hashes, 2);

        assert!(svc.file_stats("file:///nope.rs").is_none());

        let all = svc.all_file_stats();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn search_by_filename_case_insensitive() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///SRC/Main.rs", "h1", HistorySource::Auto);
        svc.add_entry("file:///src/util.rs", "h2", HistorySource::Auto);
        svc.add_entry("file:///lib/other.py", "h3", HistorySource::Auto);

        let results = svc.search_by_filename("src");
        assert_eq!(results.len(), 2);
        let results2 = svc.search_by_filename("MAIN");
        assert_eq!(results2.len(), 1);
        let results3 = svc.search_by_filename("nope");
        assert!(results3.is_empty());
    }

    #[test]
    fn deduplicate_removes_duplicate_hashes() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///a.rs", "same_hash", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "same_hash", HistorySource::Auto);
        svc.add_entry("file:///a.rs", "different", HistorySource::Auto);
        assert_eq!(svc.entry_count(), 3);

        let removed = svc.deduplicate();
        assert_eq!(removed, 1);
        assert_eq!(svc.entry_count(), 2);

        // The kept entry for "same_hash" should be the one with the higher timestamp
        let kept = svc
            .entries
            .iter()
            .find(|e| e.content_hash == "same_hash")
            .unwrap();
        assert_eq!(kept.timestamp, 2);
    }

    #[test]
    fn compare_entries_with_content() {
        let mut svc = LocalHistoryService::new(10);
        svc.entries.push(HistoryEntry {
            uri: "file:///a.rs".to_string(),
            timestamp: 1,
            content_hash: "h1".to_string(),
            label: None,
            source: HistorySource::Auto,
            content: Some("line1\nline2".to_string()),
            size_bytes: 10,
        });
        svc.entries.push(HistoryEntry {
            uri: "file:///a.rs".to_string(),
            timestamp: 2,
            content_hash: "h2".to_string(),
            label: None,
            source: HistorySource::Auto,
            content: Some("line1\nchanged".to_string()),
            size_bytes: 12,
        });

        let cmp = svc.compare_entries("file:///a.rs", 1, 2).unwrap();
        assert!(cmp.hash_changed);
        assert_eq!(cmp.size_delta, 2);
        let diff = cmp.content_diff.unwrap();
        assert_eq!(diff.modifications, 1);
        assert_eq!(diff.additions, 0);
        assert_eq!(diff.deletions, 0);

        // Missing entry returns None
        assert!(svc.compare_entries("file:///a.rs", 1, 99).is_none());
    }

    // -----------------------------------------------------------------------
    // LocalHistoryDiff tests
    // -----------------------------------------------------------------------

    #[test]
    fn diff_no_changes() {
        let d = LocalHistoryDiff::new("hello\nworld", "hello\nworld");
        assert!(!d.has_changes());
        assert_eq!(d.added_lines(), 0);
        assert_eq!(d.removed_lines(), 0);
        assert!(d.changed_line_numbers().is_empty());
        assert_eq!(d.summary(), "No changes");
    }

    #[test]
    fn diff_with_additions_and_removals() {
        let d = LocalHistoryDiff::new("aaa\nbbb\nccc", "aaa\nxxx\nccc\nddd");
        assert!(d.has_changes());
        assert_eq!(d.added_lines(), 2); // bbb→xxx, +ddd
        assert_eq!(d.removed_lines(), 1); // bbb→xxx
        assert_eq!(d.changed_line_numbers(), vec![2, 4]);
    }

    #[test]
    fn diff_unified_output() {
        let d = LocalHistoryDiff::new("a\nb", "a\nc");
        let out = d.as_unified_diff();
        assert!(out.contains("-b\n"));
        assert!(out.contains("+c\n"));
        assert!(out.contains(" a\n"));
    }

    // -----------------------------------------------------------------------
    // LocalHistoryPrune tests
    // -----------------------------------------------------------------------

    fn make_entry(ts: u64, size: u64) -> HistoryEntry {
        HistoryEntry {
            uri: "file:///test".to_string(),
            timestamp: ts,
            content_hash: format!("h{}", ts),
            label: None,
            source: HistorySource::Auto,
            content: None,
            size_bytes: size,
        }
    }

    #[test]
    fn prune_by_count_keeps_newest() {
        let entries = vec![make_entry(1, 10), make_entry(3, 10), make_entry(2, 10)];
        let remove = LocalHistoryPrune::prune_by_count(&entries, 2);
        assert_eq!(remove, vec![0]); // ts=1 is oldest
    }

    #[test]
    fn prune_by_count_no_op_when_under() {
        let entries = vec![make_entry(1, 10)];
        assert!(LocalHistoryPrune::prune_by_count(&entries, 5).is_empty());
    }

    #[test]
    fn prune_by_age_removes_old() {
        let entries = vec![make_entry(10, 5), make_entry(50, 5), make_entry(90, 5)];
        let remove = LocalHistoryPrune::prune_by_age(&entries, 50, 100);
        // cutoff = 50, entries with ts < 50 are removed
        assert_eq!(remove, vec![0]);
    }

    #[test]
    fn prune_by_size_trims_oldest() {
        let entries = vec![make_entry(1, 100), make_entry(2, 100), make_entry(3, 100)];
        let remove = LocalHistoryPrune::prune_by_size(&entries, 200);
        assert_eq!(remove, vec![0]); // oldest removed first
    }

    #[test]
    fn entries_to_keep_intersection() {
        let entries = vec![
            make_entry(10, 5),
            make_entry(50, 5),
            make_entry(90, 5),
            make_entry(95, 5),
        ];
        let keep = LocalHistoryPrune::entries_to_keep(&entries, 3, 50, 100);
        // count keeps indices 1,2,3 (newest 3). age keeps 1,2,3 (ts>=50).
        assert_eq!(keep, vec![1, 2, 3]);
    }

    // -----------------------------------------------------------------------
    // LocalHistoryRestore tests
    // -----------------------------------------------------------------------

    #[test]
    fn restore_find_at_timestamp() {
        let entries = vec![make_entry(10, 0), make_entry(20, 0), make_entry(30, 0)];
        assert_eq!(LocalHistoryRestore::find_entry_at_timestamp(&entries, 25), Some(1));
        assert_eq!(LocalHistoryRestore::find_entry_at_timestamp(&entries, 30), Some(2));
        assert_eq!(LocalHistoryRestore::find_entry_at_timestamp(&entries, 5), None);
    }

    #[test]
    fn restore_find_by_hash() {
        let entries = vec![make_entry(1, 0), make_entry(2, 0)];
        assert_eq!(LocalHistoryRestore::find_entry_by_hash(&entries, "h2"), Some(1));
        assert_eq!(LocalHistoryRestore::find_entry_by_hash(&entries, "missing"), None);
    }

    #[test]
    fn restore_find_nearest() {
        let entries = vec![make_entry(10, 0), make_entry(20, 0), make_entry(30, 0)];
        assert_eq!(LocalHistoryRestore::find_nearest(&entries, 18), Some(1)); // 20 is closer
        assert_eq!(LocalHistoryRestore::find_nearest(&entries, 26), Some(2)); // 30 is closer
    }

    // -----------------------------------------------------------------------
    // HistorySizeTracker tests
    // -----------------------------------------------------------------------

    #[test]
    fn size_tracker_basics() {
        let mut t = HistorySizeTracker::new();
        t.record("a.rs", 100);
        t.record("b.rs", 300);
        t.record("c.rs", 50);
        assert_eq!(t.file_count(), 3);
        assert_eq!(t.total_size(), 450);
        assert_eq!(t.average_size(), 150);
        let (name, sz) = t.largest_file().unwrap();
        assert_eq!(name, "b.rs");
        assert_eq!(sz, 300);
    }

    #[test]
    fn size_tracker_files_over_threshold() {
        let mut t = HistorySizeTracker::new();
        t.record("small.rs", 10);
        t.record("big.rs", 500);
        t.record("med.rs", 200);
        let over = t.files_over_threshold(100);
        assert_eq!(over.len(), 2);
        // Sorted descending by size.
        assert_eq!(over[0].0, "big.rs");
        assert_eq!(over[1].0, "med.rs");
    }

    fn make_hist_entry(uri: &str, ts: u64, hash: &str, source: HistorySource) -> HistoryEntry {
        HistoryEntry {
            uri: uri.to_string(),
            timestamp: ts,
            content_hash: hash.to_string(),
            label: Some(format!("entry-{ts}")),
            source,
            content: None,
            size_bytes: 0,
        }
    }

    #[test]
    fn formatter_compact() {
        let e = make_hist_entry("file.rs", 100, "abc", HistorySource::Auto);
        let fmt = LocalHistoryEntryFormatter::new(HistoryDisplayStyle::Compact);
        let s = fmt.format_entry(&e);
        assert!(s.starts_with("[A]"));
        assert!(s.contains("entry-100"));
    }

    #[test]
    fn formatter_detailed() {
        let e = make_hist_entry("file.rs", 3661, "abc", HistorySource::Manual);
        let fmt = LocalHistoryEntryFormatter::new(HistoryDisplayStyle::Detailed)
            .with_content_hash(true);
        let s = fmt.format_entry(&e);
        assert!(s.contains("URI:    file.rs"));
        assert!(s.contains("Manual"));
        assert!(s.contains("Hash:   abc"));
    }

    #[test]
    fn formatter_kv() {
        let e = make_hist_entry("a.rs", 50, "xyz", HistorySource::Undo);
        let fmt = LocalHistoryEntryFormatter::new(HistoryDisplayStyle::KeyValue);
        let s = fmt.format_entry(&e);
        assert!(s.contains("uri=a.rs"));
        assert!(s.contains("hash=xyz"));
    }

    #[test]
    fn formatter_truncate_label() {
        let mut e = make_hist_entry("f.rs", 1, "h", HistorySource::Auto);
        e.label = Some("a".repeat(100));
        let fmt = LocalHistoryEntryFormatter::new(HistoryDisplayStyle::Compact)
            .with_max_label_width(10);
        let s = fmt.format_entry(&e);
        assert!(s.len() < 100);
    }

    #[test]
    fn formatter_12h_time() {
        let e = make_hist_entry("f.rs", 3600 * 14 + 60 * 30, "h", HistorySource::Auto);
        let fmt = LocalHistoryEntryFormatter::new(HistoryDisplayStyle::Detailed)
            .with_24h_format(false);
        let s = fmt.format_entry(&e);
        assert!(s.contains("PM"));
    }

    #[test]
    fn formatter_entries_multiple() {
        let entries = vec![
            make_hist_entry("a.rs", 1, "h1", HistorySource::Auto),
            make_hist_entry("b.rs", 2, "h2", HistorySource::Manual),
        ];
        let fmt = LocalHistoryEntryFormatter::new(HistoryDisplayStyle::Compact);
        let s = fmt.format_entries(&entries);
        assert!(s.contains("[A]"));
        assert!(s.contains("[M]"));
    }

    #[test]
    fn formatter_style_accessor() {
        let fmt = LocalHistoryEntryFormatter::new(HistoryDisplayStyle::KeyValue);
        assert_eq!(fmt.style(), HistoryDisplayStyle::KeyValue);
    }

    #[test]
    fn merge_tool_basic() {
        let a = vec![make_hist_entry("f.rs", 1, "h1", HistorySource::Auto)];
        let b = vec![make_hist_entry("f.rs", 2, "h2", HistorySource::Auto)];
        let tool = LocalHistoryMergeTool::new(MergeConflictStrategy::PreferNewer);
        let merged = tool.merge(&a, &b);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].timestamp, 1);
        assert_eq!(merged[1].timestamp, 2);
    }

    #[test]
    fn merge_tool_dedup() {
        let a = vec![make_hist_entry("f.rs", 1, "same", HistorySource::Auto)];
        let b = vec![make_hist_entry("f.rs", 2, "same", HistorySource::Auto)];
        let tool = LocalHistoryMergeTool::new(MergeConflictStrategy::PreferOlder);
        let merged = tool.merge(&a, &b);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_tool_keep_both() {
        let a = vec![make_hist_entry("f.rs", 1, "same", HistorySource::Auto)];
        let b = vec![make_hist_entry("f.rs", 2, "same", HistorySource::Auto)];
        let tool = LocalHistoryMergeTool::new(MergeConflictStrategy::KeepBoth);
        let merged = tool.merge(&a, &b);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_tool_count_duplicates() {
        let entries = vec![
            make_hist_entry("f.rs", 1, "h1", HistorySource::Auto),
            make_hist_entry("f.rs", 2, "h1", HistorySource::Auto),
            make_hist_entry("f.rs", 3, "h2", HistorySource::Manual),
        ];
        let tool = LocalHistoryMergeTool::new(MergeConflictStrategy::PreferNewer);
        assert_eq!(tool.count_duplicates(&entries), 1);
    }

    #[test]
    fn merge_tool_merge_all() {
        let s1 = vec![make_hist_entry("a.rs", 1, "h1", HistorySource::Auto)];
        let s2 = vec![make_hist_entry("b.rs", 3, "h2", HistorySource::Auto)];
        let s3 = vec![make_hist_entry("c.rs", 2, "h3", HistorySource::Manual)];
        let tool = LocalHistoryMergeTool::new(MergeConflictStrategy::PreferNewer);
        let merged = tool.merge_all(&[s1, s2, s3]);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].timestamp, 1);
        assert_eq!(merged[1].timestamp, 2);
    }

    #[test]
    fn merge_tool_strategy_accessor() {
        let tool = LocalHistoryMergeTool::new(MergeConflictStrategy::KeepBoth);
        assert_eq!(tool.strategy(), MergeConflictStrategy::KeepBoth);
    }


    #[test]
    fn lhist_ringbuf_push_get() {
        let mut rb = LHistRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn lhist_ringbuf_overflow() {
        let mut rb = LHistRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn lhist_ringbuf_clear() {
        let mut rb = LHistRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn lhist_ringbuf_newest_oldest() {
        let mut rb = LHistRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn lhist_ringbuf_to_vec() {
        let mut rb = LHistRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn lhist_ringbuf_is_full() {
        let mut rb = LHistRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn lhist_builder_valid() {
        let cfg = LHistBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn lhist_builder_empty_name() {
        let r = LHistBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn lhist_builder_bad_priority() {
        assert!(LHistBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn lhist_builder_zero_max() {
        assert!(LHistBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn lhist_cfg_merge() {
        let mut a = LHistBuilder::new("a").property("x", "1").build().unwrap();
        let b = LHistBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn lhist_cfg_display() {
        let cfg = LHistBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    // -- local_history extended domain tests ----------------------------------------

    #[test]
    fn y_local_history_enum_index() {
        assert_eq!(YLocalHistoryHistoryEntryKind::Edit.index(), 0);
        assert_eq!(YLocalHistoryHistoryEntryKind::Save.index(), 1);
        assert_eq!(YLocalHistoryHistoryEntryKind::Revert.index(), 2);
        assert_eq!(YLocalHistoryHistoryEntryKind::Create.index(), 3);
    }

    #[test]
    fn y_local_history_enum_label() {
        assert_eq!(YLocalHistoryHistoryEntryKind::Edit.label(), "Edit");
        assert_eq!(YLocalHistoryHistoryEntryKind::Save.label(), "Save");
        assert_eq!(YLocalHistoryHistoryEntryKind::Revert.label(), "Revert");
        assert_eq!(YLocalHistoryHistoryEntryKind::Create.label(), "Create");
    }

    #[test]
    fn y_local_history_enum_all() {
        let all = YLocalHistoryHistoryEntryKind::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_local_history_enum_is_default() {
        assert!(YLocalHistoryHistoryEntryKind::Edit.is_default());
        assert!(!YLocalHistoryHistoryEntryKind::Create.is_default());
    }

    #[test]
    fn y_local_history_enum_display() {
        assert_eq!(format!("{}", YLocalHistoryHistoryEntryKind::Edit), "Edit");
    }

    #[test]
    fn y_local_history_struct_new() {
        let s = YLocalHistoryHistoryTimeline::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_local_history_struct_clear() {
        let mut s = YLocalHistoryHistoryTimeline::new();
        s.entries.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_local_history_fingerprint_deterministic() {
        let h1 = y_local_history_fingerprint("hello");
        let h2 = y_local_history_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_local_history_fingerprint("a"), y_local_history_fingerprint("b"));
    }

    #[test]
    fn y_local_history_truncate_short() {
        assert_eq!(y_local_history_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_local_history_truncate_long() {
        let r = y_local_history_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_local_history_normalize_key_basic() {
        assert_eq!(y_local_history_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_local_history_split_path_basic() {
        let parts = y_local_history_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_local_history_count_occurrences_basic() {
        assert_eq!(y_local_history_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_local_history_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_local_history_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_local_history_in_range_basic() {
        assert!(y_local_history_in_range(5, 1, 10));
        assert!(y_local_history_in_range(1, 1, 10));
        assert!(y_local_history_in_range(10, 1, 10));
        assert!(!y_local_history_in_range(0, 1, 10));
        assert!(!y_local_history_in_range(11, 1, 10));
    }

    #[test]
    fn y_local_history_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_local_history_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_local_history_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_local_history_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- local_history Z-extended tests -----------------------------------------------

    #[test]
    fn z_local_history_priority_weight() {
        assert_eq!(ZLocalHistoryPriority::Idle.weight(), 0);
        assert_eq!(ZLocalHistoryPriority::Normal.weight(), 2);
        assert_eq!(ZLocalHistoryPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_local_history_priority_label() {
        assert_eq!(ZLocalHistoryPriority::Low.label(), "low");
        assert_eq!(ZLocalHistoryPriority::High.label(), "high");
    }

    #[test]
    fn z_local_history_priority_is_elevated() {
        assert!(!ZLocalHistoryPriority::Normal.is_elevated());
        assert!(ZLocalHistoryPriority::High.is_elevated());
        assert!(ZLocalHistoryPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_local_history_priority_display() {
        assert_eq!(format!("{}", ZLocalHistoryPriority::Idle), "idle");
    }

    #[test]
    fn z_local_history_priority_all_asc() {
        let all = ZLocalHistoryPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZLocalHistoryPriority::Idle);
        assert_eq!(all[4], ZLocalHistoryPriority::Realtime);
    }

    #[test]
    fn z_local_history_struct_new() {
        let s = ZLocalHistoryHistoryCompactor::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_local_history_struct_toggled_clone() {
        let s = ZLocalHistoryHistoryCompactor::new();
        let t = s.toggled_clone();
        let _ = t.bytes_saved;
    }

    #[test]
    fn z_local_history_rolling_hash_deterministic() {
        let h1 = z_local_history_rolling_hash(b"test");
        let h2 = z_local_history_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_local_history_rolling_hash(b"a"), z_local_history_rolling_hash(b"b"));
    }

    #[test]
    fn z_local_history_pad_to_basic() {
        assert_eq!(z_local_history_pad_to("hi", 5), "hi   ");
        assert_eq!(z_local_history_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_local_history_is_identifier_basic() {
        assert!(z_local_history_is_identifier("foo_bar"));
        assert!(z_local_history_is_identifier("abc123"));
        assert!(!z_local_history_is_identifier(""));
        assert!(!z_local_history_is_identifier("has space"));
    }

    #[test]
    fn z_local_history_levenshtein_basic() {
        assert_eq!(z_local_history_levenshtein("", ""), 0);
        assert_eq!(z_local_history_levenshtein("abc", "abc"), 0);
        assert_eq!(z_local_history_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_local_history_unique_words_basic() {
        let w = z_local_history_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_local_history_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_local_history_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_local_history_common_prefix_basic() {
        assert_eq!(z_local_history_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_local_history_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_local_history_struct_clear() {
        let mut s = ZLocalHistoryHistoryCompactor::new();
        s.versions.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_local_history_rolling_hash_empty() {
        let h = z_local_history_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 116 ----

    #[test]
    fn xc_116_pool_new_empty() {
        let pool: super::Xc116Pool<i32> = super::Xc116Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_116_pool_release_acquire() {
        let mut pool = super::Xc116Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_116_pool_acquire_empty() {
        let mut pool: super::Xc116Pool<i32> = super::Xc116Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_116_pool_full() {
        let mut pool = super::Xc116Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_116_pool_drain() {
        let mut pool = super::Xc116Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_116_pool_stats() {
        let mut pool = super::Xc116Pool::new(8);
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
    fn xc_116_pool_clear() {
        let mut pool = super::Xc116Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_116_pool_shrink() {
        let mut pool = super::Xc116Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_116_pool_default() {
        let pool: super::Xc116Pool<String> = super::Xc116Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_116_pool_extend() {
        let mut pool = super::Xc116Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_116_pool_retain() {
        let mut pool = super::Xc116Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_116_scheduler_round_robin() {
        let mut sched = super::Xc116Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_116_scheduler_empty() {
        let mut sched = super::Xc116Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_116_scheduler_reset() {
        let mut sched = super::Xc116Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_116_scheduler_add_remove() {
        let mut sched = super::Xc116Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_116_scheduler_targets() {
        let sched = super::Xc116Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_116_hash_empty() {
        assert_eq!(super::xc_116_hash(b""), 5381);
    }

    #[test]
    fn xc_116_hash_data() {
        let h = super::xc_116_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_116_hash(b"hello"), h);
    }

    #[test]
    fn xc_116_reverse_str() {
        assert_eq!(super::xc_116_reverse("abc"), "cba");
        assert_eq!(super::xc_116_reverse(""), "");
    }


    // --- xd_20 deepening tests ---

    #[test]
    fn xd_20_sm_initial_state() {
        let sm = Xd20StateMachine::new();
        assert_eq!(sm.current_state(), Xd20State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_20_sm_valid_idle_to_running() {
        let mut sm = Xd20StateMachine::new();
        assert!(sm.transition(Xd20State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd20State::Running);
    }

    #[test]
    fn xd_20_sm_valid_running_to_paused() {
        let mut sm = Xd20StateMachine::new();
        sm.transition(Xd20State::Running).unwrap();
        assert!(sm.transition(Xd20State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd20State::Paused);
    }

    #[test]
    fn xd_20_sm_valid_running_to_done() {
        let mut sm = Xd20StateMachine::new();
        sm.transition(Xd20State::Running).unwrap();
        assert!(sm.transition(Xd20State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd20State::Done);
    }

    #[test]
    fn xd_20_sm_valid_paused_to_running() {
        let mut sm = Xd20StateMachine::new();
        sm.transition(Xd20State::Running).unwrap();
        sm.transition(Xd20State::Paused).unwrap();
        assert!(sm.transition(Xd20State::Running).is_ok());
    }

    #[test]
    fn xd_20_sm_valid_done_to_idle() {
        let mut sm = Xd20StateMachine::new();
        sm.transition(Xd20State::Running).unwrap();
        sm.transition(Xd20State::Done).unwrap();
        assert!(sm.transition(Xd20State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd20State::Idle);
    }

    #[test]
    fn xd_20_sm_invalid_idle_to_done() {
        let mut sm = Xd20StateMachine::new();
        assert!(sm.transition(Xd20State::Done).is_err());
    }

    #[test]
    fn xd_20_sm_invalid_idle_to_paused() {
        let mut sm = Xd20StateMachine::new();
        assert!(sm.transition(Xd20State::Paused).is_err());
    }

    #[test]
    fn xd_20_sm_history_tracking() {
        let mut sm = Xd20StateMachine::new();
        sm.transition(Xd20State::Running).unwrap();
        sm.transition(Xd20State::Paused).unwrap();
        sm.transition(Xd20State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd20State::Idle);
        assert_eq!(sm.history()[0].to, Xd20State::Running);
        assert_eq!(sm.history()[1].from, Xd20State::Running);
        assert_eq!(sm.history()[2].to, Xd20State::Done);
    }

    #[test]
    fn xd_20_sm_serialize_deserialize() {
        let mut sm = Xd20StateMachine::new();
        sm.transition(Xd20State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd20StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd20State::Running));
    }

    #[test]
    fn xd_20_sm_deserialize_invalid() {
        assert_eq!(Xd20StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_20_sm_reset() {
        let mut sm = Xd20StateMachine::new();
        sm.transition(Xd20State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd20State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_20_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd20EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd20Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_20_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd20EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd20Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd20Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_20_bus_unsubscribe() {
        let mut bus = Xd20EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_20_event_kind_and_payload() {
        let e = Xd20Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd20Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_20_bus_clear_history() {
        let mut bus = Xd20EventBus::new();
        bus.publish(Xd20Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_20_sm_step_counter_increments() {
        let mut sm = Xd20StateMachine::new();
        sm.transition(Xd20State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd20State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #18 --

    #[test]
    fn xf18_trie_insert_search() {
        let mut t = Xf18Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf18_trie_starts_with() {
        let mut t = Xf18Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf18_trie_remove() {
        let mut t = Xf18Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf18_trie_word_count() {
        let mut t = Xf18Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf18_trie_longest_prefix() {
        let mut t = Xf18Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf18_trie_all_words() {
        let mut t = Xf18Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf18_trie_autocomplete() {
        let mut t = Xf18Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf18_trie_empty_search() {
        let t = Xf18Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf18_bloom_add_contains() {
        let mut bf = Xf18BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf18_bloom_probably_absent() {
        let bf = Xf18BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf18_bloom_false_positive_rate() {
        let mut bf = Xf18BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf18_bloom_clear() {
        let mut bf = Xf18BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf18_bloom_union() {
        let mut a = Xf18BloomFilter::xf_new(512, 2);
        let mut b = Xf18BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf18_bloom_intersection_estimate() {
        let mut a = Xf18BloomFilter::xf_new(512, 2);
        let mut b = Xf18BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf18_bloom_union_size_mismatch() {
        let a = Xf18BloomFilter::xf_new(256, 2);
        let b = Xf18BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh115_skip_insert_contains() {
        let mut sl = super::Xh115SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh115_skip_remove() {
        let mut sl = super::Xh115SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh115_skip_len() {
        let mut sl = super::Xh115SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh115_skip_range_query() {
        let mut sl = super::Xh115SkipList::xh_new(4);
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
    fn xh115_skip_floor_ceiling() {
        let mut sl = super::Xh115SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh115_skip_rank() {
        let mut sl = super::Xh115SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh115_skip_empty() {
        let sl = super::Xh115SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh115_skip_duplicates() {
        let mut sl = super::Xh115SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh115_bitset_set_test() {
        let mut bs = super::Xh115BitSet::xh_new(256);
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
    fn xh115_bitset_clear_count() {
        let mut bs = super::Xh115BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh115_bitset_and_or_xor() {
        let mut a = super::Xh115BitSet::xh_new(128);
        let mut b = super::Xh115BitSet::xh_new(128);
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
    fn xh115_bitset_iter_ones() {
        let mut bs = super::Xh115BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh115_bitset_first_last() {
        let mut bs = super::Xh115BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh115_bitset_empty() {
        let bs = super::Xh115BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi115_deque_push_pop_back() {
        let mut dq = super::Xi115Deque::xi_new(4);
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
    fn xi115_deque_push_pop_front() {
        let mut dq = super::Xi115Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi115_deque_mixed_ops() {
        let mut dq = super::Xi115Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi115_deque_get_and_split() {
        let mut dq = super::Xi115Deque::xi_new(8);
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
    fn xi115_deque_rotate_left() {
        let mut dq = super::Xi115Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi115_deque_rotate_right() {
        let mut dq = super::Xi115Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi115_deque_grow() {
        let mut dq = super::Xi115Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi115_deque_empty() {
        let dq = super::Xi115Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi115_interval_tree_insert_query() {
        let mut tree = super::Xi115IntervalTree::xi_new();
        tree.xi_insert(super::Xi115Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi115Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi115Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi115_interval_tree_overlap() {
        let mut tree = super::Xi115IntervalTree::xi_new();
        tree.xi_insert(super::Xi115Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi115Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi115Interval::xi_new(12, 20));
        let q = super::Xi115Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi115_interval_tree_remove() {
        let mut tree = super::Xi115IntervalTree::xi_new();
        tree.xi_insert(super::Xi115Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi115Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi115_interval_tree_gaps() {
        let mut tree = super::Xi115IntervalTree::xi_new();
        tree.xi_insert(super::Xi115Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi115Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi115Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi115Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi115Interval::xi_new(8, 10));
    }

    #[test]
    fn xi115_interval_tree_merge() {
        let mut tree = super::Xi115IntervalTree::xi_new();
        tree.xi_insert(super::Xi115Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi115Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi115Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi115Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi115Interval::xi_new(10, 15));
    }

    #[test]
    fn xi115_interval_tree_all() {
        let mut tree = super::Xi115IntervalTree::xi_new();
        tree.xi_insert(super::Xi115Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi115Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi115_interval_tree_empty() {
        let tree = super::Xi115IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi115_interval_tree_contains_point() {
        let iv = super::Xi115Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 115) ---

    #[test]
    fn xj_115_uf_make_and_find() {
        let mut uf = super::Xj115UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_115_uf_union_connected() {
        let mut uf = super::Xj115UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_115_uf_component_count() {
        let mut uf = super::Xj115UnionFind::xj_new();
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
    fn xj_115_uf_component_size() {
        let mut uf = super::Xj115UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_115_uf_largest_component() {
        let mut uf = super::Xj115UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_115_uf_many_elements() {
        let mut uf = super::Xj115UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_115_uf_separate_components() {
        let mut uf = super::Xj115UnionFind::xj_new();
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
    fn xj_115_uf_path_compression() {
        let mut uf = super::Xj115UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_115_bt_insert_get() {
        let mut bt = super::Xj115BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_115_bt_contains_len() {
        let mut bt = super::Xj115BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_115_bt_replace() {
        let mut bt = super::Xj115BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_115_bt_remove() {
        let mut bt = super::Xj115BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_115_bt_keys_values() {
        let mut bt = super::Xj115BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_115_bt_range() {
        let mut bt = super::Xj115BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_115_bt_min_max() {
        let mut bt = super::Xj115BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_115_bt_many_inserts() {
        let mut bt = super::Xj115BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_115 segment tree tests ---

    #[test]
    fn xk_115_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk115SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_115_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk115SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_115_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk115SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_115_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk115SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_115_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk115SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_115_st_single_element() {
        let data = vec![42];
        let st = super::Xk115SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_115_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk115SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_115_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk115SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_115 disjoint intervals tests ---

    #[test]
    fn xk_115_di_add_and_count() {
        let mut di = super::Xk115DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_115_di_merge_overlap() {
        let mut di = super::Xk115DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_115_di_contains() {
        let mut di = super::Xk115DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_115_di_remove() {
        let mut di = super::Xk115DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_115_di_covered_length() {
        let mut di = super::Xk115DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_115_di_gaps() {
        let mut di = super::Xk115DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_115_di_merge_adjacent() {
        let mut di = super::Xk115DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_115_di_empty() {
        let di = super::Xk115DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_115_rope_new_empty() {
        let rope = super::Xl115Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_115_rope_from_str() {
        let rope = super::Xl115Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_115_rope_insert_at() {
        let mut rope = super::Xl115Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_115_rope_delete_range() {
        let mut rope = super::Xl115Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_115_rope_char_at() {
        let rope = super::Xl115Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_115_rope_split_concat() {
        let rope = super::Xl115Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_115_rope_line_count() {
        let rope = super::Xl115Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_115_rope_line_at() {
        let rope = super::Xl115Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_115_sa_build_and_search() {
        let sa = super::Xl115SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_115_sa_count() {
        let sa = super::Xl115SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_115_sa_longest_repeated() {
        let sa = super::Xl115SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_115_sa_all_positions() {
        let sa = super::Xl115SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_115_sa_len() {
        let sa = super::Xl115SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_115_sa_empty() {
        let sa = super::Xl115SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_115_rope_slice() {
        let rope = super::Xl115Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_115_sa_search_start() {
        let sa = super::Xl115SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_115_sparse_set_get() {
        let mut m = super::Xm115MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_115_sparse_row_col() {
        let mut m = super::Xm115MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_115_sparse_transpose() {
        let mut m = super::Xm115MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_115_sparse_multiply_vec() {
        let mut m = super::Xm115MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_115_sparse_nnz_density() {
        let mut m = super::Xm115MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_115_sparse_clear() {
        let mut m = super::Xm115MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_115_sparse_overwrite_zero() {
        let mut m = super::Xm115MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_115_tokenizer_basic() {
        let t = super::Xm115Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_115_tokenizer_count() {
        let t = super::Xm115Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_115_tokenizer_unique() {
        let t = super::Xm115Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_115_tokenizer_frequency() {
        let t = super::Xm115Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_115_tokenizer_delimiter() {
        let t = super::Xm115Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_115_tokenizer_whitespace() {
        let t = super::Xm115Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_115_tokenizer_empty() {
        let t = super::Xm115Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }

}