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
}
