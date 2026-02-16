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
// Integration with vsedit-markers MarkerService
// ---------------------------------------------------------------------------

impl MarkersService {
    /// Import diagnostics from the core `vsedit_markers::MarkerService`.
    ///
    /// Reads all markers (no filter) and replaces the local store.
    pub fn import_from_marker_service(&mut self, service: &vsedit_markers::MarkerService) {
        let filter = vsedit_markers::MarkerFilter {
            owner: None,
            uri: None,
            severities: None,
            take: None,
        };
        let results = service.read(&filter);
        self.markers.clear();
        for (uri, data) in results {
            self.markers.push(Marker {
                uri: uri.to_string(),
                message: data.message,
                severity: convert_severity(data.severity),
                start_line: data.start_line,
                start_col: data.start_column,
                end_line: data.end_line,
                end_col: data.end_column,
                source: data.source,
                code: data.code.map(|c| match c {
                    vsedit_markers::MarkerCode::String(s) => s,
                    vsedit_markers::MarkerCode::Number(n) => n.to_string(),
                }),
                tags: data.tags.iter().map(|t| match t {
                    vsedit_markers::MarkerTag::Unnecessary => MarkerTag::Unnecessary,
                    vsedit_markers::MarkerTag::Deprecated => MarkerTag::Deprecated,
                }).collect(),
                related_information: data.related_information.iter().map(|r| {
                    RelatedInformation {
                        uri: r.uri.to_string(),
                        message: r.message.clone(),
                        line: r.start_line,
                        col: r.start_column,
                    }
                }).collect(),
            });
        }
    }

    /// Format a statusbar summary string like "✖ 2 ⚠ 3".
    pub fn statusbar_summary(&self) -> String {
        let stats = self.get_stats();
        format!("✖ {} ⚠ {} ℹ {} 💡 {}", stats.errors, stats.warnings, stats.infos, stats.hints)
    }

    /// Return the (uri, line, col) for the marker at the given index (for click-to-navigate).
    pub fn navigate_to(&self, index: usize) -> Option<(&str, u32, u32)> {
        self.markers.get(index).map(|m| (m.uri.as_str(), m.start_line, m.start_col))
    }

    /// Return all unique URIs that have markers.
    pub fn affected_uris(&self) -> Vec<&str> {
        let mut uris: Vec<&str> = self.markers.iter().map(|m| m.uri.as_str()).collect();
        uris.sort();
        uris.dedup();
        uris
    }

    /// Group markers by URI, sorting within each group by severity then line.
    pub fn grouped_by_uri(&self) -> Vec<(&str, Vec<&Marker>)> {
        let mut map: std::collections::BTreeMap<&str, Vec<&Marker>> = std::collections::BTreeMap::new();
        for m in &self.markers {
            map.entry(m.uri.as_str()).or_default().push(m);
        }
        for group in map.values_mut() {
            group.sort_by(|a, b| a.severity.cmp(&b.severity).then(a.start_line.cmp(&b.start_line)));
        }
        map.into_iter().collect()
    }
}

fn convert_severity(s: vsedit_markers::MarkerSeverity) -> MarkerSeverity {
    match s {
        vsedit_markers::MarkerSeverity::Error => MarkerSeverity::Error,
        vsedit_markers::MarkerSeverity::Warning => MarkerSeverity::Warning,
        vsedit_markers::MarkerSeverity::Info => MarkerSeverity::Info,
        vsedit_markers::MarkerSeverity::Hint => MarkerSeverity::Hint,
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

/// Statistics for markers grouped by severity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerSeverityStats {
    pub severity: MarkerSeverity,
    pub count: usize,
    pub affected_files: usize,
}

/// Computes per-severity statistics from the service.
pub fn compute_severity_stats(service: &MarkersService) -> Vec<MarkerSeverityStats> {
    let severities = [
        MarkerSeverity::Error,
        MarkerSeverity::Warning,
        MarkerSeverity::Info,
        MarkerSeverity::Hint,
    ];
    severities
        .iter()
        .map(|sev| {
            let matching: Vec<&Marker> = service.markers.iter().filter(|m| m.severity == *sev).collect();
            let mut files: Vec<&str> = matching.iter().map(|m| m.uri.as_str()).collect();
            files.sort();
            files.dedup();
            MarkerSeverityStats {
                severity: *sev,
                count: matching.len(),
                affected_files: files.len(),
            }
        })
        .collect()
}

/// Represents a group of markers for a single file.
#[derive(Debug, Clone)]
pub struct FileMarkerGroup<'a> {
    pub uri: &'a str,
    pub markers: Vec<&'a Marker>,
    pub error_count: usize,
    pub warning_count: usize,
}

/// Groups markers by file, computing per-file error/warning counts.
pub fn group_markers_by_file<'a>(service: &'a MarkersService) -> Vec<FileMarkerGroup<'a>> {
    let mut map: std::collections::BTreeMap<&str, Vec<&Marker>> = std::collections::BTreeMap::new();
    for m in &service.markers {
        map.entry(m.uri.as_str()).or_default().push(m);
    }
    map.into_iter()
        .map(|(uri, markers)| {
            let error_count = markers.iter().filter(|m| m.severity == MarkerSeverity::Error).count();
            let warning_count = markers.iter().filter(|m| m.severity == MarkerSeverity::Warning).count();
            FileMarkerGroup { uri, markers, error_count, warning_count }
        })
        .collect()
}

/// A pipeline of filters to apply sequentially.
#[derive(Debug, Clone, Default)]
pub struct MarkerFilterPipeline {
    pub filters: Vec<MarkerFilter>,
}

impl MarkerFilterPipeline {
    pub fn new() -> Self {
        Self { filters: Vec::new() }
    }

    pub fn add_filter(&mut self, filter: MarkerFilter) {
        self.filters.push(filter);
    }

    /// Apply all filters; a marker must pass every filter.
    pub fn apply<'a>(&self, markers: &'a [Marker]) -> Vec<&'a Marker> {
        markers
            .iter()
            .filter(|m| self.filters.iter().all(|f| f.matches(m)))
            .collect()
    }

    pub fn filter_count(&self) -> usize {
        self.filters.len()
    }
}

// ---------------------------------------------------------------------------
// Marker group summary
// ---------------------------------------------------------------------------

/// Counts of markers broken down by severity for a group of markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerGroupSummary {
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    pub hint_count: usize,
}

impl MarkerGroupSummary {
    /// Total number of markers across all severities.
    pub fn total(&self) -> usize {
        self.error_count + self.warning_count + self.info_count + self.hint_count
    }

    /// Returns the most severe level present, or `None` if empty.
    pub fn worst_severity(&self) -> Option<MarkerSeverity> {
        if self.error_count > 0 {
            Some(MarkerSeverity::Error)
        } else if self.warning_count > 0 {
            Some(MarkerSeverity::Warning)
        } else if self.info_count > 0 {
            Some(MarkerSeverity::Info)
        } else if self.hint_count > 0 {
            Some(MarkerSeverity::Hint)
        } else {
            None
        }
    }
}

/// Summarize a slice of markers by counting each severity level.
pub fn summarize_group(markers: &[Marker]) -> MarkerGroupSummary {
    let mut summary = MarkerGroupSummary {
        error_count: 0,
        warning_count: 0,
        info_count: 0,
        hint_count: 0,
    };
    for m in markers {
        match m.severity {
            MarkerSeverity::Error => summary.error_count += 1,
            MarkerSeverity::Warning => summary.warning_count += 1,
            MarkerSeverity::Info => summary.info_count += 1,
            MarkerSeverity::Hint => summary.hint_count += 1,
        }
    }
    summary
}

// ---------------------------------------------------------------------------
// MarkerNavigation
// ---------------------------------------------------------------------------

/// Navigates markers within a service, supporting next/prev by severity.
pub struct MarkerNavigation<'a> {
    service: &'a MarkersService,
    current_index: Option<usize>,
}

impl<'a> MarkerNavigation<'a> {
    pub fn new(service: &'a MarkersService) -> Self {
        Self {
            service,
            current_index: None,
        }
    }

    /// Find the next marker with `Error` severity after `current_index`.
    pub fn next_error(&mut self) -> Option<&'a Marker> {
        self.next_by_severity(MarkerSeverity::Error)
    }

    /// Find the previous marker with `Error` severity before `current_index`.
    pub fn prev_error(&mut self) -> Option<&'a Marker> {
        self.prev_by_severity(MarkerSeverity::Error)
    }

    /// Find the next marker with `Warning` severity after `current_index`.
    pub fn next_warning(&mut self) -> Option<&'a Marker> {
        self.next_by_severity(MarkerSeverity::Warning)
    }

    /// Find the next marker matching the given severity after `current_index`.
    pub fn next_by_severity(&mut self, severity: MarkerSeverity) -> Option<&'a Marker> {
        let start = match self.current_index {
            Some(i) => i + 1,
            None => 0,
        };
        for i in start..self.service.markers.len() {
            if self.service.markers[i].severity == severity {
                self.current_index = Some(i);
                return Some(&self.service.markers[i]);
            }
        }
        None
    }

    /// Find the previous marker matching the given severity before `current_index`.
    pub fn prev_by_severity(&mut self, severity: MarkerSeverity) -> Option<&'a Marker> {
        let end = match self.current_index {
            Some(0) | None => return None,
            Some(i) => i,
        };
        for i in (0..end).rev() {
            if self.service.markers[i].severity == severity {
                self.current_index = Some(i);
                return Some(&self.service.markers[i]);
            }
        }
        None
    }

    /// Reset navigation to the beginning.
    pub fn reset(&mut self) {
        self.current_index = None;
    }
}

// ---------------------------------------------------------------------------
// MarkerGrouping
// ---------------------------------------------------------------------------

/// Groups markers by a chosen criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupingCriterion {
    File,
    Severity,
    Source,
}

/// Group markers by the chosen criterion, returning `(group_key, markers)`
/// pairs sorted by key.
pub fn group_markers(markers: &[Marker], criterion: GroupingCriterion) -> Vec<(String, Vec<&Marker>)> {
    let mut map: std::collections::BTreeMap<String, Vec<&Marker>> =
        std::collections::BTreeMap::new();
    for m in markers {
        let key = match criterion {
            GroupingCriterion::File => m.uri.clone(),
            GroupingCriterion::Severity => match m.severity {
                MarkerSeverity::Error => "Error".to_string(),
                MarkerSeverity::Warning => "Warning".to_string(),
                MarkerSeverity::Info => "Info".to_string(),
                MarkerSeverity::Hint => "Hint".to_string(),
            },
            GroupingCriterion::Source => {
                m.source.clone().unwrap_or_else(|| "unknown".to_string())
            }
        };
        map.entry(key).or_default().push(m);
    }
    map.into_iter().collect()
}

// ---------------------------------------------------------------------------
// marker_summary
// ---------------------------------------------------------------------------

/// Returns a HashMap of severity -> count for the given markers.
pub fn marker_summary(markers: &[Marker]) -> std::collections::HashMap<MarkerSeverity, usize> {
    let mut counts = std::collections::HashMap::new();
    for m in markers {
        *counts.entry(m.severity).or_insert(0) += 1;
    }
    counts
}

// ---------------------------------------------------------------------------
// Convenience methods on MarkerSeverity
// ---------------------------------------------------------------------------

impl MarkerSeverity {
    /// Human-readable label for the severity level.
    pub fn label(&self) -> &'static str {
        match self {
            MarkerSeverity::Error => "error",
            MarkerSeverity::Warning => "warning",
            MarkerSeverity::Info => "info",
            MarkerSeverity::Hint => "hint",
        }
    }

    /// Returns `true` if this severity is `Error`.
    pub fn is_error(&self) -> bool {
        matches!(self, MarkerSeverity::Error)
    }

    /// Returns `true` if this severity is `Warning`.
    pub fn is_warning(&self) -> bool {
        matches!(self, MarkerSeverity::Warning)
    }
}

// ---------------------------------------------------------------------------
// Convenience methods on Marker
// ---------------------------------------------------------------------------

impl Marker {
    /// Returns `true` if this marker has `Error` severity.
    pub fn is_error(&self) -> bool {
        self.severity.is_error()
    }

    /// Returns `true` if the marker has a `source` set.
    pub fn has_source(&self) -> bool {
        self.source.is_some()
    }
}

// ---------------------------------------------------------------------------
// Convenience methods on MarkersService
// ---------------------------------------------------------------------------

impl MarkersService {
    /// Total number of markers across all URIs.
    pub fn total_count(&self) -> usize {
        self.markers.len()
    }

    /// Returns `true` if there are no markers at all.
    pub fn is_empty(&self) -> bool {
        self.markers.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Convenience methods on MarkerStats
// ---------------------------------------------------------------------------

impl MarkerStats {
    /// Returns `true` when the error count is greater than zero.
    pub fn has_errors(&self) -> bool {
        self.errors > 0
    }
}

impl std::fmt::Display for MarkerStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} errors, {} warnings, {} info, {} hints",
            self.errors, self.warnings, self.infos, self.hints
        )
    }
}

// ---------------------------------------------------------------------------
// MarkerQuickFix — quick fix suggestions for markers
// ---------------------------------------------------------------------------

/// A suggested fix for a diagnostic marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerQuickFix {
    /// Human-readable title of the fix (shown in UI).
    pub title: String,
    /// The URI of the file to edit.
    pub uri: String,
    /// Line where the fix should be applied.
    pub line: u32,
    /// Column where the fix should be applied.
    pub col: u32,
    /// Text to insert or replace.
    pub new_text: String,
    /// Whether the fix is "preferred" (auto-applicable).
    pub is_preferred: bool,
}

impl MarkerQuickFix {
    pub fn new(title: impl Into<String>, uri: impl Into<String>, line: u32, col: u32, new_text: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            uri: uri.into(),
            line,
            col,
            new_text: new_text.into(),
            is_preferred: false,
        }
    }

    pub fn preferred(mut self) -> Self {
        self.is_preferred = true;
        self
    }
}

impl std::fmt::Display for MarkerQuickFix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}:{}:{})", self.title, self.uri, self.line, self.col)
    }
}

// ---------------------------------------------------------------------------
// MarkerTrend — track marker trends over time
// ---------------------------------------------------------------------------

/// A snapshot of marker counts at a point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerSnapshot {
    pub errors: usize,
    pub warnings: usize,
    pub total: usize,
    pub timestamp_ms: u64,
}

/// Tracks marker count trends over successive snapshots.
#[derive(Debug, Clone, Default)]
pub struct MarkerTrend {
    snapshots: Vec<MarkerSnapshot>,
}

impl MarkerTrend {
    pub fn new() -> Self {
        Self { snapshots: Vec::new() }
    }

    /// Record a snapshot from the current service state.
    pub fn record(&mut self, service: &MarkersService, timestamp_ms: u64) {
        let stats = service.get_stats();
        self.snapshots.push(MarkerSnapshot {
            errors: stats.errors,
            warnings: stats.warnings,
            total: stats.errors + stats.warnings + stats.infos + stats.hints,
            timestamp_ms,
        });
    }

    /// Return all recorded snapshots.
    pub fn snapshots(&self) -> &[MarkerSnapshot] {
        &self.snapshots
    }

    /// Return the change in total markers between the last two snapshots.
    /// Positive means markers increased, negative means decreased.
    pub fn total_delta(&self) -> i64 {
        if self.snapshots.len() < 2 {
            return 0;
        }
        let last = &self.snapshots[self.snapshots.len() - 1];
        let prev = &self.snapshots[self.snapshots.len() - 2];
        last.total as i64 - prev.total as i64
    }

    /// Return the change in error count between the last two snapshots.
    pub fn error_delta(&self) -> i64 {
        if self.snapshots.len() < 2 {
            return 0;
        }
        let last = &self.snapshots[self.snapshots.len() - 1];
        let prev = &self.snapshots[self.snapshots.len() - 2];
        last.errors as i64 - prev.errors as i64
    }

    /// Return `true` if errors are trending downward over the last N snapshots.
    pub fn errors_improving(&self, window: usize) -> bool {
        if self.snapshots.len() < 2 || window < 2 {
            return false;
        }
        let start = self.snapshots.len().saturating_sub(window);
        let slice = &self.snapshots[start..];
        slice.windows(2).all(|w| w[1].errors <= w[0].errors)
    }

    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }
}

// ---------------------------------------------------------------------------
// MarkerDeduplicator — deduplicate similar markers
// ---------------------------------------------------------------------------

/// Deduplicates markers that have the same URI, message, and severity.
pub struct MarkerDeduplicator;

impl MarkerDeduplicator {
    /// Remove duplicate markers from a slice, returning only unique ones.
    /// Two markers are considered duplicates if they share the same URI,
    /// severity, message, start_line, and start_col.
    pub fn deduplicate(markers: &[Marker]) -> Vec<&Marker> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for m in markers {
            let key = (&m.uri, &m.message, m.severity, m.start_line, m.start_col);
            if seen.insert(key) {
                result.push(m);
            }
        }
        result
    }

    /// Count the number of duplicates that would be removed.
    pub fn duplicate_count(markers: &[Marker]) -> usize {
        markers.len() - Self::deduplicate(markers).len()
    }
}

// ---------------------------------------------------------------------------
// Bulk operations on MarkersService
// ---------------------------------------------------------------------------

impl MarkersService {
    /// Replace all markers for a given URI atomically.
    pub fn set_markers_for(&mut self, uri: &str, new_markers: Vec<Marker>) {
        self.remove_markers_for(uri);
        for m in new_markers {
            self.markers.push(m);
        }
    }

    /// Remove all markers matching a filter, returning how many were removed.
    pub fn remove_matching(&mut self, filter: &MarkerFilter) -> usize {
        let before = self.markers.len();
        self.markers.retain(|m| !filter.matches(m));
        before - self.markers.len()
    }

    /// Return a deduplicated view of all markers.
    pub fn deduplicated(&self) -> Vec<&Marker> {
        MarkerDeduplicator::deduplicate(&self.markers)
    }
}

// ---------------------------------------------------------------------------
// Marker analysis and query utilities
// ---------------------------------------------------------------------------

/// Return the total number of markers per unique URI.
pub fn marker_count_by_uri(markers: &[Marker]) -> std::collections::HashMap<String, usize> {
    let mut counts = std::collections::HashMap::new();
    for m in markers {
        *counts.entry(m.uri.clone()).or_insert(0) += 1;
    }
    counts
}

/// Return the URI with the most markers, or `None` if empty.
pub fn most_problematic_uri(markers: &[Marker]) -> Option<String> {
    let counts = marker_count_by_uri(markers);
    counts.into_iter().max_by_key(|(_, c)| *c).map(|(uri, _)| uri)
}

/// Return all markers that have at least one related information entry.
pub fn markers_with_related_info(markers: &[Marker]) -> Vec<&Marker> {
    markers
        .iter()
        .filter(|m| !m.related_information.is_empty())
        .collect()
}

/// Return all unique source identifiers across all markers.
pub fn marker_unique_sources(markers: &[Marker]) -> Vec<String> {
    let mut sources: Vec<String> = markers
        .iter()
        .filter_map(|m| m.source.clone())
        .collect();
    sources.sort();
    sources.dedup();
    sources
}

/// Return markers that span multiple lines (end_line > start_line).
pub fn multiline_markers(markers: &[Marker]) -> Vec<&Marker> {
    markers.iter().filter(|m| m.end_line > m.start_line).collect()
}

/// Return a human-readable one-line description of a marker for tooltip display.
pub fn marker_tooltip(marker: &Marker) -> String {
    let sev = match marker.severity {
        MarkerSeverity::Error => "Error",
        MarkerSeverity::Warning => "Warning",
        MarkerSeverity::Info => "Info",
        MarkerSeverity::Hint => "Hint",
    };
    let source_part = marker
        .source
        .as_deref()
        .map(|s| format!(" [{}]", s))
        .unwrap_or_default();
    format!("{}{}: {} ({}:{})", sev, source_part, marker.message, marker.start_line, marker.start_col)
}

/// Return true if a marker has any of the specified tags.
pub fn marker_has_any_tag(marker: &Marker, tags: &[MarkerTag]) -> bool {
    marker.tags.iter().any(|t| tags.contains(t))
}

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

    // -- Integration with vsedit-markers --

    #[test]
    fn import_from_marker_service() {
        use vsedit_markers::{MarkerService, MarkerData, MarkerSeverity as CoreSeverity};
        use vsedit_uri::VsUri;

        let core_svc = MarkerService::new();
        let uri = VsUri::file("/import.rs");
        core_svc.change_one("rustc", &uri, vec![
            MarkerData {
                severity: CoreSeverity::Error,
                message: "type error".into(),
                source: Some("rustc".into()),
                code: None,
                start_line: 10,
                start_column: 5,
                end_line: 10,
                end_column: 15,
                related_information: vec![],
                tags: vec![],
            },
            MarkerData {
                severity: CoreSeverity::Warning,
                message: "unused var".into(),
                source: Some("rustc".into()),
                code: Some(vsedit_markers::MarkerCode::String("W001".into())),
                start_line: 20,
                start_column: 1,
                end_line: 20,
                end_column: 5,
                related_information: vec![],
                tags: vec![vsedit_markers::MarkerTag::Unnecessary],
            },
        ]);

        let mut view_svc = MarkersService::new();
        view_svc.import_from_marker_service(&core_svc);

        assert_eq!(view_svc.error_count(), 1);
        assert_eq!(view_svc.warning_count(), 1);
        let stats = view_svc.get_stats();
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.warnings, 1);
    }

    #[test]
    fn statusbar_summary_format() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e2"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Warning, "w"));
        let summary = svc.statusbar_summary();
        assert!(summary.contains("✖ 2"));
        assert!(summary.contains("⚠ 1"));
    }

    #[test]
    fn navigate_to_returns_location() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker_ext("src/main.rs", MarkerSeverity::Error, "e", 42, Some("rustc")));
        let loc = svc.navigate_to(0).unwrap();
        assert_eq!(loc, ("src/main.rs", 42, 0));
        assert!(svc.navigate_to(99).is_none());
    }

    #[test]
    fn affected_uris_deduplicates() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e1"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e2"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Warning, "w1"));
        let uris = svc.affected_uris();
        assert_eq!(uris, vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn grouped_by_uri_sorts_by_severity() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker_ext("a.rs", MarkerSeverity::Warning, "w", 5, None));
        svc.add_marker(make_marker_ext("a.rs", MarkerSeverity::Error, "e", 1, None));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Info, "i"));
        let groups = svc.grouped_by_uri();
        assert_eq!(groups.len(), 2);
        // Within a.rs, error should come before warning
        assert_eq!(groups[0].0, "a.rs");
        assert_eq!(groups[0].1[0].severity, MarkerSeverity::Error);
        assert_eq!(groups[0].1[1].severity, MarkerSeverity::Warning);
    }

    #[test]
    fn severity_stats_computation() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e1"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Error, "e2"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Warning, "w1"));
        svc.add_marker(make_marker("c.rs", MarkerSeverity::Info, "i1"));
        let stats = compute_severity_stats(&svc);
        assert_eq!(stats[0].severity, MarkerSeverity::Error);
        assert_eq!(stats[0].count, 2);
        assert_eq!(stats[0].affected_files, 2);
        assert_eq!(stats[1].count, 1); // warning
    }

    #[test]
    fn group_markers_by_file_counts() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e1"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Warning, "w1"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Error, "e2"));
        let groups = group_markers_by_file(&svc);
        assert_eq!(groups.len(), 2);
        let a_group = groups.iter().find(|g| g.uri == "a.rs").unwrap();
        assert_eq!(a_group.error_count, 1);
        assert_eq!(a_group.warning_count, 1);
    }

    #[test]
    fn filter_pipeline_multiple_filters() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker_ext("src/a.rs", MarkerSeverity::Error, "e1", 1, Some("rustc")));
        svc.add_marker(make_marker_ext("src/b.rs", MarkerSeverity::Warning, "w1", 2, Some("rustc")));
        svc.add_marker(make_marker_ext("tests/c.rs", MarkerSeverity::Error, "e2", 3, Some("clippy")));
        let mut pipeline = MarkerFilterPipeline::new();
        pipeline.add_filter(MarkerFilter { severity: Some(MarkerSeverity::Error), ..Default::default() });
        pipeline.add_filter(MarkerFilter { uri_pattern: Some("src/".into()), ..Default::default() });
        let results = pipeline.apply(&svc.markers);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message, "e1");
    }

    #[test]
    fn filter_pipeline_empty_passes_all() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e1"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Warning, "w1"));
        let pipeline = MarkerFilterPipeline::new();
        assert_eq!(pipeline.filter_count(), 0);
        let results = pipeline.apply(&svc.markers);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn severity_stats_empty_service() {
        let svc = MarkersService::new();
        let stats = compute_severity_stats(&svc);
        assert_eq!(stats.len(), 4);
        for s in &stats {
            assert_eq!(s.count, 0);
            assert_eq!(s.affected_files, 0);
        }
    }

    // -- MarkerGroupSummary tests -----------------------------------------

    #[test]
    fn summarize_group_mixed_severities() {
        let markers = vec![
            make_marker("a.rs", MarkerSeverity::Error, "e1"),
            make_marker("a.rs", MarkerSeverity::Warning, "w1"),
            make_marker("a.rs", MarkerSeverity::Warning, "w2"),
            make_marker("a.rs", MarkerSeverity::Info, "i1"),
        ];
        let s = summarize_group(&markers);
        assert_eq!(s.error_count, 1);
        assert_eq!(s.warning_count, 2);
        assert_eq!(s.info_count, 1);
        assert_eq!(s.hint_count, 0);
        assert_eq!(s.total(), 4);
    }

    #[test]
    fn summarize_group_empty() {
        let s = summarize_group(&[]);
        assert_eq!(s.total(), 0);
        assert_eq!(s.worst_severity(), None);
    }

    #[test]
    fn summarize_group_worst_severity_error() {
        let markers = vec![
            make_marker("a.rs", MarkerSeverity::Warning, "w"),
            make_marker("a.rs", MarkerSeverity::Error, "e"),
        ];
        let s = summarize_group(&markers);
        assert_eq!(s.worst_severity(), Some(MarkerSeverity::Error));
    }

    #[test]
    fn summarize_group_worst_severity_warning_only() {
        let markers = vec![
            make_marker("a.rs", MarkerSeverity::Warning, "w"),
            make_marker("a.rs", MarkerSeverity::Hint, "h"),
        ];
        let s = summarize_group(&markers);
        assert_eq!(s.worst_severity(), Some(MarkerSeverity::Warning));
    }

    #[test]
    fn summarize_group_only_hints() {
        let markers = vec![
            make_marker("a.rs", MarkerSeverity::Hint, "h1"),
            make_marker("a.rs", MarkerSeverity::Hint, "h2"),
        ];
        let s = summarize_group(&markers);
        assert_eq!(s.hint_count, 2);
        assert_eq!(s.worst_severity(), Some(MarkerSeverity::Hint));
    }

    #[test]
    fn navigation_next_error() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Warning, "warn1"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "err1"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Info, "info1"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Error, "err2"));
        let mut nav = MarkerNavigation::new(&svc);
        let m = nav.next_error().unwrap();
        assert_eq!(m.message, "err1");
        let m2 = nav.next_error().unwrap();
        assert_eq!(m2.message, "err2");
        assert!(nav.next_error().is_none());
    }

    #[test]
    fn navigation_prev_error() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "err1"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Warning, "warn1"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Error, "err2"));
        let mut nav = MarkerNavigation::new(&svc);
        // Move to end first
        nav.next_error();
        nav.next_error();
        let m = nav.prev_error().unwrap();
        assert_eq!(m.message, "err1");
    }

    #[test]
    fn navigation_next_warning() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "err1"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Warning, "warn1"));
        let mut nav = MarkerNavigation::new(&svc);
        let m = nav.next_warning().unwrap();
        assert_eq!(m.message, "warn1");
    }

    #[test]
    fn navigation_reset() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "err1"));
        let mut nav = MarkerNavigation::new(&svc);
        nav.next_error();
        assert!(nav.next_error().is_none());
        nav.reset();
        assert!(nav.next_error().is_some());
    }

    #[test]
    fn group_markers_by_file_criterion() {
        let markers = vec![
            make_marker("a.rs", MarkerSeverity::Error, "e1"),
            make_marker("b.rs", MarkerSeverity::Warning, "w1"),
            make_marker("a.rs", MarkerSeverity::Warning, "w2"),
        ];
        let groups = group_markers(&markers, GroupingCriterion::File);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "a.rs");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, "b.rs");
    }

    #[test]
    fn group_markers_by_severity_criterion() {
        let markers = vec![
            make_marker("a.rs", MarkerSeverity::Error, "e1"),
            make_marker("b.rs", MarkerSeverity::Error, "e2"),
            make_marker("a.rs", MarkerSeverity::Warning, "w1"),
        ];
        let groups = group_markers(&markers, GroupingCriterion::Severity);
        assert!(groups.iter().any(|(k, v)| k == "Error" && v.len() == 2));
        assert!(groups.iter().any(|(k, v)| k == "Warning" && v.len() == 1));
    }

    #[test]
    fn group_markers_by_source_criterion() {
        let mut m1 = make_marker("a.rs", MarkerSeverity::Error, "e1");
        m1.source = Some("rustc".to_string());
        let mut m2 = make_marker("b.rs", MarkerSeverity::Warning, "w1");
        m2.source = Some("clippy".to_string());
        let mut m3 = make_marker("a.rs", MarkerSeverity::Info, "i1");
        m3.source = Some("rustc".to_string());
        let markers = vec![m1, m2, m3];
        let groups = group_markers(&markers, GroupingCriterion::Source);
        assert!(groups.iter().any(|(k, v)| k == "rustc" && v.len() == 2));
        assert!(groups.iter().any(|(k, v)| k == "clippy" && v.len() == 1));
    }

    #[test]
    fn marker_summary_counts() {
        let markers = vec![
            make_marker("a.rs", MarkerSeverity::Error, "e1"),
            make_marker("a.rs", MarkerSeverity::Error, "e2"),
            make_marker("b.rs", MarkerSeverity::Warning, "w1"),
            make_marker("c.rs", MarkerSeverity::Info, "i1"),
        ];
        let summary = marker_summary(&markers);
        assert_eq!(summary.get(&MarkerSeverity::Error), Some(&2));
        assert_eq!(summary.get(&MarkerSeverity::Warning), Some(&1));
        assert_eq!(summary.get(&MarkerSeverity::Info), Some(&1));
        assert_eq!(summary.get(&MarkerSeverity::Hint), None);
    }

    // -- New convenience-method tests -------------------------------------

    #[test]
    fn severity_label() {
        assert_eq!(MarkerSeverity::Error.label(), "error");
        assert_eq!(MarkerSeverity::Warning.label(), "warning");
        assert_eq!(MarkerSeverity::Info.label(), "info");
        assert_eq!(MarkerSeverity::Hint.label(), "hint");
    }

    #[test]
    fn severity_is_error_and_is_warning() {
        assert!(MarkerSeverity::Error.is_error());
        assert!(!MarkerSeverity::Error.is_warning());
        assert!(MarkerSeverity::Warning.is_warning());
        assert!(!MarkerSeverity::Warning.is_error());
        assert!(!MarkerSeverity::Info.is_error());
        assert!(!MarkerSeverity::Hint.is_warning());
    }

    #[test]
    fn marker_is_error_delegates_to_severity() {
        let err = make_marker("a.rs", MarkerSeverity::Error, "e");
        let warn = make_marker("a.rs", MarkerSeverity::Warning, "w");
        assert!(err.is_error());
        assert!(!warn.is_error());
    }

    #[test]
    fn marker_has_source() {
        let without = make_marker("a.rs", MarkerSeverity::Error, "e");
        assert!(!without.has_source());
        let with = make_marker_ext("a.rs", MarkerSeverity::Error, "e", 1, Some("rustc"));
        assert!(with.has_source());
    }

    #[test]
    fn service_total_count_and_is_empty() {
        let mut svc = MarkersService::new();
        assert!(svc.is_empty());
        assert_eq!(svc.total_count(), 0);
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e1"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Warning, "w1"));
        assert!(!svc.is_empty());
        assert_eq!(svc.total_count(), 2);
        svc.clear_all();
        assert!(svc.is_empty());
    }

    #[test]
    fn marker_stats_has_errors() {
        let stats_with = MarkerStats { errors: 3, warnings: 1, infos: 0, hints: 0 };
        assert!(stats_with.has_errors());
        let stats_without = MarkerStats { errors: 0, warnings: 2, infos: 1, hints: 0 };
        assert!(!stats_without.has_errors());
    }

    #[test]
    fn marker_stats_display() {
        let stats = MarkerStats { errors: 2, warnings: 3, infos: 1, hints: 4 };
        let text = format!("{}", stats);
        assert_eq!(text, "2 errors, 3 warnings, 1 info, 4 hints");

        let empty = MarkerStats { errors: 0, warnings: 0, infos: 0, hints: 0 };
        assert_eq!(format!("{}", empty), "0 errors, 0 warnings, 0 info, 0 hints");
    }

    // -- MarkerQuickFix tests -----------------------------------------------

    #[test]
    fn quick_fix_creation_and_display() {
        let fix = MarkerQuickFix::new("Add import", "file:///main.rs", 1, 0, "use std::io;");
        assert_eq!(fix.title, "Add import");
        assert!(!fix.is_preferred);
        let display = format!("{fix}");
        assert!(display.contains("Add import"));
        assert!(display.contains("main.rs"));
    }

    #[test]
    fn quick_fix_preferred() {
        let fix = MarkerQuickFix::new("Fix typo", "a.rs", 5, 3, "correct")
            .preferred();
        assert!(fix.is_preferred);
    }

    // -- MarkerTrend tests --------------------------------------------------

    #[test]
    fn trend_records_snapshots_and_deltas() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e1"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Warning, "w1"));

        let mut trend = MarkerTrend::new();
        trend.record(&svc, 100);
        assert_eq!(trend.snapshot_count(), 1);
        assert_eq!(trend.total_delta(), 0); // only one snapshot

        svc.add_marker(make_marker("b.rs", MarkerSeverity::Error, "e2"));
        trend.record(&svc, 200);
        assert_eq!(trend.total_delta(), 1); // 3 - 2
        assert_eq!(trend.error_delta(), 1); // 2 - 1
    }

    #[test]
    fn trend_errors_improving() {
        let mut trend = MarkerTrend::new();
        trend.snapshots.push(MarkerSnapshot { errors: 5, warnings: 0, total: 5, timestamp_ms: 0 });
        trend.snapshots.push(MarkerSnapshot { errors: 3, warnings: 0, total: 3, timestamp_ms: 100 });
        trend.snapshots.push(MarkerSnapshot { errors: 1, warnings: 0, total: 1, timestamp_ms: 200 });
        assert!(trend.errors_improving(3));

        trend.snapshots.push(MarkerSnapshot { errors: 4, warnings: 0, total: 4, timestamp_ms: 300 });
        assert!(!trend.errors_improving(3));
    }

    // -- MarkerDeduplicator tests -------------------------------------------

    #[test]
    fn deduplicator_removes_exact_duplicates() {
        let m1 = make_marker("a.rs", MarkerSeverity::Error, "err");
        let m2 = make_marker("a.rs", MarkerSeverity::Error, "err"); // duplicate
        let m3 = make_marker("a.rs", MarkerSeverity::Warning, "warn");
        let markers = vec![m1, m2, m3];
        let deduped = MarkerDeduplicator::deduplicate(&markers);
        assert_eq!(deduped.len(), 2);
        assert_eq!(MarkerDeduplicator::duplicate_count(&markers), 1);
    }

    // -- Bulk operations tests ----------------------------------------------

    #[test]
    fn set_markers_for_replaces_atomically() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "old1"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "old2"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Warning, "keep"));

        let new = vec![make_marker("a.rs", MarkerSeverity::Info, "new1")];
        svc.set_markers_for("a.rs", new);

        assert_eq!(svc.get_markers("a.rs").len(), 1);
        assert_eq!(svc.get_markers("a.rs")[0].message, "new1");
        assert_eq!(svc.get_markers("b.rs").len(), 1);
    }

    #[test]
    fn remove_matching_by_filter() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Error, "e"));
        svc.add_marker(make_marker("a.rs", MarkerSeverity::Warning, "w"));
        svc.add_marker(make_marker("b.rs", MarkerSeverity::Error, "e2"));

        let filter = MarkerFilter {
            severity: Some(MarkerSeverity::Error),
            source: None,
            uri_pattern: None,
        };
        let removed = svc.remove_matching(&filter);
        assert_eq!(removed, 2);
        assert_eq!(svc.total_count(), 1);
    }

    #[test]
    fn marker_count_by_uri_groups() {
        let markers = vec![
            make_marker("a.rs", MarkerSeverity::Error, "e1"),
            make_marker("a.rs", MarkerSeverity::Warning, "w1"),
            make_marker("b.rs", MarkerSeverity::Info, "i1"),
        ];
        let counts = marker_count_by_uri(&markers);
        assert_eq!(counts.get("a.rs"), Some(&2));
        assert_eq!(counts.get("b.rs"), Some(&1));
    }

    #[test]
    fn most_problematic_uri_finds_max() {
        let markers = vec![
            make_marker("a.rs", MarkerSeverity::Error, "e1"),
            make_marker("a.rs", MarkerSeverity::Error, "e2"),
            make_marker("b.rs", MarkerSeverity::Error, "e3"),
        ];
        assert_eq!(most_problematic_uri(&markers), Some("a.rs".to_string()));
    }

    #[test]
    fn most_problematic_uri_empty() {
        assert!(most_problematic_uri(&[]).is_none());
    }

    #[test]
    fn markers_with_related_info_filters() {
        let mut m1 = make_marker("a.rs", MarkerSeverity::Error, "e1");
        m1.related_information.push(RelatedInformation {
            uri: "b.rs".to_string(),
            message: "related".to_string(),
            line: 5,
            col: 0,
        });
        let m2 = make_marker("a.rs", MarkerSeverity::Warning, "w1");
        let markers = vec![m1, m2];
        let with_related = markers_with_related_info(&markers);
        assert_eq!(with_related.len(), 1);
        assert_eq!(with_related[0].message, "e1");
    }

    #[test]
    fn marker_unique_sources_deduplicates() {
        let mut m1 = make_marker("a.rs", MarkerSeverity::Error, "e1");
        m1.source = Some("rustc".to_string());
        let mut m2 = make_marker("b.rs", MarkerSeverity::Warning, "w1");
        m2.source = Some("clippy".to_string());
        let mut m3 = make_marker("c.rs", MarkerSeverity::Info, "i1");
        m3.source = Some("rustc".to_string());
        let sources = marker_unique_sources(&[m1, m2, m3]);
        assert_eq!(sources, vec!["clippy", "rustc"]);
    }

    #[test]
    fn multiline_markers_detects_multiline() {
        let mut m1 = make_marker("a.rs", MarkerSeverity::Error, "e1");
        m1.end_line = 5; // start_line is 1 from make_marker
        let m2 = make_marker("b.rs", MarkerSeverity::Warning, "w1");
        let markers = vec![m1, m2];
        let ml = multiline_markers(&markers);
        assert_eq!(ml.len(), 1);
    }

    #[test]
    fn marker_tooltip_formats_correctly() {
        let mut m = make_marker("a.rs", MarkerSeverity::Error, "expected `;`");
        m.source = Some("rustc".to_string());
        let tip = marker_tooltip(&m);
        assert!(tip.contains("Error"));
        assert!(tip.contains("[rustc]"));
        assert!(tip.contains("expected `;`"));
    }

    #[test]
    fn marker_has_any_tag_checks() {
        let mut m = make_marker("a.rs", MarkerSeverity::Warning, "unused");
        m.tags.push(MarkerTag::Unnecessary);
        assert!(marker_has_any_tag(&m, &[MarkerTag::Unnecessary]));
        assert!(!marker_has_any_tag(&m, &[MarkerTag::Deprecated]));
    }

    #[test]
    fn marker_has_any_tag_empty_tags() {
        let m = make_marker("a.rs", MarkerSeverity::Info, "info");
        assert!(!marker_has_any_tag(&m, &[MarkerTag::Unnecessary]));
    }
}
