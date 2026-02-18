//! Problems / markers view panel.
//!
//! Collects diagnostics (errors, warnings, etc.) and exposes query methods
//! used by the problems panel UI.

use std::fmt;

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

/// Remove all markers matching a given source across all URIs.
pub fn remove_markers_by_source(markers: &mut Vec<Marker>, source: &str) {
    markers.retain(|m| m.source.as_deref() != Some(source));
}

/// Deduplicate markers that have the same URI, line range, severity, and message.
pub fn deduplicate_markers(markers: &mut Vec<Marker>) {
    let mut seen = std::collections::HashSet::new();
    markers.retain(|m| {
        let key = (m.uri.clone(), m.start_line, m.start_col, m.end_line, m.end_col, format!("{:?}", m.severity), m.message.clone());
        seen.insert(key)
    });
}

/// Split markers into actionable (Error/Warning) and informational (Info/Hint).
pub fn split_actionable(markers: &[Marker]) -> (Vec<&Marker>, Vec<&Marker>) {
    let mut actionable = Vec::new();
    let mut informational = Vec::new();
    for m in markers {
        match m.severity {
            MarkerSeverity::Error | MarkerSeverity::Warning => actionable.push(m),
            MarkerSeverity::Info | MarkerSeverity::Hint => informational.push(m),
        }
    }
    (actionable, informational)
}

/// Return markers sorted by severity (most severe first), then by URI and line.
pub fn markers_sorted_by_severity(markers: &[Marker]) -> Vec<&Marker> {
    let mut sorted: Vec<&Marker> = markers.iter().collect();
    sorted.sort_by(|a, b| a.severity.cmp(&b.severity).then(a.uri.cmp(&b.uri)).then(a.start_line.cmp(&b.start_line)));
    sorted
}

/// Return a map of source -> count of markers from that source.
pub fn count_by_source(markers: &[Marker]) -> std::collections::HashMap<String, usize> {
    let mut counts = std::collections::HashMap::new();
    for m in markers {
        let src = m.source.clone().unwrap_or_else(|| "(none)".to_string());
        *counts.entry(src).or_insert(0) += 1;
    }
    counts
}

/// Return the average line span of all markers (0.0 if empty).
pub fn average_line_span(markers: &[Marker]) -> f64 {
    if markers.is_empty() { return 0.0; }
    let total: u32 = markers.iter().map(|m| m.end_line.saturating_sub(m.start_line) + 1).sum();
    total as f64 / markers.len() as f64
}

/// Format all markers for a URI as a diagnostic report string.
pub fn format_uri_report(markers: &[Marker], uri: &str) -> String {
    let relevant: Vec<&Marker> = markers.iter().filter(|m| m.uri == uri).collect();
    if relevant.is_empty() { return format!("{uri}: no diagnostics"); }
    let mut lines = vec![format!("{uri}: {} diagnostic(s)", relevant.len())];
    for m in &relevant {
        let src = m.source.as_deref().unwrap_or("unknown");
        lines.push(format!("  L{}:{} [{}] {}: {}", m.start_line, m.start_col, src, m.severity.label(), m.message));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// MarkerTableSort — sort markers by different columns
// ---------------------------------------------------------------------------

/// Column by which to sort a list of markers in a table view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkerTableSort {
    ByFile,
    BySeverity,
    ByMessage,
    ByLine,
    BySource,
}

impl MarkerTableSort {
    /// Sort `markers` in place using the chosen column (unstable sort).
    pub fn sort(markers: &mut [Marker], order: MarkerTableSort) {
        match order {
            MarkerTableSort::ByFile => markers.sort_unstable_by(|a, b| a.uri.cmp(&b.uri)),
            MarkerTableSort::BySeverity => {
                markers.sort_unstable_by(|a, b| a.severity.cmp(&b.severity))
            }
            MarkerTableSort::ByMessage => {
                markers.sort_unstable_by(|a, b| a.message.cmp(&b.message))
            }
            MarkerTableSort::ByLine => {
                markers.sort_unstable_by(|a, b| {
                    a.start_line.cmp(&b.start_line).then(a.start_col.cmp(&b.start_col))
                })
            }
            MarkerTableSort::BySource => markers.sort_unstable_by(|a, b| a.source.cmp(&b.source)),
        }
    }

    /// Sort `markers` in place using the chosen column (stable sort).
    pub fn sort_stable(markers: &mut [Marker], order: MarkerTableSort) {
        match order {
            MarkerTableSort::ByFile => markers.sort_by(|a, b| a.uri.cmp(&b.uri)),
            MarkerTableSort::BySeverity => markers.sort_by(|a, b| a.severity.cmp(&b.severity)),
            MarkerTableSort::ByMessage => markers.sort_by(|a, b| a.message.cmp(&b.message)),
            MarkerTableSort::ByLine => {
                markers.sort_by(|a, b| {
                    a.start_line.cmp(&b.start_line).then(a.start_col.cmp(&b.start_col))
                })
            }
            MarkerTableSort::BySource => markers.sort_by(|a, b| a.source.cmp(&b.source)),
        }
    }
}

impl std::fmt::Display for MarkerTableSort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            MarkerTableSort::ByFile => "File",
            MarkerTableSort::BySeverity => "Severity",
            MarkerTableSort::ByMessage => "Message",
            MarkerTableSort::ByLine => "Line",
            MarkerTableSort::BySource => "Source",
        };
        f.write_str(label)
    }
}

// ---------------------------------------------------------------------------
// MarkerQuickFixRegistry — manage quick-fix associations
// ---------------------------------------------------------------------------

/// Registry that maps marker indices to their available quick fixes.
#[derive(Debug, Clone, Default)]
pub struct MarkerQuickFixRegistry {
    fixes: Vec<(usize, MarkerQuickFix)>,
}

impl MarkerQuickFixRegistry {
    pub fn new() -> Self {
        Self { fixes: Vec::new() }
    }

    /// Register a quick fix for the marker at `marker_index`.
    pub fn register(&mut self, marker_index: usize, fix: MarkerQuickFix) {
        self.fixes.push((marker_index, fix));
    }

    /// Return references to all fixes associated with `idx`.
    pub fn fixes_for_marker(&self, idx: usize) -> Vec<&MarkerQuickFix> {
        self.fixes.iter().filter(|(i, _)| *i == idx).map(|(_, f)| f).collect()
    }

    /// Remove every fix associated with `idx`.
    pub fn remove_for_marker(&mut self, idx: usize) {
        self.fixes.retain(|(i, _)| *i != idx);
    }

    /// All registered (index, fix) pairs.
    pub fn all(&self) -> &[(usize, MarkerQuickFix)] {
        &self.fixes
    }

    /// Total number of registered fixes.
    pub fn count(&self) -> usize {
        self.fixes.len()
    }

    /// Whether any fix is registered for `idx`.
    pub fn has_fixes(&self, idx: usize) -> bool {
        self.fixes.iter().any(|(i, _)| *i == idx)
    }
}

// ---------------------------------------------------------------------------
// MarkerBatchActions — bulk operations on marker vectors
// ---------------------------------------------------------------------------

/// Stateless helpers for bulk marker operations.
pub struct MarkerBatchActions;

impl MarkerBatchActions {
    /// Remove all markers that match `severity`.
    pub fn dismiss_all(markers: &mut Vec<Marker>, severity: MarkerSeverity) {
        markers.retain(|m| m.severity != severity);
    }

    /// Keep only markers that match `severity`, removing everything else.
    pub fn retain_only(markers: &mut Vec<Marker>, severity: MarkerSeverity) {
        markers.retain(|m| m.severity == severity);
    }

    /// Remove markers whose `source` matches `source` exactly.
    pub fn clear_source(markers: &mut Vec<Marker>, source: &str) {
        markers.retain(|m| m.source.as_deref() != Some(source));
    }

    /// Count markers grouped by severity label.
    pub fn count_by_severity(markers: &[Marker]) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for m in markers {
            *map.entry(m.severity.label().to_string()).or_insert(0) += 1;
        }
        map
    }
}

// ---------------------------------------------------------------------------
// MarkerSeverityIconMapper — emoji/text icons for severity levels
// ---------------------------------------------------------------------------

/// Maps severity levels to visual icons for terminal or UI rendering.
pub struct MarkerSeverityIconMapper;

impl MarkerSeverityIconMapper {
    /// Return an emoji icon for the given severity.
    pub fn icon(severity: &MarkerSeverity) -> &'static str {
        match severity {
            MarkerSeverity::Error => "❌",
            MarkerSeverity::Warning => "⚠️",
            MarkerSeverity::Info => "ℹ️",
            MarkerSeverity::Hint => "💡",
        }
    }

    /// Return "icon label" string, e.g. `"❌ error"`.
    pub fn label_with_icon(severity: &MarkerSeverity) -> String {
        format!("{} {}", Self::icon(severity), severity.label())
    }

    /// All severity icons as `(label, icon)` pairs.
    pub fn all_icons() -> Vec<(&'static str, &'static str)> {
        vec![
            ("error", "❌"),
            ("warning", "⚠️"),
            ("info", "ℹ️"),
            ("hint", "💡"),
        ]
    }
}


// ---------------------------------------------------------------------------
// MarkersTreeView – tree-shaped rendering of markers grouped by file
// ---------------------------------------------------------------------------

/// A node in the markers tree: either a file header or an individual marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkersTreeNode {
    /// File-level grouping node.
    File {
        uri: String,
        error_count: usize,
        warning_count: usize,
        info_count: usize,
        hint_count: usize,
        expanded: bool,
    },
    /// Individual marker entry nested under a file.
    Entry {
        message: String,
        severity: MarkerSeverity,
        line: u32,
        col: u32,
    },
}

/// Builds a flat list of `MarkersTreeNode`s from a `MarkersService`, grouped
/// by URI and sorted by severity then line number.
pub struct MarkersTreeView;

impl MarkersTreeView {
    /// Build tree nodes from the marker service.
    pub fn build(service: &MarkersService) -> Vec<MarkersTreeNode> {
        // Collect unique URIs in the order they first appear.
        let mut uri_order: Vec<String> = Vec::new();
        for m in &service.markers {
            if !uri_order.contains(&m.uri) {
                uri_order.push(m.uri.clone());
            }
        }

        let mut nodes = Vec::new();
        for uri in &uri_order {
            let file_markers: Vec<&Marker> = service.markers.iter()
                .filter(|m| m.uri == *uri)
                .collect();

            let error_count = file_markers.iter().filter(|m| m.severity == MarkerSeverity::Error).count();
            let warning_count = file_markers.iter().filter(|m| m.severity == MarkerSeverity::Warning).count();
            let info_count = file_markers.iter().filter(|m| m.severity == MarkerSeverity::Info).count();
            let hint_count = file_markers.iter().filter(|m| m.severity == MarkerSeverity::Hint).count();

            nodes.push(MarkersTreeNode::File {
                uri: uri.clone(),
                error_count,
                warning_count,
                info_count,
                hint_count,
                expanded: true,
            });

            // Sort markers: by severity (most severe first), then by line
            let mut sorted: Vec<&Marker> = file_markers;
            sorted.sort_by(|a, b| a.severity.cmp(&b.severity).then(a.start_line.cmp(&b.start_line)));

            for m in sorted {
                nodes.push(MarkersTreeNode::Entry {
                    message: m.message.clone(),
                    severity: m.severity,
                    line: m.start_line,
                    col: m.start_col,
                });
            }
        }
        nodes
    }

    /// Count the total number of file nodes.
    pub fn file_count(nodes: &[MarkersTreeNode]) -> usize {
        nodes.iter().filter(|n| matches!(n, MarkersTreeNode::File { .. })).count()
    }

    /// Count the total number of entry nodes.
    pub fn entry_count(nodes: &[MarkersTreeNode]) -> usize {
        nodes.iter().filter(|n| matches!(n, MarkersTreeNode::Entry { .. })).count()
    }

    /// Render a single node to a display string.
    pub fn render_node(node: &MarkersTreeNode) -> String {
        match node {
            MarkersTreeNode::File { uri, error_count, warning_count, .. } => {
                let name = uri.rsplit('/').next().unwrap_or(uri);
                format!("{} (errors: {}, warnings: {})", name, error_count, warning_count)
            }
            MarkersTreeNode::Entry { message, severity, line, col } => {
                let icon = match severity {
                    MarkerSeverity::Error => "❌",
                    MarkerSeverity::Warning => "⚠️",
                    MarkerSeverity::Info => "ℹ️",
                    MarkerSeverity::Hint => "💡",
                };
                format!("{} [{}:{}] {}", icon, line, col, message)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MarkersWorkspaceSummary – aggregate summary across all files
// ---------------------------------------------------------------------------

/// High-level summary of diagnostics across the entire workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkersWorkspaceSummary {
    /// Total files with at least one marker.
    pub files_with_markers: usize,
    /// Total errors across all files.
    pub total_errors: usize,
    /// Total warnings.
    pub total_warnings: usize,
    /// Total info markers.
    pub total_infos: usize,
    /// Total hints.
    pub total_hints: usize,
}

impl MarkersWorkspaceSummary {
    /// Compute a workspace summary from a marker service.
    pub fn from_service(service: &MarkersService) -> Self {
        let mut uris: Vec<&str> = Vec::new();
        for m in &service.markers {
            if !uris.contains(&m.uri.as_str()) {
                uris.push(&m.uri);
            }
        }
        let stats = service.get_stats();
        Self {
            files_with_markers: uris.len(),
            total_errors: stats.errors,
            total_warnings: stats.warnings,
            total_infos: stats.infos,
            total_hints: stats.hints,
        }
    }

    /// Total number of markers across all severities.
    pub fn total_markers(&self) -> usize {
        self.total_errors + self.total_warnings + self.total_infos + self.total_hints
    }

    /// Whether the workspace has any errors.
    pub fn has_errors(&self) -> bool {
        self.total_errors > 0
    }

    /// Whether the workspace is clean (no markers at all).
    pub fn is_clean(&self) -> bool {
        self.total_markers() == 0
    }

    /// A short status-bar string like "3 errors, 2 warnings".
    pub fn status_text(&self) -> String {
        let mut parts = Vec::new();
        if self.total_errors > 0 {
            parts.push(format!("{} error{}", self.total_errors, if self.total_errors == 1 { "" } else { "s" }));
        }
        if self.total_warnings > 0 {
            parts.push(format!("{} warning{}", self.total_warnings, if self.total_warnings == 1 { "" } else { "s" }));
        }
        if self.total_infos > 0 {
            parts.push(format!("{} info", self.total_infos));
        }
        if self.total_hints > 0 {
            parts.push(format!("{} hint{}", self.total_hints, if self.total_hints == 1 { "" } else { "s" }));
        }
        if parts.is_empty() {
            "No problems".to_string()
        } else {
            parts.join(", ")
        }
    }
}

// ---------------------------------------------------------------------------
// MarkersOutlineProvider – outline entries derived from markers
// ---------------------------------------------------------------------------

/// An outline entry representing a marker in the document outline view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerOutlineEntry {
    pub label: String,
    pub severity: MarkerSeverity,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Generates outline entries from markers for a given document.
pub struct MarkersOutlineProvider;

impl MarkersOutlineProvider {
    /// Generate outline entries for a given URI from the service.
    pub fn provide(service: &MarkersService, uri: &str) -> Vec<MarkerOutlineEntry> {
        let mut entries: Vec<MarkerOutlineEntry> = service.markers.iter()
            .filter(|m| m.uri == uri)
            .map(|m| MarkerOutlineEntry {
                label: m.message.clone(),
                severity: m.severity,
                line: m.start_line,
                col: m.start_col,
                end_line: m.end_line,
                end_col: m.end_col,
            })
            .collect();
        entries.sort_by(|a, b| a.line.cmp(&b.line).then(a.col.cmp(&b.col)));
        entries
    }

    /// Return only outline entries that are errors.
    pub fn errors_only(service: &MarkersService, uri: &str) -> Vec<MarkerOutlineEntry> {
        Self::provide(service, uri).into_iter()
            .filter(|e| e.severity == MarkerSeverity::Error)
            .collect()
    }

    /// Return the total line span covered by markers in a file.
    pub fn affected_line_range(service: &MarkersService, uri: &str) -> Option<(u32, u32)> {
        let entries = Self::provide(service, uri);
        if entries.is_empty() {
            return None;
        }
        let min_line = entries.iter().map(|e| e.line).min().unwrap();
        let max_line = entries.iter().map(|e| e.end_line).max().unwrap();
        Some((min_line, max_line))
    }

    /// Total number of affected lines (unique lines that have at least one marker).
    pub fn affected_line_count(service: &MarkersService, uri: &str) -> usize {
        let entries = Self::provide(service, uri);
        let mut lines: Vec<u32> = entries.iter().map(|e| e.line).collect();
        lines.sort_unstable();
        lines.dedup();
        lines.len()
    }
}

// ---------------------------------------------------------------------------
// Copy diagnostic text – format markers for clipboard
// ---------------------------------------------------------------------------

/// Formats markers from a service into copyable diagnostic text.
pub struct MarkersCopyDiagnosticText;

impl MarkersCopyDiagnosticText {
    /// Format a single marker to a diagnostic string.
    pub fn format_one(marker: &Marker) -> String {
        let sev = marker.severity.label();
        let loc = format!("{}:{}:{}", marker.uri, marker.start_line, marker.start_col);
        let src = marker.source.as_deref().unwrap_or("unknown");
        format!("{} - {} [{}] ({})", loc, marker.message, sev, src)
    }

    /// Format all markers in the service to a multi-line string.
    pub fn format_all(service: &MarkersService) -> String {
        service.markers.iter()
            .map(Self::format_one)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Format markers for a specific URI.
    pub fn format_for_uri(service: &MarkersService, uri: &str) -> String {
        service.markers.iter()
            .filter(|m| m.uri == uri)
            .map(Self::format_one)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Format only errors.
    pub fn format_errors(service: &MarkersService) -> String {
        service.markers.iter()
            .filter(|m| m.severity == MarkerSeverity::Error)
            .map(Self::format_one)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Number of lines the formatted output would produce.
    pub fn line_count(service: &MarkersService) -> usize {
        service.markers.len()
    }

    /// Format as a markdown table.
    pub fn format_as_table(service: &MarkersService) -> String {
        let mut out = String::from("| Severity | Location | Message | Source |\n");
        out.push_str("|----------|----------|---------|--------|\n");
        for m in &service.markers {
            let loc = format!("{}:{}:{}", m.uri, m.start_line, m.start_col);
            let src = m.source.as_deref().unwrap_or("-");
            out.push_str(&format!("| {} | {} | {} | {} |\n", m.severity.label(), loc, m.message, src));
        }
        out
    }
}


// ---------------------------------------------------------------------------
// markers_view – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XMarkersViewLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XMarkersViewPanelState {
    pub region: XMarkersViewLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XMarkersViewPanelState {
    pub fn new(region: XMarkersViewLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_markers_view_total_visible_area(panels: &[XMarkersViewPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_markers_view_count_in_region(
    panels: &[XMarkersViewPanelState],
    region: XMarkersViewLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_markers_view_widest_panel(panels: &[XMarkersViewPanelState]) -> Option<&XMarkersViewPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_markers_view_collapse_region(
    panels: &mut [XMarkersViewPanelState],
    region: XMarkersViewLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XMarkersViewLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XMarkersViewLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// markers_view – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for diagnostics markers panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YMarkersViewMarkerGroupBy {
    File,
    Severity,
    Source,
    Line,
}

impl YMarkersViewMarkerGroupBy {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::File => 0,
            Self::Severity => 1,
            Self::Source => 2,
            Self::Line => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Severity => "Severity",
            Self::Source => "Source",
            Self::Line => "Line",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YMarkersViewMarkerGroupBy] {
        &[
            YMarkersViewMarkerGroupBy::File,
            YMarkersViewMarkerGroupBy::Severity,
            YMarkersViewMarkerGroupBy::Source,
            YMarkersViewMarkerGroupBy::Line,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YMarkersViewMarkerGroupBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks marker batch data.
#[derive(Debug, Clone)]
pub struct YMarkersViewMarkerBatchUpdate {
    pub additions: Vec<(String, u32)>,
    pub removals: Vec<String>,
    pub version: u64,
}

impl YMarkersViewMarkerBatchUpdate {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            additions: Vec::new(),
            removals: Vec::new(),
            version: 0,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.additions.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.additions.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.additions.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YMarkersViewMarkerBatchUpdate({}: {:?})", "additions", self.additions)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_markers_view_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_markers_view_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_markers_view_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_markers_view_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_markers_view_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_markers_view_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_markers_view_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_markers_view_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// markers_view – Extended marker heatmap helpers
// ---------------------------------------------------------------------------

/// Priority levels for marker heatmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZMarkersViewPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZMarkersViewPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZMarkersViewPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZMarkersViewPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks marker heatmap data.
#[derive(Debug, Clone)]
pub struct ZMarkersViewMarkerHeatmap {
    pub buckets: Vec<(u32, usize)>,
    pub resolution: u32,
    pub normalized: bool,
}

impl ZMarkersViewMarkerHeatmap {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            buckets: Vec::new(),
            resolution: 0,
            normalized: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.buckets.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZMarkersViewMarkerHeatmap[resolution={:?}, normalized={:?}]", self.resolution, self.normalized)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.normalized = !c.normalized;
        c
    }
}

/// Compute a simple rolling hash for marker heatmap.
pub fn z_markers_view_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_markers_view_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_markers_view_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_markers_view_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_markers_view_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_markers_view_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_markers_view_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 61
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer61 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer61 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_61(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_61<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_61<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_61(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_61(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 120
// ---------------------------------------------------------------------------

/// Generic object pool `Xc120Pool<T>`.
pub struct Xc120Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc120Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc120PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc120Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc120PoolStats {
        Xc120PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc120Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc120Scheduler`.
pub struct Xc120Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc120Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc120Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_120 hash for the given byte slice.
pub fn xc_120_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_120 convention.
pub fn xc_120_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe74 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe74Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe74PipelineError {
    pub stage: Xe74Stage,
    pub message: String,
}

impl std::fmt::Display for Xe74PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe74Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe74Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe74PipelineError>>>,
    stage_names: Vec<Xe74Stage>,
}

impl Xe74Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe74PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe74Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe74PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe74Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe74PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe74Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe74PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe74Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe74PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe74Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe74CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe74CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe74Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe74CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe74CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe74Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe74CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_74_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe74CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_74_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe74CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_74_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe74PipelineError> {
    Ok(data)
}

pub fn xe_74_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe74PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_74_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe74PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_74_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe74PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_74_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe74PipelineError> {
    Err(Xe74PipelineError {
        stage: Xe74Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_72: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg72Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg72Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg72Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_72: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg72Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg72Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg72Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg72Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 119).
pub struct Xh119SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh119SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 161 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 119).
pub struct Xh119BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh119BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 119).
pub struct Xi119Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi119Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi119Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi119Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 119).
pub struct Xi119IntervalTree {
    xi_intervals: Vec<Xi119Interval>,
}

impl Xi119IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi119Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi119Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi119Interval) -> Vec<&Xi119Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi119Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi119Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi119Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi119Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi119Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi119Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 120) ---

/// Disjoint set / union-find for crate 120.
pub struct Xj120UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj120UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ120_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 120.
pub struct Xj120BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj120BTreeNode<K, V>>>,
    len: usize,
}

struct Xj120BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj120BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj120BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ120_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ120_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj120BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj120BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj120BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj120BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_120 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk120SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk120SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk120DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk120DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_120).
#[derive(Debug, Clone)]
pub struct Xl120Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl120Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_120).
#[derive(Debug, Clone)]
pub struct Xl120SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl120SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm120MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm120MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm120Tokenizer {
    text: String,
}

impl Xm120Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
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
    fn import_from_marker_service_works() {
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

    #[test]
    fn remove_markers_by_source_filters() {
        let mut m1 = make_marker("a.rs", MarkerSeverity::Error, "e1"); m1.source = Some("rustc".into());
        let mut m2 = make_marker("b.rs", MarkerSeverity::Warning, "w1"); m2.source = Some("clippy".into());
        let mut m3 = make_marker("c.rs", MarkerSeverity::Info, "i1"); m3.source = Some("rustc".into());
        let mut markers = vec![m1, m2, m3];
        remove_markers_by_source(&mut markers, "rustc");
        assert_eq!(markers.len(), 1);
    }

    #[test]
    fn deduplicate_markers_removes_dupes() {
        let m1 = make_marker("a.rs", MarkerSeverity::Error, "same");
        let m2 = make_marker("a.rs", MarkerSeverity::Error, "same");
        let m3 = make_marker("a.rs", MarkerSeverity::Warning, "same");
        let mut markers = vec![m1, m2, m3];
        deduplicate_markers(&mut markers);
        assert_eq!(markers.len(), 2);
    }

    #[test]
    fn split_actionable_separates() {
        let markers = vec![
            make_marker("a.rs", MarkerSeverity::Error, "e"),
            make_marker("a.rs", MarkerSeverity::Info, "i"),
            make_marker("a.rs", MarkerSeverity::Warning, "w"),
            make_marker("a.rs", MarkerSeverity::Hint, "h"),
        ];
        let (act, info) = split_actionable(&markers);
        assert_eq!(act.len(), 2);
        assert_eq!(info.len(), 2);
    }

    #[test]
    fn markers_sorted_by_severity_orders() {
        let markers = vec![
            make_marker("a.rs", MarkerSeverity::Hint, "h"),
            make_marker("a.rs", MarkerSeverity::Error, "e"),
            make_marker("a.rs", MarkerSeverity::Warning, "w"),
        ];
        let sorted = markers_sorted_by_severity(&markers);
        assert_eq!(sorted[0].severity, MarkerSeverity::Error);
        assert_eq!(sorted[2].severity, MarkerSeverity::Hint);
    }

    #[test]
    fn count_by_source_tallies() {
        let mut m1 = make_marker("a.rs", MarkerSeverity::Error, "e1"); m1.source = Some("rustc".into());
        let mut m2 = make_marker("b.rs", MarkerSeverity::Warning, "w1"); m2.source = Some("clippy".into());
        let mut m3 = make_marker("c.rs", MarkerSeverity::Info, "i1"); m3.source = Some("rustc".into());
        let counts = count_by_source(&[m1, m2, m3]);
        assert_eq!(counts.get("rustc"), Some(&2));
        assert_eq!(counts.get("clippy"), Some(&1));
    }

    #[test]
    fn average_line_span_computes() {
        let m1 = make_marker("a.rs", MarkerSeverity::Error, "e1");
        let mut m2 = make_marker("b.rs", MarkerSeverity::Warning, "w1"); m2.end_line = 5;
        let avg = average_line_span(&[m1, m2]);
        assert!((avg - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn average_line_span_empty() {
        assert!((average_line_span(&[]) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn format_uri_report_with_markers() {
        let mut m = make_marker("src/main.rs", MarkerSeverity::Error, "expected `;`");
        m.source = Some("rustc".into());
        let report = format_uri_report(&[m], "src/main.rs");
        assert!(report.contains("1 diagnostic(s)"));
        assert!(report.contains("[rustc]"));
    }

    #[test]
    fn format_uri_report_no_markers() {
        assert!(format_uri_report(&[], "src/main.rs").contains("no diagnostics"));
    }

    // -- MarkerTableSort tests -----------------------------------------------

    #[test]
    fn table_sort_by_severity() {
        let mut markers = vec![
            make_marker("a.rs", MarkerSeverity::Hint, "h"),
            make_marker("a.rs", MarkerSeverity::Error, "e"),
            make_marker("a.rs", MarkerSeverity::Warning, "w"),
        ];
        MarkerTableSort::sort(&mut markers, MarkerTableSort::BySeverity);
        assert_eq!(markers[0].severity, MarkerSeverity::Error);
        assert_eq!(markers[1].severity, MarkerSeverity::Warning);
        assert_eq!(markers[2].severity, MarkerSeverity::Hint);
    }

    #[test]
    fn table_sort_by_file() {
        let mut markers = vec![
            make_marker("z.rs", MarkerSeverity::Error, "e"),
            make_marker("a.rs", MarkerSeverity::Error, "e"),
        ];
        MarkerTableSort::sort(&mut markers, MarkerTableSort::ByFile);
        assert_eq!(markers[0].uri, "a.rs");
        assert_eq!(markers[1].uri, "z.rs");
    }

    #[test]
    fn table_sort_by_line() {
        let mut markers = vec![
            make_marker_ext("a.rs", MarkerSeverity::Error, "e2", 10, None),
            make_marker_ext("a.rs", MarkerSeverity::Error, "e1", 1, None),
        ];
        MarkerTableSort::sort(&mut markers, MarkerTableSort::ByLine);
        assert_eq!(markers[0].start_line, 1);
        assert_eq!(markers[1].start_line, 10);
    }

    #[test]
    fn table_sort_stable_preserves_order() {
        let mut markers = vec![
            make_marker("a.rs", MarkerSeverity::Error, "beta"),
            make_marker("a.rs", MarkerSeverity::Error, "alpha"),
        ];
        MarkerTableSort::sort_stable(&mut markers, MarkerTableSort::BySeverity);
        // Same severity — stable sort preserves insertion order.
        assert_eq!(markers[0].message, "beta");
        assert_eq!(markers[1].message, "alpha");
    }

    #[test]
    fn table_sort_display() {
        assert_eq!(MarkerTableSort::ByFile.to_string(), "File");
        assert_eq!(MarkerTableSort::BySeverity.to_string(), "Severity");
        assert_eq!(MarkerTableSort::BySource.to_string(), "Source");
    }

    // -- MarkerQuickFixRegistry tests ----------------------------------------

    #[test]
    fn registry_register_and_query() {
        let mut reg = MarkerQuickFixRegistry::new();
        let fix = MarkerQuickFix::new("Add import", "a.rs", 1, 0, "use std::io;");
        reg.register(0, fix);
        assert_eq!(reg.count(), 1);
        assert!(reg.has_fixes(0));
        assert!(!reg.has_fixes(1));
        assert_eq!(reg.fixes_for_marker(0).len(), 1);
        assert_eq!(reg.fixes_for_marker(0)[0].title, "Add import");
    }

    #[test]
    fn registry_remove_for_marker() {
        let mut reg = MarkerQuickFixRegistry::new();
        reg.register(0, MarkerQuickFix::new("fix1", "a.rs", 1, 0, "t1"));
        reg.register(0, MarkerQuickFix::new("fix2", "a.rs", 2, 0, "t2"));
        reg.register(1, MarkerQuickFix::new("fix3", "b.rs", 1, 0, "t3"));
        assert_eq!(reg.count(), 3);
        reg.remove_for_marker(0);
        assert_eq!(reg.count(), 1);
        assert!(!reg.has_fixes(0));
        assert!(reg.has_fixes(1));
    }

    #[test]
    fn registry_all_returns_slice() {
        let mut reg = MarkerQuickFixRegistry::new();
        reg.register(5, MarkerQuickFix::new("f", "x.rs", 1, 0, "txt"));
        let all = reg.all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].0, 5);
    }

    // -- MarkerBatchActions tests --------------------------------------------

    #[test]
    fn batch_dismiss_all_removes_severity() {
        let mut markers = vec![
            make_marker("a.rs", MarkerSeverity::Error, "e"),
            make_marker("a.rs", MarkerSeverity::Warning, "w"),
            make_marker("a.rs", MarkerSeverity::Error, "e2"),
        ];
        MarkerBatchActions::dismiss_all(&mut markers, MarkerSeverity::Error);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].severity, MarkerSeverity::Warning);
    }

    #[test]
    fn batch_retain_only_keeps_severity() {
        let mut markers = vec![
            make_marker("a.rs", MarkerSeverity::Error, "e"),
            make_marker("a.rs", MarkerSeverity::Warning, "w"),
            make_marker("a.rs", MarkerSeverity::Info, "i"),
        ];
        MarkerBatchActions::retain_only(&mut markers, MarkerSeverity::Warning);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].severity, MarkerSeverity::Warning);
    }

    #[test]
    fn batch_clear_source() {
        let mut markers = vec![
            make_marker_ext("a.rs", MarkerSeverity::Error, "e", 1, Some("rustc")),
            make_marker_ext("a.rs", MarkerSeverity::Warning, "w", 2, Some("clippy")),
            make_marker_ext("a.rs", MarkerSeverity::Info, "i", 3, Some("rustc")),
        ];
        MarkerBatchActions::clear_source(&mut markers, "rustc");
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].source.as_deref(), Some("clippy"));
    }

    #[test]
    fn batch_count_by_severity() {
        let markers = vec![
            make_marker("a.rs", MarkerSeverity::Error, "e1"),
            make_marker("a.rs", MarkerSeverity::Error, "e2"),
            make_marker("a.rs", MarkerSeverity::Warning, "w"),
        ];
        let counts = MarkerBatchActions::count_by_severity(&markers);
        assert_eq!(counts.get("error"), Some(&2));
        assert_eq!(counts.get("warning"), Some(&1));
        assert_eq!(counts.get("info"), None);
    }

    // -- MarkerSeverityIconMapper tests --------------------------------------

    #[test]
    fn icon_mapper_returns_correct_icons() {
        assert_eq!(MarkerSeverityIconMapper::icon(&MarkerSeverity::Error), "❌");
        assert_eq!(MarkerSeverityIconMapper::icon(&MarkerSeverity::Warning), "⚠️");
        assert_eq!(MarkerSeverityIconMapper::icon(&MarkerSeverity::Info), "ℹ️");
        assert_eq!(MarkerSeverityIconMapper::icon(&MarkerSeverity::Hint), "💡");
    }

    #[test]
    fn icon_mapper_label_with_icon() {
        let label = MarkerSeverityIconMapper::label_with_icon(&MarkerSeverity::Error);
        assert!(label.contains("❌"));
        assert!(label.contains("error"));
    }

    #[test]
    fn icon_mapper_all_icons() {
        let icons = MarkerSeverityIconMapper::all_icons();
        assert_eq!(icons.len(), 4);
        assert_eq!(icons[0], ("error", "❌"));
    }

    // -- MarkersTreeView tests --

    fn make_test_marker(uri: &str, msg: &str, severity: MarkerSeverity, line: u32) -> Marker {
        Marker {
            uri: uri.to_string(),
            message: msg.to_string(),
            severity,
            start_line: line,
            start_col: 1,
            end_line: line,
            end_col: 10,
            source: Some("test".to_string()),
            code: None,
            tags: vec![],
            related_information: vec![],
        }
    }

    #[test]
    fn tree_view_build_groups_by_uri() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_test_marker("file:///a.rs", "err1", MarkerSeverity::Error, 1));
        svc.add_marker(make_test_marker("file:///b.rs", "warn1", MarkerSeverity::Warning, 5));
        svc.add_marker(make_test_marker("file:///a.rs", "err2", MarkerSeverity::Error, 10));

        let nodes = MarkersTreeView::build(&svc);
        assert_eq!(MarkersTreeView::file_count(&nodes), 2);
        assert_eq!(MarkersTreeView::entry_count(&nodes), 3);
    }

    #[test]
    fn tree_view_render_file_node() {
        let node = MarkersTreeNode::File {
            uri: "file:///src/main.rs".to_string(),
            error_count: 2,
            warning_count: 1,
            info_count: 0,
            hint_count: 0,
            expanded: true,
        };
        let rendered = MarkersTreeView::render_node(&node);
        assert!(rendered.contains("main.rs"));
        assert!(rendered.contains("errors: 2"));
    }

    #[test]
    fn tree_view_render_entry_node() {
        let node = MarkersTreeNode::Entry {
            message: "unused variable".to_string(),
            severity: MarkerSeverity::Warning,
            line: 42,
            col: 5,
        };
        let rendered = MarkersTreeView::render_node(&node);
        assert!(rendered.contains("[42:5]"));
        assert!(rendered.contains("unused variable"));
    }

    // -- MarkersWorkspaceSummary tests --

    #[test]
    fn workspace_summary_empty() {
        let svc = MarkersService::new();
        let summary = MarkersWorkspaceSummary::from_service(&svc);
        assert!(summary.is_clean());
        assert!(!summary.has_errors());
        assert_eq!(summary.total_markers(), 0);
        assert_eq!(summary.status_text(), "No problems");
    }

    #[test]
    fn workspace_summary_with_markers() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_test_marker("file:///a.rs", "e1", MarkerSeverity::Error, 1));
        svc.add_marker(make_test_marker("file:///a.rs", "w1", MarkerSeverity::Warning, 2));
        svc.add_marker(make_test_marker("file:///b.rs", "e2", MarkerSeverity::Error, 3));

        let summary = MarkersWorkspaceSummary::from_service(&svc);
        assert_eq!(summary.files_with_markers, 2);
        assert_eq!(summary.total_errors, 2);
        assert_eq!(summary.total_warnings, 1);
        assert!(summary.has_errors());
        assert!(!summary.is_clean());
        assert_eq!(summary.total_markers(), 3);
    }

    #[test]
    fn workspace_summary_status_text() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_test_marker("file:///a.rs", "e", MarkerSeverity::Error, 1));
        let summary = MarkersWorkspaceSummary::from_service(&svc);
        assert_eq!(summary.status_text(), "1 error");
    }

    // -- MarkersOutlineProvider tests --

    #[test]
    fn outline_provider_basic() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_test_marker("file:///a.rs", "msg1", MarkerSeverity::Error, 10));
        svc.add_marker(make_test_marker("file:///a.rs", "msg2", MarkerSeverity::Warning, 5));
        svc.add_marker(make_test_marker("file:///b.rs", "msg3", MarkerSeverity::Info, 1));

        let entries = MarkersOutlineProvider::provide(&svc, "file:///a.rs");
        assert_eq!(entries.len(), 2);
        // Sorted by line: line 5 before line 10
        assert_eq!(entries[0].line, 5);
        assert_eq!(entries[1].line, 10);
    }

    #[test]
    fn outline_provider_errors_only() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_test_marker("file:///a.rs", "e", MarkerSeverity::Error, 1));
        svc.add_marker(make_test_marker("file:///a.rs", "w", MarkerSeverity::Warning, 2));

        let errors = MarkersOutlineProvider::errors_only(&svc, "file:///a.rs");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, MarkerSeverity::Error);
    }

    #[test]
    fn outline_provider_affected_range() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_test_marker("file:///a.rs", "m1", MarkerSeverity::Error, 5));
        svc.add_marker(make_test_marker("file:///a.rs", "m2", MarkerSeverity::Warning, 20));

        let range = MarkersOutlineProvider::affected_line_range(&svc, "file:///a.rs");
        assert_eq!(range, Some((5, 20)));
    }

    #[test]
    fn outline_provider_affected_line_count() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_test_marker("file:///a.rs", "m1", MarkerSeverity::Error, 5));
        svc.add_marker(make_test_marker("file:///a.rs", "m2", MarkerSeverity::Warning, 5));
        svc.add_marker(make_test_marker("file:///a.rs", "m3", MarkerSeverity::Info, 10));

        assert_eq!(MarkersOutlineProvider::affected_line_count(&svc, "file:///a.rs"), 2);
    }

    #[test]
    fn outline_provider_no_markers() {
        let svc = MarkersService::new();
        assert!(MarkersOutlineProvider::provide(&svc, "file:///x.rs").is_empty());
        assert_eq!(MarkersOutlineProvider::affected_line_range(&svc, "file:///x.rs"), None);
    }

    // -- MarkersCopyDiagnosticText tests --

    #[test]
    fn copy_diagnostic_format_one() {
        let m = make_test_marker("file:///a.rs", "unused var", MarkerSeverity::Warning, 10);
        let text = MarkersCopyDiagnosticText::format_one(&m);
        assert!(text.contains("file:///a.rs:10:1"));
        assert!(text.contains("unused var"));
        assert!(text.contains("warning"));
        assert!(text.contains("test"));
    }

    #[test]
    fn copy_diagnostic_format_all() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_test_marker("file:///a.rs", "e1", MarkerSeverity::Error, 1));
        svc.add_marker(make_test_marker("file:///b.rs", "w1", MarkerSeverity::Warning, 2));

        let text = MarkersCopyDiagnosticText::format_all(&svc);
        let line_count = text.lines().count();
        assert_eq!(line_count, 2);
        assert_eq!(MarkersCopyDiagnosticText::line_count(&svc), 2);
    }

    #[test]
    fn copy_diagnostic_format_for_uri() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_test_marker("file:///a.rs", "e1", MarkerSeverity::Error, 1));
        svc.add_marker(make_test_marker("file:///b.rs", "w1", MarkerSeverity::Warning, 2));

        let text = MarkersCopyDiagnosticText::format_for_uri(&svc, "file:///a.rs");
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("e1"));
    }

    #[test]
    fn copy_diagnostic_format_errors() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_test_marker("file:///a.rs", "e1", MarkerSeverity::Error, 1));
        svc.add_marker(make_test_marker("file:///a.rs", "w1", MarkerSeverity::Warning, 2));

        let text = MarkersCopyDiagnosticText::format_errors(&svc);
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("e1"));
    }

    #[test]
    fn copy_diagnostic_format_table() {
        let mut svc = MarkersService::new();
        svc.add_marker(make_test_marker("file:///a.rs", "err", MarkerSeverity::Error, 1));

        let table = MarkersCopyDiagnosticText::format_as_table(&svc);
        assert!(table.contains("| Severity |"));
        assert!(table.contains("| error |"));
    }


    // -- markers_view additional tests -------------------------------------------

    #[test]
    fn x_markers_view_panel_state_new() {
        let p = XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XMarkersViewLayoutRegion::Sidebar);
    }

    #[test]
    fn x_markers_view_panel_area() {
        let p = XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_markers_view_panel_toggle() {
        let mut p = XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_markers_view_panel_resize() {
        let mut p = XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_markers_view_panel_is_narrow() {
        let mut p = XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_markers_view_total_visible_area_basic() {
        let panels = vec![
            XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Sidebar, "a"),
            XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_markers_view_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_markers_view_total_visible_area_hidden() {
        let mut panels = vec![
            XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Sidebar, "a"),
            XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_markers_view_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_markers_view_count_in_region_basic() {
        let panels = vec![
            XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Sidebar, "a"),
            XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Sidebar, "b"),
            XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_markers_view_count_in_region(&panels, XMarkersViewLayoutRegion::Sidebar), 2);
        assert_eq!(x_markers_view_count_in_region(&panels, XMarkersViewLayoutRegion::Editor), 1);
        assert_eq!(x_markers_view_count_in_region(&panels, XMarkersViewLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_markers_view_widest_panel_basic() {
        let mut panels = vec![
            XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Sidebar, "narrow"),
            XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_markers_view_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_markers_view_collapse_region_basic() {
        let mut panels = vec![
            XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Sidebar, "a"),
            XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Sidebar, "b"),
            XMarkersViewPanelState::new(XMarkersViewLayoutRegion::Editor, "c"),
        ];
        x_markers_view_collapse_region(&mut panels, XMarkersViewLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_markers_view_layout_constraint_clamp() {
        let lc = XMarkersViewLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_markers_view_layout_constraint_satisfied() {
        let lc = XMarkersViewLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_markers_view_widest_panel_empty() {
        let panels: Vec<XMarkersViewPanelState> = vec![];
        assert!(x_markers_view_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_markers_view_layout_region_eq() {
        assert_eq!(XMarkersViewLayoutRegion::Sidebar, XMarkersViewLayoutRegion::Sidebar);
        assert_ne!(XMarkersViewLayoutRegion::Sidebar, XMarkersViewLayoutRegion::Panel);
    }


    // -- markers_view extended domain tests ----------------------------------------

    #[test]
    fn y_markers_view_enum_index() {
        assert_eq!(YMarkersViewMarkerGroupBy::File.index(), 0);
        assert_eq!(YMarkersViewMarkerGroupBy::Severity.index(), 1);
        assert_eq!(YMarkersViewMarkerGroupBy::Source.index(), 2);
        assert_eq!(YMarkersViewMarkerGroupBy::Line.index(), 3);
    }

    #[test]
    fn y_markers_view_enum_label() {
        assert_eq!(YMarkersViewMarkerGroupBy::File.label(), "File");
        assert_eq!(YMarkersViewMarkerGroupBy::Severity.label(), "Severity");
        assert_eq!(YMarkersViewMarkerGroupBy::Source.label(), "Source");
        assert_eq!(YMarkersViewMarkerGroupBy::Line.label(), "Line");
    }

    #[test]
    fn y_markers_view_enum_all() {
        let all = YMarkersViewMarkerGroupBy::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_markers_view_enum_is_default() {
        assert!(YMarkersViewMarkerGroupBy::File.is_default());
        assert!(!YMarkersViewMarkerGroupBy::Line.is_default());
    }

    #[test]
    fn y_markers_view_enum_display() {
        assert_eq!(format!("{}", YMarkersViewMarkerGroupBy::File), "File");
    }

    #[test]
    fn y_markers_view_struct_new() {
        let s = YMarkersViewMarkerBatchUpdate::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_markers_view_struct_clear() {
        let mut s = YMarkersViewMarkerBatchUpdate::new();
        s.additions.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_markers_view_fingerprint_deterministic() {
        let h1 = y_markers_view_fingerprint("hello");
        let h2 = y_markers_view_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_markers_view_fingerprint("a"), y_markers_view_fingerprint("b"));
    }

    #[test]
    fn y_markers_view_truncate_short() {
        assert_eq!(y_markers_view_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_markers_view_truncate_long() {
        let r = y_markers_view_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_markers_view_normalize_key_basic() {
        assert_eq!(y_markers_view_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_markers_view_split_path_basic() {
        let parts = y_markers_view_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_markers_view_count_occurrences_basic() {
        assert_eq!(y_markers_view_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_markers_view_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_markers_view_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_markers_view_in_range_basic() {
        assert!(y_markers_view_in_range(5, 1, 10));
        assert!(y_markers_view_in_range(1, 1, 10));
        assert!(y_markers_view_in_range(10, 1, 10));
        assert!(!y_markers_view_in_range(0, 1, 10));
        assert!(!y_markers_view_in_range(11, 1, 10));
    }

    #[test]
    fn y_markers_view_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_markers_view_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_markers_view_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_markers_view_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- markers_view Z-extended tests -----------------------------------------------

    #[test]
    fn z_markers_view_priority_weight() {
        assert_eq!(ZMarkersViewPriority::Idle.weight(), 0);
        assert_eq!(ZMarkersViewPriority::Normal.weight(), 2);
        assert_eq!(ZMarkersViewPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_markers_view_priority_label() {
        assert_eq!(ZMarkersViewPriority::Low.label(), "low");
        assert_eq!(ZMarkersViewPriority::High.label(), "high");
    }

    #[test]
    fn z_markers_view_priority_is_elevated() {
        assert!(!ZMarkersViewPriority::Normal.is_elevated());
        assert!(ZMarkersViewPriority::High.is_elevated());
        assert!(ZMarkersViewPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_markers_view_priority_display() {
        assert_eq!(format!("{}", ZMarkersViewPriority::Idle), "idle");
    }

    #[test]
    fn z_markers_view_priority_all_asc() {
        let all = ZMarkersViewPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZMarkersViewPriority::Idle);
        assert_eq!(all[4], ZMarkersViewPriority::Realtime);
    }

    #[test]
    fn z_markers_view_struct_new() {
        let s = ZMarkersViewMarkerHeatmap::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_markers_view_struct_toggled_clone() {
        let s = ZMarkersViewMarkerHeatmap::new();
        let t = s.toggled_clone();
        assert_ne!(s.normalized, t.normalized);
    }

    #[test]
    fn z_markers_view_rolling_hash_deterministic() {
        let h1 = z_markers_view_rolling_hash(b"test");
        let h2 = z_markers_view_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_markers_view_rolling_hash(b"a"), z_markers_view_rolling_hash(b"b"));
    }

    #[test]
    fn z_markers_view_pad_to_basic() {
        assert_eq!(z_markers_view_pad_to("hi", 5), "hi   ");
        assert_eq!(z_markers_view_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_markers_view_is_identifier_basic() {
        assert!(z_markers_view_is_identifier("foo_bar"));
        assert!(z_markers_view_is_identifier("abc123"));
        assert!(!z_markers_view_is_identifier(""));
        assert!(!z_markers_view_is_identifier("has space"));
    }

    #[test]
    fn z_markers_view_levenshtein_basic() {
        assert_eq!(z_markers_view_levenshtein("", ""), 0);
        assert_eq!(z_markers_view_levenshtein("abc", "abc"), 0);
        assert_eq!(z_markers_view_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_markers_view_unique_words_basic() {
        let w = z_markers_view_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_markers_view_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_markers_view_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_markers_view_common_prefix_basic() {
        assert_eq!(z_markers_view_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_markers_view_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_markers_view_struct_clear() {
        let mut s = ZMarkersViewMarkerHeatmap::new();
        s.buckets.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_markers_view_rolling_hash_empty() {
        let h = z_markers_view_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_61_push_and_len() {
        let mut rb = super::XbRingBuffer61::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_61_overwrite() {
        let mut rb = super::XbRingBuffer61::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_61_get_out_of_bounds() {
        let rb = super::XbRingBuffer61::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_61_drain_all() {
        let mut rb = super::XbRingBuffer61::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_61_peek_front_back() {
        let mut rb = super::XbRingBuffer61::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_61_clear() {
        let mut rb = super::XbRingBuffer61::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_61_capacity() {
        let rb = super::XbRingBuffer61::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_61_basic() {
        let h = super::xb_fnv1a_61(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_61(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_61_different_inputs() {
        let h1 = super::xb_fnv1a_61(b"abc");
        let h2 = super::xb_fnv1a_61(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_61_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_61(&data);
        let dec = super::xb_rle_decode_61(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_61_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_61(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_61(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_61_values() {
        assert!((super::xb_clamp_61(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_61(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_61(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_61_values() {
        assert!((super::xb_lerp_61(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_61(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_61(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_61_wrap_around_twice() {
        let mut rb = super::XbRingBuffer61::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 120 ----

    #[test]
    fn xc_120_pool_new_empty() {
        let pool: super::Xc120Pool<i32> = super::Xc120Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_120_pool_release_acquire() {
        let mut pool = super::Xc120Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_120_pool_acquire_empty() {
        let mut pool: super::Xc120Pool<i32> = super::Xc120Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_120_pool_full() {
        let mut pool = super::Xc120Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_120_pool_drain() {
        let mut pool = super::Xc120Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_120_pool_stats() {
        let mut pool = super::Xc120Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_120_pool_clear() {
        let mut pool = super::Xc120Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_120_pool_shrink() {
        let mut pool = super::Xc120Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_120_pool_default() {
        let pool: super::Xc120Pool<String> = super::Xc120Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_120_pool_extend() {
        let mut pool = super::Xc120Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_120_pool_retain() {
        let mut pool = super::Xc120Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_120_scheduler_round_robin() {
        let mut sched = super::Xc120Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_120_scheduler_empty() {
        let mut sched = super::Xc120Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_120_scheduler_reset() {
        let mut sched = super::Xc120Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_120_scheduler_add_remove() {
        let mut sched = super::Xc120Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_120_scheduler_targets() {
        let sched = super::Xc120Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_120_hash_empty() {
        assert_eq!(super::xc_120_hash(b""), 5381);
    }

    #[test]
    fn xc_120_hash_data() {
        let h = super::xc_120_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_120_hash(b"hello"), h);
    }

    #[test]
    fn xc_120_reverse_str() {
        assert_eq!(super::xc_120_reverse("abc"), "cba");
        assert_eq!(super::xc_120_reverse(""), "");
    }


    #[test]
    fn xe_74_pipeline_empty() {
        let p = super::Xe74Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_74_pipeline_parse_stage() {
        let p = super::Xe74Pipeline::new()
            .add_parse(super::xe_74_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_74_pipeline_transform_double() {
        let p = super::Xe74Pipeline::new()
            .add_transform(super::xe_74_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_74_pipeline_validate_reverse() {
        let p = super::Xe74Pipeline::new()
            .add_validate(super::xe_74_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_74_pipeline_emit_filter() {
        let p = super::Xe74Pipeline::new()
            .add_emit(super::xe_74_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_74_pipeline_multi_stage() {
        let p = super::Xe74Pipeline::new()
            .add_parse(super::xe_74_pipeline_identity)
            .add_transform(super::xe_74_pipeline_double)
            .add_validate(super::xe_74_pipeline_reverse)
            .add_emit(super::xe_74_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_74_pipeline_error_propagation() {
        let p = super::Xe74Pipeline::new()
            .add_parse(super::xe_74_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe74Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_74_pipeline_compose() {
        let p1 = super::Xe74Pipeline::new()
            .add_parse(super::xe_74_pipeline_identity);
        let p2 = super::Xe74Pipeline::new()
            .add_transform(super::xe_74_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_74_pipeline_error_display() {
        let e = super::Xe74PipelineError {
            stage: super::Xe74Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_74_cache_put_get() {
        let mut c = super::Xe74Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_74_cache_miss() {
        let mut c: super::Xe74Cache<&str, i32> = super::Xe74Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_74_cache_ttl_expiry() {
        let mut c = super::Xe74Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_74_cache_evict() {
        let mut c = super::Xe74Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_74_cache_capacity() {
        let mut c = super::Xe74Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_74_cache_stats() {
        let mut c = super::Xe74Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_74_cache_clear() {
        let mut c = super::Xe74Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_72 graph tests ------------------------------------------------

    #[test]
    fn xg_72_graph_empty() {
        let g = super::Xg72Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_72_graph_add_node() {
        let mut g = super::Xg72Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_72_graph_add_edge() {
        let mut g = super::Xg72Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_72_graph_neighbors() {
        let mut g = super::Xg72Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_72_graph_has_path() {
        let mut g = super::Xg72Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_72_graph_self_path() {
        let g = super::Xg72Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_72_graph_topo_sort() {
        let mut g = super::Xg72Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_72_graph_cycle_detect_false() {
        let mut g = super::Xg72Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_72_graph_cycle_detect_true() {
        let mut g = super::Xg72Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_72 heap tests -------------------------------------------------

    #[test]
    fn xg_72_heap_empty() {
        let h: super::Xg72Heap<i32> = super::Xg72Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_72_heap_push_pop() {
        let mut h = super::Xg72Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_72_heap_peek() {
        let mut h = super::Xg72Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_72_heap_drain_sorted() {
        let mut h = super::Xg72Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_72_heap_merge() {
        let mut a = super::Xg72Heap::new();
        let mut b = super::Xg72Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_72_heap_default() {
        let h: super::Xg72Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_72_graph_default() {
        let g: super::Xg72Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh119_skip_insert_contains() {
        let mut sl = super::Xh119SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh119_skip_remove() {
        let mut sl = super::Xh119SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh119_skip_len() {
        let mut sl = super::Xh119SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh119_skip_range_query() {
        let mut sl = super::Xh119SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh119_skip_floor_ceiling() {
        let mut sl = super::Xh119SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh119_skip_rank() {
        let mut sl = super::Xh119SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh119_skip_empty() {
        let sl = super::Xh119SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh119_skip_duplicates() {
        let mut sl = super::Xh119SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh119_bitset_set_test() {
        let mut bs = super::Xh119BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh119_bitset_clear_count() {
        let mut bs = super::Xh119BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh119_bitset_and_or_xor() {
        let mut a = super::Xh119BitSet::xh_new(128);
        let mut b = super::Xh119BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh119_bitset_iter_ones() {
        let mut bs = super::Xh119BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh119_bitset_first_last() {
        let mut bs = super::Xh119BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh119_bitset_empty() {
        let bs = super::Xh119BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi119_deque_push_pop_back() {
        let mut dq = super::Xi119Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi119_deque_push_pop_front() {
        let mut dq = super::Xi119Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi119_deque_mixed_ops() {
        let mut dq = super::Xi119Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi119_deque_get_and_split() {
        let mut dq = super::Xi119Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi119_deque_rotate_left() {
        let mut dq = super::Xi119Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi119_deque_rotate_right() {
        let mut dq = super::Xi119Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi119_deque_grow() {
        let mut dq = super::Xi119Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi119_deque_empty() {
        let dq = super::Xi119Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi119_interval_tree_insert_query() {
        let mut tree = super::Xi119IntervalTree::xi_new();
        tree.xi_insert(super::Xi119Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi119Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi119Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi119_interval_tree_overlap() {
        let mut tree = super::Xi119IntervalTree::xi_new();
        tree.xi_insert(super::Xi119Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi119Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi119Interval::xi_new(12, 20));
        let q = super::Xi119Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi119_interval_tree_remove() {
        let mut tree = super::Xi119IntervalTree::xi_new();
        tree.xi_insert(super::Xi119Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi119Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi119_interval_tree_gaps() {
        let mut tree = super::Xi119IntervalTree::xi_new();
        tree.xi_insert(super::Xi119Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi119Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi119Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi119Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi119Interval::xi_new(8, 10));
    }

    #[test]
    fn xi119_interval_tree_merge() {
        let mut tree = super::Xi119IntervalTree::xi_new();
        tree.xi_insert(super::Xi119Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi119Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi119Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi119Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi119Interval::xi_new(10, 15));
    }

    #[test]
    fn xi119_interval_tree_all() {
        let mut tree = super::Xi119IntervalTree::xi_new();
        tree.xi_insert(super::Xi119Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi119Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi119_interval_tree_empty() {
        let tree = super::Xi119IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi119_interval_tree_contains_point() {
        let iv = super::Xi119Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 120) ---

    #[test]
    fn xj_120_uf_make_and_find() {
        let mut uf = super::Xj120UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_120_uf_union_connected() {
        let mut uf = super::Xj120UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_120_uf_component_count() {
        let mut uf = super::Xj120UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_120_uf_component_size() {
        let mut uf = super::Xj120UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_120_uf_largest_component() {
        let mut uf = super::Xj120UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_120_uf_many_elements() {
        let mut uf = super::Xj120UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_120_uf_separate_components() {
        let mut uf = super::Xj120UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_120_uf_path_compression() {
        let mut uf = super::Xj120UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_120_bt_insert_get() {
        let mut bt = super::Xj120BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_120_bt_contains_len() {
        let mut bt = super::Xj120BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_120_bt_replace() {
        let mut bt = super::Xj120BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_120_bt_remove() {
        let mut bt = super::Xj120BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_120_bt_keys_values() {
        let mut bt = super::Xj120BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_120_bt_range() {
        let mut bt = super::Xj120BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_120_bt_min_max() {
        let mut bt = super::Xj120BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_120_bt_many_inserts() {
        let mut bt = super::Xj120BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_120 segment tree tests ---

    #[test]
    fn xk_120_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk120SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_120_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk120SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_120_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk120SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_120_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk120SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_120_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk120SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_120_st_single_element() {
        let data = vec![42];
        let st = super::Xk120SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_120_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk120SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_120_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk120SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_120 disjoint intervals tests ---

    #[test]
    fn xk_120_di_add_and_count() {
        let mut di = super::Xk120DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_120_di_merge_overlap() {
        let mut di = super::Xk120DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_120_di_contains() {
        let mut di = super::Xk120DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_120_di_remove() {
        let mut di = super::Xk120DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_120_di_covered_length() {
        let mut di = super::Xk120DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_120_di_gaps() {
        let mut di = super::Xk120DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_120_di_merge_adjacent() {
        let mut di = super::Xk120DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_120_di_empty() {
        let di = super::Xk120DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_120_rope_new_empty() {
        let rope = super::Xl120Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_120_rope_from_str() {
        let rope = super::Xl120Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_120_rope_insert_at() {
        let mut rope = super::Xl120Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_120_rope_delete_range() {
        let mut rope = super::Xl120Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_120_rope_char_at() {
        let rope = super::Xl120Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_120_rope_split_concat() {
        let rope = super::Xl120Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_120_rope_line_count() {
        let rope = super::Xl120Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_120_rope_line_at() {
        let rope = super::Xl120Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_120_sa_build_and_search() {
        let sa = super::Xl120SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_120_sa_count() {
        let sa = super::Xl120SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_120_sa_longest_repeated() {
        let sa = super::Xl120SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_120_sa_all_positions() {
        let sa = super::Xl120SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_120_sa_len() {
        let sa = super::Xl120SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_120_sa_empty() {
        let sa = super::Xl120SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_120_rope_slice() {
        let rope = super::Xl120Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_120_sa_search_start() {
        let sa = super::Xl120SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_120_sparse_set_get() {
        let mut m = super::Xm120MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_120_sparse_row_col() {
        let mut m = super::Xm120MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_120_sparse_transpose() {
        let mut m = super::Xm120MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_120_sparse_multiply_vec() {
        let mut m = super::Xm120MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_120_sparse_nnz_density() {
        let mut m = super::Xm120MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_120_sparse_clear() {
        let mut m = super::Xm120MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_120_sparse_overwrite_zero() {
        let mut m = super::Xm120MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_120_tokenizer_basic() {
        let t = super::Xm120Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_120_tokenizer_count() {
        let t = super::Xm120Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_120_tokenizer_unique() {
        let t = super::Xm120Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_120_tokenizer_frequency() {
        let t = super::Xm120Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_120_tokenizer_delimiter() {
        let t = super::Xm120Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_120_tokenizer_whitespace() {
        let t = super::Xm120Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_120_tokenizer_empty() {
        let t = super::Xm120Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }

}
