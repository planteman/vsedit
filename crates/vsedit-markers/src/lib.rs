//! Diagnostic markers service

use std::collections::HashMap;
use std::sync::Mutex;
use std::fmt;

use vsedit_events::{Emitter, Event};
use vsedit_uri::VsUri;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Severity of a diagnostic marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkerSeverity {
    Hint,
    Info,
    Warning,
    Error,
}

/// Code attached to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkerCode {
    String(String),
    Number(i32),
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
    pub uri: VsUri,
    pub message: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// A single diagnostic marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerData {
    pub severity: MarkerSeverity,
    pub message: String,
    pub source: Option<String>,
    pub code: Option<MarkerCode>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub related_information: Vec<RelatedInformation>,
    pub tags: Vec<MarkerTag>,
}

// ---------------------------------------------------------------------------
// DiagnosticEntry & DiagnosticCollection (high-level API)
// ---------------------------------------------------------------------------

/// Severity of a diagnostic entry (mirrors VS Code DiagnosticSeverity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl DiagnosticSeverity {
    /// Convert to the lower-level `MarkerSeverity`.
    pub fn to_marker_severity(self) -> MarkerSeverity {
        match self {
            DiagnosticSeverity::Error => MarkerSeverity::Error,
            DiagnosticSeverity::Warning => MarkerSeverity::Warning,
            DiagnosticSeverity::Info => MarkerSeverity::Info,
            DiagnosticSeverity::Hint => MarkerSeverity::Hint,
        }
    }
}

/// A single diagnostic entry attached to a resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticEntry {
    pub uri: VsUri,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub source: Option<String>,
    pub code: Option<String>,
}

impl DiagnosticEntry {
    pub fn new(uri: &VsUri, line: u32, col: u32, severity: DiagnosticSeverity, message: impl Into<String>) -> Self {
        Self {
            uri: uri.clone(),
            start_line: line,
            start_col: col,
            end_line: line,
            end_col: col,
            message: message.into(),
            severity,
            source: None,
            code: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_range(mut self, start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        self.start_line = start_line;
        self.start_col = start_col;
        self.end_line = end_line;
        self.end_col = end_col;
        self
    }

    /// Convert to a `MarkerData` for storage in the `MarkerService`.
    pub fn to_marker_data(&self) -> MarkerData {
        MarkerData {
            severity: self.severity.to_marker_severity(),
            message: self.message.clone(),
            source: self.source.clone(),
            code: self.code.as_ref().map(|c| MarkerCode::String(c.clone())),
            start_line: self.start_line,
            start_column: self.start_col,
            end_line: self.end_line,
            end_column: self.end_col,
            related_information: vec![],
            tags: vec![],
        }
    }
}

/// A named collection of diagnostics, analogous to VS Code's `DiagnosticCollection`.
///
/// Each collection has a `name` (the source/owner) and stores diagnostics keyed by URI.
#[derive(Debug, Clone)]
pub struct DiagnosticCollection {
    name: String,
    entries: HashMap<VsUri, Vec<DiagnosticEntry>>,
}

impl DiagnosticCollection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entries: HashMap::new(),
        }
    }

    /// The source/owner name of this collection.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Set diagnostics for a URI, replacing any previously stored.
    pub fn set(&mut self, uri: &VsUri, diagnostics: Vec<DiagnosticEntry>) {
        if diagnostics.is_empty() {
            self.entries.remove(uri);
        } else {
            self.entries.insert(uri.clone(), diagnostics);
        }
    }

    /// Delete all diagnostics for a URI.
    pub fn delete(&mut self, uri: &VsUri) {
        self.entries.remove(uri);
    }

    /// Clear all diagnostics from this collection.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get diagnostics for a specific URI.
    pub fn get(&self, uri: &VsUri) -> Option<&[DiagnosticEntry]> {
        self.entries.get(uri).map(|v| v.as_slice())
    }

    /// Iterate over all `(uri, diagnostics)` pairs.
    pub fn entries(&self) -> impl Iterator<Item = (&VsUri, &[DiagnosticEntry])> {
        self.entries.iter().map(|(k, v)| (k, v.as_slice()))
    }

    /// Total number of diagnostics across all URIs.
    pub fn total_count(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }

    /// Number of URIs with diagnostics.
    pub fn uri_count(&self) -> usize {
        self.entries.len()
    }

    /// Push all diagnostics into a `MarkerService` under this collection's name.
    pub fn sync_to_marker_service(&self, service: &MarkerService) {
        let pairs: Vec<(VsUri, Vec<MarkerData>)> = self
            .entries
            .iter()
            .map(|(uri, diags)| {
                let markers = diags.iter().map(|d| d.to_marker_data()).collect();
                (uri.clone(), markers)
            })
            .collect();
        service.change_all(&self.name, pairs);
    }
}

// ---------------------------------------------------------------------------
// Filter & Statistics
// ---------------------------------------------------------------------------

/// Filter for querying markers.
pub struct MarkerFilter {
    pub owner: Option<String>,
    pub uri: Option<VsUri>,
    pub severities: Option<Vec<MarkerSeverity>>,
    pub take: Option<usize>,
}

/// Aggregate counts by severity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerStatistics {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub hints: usize,
}

// ---------------------------------------------------------------------------
// MarkerService
// ---------------------------------------------------------------------------

/// Manages diagnostic markers per (owner, resource) pair.
pub struct MarkerService {
    markers: Mutex<HashMap<(String, VsUri), Vec<MarkerData>>>,
    on_marker_changed: Emitter<Vec<VsUri>>,
}

impl MarkerService {
    pub fn new() -> Self {
        Self {
            markers: Mutex::new(HashMap::new()),
            on_marker_changed: Emitter::new(),
        }
    }

    /// Set markers for a single resource owned by `owner`.
    pub fn change_one(&self, owner: &str, uri: &VsUri, markers: Vec<MarkerData>) {
        let key = (owner.to_string(), uri.clone());
        let mut map = self.markers.lock().unwrap();
        if markers.is_empty() {
            map.remove(&key);
        } else {
            map.insert(key, markers);
        }
        drop(map);
        self.on_marker_changed.fire(&vec![uri.clone()]);
    }

    /// Set markers for multiple resources owned by `owner`, firing a single event.
    pub fn change_all(&self, owner: &str, markers: Vec<(VsUri, Vec<MarkerData>)>) {
        let mut changed: Vec<VsUri> = Vec::new();
        let mut map = self.markers.lock().unwrap();
        for (uri, data) in markers {
            let key = (owner.to_string(), uri.clone());
            if data.is_empty() {
                map.remove(&key);
            } else {
                map.insert(key, data);
            }
            changed.push(uri);
        }
        drop(map);
        if !changed.is_empty() {
            self.on_marker_changed.fire(&changed);
        }
    }

    /// Read markers matching a filter. Returns `(uri, marker)` pairs.
    pub fn read(&self, filter: &MarkerFilter) -> Vec<(VsUri, MarkerData)> {
        let map = self.markers.lock().unwrap();
        let mut results: Vec<(VsUri, MarkerData)> = Vec::new();

        for ((owner, uri), data) in map.iter() {
            if let Some(ref fo) = filter.owner {
                if owner != fo {
                    continue;
                }
            }
            if let Some(ref fu) = filter.uri {
                if uri != fu {
                    continue;
                }
            }
            for marker in data {
                if let Some(ref sevs) = filter.severities {
                    if !sevs.contains(&marker.severity) {
                        continue;
                    }
                }
                results.push((uri.clone(), marker.clone()));
                if let Some(take) = filter.take {
                    if results.len() >= take {
                        return results;
                    }
                }
            }
        }
        results
    }

    /// Remove all markers for `owner` on the given URIs.
    pub fn remove(&self, owner: &str, uris: &[VsUri]) {
        let mut map = self.markers.lock().unwrap();
        let mut changed: Vec<VsUri> = Vec::new();
        for uri in uris {
            let key = (owner.to_string(), uri.clone());
            if map.remove(&key).is_some() {
                changed.push(uri.clone());
            }
        }
        drop(map);
        if !changed.is_empty() {
            self.on_marker_changed.fire(&changed);
        }
    }

    /// Subscribe to marker-change events.
    pub fn on_marker_changed(&self) -> Event<Vec<VsUri>> {
        self.on_marker_changed.event()
    }

    /// Aggregate statistics across all stored markers.
    pub fn get_statistics(&self) -> MarkerStatistics {
        let map = self.markers.lock().unwrap();
        let mut stats = MarkerStatistics {
            errors: 0,
            warnings: 0,
            infos: 0,
            hints: 0,
        };
        for data in map.values() {
            for m in data {
                match m.severity {
                    MarkerSeverity::Error => stats.errors += 1,
                    MarkerSeverity::Warning => stats.warnings += 1,
                    MarkerSeverity::Info => stats.infos += 1,
                    MarkerSeverity::Hint => stats.hints += 1,
                }
            }
        }
        stats
    }
}

impl Default for MarkerService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// MarkerFilter builder & MarkerSummary
// ---------------------------------------------------------------------------

/// Builder-style filter for querying markers by severity, source, or tag.
pub struct MarkerQueryFilter {
    pub severities: Option<Vec<MarkerSeverity>>,
    pub source: Option<String>,
    pub tags: Option<Vec<MarkerTag>>,
}

impl MarkerQueryFilter {
    pub fn new() -> Self {
        Self {
            severities: None,
            source: None,
            tags: None,
        }
    }

    pub fn with_severities(mut self, sevs: Vec<MarkerSeverity>) -> Self {
        self.severities = Some(sevs);
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_tags(mut self, tags: Vec<MarkerTag>) -> Self {
        self.tags = Some(tags);
        self
    }

    /// Returns `true` if `marker` passes all filter criteria.
    pub fn matches(&self, marker: &MarkerData) -> bool {
        if let Some(ref sevs) = self.severities {
            if !sevs.contains(&marker.severity) {
                return false;
            }
        }
        if let Some(ref src) = self.source {
            if marker.source.as_deref() != Some(src.as_str()) {
                return false;
            }
        }
        if let Some(ref tags) = self.tags {
            if !tags.iter().any(|t| marker.tags.contains(t)) {
                return false;
            }
        }
        true
    }
}

impl Default for MarkerQueryFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Aggregated summary of markers grouped by severity and source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerSummary {
    pub count_by_severity: HashMap<MarkerSeverity, usize>,
    pub count_by_source: HashMap<String, usize>,
    pub total: usize,
}

impl MarkerSummary {
    /// Build a summary from a slice of markers.
    pub fn from_markers(markers: &[MarkerData]) -> Self {
        let mut count_by_severity: HashMap<MarkerSeverity, usize> = HashMap::new();
        let mut count_by_source: HashMap<String, usize> = HashMap::new();
        for m in markers {
            *count_by_severity.entry(m.severity).or_insert(0) += 1;
            let src = m.source.clone().unwrap_or_else(|| "(unknown)".to_string());
            *count_by_source.entry(src).or_insert(0) += 1;
        }
        Self {
            count_by_severity,
            count_by_source,
            total: markers.len(),
        }
    }

    pub fn error_count(&self) -> usize {
        self.count_by_severity.get(&MarkerSeverity::Error).copied().unwrap_or(0)
    }

    pub fn warning_count(&self) -> usize {
        self.count_by_severity.get(&MarkerSeverity::Warning).copied().unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// MarkerCollection
// ---------------------------------------------------------------------------

/// A collection that aggregates markers from multiple owners for a single file.
pub struct MarkerCollection {
    uri: VsUri,
    markers_by_owner: HashMap<String, Vec<MarkerData>>,
}

impl MarkerCollection {
    pub fn new(uri: VsUri) -> Self {
        Self {
            uri,
            markers_by_owner: HashMap::new(),
        }
    }

    /// Set (replace) markers for the given owner.
    pub fn set_markers(&mut self, owner: &str, markers: Vec<MarkerData>) {
        if markers.is_empty() {
            self.markers_by_owner.remove(owner);
        } else {
            self.markers_by_owner.insert(owner.to_string(), markers);
        }
    }

    /// Remove an owner and its markers. Returns `true` if the owner existed.
    pub fn remove_owner(&mut self, owner: &str) -> bool {
        self.markers_by_owner.remove(owner).is_some()
    }

    /// Get the markers for a specific owner.
    pub fn get_markers(&self, owner: &str) -> Option<&[MarkerData]> {
        self.markers_by_owner.get(owner).map(|v| v.as_slice())
    }

    /// Return references to all markers from every owner, flattened.
    pub fn all_markers(&self) -> Vec<&MarkerData> {
        self.markers_by_owner.values().flat_map(|v| v.iter()).collect()
    }

    /// Total number of markers across all owners.
    pub fn total_count(&self) -> usize {
        self.markers_by_owner.values().map(|v| v.len()).sum()
    }

    /// Number of distinct owners.
    pub fn owner_count(&self) -> usize {
        self.markers_by_owner.len()
    }

    /// Count markers matching a given severity across all owners.
    pub fn severity_count(&self, severity: MarkerSeverity) -> usize {
        self.markers_by_owner
            .values()
            .flat_map(|v| v.iter())
            .filter(|m| m.severity == severity)
            .count()
    }

    /// Returns `true` if any marker has `Error` severity.
    pub fn has_errors(&self) -> bool {
        self.markers_by_owner
            .values()
            .flat_map(|v| v.iter())
            .any(|m| m.severity == MarkerSeverity::Error)
    }

    /// Remove all owners and their markers.
    pub fn clear(&mut self) {
        self.markers_by_owner.clear();
    }

    /// The URI this collection belongs to.
    pub fn uri(&self) -> &VsUri {
        &self.uri
    }

    /// Returns `true` if there are no markers from any owner.
    pub fn is_empty(&self) -> bool {
        self.markers_by_owner.values().all(|v| v.is_empty())
    }
}

// ---------------------------------------------------------------------------
// Terminal rendering helpers
// ---------------------------------------------------------------------------

/// Return a terminal-friendly icon string for a marker severity.
pub fn marker_severity_icon(severity: MarkerSeverity) -> &'static str {
    match severity {
        MarkerSeverity::Error => "✖",
        MarkerSeverity::Warning => "⚠",
        MarkerSeverity::Info => "ℹ",
        MarkerSeverity::Hint => "💡",
    }
}

/// Return a short label for a marker severity.
pub fn marker_severity_label(severity: MarkerSeverity) -> &'static str {
    match severity {
        MarkerSeverity::Error => "error",
        MarkerSeverity::Warning => "warning",
        MarkerSeverity::Info => "info",
        MarkerSeverity::Hint => "hint",
    }
}

/// Format a marker for display in a terminal problems panel.
pub fn format_marker_for_terminal(marker: &MarkerData, uri: &VsUri) -> String {
    let icon = marker_severity_icon(marker.severity);
    let source_part = match &marker.source {
        Some(s) => format!(" [{s}]"),
        None => String::new(),
    };
    format!(
        "{icon} {uri}:{}:{} - {}{source_part}",
        marker.start_line, marker.start_column, marker.message,
    )
}

// ---------------------------------------------------------------------------
// WorkspaceMarkerStats
// ---------------------------------------------------------------------------

/// Workspace-wide marker statistics across all files and owners.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMarkerStats {
    pub total_files: usize,
    pub total_markers: usize,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub hints: usize,
    pub files_with_errors: usize,
}

impl std::fmt::Display for WorkspaceMarkerStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} errors, {} warnings, {} info in {} files",
            self.errors, self.warnings, self.infos, self.total_files,
        )
    }
}

/// Compute workspace-wide marker statistics from a `MarkerService`.
pub fn markers_stats(service: &MarkerService) -> WorkspaceMarkerStats {
    let map = service.markers.lock().unwrap();

    // Collect per-URI aggregated counts.
    let mut per_uri: HashMap<&VsUri, (usize, usize, usize, usize)> = HashMap::new();

    for ((_, uri), data) in map.iter() {
        let entry = per_uri.entry(uri).or_insert((0, 0, 0, 0));
        for m in data {
            match m.severity {
                MarkerSeverity::Error => entry.0 += 1,
                MarkerSeverity::Warning => entry.1 += 1,
                MarkerSeverity::Info => entry.2 += 1,
                MarkerSeverity::Hint => entry.3 += 1,
            }
        }
    }

    let mut stats = WorkspaceMarkerStats {
        total_files: per_uri.len(),
        total_markers: 0,
        errors: 0,
        warnings: 0,
        infos: 0,
        hints: 0,
        files_with_errors: 0,
    };

    for (e, w, i, h) in per_uri.values() {
        stats.errors += e;
        stats.warnings += w;
        stats.infos += i;
        stats.hints += h;
        stats.total_markers += e + w + i + h;
        if *e > 0 {
            stats.files_with_errors += 1;
        }
    }

    stats
}

// ---------------------------------------------------------------------------
// QuickFix association tracking
// ---------------------------------------------------------------------------

/// A quick-fix action that can be applied to resolve a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickFix {
    pub title: String,
    pub replacement_text: Option<String>,
    pub is_preferred: bool,
}

impl QuickFix {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            replacement_text: None,
            is_preferred: false,
        }
    }

    pub fn with_replacement(mut self, text: impl Into<String>) -> Self {
        self.replacement_text = Some(text.into());
        self
    }

    pub fn preferred(mut self) -> Self {
        self.is_preferred = true;
        self
    }
}

/// Registry that associates quick-fix actions with markers by (uri, line, message) key.
pub struct QuickFixRegistry {
    /// Key: (uri, start_line, message) → list of quick-fixes.
    fixes: HashMap<(VsUri, u32, String), Vec<QuickFix>>,
}

impl QuickFixRegistry {
    pub fn new() -> Self {
        Self {
            fixes: HashMap::new(),
        }
    }

    /// Register quick-fixes for a specific marker identified by URI, line, and message.
    pub fn register(&mut self, uri: &VsUri, marker: &MarkerData, fixes: Vec<QuickFix>) {
        let key = (uri.clone(), marker.start_line, marker.message.clone());
        self.fixes.entry(key).or_default().extend(fixes);
    }

    /// Look up quick-fixes for a marker.
    pub fn get_fixes(&self, uri: &VsUri, marker: &MarkerData) -> Option<&[QuickFix]> {
        let key = (uri.clone(), marker.start_line, marker.message.clone());
        self.fixes.get(&key).map(|v| v.as_slice())
    }

    /// Return only the preferred quick-fix for a marker, if any.
    pub fn preferred_fix(&self, uri: &VsUri, marker: &MarkerData) -> Option<&QuickFix> {
        self.get_fixes(uri, marker)
            .and_then(|fixes| fixes.iter().find(|f| f.is_preferred))
    }

    /// Remove all quick-fixes for a given URI.
    pub fn clear_uri(&mut self, uri: &VsUri) {
        self.fixes.retain(|(u, _, _), _| u != uri);
    }

    /// Remove all registered quick-fixes.
    pub fn clear(&mut self) {
        self.fixes.clear();
    }

    /// Total number of marker-to-fix associations.
    pub fn total_associations(&self) -> usize {
        self.fixes.len()
    }

    /// Total number of individual quick-fix actions.
    pub fn total_fixes(&self) -> usize {
        self.fixes.values().map(|v| v.len()).sum()
    }
}

impl Default for QuickFixRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Marker staleness detection
// ---------------------------------------------------------------------------

/// Tracks file versions and marks diagnostics as stale when the file changes.
pub struct StalenessTracker {
    /// Maps URI → version at which diagnostics were last set.
    diagnostic_versions: HashMap<VsUri, u64>,
    /// Maps URI → current file version.
    file_versions: HashMap<VsUri, u64>,
}

impl StalenessTracker {
    pub fn new() -> Self {
        Self {
            diagnostic_versions: HashMap::new(),
            file_versions: HashMap::new(),
        }
    }

    /// Record that a file has been modified, bumping its version.
    pub fn notify_file_changed(&mut self, uri: &VsUri) {
        let v = self.file_versions.entry(uri.clone()).or_insert(0);
        *v += 1;
    }

    /// Set the file version explicitly (e.g. from editor save events).
    pub fn set_file_version(&mut self, uri: &VsUri, version: u64) {
        self.file_versions.insert(uri.clone(), version);
    }

    /// Record that diagnostics were produced for a file at its current version.
    pub fn mark_diagnostics_fresh(&mut self, uri: &VsUri) {
        let current = self.file_versions.get(uri).copied().unwrap_or(0);
        self.diagnostic_versions.insert(uri.clone(), current);
    }

    /// Returns `true` if the file has changed since diagnostics were last set.
    pub fn is_stale(&self, uri: &VsUri) -> bool {
        let file_v = self.file_versions.get(uri).copied().unwrap_or(0);
        let diag_v = self.diagnostic_versions.get(uri).copied();
        match diag_v {
            Some(dv) => dv < file_v,
            None => file_v > 0,
        }
    }

    /// Return all URIs whose diagnostics are currently stale.
    pub fn stale_uris(&self) -> Vec<VsUri> {
        self.file_versions
            .keys()
            .filter(|uri| self.is_stale(uri))
            .cloned()
            .collect()
    }

    /// Remove tracking data for a URI (e.g. when a file is closed).
    pub fn remove(&mut self, uri: &VsUri) {
        self.file_versions.remove(uri);
        self.diagnostic_versions.remove(uri);
    }
}

impl Default for StalenessTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Marker diff between diagnostic runs
// ---------------------------------------------------------------------------

/// The result of comparing two sets of markers for the same URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerDiff {
    pub added: Vec<MarkerData>,
    pub removed: Vec<MarkerData>,
    pub unchanged: Vec<MarkerData>,
}

impl MarkerDiff {
    /// Compute the diff between an old and new set of markers.
    ///
    /// Uses equality on `MarkerData` to determine added/removed/unchanged.
    pub fn compute(old: &[MarkerData], new: &[MarkerData]) -> Self {
        let mut removed = Vec::new();
        let mut unchanged = Vec::new();
        let mut new_remaining: Vec<&MarkerData> = new.iter().collect();

        for old_m in old {
            if let Some(pos) = new_remaining.iter().position(|n| *n == old_m) {
                unchanged.push(old_m.clone());
                new_remaining.remove(pos);
            } else {
                removed.push(old_m.clone());
            }
        }

        let added: Vec<MarkerData> = new_remaining.into_iter().cloned().collect();

        Self {
            added,
            removed,
            unchanged,
        }
    }

    /// Returns `true` if there are no changes between old and new.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    /// Number of markers that changed (added + removed).
    pub fn change_count(&self) -> usize {
        self.added.len() + self.removed.len()
    }

    /// Summary string for the diff.
    pub fn summary(&self) -> String {
        format!(
            "+{} -{} ={} markers",
            self.added.len(),
            self.removed.len(),
            self.unchanged.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Marker grouping by file (sorted output)
// ---------------------------------------------------------------------------

/// A file's markers sorted by line number, with severity counts.
#[derive(Debug, Clone)]
pub struct FileMarkerGroup {
    pub uri: VsUri,
    pub markers: Vec<MarkerData>,
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    pub hint_count: usize,
}

/// Group markers by file URI and sort each group by (severity desc, line asc).
///
/// Files are sorted by URI for deterministic output. Within each file, markers
/// are sorted with errors first, then warnings, info, and hints.
pub fn group_markers_by_file(pairs: &[(VsUri, MarkerData)]) -> Vec<FileMarkerGroup> {
    let mut by_uri: HashMap<VsUri, Vec<MarkerData>> = HashMap::new();
    for (uri, marker) in pairs {
        by_uri.entry(uri.clone()).or_default().push(marker.clone());
    }

    let severity_order = |s: &MarkerSeverity| -> u8 {
        match s {
            MarkerSeverity::Error => 0,
            MarkerSeverity::Warning => 1,
            MarkerSeverity::Info => 2,
            MarkerSeverity::Hint => 3,
        }
    };

    let mut groups: Vec<FileMarkerGroup> = by_uri
        .into_iter()
        .map(|(uri, mut markers)| {
            markers.sort_by(|a, b| {
                severity_order(&a.severity)
                    .cmp(&severity_order(&b.severity))
                    .then_with(|| a.start_line.cmp(&b.start_line))
            });
            let error_count = markers.iter().filter(|m| m.severity == MarkerSeverity::Error).count();
            let warning_count = markers.iter().filter(|m| m.severity == MarkerSeverity::Warning).count();
            let info_count = markers.iter().filter(|m| m.severity == MarkerSeverity::Info).count();
            let hint_count = markers.iter().filter(|m| m.severity == MarkerSeverity::Hint).count();
            FileMarkerGroup {
                uri,
                markers,
                error_count,
                warning_count,
                info_count,
                hint_count,
            }
        })
        .collect();

    groups.sort_by(|a, b| a.uri.cmp(&b.uri));
    groups
}

// ---------------------------------------------------------------------------
// Per-URI severity statistics
// ---------------------------------------------------------------------------

/// Severity counts for a single URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriMarkerStats {
    pub uri: VsUri,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub hints: usize,
}

impl UriMarkerStats {
    pub fn total(&self) -> usize {
        self.errors + self.warnings + self.infos + self.hints
    }

    pub fn has_errors(&self) -> bool {
        self.errors > 0
    }

    /// The highest (most severe) severity present, if any markers exist.
    pub fn worst_severity(&self) -> Option<MarkerSeverity> {
        if self.errors > 0 {
            Some(MarkerSeverity::Error)
        } else if self.warnings > 0 {
            Some(MarkerSeverity::Warning)
        } else if self.infos > 0 {
            Some(MarkerSeverity::Info)
        } else if self.hints > 0 {
            Some(MarkerSeverity::Hint)
        } else {
            None
        }
    }
}

impl MarkerService {
    /// Compute per-URI severity statistics across all owners.
    pub fn per_uri_stats(&self) -> Vec<UriMarkerStats> {
        let map = self.markers.lock().unwrap();
        let mut per_uri: HashMap<VsUri, (usize, usize, usize, usize)> = HashMap::new();

        for ((_, uri), data) in map.iter() {
            let entry = per_uri.entry(uri.clone()).or_insert((0, 0, 0, 0));
            for m in data {
                match m.severity {
                    MarkerSeverity::Error => entry.0 += 1,
                    MarkerSeverity::Warning => entry.1 += 1,
                    MarkerSeverity::Info => entry.2 += 1,
                    MarkerSeverity::Hint => entry.3 += 1,
                }
            }
        }

        let mut stats: Vec<UriMarkerStats> = per_uri
            .into_iter()
            .map(|(uri, (e, w, i, h))| UriMarkerStats {
                uri,
                errors: e,
                warnings: w,
                infos: i,
                hints: h,
            })
            .collect();

        stats.sort_by(|a, b| a.uri.cmp(&b.uri));
        stats
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// MarkerQuickNavigation
// ---------------------------------------------------------------------------

pub struct MarkerQuickNavigation {
    current_index: Option<usize>,
    markers: Vec<(VsUri, u32)>,
}

impl MarkerQuickNavigation {
    pub fn new() -> Self { Self { current_index: None, markers: Vec::new() } }

    pub fn load_markers(&mut self, service: &MarkerService) {
        let all = service.read(&MarkerFilter { owner: None, uri: None, severities: None, take: None });
        self.markers = all.into_iter().map(|(uri, m)| (uri, m.start_line)).collect();
        self.markers.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
        self.current_index = None;
    }

    pub fn next(&mut self) -> Option<(&VsUri, u32)> {
        if self.markers.is_empty() { return None; }
        let idx = match self.current_index {
            Some(i) => (i + 1) % self.markers.len(),
            None => 0,
        };
        self.current_index = Some(idx);
        let (ref uri, line) = self.markers[idx];
        Some((uri, line))
    }

    pub fn prev(&mut self) -> Option<(&VsUri, u32)> {
        if self.markers.is_empty() { return None; }
        let idx = match self.current_index {
            Some(0) | None => self.markers.len() - 1,
            Some(i) => i - 1,
        };
        self.current_index = Some(idx);
        let (ref uri, line) = self.markers[idx];
        Some((uri, line))
    }

    pub fn count(&self) -> usize { self.markers.len() }
    pub fn current_index(&self) -> Option<usize> { self.current_index }
}

impl Default for MarkerQuickNavigation { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// MarkerCodeActionLink
// ---------------------------------------------------------------------------

pub struct MarkerCodeActionLink {
    pub marker_owner: String,
    pub action_title: String,
    pub uri: VsUri,
    pub line: u32,
}

impl MarkerCodeActionLink {
    pub fn new(owner: impl Into<String>, title: impl Into<String>, uri: VsUri, line: u32) -> Self {
        Self { marker_owner: owner.into(), action_title: title.into(), uri, line }
    }
}

impl std::fmt::Display for MarkerCodeActionLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {} at L{}", self.marker_owner, self.action_title, self.line)
    }
}

/// Collects code action links for markers.
pub struct MarkerCodeActionLinker {
    links: Vec<MarkerCodeActionLink>,
}

impl MarkerCodeActionLinker {
    pub fn new() -> Self { Self { links: Vec::new() } }
    pub fn add(&mut self, link: MarkerCodeActionLink) { self.links.push(link); }
    pub fn links_for_uri(&self, uri: &VsUri) -> Vec<&MarkerCodeActionLink> {
        self.links.iter().filter(|l| &l.uri == uri).collect()
    }
    pub fn len(&self) -> usize { self.links.len() }
    pub fn is_empty(&self) -> bool { self.links.is_empty() }
}

impl Default for MarkerCodeActionLinker { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// MarkerBatchUpdater
// ---------------------------------------------------------------------------

pub struct MarkerBatchUpdate {
    pub owner: String,
    pub uri: VsUri,
    pub markers: Vec<MarkerData>,
}

pub struct MarkerBatchUpdater {
    updates: Vec<MarkerBatchUpdate>,
}

impl MarkerBatchUpdater {
    pub fn new() -> Self { Self { updates: Vec::new() } }

    pub fn queue(&mut self, owner: impl Into<String>, uri: VsUri, markers: Vec<MarkerData>) {
        self.updates.push(MarkerBatchUpdate { owner: owner.into(), uri, markers });
    }

    pub fn apply(&mut self, service: &MarkerService) {
        for update in self.updates.drain(..) {
            service.change_one(&update.owner, &update.uri, update.markers);
        }
    }

    pub fn pending_count(&self) -> usize { self.updates.len() }
    pub fn is_empty(&self) -> bool { self.updates.is_empty() }
    pub fn clear(&mut self) { self.updates.clear(); }
}

impl Default for MarkerBatchUpdater { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// MarkerOwnerTracker
// ---------------------------------------------------------------------------

pub struct MarkerOwnerTracker {
    owners: std::collections::HashMap<String, u64>,
}

impl MarkerOwnerTracker {
    pub fn new() -> Self { Self { owners: std::collections::HashMap::new() } }

    pub fn record(&mut self, owner: &str) {
        *self.owners.entry(owner.to_string()).or_insert(0) += 1;
    }

    pub fn count_for(&self, owner: &str) -> u64 { self.owners.get(owner).copied().unwrap_or(0) }
    pub fn unique_owners(&self) -> Vec<&str> { self.owners.keys().map(|k| k.as_str()).collect() }
    pub fn total_markers(&self) -> u64 { self.owners.values().sum() }
    pub fn clear(&mut self) { self.owners.clear(); }
}

impl Default for MarkerOwnerTracker { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// MarkerNavigationRing – circular navigation through markers
// ---------------------------------------------------------------------------

/// An entry in the navigation ring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavRingEntry {
    pub uri: String,
    pub line: u32,
    pub column: u32,
    pub severity: MarkerSeverity,
    pub message: String,
}

/// Circular buffer for navigating through diagnostic markers.
#[derive(Debug)]
pub struct MarkerNavigationRing {
    entries: Vec<NavRingEntry>,
    cursor: Option<usize>,
    wrap_around: bool,
}

impl MarkerNavigationRing {
    pub fn new(wrap_around: bool) -> Self {
        Self { entries: Vec::new(), cursor: None, wrap_around }
    }

    /// Replace all entries (resets cursor).
    pub fn set_entries(&mut self, entries: Vec<NavRingEntry>) {
        self.entries = entries;
        self.cursor = if self.entries.is_empty() { None } else { Some(0) };
    }

    /// Move to the next marker. Returns it if available.
    pub fn next(&mut self) -> Option<&NavRingEntry> {
        if self.entries.is_empty() { return None; }
        let idx = match self.cursor {
            Some(i) => {
                let next = i + 1;
                if next >= self.entries.len() {
                    if self.wrap_around { 0 } else { return None; }
                } else {
                    next
                }
            }
            None => 0,
        };
        self.cursor = Some(idx);
        self.entries.get(idx)
    }

    /// Move to the previous marker.
    pub fn prev(&mut self) -> Option<&NavRingEntry> {
        if self.entries.is_empty() { return None; }
        let idx = match self.cursor {
            Some(0) => {
                if self.wrap_around { self.entries.len() - 1 } else { return None; }
            }
            Some(i) => i - 1,
            None => 0,
        };
        self.cursor = Some(idx);
        self.entries.get(idx)
    }

    /// Current entry without moving.
    pub fn current(&self) -> Option<&NavRingEntry> {
        self.cursor.and_then(|i| self.entries.get(i))
    }

    /// Jump to a specific index.
    pub fn jump_to(&mut self, index: usize) -> Option<&NavRingEntry> {
        if index < self.entries.len() {
            self.cursor = Some(index);
            self.entries.get(index)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn cursor_index(&self) -> Option<usize> { self.cursor }

    /// Filter entries by severity, returning a new ring.
    pub fn filter_severity(&self, severity: MarkerSeverity) -> Self {
        let filtered = self.entries.iter()
            .filter(|e| e.severity == severity)
            .cloned()
            .collect::<Vec<_>>();
        let mut ring = Self::new(self.wrap_around);
        ring.set_entries(filtered);
        ring
    }

    /// Count entries by severity.
    pub fn count_by_severity(&self, severity: MarkerSeverity) -> usize {
        self.entries.iter().filter(|e| e.severity == severity).count()
    }
}

// ---------------------------------------------------------------------------
// MarkerFilterExpressionParser – parses filter expressions
// ---------------------------------------------------------------------------

/// A token in a filter expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterToken {
    /// A bare text term to match against message.
    Text(String),
    /// A key:value filter (e.g. "file:*.rs").
    KeyValue(String, String),
    /// Boolean AND.
    And,
    /// Boolean OR.
    Or,
    /// Boolean NOT (prefix).
    Not,
}

/// Parsed filter expression tree node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterExprNode {
    Term(String),
    Field(String, String),
    And(Box<FilterExprNode>, Box<FilterExprNode>),
    Or(Box<FilterExprNode>, Box<FilterExprNode>),
    Not(Box<FilterExprNode>),
}

/// Parses filter expressions like `"error AND file:*.rs"`.
#[derive(Debug)]
pub struct MarkerFilterExpressionParser;

impl MarkerFilterExpressionParser {
    /// Tokenize a raw filter string.
    pub fn tokenize(input: &str) -> Vec<FilterToken> {
        let mut tokens = Vec::new();
        for word in input.split_whitespace() {
            match word {
                "AND" | "&&" => tokens.push(FilterToken::And),
                "OR" | "||" => tokens.push(FilterToken::Or),
                "NOT" | "!" => tokens.push(FilterToken::Not),
                _ => {
                    if let Some((key, value)) = word.split_once(':') {
                        tokens.push(FilterToken::KeyValue(key.to_string(), value.to_string()));
                    } else {
                        tokens.push(FilterToken::Text(word.to_string()));
                    }
                }
            }
        }
        tokens
    }

    /// Parse tokens into an expression tree (simple left-to-right precedence).
    pub fn parse(input: &str) -> Option<FilterExprNode> {
        let tokens = Self::tokenize(input);
        if tokens.is_empty() { return None; }

        let mut iter = tokens.into_iter().peekable();
        let mut result = Self::parse_primary(&mut iter)?;

        while let Some(tok) = iter.peek() {
            match tok {
                FilterToken::And => {
                    iter.next();
                    let right = Self::parse_primary(&mut iter)?;
                    result = FilterExprNode::And(Box::new(result), Box::new(right));
                }
                FilterToken::Or => {
                    iter.next();
                    let right = Self::parse_primary(&mut iter)?;
                    result = FilterExprNode::Or(Box::new(result), Box::new(right));
                }
                _ => break,
            }
        }

        Some(result)
    }

    fn parse_primary(iter: &mut std::iter::Peekable<std::vec::IntoIter<FilterToken>>) -> Option<FilterExprNode> {
        let tok = iter.next()?;
        match tok {
            FilterToken::Text(s) => Some(FilterExprNode::Term(s)),
            FilterToken::KeyValue(k, v) => Some(FilterExprNode::Field(k, v)),
            FilterToken::Not => {
                let inner = Self::parse_primary(iter)?;
                Some(FilterExprNode::Not(Box::new(inner)))
            }
            _ => None,
        }
    }

    /// Check if a marker entry matches the parsed expression.
    pub fn matches(node: &FilterExprNode, entry: &NavRingEntry) -> bool {
        match node {
            FilterExprNode::Term(t) => {
                let lower = t.to_lowercase();
                entry.message.to_lowercase().contains(&lower)
            }
            FilterExprNode::Field(key, value) => {
                match key.as_str() {
                    "file" => Self::glob_match(&entry.uri, value),
                    "severity" | "sev" => format!("{:?}", entry.severity).to_lowercase() == value.to_lowercase(),
                    "line" => entry.line.to_string() == *value,
                    _ => false,
                }
            }
            FilterExprNode::And(l, r) => Self::matches(l, entry) && Self::matches(r, entry),
            FilterExprNode::Or(l, r) => Self::matches(l, entry) || Self::matches(r, entry),
            FilterExprNode::Not(inner) => !Self::matches(inner, entry),
        }
    }

    /// Simple glob matching (only `*` is supported as wildcard).
    fn glob_match(text: &str, pattern: &str) -> bool {
        if pattern == "*" { return true; }
        if let Some(suffix) = pattern.strip_prefix('*') {
            return text.ends_with(suffix);
        }
        if let Some(prefix) = pattern.strip_suffix('*') {
            return text.starts_with(prefix);
        }
        text == pattern
    }
}



// ─── MkrBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for marker events.
#[derive(Debug, Clone)]
pub struct MkrBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> MkrBufRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for MkrBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MkrBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── MkrBld Builder & Validator ─────────────────────────────

/// Builder for constructing marker configurations.
#[derive(Debug, Clone)]
pub struct MkrBldBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl MkrBldBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<MkrBldCfg, MkrBldBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(MkrBldBuildErr { errors }); }
        Ok(MkrBldCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated marker configuration.
#[derive(Debug, Clone)]
pub struct MkrBldCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl MkrBldCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &MkrBldCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for MkrBldCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MkrBldCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct MkrBldBuildErr { pub errors: Vec<String> }

impl fmt::Display for MkrBldBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MkrBldBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for MkrBldBuildErr {}


/// Configuration manager for markers functionality.
pub struct MarkersConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl MarkersConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &MarkersConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for markers operations.
pub struct MarkersRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl MarkersRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for markers.
pub struct MarkersValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl MarkersValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &MarkersValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for markers
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaMarkersRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaMarkersRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaMarkersCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaMarkersCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaMarkersCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 121
// ---------------------------------------------------------------------------

/// Generic object pool `Xc121Pool<T>`.
pub struct Xc121Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc121Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc121PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc121Pool<T> {
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
    pub fn stats(&self) -> Xc121PoolStats {
        Xc121PoolStats {
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

impl<T> Default for Xc121Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc121Scheduler`.
pub struct Xc121Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc121Scheduler {
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

impl Default for Xc121Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_121 hash for the given byte slice.
pub fn xc_121_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_121 convention.
pub fn xc_121_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_111 deepening: state machine + event bus ---

/// States for the Xd111 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd111State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd111State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd111Transition {
    pub from: Xd111State,
    pub to: Xd111State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd111StateMachine {
    current: Xd111State,
    history: Vec<Xd111Transition>,
    step_counter: usize,
}

impl Xd111StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd111State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd111State {
        self.current
    }

    pub fn history(&self) -> &[Xd111Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd111State) -> Result<Xd111State, String> {
        let allowed = match (self.current, target) {
            (Xd111State::Idle, Xd111State::Running) => true,
            (Xd111State::Running, Xd111State::Paused) => true,
            (Xd111State::Running, Xd111State::Done) => true,
            (Xd111State::Paused, Xd111State::Running) => true,
            (Xd111State::Paused, Xd111State::Done) => true,
            (Xd111State::Done, Xd111State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_111: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd111Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd111SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd111State> {
        let prefix = "Xd111SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd111State::Idle),
            "Running" => Some(Xd111State::Running),
            "Paused" => Some(Xd111State::Paused),
            "Done" => Some(Xd111State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd111State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd111 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd111Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd111Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd111HandlerFn = Box<dyn Fn(&Xd111Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd111EventBus {
    handlers: Vec<(usize, Option<String>, Xd111HandlerFn)>,
    next_id: usize,
    published: Vec<Xd111Event>,
}

impl Xd111EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd111Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd111Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd111Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd111Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xg_36: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg36Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg36Graph {
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

impl Default for Xg36Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_36: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg36Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg36Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg36Heap<T>) {
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

impl<T: Ord> Default for Xg36Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 120).
pub struct Xh120SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh120SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 162 as u64,
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

/// A compact bit set supporting boolean operations (variant 120).
pub struct Xh120BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh120BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 120).
pub struct Xi120Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi120Deque<T> {
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
pub struct Xi120Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi120Interval {
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

/// A simple interval tree (variant 120).
pub struct Xi120IntervalTree {
    xi_intervals: Vec<Xi120Interval>,
}

impl Xi120IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi120Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi120Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi120Interval) -> Vec<&Xi120Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi120Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi120Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi120Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi120Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi120Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi120Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 119) ---

/// Disjoint set / union-find for crate 119.
pub struct Xj119UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj119UnionFind {
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

const XJ119_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 119.
pub struct Xj119BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj119BTreeNode<K, V>>>,
    len: usize,
}

struct Xj119BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj119BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj119BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ119_BTREE_ORDER - 1
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
        let mid = XJ119_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj119BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj119BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj119BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj119BTreeNode::xj_new_leaf();
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


// --- xk_119 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk119SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk119SegmentTree {
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
pub struct Xk119DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk119DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_119).
#[derive(Debug, Clone)]
pub struct Xl119Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl119Rope {
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

/// Suffix array for efficient string searching (xl_119).
#[derive(Debug, Clone)]
pub struct Xl119SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl119SuffixArray {
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
pub struct Xm119MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm119MatrixSparse {
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
pub struct Xm119Tokenizer {
    text: String,
}

impl Xm119Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 120.
pub struct Xn120Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn120Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 120 -----

#[derive(Debug, Clone)]
struct Xn120AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn120AvlNode<K, V>>>,
    right: Option<Box<Xn120AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 120.
#[derive(Debug, Clone)]
pub struct Xn120AVL<K, V> {
    root: Option<Box<Xn120AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn120AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn120AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn120AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn120AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn120AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn120AvlNode<K, V>>) -> Box<Xn120AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn120AvlNode<K, V>>) -> Box<Xn120AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn120AvlNode<K, V>>) -> Box<Xn120AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn120AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn120AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn120AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn120AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn120AvlNode<K, V>>) -> &Xn120AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn120AvlNode<K, V>>) -> (Box<Xn120AvlNode<K, V>>, Option<Box<Xn120AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn120AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn120AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn120AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn120AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn120AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn120AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn120AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo120RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo120Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo120RBNode<K, V> {
    key: K,
    value: V,
    color: Xo120Color,
    left: Option<Box<Xo120RBNode<K, V>>>,
    right: Option<Box<Xo120RBNode<K, V>>>,
}

/// A red-black tree map for crate 120.
#[derive(Debug, Clone)]
pub struct Xo120RedBlack<K, V> {
    root: Option<Box<Xo120RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo120RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo120Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo120RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo120RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo120RBNode {
                    key, value, color: Xo120Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo120RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo120Color::Red)
    }

    fn xo_balance(mut h: Box<Xo120RBNode<K, V>>) -> Box<Xo120RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo120Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo120RBNode<K, V>>) -> Box<Xo120RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo120Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo120RBNode<K, V>>) -> Box<Xo120RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo120Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo120RBNode<K, V>>) {
        h.color = Xo120Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo120Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo120Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo120Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo120RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo120RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo120RBNode<K, V>) -> (K, V, Option<Box<Xo120RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo120RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo120Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo120RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo120ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 120.
#[derive(Debug, Clone)]
pub struct Xo120ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo120ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo120#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo120#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 119).
#[derive(Debug)]
pub struct Xp119SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp119Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp119Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp119Node<K, V>>>,
    xp_right: Option<Box<Xp119Node<K, V>>>,
}

impl<K: Ord, V> Xp119Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp119SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp119SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp119Node<K, V>>>, key: &K) -> Option<Box<Xp119Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp119Node<K, V>>) -> Box<Xp119Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp119Node<K, V>>) -> Box<Xp119Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp119Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp119Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp119Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq120Treap ---------------

use std::cmp::Ordering as Xq120Ord;

struct Xq120TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq120TreapNode<K, V>>>,
    right: Option<Box<Xq120TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq120Treap<K, V> {
    root: Option<Box<Xq120TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq120TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_120_size<K, V>(node: &Option<Box<Xq120TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_120_update_size<K, V>(node: &mut Xq120TreapNode<K, V>) {
    node.size = 1 + xq_120_size(&node.left) + xq_120_size(&node.right);
}

fn xq_120_rotate_right<K, V>(mut node: Box<Xq120TreapNode<K, V>>) -> Box<Xq120TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_120_update_size(&mut node);
    left.right = Some(node);
    xq_120_update_size(&mut left);
    left
}

fn xq_120_rotate_left<K, V>(mut node: Box<Xq120TreapNode<K, V>>) -> Box<Xq120TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_120_update_size(&mut node);
    right.left = Some(node);
    xq_120_update_size(&mut right);
    right
}

fn xq_120_insert_node<K: Ord, V>(
    node: Option<Box<Xq120TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq120TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq120TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq120Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq120Ord::Less => {
                let (new_left, old) = xq_120_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_120_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_120_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq120Ord::Greater => {
                let (new_right, old) = xq_120_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_120_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_120_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_120_remove_node<K: Ord, V>(
    node: Option<Box<Xq120TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq120TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq120Ord::Less => {
                let (new_left, old) = xq_120_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_120_update_size(&mut n);
                (Some(n), old)
            }
            Xq120Ord::Greater => {
                let (new_right, old) = xq_120_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_120_update_size(&mut n);
                (Some(n), old)
            }
            Xq120Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_120_rotate_right(n);
                    let (new_right, old) = xq_120_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_120_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_120_rotate_left(n);
                    let (new_left, old) = xq_120_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_120_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_120_find_min<K, V>(node: &Option<Box<Xq120TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_120_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_120_find_max<K, V>(node: &Option<Box<Xq120TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_120_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_120_rank<K: Ord, V>(node: &Option<Box<Xq120TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq120Ord::Less => xq_120_rank(&n.left, key),
            Xq120Ord::Equal => xq_120_size(&n.left),
            Xq120Ord::Greater => 1 + xq_120_size(&n.left) + xq_120_rank(&n.right, key),
        },
    }
}

fn xq_120_kth<K, V>(node: &Option<Box<Xq120TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_120_size(&n.left);
        if k < left_size {
            xq_120_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_120_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_120_in_order<K: Clone, V>(node: &Option<Box<Xq120TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_120_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_120_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq120Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 120 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_120_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq120Ord::Equal => return Some(&n.value),
                Xq120Ord::Less => cur = &n.left,
                Xq120Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_120_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_120_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_120_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_120_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_120_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_120_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_120_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq120VEBTree ---------------

pub struct Xq120VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq120VEBTree>>,
    clusters: Vec<Option<Box<Xq120VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq120VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq120VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq120VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr120KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr120KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr120BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr120KDNode {
    xr_point: Xr120KDPoint,
    xr_left: Option<Box<Xr120KDNode>>,
    xr_right: Option<Box<Xr120KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr120KDTree {
    xr_root: Option<Box<Xr120KDNode>>,
    xr_size: usize,
}

impl Xr120KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr120KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr120KDNode>>,
        point: Xr120KDPoint,
        depth: usize,
    ) -> Box<Xr120KDNode> {
        match node {
            None => Box::new(Xr120KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr120KDPoint) -> Option<Xr120KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr120KDNode>,
        query: &Xr120KDPoint,
        depth: usize,
        best: &mut Xr120KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr120KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr120KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr120KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr120KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr120KDNode>>, pts: &mut Vec<Xr120KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr120KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr120BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr120BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs119PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs119PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs119PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs119PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs119ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs119ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs119ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs119RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs119RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs119RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs119CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs119CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs119CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}

/// Auxiliary statistics tracker for xs_120 data structures.
#[derive(Debug, Clone)]
pub struct Xs120StatsTracker {
    xs_samples: Vec<f64>,
    xs_sorted: bool,
}

impl Xs120StatsTracker {
    /// Create a new stats tracker.
    pub fn xs_new() -> Self {
        Xs120StatsTracker {
            xs_samples: Vec::new(),
            xs_sorted: true,
        }
    }

    /// Add a sample value.
    pub fn xs_add(&mut self, value: f64) {
        self.xs_samples.push(value);
        self.xs_sorted = false;
    }

    /// Return the number of samples.
    pub fn xs_count(&self) -> usize {
        self.xs_samples.len()
    }

    /// Return the mean of all samples.
    pub fn xs_mean(&self) -> f64 {
        if self.xs_samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.xs_samples.iter().sum();
        sum / self.xs_samples.len() as f64
    }

    /// Return the minimum value.
    pub fn xs_min(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::min)
    }

    /// Return the maximum value.
    pub fn xs_max(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::max)
    }

    /// Return the variance of all samples.
    pub fn xs_variance(&self) -> f64 {
        if self.xs_samples.len() < 2 {
            return 0.0;
        }
        let mean = self.xs_mean();
        let sum_sq: f64 = self.xs_samples.iter()
            .map(|x| (x - mean) * (x - mean))
            .sum();
        sum_sq / (self.xs_samples.len() - 1) as f64
    }

    /// Return the standard deviation.
    pub fn xs_std_dev(&self) -> f64 {
        self.xs_variance().sqrt()
    }

    /// Return the median value.
    pub fn xs_median(&mut self) -> Option<f64> {
        if self.xs_samples.is_empty() {
            return None;
        }
        if !self.xs_sorted {
            self.xs_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.xs_sorted = true;
        }
        let mid = self.xs_samples.len() / 2;
        if self.xs_samples.len() % 2 == 0 {
            Some((self.xs_samples[mid - 1] + self.xs_samples[mid]) / 2.0)
        } else {
            Some(self.xs_samples[mid])
        }
    }

    /// Check if the tracker is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_samples.is_empty()
    }

    /// Clear all samples.
    pub fn xs_clear(&mut self) {
        self.xs_samples.clear();
        self.xs_sorted = true;
    }

    /// Return the range (max - min).
    pub fn xs_range(&self) -> f64 {
        match (self.xs_min(), self.xs_max()) {
            (Some(min), Some(max)) => max - min,
            _ => 0.0,
        }
    }

    /// Return the sum of all samples.
    pub fn xs_sum(&self) -> f64 {
        self.xs_samples.iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    fn error_marker(msg: &str, line: u32) -> MarkerData {
        MarkerData {
            severity: MarkerSeverity::Error,
            message: msg.to_string(),
            source: None,
            code: None,
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: 1,
            related_information: vec![],
            tags: vec![],
        }
    }

    fn warning_marker(msg: &str, line: u32) -> MarkerData {
        MarkerData {
            severity: MarkerSeverity::Warning,
            message: msg.to_string(),
            source: None,
            code: None,
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: 1,
            related_information: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn add_and_read_markers() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/foo.rs");

        svc.change_one("rust", &uri, vec![error_marker("E1", 1), error_marker("E2", 5)]);

        let results = svc.read(&MarkerFilter {
            owner: Some("rust".into()),
            uri: Some(uri.clone()),
            severities: None,
            take: None,
        });
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1.message, "E1");
        assert_eq!(results[1].1.message, "E2");
    }

    #[test]
    fn read_with_severity_filter() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/bar.rs");

        svc.change_one(
            "lint",
            &uri,
            vec![
                error_marker("err", 1),
                warning_marker("warn", 2),
            ],
        );

        let results = svc.read(&MarkerFilter {
            owner: None,
            uri: None,
            severities: Some(vec![MarkerSeverity::Warning]),
            take: None,
        });
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.severity, MarkerSeverity::Warning);
    }

    #[test]
    fn read_with_take_limit() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/baz.rs");

        svc.change_one(
            "owner",
            &uri,
            vec![error_marker("a", 1), error_marker("b", 2), error_marker("c", 3)],
        );

        let results = svc.read(&MarkerFilter {
            owner: None,
            uri: None,
            severities: None,
            take: Some(2),
        });
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn remove_by_owner() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/rem.rs");

        svc.change_one("owner_a", &uri, vec![error_marker("a", 1)]);
        svc.change_one("owner_b", &uri, vec![error_marker("b", 1)]);

        svc.remove("owner_a", &[uri.clone()]);

        let all = svc.read(&MarkerFilter {
            owner: None,
            uri: Some(uri),
            severities: None,
            take: None,
        });
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].1.message, "b");
    }

    #[test]
    fn statistics() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/stats.rs");

        svc.change_one(
            "test",
            &uri,
            vec![
                error_marker("e1", 1),
                error_marker("e2", 2),
                warning_marker("w1", 3),
                MarkerData {
                    severity: MarkerSeverity::Info,
                    message: "i1".into(),
                    source: None,
                    code: None,
                    start_line: 4,
                    start_column: 1,
                    end_line: 4,
                    end_column: 1,
                    related_information: vec![],
                    tags: vec![],
                },
            ],
        );

        let stats = svc.get_statistics();
        assert_eq!(stats.errors, 2);
        assert_eq!(stats.warnings, 1);
        assert_eq!(stats.infos, 1);
        assert_eq!(stats.hints, 0);
    }

    #[test]
    fn change_events_fire() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/evt.rs");

        let fired: Arc<StdMutex<Vec<Vec<VsUri>>>> = Arc::new(StdMutex::new(Vec::new()));
        let fired_clone = Arc::clone(&fired);

        let _handle = svc.on_marker_changed().on(move |uris: &Vec<VsUri>| {
            fired_clone.lock().unwrap().push(uris.clone());
        });

        svc.change_one("o", &uri, vec![error_marker("x", 1)]);

        let events = fired.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], vec![uri.clone()]);
    }

    #[test]
    fn change_all_fires_single_event() {
        let svc = MarkerService::new();
        let u1 = VsUri::file("/a.rs");
        let u2 = VsUri::file("/b.rs");

        let fired: Arc<StdMutex<Vec<Vec<VsUri>>>> = Arc::new(StdMutex::new(Vec::new()));
        let fired_clone = Arc::clone(&fired);

        let _handle = svc.on_marker_changed().on(move |uris: &Vec<VsUri>| {
            fired_clone.lock().unwrap().push(uris.clone());
        });

        svc.change_all(
            "o",
            vec![
                (u1.clone(), vec![error_marker("a", 1)]),
                (u2.clone(), vec![warning_marker("b", 2)]),
            ],
        );

        let events = fired.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].len(), 2);
    }

    #[test]
    fn empty_markers_removes_entry() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/empty.rs");

        svc.change_one("o", &uri, vec![error_marker("x", 1)]);
        assert_eq!(svc.get_statistics().errors, 1);

        svc.change_one("o", &uri, vec![]);
        assert_eq!(svc.get_statistics().errors, 0);
    }

    // -- DiagnosticSeverity tests --

    #[test]
    fn diagnostic_severity_to_marker_severity() {
        assert_eq!(DiagnosticSeverity::Error.to_marker_severity(), MarkerSeverity::Error);
        assert_eq!(DiagnosticSeverity::Warning.to_marker_severity(), MarkerSeverity::Warning);
        assert_eq!(DiagnosticSeverity::Info.to_marker_severity(), MarkerSeverity::Info);
        assert_eq!(DiagnosticSeverity::Hint.to_marker_severity(), MarkerSeverity::Hint);
    }

    #[test]
    fn diagnostic_severity_ordering() {
        assert!(DiagnosticSeverity::Error < DiagnosticSeverity::Warning);
        assert!(DiagnosticSeverity::Warning < DiagnosticSeverity::Info);
        assert!(DiagnosticSeverity::Info < DiagnosticSeverity::Hint);
    }

    // -- DiagnosticEntry tests --

    #[test]
    fn diagnostic_entry_new() {
        let uri = VsUri::file("/src/main.rs");
        let entry = DiagnosticEntry::new(&uri, 10, 5, DiagnosticSeverity::Error, "type mismatch");
        assert_eq!(entry.uri, uri);
        assert_eq!(entry.start_line, 10);
        assert_eq!(entry.start_col, 5);
        assert_eq!(entry.message, "type mismatch");
        assert_eq!(entry.severity, DiagnosticSeverity::Error);
        assert!(entry.source.is_none());
        assert!(entry.code.is_none());
    }

    #[test]
    fn diagnostic_entry_builders() {
        let uri = VsUri::file("/foo.rs");
        let entry = DiagnosticEntry::new(&uri, 1, 1, DiagnosticSeverity::Warning, "unused")
            .with_source("clippy")
            .with_code("W0001")
            .with_range(1, 1, 3, 10);
        assert_eq!(entry.source.as_deref(), Some("clippy"));
        assert_eq!(entry.code.as_deref(), Some("W0001"));
        assert_eq!(entry.end_line, 3);
        assert_eq!(entry.end_col, 10);
    }

    #[test]
    fn diagnostic_entry_to_marker_data() {
        let uri = VsUri::file("/foo.rs");
        let entry = DiagnosticEntry::new(&uri, 5, 3, DiagnosticSeverity::Error, "oops")
            .with_source("rustc")
            .with_code("E0001");
        let marker = entry.to_marker_data();
        assert_eq!(marker.severity, MarkerSeverity::Error);
        assert_eq!(marker.message, "oops");
        assert_eq!(marker.source, Some("rustc".into()));
        assert_eq!(marker.code, Some(MarkerCode::String("E0001".into())));
        assert_eq!(marker.start_line, 5);
        assert_eq!(marker.start_column, 3);
    }

    // -- DiagnosticCollection tests --

    #[test]
    fn collection_set_and_get() {
        let mut coll = DiagnosticCollection::new("rustc");
        assert_eq!(coll.name(), "rustc");
        let uri = VsUri::file("/a.rs");
        let d1 = DiagnosticEntry::new(&uri, 1, 0, DiagnosticSeverity::Error, "e1");
        let d2 = DiagnosticEntry::new(&uri, 2, 0, DiagnosticSeverity::Warning, "w1");
        coll.set(&uri, vec![d1, d2]);
        let got = coll.get(&uri).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].message, "e1");
    }

    #[test]
    fn collection_set_empty_removes() {
        let mut coll = DiagnosticCollection::new("test");
        let uri = VsUri::file("/b.rs");
        coll.set(&uri, vec![DiagnosticEntry::new(&uri, 1, 0, DiagnosticSeverity::Info, "i")]);
        assert_eq!(coll.total_count(), 1);
        coll.set(&uri, vec![]);
        assert!(coll.get(&uri).is_none());
        assert_eq!(coll.total_count(), 0);
    }

    #[test]
    fn collection_delete() {
        let mut coll = DiagnosticCollection::new("test");
        let uri = VsUri::file("/c.rs");
        coll.set(&uri, vec![DiagnosticEntry::new(&uri, 1, 0, DiagnosticSeverity::Error, "e")]);
        coll.delete(&uri);
        assert!(coll.get(&uri).is_none());
    }

    #[test]
    fn collection_clear() {
        let mut coll = DiagnosticCollection::new("test");
        let u1 = VsUri::file("/d.rs");
        let u2 = VsUri::file("/e.rs");
        coll.set(&u1, vec![DiagnosticEntry::new(&u1, 1, 0, DiagnosticSeverity::Error, "e")]);
        coll.set(&u2, vec![DiagnosticEntry::new(&u2, 1, 0, DiagnosticSeverity::Warning, "w")]);
        assert_eq!(coll.uri_count(), 2);
        coll.clear();
        assert_eq!(coll.uri_count(), 0);
        assert_eq!(coll.total_count(), 0);
    }

    #[test]
    fn collection_entries_iteration() {
        let mut coll = DiagnosticCollection::new("test");
        let u1 = VsUri::file("/f.rs");
        let u2 = VsUri::file("/g.rs");
        coll.set(&u1, vec![DiagnosticEntry::new(&u1, 1, 0, DiagnosticSeverity::Error, "e")]);
        coll.set(&u2, vec![
            DiagnosticEntry::new(&u2, 1, 0, DiagnosticSeverity::Warning, "w1"),
            DiagnosticEntry::new(&u2, 2, 0, DiagnosticSeverity::Warning, "w2"),
        ]);
        let all: Vec<_> = coll.entries().collect();
        assert_eq!(all.len(), 2);
        assert_eq!(coll.total_count(), 3);
    }

    #[test]
    fn collection_sync_to_marker_service() {
        let svc = MarkerService::new();
        let mut coll = DiagnosticCollection::new("rustc");
        let u1 = VsUri::file("/sync.rs");
        coll.set(&u1, vec![
            DiagnosticEntry::new(&u1, 1, 0, DiagnosticSeverity::Error, "e1"),
            DiagnosticEntry::new(&u1, 5, 0, DiagnosticSeverity::Warning, "w1"),
        ]);
        coll.sync_to_marker_service(&svc);

        let results = svc.read(&MarkerFilter {
            owner: Some("rustc".into()),
            uri: Some(u1.clone()),
            severities: None,
            take: None,
        });
        assert_eq!(results.len(), 2);
        assert_eq!(svc.get_statistics().errors, 1);
        assert_eq!(svc.get_statistics().warnings, 1);
    }

    #[test]
    fn collection_sync_clears_old_markers() {
        let svc = MarkerService::new();
        let mut coll = DiagnosticCollection::new("lint");
        let uri = VsUri::file("/clear.rs");

        coll.set(&uri, vec![DiagnosticEntry::new(&uri, 1, 0, DiagnosticSeverity::Error, "old")]);
        coll.sync_to_marker_service(&svc);
        assert_eq!(svc.get_statistics().errors, 1);

        // Update collection to have only a warning, sync again
        coll.set(&uri, vec![DiagnosticEntry::new(&uri, 2, 0, DiagnosticSeverity::Warning, "new")]);
        coll.sync_to_marker_service(&svc);
        assert_eq!(svc.get_statistics().errors, 0);
        assert_eq!(svc.get_statistics().warnings, 1);
    }

    #[test]
    fn multiple_collections_same_service() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/multi.rs");

        let mut coll_rustc = DiagnosticCollection::new("rustc");
        coll_rustc.set(&uri, vec![DiagnosticEntry::new(&uri, 1, 0, DiagnosticSeverity::Error, "e")]);
        coll_rustc.sync_to_marker_service(&svc);

        let mut coll_clippy = DiagnosticCollection::new("clippy");
        coll_clippy.set(&uri, vec![DiagnosticEntry::new(&uri, 2, 0, DiagnosticSeverity::Warning, "w")]);
        coll_clippy.sync_to_marker_service(&svc);

        // Both owners contribute to statistics
        let stats = svc.get_statistics();
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.warnings, 1);

        // Read all for URI
        let all = svc.read(&MarkerFilter {
            owner: None,
            uri: Some(uri),
            severities: None,
            take: None,
        });
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn marker_code_variants() {
        let str_code = MarkerCode::String("E0001".into());
        let num_code = MarkerCode::Number(42);
        assert_ne!(str_code, num_code);
        assert_eq!(str_code, MarkerCode::String("E0001".into()));
    }

    #[test]
    fn marker_data_with_related_info_and_tags() {
        let related = RelatedInformation {
            uri: VsUri::file("/related.rs"),
            message: "defined here".into(),
            start_line: 10,
            start_column: 5,
            end_line: 10,
            end_column: 15,
        };
        let marker = MarkerData {
            severity: MarkerSeverity::Error,
            message: "type mismatch".into(),
            source: Some("rustc".into()),
            code: Some(MarkerCode::String("E0308".into())),
            start_line: 20,
            start_column: 1,
            end_line: 20,
            end_column: 10,
            related_information: vec![related],
            tags: vec![MarkerTag::Deprecated],
        };
        assert_eq!(marker.related_information.len(), 1);
        assert_eq!(marker.related_information[0].message, "defined here");
        assert_eq!(marker.tags, vec![MarkerTag::Deprecated]);
    }

    #[test]
    fn marker_filter_owner_only() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/filter_owner.rs");
        svc.change_one("rustc", &uri, vec![error_marker("e1", 1)]);
        svc.change_one("clippy", &uri, vec![warning_marker("w1", 2)]);

        let rustc_only = svc.read(&MarkerFilter {
            owner: Some("rustc".into()),
            uri: None,
            severities: None,
            take: None,
        });
        assert_eq!(rustc_only.len(), 1);
        assert_eq!(rustc_only[0].1.message, "e1");
    }

    #[test]
    fn marker_service_default_is_empty() {
        let svc = MarkerService::default();
        let stats = svc.get_statistics();
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.warnings, 0);
        assert_eq!(stats.infos, 0);
        assert_eq!(stats.hints, 0);
    }

    // -- MarkerQueryFilter tests --

    fn make_marker(severity: MarkerSeverity, source: Option<&str>, tags: Vec<MarkerTag>) -> MarkerData {
        MarkerData {
            severity,
            message: "msg".into(),
            source: source.map(|s| s.to_string()),
            code: None,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            related_information: vec![],
            tags,
        }
    }

    #[test]
    fn query_filter_severity() {
        let f = MarkerQueryFilter::new().with_severities(vec![MarkerSeverity::Error]);
        assert!(f.matches(&make_marker(MarkerSeverity::Error, None, vec![])));
        assert!(!f.matches(&make_marker(MarkerSeverity::Warning, None, vec![])));
    }

    #[test]
    fn query_filter_source() {
        let f = MarkerQueryFilter::new().with_source("rustc");
        assert!(f.matches(&make_marker(MarkerSeverity::Error, Some("rustc"), vec![])));
        assert!(!f.matches(&make_marker(MarkerSeverity::Error, Some("clippy"), vec![])));
        assert!(!f.matches(&make_marker(MarkerSeverity::Error, None, vec![])));
    }

    #[test]
    fn query_filter_tags() {
        let f = MarkerQueryFilter::new().with_tags(vec![MarkerTag::Deprecated]);
        assert!(f.matches(&make_marker(MarkerSeverity::Hint, None, vec![MarkerTag::Deprecated])));
        assert!(!f.matches(&make_marker(MarkerSeverity::Hint, None, vec![MarkerTag::Unnecessary])));
        assert!(!f.matches(&make_marker(MarkerSeverity::Hint, None, vec![])));
    }

    #[test]
    fn query_filter_combined() {
        let f = MarkerQueryFilter::new()
            .with_severities(vec![MarkerSeverity::Warning])
            .with_source("clippy");
        assert!(f.matches(&make_marker(MarkerSeverity::Warning, Some("clippy"), vec![])));
        assert!(!f.matches(&make_marker(MarkerSeverity::Error, Some("clippy"), vec![])));
        assert!(!f.matches(&make_marker(MarkerSeverity::Warning, Some("rustc"), vec![])));
    }

    #[test]
    fn query_filter_default_matches_all() {
        let f = MarkerQueryFilter::default();
        assert!(f.matches(&make_marker(MarkerSeverity::Error, None, vec![])));
        assert!(f.matches(&make_marker(MarkerSeverity::Hint, Some("x"), vec![MarkerTag::Unnecessary])));
    }

    // -- MarkerSummary tests --

    #[test]
    fn marker_summary_counts() {
        let markers = vec![
            make_marker(MarkerSeverity::Error, Some("rustc"), vec![]),
            make_marker(MarkerSeverity::Error, Some("rustc"), vec![]),
            make_marker(MarkerSeverity::Warning, Some("clippy"), vec![]),
            make_marker(MarkerSeverity::Info, None, vec![]),
        ];
        let summary = MarkerSummary::from_markers(&markers);
        assert_eq!(summary.total, 4);
        assert_eq!(summary.error_count(), 2);
        assert_eq!(summary.warning_count(), 1);
        assert_eq!(summary.count_by_source.get("rustc"), Some(&2));
        assert_eq!(summary.count_by_source.get("clippy"), Some(&1));
        assert_eq!(summary.count_by_source.get("(unknown)"), Some(&1));
    }

    // -- MarkerCollection tests --

    fn info_marker(msg: &str, line: u32) -> MarkerData {
        MarkerData {
            severity: MarkerSeverity::Info,
            message: msg.to_string(),
            source: None,
            code: None,
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: 1,
            related_information: vec![],
            tags: vec![],
        }
    }

    fn hint_marker(msg: &str, line: u32) -> MarkerData {
        MarkerData {
            severity: MarkerSeverity::Hint,
            message: msg.to_string(),
            source: None,
            code: None,
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: 1,
            related_information: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn collection_new_is_empty() {
        let col = MarkerCollection::new(VsUri::file("/test.rs"));
        assert!(col.is_empty());
        assert_eq!(col.total_count(), 0);
        assert_eq!(col.owner_count(), 0);
        assert_eq!(col.uri(), &VsUri::file("/test.rs"));
    }

    #[test]
    fn collection_set_and_get_markers() {
        let mut col = MarkerCollection::new(VsUri::file("/a.rs"));
        col.set_markers("rustc", vec![error_marker("e1", 1), error_marker("e2", 2)]);
        col.set_markers("clippy", vec![warning_marker("w1", 3)]);

        assert_eq!(col.get_markers("rustc").unwrap().len(), 2);
        assert_eq!(col.get_markers("clippy").unwrap().len(), 1);
        assert!(col.get_markers("unknown").is_none());
    }

    #[test]
    fn collection_all_markers_flattened() {
        let mut col = MarkerCollection::new(VsUri::file("/b.rs"));
        col.set_markers("rustc", vec![error_marker("e1", 1)]);
        col.set_markers("clippy", vec![warning_marker("w1", 2), warning_marker("w2", 3)]);

        let all = col.all_markers();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn collection_total_and_owner_count() {
        let mut col = MarkerCollection::new(VsUri::file("/c.rs"));
        col.set_markers("a", vec![error_marker("e", 1)]);
        col.set_markers("b", vec![warning_marker("w", 2), info_marker("i", 3)]);

        assert_eq!(col.total_count(), 3);
        assert_eq!(col.owner_count(), 2);
    }

    #[test]
    fn collection_remove_owner() {
        let mut col = MarkerCollection::new(VsUri::file("/d.rs"));
        col.set_markers("rustc", vec![error_marker("e1", 1)]);
        col.set_markers("clippy", vec![warning_marker("w1", 2)]);

        assert!(col.remove_owner("rustc"));
        assert!(!col.remove_owner("rustc")); // already removed
        assert!(col.get_markers("rustc").is_none());
        assert_eq!(col.owner_count(), 1);
    }

    #[test]
    fn collection_severity_count() {
        let mut col = MarkerCollection::new(VsUri::file("/e.rs"));
        col.set_markers("a", vec![error_marker("e1", 1), error_marker("e2", 2)]);
        col.set_markers("b", vec![warning_marker("w1", 3), error_marker("e3", 4)]);

        assert_eq!(col.severity_count(MarkerSeverity::Error), 3);
        assert_eq!(col.severity_count(MarkerSeverity::Warning), 1);
        assert_eq!(col.severity_count(MarkerSeverity::Info), 0);
    }

    #[test]
    fn collection_has_errors() {
        let mut col = MarkerCollection::new(VsUri::file("/f.rs"));
        col.set_markers("a", vec![warning_marker("w", 1)]);
        assert!(!col.has_errors());

        col.set_markers("b", vec![error_marker("e", 2)]);
        assert!(col.has_errors());
    }

    #[test]
    fn marker_collection_clear() {
        let mut col = MarkerCollection::new(VsUri::file("/g.rs"));
        col.set_markers("a", vec![error_marker("e", 1)]);
        col.set_markers("b", vec![warning_marker("w", 2)]);
        col.clear();

        assert!(col.is_empty());
        assert_eq!(col.owner_count(), 0);
        assert_eq!(col.total_count(), 0);
    }

    #[test]
    fn collection_set_empty_removes_owner() {
        let mut col = MarkerCollection::new(VsUri::file("/h.rs"));
        col.set_markers("rustc", vec![error_marker("e", 1)]);
        assert_eq!(col.owner_count(), 1);

        col.set_markers("rustc", vec![]);
        assert_eq!(col.owner_count(), 0);
        assert!(col.get_markers("rustc").is_none());
    }

    // -- Terminal rendering helper tests --

    #[test]
    fn severity_icon_error() {
        assert_eq!(marker_severity_icon(MarkerSeverity::Error), "✖");
    }

    #[test]
    fn severity_icon_warning() {
        assert_eq!(marker_severity_icon(MarkerSeverity::Warning), "⚠");
    }

    #[test]
    fn severity_icon_info() {
        assert_eq!(marker_severity_icon(MarkerSeverity::Info), "ℹ");
    }

    #[test]
    fn severity_icon_hint() {
        assert_eq!(marker_severity_icon(MarkerSeverity::Hint), "💡");
    }

    #[test]
    fn severity_label_all() {
        assert_eq!(marker_severity_label(MarkerSeverity::Error), "error");
        assert_eq!(marker_severity_label(MarkerSeverity::Warning), "warning");
        assert_eq!(marker_severity_label(MarkerSeverity::Info), "info");
        assert_eq!(marker_severity_label(MarkerSeverity::Hint), "hint");
    }

    #[test]
    fn format_marker_with_source() {
        let m = MarkerData {
            severity: MarkerSeverity::Error,
            message: "unused variable".to_string(),
            source: Some("rustc".to_string()),
            code: None,
            start_line: 10,
            start_column: 5,
            end_line: 10,
            end_column: 15,
            related_information: vec![],
            tags: vec![],
        };
        let uri = VsUri::file("/src/main.rs");
        let formatted = format_marker_for_terminal(&m, &uri);
        assert!(formatted.starts_with("✖"));
        assert!(formatted.contains("10:5"));
        assert!(formatted.contains("unused variable"));
        assert!(formatted.contains("[rustc]"));
    }

    #[test]
    fn format_marker_without_source() {
        let m = MarkerData {
            severity: MarkerSeverity::Warning,
            message: "something fishy".to_string(),
            source: None,
            code: None,
            start_line: 3,
            start_column: 1,
            end_line: 3,
            end_column: 1,
            related_information: vec![],
            tags: vec![],
        };
        let uri = VsUri::file("/lib.rs");
        let formatted = format_marker_for_terminal(&m, &uri);
        assert!(formatted.starts_with("⚠"));
        assert!(formatted.contains("3:1"));
        assert!(formatted.contains("something fishy"));
        assert!(!formatted.contains("["));
    }

    #[test]
    fn format_marker_info_severity() {
        let m = info_marker("just info", 42);
        let uri = VsUri::file("/info.rs");
        let formatted = format_marker_for_terminal(&m, &uri);
        assert!(formatted.starts_with("ℹ"));
        assert!(formatted.contains("42:1"));
    }

    // -- WorkspaceMarkerStats tests --

    #[test]
    fn workspace_stats_empty_service() {
        let svc = MarkerService::new();
        let stats = markers_stats(&svc);
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_markers, 0);
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.warnings, 0);
        assert_eq!(stats.infos, 0);
        assert_eq!(stats.hints, 0);
        assert_eq!(stats.files_with_errors, 0);
    }

    #[test]
    fn workspace_stats_single_file() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/ws1.rs");
        svc.change_one("rustc", &uri, vec![
            error_marker("e1", 1),
            error_marker("e2", 2),
            warning_marker("w1", 3),
        ]);

        let stats = markers_stats(&svc);
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.total_markers, 3);
        assert_eq!(stats.errors, 2);
        assert_eq!(stats.warnings, 1);
        assert_eq!(stats.files_with_errors, 1);
    }

    #[test]
    fn workspace_stats_multiple_files() {
        let svc = MarkerService::new();
        svc.change_one("rustc", &VsUri::file("/a.rs"), vec![error_marker("e", 1)]);
        svc.change_one("clippy", &VsUri::file("/b.rs"), vec![warning_marker("w", 1)]);
        svc.change_one("rustc", &VsUri::file("/c.rs"), vec![
            info_marker("i", 1),
            hint_marker("h", 2),
        ]);

        let stats = markers_stats(&svc);
        assert_eq!(stats.total_files, 3);
        assert_eq!(stats.total_markers, 4);
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.warnings, 1);
        assert_eq!(stats.infos, 1);
        assert_eq!(stats.hints, 1);
        assert_eq!(stats.files_with_errors, 1);
    }

    #[test]
    fn workspace_stats_multiple_owners_same_file() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/shared.rs");
        svc.change_one("rustc", &uri, vec![error_marker("e", 1)]);
        svc.change_one("clippy", &uri, vec![warning_marker("w", 2)]);

        let stats = markers_stats(&svc);
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.total_markers, 2);
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.warnings, 1);
        assert_eq!(stats.files_with_errors, 1);
    }

    #[test]
    fn workspace_stats_files_with_errors_count() {
        let svc = MarkerService::new();
        svc.change_one("r", &VsUri::file("/x.rs"), vec![error_marker("e", 1)]);
        svc.change_one("r", &VsUri::file("/y.rs"), vec![warning_marker("w", 1)]);
        svc.change_one("r", &VsUri::file("/z.rs"), vec![error_marker("e", 1)]);

        let stats = markers_stats(&svc);
        assert_eq!(stats.files_with_errors, 2);
        assert_eq!(stats.total_files, 3);
    }

    #[test]
    fn workspace_stats_display_format() {
        let stats = WorkspaceMarkerStats {
            total_files: 5,
            total_markers: 12,
            errors: 3,
            warnings: 7,
            infos: 2,
            hints: 0,
            files_with_errors: 2,
        };
        let display = format!("{stats}");
        assert_eq!(display, "3 errors, 7 warnings, 2 info in 5 files");
    }

    #[test]
    fn workspace_stats_display_zeros() {
        let stats = WorkspaceMarkerStats {
            total_files: 0,
            total_markers: 0,
            errors: 0,
            warnings: 0,
            infos: 0,
            hints: 0,
            files_with_errors: 0,
        };
        assert_eq!(format!("{stats}"), "0 errors, 0 warnings, 0 info in 0 files");
    }

    #[test]
    fn workspace_stats_hints_counted() {
        let svc = MarkerService::new();
        svc.change_one("r", &VsUri::file("/h.rs"), vec![
            hint_marker("h1", 1),
            hint_marker("h2", 2),
            hint_marker("h3", 3),
        ]);

        let stats = markers_stats(&svc);
        assert_eq!(stats.hints, 3);
        assert_eq!(stats.total_markers, 3);
        assert_eq!(stats.files_with_errors, 0);
    }

    // -- QuickFixRegistry tests --

    #[test]
    fn quickfix_register_and_lookup() {
        let mut reg = QuickFixRegistry::new();
        let uri = VsUri::file("/fix.rs");
        let marker = error_marker("unused import", 5);

        let fix = QuickFix::new("Remove import").with_replacement("").preferred();
        reg.register(&uri, &marker, vec![fix]);

        let fixes = reg.get_fixes(&uri, &marker).unwrap();
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].title, "Remove import");
        assert!(fixes[0].is_preferred);
        assert_eq!(fixes[0].replacement_text.as_deref(), Some(""));

        let pref = reg.preferred_fix(&uri, &marker).unwrap();
        assert_eq!(pref.title, "Remove import");

        // No fixes for a different marker
        let other = warning_marker("something else", 10);
        assert!(reg.get_fixes(&uri, &other).is_none());
        assert!(reg.preferred_fix(&uri, &other).is_none());

        assert_eq!(reg.total_associations(), 1);
        assert_eq!(reg.total_fixes(), 1);
    }

    #[test]
    fn quickfix_clear_uri() {
        let mut reg = QuickFixRegistry::new();
        let u1 = VsUri::file("/a.rs");
        let u2 = VsUri::file("/b.rs");
        let m1 = error_marker("e1", 1);
        let m2 = error_marker("e2", 1);

        reg.register(&u1, &m1, vec![QuickFix::new("fix a")]);
        reg.register(&u2, &m2, vec![QuickFix::new("fix b")]);
        assert_eq!(reg.total_associations(), 2);

        reg.clear_uri(&u1);
        assert!(reg.get_fixes(&u1, &m1).is_none());
        assert!(reg.get_fixes(&u2, &m2).is_some());
        assert_eq!(reg.total_associations(), 1);
    }

    // -- StalenessTracker tests --

    #[test]
    fn staleness_fresh_then_stale() {
        let mut tracker = StalenessTracker::new();
        let uri = VsUri::file("/src/main.rs");

        // No file version yet — not stale
        assert!(!tracker.is_stale(&uri));

        // File changes → stale (no diagnostics recorded yet)
        tracker.notify_file_changed(&uri);
        assert!(tracker.is_stale(&uri));

        // Diagnostics are published → fresh
        tracker.mark_diagnostics_fresh(&uri);
        assert!(!tracker.is_stale(&uri));

        // File changes again → stale
        tracker.notify_file_changed(&uri);
        assert!(tracker.is_stale(&uri));

        // Explicit version set
        tracker.set_file_version(&uri, 10);
        assert!(tracker.is_stale(&uri));

        tracker.mark_diagnostics_fresh(&uri);
        assert!(!tracker.is_stale(&uri));
    }

    #[test]
    fn staleness_stale_uris() {
        let mut tracker = StalenessTracker::new();
        let u1 = VsUri::file("/a.rs");
        let u2 = VsUri::file("/b.rs");
        let u3 = VsUri::file("/c.rs");

        tracker.notify_file_changed(&u1);
        tracker.notify_file_changed(&u2);
        tracker.notify_file_changed(&u3);

        tracker.mark_diagnostics_fresh(&u2);

        let mut stale = tracker.stale_uris();
        stale.sort();
        assert_eq!(stale.len(), 2);
        assert!(stale.contains(&u1));
        assert!(stale.contains(&u3));

        tracker.remove(&u1);
        let stale2 = tracker.stale_uris();
        assert_eq!(stale2.len(), 1);
        assert!(stale2.contains(&u3));
    }

    // -- MarkerDiff tests --

    #[test]
    fn marker_diff_added_removed_unchanged() {
        let old = vec![
            error_marker("e1", 1),
            warning_marker("w1", 2),
            error_marker("e2", 3),
        ];
        let new = vec![
            error_marker("e1", 1),   // unchanged
            error_marker("e3", 4),   // added
            info_marker("i1", 5),    // added
        ];

        let diff = MarkerDiff::compute(&old, &new);

        assert_eq!(diff.unchanged.len(), 1);
        assert_eq!(diff.unchanged[0].message, "e1");

        assert_eq!(diff.removed.len(), 2);
        assert!(diff.removed.iter().any(|m| m.message == "w1"));
        assert!(diff.removed.iter().any(|m| m.message == "e2"));

        assert_eq!(diff.added.len(), 2);
        assert!(diff.added.iter().any(|m| m.message == "e3"));
        assert!(diff.added.iter().any(|m| m.message == "i1"));

        assert!(!diff.is_empty());
        assert_eq!(diff.change_count(), 4);
        assert_eq!(diff.summary(), "+2 -2 =1 markers");
    }

    #[test]
    fn marker_diff_no_changes() {
        let markers = vec![error_marker("e1", 1), warning_marker("w1", 2)];
        let diff = MarkerDiff::compute(&markers, &markers);

        assert!(diff.is_empty());
        assert_eq!(diff.change_count(), 0);
        assert_eq!(diff.unchanged.len(), 2);
    }

    // -- group_markers_by_file tests --

    #[test]
    fn group_markers_sorts_by_severity_then_line() {
        let u1 = VsUri::file("/alpha.rs");
        let u2 = VsUri::file("/beta.rs");

        let pairs = vec![
            (u1.clone(), warning_marker("w1", 10)),
            (u1.clone(), error_marker("e1", 20)),
            (u1.clone(), hint_marker("h1", 5)),
            (u2.clone(), info_marker("i1", 1)),
        ];

        let groups = group_markers_by_file(&pairs);
        assert_eq!(groups.len(), 2);

        // Files sorted by URI
        assert_eq!(groups[0].uri, u1);
        assert_eq!(groups[1].uri, u2);

        // First file: errors first, then warnings, then hints
        let g = &groups[0];
        assert_eq!(g.markers.len(), 3);
        assert_eq!(g.markers[0].severity, MarkerSeverity::Error);
        assert_eq!(g.markers[1].severity, MarkerSeverity::Warning);
        assert_eq!(g.markers[2].severity, MarkerSeverity::Hint);

        assert_eq!(g.error_count, 1);
        assert_eq!(g.warning_count, 1);
        assert_eq!(g.hint_count, 1);
        assert_eq!(g.info_count, 0);
    }

    // -- Per-URI stats tests --

    #[test]
    fn per_uri_stats_multiple_files() {
        let svc = MarkerService::new();
        let u1 = VsUri::file("/a.rs");
        let u2 = VsUri::file("/b.rs");

        svc.change_one("rustc", &u1, vec![error_marker("e1", 1), warning_marker("w1", 2)]);
        svc.change_one("clippy", &u1, vec![warning_marker("w2", 3)]);
        svc.change_one("rustc", &u2, vec![info_marker("i1", 1)]);

        let stats = svc.per_uri_stats();
        assert_eq!(stats.len(), 2);

        let s1 = stats.iter().find(|s| s.uri == u1).unwrap();
        assert_eq!(s1.errors, 1);
        assert_eq!(s1.warnings, 2);
        assert_eq!(s1.total(), 3);
        assert!(s1.has_errors());
        assert_eq!(s1.worst_severity(), Some(MarkerSeverity::Error));

        let s2 = stats.iter().find(|s| s.uri == u2).unwrap();
        assert_eq!(s2.infos, 1);
        assert_eq!(s2.total(), 1);
        assert!(!s2.has_errors());
        assert_eq!(s2.worst_severity(), Some(MarkerSeverity::Info));
    }


    #[test]
    fn quick_nav_empty() {
        let mut nav = MarkerQuickNavigation::new();
        assert!(nav.next().is_none());
        assert!(nav.prev().is_none());
    }

    #[test]
    fn quick_nav_load() {
        let service = MarkerService::new();
        let uri = VsUri::parse("file:///test.rs");
        service.change_one("test", &uri, vec![MarkerData {
            severity: MarkerSeverity::Error,
            message: "err".into(),
            start_line: 10,
            start_column: 1,
            end_line: 10,
            end_column: 5,
            source: None,
            code: None,
            tags: vec![],
            related_information: vec![],
        }]);
        let mut nav = MarkerQuickNavigation::new();
        nav.load_markers(&service);
        assert_eq!(nav.count(), 1);
        let (_, line) = nav.next().unwrap();
        assert_eq!(line, 10);
    }

    #[test]
    fn code_action_link_display() {
        let link = MarkerCodeActionLink::new("rust", "fix import", VsUri::parse("file:///a.rs"), 5);
        assert!(format!("{link}").contains("fix import"));
    }

    #[test]
    fn code_action_linker_basic() {
        let mut linker = MarkerCodeActionLinker::new();
        let uri = VsUri::parse("file:///a.rs");
        linker.add(MarkerCodeActionLink::new("rust", "fix", uri.clone(), 1));
        assert_eq!(linker.links_for_uri(&uri).len(), 1);
    }

    #[test]
    fn batch_updater_basic() {
        let mut updater = MarkerBatchUpdater::new();
        let uri = VsUri::parse("file:///a.rs");
        updater.queue("test", uri, vec![]);
        assert_eq!(updater.pending_count(), 1);
        let service = MarkerService::new();
        updater.apply(&service);
        assert!(updater.is_empty());
    }

    #[test]
    fn batch_updater_clear() {
        let mut updater = MarkerBatchUpdater::new();
        updater.queue("test", VsUri::parse("file:///a.rs"), vec![]);
        updater.clear();
        assert!(updater.is_empty());
    }

    #[test]
    fn owner_tracker_basic() {
        let mut tracker = MarkerOwnerTracker::new();
        tracker.record("rust-analyzer");
        tracker.record("rust-analyzer");
        tracker.record("eslint");
        assert_eq!(tracker.count_for("rust-analyzer"), 2);
        assert_eq!(tracker.total_markers(), 3);
    }

    #[test]
    fn owner_tracker_unique() {
        let mut tracker = MarkerOwnerTracker::new();
        tracker.record("a");
        tracker.record("b");
        assert_eq!(tracker.unique_owners().len(), 2);
    }

    #[test]
    fn owner_tracker_clear() {
        let mut tracker = MarkerOwnerTracker::new();
        tracker.record("a");
        tracker.clear();
        assert_eq!(tracker.total_markers(), 0);
    }

    #[test]
    fn quick_nav_count() {
        let nav = MarkerQuickNavigation::new();
        assert_eq!(nav.count(), 0);
        assert_eq!(nav.current_index(), None);
    }

    #[test]
    fn code_action_linker_empty() {
        let linker = MarkerCodeActionLinker::new();
        assert!(linker.is_empty());
    }


    fn nav_entry(uri: &str, line: u32, sev: MarkerSeverity, msg: &str) -> NavRingEntry {
        NavRingEntry {
            uri: uri.to_string(),
            line,
            column: 0,
            severity: sev,
            message: msg.to_string(),
        }
    }

    #[test]
    fn nav_ring_basic() {
        let mut ring = MarkerNavigationRing::new(true);
        ring.set_entries(vec![
            nav_entry("a.rs", 1, MarkerSeverity::Error, "e1"),
            nav_entry("b.rs", 2, MarkerSeverity::Warning, "w1"),
        ]);
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.current().unwrap().message, "e1");
    }

    #[test]
    fn nav_ring_next_prev() {
        let mut ring = MarkerNavigationRing::new(false);
        ring.set_entries(vec![
            nav_entry("a.rs", 1, MarkerSeverity::Error, "e1"),
            nav_entry("b.rs", 2, MarkerSeverity::Warning, "w1"),
        ]);
        let n = ring.next().unwrap();
        assert_eq!(n.message, "w1");
        let p = ring.prev().unwrap();
        assert_eq!(p.message, "e1");
    }

    #[test]
    fn nav_ring_wrap_around() {
        let mut ring = MarkerNavigationRing::new(true);
        ring.set_entries(vec![
            nav_entry("a.rs", 1, MarkerSeverity::Error, "e1"),
            nav_entry("b.rs", 2, MarkerSeverity::Warning, "w1"),
        ]);
        ring.next(); // w1
        let wrapped = ring.next().unwrap();
        assert_eq!(wrapped.message, "e1");
    }

    #[test]
    fn nav_ring_no_wrap() {
        let mut ring = MarkerNavigationRing::new(false);
        ring.set_entries(vec![
            nav_entry("a.rs", 1, MarkerSeverity::Error, "e1"),
        ]);
        ring.next(); // beyond end
        assert!(ring.next().is_none());
    }

    #[test]
    fn nav_ring_empty() {
        let mut ring = MarkerNavigationRing::new(true);
        assert!(ring.is_empty());
        assert!(ring.next().is_none());
        assert!(ring.prev().is_none());
    }

    #[test]
    fn nav_ring_jump_to() {
        let mut ring = MarkerNavigationRing::new(false);
        ring.set_entries(vec![
            nav_entry("a.rs", 1, MarkerSeverity::Error, "e1"),
            nav_entry("b.rs", 2, MarkerSeverity::Warning, "w1"),
            nav_entry("c.rs", 3, MarkerSeverity::Hint, "h1"),
        ]);
        let e = ring.jump_to(2).unwrap();
        assert_eq!(e.message, "h1");
        assert!(ring.jump_to(99).is_none());
    }

    #[test]
    fn nav_ring_filter_severity() {
        let mut ring = MarkerNavigationRing::new(true);
        ring.set_entries(vec![
            nav_entry("a.rs", 1, MarkerSeverity::Error, "e1"),
            nav_entry("b.rs", 2, MarkerSeverity::Warning, "w1"),
            nav_entry("c.rs", 3, MarkerSeverity::Error, "e2"),
        ]);
        let errors = ring.filter_severity(MarkerSeverity::Error);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn nav_ring_count_by_severity() {
        let mut ring = MarkerNavigationRing::new(true);
        ring.set_entries(vec![
            nav_entry("a.rs", 1, MarkerSeverity::Warning, "w1"),
            nav_entry("b.rs", 2, MarkerSeverity::Warning, "w2"),
        ]);
        assert_eq!(ring.count_by_severity(MarkerSeverity::Warning), 2);
        assert_eq!(ring.count_by_severity(MarkerSeverity::Error), 0);
    }

    #[test]
    fn filter_tokenize() {
        let tokens = MarkerFilterExpressionParser::tokenize("error AND file:*.rs");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], FilterToken::Text("error".into()));
        assert_eq!(tokens[1], FilterToken::And);
        assert_eq!(tokens[2], FilterToken::KeyValue("file".into(), "*.rs".into()));
    }

    #[test]
    fn filter_parse_and() {
        let node = MarkerFilterExpressionParser::parse("error AND file:*.rs").unwrap();
        assert!(matches!(node, FilterExprNode::And(_, _)));
    }

    #[test]
    fn filter_matches_term() {
        let entry = nav_entry("a.rs", 1, MarkerSeverity::Error, "undefined variable");
        let node = MarkerFilterExpressionParser::parse("undefined").unwrap();
        assert!(MarkerFilterExpressionParser::matches(&node, &entry));
    }

    #[test]
    fn filter_matches_field() {
        let entry = nav_entry("main.rs", 5, MarkerSeverity::Error, "oops");
        let node = MarkerFilterExpressionParser::parse("file:*.rs").unwrap();
        assert!(MarkerFilterExpressionParser::matches(&node, &entry));
    }

    #[test]
    fn filter_matches_not() {
        let entry = nav_entry("a.rs", 1, MarkerSeverity::Warning, "unused");
        let node = MarkerFilterExpressionParser::parse("NOT error").unwrap();
        assert!(MarkerFilterExpressionParser::matches(&node, &entry));
    }

    #[test]
    fn filter_matches_severity() {
        let entry = nav_entry("a.rs", 1, MarkerSeverity::Error, "x");
        let node = MarkerFilterExpressionParser::parse("sev:error").unwrap();
        assert!(MarkerFilterExpressionParser::matches(&node, &entry));
    }


    #[test]
    fn mkrbuf_ringbuf_push_get() {
        let mut rb = MkrBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn mkrbuf_ringbuf_overflow() {
        let mut rb = MkrBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn mkrbuf_ringbuf_clear() {
        let mut rb = MkrBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn mkrbuf_ringbuf_newest_oldest() {
        let mut rb = MkrBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn mkrbuf_ringbuf_to_vec() {
        let mut rb = MkrBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn mkrbuf_ringbuf_is_full() {
        let mut rb = MkrBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn mkrbld_builder_valid() {
        let cfg = MkrBldBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn mkrbld_builder_empty_name() {
        let r = MkrBldBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn mkrbld_builder_bad_priority() {
        assert!(MkrBldBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn mkrbld_builder_zero_max() {
        assert!(MkrBldBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn mkrbld_cfg_merge() {
        let mut a = MkrBldBuilder::new("a").property("x", "1").build().unwrap();
        let b = MkrBldBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn mkrbld_cfg_display() {
        let cfg = MkrBldBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    #[test]
    fn markers_config_new() {
        let cfg = MarkersConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn markers_config_set_get() {
        let mut cfg = MarkersConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn markers_config_remove() {
        let mut cfg = MarkersConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn markers_config_keys_sorted() {
        let mut cfg = MarkersConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn markers_config_bump_version() {
        let mut cfg = MarkersConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn markers_config_clear() {
        let mut cfg = MarkersConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn markers_config_merge() {
        let mut cfg1 = MarkersConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = MarkersConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn markers_config_disable() {
        let mut cfg = MarkersConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn markers_rate_tracker_empty() {
        let rt = MarkersRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn markers_rate_tracker_record() {
        let mut rt = MarkersRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn markers_rate_tracker_prune() {
        let mut rt = MarkersRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn markers_validator_valid() {
        let v = MarkersValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn markers_validator_errors() {
        let mut v = MarkersValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn markers_validator_clear() {
        let mut v = MarkersValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn markers_validator_merge() {
        let mut v1 = MarkersValidator::new();
        v1.add_error("e1");
        let mut v2 = MarkersValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn markers_rate_tracker_clear() {
        let mut rt = MarkersRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for markers
    #[test]
    fn xa_markers_ring_new() {
        let rb = super::XaMarkersRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_markers_ring_push_len() {
        let mut rb = super::XaMarkersRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_markers_ring_wrap() {
        let mut rb = super::XaMarkersRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_markers_ring_mean_empty() {
        let rb = super::XaMarkersRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_markers_ring_mean_values() {
        let mut rb = super::XaMarkersRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_markers_ring_min_max() {
        let mut rb = super::XaMarkersRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_markers_ring_iter() {
        let mut rb = super::XaMarkersRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_markers_counter_new() {
        let c = super::XaMarkersCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_markers_counter_inc() {
        let mut c = super::XaMarkersCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_markers_counter_inc_by() {
        let mut c = super::XaMarkersCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_markers_counter_reset() {
        let mut c = super::XaMarkersCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_markers_counter_clear() {
        let mut c = super::XaMarkersCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_markers_counter_default() {
        let c = super::XaMarkersCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 121 ----

    #[test]
    fn xc_121_pool_new_empty() {
        let pool: super::Xc121Pool<i32> = super::Xc121Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_121_pool_release_acquire() {
        let mut pool = super::Xc121Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_121_pool_acquire_empty() {
        let mut pool: super::Xc121Pool<i32> = super::Xc121Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_121_pool_full() {
        let mut pool = super::Xc121Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_121_pool_drain() {
        let mut pool = super::Xc121Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_121_pool_stats() {
        let mut pool = super::Xc121Pool::new(8);
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
    fn xc_121_pool_clear() {
        let mut pool = super::Xc121Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_121_pool_shrink() {
        let mut pool = super::Xc121Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_121_pool_default() {
        let pool: super::Xc121Pool<String> = super::Xc121Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_121_pool_extend() {
        let mut pool = super::Xc121Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_121_pool_retain() {
        let mut pool = super::Xc121Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_121_scheduler_round_robin() {
        let mut sched = super::Xc121Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_121_scheduler_empty() {
        let mut sched = super::Xc121Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_121_scheduler_reset() {
        let mut sched = super::Xc121Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_121_scheduler_add_remove() {
        let mut sched = super::Xc121Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_121_scheduler_targets() {
        let sched = super::Xc121Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_121_hash_empty() {
        assert_eq!(super::xc_121_hash(b""), 5381);
    }

    #[test]
    fn xc_121_hash_data() {
        let h = super::xc_121_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_121_hash(b"hello"), h);
    }

    #[test]
    fn xc_121_reverse_str() {
        assert_eq!(super::xc_121_reverse("abc"), "cba");
        assert_eq!(super::xc_121_reverse(""), "");
    }


    // --- xd_111 deepening tests ---

    #[test]
    fn xd_111_sm_initial_state() {
        let sm = Xd111StateMachine::new();
        assert_eq!(sm.current_state(), Xd111State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_111_sm_valid_idle_to_running() {
        let mut sm = Xd111StateMachine::new();
        assert!(sm.transition(Xd111State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd111State::Running);
    }

    #[test]
    fn xd_111_sm_valid_running_to_paused() {
        let mut sm = Xd111StateMachine::new();
        sm.transition(Xd111State::Running).unwrap();
        assert!(sm.transition(Xd111State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd111State::Paused);
    }

    #[test]
    fn xd_111_sm_valid_running_to_done() {
        let mut sm = Xd111StateMachine::new();
        sm.transition(Xd111State::Running).unwrap();
        assert!(sm.transition(Xd111State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd111State::Done);
    }

    #[test]
    fn xd_111_sm_valid_paused_to_running() {
        let mut sm = Xd111StateMachine::new();
        sm.transition(Xd111State::Running).unwrap();
        sm.transition(Xd111State::Paused).unwrap();
        assert!(sm.transition(Xd111State::Running).is_ok());
    }

    #[test]
    fn xd_111_sm_valid_done_to_idle() {
        let mut sm = Xd111StateMachine::new();
        sm.transition(Xd111State::Running).unwrap();
        sm.transition(Xd111State::Done).unwrap();
        assert!(sm.transition(Xd111State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd111State::Idle);
    }

    #[test]
    fn xd_111_sm_invalid_idle_to_done() {
        let mut sm = Xd111StateMachine::new();
        assert!(sm.transition(Xd111State::Done).is_err());
    }

    #[test]
    fn xd_111_sm_invalid_idle_to_paused() {
        let mut sm = Xd111StateMachine::new();
        assert!(sm.transition(Xd111State::Paused).is_err());
    }

    #[test]
    fn xd_111_sm_history_tracking() {
        let mut sm = Xd111StateMachine::new();
        sm.transition(Xd111State::Running).unwrap();
        sm.transition(Xd111State::Paused).unwrap();
        sm.transition(Xd111State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd111State::Idle);
        assert_eq!(sm.history()[0].to, Xd111State::Running);
        assert_eq!(sm.history()[1].from, Xd111State::Running);
        assert_eq!(sm.history()[2].to, Xd111State::Done);
    }

    #[test]
    fn xd_111_sm_serialize_deserialize() {
        let mut sm = Xd111StateMachine::new();
        sm.transition(Xd111State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd111StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd111State::Running));
    }

    #[test]
    fn xd_111_sm_deserialize_invalid() {
        assert_eq!(Xd111StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_111_sm_reset() {
        let mut sm = Xd111StateMachine::new();
        sm.transition(Xd111State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd111State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_111_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd111EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd111Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_111_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd111EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd111Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd111Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_111_bus_unsubscribe() {
        let mut bus = Xd111EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_111_event_kind_and_payload() {
        let e = Xd111Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd111Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_111_bus_clear_history() {
        let mut bus = Xd111EventBus::new();
        bus.publish(Xd111Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_111_sm_step_counter_increments() {
        let mut sm = Xd111StateMachine::new();
        sm.transition(Xd111State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd111State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_36 graph tests ------------------------------------------------

    #[test]
    fn xg_36_graph_empty() {
        let g = super::Xg36Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_36_graph_add_node() {
        let mut g = super::Xg36Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_36_graph_add_edge() {
        let mut g = super::Xg36Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_36_graph_neighbors() {
        let mut g = super::Xg36Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_36_graph_has_path() {
        let mut g = super::Xg36Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_36_graph_self_path() {
        let g = super::Xg36Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_36_graph_topo_sort() {
        let mut g = super::Xg36Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_36_graph_cycle_detect_false() {
        let mut g = super::Xg36Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_36_graph_cycle_detect_true() {
        let mut g = super::Xg36Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_36 heap tests -------------------------------------------------

    #[test]
    fn xg_36_heap_empty() {
        let h: super::Xg36Heap<i32> = super::Xg36Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_36_heap_push_pop() {
        let mut h = super::Xg36Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_36_heap_peek() {
        let mut h = super::Xg36Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_36_heap_drain_sorted() {
        let mut h = super::Xg36Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_36_heap_merge() {
        let mut a = super::Xg36Heap::new();
        let mut b = super::Xg36Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_36_heap_default() {
        let h: super::Xg36Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_36_graph_default() {
        let g: super::Xg36Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh120_skip_insert_contains() {
        let mut sl = super::Xh120SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh120_skip_remove() {
        let mut sl = super::Xh120SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh120_skip_len() {
        let mut sl = super::Xh120SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh120_skip_range_query() {
        let mut sl = super::Xh120SkipList::xh_new(4);
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
    fn xh120_skip_floor_ceiling() {
        let mut sl = super::Xh120SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh120_skip_rank() {
        let mut sl = super::Xh120SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh120_skip_empty() {
        let sl = super::Xh120SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh120_skip_duplicates() {
        let mut sl = super::Xh120SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh120_bitset_set_test() {
        let mut bs = super::Xh120BitSet::xh_new(256);
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
    fn xh120_bitset_clear_count() {
        let mut bs = super::Xh120BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh120_bitset_and_or_xor() {
        let mut a = super::Xh120BitSet::xh_new(128);
        let mut b = super::Xh120BitSet::xh_new(128);
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
    fn xh120_bitset_iter_ones() {
        let mut bs = super::Xh120BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh120_bitset_first_last() {
        let mut bs = super::Xh120BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh120_bitset_empty() {
        let bs = super::Xh120BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi120_deque_push_pop_back() {
        let mut dq = super::Xi120Deque::xi_new(4);
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
    fn xi120_deque_push_pop_front() {
        let mut dq = super::Xi120Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi120_deque_mixed_ops() {
        let mut dq = super::Xi120Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi120_deque_get_and_split() {
        let mut dq = super::Xi120Deque::xi_new(8);
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
    fn xi120_deque_rotate_left() {
        let mut dq = super::Xi120Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi120_deque_rotate_right() {
        let mut dq = super::Xi120Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi120_deque_grow() {
        let mut dq = super::Xi120Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi120_deque_empty() {
        let dq = super::Xi120Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi120_interval_tree_insert_query() {
        let mut tree = super::Xi120IntervalTree::xi_new();
        tree.xi_insert(super::Xi120Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi120Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi120Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi120_interval_tree_overlap() {
        let mut tree = super::Xi120IntervalTree::xi_new();
        tree.xi_insert(super::Xi120Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi120Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi120Interval::xi_new(12, 20));
        let q = super::Xi120Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi120_interval_tree_remove() {
        let mut tree = super::Xi120IntervalTree::xi_new();
        tree.xi_insert(super::Xi120Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi120Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi120_interval_tree_gaps() {
        let mut tree = super::Xi120IntervalTree::xi_new();
        tree.xi_insert(super::Xi120Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi120Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi120Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi120Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi120Interval::xi_new(8, 10));
    }

    #[test]
    fn xi120_interval_tree_merge() {
        let mut tree = super::Xi120IntervalTree::xi_new();
        tree.xi_insert(super::Xi120Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi120Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi120Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi120Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi120Interval::xi_new(10, 15));
    }

    #[test]
    fn xi120_interval_tree_all() {
        let mut tree = super::Xi120IntervalTree::xi_new();
        tree.xi_insert(super::Xi120Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi120Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi120_interval_tree_empty() {
        let tree = super::Xi120IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi120_interval_tree_contains_point() {
        let iv = super::Xi120Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 119) ---

    #[test]
    fn xj_119_uf_make_and_find() {
        let mut uf = super::Xj119UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_119_uf_union_connected() {
        let mut uf = super::Xj119UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_119_uf_component_count() {
        let mut uf = super::Xj119UnionFind::xj_new();
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
    fn xj_119_uf_component_size() {
        let mut uf = super::Xj119UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_119_uf_largest_component() {
        let mut uf = super::Xj119UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_119_uf_many_elements() {
        let mut uf = super::Xj119UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_119_uf_separate_components() {
        let mut uf = super::Xj119UnionFind::xj_new();
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
    fn xj_119_uf_path_compression() {
        let mut uf = super::Xj119UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_119_bt_insert_get() {
        let mut bt = super::Xj119BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_119_bt_contains_len() {
        let mut bt = super::Xj119BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_119_bt_replace() {
        let mut bt = super::Xj119BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_119_bt_remove() {
        let mut bt = super::Xj119BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_119_bt_keys_values() {
        let mut bt = super::Xj119BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_119_bt_range() {
        let mut bt = super::Xj119BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_119_bt_min_max() {
        let mut bt = super::Xj119BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_119_bt_many_inserts() {
        let mut bt = super::Xj119BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_119 segment tree tests ---

    #[test]
    fn xk_119_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk119SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_119_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk119SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_119_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk119SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_119_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk119SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_119_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk119SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_119_st_single_element() {
        let data = vec![42];
        let st = super::Xk119SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_119_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk119SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_119_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk119SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_119 disjoint intervals tests ---

    #[test]
    fn xk_119_di_add_and_count() {
        let mut di = super::Xk119DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_119_di_merge_overlap() {
        let mut di = super::Xk119DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_119_di_contains() {
        let mut di = super::Xk119DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_119_di_remove() {
        let mut di = super::Xk119DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_119_di_covered_length() {
        let mut di = super::Xk119DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_119_di_gaps() {
        let mut di = super::Xk119DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_119_di_merge_adjacent() {
        let mut di = super::Xk119DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_119_di_empty() {
        let di = super::Xk119DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_119_rope_new_empty() {
        let rope = super::Xl119Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_119_rope_from_str() {
        let rope = super::Xl119Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_119_rope_insert_at() {
        let mut rope = super::Xl119Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_119_rope_delete_range() {
        let mut rope = super::Xl119Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_119_rope_char_at() {
        let rope = super::Xl119Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_119_rope_split_concat() {
        let rope = super::Xl119Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_119_rope_line_count() {
        let rope = super::Xl119Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_119_rope_line_at() {
        let rope = super::Xl119Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_119_sa_build_and_search() {
        let sa = super::Xl119SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_119_sa_count() {
        let sa = super::Xl119SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_119_sa_longest_repeated() {
        let sa = super::Xl119SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_119_sa_all_positions() {
        let sa = super::Xl119SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_119_sa_len() {
        let sa = super::Xl119SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_119_sa_empty() {
        let sa = super::Xl119SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_119_rope_slice() {
        let rope = super::Xl119Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_119_sa_search_start() {
        let sa = super::Xl119SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_119_sparse_set_get() {
        let mut m = super::Xm119MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_119_sparse_row_col() {
        let mut m = super::Xm119MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_119_sparse_transpose() {
        let mut m = super::Xm119MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_119_sparse_multiply_vec() {
        let mut m = super::Xm119MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_119_sparse_nnz_density() {
        let mut m = super::Xm119MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_119_sparse_clear() {
        let mut m = super::Xm119MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_119_sparse_overwrite_zero() {
        let mut m = super::Xm119MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_119_tokenizer_basic() {
        let t = super::Xm119Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_119_tokenizer_count() {
        let t = super::Xm119Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_119_tokenizer_unique() {
        let t = super::Xm119Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_119_tokenizer_frequency() {
        let t = super::Xm119Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_119_tokenizer_delimiter() {
        let t = super::Xm119Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_119_tokenizer_whitespace() {
        let t = super::Xm119Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_119_tokenizer_empty() {
        let t = super::Xm119Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 120 ----

    #[test]
    fn xn_120_fenwick_prefix_sum() {
        let mut ft = super::Xn120Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_120_fenwick_range_sum() {
        let mut ft = super::Xn120Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_120_fenwick_point_query() {
        let mut ft = super::Xn120Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_120_fenwick_len() {
        let ft = super::Xn120Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_120_fenwick_multiple_updates() {
        let mut ft = super::Xn120Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_120_fenwick_single_element() {
        let mut ft = super::Xn120Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_120_fenwick_find_kth() {
        let mut ft = super::Xn120Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_120_fenwick_negative_delta() {
        let mut ft = super::Xn120Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 120 ----

    #[test]
    fn xn_120_avl_insert_get() {
        let mut m = super::Xn120AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_120_avl_remove() {
        let mut m = super::Xn120AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_120_avl_in_order() {
        let mut m = super::Xn120AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_120_avl_min_max() {
        let mut m = super::Xn120AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_120_avl_floor_ceiling() {
        let mut m = super::Xn120AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_120_avl_height_balanced() {
        let mut m = super::Xn120AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_120_avl_overwrite() {
        let mut m = super::Xn120AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_120_avl_empty() {
        let m: super::Xn120AVL<i32, i32> = super::Xn120AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo120RedBlack tests ---

    #[test]
    fn xo_120_rb_insert_and_get() {
        let mut tree = super::Xo120RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_120_rb_len_and_empty() {
        let mut tree = super::Xo120RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_120_rb_min_max() {
        let mut tree = super::Xo120RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_120_rb_contains() {
        let mut tree = super::Xo120RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_120_rb_remove() {
        let mut tree = super::Xo120RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_120_rb_in_order() {
        let mut tree = super::Xo120RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_120_rb_black_height() {
        let mut tree = super::Xo120RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_120_rb_overwrite() {
        let mut tree = super::Xo120RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo120ConsistentHash tests ---

    #[test]
    fn xo_120_ch_add_and_count() {
        let mut ring = super::Xo120ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_120_ch_remove_node() {
        let mut ring = super::Xo120ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_120_ch_get_node() {
        let mut ring = super::Xo120ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_120_ch_empty_ring() {
        let ring = super::Xo120ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_120_ch_distribution() {
        let mut ring = super::Xo120ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_120_ch_rebalance() {
        let mut ring = super::Xo120ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_120_ch_virtual_nodes() {
        let mut ring = super::Xo120ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_120_ch_consistent_lookup() {
        let mut ring = super::Xo120ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_119_splay_insert_get() {
        let mut t = super::Xp119SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_119_splay_remove() {
        let mut t = super::Xp119SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_119_splay_count_increases() {
        let mut t = super::Xp119SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_119_splay_depth() {
        let mut t = super::Xp119SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_119_splay_len_empty() {
        let t = super::Xp119SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_119_splay_min_max() {
        let mut t = super::Xp119SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_119_splay_overwrite() {
        let mut t = super::Xp119SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_119_splay_remove_missing() {
        let mut t = super::Xp119SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_120 treap tests ----
    #[test]
    fn xq_120_treap_empty() {
        let t = super::Xq120Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_120_treap_insert_get() {
        let mut t = super::Xq120Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_120_treap_overwrite() {
        let mut t = super::Xq120Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_120_treap_remove() {
        let mut t = super::Xq120Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_120_treap_min_max() {
        let mut t = super::Xq120Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_120_treap_rank() {
        let mut t = super::Xq120Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_120_treap_kth() {
        let mut t = super::Xq120Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_120_treap_in_order() {
        let mut t = super::Xq120Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_120 VEB tree tests ----
    #[test]
    fn xq_120_veb_empty() {
        let v = super::Xq120VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_120_veb_insert_contains() {
        let mut v = super::Xq120VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_120_veb_min_max() {
        let mut v = super::Xq120VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_120_veb_delete() {
        let mut v = super::Xq120VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_120_veb_successor() {
        let mut v = super::Xq120VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_120_veb_predecessor() {
        let mut v = super::Xq120VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_120_veb_count() {
        let mut v = super::Xq120VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_120_veb_duplicate_insert() {
        let mut v = super::Xq120VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_120_kdtree_empty() {
        let tree = super::Xr120KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_120_kdtree_insert_one() {
        let mut tree = super::Xr120KDTree::xr_new();
        tree.xr_insert(super::Xr120KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_120_kdtree_insert_multiple() {
        let mut tree = super::Xr120KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr120KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_120_kdtree_nearest_neighbor() {
        let mut tree = super::Xr120KDTree::xr_new();
        tree.xr_insert(super::Xr120KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr120KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr120KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_120_kdtree_nn_empty() {
        let tree = super::Xr120KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr120KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_120_kdtree_range_search() {
        let mut tree = super::Xr120KDTree::xr_new();
        tree.xr_insert(super::Xr120KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr120KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr120KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_120_kdtree_range_empty() {
        let mut tree = super::Xr120KDTree::xr_new();
        tree.xr_insert(super::Xr120KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_120_kdtree_all_points() {
        let mut tree = super::Xr120KDTree::xr_new();
        tree.xr_insert(super::Xr120KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr120KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_120_kdtree_depth() {
        let mut tree = super::Xr120KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr120KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_120_kdtree_bounding_box() {
        let mut tree = super::Xr120KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr120KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr120KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_119_persistent_array_new() {
        let arr = super::Xs119PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_119_persistent_array_push() {
        let mut arr = super::Xs119PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_119_persistent_array_set() {
        let mut arr = super::Xs119PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_119_persistent_array_diff() {
        let mut arr = super::Xs119PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_119_persistent_array_rollback() {
        let mut arr = super::Xs119PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_119_persistent_array_history() {
        let mut arr = super::Xs119PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_119_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs119PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_119_persistent_array_from_vec() {
        let arr = super::Xs119PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_119_concurrent_queue_new() {
        let q = super::Xs119ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_119_concurrent_queue_push_pop() {
        let mut q = super::Xs119ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_119_concurrent_queue_full() {
        let mut q = super::Xs119ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_119_concurrent_queue_drain() {
        let mut q = super::Xs119ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_119_concurrent_queue_try_pop() {
        let mut q = super::Xs119ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_119_concurrent_queue_clear() {
        let mut q = super::Xs119ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_119_range_map_new() {
        let rm = super::Xs119RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_119_range_map_insert_get() {
        let mut rm = super::Xs119RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_119_range_map_overlap() {
        let mut rm = super::Xs119RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_119_range_map_remove() {
        let mut rm = super::Xs119RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_119_range_map_gaps() {
        let mut rm = super::Xs119RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_119_range_map_coverage() {
        let mut rm = super::Xs119RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_119_range_map_contains() {
        let mut rm = super::Xs119RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_119_range_map_clear() {
        let mut rm = super::Xs119RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_119_circular_buffer_new() {
        let buf = super::Xs119CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_119_circular_buffer_push_pop() {
        let mut buf = super::Xs119CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_119_circular_buffer_overwrite() {
        let mut buf = super::Xs119CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_119_circular_buffer_peek() {
        let mut buf = super::Xs119CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_119_circular_buffer_is_full() {
        let mut buf = super::Xs119CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_119_circular_buffer_iter() {
        let mut buf = super::Xs119CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_119_circular_buffer_clear() {
        let mut buf = super::Xs119CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_119_circular_buffer_to_vec() {
        let mut buf = super::Xs119CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

    #[test]
    fn xs_120_stats_tracker_new() {
        let tracker = super::Xs120StatsTracker::xs_new();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_120_stats_tracker_mean() {
        let mut tracker = super::Xs120StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        tracker.xs_add(30.0);
        assert!((tracker.xs_mean() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn xs_120_stats_tracker_min_max() {
        let mut tracker = super::Xs120StatsTracker::xs_new();
        tracker.xs_add(5.0);
        tracker.xs_add(15.0);
        tracker.xs_add(10.0);
        assert_eq!(tracker.xs_min(), Some(5.0));
        assert_eq!(tracker.xs_max(), Some(15.0));
    }

    #[test]
    fn xs_120_stats_tracker_median() {
        let mut tracker = super::Xs120StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(3.0);
        tracker.xs_add(2.0);
        assert_eq!(tracker.xs_median(), Some(2.0));
    }

    #[test]
    fn xs_120_stats_tracker_variance() {
        let mut tracker = super::Xs120StatsTracker::xs_new();
        tracker.xs_add(2.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(5.0);
        tracker.xs_add(5.0);
        tracker.xs_add(7.0);
        tracker.xs_add(9.0);
        let var = tracker.xs_variance();
        assert!(var > 0.0);
    }

    #[test]
    fn xs_120_stats_tracker_range() {
        let mut tracker = super::Xs120StatsTracker::xs_new();
        tracker.xs_add(3.0);
        tracker.xs_add(7.0);
        tracker.xs_add(1.0);
        assert!((tracker.xs_range() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn xs_120_stats_tracker_clear() {
        let mut tracker = super::Xs120StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(2.0);
        tracker.xs_clear();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_120_stats_tracker_sum() {
        let mut tracker = super::Xs120StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        assert!((tracker.xs_sum() - 30.0).abs() < 1e-9);
    }

}