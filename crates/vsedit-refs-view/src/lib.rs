//! References view.

use std::collections::{HashMap, HashSet};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Groups references by file path or by kind.
pub struct ReferencesGrouper;

impl ReferencesGrouper {
    /// Group kinded references by their file URI, sorted alphabetically.
    pub fn group_by_file<'a>(
        refs: &'a [KindedReferenceItem],
    ) -> Vec<(String, Vec<&'a KindedReferenceItem>)> {
        let mut map: HashMap<&str, Vec<&'a KindedReferenceItem>> = HashMap::new();
        for r in refs {
            map.entry(r.item.location.uri.as_str())
                .or_default()
                .push(r);
        }
        let mut groups: Vec<(String, Vec<&'a KindedReferenceItem>)> = map
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        groups.sort_by(|a, b| a.0.cmp(&b.0));
        groups
    }

    /// Group kinded references by their `ReferenceKind`.
    pub fn group_by_kind<'a>(
        refs: &'a [KindedReferenceItem],
    ) -> Vec<(ReferenceKind, Vec<&'a KindedReferenceItem>)> {
        let mut map: HashMap<ReferenceKind, Vec<&'a KindedReferenceItem>> = HashMap::new();
        for r in refs {
            map.entry(r.kind).or_default().push(r);
        }
        let mut groups: Vec<(ReferenceKind, Vec<&'a KindedReferenceItem>)> =
            map.into_iter().collect();
        groups.sort_by_key(|(k, _)| format!("{k:?}"));
        groups
    }

    /// Count the number of distinct files across kinded references.
    pub fn file_count(refs: &[KindedReferenceItem]) -> usize {
        let files: HashSet<&str> = refs.iter().map(|r| r.item.location.uri.as_str()).collect();
        files.len()
    }
}

/// Filters references by included or excluded kinds.
#[derive(Debug, Clone)]
pub struct ReferencesFilter {
    included: HashSet<ReferenceKind>,
    excluded: HashSet<ReferenceKind>,
}

impl ReferencesFilter {
    /// Create a new filter with no constraints.
    pub fn new() -> Self {
        Self {
            included: HashSet::new(),
            excluded: HashSet::new(),
        }
    }

    /// Add a kind to the inclusion set.
    pub fn include_kind(&mut self, kind: ReferenceKind) {
        self.included.insert(kind);
        self.excluded.remove(&kind);
    }

    /// Add a kind to the exclusion set.
    pub fn exclude_kind(&mut self, kind: ReferenceKind) {
        self.excluded.insert(kind);
        self.included.remove(&kind);
    }

    /// Apply the filter to a slice of kinded references.
    ///
    /// If inclusions are set, only matching kinds are kept.
    /// Exclusions are then removed from the result.
    pub fn apply<'a>(&self, refs: &'a [KindedReferenceItem]) -> Vec<&'a KindedReferenceItem> {
        refs.iter()
            .filter(|r| {
                if !self.included.is_empty() && !self.included.contains(&r.kind) {
                    return false;
                }
                if self.excluded.contains(&r.kind) {
                    return false;
                }
                true
            })
            .collect()
    }

    /// Returns true when no include or exclude constraints have been set.
    pub fn is_empty(&self) -> bool {
        self.included.is_empty() && self.excluded.is_empty()
    }
}

impl Default for ReferencesFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// A single line in a peek preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewLine {
    /// 1-based line number in the original file.
    pub line_number: u32,
    /// Text content of the line.
    pub text: String,
    /// Whether this line is the target reference line.
    pub is_target: bool,
}

/// Generates peek preview context for a reference location.
#[derive(Debug, Clone)]
pub struct ReferencePeekPreview {
    context_lines: usize,
}

impl ReferencePeekPreview {
    /// Create a new peek preview generator with the given number of context lines
    /// above and below the target line.
    pub fn new(context_lines: usize) -> Self {
        Self { context_lines }
    }

    /// Extract surrounding lines from `content` around the 0-based `line` index,
    /// returning them as a single string.
    pub fn generate(&self, content: &str, line: u32) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = line as usize;
        if line_idx >= lines.len() {
            return String::new();
        }
        let start = line_idx.saturating_sub(self.context_lines);
        let end = (line_idx + self.context_lines + 1).min(lines.len());
        lines[start..end].join("\n")
    }

    /// Extract surrounding lines with highlight metadata, returning a vec of
    /// `PreviewLine` values. The target line is marked with `is_target = true`.
    pub fn generate_with_highlight(&self, content: &str, line: u32) -> Vec<PreviewLine> {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = line as usize;
        if line_idx >= lines.len() {
            return Vec::new();
        }
        let start = line_idx.saturating_sub(self.context_lines);
        let end = (line_idx + self.context_lines + 1).min(lines.len());
        (start..end)
            .map(|i| PreviewLine {
                line_number: (i + 1) as u32,
                text: lines[i].to_string(),
                is_target: i == line_idx,
            })
            .collect()
    }
}

/// Tracks which reference locations have been visited during navigation.
#[derive(Debug, Clone)]
pub struct ReferenceNavigationHistory {
    visits: Vec<(String, u32)>,
    visited_set: HashSet<(String, u32)>,
}

impl ReferenceNavigationHistory {
    /// Create a new empty navigation history.
    pub fn new() -> Self {
        Self {
            visits: Vec::new(),
            visited_set: HashSet::new(),
        }
    }

    /// Record a visit to the given URI and line.
    pub fn visit(&mut self, uri: &str, line: u32) {
        let key = (uri.to_string(), line);
        if self.visited_set.insert(key.clone()) {
            self.visits.push(key);
        }
    }

    /// Check whether a specific location has been visited.
    pub fn is_visited(&self, uri: &str, line: u32) -> bool {
        self.visited_set.contains(&(uri.to_string(), line))
    }

    /// Return the total number of unique visited locations.
    pub fn visited_count(&self) -> usize {
        self.visited_set.len()
    }

    /// Clear all visit history.
    pub fn clear(&mut self) {
        self.visits.clear();
        self.visited_set.clear();
    }

    /// Return the most recent `n` visits in reverse chronological order.
    pub fn recent_visits(&self, n: usize) -> Vec<(String, u32)> {
        self.visits.iter().rev().take(n).cloned().collect()
    }
}

impl Default for ReferenceNavigationHistory {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ReferenceCountBadge – display reference counts
// ---------------------------------------------------------------------------

/// A badge showing the count of references for a symbol.
#[derive(Debug, Clone)]
pub struct ReferenceCountBadge {
    pub symbol_name: String,
    pub total_refs: usize,
    pub file_count: usize,
    pub kind_counts: HashMap<ReferenceKind, usize>,
}

impl ReferenceCountBadge {
    /// Create a badge from a references model.
    pub fn from_model(symbol_name: impl Into<String>, model: &ReferencesModel) -> Self {
        Self {
            symbol_name: symbol_name.into(),
            total_refs: model.total_count(),
            file_count: model.file_count(),
            kind_counts: HashMap::new(),
        }
    }

    /// Increment the count for a given reference kind.
    pub fn add_kind(&mut self, kind: ReferenceKind, count: usize) {
        *self.kind_counts.entry(kind).or_insert(0) += count;
    }

    /// Get the count for a specific kind.
    pub fn kind_count(&self, kind: &ReferenceKind) -> usize {
        self.kind_counts.get(kind).copied().unwrap_or(0)
    }

    /// Format as a short badge string, e.g. "foo (5 refs in 3 files)".
    pub fn format_short(&self) -> String {
        format!(
            "{} ({} ref{} in {} file{})",
            self.symbol_name,
            self.total_refs,
            if self.total_refs == 1 { "" } else { "s" },
            self.file_count,
            if self.file_count == 1 { "" } else { "s" },
        )
    }

    /// Format as a detailed badge with kind breakdown.
    pub fn format_detailed(&self) -> String {
        let mut parts = vec![self.format_short()];
        if !self.kind_counts.is_empty() {
            let mut kinds: Vec<(&ReferenceKind, &usize)> = self.kind_counts.iter().collect();
            kinds.sort_by(|a, b| b.1.cmp(a.1));
            for (kind, count) in kinds {
                parts.push(format!("  {kind}: {count}"));
            }
        }
        parts.join("\n")
    }

    /// Returns true if there are any references.
    pub fn has_references(&self) -> bool {
        self.total_refs > 0
    }

    /// Total across all tracked kinds.
    pub fn tracked_kind_total(&self) -> usize {
        self.kind_counts.values().sum()
    }
}

impl fmt::Display for ReferenceCountBadge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_short())
    }
}

// ---------------------------------------------------------------------------
// ReferenceInlinePreview – inline preview of references
// ---------------------------------------------------------------------------

/// An inline preview showing a reference with surrounding context.
#[derive(Debug, Clone)]
pub struct ReferenceInlinePreview {
    pub location: Location,
    pub preview_lines: Vec<String>,
    pub highlight_line_index: usize,
    pub max_preview_lines: usize,
}

impl ReferenceInlinePreview {
    pub fn new(location: Location, context_line: String, max_preview_lines: usize) -> Self {
        Self {
            location,
            preview_lines: vec![context_line],
            highlight_line_index: 0,
            max_preview_lines,
        }
    }

    /// Create from a ReferenceItem, including its context if available.
    pub fn from_reference_item(item: &ReferenceItem, max_lines: usize) -> Self {
        let mut lines = Vec::new();
        let mut highlight_idx = 0;
        if let Some(ref before) = item.context_before {
            lines.push(before.clone());
            highlight_idx = 1;
        }
        lines.push(item.context_line.clone());
        if let Some(ref after) = item.context_after {
            lines.push(after.clone());
        }
        Self {
            location: item.location.clone(),
            preview_lines: lines,
            highlight_line_index: highlight_idx,
            max_preview_lines: max_lines,
        }
    }

    /// Get the highlighted (main) line.
    pub fn highlighted_line(&self) -> &str {
        self.preview_lines.get(self.highlight_line_index).map_or("", |s| s.as_str())
    }

    /// Total number of preview lines.
    pub fn line_count(&self) -> usize {
        self.preview_lines.len()
    }

    /// Is the preview truncated?
    pub fn is_truncated(&self) -> bool {
        self.preview_lines.len() > self.max_preview_lines
    }

    /// Get lines capped to max.
    pub fn visible_lines(&self) -> &[String] {
        let end = self.preview_lines.len().min(self.max_preview_lines);
        &self.preview_lines[..end]
    }

    /// Format the preview header (file:line).
    pub fn format_header(&self) -> String {
        format!("{}:{}", self.location.uri, self.location.start_line)
    }

    /// Render the full preview as a string block.
    pub fn render(&self) -> String {
        let mut output = vec![self.format_header()];
        for (i, line) in self.visible_lines().iter().enumerate() {
            let prefix = if i == self.highlight_line_index { ">" } else { " " };
            output.push(format!("{prefix} {line}"));
        }
        output.join("\n")
    }
}

// ---------------------------------------------------------------------------
// ReferenceNavigationBreadcrumb – breadcrumb trail for navigation
// ---------------------------------------------------------------------------

/// A single breadcrumb entry in reference navigation.
#[derive(Debug, Clone)]
pub struct BreadcrumbEntry {
    pub label: String,
    pub uri: String,
    pub line: u32,
}

impl BreadcrumbEntry {
    pub fn new(label: impl Into<String>, uri: impl Into<String>, line: u32) -> Self {
        Self {
            label: label.into(),
            uri: uri.into(),
            line,
        }
    }
}

impl fmt::Display for BreadcrumbEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.label, self.line)
    }
}

/// A breadcrumb trail for navigating through references.
#[derive(Debug)]
pub struct ReferenceNavigationBreadcrumb {
    entries: Vec<BreadcrumbEntry>,
    max_entries: usize,
    current_index: usize,
}

impl ReferenceNavigationBreadcrumb {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
            current_index: 0,
        }
    }

    /// Push a new entry onto the breadcrumb trail.
    pub fn push(&mut self, entry: BreadcrumbEntry) {
        // Truncate any forward history
        if self.current_index < self.entries.len() {
            self.entries.truncate(self.current_index);
        }
        self.entries.push(entry);
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
        self.current_index = self.entries.len();
    }

    /// Navigate back one step.
    pub fn go_back(&mut self) -> Option<&BreadcrumbEntry> {
        if self.current_index > 0 {
            self.current_index -= 1;
            Some(&self.entries[self.current_index])
        } else {
            None
        }
    }

    /// Navigate forward one step.
    pub fn go_forward(&mut self) -> Option<&BreadcrumbEntry> {
        if self.current_index < self.entries.len() {
            let entry = &self.entries[self.current_index];
            self.current_index += 1;
            Some(entry)
        } else {
            None
        }
    }

    /// Can we go back?
    pub fn can_go_back(&self) -> bool {
        self.current_index > 0
    }

    /// Can we go forward?
    pub fn can_go_forward(&self) -> bool {
        self.current_index < self.entries.len()
    }

    /// Get current position (0-based).
    pub fn current_position(&self) -> usize {
        self.current_index
    }

    /// Total entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Render breadcrumb trail as a string like "file1:10 > file2:20 > file3:30".
    pub fn render(&self) -> String {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                if i + 1 == self.current_index {
                    format!("[{}]", e)
                } else {
                    format!("{}", e)
                }
            })
            .collect::<Vec<_>>()
            .join(" > ")
    }

    /// Clear the trail.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_index = 0;
    }

    /// Get all entries.
    pub fn entries(&self) -> &[BreadcrumbEntry] {
        &self.entries
    }
}

// ---------------------------------------------------------------------------
// ReferenceTypeIndicator – visual indicator for reference kinds
// ---------------------------------------------------------------------------

/// Visual indicator for different reference types.
#[derive(Debug, Clone)]
pub struct ReferenceTypeIndicator {
    kind: ReferenceKind,
}

impl ReferenceTypeIndicator {
    pub fn new(kind: ReferenceKind) -> Self {
        Self { kind }
    }

    /// Get the icon character for this reference kind.
    pub fn icon(&self) -> &'static str {
        match self.kind {
            ReferenceKind::Declaration => "◇",
            ReferenceKind::Definition => "◆",
            ReferenceKind::Read => "→",
            ReferenceKind::Write => "←",
            ReferenceKind::Call => "⊕",
            ReferenceKind::Import => "⬆",
            ReferenceKind::Other => "○",
        }
    }

    /// Get a short label for this reference kind.
    pub fn short_label(&self) -> &'static str {
        match self.kind {
            ReferenceKind::Declaration => "decl",
            ReferenceKind::Definition => "def",
            ReferenceKind::Read => "read",
            ReferenceKind::Write => "write",
            ReferenceKind::Call => "call",
            ReferenceKind::Import => "import",
            ReferenceKind::Other => "other",
        }
    }

    /// Format as "icon label", e.g. "◆ def".
    pub fn format(&self) -> String {
        format!("{} {}", self.icon(), self.short_label())
    }

    /// Get the kind.
    pub fn kind(&self) -> ReferenceKind {
        self.kind
    }

    /// Returns true if this is a write-type reference (Write or Definition).
    pub fn is_write_type(&self) -> bool {
        matches!(self.kind, ReferenceKind::Write | ReferenceKind::Definition)
    }

    /// Returns true if this is a read-type reference.
    pub fn is_read_type(&self) -> bool {
        matches!(self.kind, ReferenceKind::Read | ReferenceKind::Call | ReferenceKind::Import)
    }
}

impl fmt::Display for ReferenceTypeIndicator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

/// Classify and annotate a list of reference items with their kinds.
pub struct ReferenceTypeClassifier;

impl ReferenceTypeClassifier {
    /// Given a list of items and a parallel list of kinds, produce annotated display strings.
    pub fn annotate(items: &[ReferenceItem], kinds: &[ReferenceKind]) -> Vec<String> {
        items
            .iter()
            .zip(kinds.iter())
            .map(|(item, kind)| {
                let indicator = ReferenceTypeIndicator::new(*kind);
                format!("{} {}", indicator.format(), item)
            })
            .collect()
    }

    /// Count items by kind.
    pub fn count_by_kind(kinds: &[ReferenceKind]) -> HashMap<ReferenceKind, usize> {
        let mut counts = HashMap::new();
        for k in kinds {
            *counts.entry(*k).or_insert(0) += 1;
        }
        counts
    }

    /// Partition kinds into write-types and read-types.
    pub fn partition_by_access(kinds: &[ReferenceKind]) -> (Vec<ReferenceKind>, Vec<ReferenceKind>) {
        let mut writes = Vec::new();
        let mut reads = Vec::new();
        for k in kinds {
            let ind = ReferenceTypeIndicator::new(*k);
            if ind.is_write_type() {
                writes.push(*k);
            } else if ind.is_read_type() {
                reads.push(*k);
            }
        }
        (writes, reads)
    }

    /// Format a summary of kind counts.
    pub fn format_summary(kinds: &[ReferenceKind]) -> String {
        let counts = Self::count_by_kind(kinds);
        let mut parts: Vec<String> = counts
            .iter()
            .map(|(k, v)| {
                let ind = ReferenceTypeIndicator::new(*k);
                format!("{}: {}", ind.short_label(), v)
            })
            .collect();
        parts.sort();
        parts.join(", ")
    }
}


/// References view configuration manager.
#[derive(Debug, Clone)]
pub struct RefsViewConfig {
    entries: Vec<RefsViewEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single references view entry.
#[derive(Debug, Clone, PartialEq)]
pub struct RefsViewEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl RefsViewEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl RefsViewConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: RefsViewEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&RefsViewEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut RefsViewEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&RefsViewEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&RefsViewEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&RefsViewEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<RefsViewEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
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
// xa_ extended helpers for refs_view
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaRefsViewRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaRefsViewRingBuf {
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
pub struct XaRefsViewCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaRefsViewCounter {
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

impl Default for XaRefsViewCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 146
// ---------------------------------------------------------------------------

/// Generic object pool `Xc146Pool<T>`.
pub struct Xc146Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc146Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc146PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc146Pool<T> {
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
    pub fn stats(&self) -> Xc146PoolStats {
        Xc146PoolStats {
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

impl<T> Default for Xc146Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc146Scheduler`.
pub struct Xc146Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc146Scheduler {
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

impl Default for Xc146Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_146 hash for the given byte slice.
pub fn xc_146_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_146 convention.
pub fn xc_146_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_59 deepening: state machine + event bus ---

/// States for the Xd59 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd59State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd59State {
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
pub struct Xd59Transition {
    pub from: Xd59State,
    pub to: Xd59State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd59StateMachine {
    current: Xd59State,
    history: Vec<Xd59Transition>,
    step_counter: usize,
}

impl Xd59StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd59State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd59State {
        self.current
    }

    pub fn history(&self) -> &[Xd59Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd59State) -> Result<Xd59State, String> {
        let allowed = match (self.current, target) {
            (Xd59State::Idle, Xd59State::Running) => true,
            (Xd59State::Running, Xd59State::Paused) => true,
            (Xd59State::Running, Xd59State::Done) => true,
            (Xd59State::Paused, Xd59State::Running) => true,
            (Xd59State::Paused, Xd59State::Done) => true,
            (Xd59State::Done, Xd59State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_59: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd59Transition {
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
            "Xd59SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd59State> {
        let prefix = "Xd59SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd59State::Idle),
            "Running" => Some(Xd59State::Running),
            "Paused" => Some(Xd59State::Paused),
            "Done" => Some(Xd59State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd59State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd59 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd59Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd59Event {
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

type Xd59HandlerFn = Box<dyn Fn(&Xd59Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd59EventBus {
    handlers: Vec<(usize, Option<String>, Xd59HandlerFn)>,
    next_id: usize,
    published: Vec<Xd59Event>,
}

impl Xd59EventBus {
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
        F: Fn(&Xd59Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd59Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd59Event) {
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

    pub fn published_events(&self) -> &[Xd59Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #57
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf57Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf57TrieNode {
    children: std::collections::HashMap<char, Xf57TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf57Trie {
    root: Xf57TrieNode,
    count: usize,
}

impl Xf57Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf57TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf57TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf57TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf57BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf57BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 145).
pub struct Xh145SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh145SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 187 as u64,
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

/// A compact bit set supporting boolean operations (variant 145).
pub struct Xh145BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh145BitSet {
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

    fn kinded(uri: &str, line: u32, col: u32, kind: ReferenceKind) -> KindedReferenceItem {
        KindedReferenceItem {
            item: ref_item(uri, line, col),
            kind,
        }
    }

    #[test]
    fn grouper_group_by_file() {
        let refs = vec![
            kinded("a.rs", 1, 0, ReferenceKind::Read),
            kinded("b.rs", 2, 0, ReferenceKind::Write),
            kinded("a.rs", 5, 0, ReferenceKind::Call),
        ];
        let groups = ReferencesGrouper::group_by_file(&refs);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "a.rs");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, "b.rs");
        assert_eq!(groups[1].1.len(), 1);
    }

    #[test]
    fn grouper_group_by_kind() {
        let refs = vec![
            kinded("a.rs", 1, 0, ReferenceKind::Read),
            kinded("b.rs", 2, 0, ReferenceKind::Read),
            kinded("a.rs", 3, 0, ReferenceKind::Write),
        ];
        let groups = ReferencesGrouper::group_by_kind(&refs);
        assert_eq!(groups.len(), 2);
        let read_group = groups.iter().find(|(k, _)| *k == ReferenceKind::Read).unwrap();
        assert_eq!(read_group.1.len(), 2);
    }

    #[test]
    fn grouper_file_count() {
        let refs = vec![
            kinded("a.rs", 1, 0, ReferenceKind::Read),
            kinded("b.rs", 2, 0, ReferenceKind::Read),
            kinded("a.rs", 3, 0, ReferenceKind::Write),
        ];
        assert_eq!(ReferencesGrouper::file_count(&refs), 2);
    }

    #[test]
    fn filter_include_kind() {
        let refs = vec![
            kinded("a.rs", 1, 0, ReferenceKind::Read),
            kinded("b.rs", 2, 0, ReferenceKind::Write),
            kinded("c.rs", 3, 0, ReferenceKind::Call),
        ];
        let mut filter = ReferencesFilter::new();
        assert!(filter.is_empty());
        filter.include_kind(ReferenceKind::Read);
        let result = filter.apply(&refs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].kind, ReferenceKind::Read);
    }

    #[test]
    fn filter_exclude_kind() {
        let refs = vec![
            kinded("a.rs", 1, 0, ReferenceKind::Read),
            kinded("b.rs", 2, 0, ReferenceKind::Write),
            kinded("c.rs", 3, 0, ReferenceKind::Call),
        ];
        let mut filter = ReferencesFilter::new();
        filter.exclude_kind(ReferenceKind::Write);
        let result = filter.apply(&refs);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|r| r.kind != ReferenceKind::Write));
    }

    #[test]
    fn filter_include_and_exclude() {
        let refs = vec![
            kinded("a.rs", 1, 0, ReferenceKind::Read),
            kinded("b.rs", 2, 0, ReferenceKind::Write),
            kinded("c.rs", 3, 0, ReferenceKind::Call),
        ];
        let mut filter = ReferencesFilter::new();
        filter.include_kind(ReferenceKind::Read);
        filter.include_kind(ReferenceKind::Write);
        filter.exclude_kind(ReferenceKind::Write);
        let result = filter.apply(&refs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].kind, ReferenceKind::Read);
    }

    #[test]
    fn peek_preview_generate() {
        let content = "line0\nline1\nline2\nline3\nline4\nline5";
        let preview = ReferencePeekPreview::new(1);
        let result = preview.generate(content, 2);
        assert_eq!(result, "line1\nline2\nline3");
    }

    #[test]
    fn peek_preview_at_start() {
        let content = "first\nsecond\nthird";
        let preview = ReferencePeekPreview::new(2);
        let result = preview.generate(content, 0);
        assert_eq!(result, "first\nsecond\nthird");
    }

    #[test]
    fn peek_preview_highlight() {
        let content = "aaa\nbbb\nccc\nddd\neee";
        let preview = ReferencePeekPreview::new(1);
        let lines = preview.generate_with_highlight(content, 2);
        assert_eq!(lines.len(), 3);
        assert!(!lines[0].is_target);
        assert!(lines[1].is_target);
        assert_eq!(lines[1].text, "ccc");
        assert_eq!(lines[1].line_number, 3);
        assert!(!lines[2].is_target);
    }

    #[test]
    fn peek_preview_out_of_bounds() {
        let content = "only";
        let preview = ReferencePeekPreview::new(2);
        let result = preview.generate(content, 99);
        assert!(result.is_empty());
        let lines = preview.generate_with_highlight(content, 99);
        assert!(lines.is_empty());
    }

    #[test]
    fn navigation_history_basic() {
        let mut history = ReferenceNavigationHistory::new();
        assert_eq!(history.visited_count(), 0);
        history.visit("a.rs", 10);
        history.visit("b.rs", 20);
        assert_eq!(history.visited_count(), 2);
        assert!(history.is_visited("a.rs", 10));
        assert!(!history.is_visited("a.rs", 11));
    }

    #[test]
    fn navigation_history_dedup() {
        let mut history = ReferenceNavigationHistory::new();
        history.visit("a.rs", 10);
        history.visit("a.rs", 10);
        assert_eq!(history.visited_count(), 1);
    }

    #[test]
    fn navigation_history_recent_and_clear() {
        let mut history = ReferenceNavigationHistory::new();
        history.visit("a.rs", 1);
        history.visit("b.rs", 2);
        history.visit("c.rs", 3);
        let recent = history.recent_visits(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0], ("c.rs".to_string(), 3));
        assert_eq!(recent[1], ("b.rs".to_string(), 2));
        history.clear();
        assert_eq!(history.visited_count(), 0);
        assert!(history.recent_visits(5).is_empty());
    }
    #[test]
    fn reference_count_badge_basic() {
        let base = loc("file.rs", 1, 0);
        let mut model = ReferencesModel::new("foo", base);
        model.add_reference(ref_item("a.rs", 10, 5));
        model.add_reference(ref_item("b.rs", 20, 3));
        let badge = ReferenceCountBadge::from_model("foo", &model);
        assert_eq!(badge.total_refs, 2);
        assert_eq!(badge.file_count, 2);
        assert!(badge.has_references());
    }

    #[test]
    fn reference_count_badge_format() {
        let base = loc("file.rs", 1, 0);
        let mut model = ReferencesModel::new("bar", base);
        model.add_reference(ref_item("a.rs", 10, 5));
        let badge = ReferenceCountBadge::from_model("bar", &model);
        assert_eq!(badge.format_short(), "bar (1 ref in 1 file)");
    }

    #[test]
    fn reference_count_badge_kinds() {
        let base = loc("file.rs", 1, 0);
        let model = ReferencesModel::new("x", base);
        let mut badge = ReferenceCountBadge::from_model("x", &model);
        badge.add_kind(ReferenceKind::Read, 3);
        badge.add_kind(ReferenceKind::Write, 1);
        assert_eq!(badge.kind_count(&ReferenceKind::Read), 3);
        assert_eq!(badge.kind_count(&ReferenceKind::Call), 0);
        assert_eq!(badge.tracked_kind_total(), 4);
    }

    #[test]
    fn reference_count_badge_detailed() {
        let base = loc("f.rs", 1, 0);
        let model = ReferencesModel::new("z", base);
        let mut badge = ReferenceCountBadge::from_model("z", &model);
        badge.add_kind(ReferenceKind::Read, 2);
        let detail = badge.format_detailed();
        assert!(detail.contains("Read: 2"));
    }

    #[test]
    fn inline_preview_basic() {
        let location = loc("test.rs", 10, 5);
        let preview = ReferenceInlinePreview::new(location, "let x = 42;".into(), 5);
        assert_eq!(preview.highlighted_line(), "let x = 42;");
        assert_eq!(preview.line_count(), 1);
        assert!(!preview.is_truncated());
    }

    #[test]
    fn inline_preview_from_item() {
        let mut item = ref_item("test.rs", 10, 5);
        item.context_before = Some("// before".into());
        item.context_after = Some("// after".into());
        let preview = ReferenceInlinePreview::from_reference_item(&item, 10);
        assert_eq!(preview.line_count(), 3);
        assert_eq!(preview.highlight_line_index, 1);
    }

    #[test]
    fn inline_preview_render() {
        let location = loc("test.rs", 10, 5);
        let preview = ReferenceInlinePreview::new(location, "let x = 42;".into(), 5);
        let rendered = preview.render();
        assert!(rendered.contains("test.rs:10"));
        assert!(rendered.contains("let x = 42;"));
    }

    #[test]
    fn inline_preview_header() {
        let location = loc("main.rs", 42, 0);
        let preview = ReferenceInlinePreview::new(location, "fn main()".into(), 5);
        assert_eq!(preview.format_header(), "main.rs:42");
    }

    #[test]
    fn breadcrumb_navigation() {
        let mut crumb = ReferenceNavigationBreadcrumb::new(10);
        assert!(crumb.is_empty());
        crumb.push(BreadcrumbEntry::new("main.rs", "file:///main.rs", 10));
        crumb.push(BreadcrumbEntry::new("lib.rs", "file:///lib.rs", 20));
        assert_eq!(crumb.len(), 2);
        assert!(crumb.can_go_back());
        let back = crumb.go_back().unwrap();
        assert_eq!(back.label, "lib.rs");
    }

    #[test]
    fn breadcrumb_forward() {
        let mut crumb = ReferenceNavigationBreadcrumb::new(10);
        crumb.push(BreadcrumbEntry::new("a", "a", 1));
        crumb.push(BreadcrumbEntry::new("b", "b", 2));
        crumb.go_back();
        assert!(crumb.can_go_forward());
        let fwd = crumb.go_forward().unwrap();
        assert_eq!(fwd.label, "b");
    }

    #[test]
    fn breadcrumb_truncate_on_push() {
        let mut crumb = ReferenceNavigationBreadcrumb::new(10);
        crumb.push(BreadcrumbEntry::new("a", "a", 1));
        crumb.push(BreadcrumbEntry::new("b", "b", 2));
        crumb.go_back();
        crumb.push(BreadcrumbEntry::new("c", "c", 3));
        assert_eq!(crumb.len(), 2);
    }

    #[test]
    fn breadcrumb_max_entries() {
        let mut crumb = ReferenceNavigationBreadcrumb::new(3);
        for i in 0..5 {
            crumb.push(BreadcrumbEntry::new(&format!("f{i}"), &format!("f{i}"), i));
        }
        assert_eq!(crumb.len(), 3);
    }

    #[test]
    fn breadcrumb_render() {
        let mut crumb = ReferenceNavigationBreadcrumb::new(10);
        crumb.push(BreadcrumbEntry::new("a", "a", 1));
        crumb.push(BreadcrumbEntry::new("b", "b", 2));
        let rendered = crumb.render();
        assert!(rendered.contains("a:1"));
        assert!(rendered.contains("b:2"));
    }

    #[test]
    fn breadcrumb_clear() {
        let mut crumb = ReferenceNavigationBreadcrumb::new(10);
        crumb.push(BreadcrumbEntry::new("a", "a", 1));
        crumb.clear();
        assert!(crumb.is_empty());
        assert!(!crumb.can_go_back());
    }

    #[test]
    fn type_indicator_icons() {
        let decl = ReferenceTypeIndicator::new(ReferenceKind::Declaration);
        assert_eq!(decl.icon(), "◇");
        assert_eq!(decl.short_label(), "decl");
        assert!(!decl.is_write_type());
        assert!(!decl.is_read_type());

        let def = ReferenceTypeIndicator::new(ReferenceKind::Definition);
        assert!(def.is_write_type());

        let read = ReferenceTypeIndicator::new(ReferenceKind::Read);
        assert!(read.is_read_type());
    }

    #[test]
    fn type_indicator_format() {
        let ind = ReferenceTypeIndicator::new(ReferenceKind::Call);
        assert_eq!(ind.format(), "⊕ call");
        assert_eq!(ind.to_string(), "⊕ call");
    }

    #[test]
    fn type_classifier_count_by_kind() {
        let kinds = vec![
            ReferenceKind::Read, ReferenceKind::Read,
            ReferenceKind::Write, ReferenceKind::Call,
        ];
        let counts = ReferenceTypeClassifier::count_by_kind(&kinds);
        assert_eq!(counts[&ReferenceKind::Read], 2);
        assert_eq!(counts[&ReferenceKind::Write], 1);
        assert_eq!(counts[&ReferenceKind::Call], 1);
    }

    #[test]
    fn type_classifier_partition() {
        let kinds = vec![
            ReferenceKind::Read, ReferenceKind::Write,
            ReferenceKind::Definition, ReferenceKind::Call,
        ];
        let (writes, reads) = ReferenceTypeClassifier::partition_by_access(&kinds);
        assert_eq!(writes.len(), 2);
        assert_eq!(reads.len(), 2);
    }

    #[test]
    fn type_classifier_annotate() {
        let items = vec![ref_item("a.rs", 1, 0)];
        let kinds = vec![ReferenceKind::Read];
        let annotated = ReferenceTypeClassifier::annotate(&items, &kinds);
        assert_eq!(annotated.len(), 1);
        assert!(annotated[0].contains("→ read"));
    }

    #[test]
    fn type_classifier_format_summary() {
        let kinds = vec![ReferenceKind::Read, ReferenceKind::Read, ReferenceKind::Write];
        let summary = ReferenceTypeClassifier::format_summary(&kinds);
        assert!(summary.contains("read: 2"));
        assert!(summary.contains("write: 1"));
    }


    #[test]
    fn refs_view_entry_creation() {
        let e = RefsViewEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn refs_view_entry_with_priority() {
        let e = RefsViewEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn refs_view_entry_metadata() {
        let e = RefsViewEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn refs_view_entry_remove_meta() {
        let mut e = RefsViewEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn refs_view_entry_activate_deactivate() {
        let mut e = RefsViewEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn refs_view_config_add_sorted() {
        let mut c = RefsViewConfig::new(10);
        c.add(RefsViewEntry::new("lo", "Lo").with_priority(1));
        c.add(RefsViewEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn refs_view_config_capacity() {
        let mut c = RefsViewConfig::new(1);
        assert!(c.add(RefsViewEntry::new("a", "A")));
        assert!(!c.add(RefsViewEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn refs_view_config_remove() {
        let mut c = RefsViewConfig::new(10);
        c.add(RefsViewEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn refs_view_config_get() {
        let mut c = RefsViewConfig::new(10);
        c.add(RefsViewEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn refs_view_config_active_entries() {
        let mut c = RefsViewConfig::new(10);
        c.add(RefsViewEntry::new("a", "A"));
        c.add(RefsViewEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn refs_view_config_enable_disable() {
        let mut c = RefsViewConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn refs_view_config_clear() {
        let mut c = RefsViewConfig::new(10);
        c.add(RefsViewEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn refs_view_config_find_by_label() {
        let mut c = RefsViewConfig::new(10);
        c.add(RefsViewEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn refs_view_config_top_n() {
        let mut c = RefsViewConfig::new(10);
        c.add(RefsViewEntry::new("a", "A").with_priority(1));
        c.add(RefsViewEntry::new("b", "B").with_priority(2));
        c.add(RefsViewEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn refs_view_config_deactivate_activate_all() {
        let mut c = RefsViewConfig::new(10);
        c.add(RefsViewEntry::new("a", "A"));
        c.add(RefsViewEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn refs_view_config_highest_priority() {
        let mut c = RefsViewConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(RefsViewEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn refs_view_config_contains() {
        let mut c = RefsViewConfig::new(10);
        c.add(RefsViewEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn refs_view_config_labels() {
        let mut c = RefsViewConfig::new(10);
        c.add(RefsViewEntry::new("a", "Alpha"));
        c.add(RefsViewEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn refs_view_config_drain_inactive() {
        let mut c = RefsViewConfig::new(10);
        c.add(RefsViewEntry::new("a", "A"));
        c.add(RefsViewEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
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


    // xa_ extended tests for refs_view
    #[test]
    fn xa_refs_view_ring_new() {
        let rb = super::XaRefsViewRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_refs_view_ring_push_len() {
        let mut rb = super::XaRefsViewRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_refs_view_ring_wrap() {
        let mut rb = super::XaRefsViewRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_refs_view_ring_mean_empty() {
        let rb = super::XaRefsViewRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_refs_view_ring_mean_values() {
        let mut rb = super::XaRefsViewRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_refs_view_ring_min_max() {
        let mut rb = super::XaRefsViewRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_refs_view_ring_iter() {
        let mut rb = super::XaRefsViewRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_refs_view_counter_new() {
        let c = super::XaRefsViewCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_refs_view_counter_inc() {
        let mut c = super::XaRefsViewCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_refs_view_counter_inc_by() {
        let mut c = super::XaRefsViewCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_refs_view_counter_reset() {
        let mut c = super::XaRefsViewCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_refs_view_counter_clear() {
        let mut c = super::XaRefsViewCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_refs_view_counter_default() {
        let c = super::XaRefsViewCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 146 ----

    #[test]
    fn xc_146_pool_new_empty() {
        let pool: super::Xc146Pool<i32> = super::Xc146Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_146_pool_release_acquire() {
        let mut pool = super::Xc146Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_146_pool_acquire_empty() {
        let mut pool: super::Xc146Pool<i32> = super::Xc146Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_146_pool_full() {
        let mut pool = super::Xc146Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_146_pool_drain() {
        let mut pool = super::Xc146Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_146_pool_stats() {
        let mut pool = super::Xc146Pool::new(8);
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
    fn xc_146_pool_clear() {
        let mut pool = super::Xc146Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_146_pool_shrink() {
        let mut pool = super::Xc146Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_146_pool_default() {
        let pool: super::Xc146Pool<String> = super::Xc146Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_146_pool_extend() {
        let mut pool = super::Xc146Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_146_pool_retain() {
        let mut pool = super::Xc146Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_146_scheduler_round_robin() {
        let mut sched = super::Xc146Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_146_scheduler_empty() {
        let mut sched = super::Xc146Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_146_scheduler_reset() {
        let mut sched = super::Xc146Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_146_scheduler_add_remove() {
        let mut sched = super::Xc146Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_146_scheduler_targets() {
        let sched = super::Xc146Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_146_hash_empty() {
        assert_eq!(super::xc_146_hash(b""), 5381);
    }

    #[test]
    fn xc_146_hash_data() {
        let h = super::xc_146_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_146_hash(b"hello"), h);
    }

    #[test]
    fn xc_146_reverse_str() {
        assert_eq!(super::xc_146_reverse("abc"), "cba");
        assert_eq!(super::xc_146_reverse(""), "");
    }


    // --- xd_59 deepening tests ---

    #[test]
    fn xd_59_sm_initial_state() {
        let sm = Xd59StateMachine::new();
        assert_eq!(sm.current_state(), Xd59State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_59_sm_valid_idle_to_running() {
        let mut sm = Xd59StateMachine::new();
        assert!(sm.transition(Xd59State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd59State::Running);
    }

    #[test]
    fn xd_59_sm_valid_running_to_paused() {
        let mut sm = Xd59StateMachine::new();
        sm.transition(Xd59State::Running).unwrap();
        assert!(sm.transition(Xd59State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd59State::Paused);
    }

    #[test]
    fn xd_59_sm_valid_running_to_done() {
        let mut sm = Xd59StateMachine::new();
        sm.transition(Xd59State::Running).unwrap();
        assert!(sm.transition(Xd59State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd59State::Done);
    }

    #[test]
    fn xd_59_sm_valid_paused_to_running() {
        let mut sm = Xd59StateMachine::new();
        sm.transition(Xd59State::Running).unwrap();
        sm.transition(Xd59State::Paused).unwrap();
        assert!(sm.transition(Xd59State::Running).is_ok());
    }

    #[test]
    fn xd_59_sm_valid_done_to_idle() {
        let mut sm = Xd59StateMachine::new();
        sm.transition(Xd59State::Running).unwrap();
        sm.transition(Xd59State::Done).unwrap();
        assert!(sm.transition(Xd59State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd59State::Idle);
    }

    #[test]
    fn xd_59_sm_invalid_idle_to_done() {
        let mut sm = Xd59StateMachine::new();
        assert!(sm.transition(Xd59State::Done).is_err());
    }

    #[test]
    fn xd_59_sm_invalid_idle_to_paused() {
        let mut sm = Xd59StateMachine::new();
        assert!(sm.transition(Xd59State::Paused).is_err());
    }

    #[test]
    fn xd_59_sm_history_tracking() {
        let mut sm = Xd59StateMachine::new();
        sm.transition(Xd59State::Running).unwrap();
        sm.transition(Xd59State::Paused).unwrap();
        sm.transition(Xd59State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd59State::Idle);
        assert_eq!(sm.history()[0].to, Xd59State::Running);
        assert_eq!(sm.history()[1].from, Xd59State::Running);
        assert_eq!(sm.history()[2].to, Xd59State::Done);
    }

    #[test]
    fn xd_59_sm_serialize_deserialize() {
        let mut sm = Xd59StateMachine::new();
        sm.transition(Xd59State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd59StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd59State::Running));
    }

    #[test]
    fn xd_59_sm_deserialize_invalid() {
        assert_eq!(Xd59StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_59_sm_reset() {
        let mut sm = Xd59StateMachine::new();
        sm.transition(Xd59State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd59State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_59_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd59EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd59Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_59_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd59EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd59Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd59Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_59_bus_unsubscribe() {
        let mut bus = Xd59EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_59_event_kind_and_payload() {
        let e = Xd59Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd59Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_59_bus_clear_history() {
        let mut bus = Xd59EventBus::new();
        bus.publish(Xd59Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_59_sm_step_counter_increments() {
        let mut sm = Xd59StateMachine::new();
        sm.transition(Xd59State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd59State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #57 --

    #[test]
    fn xf57_trie_insert_search() {
        let mut t = Xf57Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf57_trie_starts_with() {
        let mut t = Xf57Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf57_trie_remove() {
        let mut t = Xf57Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf57_trie_word_count() {
        let mut t = Xf57Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf57_trie_longest_prefix() {
        let mut t = Xf57Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf57_trie_all_words() {
        let mut t = Xf57Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf57_trie_autocomplete() {
        let mut t = Xf57Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf57_trie_empty_search() {
        let t = Xf57Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf57_bloom_add_contains() {
        let mut bf = Xf57BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf57_bloom_probably_absent() {
        let bf = Xf57BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf57_bloom_false_positive_rate() {
        let mut bf = Xf57BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf57_bloom_clear() {
        let mut bf = Xf57BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf57_bloom_union() {
        let mut a = Xf57BloomFilter::xf_new(512, 2);
        let mut b = Xf57BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf57_bloom_intersection_estimate() {
        let mut a = Xf57BloomFilter::xf_new(512, 2);
        let mut b = Xf57BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf57_bloom_union_size_mismatch() {
        let a = Xf57BloomFilter::xf_new(256, 2);
        let b = Xf57BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh145_skip_insert_contains() {
        let mut sl = super::Xh145SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh145_skip_remove() {
        let mut sl = super::Xh145SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh145_skip_len() {
        let mut sl = super::Xh145SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh145_skip_range_query() {
        let mut sl = super::Xh145SkipList::xh_new(4);
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
    fn xh145_skip_floor_ceiling() {
        let mut sl = super::Xh145SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh145_skip_rank() {
        let mut sl = super::Xh145SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh145_skip_empty() {
        let sl = super::Xh145SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh145_skip_duplicates() {
        let mut sl = super::Xh145SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh145_bitset_set_test() {
        let mut bs = super::Xh145BitSet::xh_new(256);
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
    fn xh145_bitset_clear_count() {
        let mut bs = super::Xh145BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh145_bitset_and_or_xor() {
        let mut a = super::Xh145BitSet::xh_new(128);
        let mut b = super::Xh145BitSet::xh_new(128);
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
    fn xh145_bitset_iter_ones() {
        let mut bs = super::Xh145BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh145_bitset_first_last() {
        let mut bs = super::Xh145BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh145_bitset_empty() {
        let bs = super::Xh145BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
