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
}
