//! Navigation history.

/// A single entry in the navigation history.
#[derive(Debug, Clone)]
pub struct HistoryNavigationEntry {
    pub uri: String,
    pub line: u32,
    pub column: u32,
    pub label: Option<String>,
}

/// Maintains a bounded navigation history with back/forward support.
#[derive(Debug, Clone)]
pub struct NavigationHistory {
    pub entries: Vec<HistoryNavigationEntry>,
    pub current: Option<usize>,
    pub max_size: usize,
}

impl NavigationHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            current: None,
            max_size,
        }
    }

    /// Push a new entry, discarding any forward history and enforcing `max_size`.
    pub fn push(&mut self, entry: HistoryNavigationEntry) {
        // Trim forward history beyond current position.
        if let Some(idx) = self.current {
            self.entries.truncate(idx + 1);
        }

        self.entries.push(entry);

        // Evict oldest entries when exceeding max_size.
        while self.entries.len() > self.max_size {
            self.entries.remove(0);
        }

        self.current = Some(self.entries.len() - 1);
    }

    pub fn go_back(&mut self) -> Option<&HistoryNavigationEntry> {
        let idx = self.current?;
        if idx == 0 {
            return None;
        }
        self.current = Some(idx - 1);
        self.entries.get(idx - 1)
    }

    pub fn go_forward(&mut self) -> Option<&HistoryNavigationEntry> {
        let idx = self.current?;
        if idx + 1 >= self.entries.len() {
            return None;
        }
        self.current = Some(idx + 1);
        self.entries.get(idx + 1)
    }

    pub fn can_go_back(&self) -> bool {
        matches!(self.current, Some(idx) if idx > 0)
    }

    pub fn can_go_forward(&self) -> bool {
        matches!(self.current, Some(idx) if idx + 1 < self.entries.len())
    }

    pub fn current(&self) -> Option<&HistoryNavigationEntry> {
        self.entries.get(self.current?)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current = None;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(uri: &str, line: u32) -> HistoryNavigationEntry {
        HistoryNavigationEntry {
            uri: uri.to_string(),
            line,
            column: 0,
            label: None,
        }
    }

    #[test]
    fn push_and_navigate_back_forward() {
        let mut h = NavigationHistory::new(10);
        assert!(!h.can_go_back());
        assert!(!h.can_go_forward());

        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("c.rs", 3));
        assert_eq!(h.len(), 3);
        assert_eq!(h.current().unwrap().uri, "c.rs");

        let back = h.go_back().unwrap();
        assert_eq!(back.uri, "b.rs");
        assert!(h.can_go_forward());

        let fwd = h.go_forward().unwrap();
        assert_eq!(fwd.uri, "c.rs");
        assert!(!h.can_go_forward());
    }

    #[test]
    fn push_truncates_forward_history() {
        let mut h = NavigationHistory::new(10);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("c.rs", 3));

        h.go_back();
        h.go_back(); // now at "a.rs"

        h.push(entry("d.rs", 4));
        assert_eq!(h.len(), 2); // "a.rs" and "d.rs"
        assert_eq!(h.current().unwrap().uri, "d.rs");
        assert!(!h.can_go_forward());
    }

    #[test]
    fn max_size_evicts_oldest() {
        let mut h = NavigationHistory::new(3);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("c.rs", 3));
        h.push(entry("d.rs", 4));

        assert_eq!(h.len(), 3);
        assert_eq!(h.entries[0].uri, "b.rs");
        assert_eq!(h.current().unwrap().uri, "d.rs");
    }

    #[test]
    fn clear_resets_history() {
        let mut h = NavigationHistory::new(10);
        h.push(entry("a.rs", 1));
        h.clear();
        assert_eq!(h.len(), 0);
        assert!(h.current().is_none());
    }
}
