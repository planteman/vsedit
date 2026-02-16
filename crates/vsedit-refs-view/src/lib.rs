//! References view.

use std::fmt;

/// A source location in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub uri: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl Location {
    pub fn new(uri: &str, start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            uri: uri.to_string(),
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    pub fn is_single_line(&self) -> bool {
        self.start_line == self.end_line
    }

    pub fn contains_position(&self, line: u32, col: u32) -> bool {
        if line < self.start_line || line > self.end_line {
            return false;
        }
        if line == self.start_line && col < self.start_col {
            return false;
        }
        if line == self.end_line && col > self.end_col {
            return false;
        }
        true
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.uri, self.start_line, self.start_col)
    }
}

/// A single reference with surrounding context.
#[derive(Debug, Clone)]
pub struct ReferenceItem {
    pub location: Location,
    pub context_before: Option<String>,
    pub context_line: String,
    pub context_after: Option<String>,
}

impl ReferenceItem {
    pub fn has_context(&self) -> bool {
        self.context_before.is_some() || self.context_after.is_some()
    }
}

impl fmt::Display for ReferenceItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.context_line)
    }
}

/// Model holding all references for a symbol.
#[derive(Debug, Clone)]
pub struct ReferencesModel {
    pub title: String,
    pub base_location: Location,
    pub references: Vec<ReferenceItem>,
}

impl ReferencesModel {
    pub fn new(title: impl Into<String>, base: Location) -> Self {
        Self {
            title: title.into(),
            base_location: base,
            references: Vec::new(),
        }
    }

    pub fn add_reference(&mut self, item: ReferenceItem) {
        self.references.push(item);
    }

    pub fn references_in_file(&self, uri: &str) -> Vec<&ReferenceItem> {
        self.references
            .iter()
            .filter(|r| r.location.uri == uri)
            .collect()
    }

    pub fn file_count(&self) -> usize {
        let mut uris: Vec<&str> = self.references.iter().map(|r| r.location.uri.as_str()).collect();
        uris.sort_unstable();
        uris.dedup();
        uris.len()
    }

    pub fn total_count(&self) -> usize {
        self.references.len()
    }

    pub fn sort_by_location(&mut self) {
        self.references.sort_by(|a, b| {
            a.location
                .uri
                .cmp(&b.location.uri)
                .then(a.location.start_line.cmp(&b.location.start_line))
                .then(a.location.start_col.cmp(&b.location.start_col))
        });
    }

    pub fn unique_files(&self) -> Vec<&str> {
        let mut uris: Vec<&str> = self.references.iter().map(|r| r.location.uri.as_str()).collect();
        uris.sort_unstable();
        uris.dedup();
        uris
    }

    pub fn remove_references_in_file(&mut self, uri: &str) -> usize {
        let before = self.references.len();
        self.references.retain(|r| r.location.uri != uri);
        before - self.references.len()
    }

    pub fn find_at_position(&self, uri: &str, line: u32, col: u32) -> Option<&ReferenceItem> {
        self.references
            .iter()
            .find(|r| r.location.uri == uri && r.location.contains_position(line, col))
    }

    pub fn is_empty(&self) -> bool {
        self.references.is_empty()
    }

    pub fn group_by_file(&self) -> Vec<(&str, Vec<&ReferenceItem>)> {
        let files = self.unique_files();
        files
            .into_iter()
            .map(|uri| {
                let refs = self.references_in_file(uri);
                (uri, refs)
            })
            .collect()
    }
}

/// The kind of reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Declaration,
    Definition,
    Read,
    Write,
    Call,
    Import,
    Other,
}

impl fmt::Display for ReferenceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration => write!(f, "Declaration"),
            Self::Definition => write!(f, "Definition"),
            Self::Read => write!(f, "Read"),
            Self::Write => write!(f, "Write"),
            Self::Call => write!(f, "Call"),
            Self::Import => write!(f, "Import"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Result of a reference search with metadata.
#[derive(Debug, Clone)]
pub struct ReferenceSearchResult {
    pub symbol_name: String,
    pub model: ReferencesModel,
    pub search_duration_ms: u64,
    pub include_declaration: bool,
}

impl ReferenceSearchResult {
    pub fn new(symbol_name: impl Into<String>, model: ReferencesModel, duration_ms: u64) -> Self {
        Self {
            symbol_name: symbol_name.into(),
            model,
            search_duration_ms: duration_ms,
            include_declaration: true,
        }
    }

    pub fn without_declaration(mut self) -> Self {
        self.include_declaration = false;
        self
    }

    pub fn summary(&self) -> String {
        format!(
            "'{}': {} references in {} files ({}ms)",
            self.symbol_name,
            self.model.total_count(),
            self.model.file_count(),
            self.search_duration_ms
        )
    }
}

impl Location {
    /// Return the number of lines this location spans.
    pub fn line_span(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Return true if this location overlaps with another location in the same file.
    pub fn overlaps(&self, other: &Location) -> bool {
        if self.uri != other.uri {
            return false;
        }
        // No overlap if one entirely precedes the other
        if self.end_line < other.start_line || other.end_line < self.start_line {
            return false;
        }
        if self.end_line == other.start_line && self.end_col < other.start_col {
            return false;
        }
        if other.end_line == self.start_line && other.end_col < self.start_col {
            return false;
        }
        true
    }

    /// Merge two overlapping locations into a single encompassing location.
    /// Returns None if they don't overlap or are in different files.
    pub fn merge(&self, other: &Location) -> Option<Location> {
        if !self.overlaps(other) {
            return None;
        }
        let start_line = self.start_line.min(other.start_line);
        let start_col = if self.start_line < other.start_line {
            self.start_col
        } else if other.start_line < self.start_line {
            other.start_col
        } else {
            self.start_col.min(other.start_col)
        };
        let end_line = self.end_line.max(other.end_line);
        let end_col = if self.end_line > other.end_line {
            self.end_col
        } else if other.end_line > self.end_line {
            other.end_col
        } else {
            self.end_col.max(other.end_col)
        };
        Some(Location {
            uri: self.uri.clone(),
            start_line,
            start_col,
            end_line,
            end_col,
        })
    }

    /// Get the file name (last path component) from the URI.
    pub fn file_name(&self) -> &str {
        self.uri.rsplit('/').next().unwrap_or(&self.uri)
    }
}

impl ReferencesModel {
    /// Filter references keeping only those in the specified file.
    pub fn filter_by_file(&self, uri: &str) -> ReferencesModel {
        let mut filtered = ReferencesModel::new(self.title.clone(), self.base_location.clone());
        for r in &self.references {
            if r.location.uri == uri {
                filtered.add_reference(r.clone());
            }
        }
        filtered
    }

    /// Count references per file, returning sorted pairs.
    pub fn count_per_file(&self) -> Vec<(&str, usize)> {
        let files = self.unique_files();
        let mut counts: Vec<(&str, usize)> = files
            .into_iter()
            .map(|f| (f, self.references_in_file(f).len()))
            .collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1));
        counts
    }

    /// Get references sorted by line number within a single file.
    pub fn sorted_refs_in_file(&self, uri: &str) -> Vec<&ReferenceItem> {
        let mut refs = self.references_in_file(uri);
        refs.sort_by_key(|r| (r.location.start_line, r.location.start_col));
        refs
    }

    /// Merge consecutive references that are on adjacent lines in the same file.
    /// Returns groups of related references.
    pub fn cluster_by_proximity(&self, max_gap: u32) -> Vec<Vec<&ReferenceItem>> {
        let mut clusters: Vec<Vec<&ReferenceItem>> = Vec::new();
        for file in self.unique_files() {
            let sorted = self.sorted_refs_in_file(file);
            if sorted.is_empty() {
                continue;
            }
            let mut current_cluster: Vec<&ReferenceItem> = vec![sorted[0]];
            for r in &sorted[1..] {
                let last = current_cluster.last().unwrap();
                if r.location.start_line <= last.location.end_line + max_gap {
                    current_cluster.push(r);
                } else {
                    clusters.push(std::mem::take(&mut current_cluster));
                    current_cluster.push(r);
                }
            }
            if !current_cluster.is_empty() {
                clusters.push(current_cluster);
            }
        }
        clusters
    }

    /// Get a flat summary string of all references.
    pub fn flat_summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("References for '{}' ({})", self.title, self.total_count()));
        for (file, refs) in self.group_by_file() {
            lines.push(format!("  {} ({} refs)", file, refs.len()));
            for r in refs {
                lines.push(format!("    L{}: {}", r.location.start_line, r.context_line));
            }
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(uri: &str, line: u32, col: u32) -> Location {
        Location {
            uri: uri.to_string(),
            start_line: line,
            start_col: col,
            end_line: line,
            end_col: col + 5,
        }
    }

    fn ref_item(uri: &str, line: u32, col: u32) -> ReferenceItem {
        ReferenceItem {
            location: loc(uri, line, col),
            context_before: None,
            context_line: format!("code at {line}:{col}"),
            context_after: None,
        }
    }

    #[test]
    fn add_and_count() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 10, 5));
        model.add_reference(ref_item("b.rs", 20, 3));
        assert_eq!(model.total_count(), 2);
        assert_eq!(model.file_count(), 2);
    }

    #[test]
    fn references_in_file() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 10, 5));
        model.add_reference(ref_item("b.rs", 20, 3));
        model.add_reference(ref_item("a.rs", 30, 1));
        assert_eq!(model.references_in_file("a.rs").len(), 2);
        assert_eq!(model.references_in_file("c.rs").len(), 0);
    }

    #[test]
    fn sort_by_location() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        model.add_reference(ref_item("b.rs", 5, 0));
        model.add_reference(ref_item("a.rs", 20, 0));
        model.add_reference(ref_item("a.rs", 10, 0));
        model.sort_by_location();
        assert_eq!(model.references[0].location.uri, "a.rs");
        assert_eq!(model.references[0].location.start_line, 10);
        assert_eq!(model.references[1].location.start_line, 20);
        assert_eq!(model.references[2].location.uri, "b.rs");
    }

    #[test]
    fn location_new_and_display() {
        let l = Location::new("main.rs", 10, 4, 10, 12);
        assert_eq!(l.uri, "main.rs");
        assert_eq!(l.start_line, 10);
        assert_eq!(l.end_col, 12);
        assert_eq!(l.to_string(), "main.rs:10:4");
    }

    #[test]
    fn location_is_single_line() {
        assert!(Location::new("a.rs", 5, 0, 5, 10).is_single_line());
        assert!(!Location::new("a.rs", 5, 0, 7, 10).is_single_line());
    }

    #[test]
    fn location_contains_position() {
        let l = Location::new("a.rs", 5, 3, 8, 10);
        assert!(l.contains_position(5, 3));
        assert!(l.contains_position(6, 0));
        assert!(l.contains_position(8, 10));
        assert!(!l.contains_position(5, 2));
        assert!(!l.contains_position(8, 11));
        assert!(!l.contains_position(4, 5));
        assert!(!l.contains_position(9, 0));
    }

    #[test]
    fn reference_item_has_context() {
        let without = ref_item("a.rs", 1, 0);
        assert!(!without.has_context());

        let with = ReferenceItem {
            location: loc("a.rs", 1, 0),
            context_before: Some("before".into()),
            context_line: "line".into(),
            context_after: None,
        };
        assert!(with.has_context());
    }

    #[test]
    fn reference_item_display() {
        let r = ref_item("a.rs", 10, 5);
        assert_eq!(r.to_string(), "a.rs:10:5: code at 10:5");
    }

    #[test]
    fn unique_files_sorted() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        model.add_reference(ref_item("c.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 2, 0));
        model.add_reference(ref_item("b.rs", 3, 0));
        model.add_reference(ref_item("a.rs", 4, 0));
        assert_eq!(model.unique_files(), vec!["a.rs", "b.rs", "c.rs"]);
    }

    #[test]
    fn remove_references_in_file() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 10, 0));
        model.add_reference(ref_item("b.rs", 20, 0));
        model.add_reference(ref_item("a.rs", 30, 0));
        let removed = model.remove_references_in_file("a.rs");
        assert_eq!(removed, 2);
        assert_eq!(model.total_count(), 1);
        assert_eq!(model.references[0].location.uri, "b.rs");
    }

    #[test]
    fn find_at_position() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 10, 5));
        model.add_reference(ref_item("b.rs", 20, 3));
        let found = model.find_at_position("a.rs", 10, 7);
        assert!(found.is_some());
        assert_eq!(found.unwrap().location.start_line, 10);
        assert!(model.find_at_position("a.rs", 99, 0).is_none());
        assert!(model.find_at_position("c.rs", 10, 5).is_none());
    }

    #[test]
    fn is_empty() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        assert!(model.is_empty());
        model.add_reference(ref_item("a.rs", 1, 0));
        assert!(!model.is_empty());
    }

    #[test]
    fn group_by_file() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        model.add_reference(ref_item("b.rs", 5, 0));
        model.add_reference(ref_item("a.rs", 10, 0));
        model.add_reference(ref_item("a.rs", 20, 0));
        let groups = model.group_by_file();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "a.rs");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, "b.rs");
        assert_eq!(groups[1].1.len(), 1);
    }

    #[test]
    fn reference_kind_display() {
        assert_eq!(ReferenceKind::Declaration.to_string(), "Declaration");
        assert_eq!(ReferenceKind::Definition.to_string(), "Definition");
        assert_eq!(ReferenceKind::Read.to_string(), "Read");
        assert_eq!(ReferenceKind::Write.to_string(), "Write");
        assert_eq!(ReferenceKind::Call.to_string(), "Call");
        assert_eq!(ReferenceKind::Import.to_string(), "Import");
        assert_eq!(ReferenceKind::Other.to_string(), "Other");
    }

    #[test]
    fn location_line_span() {
        assert_eq!(Location::new("a.rs", 5, 0, 5, 10).line_span(), 1);
        assert_eq!(Location::new("a.rs", 5, 0, 7, 10).line_span(), 3);
    }

    #[test]
    fn location_overlaps() {
        let a = Location::new("a.rs", 5, 0, 8, 10);
        let b = Location::new("a.rs", 7, 0, 12, 5);
        let c = Location::new("a.rs", 10, 0, 15, 5);
        let d = Location::new("b.rs", 5, 0, 8, 10);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
        assert!(!a.overlaps(&c));
        assert!(!a.overlaps(&d)); // different file
    }

    #[test]
    fn location_merge() {
        let a = Location::new("a.rs", 5, 0, 8, 10);
        let b = Location::new("a.rs", 7, 3, 12, 5);
        let merged = a.merge(&b).unwrap();
        assert_eq!(merged.start_line, 5);
        assert_eq!(merged.start_col, 0);
        assert_eq!(merged.end_line, 12);
        assert_eq!(merged.end_col, 5);
    }

    #[test]
    fn location_merge_none_for_non_overlapping() {
        let a = Location::new("a.rs", 1, 0, 3, 10);
        let b = Location::new("a.rs", 5, 0, 7, 10);
        assert!(a.merge(&b).is_none());
    }

    #[test]
    fn location_file_name() {
        assert_eq!(Location::new("src/main.rs", 1, 0, 1, 5).file_name(), "main.rs");
        assert_eq!(Location::new("lib.rs", 1, 0, 1, 5).file_name(), "lib.rs");
    }

    #[test]
    fn search_result_summary() {
        let model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        let result = ReferenceSearchResult::new("foo", model, 42);
        let summary = result.summary();
        assert!(summary.contains("'foo'"));
        assert!(summary.contains("42ms"));
    }

    #[test]
    fn search_result_without_declaration() {
        let model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        let result = ReferenceSearchResult::new("foo", model, 10).without_declaration();
        assert!(!result.include_declaration);
    }

    #[test]
    fn filter_by_file() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 10, 0));
        model.add_reference(ref_item("b.rs", 20, 0));
        model.add_reference(ref_item("a.rs", 30, 0));
        let filtered = model.filter_by_file("a.rs");
        assert_eq!(filtered.total_count(), 2);
        assert_eq!(filtered.file_count(), 1);
    }

    #[test]
    fn count_per_file() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 10, 0));
        model.add_reference(ref_item("b.rs", 20, 0));
        model.add_reference(ref_item("a.rs", 30, 0));
        let counts = model.count_per_file();
        assert_eq!(counts[0], ("a.rs", 2));
        assert_eq!(counts[1], ("b.rs", 1));
    }

    #[test]
    fn sorted_refs_in_file() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 30, 0));
        model.add_reference(ref_item("a.rs", 10, 5));
        model.add_reference(ref_item("a.rs", 10, 0));
        let sorted = model.sorted_refs_in_file("a.rs");
        assert_eq!(sorted[0].location.start_line, 10);
        assert_eq!(sorted[0].location.start_col, 0);
        assert_eq!(sorted[1].location.start_col, 5);
        assert_eq!(sorted[2].location.start_line, 30);
    }

    #[test]
    fn cluster_by_proximity() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 2, 0));
        model.add_reference(ref_item("a.rs", 3, 0));
        model.add_reference(ref_item("a.rs", 20, 0));
        model.add_reference(ref_item("a.rs", 21, 0));
        let clusters = model.cluster_by_proximity(2);
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].len(), 3);
        assert_eq!(clusters[1].len(), 2);
    }

    #[test]
    fn flat_summary() {
        let mut model = ReferencesModel::new("my_func", loc("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 10, 0));
        model.add_reference(ref_item("b.rs", 5, 0));
        let summary = model.flat_summary();
        assert!(summary.contains("my_func"));
        assert!(summary.contains("a.rs"));
        assert!(summary.contains("b.rs"));
    }
}
