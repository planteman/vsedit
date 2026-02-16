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

/// Accumulated statistics for refs-view operations.
#[derive(Debug, Clone, PartialEq)]
pub struct RefsViewStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl RefsViewStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &RefsViewStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for RefsViewStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RefsViewStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RefsViewStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for refs-view.
#[derive(Debug, Clone)]
pub struct RefsViewValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl RefsViewValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for RefsViewValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ReferenceService — aggregates multiple reference providers
// ---------------------------------------------------------------------------

/// Trait for providing references.
pub trait ReferenceProvider: Send + Sync {
    fn find_references(
        &self,
        uri: &str,
        line: u32,
        col: u32,
        include_declaration: bool,
    ) -> Vec<ReferenceItem>;
}

/// Aggregates reference providers and returns merged results.
pub struct ReferenceService {
    providers: Vec<Box<dyn ReferenceProvider>>,
}

impl ReferenceService {
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    pub fn register(&mut self, provider: Box<dyn ReferenceProvider>) {
        self.providers.push(provider);
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Query all providers and merge into a `ReferencesModel`.
    pub fn find_references(
        &self,
        symbol_name: &str,
        uri: &str,
        line: u32,
        col: u32,
        include_declaration: bool,
    ) -> ReferencesModel {
        let base = Location::new(uri, line, col, line, col);
        let mut model = ReferencesModel::new(symbol_name, base);
        for provider in &self.providers {
            let refs = provider.find_references(uri, line, col, include_declaration);
            for r in refs {
                model.add_reference(r);
            }
        }
        model.sort_by_location();
        model
    }

    /// Build a complete `ReferenceSearchResult` with timing placeholder.
    pub fn search(
        &self,
        symbol_name: &str,
        uri: &str,
        line: u32,
        col: u32,
        include_declaration: bool,
    ) -> ReferenceSearchResult {
        let model = self.find_references(symbol_name, uri, line, col, include_declaration);
        let mut result = ReferenceSearchResult::new(symbol_name, model, 0);
        if !include_declaration {
            result = result.without_declaration();
        }
        result
    }
}

impl Default for ReferenceService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Additional methods requested by user
// ---------------------------------------------------------------------------

impl Location {
    /// Return the number of lines this location spans (alias for clarity).
    pub fn line_count(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Return true if the given line falls within this location's line range.
    pub fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }
}

impl ReferenceItem {
    /// Return true if this reference points to the given URI.
    pub fn is_same_file_as(&self, uri: &str) -> bool {
        self.location.uri == uri
    }
}

impl ReferenceKind {
    /// Return a human-readable label for this kind.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Declaration => "declaration",
            Self::Definition => "definition",
            Self::Read => "read",
            Self::Write => "write",
            Self::Call => "call",
            Self::Import => "import",
            Self::Other => "other",
        }
    }

    /// Return true if this kind is `Definition`.
    pub fn is_definition(&self) -> bool {
        matches!(self, Self::Definition)
    }
}

/// A reference item annotated with its kind.
#[derive(Debug, Clone)]
pub struct KindedReferenceItem {
    pub item: ReferenceItem,
    pub kind: ReferenceKind,
}

impl ReferencesModel {
    /// Return references matching the given kind from a parallel kinded list.
    ///
    /// This operates on a supplied slice of `KindedReferenceItem` because
    /// `ReferencesModel` does not store kinds itself.
    pub fn filter_by_kind<'a>(
        items: &'a [KindedReferenceItem],
        kind: ReferenceKind,
    ) -> Vec<&'a ReferenceItem> {
        items
            .iter()
            .filter(|k| k.kind == kind)
            .map(|k| &k.item)
            .collect()
    }

    /// Merge all references from `other` into `self`.
    pub fn merge(&mut self, other: ReferencesModel) {
        for r in other.references {
            self.references.push(r);
        }
    }
}

impl fmt::Display for ReferencesModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} references in {} files",
            self.total_count(),
            self.file_count()
        )
    }
}

/// Convenience function: find all references at a position.
pub fn find_references_at(
    service: &ReferenceService,
    symbol_name: &str,
    uri: &str,
    line: u32,
    col: u32,
    include_declaration: bool,
) -> ReferencesModel {
    service.find_references(symbol_name, uri, line, col, include_declaration)
}

// ---------------------------------------------------------------------------
// ReferenceGraph — graph of references between files
// ---------------------------------------------------------------------------

/// An edge in the reference graph: file A references file B.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceEdge {
    pub from_uri: String,
    pub to_uri: String,
    pub count: usize,
}

/// A directed graph of file-to-file references.
#[derive(Debug, Clone, Default)]
pub struct ReferenceGraph {
    edges: Vec<ReferenceEdge>,
}

impl ReferenceGraph {
    pub fn new() -> Self {
        Self { edges: Vec::new() }
    }

    /// Build a reference graph from a ReferencesModel.
    /// The base location's URI is the "from" file; each reference's URI is the "to" file.
    pub fn from_model(model: &ReferencesModel) -> Self {
        let from = &model.base_location.uri;
        let mut edge_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for r in &model.references {
            *edge_map.entry(r.location.uri.clone()).or_insert(0) += 1;
        }
        let edges = edge_map.into_iter().map(|(to_uri, count)| {
            ReferenceEdge { from_uri: from.clone(), to_uri, count }
        }).collect();
        Self { edges }
    }

    /// Return all edges in the graph.
    pub fn edges(&self) -> &[ReferenceEdge] {
        &self.edges
    }

    /// Return files that the given URI references (outgoing edges).
    pub fn outgoing(&self, uri: &str) -> Vec<&ReferenceEdge> {
        self.edges.iter().filter(|e| e.from_uri == uri).collect()
    }

    /// Return files that reference the given URI (incoming edges).
    pub fn incoming(&self, uri: &str) -> Vec<&ReferenceEdge> {
        self.edges.iter().filter(|e| e.to_uri == uri).collect()
    }

    /// Return the total number of edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Return all unique file URIs involved in the graph.
    pub fn all_files(&self) -> Vec<&str> {
        let mut files: Vec<&str> = self.edges.iter()
            .flat_map(|e| vec![e.from_uri.as_str(), e.to_uri.as_str()])
            .collect();
        files.sort_unstable();
        files.dedup();
        files
    }

    /// Add an edge or increment count if one already exists.
    pub fn add_edge(&mut self, from: &str, to: &str) {
        if let Some(edge) = self.edges.iter_mut().find(|e| e.from_uri == from && e.to_uri == to) {
            edge.count += 1;
        } else {
            self.edges.push(ReferenceEdge {
                from_uri: from.to_string(),
                to_uri: to.to_string(),
                count: 1,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// ReferenceCluster — cluster related references
// ---------------------------------------------------------------------------

/// A cluster of references that are logically related.
#[derive(Debug, Clone)]
pub struct ReferenceCluster<'a> {
    pub uri: String,
    pub items: Vec<&'a ReferenceItem>,
    pub start_line: u32,
    pub end_line: u32,
}

impl<'a> ReferenceCluster<'a> {
    /// Number of references in this cluster.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Line span of the cluster.
    pub fn line_span(&self) -> u32 {
        self.end_line - self.start_line + 1
    }
}

/// Cluster references within a model by proximity within files.
pub fn cluster_references<'a>(model: &'a ReferencesModel, max_gap: u32) -> Vec<ReferenceCluster<'a>> {
    let mut clusters = Vec::new();
    for file_uri in model.unique_files() {
        let sorted = model.sorted_refs_in_file(file_uri);
        if sorted.is_empty() {
            continue;
        }
        let mut current_items: Vec<&ReferenceItem> = vec![sorted[0]];
        let mut start_line = sorted[0].location.start_line;
        let mut end_line = sorted[0].location.end_line;
        for r in &sorted[1..] {
            if r.location.start_line <= end_line + max_gap {
                current_items.push(r);
                end_line = end_line.max(r.location.end_line);
            } else {
                clusters.push(ReferenceCluster {
                    uri: file_uri.to_string(),
                    items: std::mem::take(&mut current_items),
                    start_line,
                    end_line,
                });
                current_items.push(r);
                start_line = r.location.start_line;
                end_line = r.location.end_line;
            }
        }
        if !current_items.is_empty() {
            clusters.push(ReferenceCluster {
                uri: file_uri.to_string(),
                items: current_items,
                start_line,
                end_line,
            });
        }
    }
    clusters
}

// ---------------------------------------------------------------------------
// ReferenceExporter — export references in different formats
// ---------------------------------------------------------------------------

/// Format for exporting references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    PlainText,
    Csv,
    Json,
}

/// Export a ReferencesModel in the specified format.
pub fn export_references(model: &ReferencesModel, format: ExportFormat) -> String {
    match format {
        ExportFormat::PlainText => {
            let mut lines = Vec::new();
            for r in &model.references {
                lines.push(format!("{}:{}:{}: {}", r.location.uri, r.location.start_line, r.location.start_col, r.context_line));
            }
            lines.join("\n")
        }
        ExportFormat::Csv => {
            let mut lines = vec!["uri,line,col,context".to_string()];
            for r in &model.references {
                let escaped = r.context_line.replace('"', "\"\"");
                lines.push(format!("{},{},{},\"{}\"", r.location.uri, r.location.start_line, r.location.start_col, escaped));
            }
            lines.join("\n")
        }
        ExportFormat::Json => {
            let entries: Vec<String> = model.references.iter().map(|r| {
                format!(
                    "{{\"uri\":\"{}\",\"line\":{},\"col\":{},\"context\":\"{}\"}}",
                    r.location.uri, r.location.start_line, r.location.start_col,
                    r.context_line.replace('\\', "\\\\").replace('"', "\\\"")
                )
            }).collect();
            format!("[{}]", entries.join(","))
        }
    }
}

// ---------------------------------------------------------------------------
// ReferencesModel grouping by kind
// ---------------------------------------------------------------------------

impl ReferencesModel {
    /// Group kinded reference items by their kind.
    pub fn group_by_kind(items: &[KindedReferenceItem]) -> Vec<(ReferenceKind, Vec<&ReferenceItem>)> {
        let kinds = [
            ReferenceKind::Declaration, ReferenceKind::Definition,
            ReferenceKind::Read, ReferenceKind::Write,
            ReferenceKind::Call, ReferenceKind::Import,
            ReferenceKind::Other,
        ];
        kinds.iter().filter_map(|kind| {
            let refs: Vec<&ReferenceItem> = items.iter()
                .filter(|k| k.kind == *kind)
                .map(|k| &k.item)
                .collect();
            if refs.is_empty() { None } else { Some((*kind, refs)) }
        }).collect()
    }
}

// ---------------------------------------------------------------------------
// ReferenceDiff — compare two reference models
// ---------------------------------------------------------------------------

/// Result of diffing two `ReferencesModel` instances.
#[derive(Debug, Clone)]
pub struct ReferenceDiff {
    /// References present in `new` but not in `old`.
    pub added: Vec<Location>,
    /// References present in `old` but not in `new`.
    pub removed: Vec<Location>,
    /// Number of references shared between both models.
    pub unchanged_count: usize,
}

impl ReferenceDiff {
    /// Compute the diff between an old and new model based on location equality.
    pub fn diff(old: &ReferencesModel, new: &ReferencesModel) -> Self {
        let old_locs: Vec<&Location> = old.references.iter().map(|r| &r.location).collect();
        let new_locs: Vec<&Location> = new.references.iter().map(|r| &r.location).collect();

        let added: Vec<Location> = new_locs
            .iter()
            .filter(|loc| !old_locs.contains(loc))
            .map(|loc| (*loc).clone())
            .collect();

        let removed: Vec<Location> = old_locs
            .iter()
            .filter(|loc| !new_locs.contains(loc))
            .map(|loc| (*loc).clone())
            .collect();

        let unchanged_count = new_locs
            .iter()
            .filter(|loc| old_locs.contains(loc))
            .count();

        Self {
            added,
            removed,
            unchanged_count,
        }
    }

    /// Whether the two models are identical (no additions or removals).
    pub fn is_identical(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    /// Total number of changes (added + removed).
    pub fn change_count(&self) -> usize {
        self.added.len() + self.removed.len()
    }
}

impl fmt::Display for ReferenceDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ReferenceDiff(+{} -{} ={} )",
            self.added.len(),
            self.removed.len(),
            self.unchanged_count,
        )
    }
}

// ---------------------------------------------------------------------------
// ReferenceNavigator — stateful navigation through references
// ---------------------------------------------------------------------------

/// Enables sequential navigation through references in a model.
pub struct ReferenceNavigator<'a> {
    refs: Vec<&'a ReferenceItem>,
    index: Option<usize>,
}

impl<'a> ReferenceNavigator<'a> {
    /// Create a navigator from a model's references.
    pub fn new(model: &'a ReferencesModel) -> Self {
        Self {
            refs: model.references.iter().collect(),
            index: None,
        }
    }

    /// Move to the next reference. Wraps around.
    pub fn next(&mut self) -> Option<&'a ReferenceItem> {
        if self.refs.is_empty() {
            return None;
        }
        let next = match self.index {
            Some(i) => (i + 1) % self.refs.len(),
            None => 0,
        };
        self.index = Some(next);
        Some(self.refs[next])
    }

    /// Move to the previous reference. Wraps around.
    pub fn previous(&mut self) -> Option<&'a ReferenceItem> {
        if self.refs.is_empty() {
            return None;
        }
        let prev = match self.index {
            Some(0) => self.refs.len() - 1,
            Some(i) => i - 1,
            None => self.refs.len() - 1,
        };
        self.index = Some(prev);
        Some(self.refs[prev])
    }

    /// Current reference, if any.
    pub fn current(&self) -> Option<&'a ReferenceItem> {
        self.index.map(|i| self.refs[i])
    }

    /// Current index (1-based display), or None if not started.
    pub fn position(&self) -> Option<(usize, usize)> {
        self.index.map(|i| (i + 1, self.refs.len()))
    }

    /// Total number of navigable references.
    pub fn len(&self) -> usize {
        self.refs.len()
    }

    /// Whether there are no references to navigate.
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
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

    #[test]
    fn refs_view_stats_new_defaults() {
        let stats = RefsViewStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn refs_view_stats_record_success() {
        let mut stats = RefsViewStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn refs_view_stats_record_failure() {
        let mut stats = RefsViewStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn refs_view_stats_reset() {
        let mut stats = RefsViewStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn refs_view_stats_merge() {
        let mut a = RefsViewStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = RefsViewStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn refs_view_stats_display() {
        let mut stats = RefsViewStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn refs_view_stats_default() {
        let stats = RefsViewStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn refs_view_validator_accepts_valid_name() {
        let v = RefsViewValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn refs_view_validator_rejects_empty() {
        let v = RefsViewValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn refs_view_validator_rejects_too_long() {
        let v = RefsViewValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn refs_view_validator_forbidden_prefix() {
        let v = RefsViewValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn refs_view_validator_allowed_chars() {
        let v = RefsViewValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn refs_view_validator_range() {
        let v = RefsViewValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn refs_view_sanitize_removes_control() {
        let result = RefsViewValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn refs_view_truncate_short_string() {
        assert_eq!(RefsViewValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn refs_view_truncate_long_string() {
        let result = RefsViewValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn refs_view_is_ascii_printable() {
        assert!(RefsViewValidator::is_ascii_printable("Hello World 123"));
        assert!(!RefsViewValidator::is_ascii_printable("Hello\x00World"));
    }

    // --- ReferenceService tests ---

    struct DummyRefProvider {
        refs: Vec<ReferenceItem>,
    }

    impl ReferenceProvider for DummyRefProvider {
        fn find_references(&self, _uri: &str, _line: u32, _col: u32, _incl: bool) -> Vec<ReferenceItem> {
            self.refs.clone()
        }
    }

    fn make_ref_item(uri: &str, line: u32) -> ReferenceItem {
        ReferenceItem {
            location: Location::new(uri, line, 0, line, 10),
            context_before: None,
            context_line: format!("line {line}"),
            context_after: None,
        }
    }

    #[test]
    fn reference_service_empty() {
        let svc = ReferenceService::new();
        assert_eq!(svc.provider_count(), 0);
        let model = svc.find_references("sym", "f.rs", 1, 0, true);
        assert!(model.is_empty());
    }

    #[test]
    fn reference_service_single_provider() {
        let mut svc = ReferenceService::new();
        svc.register(Box::new(DummyRefProvider {
            refs: vec![make_ref_item("a.rs", 5), make_ref_item("b.rs", 10)],
        }));
        assert_eq!(svc.provider_count(), 1);
        let model = svc.find_references("foo", "a.rs", 5, 0, true);
        assert_eq!(model.total_count(), 2);
        assert_eq!(model.file_count(), 2);
    }

    #[test]
    fn reference_service_multiple_providers() {
        let mut svc = ReferenceService::new();
        svc.register(Box::new(DummyRefProvider {
            refs: vec![make_ref_item("a.rs", 1)],
        }));
        svc.register(Box::new(DummyRefProvider {
            refs: vec![make_ref_item("b.rs", 2)],
        }));
        let model = svc.find_references("sym", "f", 0, 0, true);
        assert_eq!(model.total_count(), 2);
    }

    #[test]
    fn reference_service_search() {
        let mut svc = ReferenceService::new();
        svc.register(Box::new(DummyRefProvider {
            refs: vec![make_ref_item("x.rs", 3)],
        }));
        let result = svc.search("myfn", "x.rs", 3, 0, true);
        assert_eq!(result.symbol_name, "myfn");
        assert!(result.include_declaration);
        assert_eq!(result.model.total_count(), 1);
    }

    #[test]
    fn reference_service_search_no_decl() {
        let svc = ReferenceService::default();
        let result = svc.search("sym", "f", 0, 0, false);
        assert!(!result.include_declaration);
    }

    #[test]
    fn find_references_at_convenience() {
        let svc = ReferenceService::new();
        let model = find_references_at(&svc, "sym", "f", 0, 0, true);
        assert!(model.is_empty());
    }

    // --- Tests for newly added functionality ---

    #[test]
    fn location_line_count() {
        assert_eq!(Location::new("a.rs", 5, 0, 5, 10).line_count(), 1);
        assert_eq!(Location::new("a.rs", 5, 0, 9, 10).line_count(), 5);
        assert_eq!(Location::new("a.rs", 1, 0, 100, 0).line_count(), 100);
    }

    #[test]
    fn location_contains_line() {
        let l = Location::new("a.rs", 5, 0, 10, 0);
        assert!(l.contains_line(5));
        assert!(l.contains_line(7));
        assert!(l.contains_line(10));
        assert!(!l.contains_line(4));
        assert!(!l.contains_line(11));
    }

    #[test]
    fn reference_item_is_same_file_as() {
        let r = ref_item("src/main.rs", 1, 0);
        assert!(r.is_same_file_as("src/main.rs"));
        assert!(!r.is_same_file_as("src/lib.rs"));
    }

    #[test]
    fn reference_kind_label() {
        assert_eq!(ReferenceKind::Declaration.label(), "declaration");
        assert_eq!(ReferenceKind::Definition.label(), "definition");
        assert_eq!(ReferenceKind::Read.label(), "read");
        assert_eq!(ReferenceKind::Write.label(), "write");
        assert_eq!(ReferenceKind::Call.label(), "call");
        assert_eq!(ReferenceKind::Import.label(), "import");
        assert_eq!(ReferenceKind::Other.label(), "other");
    }

    #[test]
    fn reference_kind_is_definition() {
        assert!(ReferenceKind::Definition.is_definition());
        assert!(!ReferenceKind::Declaration.is_definition());
        assert!(!ReferenceKind::Read.is_definition());
        assert!(!ReferenceKind::Call.is_definition());
        assert!(!ReferenceKind::Other.is_definition());
    }

    #[test]
    fn filter_by_kind() {
        let items = vec![
            KindedReferenceItem { item: ref_item("a.rs", 1, 0), kind: ReferenceKind::Read },
            KindedReferenceItem { item: ref_item("a.rs", 2, 0), kind: ReferenceKind::Write },
            KindedReferenceItem { item: ref_item("a.rs", 3, 0), kind: ReferenceKind::Read },
            KindedReferenceItem { item: ref_item("b.rs", 4, 0), kind: ReferenceKind::Definition },
        ];
        let reads = ReferencesModel::filter_by_kind(&items, ReferenceKind::Read);
        assert_eq!(reads.len(), 2);
        assert_eq!(reads[0].location.start_line, 1);
        assert_eq!(reads[1].location.start_line, 3);

        let defs = ReferencesModel::filter_by_kind(&items, ReferenceKind::Definition);
        assert_eq!(defs.len(), 1);

        let calls = ReferencesModel::filter_by_kind(&items, ReferenceKind::Call);
        assert!(calls.is_empty());
    }

    #[test]
    fn model_merge() {
        let mut a = ReferencesModel::new("sym", loc("a.rs", 1, 0));
        a.add_reference(ref_item("a.rs", 10, 0));
        let mut b = ReferencesModel::new("sym", loc("a.rs", 1, 0));
        b.add_reference(ref_item("b.rs", 20, 0));
        b.add_reference(ref_item("c.rs", 30, 0));
        a.merge(b);
        assert_eq!(a.total_count(), 3);
        assert_eq!(a.file_count(), 3);
    }

    #[test]
    fn model_display() {
        let mut model = ReferencesModel::new("foo", loc("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 10, 0));
        model.add_reference(ref_item("b.rs", 20, 0));
        model.add_reference(ref_item("a.rs", 30, 0));
        let s = format!("{model}");
        assert_eq!(s, "3 references in 2 files");
    }

    // -- ReferenceGraph tests -----------------------------------------------

    #[test]
    fn graph_from_model() {
        let mut model = ReferencesModel::new("sym", loc("main.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 10, 0));
        model.add_reference(ref_item("a.rs", 20, 0));
        model.add_reference(ref_item("b.rs", 5, 0));

        let graph = ReferenceGraph::from_model(&model);
        assert_eq!(graph.edge_count(), 2);
        let outgoing = graph.outgoing("main.rs");
        assert_eq!(outgoing.len(), 2);
        // a.rs should have count 2
        let a_edge = outgoing.iter().find(|e| e.to_uri == "a.rs").unwrap();
        assert_eq!(a_edge.count, 2);
    }

    #[test]
    fn graph_add_edge_increments() {
        let mut graph = ReferenceGraph::new();
        graph.add_edge("x.rs", "y.rs");
        graph.add_edge("x.rs", "y.rs");
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(graph.edges()[0].count, 2);
    }

    #[test]
    fn graph_incoming_and_all_files() {
        let mut graph = ReferenceGraph::new();
        graph.add_edge("a.rs", "b.rs");
        graph.add_edge("c.rs", "b.rs");
        let incoming = graph.incoming("b.rs");
        assert_eq!(incoming.len(), 2);
        let files = graph.all_files();
        assert_eq!(files.len(), 3);
    }

    // -- ReferenceCluster tests ---------------------------------------------

    #[test]
    fn cluster_references_groups_nearby() {
        let mut model = ReferencesModel::new("sym", loc("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 10, 0));
        model.add_reference(ref_item("a.rs", 12, 0));
        model.add_reference(ref_item("a.rs", 50, 0));
        let clusters = cluster_references(&model, 5);
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].len(), 2);
        assert_eq!(clusters[1].len(), 1);
    }

    // -- ReferenceExporter tests --------------------------------------------

    #[test]
    fn export_plain_text_format() {
        let mut model = ReferencesModel::new("sym", loc("a.rs", 1, 0));
        model.add_reference(ref_item("a.rs", 10, 5));
        let text = export_references(&model, ExportFormat::PlainText);
        assert!(text.contains("a.rs:10:5:"));
    }

    #[test]
    fn export_csv_format() {
        let mut model = ReferencesModel::new("sym", loc("a.rs", 1, 0));
        model.add_reference(ref_item("b.rs", 20, 0));
        let csv = export_references(&model, ExportFormat::Csv);
        assert!(csv.starts_with("uri,line,col,context"));
        assert!(csv.contains("b.rs,20,0,"));
    }

    // -- Group by kind tests ------------------------------------------------

    #[test]
    fn group_by_kind_partitions_items() {
        let items = vec![
            KindedReferenceItem { item: ref_item("a.rs", 1, 0), kind: ReferenceKind::Read },
            KindedReferenceItem { item: ref_item("a.rs", 5, 0), kind: ReferenceKind::Write },
            KindedReferenceItem { item: ref_item("b.rs", 10, 0), kind: ReferenceKind::Read },
        ];
        let groups = ReferencesModel::group_by_kind(&items);
        assert_eq!(groups.len(), 2); // Read and Write
        let read_group = groups.iter().find(|(k, _)| *k == ReferenceKind::Read).unwrap();
        assert_eq!(read_group.1.len(), 2);
    }

    // -- ReferenceDiff tests ---------------------------------------------------

    #[test]
    fn diff_identical_models() {
        let base = Location::new("main.rs", 1, 0, 1, 5);
        let mut m1 = ReferencesModel::new("foo", base.clone());
        m1.add_reference(ref_item("a.rs", 10, 5));
        m1.add_reference(ref_item("b.rs", 20, 0));

        let mut m2 = ReferencesModel::new("foo", base);
        m2.add_reference(ref_item("a.rs", 10, 5));
        m2.add_reference(ref_item("b.rs", 20, 0));

        let diff = ReferenceDiff::diff(&m1, &m2);
        assert!(diff.is_identical());
        assert_eq!(diff.change_count(), 0);
        assert_eq!(diff.unchanged_count, 2);
    }

    #[test]
    fn diff_detects_added_and_removed() {
        let base = Location::new("main.rs", 1, 0, 1, 5);
        let mut old = ReferencesModel::new("foo", base.clone());
        old.add_reference(ref_item("a.rs", 10, 5));

        let mut new = ReferencesModel::new("foo", base);
        new.add_reference(ref_item("b.rs", 20, 0));

        let diff = ReferenceDiff::diff(&old, &new);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.added[0].uri, "b.rs");
        assert_eq!(diff.removed[0].uri, "a.rs");
    }

    #[test]
    fn diff_display() {
        let base = Location::new("main.rs", 1, 0, 1, 5);
        let old = ReferencesModel::new("foo", base.clone());
        let mut new = ReferencesModel::new("foo", base);
        new.add_reference(ref_item("c.rs", 1, 0));
        let diff = ReferenceDiff::diff(&old, &new);
        let display = format!("{diff}");
        assert!(display.contains("+1"));
    }

    // -- ReferenceNavigator tests ---------------------------------------------

    #[test]
    fn navigator_next_wraps() {
        let base = Location::new("main.rs", 1, 0, 1, 5);
        let mut model = ReferencesModel::new("foo", base);
        model.add_reference(ref_item("a.rs", 1, 0));
        model.add_reference(ref_item("b.rs", 2, 0));

        let mut nav = ReferenceNavigator::new(&model);
        assert!(nav.current().is_none());
        let first = nav.next().unwrap();
        assert_eq!(first.location.uri, "a.rs");
        let second = nav.next().unwrap();
        assert_eq!(second.location.uri, "b.rs");
        let wrapped = nav.next().unwrap();
        assert_eq!(wrapped.location.uri, "a.rs");
    }

    #[test]
    fn navigator_previous_wraps() {
        let base = Location::new("main.rs", 1, 0, 1, 5);
        let mut model = ReferencesModel::new("foo", base);
        model.add_reference(ref_item("a.rs", 1, 0));
        model.add_reference(ref_item("b.rs", 2, 0));

        let mut nav = ReferenceNavigator::new(&model);
        let last = nav.previous().unwrap();
        assert_eq!(last.location.uri, "b.rs");
        assert_eq!(nav.position(), Some((2, 2)));
    }

    #[test]
    fn navigator_empty_model() {
        let base = Location::new("main.rs", 1, 0, 1, 5);
        let model = ReferencesModel::new("foo", base);
        let mut nav = ReferenceNavigator::new(&model);
        assert!(nav.is_empty());
        assert!(nav.next().is_none());
        assert!(nav.previous().is_none());
    }
}
