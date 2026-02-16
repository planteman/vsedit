//! Problems / markers view panel.
//!
//! Collects diagnostics (errors, warnings, etc.) and exposes query methods
//! used by the problems panel UI.

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Severity levels for diagnostics, ordered from most to least severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkerSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl PartialOrd for MarkerSeverity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MarkerSeverity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        fn rank(s: &MarkerSeverity) -> u8 {
            match s {
                MarkerSeverity::Error => 0,
                MarkerSeverity::Warning => 1,
                MarkerSeverity::Info => 2,
                MarkerSeverity::Hint => 3,
            }
        }
        rank(self).cmp(&rank(other))
    }
}

/// Tag that modifies how a diagnostic is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkerTag {
    Unnecessary,
    Deprecated,
}

/// Information related to a diagnostic in another resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedInformation {
    pub uri: String,
    pub message: String,
    pub line: u32,
    pub col: u32,
}

/// A single diagnostic marker attached to a document range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub uri: String,
    pub message: String,
    pub severity: MarkerSeverity,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub source: Option<String>,
    pub code: Option<String>,
    pub tags: Vec<MarkerTag>,
    pub related_information: Vec<RelatedInformation>,
}

// ---------------------------------------------------------------------------
// Filter & Statistics
// ---------------------------------------------------------------------------

/// Filter criteria for querying markers.
#[derive(Debug, Clone, Default)]
pub struct MarkerFilter {
    pub severity: Option<MarkerSeverity>,
    pub source: Option<String>,
    pub uri_pattern: Option<String>,
}

impl MarkerFilter {
    /// Returns `true` if `marker` satisfies every set filter criterion.
    pub fn matches(&self, marker: &Marker) -> bool {
        if let Some(ref sev) = self.severity {
            if marker.severity != *sev {
                return false;
            }
        }
        if let Some(ref src) = self.source {
            match &marker.source {
                Some(ms) if ms == src => {}
                _ => return false,
            }
        }
        if let Some(ref pat) = self.uri_pattern {
            if !marker.uri.contains(pat.as_str()) {
                return false;
            }
        }
        true
    }
}

/// Aggregate counts per severity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerStats {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub hints: usize,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Collects and queries diagnostic markers across all open documents.
#[derive(Debug, Clone, Default)]
pub struct MarkersService {
    pub markers: Vec<Marker>,
}

impl MarkersService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_marker(&mut self, marker: Marker) {
        self.markers.push(marker);
    }

    /// Remove all markers associated with `uri`.
    pub fn remove_markers_for(&mut self, uri: &str) {
        self.markers.retain(|m| m.uri != uri);
    }

    /// Get all markers for a given URI.
    pub fn get_markers(&self, uri: &str) -> Vec<&Marker> {
        self.markers.iter().filter(|m| m.uri == uri).collect()
    }

    /// Get all markers matching the given severity.
    pub fn get_all_by_severity(&self, severity: MarkerSeverity) -> Vec<&Marker> {
        self.markers.iter().filter(|m| m.severity == severity).collect()
    }

    pub fn error_count(&self) -> usize {
        self.markers.iter().filter(|m| m.severity == MarkerSeverity::Error).count()
    }

    pub fn warning_count(&self) -> usize {
        self.markers
            .iter()
            .filter(|m| m.severity == MarkerSeverity::Warning)
            .count()
    }

    pub fn clear_all(&mut self) {
        self.markers.clear();
    }

    /// Return markers that satisfy the given filter.
    pub fn get_filtered(&self, filter: &MarkerFilter) -> Vec<&Marker> {
        self.markers.iter().filter(|m| filter.matches(m)).collect()
    }

    /// Compute aggregate statistics across all stored markers.
    pub fn get_stats(&self) -> MarkerStats {
        let mut stats = MarkerStats { errors: 0, warnings: 0, infos: 0, hints: 0 };
        for m in &self.markers {
            match m.severity {
                MarkerSeverity::Error => stats.errors += 1,
                MarkerSeverity::Warning => stats.warnings += 1,
                MarkerSeverity::Info => stats.infos += 1,
                MarkerSeverity::Hint => stats.hints += 1,
            }
        }
        stats
    }

    /// Return the deduplicated set of source identifiers present in the markers.
    pub fn get_unique_sources(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for m in &self.markers {
            if let Some(ref s) = m.source {
                if seen.insert(s.as_str()) {
                    result.push(s.as_str());
                }
            }
        }
        result
    }

    /// Return markers for `uri` whose line range overlaps `[start_line, end_line]`.
    pub fn get_markers_in_range(
        &self,
        uri: &str,
        start_line: u32,
        end_line: u32,
    ) -> Vec<&Marker> {
        self.markers
            .iter()
            .filter(|m| {
                m.uri == uri && m.start_line <= end_line && m.end_line >= start_line
            })
            .collect()
    }

    /// Sort markers by URI, then severity (most severe first), then start line.
    pub fn sort_markers(&mut self) {
        self.markers.sort_by(|a, b| {
            a.uri
                .cmp(&b.uri)
                .then(a.severity.cmp(&b.severity))
                .then(a.start_line.cmp(&b.start_line))
        });
    }
}

// ---------------------------------------------------------------------------
// MarkerProvider trait
// ---------------------------------------------------------------------------

/// A provider that can report diagnostics for a set of resources.
pub trait MarkerProvider {
    /// Human-readable name for this provider (e.g. "rustc", "clippy").
    fn name(&self) -> &str;

    /// Provide markers for all known resources.
    fn provide_markers(&self) -> Vec<Marker>;

    /// Provide markers for a single URI. Defaults to filtering `provide_markers`.
    fn provide_markers_for(&self, uri: &str) -> Vec<Marker> {
        self.provide_markers()
            .into_iter()
            .filter(|m| m.uri == uri)
            .collect()
    }

    /// Return the set of URIs this provider has diagnostics for.
    fn known_uris(&self) -> Vec<String> {
        let mut uris: Vec<String> = self
            .provide_markers()
            .iter()
            .map(|m| m.uri.clone())
            .collect();
        uris.sort();
        uris.dedup();
        uris
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_marker(uri: &str, severity: MarkerSeverity, msg: &str) -> Marker {
        Marker {
            uri: uri.to_string(),
            message: msg.to_string(),
            severity,
            start_line: 1,
            start_col: 0,
            end_line: 1,
            end_col: 10,
            source: None,
            code: None,
            tags: vec![],
            related_information: vec![],
        }
    }

    fn make_marker_ext(
        uri: &str,
        severity: MarkerSeverity,
        msg: &str,
        start_line: u32,
        source: Option<&str>,
    ) -> Marker {
        Marker {
            uri: uri.to_string(),
            message: msg.to_string(),
            severity,
            start_line,
            start_col: 0,
            end_line: start_line,
            end_col: 10,
            source: source.map(|s| s.to_string()),
            code: None,
            tags: vec![],
            related_information: vec![],
        }
    }

    // ---- original tests (unchanged logic) ----

    #[test]
    fn add_and_query_markers() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e1"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Warning, "w1"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Info, "i1"));
        assert_eq!(svc.get_markers("a.rs").len(), 2);
        assert_eq!(svc.get_markers("b.rs").len(), 1);
    }

    #[test]
    fn error_and_warning_counts() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e1"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e2"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Warning, "w1"));
        assert_eq!(svc.error_count(), 2);
        assert_eq!(svc.warning_count(), 1);
    }

    #[test]
    fn remove_markers_for_uri() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e1"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Error, "e2"));
        svc.remove_markers_for("a.rs");
        assert!(svc.get_markers("a.rs").is_empty());
        assert_eq!(svc.get_markers("b.rs").len(), 1);
    }

    #[test]
    fn severity_ordering() {
        assert!(MarkerSeverity::Error < MarkerSeverity::Warning);
        assert!(MarkerSeverity::Warning < MarkerSeverity::Info);
        assert!(MarkerSeverity::Info < MarkerSeverity::Hint);
    }

    // ---- new tests ----

    #[test]
    fn marker_tags_and_related_info() {
        let marker = Marker {
            uri: "a.rs".into(),
            message: "unused import".into(),
            severity: MarkerSeverity::Warning,
            start_line: 3,
            start_col: 0,
            end_line: 3,
            end_col: 15,
            source: Some("rustc".into()),
            code: Some("W001".into()),
            tags: vec![MarkerTag::Unnecessary],
            related_information: vec![RelatedInformation {
                uri: "b.rs".into(),
                message: "defined here".into(),
                line: 10,
                col: 5,
            }],
        };
        assert_eq!(marker.tags, vec![MarkerTag::Unnecessary]);
        assert_eq!(marker.related_information.len(), 1);
        assert_eq!(marker.related_information[0].uri, "b.rs");
        assert_eq!(marker.related_information[0].line, 10);
    }

    #[test]
    fn filter_matches_by_severity() {
        let m = make_marker("a.rs", MarkerSeverity::Error, "e");
        let filter = MarkerFilter { severity: Some(MarkerSeverity::Error), ..Default::default() };
        assert!(filter.matches(&m));

        let filter2 = MarkerFilter { severity: Some(MarkerSeverity::Warning), ..Default::default() };
        assert!(!filter2.matches(&m));
    }

    #[test]
    fn filter_matches_by_source() {
        let m = make_marker_ext("a.rs", MarkerSeverity::Error, "e", 1, Some("clippy"));
        let filter = MarkerFilter { source: Some("clippy".into()), ..Default::default() };
        assert!(filter.matches(&m));

        let filter2 = MarkerFilter { source: Some("rustc".into()), ..Default::default() };
        assert!(!filter2.matches(&m));

        // No source on marker should not match a source filter.
        let m2 = make_marker("a.rs", MarkerSeverity::Error, "e");
        assert!(!filter.matches(&m2));
    }

    #[test]
    fn filter_matches_by_uri_pattern() {
        let m = make_marker("src/lib.rs", MarkerSeverity::Error, "e");
        let filter = MarkerFilter { uri_pattern: Some("src/".into()), ..Default::default() };
        assert!(filter.matches(&m));

        let filter2 = MarkerFilter { uri_pattern: Some("tests/".into()), ..Default::default() };
        assert!(!filter2.matches(&m));
    }

    #[test]
    fn filter_combined_criteria() {
        let m = make_marker_ext("src/lib.rs", MarkerSeverity::Warning, "w", 5, Some("clippy"));
        let filter = MarkerFilter {
            severity: Some(MarkerSeverity::Warning),
            source: Some("clippy".into()),
            uri_pattern: Some("src/".into()),
        };
        assert!(filter.matches(&m));

        // Fails when one criterion does not match.
        let filter2 = MarkerFilter {
            severity: Some(MarkerSeverity::Error),
            source: Some("clippy".into()),
            uri_pattern: Some("src/".into()),
        };
        assert!(!filter2.matches(&m));
    }

    #[test]
    fn get_filtered_returns_matching() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker_ext("a.rs", MarkerSeverity::Error, "e1", 1, Some("rustc")));
        svc.add_marker(make_marker_ext("b.rs", MarkerSeverity::Warning, "w1", 2, Some("clippy")));
        svc.add_marker(make_marker_ext("a.rs", MarkerSeverity::Warning, "w2", 3, Some("rustc")));

        let filter = MarkerFilter { source: Some("rustc".into()), ..Default::default() };
        let results = svc.get_filtered(&filter);
        assert_eq!(results.len(), 2);

        let filter2 = MarkerFilter {
            severity: Some(MarkerSeverity::Warning),
            source: Some("rustc".into()),
            ..Default::default()
        };
        assert_eq!(svc.get_filtered(&filter2).len(), 1);
    }

    #[test]
    fn get_stats_counts_correctly() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e1"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e2"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Warning, "w1"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Info, "i1"));
        svc.add_marker(make_marker("c.rs", MarkerSeverity::Hint, "h1"));
        let stats = svc.get_stats();
        assert_eq!(stats, MarkerStats { errors: 2, warnings: 1, infos: 1, hints: 1 });
    }

    #[test]
    fn get_unique_sources_deduplicates() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker_ext("a.rs", MarkerSeverity::Error, "e1", 1, Some("rustc")));
        svc.add_marker(make_marker_ext("a.rs", MarkerSeverity::Error, "e2", 2, Some("rustc")));
        svc.add_marker(make_marker_ext("b.rs", MarkerSeverity::Warning, "w1", 1, Some("clippy")));
        svc.add_marker(make_marker("c.rs", MarkerSeverity::Info, "i1")); // no source
        let sources = svc.get_unique_sources();
        assert_eq!(sources.len(), 2);
        assert!(sources.contains(&"rustc"));
        assert!(sources.contains(&"clippy"));
    }

    #[test]
    fn get_markers_in_range_overlapping() {
        let mut svc = MarkersService::new();
        // Marker spanning lines 5-10
        svc.add_marker(Marker {
            uri: "a.rs".into(),
            message: "m1".into(),
            severity: MarkerSeverity::Error,
            start_line: 5,
            start_col: 0,
            end_line: 10,
            end_col: 5,
            source: None,
            code: None,
            tags: vec![],
            related_information: vec![],
        });
        // Marker at line 15
        svc.add_marker(make_marker_ext("a.rs", MarkerSeverity::Warning, "m2", 15, None));
        // Marker at line 8 in different file
        svc.add_marker(make_marker_ext("b.rs", MarkerSeverity::Error, "m3", 8, None));

        // Range 7-12 overlaps with m1 (5-10) only in a.rs
        let result = svc.get_markers_in_range("a.rs", 7, 12);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message, "m1");

        // Range 1-20 gets both a.rs markers
        assert_eq!(svc.get_markers_in_range("a.rs", 1, 20).len(), 2);

        // Range 11-14 gets nothing in a.rs
        assert!(svc.get_markers_in_range("a.rs", 11, 14).is_empty());
    }

    #[test]
    fn sort_markers_orders_correctly() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker_ext("b.rs", MarkerSeverity::Warning, "w1", 5, None));
        svc.add_marker(make_marker_ext("a.rs", MarkerSeverity::Warning, "w2", 1, None));
        svc.add_marker(make_marker_ext("a.rs", MarkerSeverity::Error, "e1", 10, None));
        svc.add_marker(make_marker_ext("a.rs", MarkerSeverity::Error, "e2", 3, None));

        svc.sort_markers();

        // a.rs errors first (by line), then a.rs warnings, then b.rs
        assert_eq!(svc.markers[0].message, "e2"); // a.rs Error line 3
        assert_eq!(svc.markers[1].message, "e1"); // a.rs Error line 10
        assert_eq!(svc.markers[2].message, "w2"); // a.rs Warning line 1
        assert_eq!(svc.markers[3].message, "w1"); // b.rs Warning line 5
    }

    #[test]
    fn marker_provider_trait_defaults() {
        struct TestProvider;
        impl MarkerProvider for TestProvider {
            fn name(&self) -> &str {
                "test-provider"
            }
            fn provide_markers(&self) -> Vec<Marker> {
                vec![
                    make_marker("x.rs", MarkerSeverity::Error, "e1"),
                    make_marker("x.rs", MarkerSeverity::Warning, "w1"),
                    make_marker("y.rs", MarkerSeverity::Info, "i1"),
                ]
            }
        }

        let p = TestProvider;
        assert_eq!(p.name(), "test-provider");
        assert_eq!(p.provide_markers_for("x.rs").len(), 2);
        assert!(p.provide_markers_for("z.rs").is_empty());
        let uris = p.known_uris();
        assert_eq!(uris, vec!["x.rs", "y.rs"]);
    }
}
