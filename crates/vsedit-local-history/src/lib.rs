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
}
