//! Diagnostic markers service

use std::collections::HashMap;
use std::sync::Mutex;

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
// Tests
// ---------------------------------------------------------------------------

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
}
