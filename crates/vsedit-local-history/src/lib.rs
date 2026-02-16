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
    fn clear_all_empties_entries() {
        let mut svc = LocalHistoryService::new(10);
        svc.add_entry("file:///d.rs", "x", HistorySource::Auto);
        svc.clear_all();
        assert_eq!(svc.entry_count(), 0);
    }
}
