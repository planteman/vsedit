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
#[derive(Debug, Clone)]
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
}
