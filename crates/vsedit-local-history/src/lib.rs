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
}
