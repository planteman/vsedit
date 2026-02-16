//! File decorations.

/// Defines the visual style of a decoration.
#[derive(Debug, Clone)]
pub struct DecorationType {
    pub id: String,
    pub background_color: Option<String>,
    pub border: Option<String>,
    pub outline: Option<String>,
    pub gutter_icon: Option<String>,
    pub is_whole_line: bool,
    pub after_text: Option<String>,
    pub before_text: Option<String>,
}

/// A range within a document where a decoration applies.
#[derive(Debug, Clone, PartialEq)]
pub struct DecorationRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub hover_message: Option<String>,
}

/// A set of decoration ranges applied to a specific document.
#[derive(Debug, Clone)]
pub struct DecorationSet {
    pub type_id: String,
    pub uri: String,
    pub ranges: Vec<DecorationRange>,
}

/// Service for managing decorations across documents.
pub struct DecorationService {
    pub types: Vec<DecorationType>,
    pub sets: Vec<DecorationSet>,
}

impl DecorationService {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            sets: Vec::new(),
        }
    }

    /// Register a new decoration type.
    pub fn register_type(&mut self, dt: DecorationType) {
        self.types.push(dt);
    }

    /// Set decorations for a given type and URI, replacing any existing set
    /// with the same type_id and uri.
    pub fn set_decorations(
        &mut self,
        type_id: String,
        uri: String,
        ranges: Vec<DecorationRange>,
    ) {
        self.sets
            .retain(|s| !(s.type_id == type_id && s.uri == uri));
        self.sets.push(DecorationSet {
            type_id,
            uri,
            ranges,
        });
    }

    /// Get all decoration sets for a given URI.
    pub fn get_decorations(&self, uri: &str) -> Vec<&DecorationSet> {
        self.sets.iter().filter(|s| s.uri == uri).collect()
    }

    /// Remove decorations for a specific type and URI.
    pub fn remove_decorations(&mut self, type_id: &str, uri: &str) {
        self.sets
            .retain(|s| !(s.type_id == type_id && s.uri == uri));
    }

    /// Clear all decoration types and sets.
    pub fn clear_all(&mut self) {
        self.types.clear();
        self.sets.clear();
    }

    pub fn get_type(&self, id: &str) -> Option<&DecorationType> {
        self.types.iter().find(|t| t.id == id)
    }

    pub fn has_type(&self, id: &str) -> bool {
        self.types.iter().any(|t| t.id == id)
    }

    pub fn unregister_type(&mut self, id: &str) -> bool {
        let len = self.types.len();
        self.types.retain(|t| t.id != id);
        self.sets.retain(|s| s.type_id != id);
        self.types.len() != len
    }

    pub fn get_all_uris(&self) -> Vec<&str> {
        let mut uris: Vec<&str> = self.sets.iter().map(|s| s.uri.as_str()).collect();
        uris.sort();
        uris.dedup();
        uris
    }

    pub fn decoration_count(&self) -> usize {
        self.sets.iter().map(|s| s.ranges.len()).sum()
    }
}

impl Default for DecorationService {
    fn default() -> Self {
        Self::new()
    }
}

/// Merge overlapping or adjacent ranges on the same line into combined ranges.
pub fn merge_ranges(mut ranges: Vec<DecorationRange>) -> Vec<DecorationRange> {
    if ranges.is_empty() {
        return ranges;
    }
    ranges.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.start_col.cmp(&b.start_col))
    });
    let mut merged: Vec<DecorationRange> = Vec::new();
    merged.push(ranges[0].clone());
    for r in ranges.into_iter().skip(1) {
        let last = merged.last_mut().unwrap();
        if r.start_line <= last.end_line && r.start_col <= last.end_col {
            if r.end_line > last.end_line || (r.end_line == last.end_line && r.end_col > last.end_col) {
                last.end_line = r.end_line;
                last.end_col = r.end_col;
            }
        } else {
            merged.push(r);
        }
    }
    merged
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeBehavior {
    OpenOpen,
    ClosedClosed,
    OpenClosed,
    ClosedOpen,
}

#[derive(Debug, Clone)]
pub struct DecorationRenderOptions {
    pub background_color: Option<String>,
    pub border_color: Option<String>,
    pub border_width: Option<String>,
    pub border_style: Option<String>,
    pub font_weight: Option<String>,
    pub font_style: Option<String>,
    pub opacity: Option<f32>,
    pub range_behavior: RangeBehavior,
}

impl Default for DecorationRenderOptions {
    fn default() -> Self {
        Self {
            background_color: None,
            border_color: None,
            border_width: None,
            border_style: None,
            font_weight: None,
            font_style: None,
            opacity: None,
            range_behavior: RangeBehavior::OpenOpen,
        }
    }
}

pub trait DecorationProvider {
    fn provide_decorations(&self, uri: &str) -> Vec<DecorationRange>;

    fn event_uri_filter(&self) -> Option<Vec<String>> {
        None
    }
}

use std::fmt;

/// Priority level for decorations, ordered from Low to Critical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DecorationPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// A decoration range paired with a priority and type identifier.
#[derive(Debug, Clone)]
pub struct PrioritizedDecoration {
    pub range: DecorationRange,
    pub priority: DecorationPriority,
    pub type_id: String,
}

/// Utility struct with static methods for sorting and filtering decoration ranges.
pub struct DecorationSorter;

impl DecorationSorter {
    /// Sort ranges by start_line, then by start_col.
    pub fn sort_by_line(ranges: &mut Vec<DecorationRange>) {
        ranges.sort_by(|a, b| {
            a.start_line
                .cmp(&b.start_line)
                .then(a.start_col.cmp(&b.start_col))
        });
    }

    /// Sort prioritized decorations by priority descending (Critical first).
    pub fn sort_by_priority(items: &mut Vec<PrioritizedDecoration>) {
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Return ranges that overlap with the inclusive line range [start, end].
    pub fn filter_by_line_range(
        ranges: &[DecorationRange],
        start: u32,
        end: u32,
    ) -> Vec<DecorationRange> {
        ranges
            .iter()
            .filter(|r| r.start_line <= end && r.end_line >= start)
            .cloned()
            .collect()
    }

    /// Count how many decoration ranges touch each line.
    pub fn count_by_line(ranges: &[DecorationRange]) -> std::collections::HashMap<u32, usize> {
        let mut counts = std::collections::HashMap::new();
        for r in ranges {
            for line in r.start_line..=r.end_line {
                *counts.entry(line).or_insert(0) += 1;
            }
        }
        counts
    }
}

impl fmt::Display for DecorationRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "L{}:{}-L{}:{}",
            self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
}

impl fmt::Display for DecorationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DecorationType({})", self.id)
    }
}

impl DecorationService {
    /// Get a slice of all registered decoration types.
    pub fn get_types(&self) -> &[DecorationType] {
        &self.types
    }

    /// Return the number of registered decoration types.
    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    /// Return the number of decoration sets.
    pub fn set_count(&self) -> usize {
        self.sets.len()
    }

    /// Get all decoration sets whose type_id matches `type_id`.
    pub fn get_decorations_by_type(&self, type_id: &str) -> Vec<&DecorationSet> {
        self.sets.iter().filter(|s| s.type_id == type_id).collect()
    }

    /// Remove all decoration sets for the given URI.
    pub fn remove_all_for_uri(&mut self, uri: &str) {
        self.sets.retain(|s| s.uri != uri);
    }

    /// Check whether any decoration set targets the given URI.
    pub fn has_decorations(&self, uri: &str) -> bool {
        self.sets.iter().any(|s| s.uri == uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_type(id: &str) -> DecorationType {
        DecorationType {
            id: id.to_string(),
            background_color: None,
            border: None,
            outline: None,
            gutter_icon: None,
            is_whole_line: false,
            after_text: None,
            before_text: None,
        }
    }

    fn sample_range(line: u32) -> DecorationRange {
        DecorationRange {
            start_line: line,
            start_col: 0,
            end_line: line,
            end_col: 10,
            hover_message: None,
        }
    }

    fn sample_range_cols(line: u32, start: u32, end: u32) -> DecorationRange {
        DecorationRange {
            start_line: line,
            start_col: start,
            end_line: line,
            end_col: end,
            hover_message: None,
        }
    }

    #[test]
    fn register_and_set_decorations() {
        let mut svc = DecorationService::new();
        svc.register_type(sample_type("highlight"));
        svc.set_decorations(
            "highlight".into(),
            "file:///a.rs".into(),
            vec![sample_range(1), sample_range(5)],
        );

        let decs = svc.get_decorations("file:///a.rs");
        assert_eq!(decs.len(), 1);
        assert_eq!(decs[0].ranges.len(), 2);
        assert_eq!(decs[0].ranges[0].start_line, 1);
    }

    #[test]
    fn remove_decorations() {
        let mut svc = DecorationService::new();
        svc.set_decorations(
            "err".into(),
            "file:///b.rs".into(),
            vec![sample_range(3)],
        );
        assert_eq!(svc.get_decorations("file:///b.rs").len(), 1);

        svc.remove_decorations("err", "file:///b.rs");
        assert!(svc.get_decorations("file:///b.rs").is_empty());
    }

    #[test]
    fn clear_all() {
        let mut svc = DecorationService::new();
        svc.register_type(sample_type("a"));
        svc.register_type(sample_type("b"));
        svc.set_decorations("a".into(), "file:///x.rs".into(), vec![sample_range(1)]);
        svc.set_decorations("b".into(), "file:///y.rs".into(), vec![sample_range(2)]);

        svc.clear_all();
        assert!(svc.types.is_empty());
        assert!(svc.sets.is_empty());
    }

    #[test]
    fn set_decorations_replaces_existing() {
        let mut svc = DecorationService::new();
        svc.set_decorations("t".into(), "file:///c.rs".into(), vec![sample_range(1)]);
        svc.set_decorations("t".into(), "file:///c.rs".into(), vec![sample_range(9)]);

        let decs = svc.get_decorations("file:///c.rs");
        assert_eq!(decs.len(), 1);
        assert_eq!(decs[0].ranges[0].start_line, 9);
    }

    #[test]
    fn get_type_and_has_type() {
        let mut svc = DecorationService::new();
        svc.register_type(sample_type("err"));
        assert!(svc.has_type("err"));
        assert!(!svc.has_type("warn"));
        assert_eq!(svc.get_type("err").unwrap().id, "err");
        assert!(svc.get_type("warn").is_none());
    }

    #[test]
    fn unregister_type_removes_sets() {
        let mut svc = DecorationService::new();
        svc.register_type(sample_type("err"));
        svc.set_decorations("err".into(), "file:///a.rs".into(), vec![sample_range(1)]);
        assert!(svc.unregister_type("err"));
        assert!(!svc.has_type("err"));
        assert!(svc.get_decorations("file:///a.rs").is_empty());
        assert!(!svc.unregister_type("err"));
    }

    #[test]
    fn get_all_uris_deduplicates() {
        let mut svc = DecorationService::new();
        svc.set_decorations("a".into(), "file:///x.rs".into(), vec![sample_range(1)]);
        svc.set_decorations("b".into(), "file:///x.rs".into(), vec![sample_range(2)]);
        svc.set_decorations("a".into(), "file:///y.rs".into(), vec![sample_range(3)]);
        let uris = svc.get_all_uris();
        assert_eq!(uris.len(), 2);
        assert!(uris.contains(&"file:///x.rs"));
        assert!(uris.contains(&"file:///y.rs"));
    }

    #[test]
    fn decoration_count_sums_ranges() {
        let mut svc = DecorationService::new();
        svc.set_decorations("a".into(), "file:///a.rs".into(), vec![sample_range(1), sample_range(2)]);
        svc.set_decorations("b".into(), "file:///b.rs".into(), vec![sample_range(3)]);
        assert_eq!(svc.decoration_count(), 3);
    }

    #[test]
    fn merge_ranges_combines_overlapping() {
        let ranges = vec![
            sample_range_cols(1, 0, 5),
            sample_range_cols(1, 3, 8),
            sample_range_cols(1, 10, 15),
        ];
        let merged = merge_ranges(ranges);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].start_col, 0);
        assert_eq!(merged[0].end_col, 8);
        assert_eq!(merged[1].start_col, 10);
    }

    #[test]
    fn merge_ranges_empty() {
        let merged = merge_ranges(vec![]);
        assert!(merged.is_empty());
    }

    #[test]
    fn decoration_render_options_default() {
        let opts = DecorationRenderOptions::default();
        assert!(opts.background_color.is_none());
        assert_eq!(opts.range_behavior, RangeBehavior::OpenOpen);
        assert!(opts.opacity.is_none());
    }

    #[test]
    fn range_behavior_variants() {
        assert_ne!(RangeBehavior::OpenOpen, RangeBehavior::ClosedClosed);
        assert_ne!(RangeBehavior::OpenClosed, RangeBehavior::ClosedOpen);
    }

    #[test]
    fn decoration_provider_default_filter() {
        struct TestProvider;
        impl DecorationProvider for TestProvider {
            fn provide_decorations(&self, _uri: &str) -> Vec<DecorationRange> {
                vec![sample_range(1)]
            }
        }
        let provider = TestProvider;
        assert!(provider.event_uri_filter().is_none());
        assert_eq!(provider.provide_decorations("file:///a.rs").len(), 1);
    }

    #[test]
    fn test_decoration_priority_ordering() {
        assert!(DecorationPriority::Low < DecorationPriority::Normal);
        assert!(DecorationPriority::Normal < DecorationPriority::High);
        assert!(DecorationPriority::High < DecorationPriority::Critical);
        assert!(DecorationPriority::Low < DecorationPriority::Critical);
    }

    #[test]
    fn test_sort_by_line() {
        let mut ranges = vec![
            sample_range_cols(5, 3, 10),
            sample_range_cols(1, 0, 5),
            sample_range_cols(1, 2, 8),
            sample_range_cols(3, 0, 4),
        ];
        DecorationSorter::sort_by_line(&mut ranges);
        assert_eq!(ranges[0].start_line, 1);
        assert_eq!(ranges[0].start_col, 0);
        assert_eq!(ranges[1].start_line, 1);
        assert_eq!(ranges[1].start_col, 2);
        assert_eq!(ranges[2].start_line, 3);
        assert_eq!(ranges[3].start_line, 5);
    }

    #[test]
    fn test_sort_by_priority() {
        let mut items = vec![
            PrioritizedDecoration {
                range: sample_range(1),
                priority: DecorationPriority::Low,
                type_id: "a".into(),
            },
            PrioritizedDecoration {
                range: sample_range(2),
                priority: DecorationPriority::Critical,
                type_id: "b".into(),
            },
            PrioritizedDecoration {
                range: sample_range(3),
                priority: DecorationPriority::Normal,
                type_id: "c".into(),
            },
        ];
        DecorationSorter::sort_by_priority(&mut items);
        assert_eq!(items[0].priority, DecorationPriority::Critical);
        assert_eq!(items[1].priority, DecorationPriority::Normal);
        assert_eq!(items[2].priority, DecorationPriority::Low);
    }

    #[test]
    fn test_filter_by_line_range() {
        let ranges = vec![
            sample_range(1),
            sample_range(5),
            sample_range(10),
            sample_range(15),
        ];
        let filtered = DecorationSorter::filter_by_line_range(&ranges, 4, 11);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].start_line, 5);
        assert_eq!(filtered[1].start_line, 10);
    }

    #[test]
    fn test_count_by_line() {
        let ranges = vec![
            sample_range(1),
            sample_range(1),
            sample_range(3),
        ];
        let counts = DecorationSorter::count_by_line(&ranges);
        assert_eq!(counts[&1], 2);
        assert_eq!(counts[&3], 1);
        assert!(!counts.contains_key(&2));
    }

    #[test]
    fn test_decoration_range_display() {
        let r = DecorationRange {
            start_line: 10,
            start_col: 5,
            end_line: 12,
            end_col: 20,
            hover_message: None,
        };
        assert_eq!(format!("{}", r), "L10:5-L12:20");
    }

    #[test]
    fn test_decoration_type_display() {
        let dt = sample_type("error");
        assert_eq!(format!("{}", dt), "DecorationType(error)");
    }

    #[test]
    fn test_get_types() {
        let mut svc = DecorationService::new();
        svc.register_type(sample_type("a"));
        svc.register_type(sample_type("b"));
        let types = svc.get_types();
        assert_eq!(types.len(), 2);
        assert_eq!(types[0].id, "a");
        assert_eq!(types[1].id, "b");
    }

    #[test]
    fn test_type_count_and_set_count() {
        let mut svc = DecorationService::new();
        assert_eq!(svc.type_count(), 0);
        assert_eq!(svc.set_count(), 0);
        svc.register_type(sample_type("x"));
        svc.register_type(sample_type("y"));
        svc.set_decorations("x".into(), "file:///a.rs".into(), vec![sample_range(1)]);
        assert_eq!(svc.type_count(), 2);
        assert_eq!(svc.set_count(), 1);
    }

    #[test]
    fn test_get_decorations_by_type() {
        let mut svc = DecorationService::new();
        svc.set_decorations("err".into(), "file:///a.rs".into(), vec![sample_range(1)]);
        svc.set_decorations("warn".into(), "file:///a.rs".into(), vec![sample_range(2)]);
        svc.set_decorations("err".into(), "file:///b.rs".into(), vec![sample_range(3)]);
        let err_sets = svc.get_decorations_by_type("err");
        assert_eq!(err_sets.len(), 2);
        let warn_sets = svc.get_decorations_by_type("warn");
        assert_eq!(warn_sets.len(), 1);
    }

    #[test]
    fn test_remove_all_for_uri() {
        let mut svc = DecorationService::new();
        svc.set_decorations("a".into(), "file:///x.rs".into(), vec![sample_range(1)]);
        svc.set_decorations("b".into(), "file:///x.rs".into(), vec![sample_range(2)]);
        svc.set_decorations("a".into(), "file:///y.rs".into(), vec![sample_range(3)]);
        svc.remove_all_for_uri("file:///x.rs");
        assert!(svc.get_decorations("file:///x.rs").is_empty());
        assert_eq!(svc.get_decorations("file:///y.rs").len(), 1);
    }

    #[test]
    fn test_has_decorations() {
        let mut svc = DecorationService::new();
        assert!(!svc.has_decorations("file:///a.rs"));
        svc.set_decorations("t".into(), "file:///a.rs".into(), vec![sample_range(1)]);
        assert!(svc.has_decorations("file:///a.rs"));
        assert!(!svc.has_decorations("file:///b.rs"));
    }

    #[test]
    fn test_decoration_range_partial_eq() {
        let r1 = sample_range_cols(1, 0, 10);
        let r2 = sample_range_cols(1, 0, 10);
        let r3 = sample_range_cols(2, 0, 10);
        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
    }
}
