//! Local file history tracking.
//!
//! Records content snapshots keyed by URI + timestamp so that previous
//! versions of a file can be inspected or restored.

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
// Tests
// ---------------------------------------------------------------------------

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
}
