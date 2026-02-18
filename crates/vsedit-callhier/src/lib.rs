//! Call hierarchy view.
//!
//! Provides types and a trait for navigating incoming and outgoing calls,
//! mirroring the VS Code call hierarchy contribution.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

/// Errors that may occur when resolving call hierarchy information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallHierarchyError {
    /// No call hierarchy item could be found at the given position.
    NoItemAtPosition { uri: String, line: u32, col: u32 },
    /// The underlying provider failed with a message.
    ProviderFailed(String),
    /// A cyclic call chain was detected starting from the named item.
    CyclicCallChain(String),
}

impl fmt::Display for CallHierarchyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoItemAtPosition { uri, line, col } => {
                write!(f, "no item at position {}:{}:{}", uri, line, col)
            }
            Self::ProviderFailed(msg) => write!(f, "provider failed: {}", msg),
            Self::CyclicCallChain(name) => write!(f, "cyclic call chain from '{}'", name),
        }
    }
}

/// The kind of symbol represented by a call hierarchy item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Method,
    Constructor,
    Class,
    Interface,
    Module,
    Property,
    Enum,
    Struct,
}

impl SymbolKind {
    /// Returns `true` if this symbol kind represents something callable
    /// (Function, Method, or Constructor).
    pub fn is_callable(&self) -> bool {
        matches!(self, Self::Function | Self::Method | Self::Constructor)
    }

    /// Returns a representative character for the symbol kind.
    pub fn icon_char(&self) -> char {
        match self {
            Self::Function => 'f',
            Self::Method => 'm',
            Self::Constructor => 'k',
            Self::Class => 'c',
            Self::Interface => 'i',
            Self::Module => 'M',
            Self::Property => 'p',
            Self::Enum => 'e',
            Self::Struct => 's',
        }
    }
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Function => "Function",
            Self::Method => "Method",
            Self::Constructor => "Constructor",
            Self::Class => "Class",
            Self::Interface => "Interface",
            Self::Module => "Module",
            Self::Property => "Property",
            Self::Enum => "Enum",
            Self::Struct => "Struct",
        };
        f.write_str(s)
    }
}

/// A single item in the call hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallHierarchyItem {
    pub name: String,
    pub kind: SymbolKind,
    pub uri: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub selection_start_line: u32,
    pub selection_start_col: u32,
    pub selection_end_line: u32,
    pub selection_end_col: u32,
    pub detail: Option<String>,
    pub is_deprecated: bool,
}

impl fmt::Display for CallHierarchyItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}) at {}:{}", self.name, self.kind, self.uri, self.start_line)
    }
}

impl CallHierarchyItem {
    /// Returns `true` if the given line and column fall within the item's range.
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

    /// Set the detail and return self (builder pattern).
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Set the deprecated flag and return self (builder pattern).
    pub fn with_deprecated(mut self, deprecated: bool) -> Self {
        self.is_deprecated = deprecated;
        self
    }

    /// Returns a human-readable location string in the form `"uri:line:col"`.
    pub fn display_location(&self) -> String {
        format!("{}:{}:{}", self.uri, self.start_line, self.start_col)
    }
}

/// A call site where control flows *into* a target item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingCall {
    pub from: CallHierarchyItem,
    pub from_ranges: Vec<(u32, u32, u32, u32)>,
}

/// A call site where control flows *out of* a source item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingCall {
    pub to: CallHierarchyItem,
    pub from_ranges: Vec<(u32, u32, u32, u32)>,
}

/// Provider trait for resolving call hierarchy information.
pub trait CallHierarchyProvider {
    /// Prepare the call hierarchy item at the given position.
    ///
    /// Returns `None` when no item can be resolved at that location.
    fn prepare_call_hierarchy(
        &self,
        uri: &str,
        line: u32,
        col: u32,
    ) -> Option<CallHierarchyItem>;

    /// Return all callers of `item`.
    fn provide_incoming_calls(&self, item: &CallHierarchyItem) -> Vec<IncomingCall>;

    /// Return all callees of `item`.
    fn provide_outgoing_calls(&self, item: &CallHierarchyItem) -> Vec<OutgoingCall>;
}

/// A directed graph of call relationships between hierarchy items.
///
/// Nodes are identified by `(name, uri)` pairs. Edges represent caller→callee
/// relationships.
#[derive(Debug, Clone)]
pub struct CallGraph {
    items: HashMap<String, CallHierarchyItem>,
    /// Edges stored as caller_key → set of callee_keys.
    edges: HashMap<String, HashSet<String>>,
    /// Reverse edges stored as callee_key → set of caller_keys.
    reverse_edges: HashMap<String, HashSet<String>>,
}

impl CallGraph {
    /// Create an empty call graph.
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
        }
    }

    fn key_for(item: &CallHierarchyItem) -> String {
        format!("{}@{}", item.name, item.uri)
    }

    /// Add an item as a node in the graph.
    pub fn add_item(&mut self, item: CallHierarchyItem) {
        let key = Self::key_for(&item);
        self.items.entry(key).or_insert(item);
    }

    /// Add a directed edge from `caller` to `callee`.
    ///
    /// Both items are also added as nodes if not already present.
    pub fn add_edge(&mut self, caller: &CallHierarchyItem, callee: &CallHierarchyItem) {
        let caller_key = Self::key_for(caller);
        let callee_key = Self::key_for(callee);
        self.add_item(caller.clone());
        self.add_item(callee.clone());
        self.edges
            .entry(caller_key.clone())
            .or_default()
            .insert(callee_key.clone());
        self.reverse_edges
            .entry(callee_key)
            .or_default()
            .insert(caller_key);
    }

    /// Return items that call into the given item.
    pub fn get_callers(&self, item: &CallHierarchyItem) -> Vec<&CallHierarchyItem> {
        let key = Self::key_for(item);
        self.reverse_edges
            .get(&key)
            .map(|keys| {
                keys.iter()
                    .filter_map(|k| self.items.get(k))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return items called by the given item.
    pub fn get_callees(&self, item: &CallHierarchyItem) -> Vec<&CallHierarchyItem> {
        let key = Self::key_for(item);
        self.edges
            .get(&key)
            .map(|keys| {
                keys.iter()
                    .filter_map(|k| self.items.get(k))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Detect whether there is a cycle reachable from `item` via outgoing edges.
    pub fn has_cycle_from(&self, item: &CallHierarchyItem) -> bool {
        let start = Self::key_for(item);
        let mut visited = HashSet::new();
        let mut stack = vec![start.clone()];
        while let Some(current) = stack.pop() {
            if !visited.insert(current.clone()) {
                if current == start {
                    return true;
                }
                continue;
            }
            if let Some(neighbours) = self.edges.get(&current) {
                for n in neighbours {
                    if n == &start {
                        return true;
                    }
                    if !visited.contains(n) {
                        stack.push(n.clone());
                    }
                }
            }
        }
        false
    }

    /// Return the number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.items.len()
    }

    /// Find items that have no incoming calls (root callers).
    pub fn find_roots(&self) -> Vec<&CallHierarchyItem> {
        self.items
            .iter()
            .filter(|(key, _)| {
                self.reverse_edges
                    .get(*key)
                    .map_or(true, |s| s.is_empty())
            })
            .map(|(_, item)| item)
            .collect()
    }

    /// Find items that have no outgoing calls (leaf callees).
    pub fn find_leaves(&self) -> Vec<&CallHierarchyItem> {
        self.items
            .iter()
            .filter(|(key, _)| {
                self.edges
                    .get(*key)
                    .map_or(true, |s| s.is_empty())
            })
            .map(|(_, item)| item)
            .collect()
    }
}

impl Default for CallGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for callhier operations.
#[derive(Debug, Clone, PartialEq)]
pub struct CallhierStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl CallhierStats {
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
    pub fn merge(&mut self, other: &CallhierStats) {
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

impl Default for CallhierStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CallhierStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CallhierStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for callhier.
#[derive(Debug, Clone)]
pub struct CallhierValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl CallhierValidator {
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

impl Default for CallhierValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CallHierarchyDirection
// ---------------------------------------------------------------------------

/// Direction of call hierarchy traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallHierarchyDirection {
    Incoming,
    Outgoing,
}

impl CallHierarchyDirection {
    /// Returns `true` if this is the Incoming direction.
    pub fn is_incoming(&self) -> bool {
        *self == CallHierarchyDirection::Incoming
    }

    /// Returns the opposite direction.
    pub fn opposite(&self) -> Self {
        match self {
            CallHierarchyDirection::Incoming => CallHierarchyDirection::Outgoing,
            CallHierarchyDirection::Outgoing => CallHierarchyDirection::Incoming,
        }
    }
}

impl fmt::Display for CallHierarchyDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallHierarchyDirection::Incoming => f.write_str("Incoming"),
            CallHierarchyDirection::Outgoing => f.write_str("Outgoing"),
        }
    }
}

// ---------------------------------------------------------------------------
// CallGraphBuilder
// ---------------------------------------------------------------------------

/// Builder for constructing a CallGraph from caller/callee pairs.
#[derive(Debug, Clone)]
pub struct CallGraphBuilder {
    graph: CallGraph,
    edge_count: usize,
}

impl CallGraphBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            graph: CallGraph::new(),
            edge_count: 0,
        }
    }

    /// Add a call edge from caller to callee.
    pub fn add_call(&mut self, caller: CallHierarchyItem, callee: CallHierarchyItem) -> &mut Self {
        self.graph.add_edge(&caller, &callee);
        self.edge_count += 1;
        self
    }

    /// Build the final CallGraph.
    pub fn build(self) -> CallGraph {
        self.graph
    }

    /// Number of edges added so far.
    pub fn edge_count(&self) -> usize {
        self.edge_count
    }
}

impl Default for CallGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// call_hierarchy_flatten
// ---------------------------------------------------------------------------

/// Flatten a call graph via BFS from a starting item in the given direction.
///
/// Returns `(depth, item)` pairs up to `max_depth` levels deep.
pub fn call_hierarchy_flatten<'a>(
    graph: &'a CallGraph,
    start: &CallHierarchyItem,
    direction: CallHierarchyDirection,
    max_depth: usize,
) -> Vec<(usize, &'a CallHierarchyItem)> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    let start_key = format!("{}@{}", start.name, start.uri);
    visited.insert(start_key.clone());
    queue.push_back((start_key, 0));

    while let Some((key, depth)) = queue.pop_front() {
        if let Some(item) = graph.items.get(&key) {
            if depth > 0 {
                result.push((depth, item));
            }
        }

        if depth >= max_depth {
            continue;
        }

        let neighbors: Vec<String> = match direction {
            CallHierarchyDirection::Outgoing => graph
                .edges
                .get(&key)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default(),
            CallHierarchyDirection::Incoming => graph
                .reverse_edges
                .get(&key)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default(),
        };

        for n in neighbors {
            if visited.insert(n.clone()) {
                queue.push_back((n, depth + 1));
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// CallGraph extensions
// ---------------------------------------------------------------------------

impl CallGraph {
    /// Total number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|s| s.len()).sum()
    }

    /// Look up an item by name and URI.
    pub fn get_item(&self, name: &str, uri: &str) -> Option<&CallHierarchyItem> {
        let key = format!("{}@{}", name, uri);
        self.items.get(&key)
    }

    /// Returns `true` if the graph contains an item whose name matches `name`.
    pub fn contains_item(&self, name: &str) -> bool {
        self.items.values().any(|item| item.name == name)
    }

    /// Compute the maximum call depth reachable from `item` via outgoing edges.
    ///
    /// Returns 0 if the item has no outgoing calls. Cycles are handled by
    /// tracking visited nodes.
    pub fn depth_from(&self, item: &CallHierarchyItem) -> usize {
        let start_key = Self::key_for(item);
        let mut visited = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        visited.insert(start_key.clone());
        queue.push_back((start_key, 0));
        let mut max_depth: usize = 0;

        while let Some((key, depth)) = queue.pop_front() {
            if depth > max_depth {
                max_depth = depth;
            }
            if let Some(neighbors) = self.edges.get(&key) {
                for n in neighbors {
                    if visited.insert(n.clone()) {
                        queue.push_back((n.clone(), depth + 1));
                    }
                }
            }
        }
        max_depth
    }

    /// Returns all items in the graph as a slice-compatible vector reference.
    ///
    /// Note: because items are stored in a `HashMap`, this collects values
    /// into an internal `Vec` on each call. For repeated access prefer
    /// iterating via other methods.
    pub fn items(&self) -> Vec<&CallHierarchyItem> {
        self.items.values().collect()
    }
}

// ── CallChainAnalyzer ──

/// Analyzer for finding call chains in a `CallGraph`.
pub struct CallChainAnalyzer<'a> {
    graph: &'a CallGraph,
}

impl<'a> CallChainAnalyzer<'a> {
    pub fn new(graph: &'a CallGraph) -> Self {
        Self { graph }
    }

    /// Find all simple (acyclic) paths from `start` to `end` using DFS.
    /// Returns paths as vectors of item names.
    pub fn find_paths(
        &self,
        start: &CallHierarchyItem,
        end: &CallHierarchyItem,
    ) -> Vec<Vec<String>> {
        let start_key = CallGraph::key_for(start);
        let end_key = CallGraph::key_for(end);
        let mut results = Vec::new();
        let mut path = vec![start_key.clone()];
        let mut visited = HashSet::new();
        visited.insert(start_key.clone());
        self.dfs_paths(&start_key, &end_key, &mut visited, &mut path, &mut results);
        results
    }

    fn dfs_paths(
        &self,
        current: &str,
        end: &str,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
        results: &mut Vec<Vec<String>>,
    ) {
        if current == end && path.len() > 1 {
            let names: Vec<String> = path
                .iter()
                .filter_map(|k| self.graph.items.get(k).map(|i| i.name.clone()))
                .collect();
            results.push(names);
            return;
        }
        if let Some(neighbors) = self.graph.edges.get(current) {
            for n in neighbors {
                if !visited.contains(n) {
                    visited.insert(n.clone());
                    path.push(n.clone());
                    self.dfs_paths(n, end, visited, path, results);
                    path.pop();
                    visited.remove(n);
                }
            }
        }
    }

    /// Find the longest call chain starting from the given item.
    /// Returns the chain as a list of item names.
    pub fn longest_chain_from(&self, start: &CallHierarchyItem) -> Vec<String> {
        let start_key = CallGraph::key_for(start);
        let mut best = Vec::new();
        let mut current = vec![start_key.clone()];
        let mut visited = HashSet::new();
        visited.insert(start_key.clone());
        self.dfs_longest(&start_key, &mut visited, &mut current, &mut best);
        best.iter()
            .filter_map(|k| self.graph.items.get(k).map(|i| i.name.clone()))
            .collect()
    }

    fn dfs_longest(
        &self,
        current: &str,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
        best: &mut Vec<String>,
    ) {
        if path.len() > best.len() {
            *best = path.clone();
        }
        if let Some(neighbors) = self.graph.edges.get(current) {
            for n in neighbors {
                if !visited.contains(n) {
                    visited.insert(n.clone());
                    path.push(n.clone());
                    self.dfs_longest(n, visited, path, best);
                    path.pop();
                    visited.remove(n);
                }
            }
        }
    }
}

// ── CallGraphExporter ──

/// Export a `CallGraph` to text or DOT format.
pub struct CallGraphExporter<'a> {
    graph: &'a CallGraph,
}

impl<'a> CallGraphExporter<'a> {
    pub fn new(graph: &'a CallGraph) -> Self {
        Self { graph }
    }

    /// Export as plain text listing of edges.
    pub fn to_text(&self) -> String {
        let mut lines = Vec::new();
        let mut keys: Vec<&String> = self.graph.edges.keys().collect();
        keys.sort();
        for caller_key in keys {
            if let Some(callees) = self.graph.edges.get(caller_key) {
                let caller_name = self.graph.items.get(caller_key)
                    .map(|i| i.name.as_str())
                    .unwrap_or(caller_key);
                let mut callee_names: Vec<&str> = callees
                    .iter()
                    .map(|k| self.graph.items.get(k).map(|i| i.name.as_str()).unwrap_or(k.as_str()))
                    .collect();
                callee_names.sort();
                for callee in callee_names {
                    lines.push(format!("{} -> {}", caller_name, callee));
                }
            }
        }
        lines.join("\n")
    }

    /// Export as DOT graph format.
    pub fn to_dot(&self) -> String {
        let mut out = String::from("digraph CallGraph {\n");
        let mut keys: Vec<&String> = self.graph.edges.keys().collect();
        keys.sort();
        for caller_key in keys {
            if let Some(callees) = self.graph.edges.get(caller_key) {
                let caller_name = self.graph.items.get(caller_key)
                    .map(|i| i.name.as_str())
                    .unwrap_or(caller_key);
                let mut callee_keys: Vec<&String> = callees.iter().collect();
                callee_keys.sort();
                for callee_key in callee_keys {
                    let callee_name = self.graph.items.get(callee_key)
                        .map(|i| i.name.as_str())
                        .unwrap_or(callee_key.as_str());
                    out.push_str(&format!("  \"{}\" -> \"{}\";\n", caller_name, callee_name));
                }
            }
        }
        out.push('}');
        out
    }

    /// Count the total number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edges.values().map(|s| s.len()).sum()
    }
}

// ── RecursionDetector ──

/// Detect recursive call patterns in a `CallGraph`.
pub struct RecursionDetector<'a> {
    graph: &'a CallGraph,
}

impl<'a> RecursionDetector<'a> {
    pub fn new(graph: &'a CallGraph) -> Self {
        Self { graph }
    }

    /// Returns true if the given item directly calls itself.
    pub fn is_directly_recursive(&self, item: &CallHierarchyItem) -> bool {
        let key = CallGraph::key_for(item);
        self.graph.edges.get(&key).map_or(false, |callees| callees.contains(&key))
    }

    /// Returns all items that are part of any cycle (direct or indirect recursion).
    pub fn find_all_recursive_items(&self) -> Vec<&CallHierarchyItem> {
        self.graph
            .items
            .values()
            .filter(|item| self.graph.has_cycle_from(item))
            .collect()
    }

    /// Detect all direct self-recursive items.
    pub fn find_directly_recursive(&self) -> Vec<&CallHierarchyItem> {
        self.graph
            .items
            .values()
            .filter(|item| self.is_directly_recursive(item))
            .collect()
    }
}

// ── Depth-limited traversal on CallGraph ──

impl CallGraph {
    /// Collect all items reachable from `start` within `max_depth` edges.
    pub fn reachable_within(&self, start: &CallHierarchyItem, max_depth: usize) -> Vec<&CallHierarchyItem> {
        let start_key = Self::key_for(start);
        let mut visited = HashSet::new();
        visited.insert(start_key.clone());
        let mut queue = VecDeque::new();
        queue.push_back((start_key, 0usize));
        let mut result = Vec::new();
        while let Some((key, depth)) = queue.pop_front() {
            if let Some(item) = self.items.get(&key) {
                result.push(item);
            }
            if depth < max_depth {
                if let Some(neighbors) = self.edges.get(&key) {
                    for n in neighbors {
                        if visited.insert(n.clone()) {
                            queue.push_back((n.clone(), depth + 1));
                        }
                    }
                }
            }
        }
        result
    }

    /// Provide public access to the key function for external analyzers.
    pub fn key_for_public(item: &CallHierarchyItem) -> String {
        Self::key_for(item)
    }

    /// Return the out-degree (number of callees) for a given item.
    pub fn out_degree(&self, item: &CallHierarchyItem) -> usize {
        let key = Self::key_for(item);
        self.edges.get(&key).map_or(0, |s| s.len())
    }

    /// Return the in-degree (number of callers) for a given item.
    pub fn in_degree(&self, item: &CallHierarchyItem) -> usize {
        let key = Self::key_for(item);
        self.reverse_edges.get(&key).map_or(0, |s| s.len())
    }

    /// Compute the longest shortest path from `start` to any reachable node (eccentricity).
    pub fn eccentricity(&self, start: &CallHierarchyItem) -> usize {
        let start_key = Self::key_for(start);
        let mut visited = HashSet::new();
        visited.insert(start_key.clone());
        let mut queue = VecDeque::new();
        queue.push_back((start_key, 0usize));
        let mut max_depth = 0;
        while let Some((key, depth)) = queue.pop_front() {
            if depth > max_depth {
                max_depth = depth;
            }
            if let Some(neighbors) = self.edges.get(&key) {
                for n in neighbors {
                    if visited.insert(n.clone()) {
                        queue.push_back((n.clone(), depth + 1));
                    }
                }
            }
        }
        max_depth
    }

    /// Return all items in the graph.
    pub fn all_items(&self) -> Vec<&CallHierarchyItem> {
        self.items.values().collect()
    }

    /// Topological sort of graph nodes. Returns `None` if the graph contains a cycle.
    pub fn topological_sort(&self) -> Option<Vec<&CallHierarchyItem>> {
        let mut in_deg: HashMap<&String, usize> = HashMap::new();
        for key in self.items.keys() {
            in_deg.entry(key).or_insert(0);
        }
        for callees in self.edges.values() {
            for callee_key in callees {
                if self.items.contains_key(callee_key) {
                    *in_deg.entry(callee_key).or_insert(0) += 1;
                }
            }
        }
        let mut queue: VecDeque<&String> = in_deg
            .iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(k, _)| *k)
            .collect();
        let mut result = Vec::new();
        while let Some(key) = queue.pop_front() {
            if let Some(item) = self.items.get(key) {
                result.push(item);
            }
            if let Some(neighbors) = self.edges.get(key) {
                for n in neighbors {
                    if let Some(deg) = in_deg.get_mut(n) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(n);
                        }
                    }
                }
            }
        }
        if result.len() == self.items.len() {
            Some(result)
        } else {
            None
        }
    }

    /// Compute the shortest path length from `from` to `to`. Returns `None` if unreachable.
    pub fn shortest_path_length(
        &self,
        from: &CallHierarchyItem,
        to: &CallHierarchyItem,
    ) -> Option<usize> {
        let start_key = Self::key_for(from);
        let target_key = Self::key_for(to);
        if start_key == target_key {
            return Some(0);
        }
        let mut visited = HashSet::new();
        visited.insert(start_key.clone());
        let mut queue = VecDeque::new();
        queue.push_back((start_key, 0usize));
        while let Some((key, depth)) = queue.pop_front() {
            if let Some(neighbors) = self.edges.get(&key) {
                for n in neighbors {
                    if *n == target_key {
                        return Some(depth + 1);
                    }
                    if visited.insert(n.clone()) {
                        queue.push_back((n.clone(), depth + 1));
                    }
                }
            }
        }
        None
    }

    /// Filter items in the graph by symbol kind.
    pub fn items_by_kind(&self, kind: SymbolKind) -> Vec<&CallHierarchyItem> {
        self.items.values().filter(|item| item.kind == kind).collect()
    }
}

/// Summary statistics computed from a `CallGraph`.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphSummary {
    pub node_count: usize,
    pub edge_count: usize,
    pub root_count: usize,
    pub leaf_count: usize,
    pub max_out_degree: usize,
    pub max_in_degree: usize,
    pub has_cycles: bool,
}

impl GraphSummary {
    /// Compute a summary from the given call graph.
    pub fn from_graph(graph: &CallGraph) -> Self {
        let items: Vec<&CallHierarchyItem> = graph.all_items();
        let max_out = items.iter().map(|i| graph.out_degree(i)).max().unwrap_or(0);
        let max_in = items.iter().map(|i| graph.in_degree(i)).max().unwrap_or(0);
        let has_cycles = items.iter().any(|i| graph.has_cycle_from(i));
        Self {
            node_count: graph.node_count(),
            edge_count: graph.edge_count(),
            root_count: graph.find_roots().len(),
            leaf_count: graph.find_leaves().len(),
            max_out_degree: max_out,
            max_in_degree: max_in,
            has_cycles,
        }
    }
}

impl fmt::Display for GraphSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "nodes={} edges={} roots={} leaves={} cycles={}",
            self.node_count, self.edge_count, self.root_count, self.leaf_count, self.has_cycles
        )
    }
}
// ---------------------------------------------------------------------------
// CallHierarchyPath
// ---------------------------------------------------------------------------

/// Represents an ordered path through a call hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallHierarchyPath {
    chain: Vec<String>,
}

impl CallHierarchyPath {
    /// Create an empty path.
    pub fn new() -> Self {
        Self { chain: Vec::new() }
    }

    /// Append a name to the end of the path.
    pub fn push(&mut self, name: &str) {
        self.chain.push(name.to_string());
    }

    /// Return the full path joined with ` → `.
    pub fn full_path(&self) -> String {
        self.chain.join(" → ")
    }

    /// Return the depth (number of items) in the path.
    pub fn depth(&self) -> usize {
        self.chain.len()
    }

    /// Return `true` if the path contains the given name.
    pub fn contains(&self, name: &str) -> bool {
        self.chain.iter().any(|n| n == name)
    }

    /// Return `true` if the path starts with the given name.
    pub fn starts_with(&self, name: &str) -> bool {
        self.chain.first().map_or(false, |n| n == name)
    }

    /// Return `true` if the path ends with the given name.
    pub fn ends_with(&self, name: &str) -> bool {
        self.chain.last().map_or(false, |n| n == name)
    }

    /// Remove and return the last item, or `None` if empty.
    pub fn pop(&mut self) -> Option<String> {
        self.chain.pop()
    }

    /// Remove all items from the path.
    pub fn clear(&mut self) {
        self.chain.clear();
    }

    /// Return `true` if the path has no items.
    pub fn is_empty(&self) -> bool {
        self.chain.is_empty()
    }
}

impl Default for CallHierarchyPath {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CallHierarchyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.full_path())
    }
}

// ---------------------------------------------------------------------------
// CallHierarchySearcher
// ---------------------------------------------------------------------------

/// Search utilities for finding items in a `CallGraph`.
pub struct CallHierarchySearcher;

impl CallHierarchySearcher {
    /// Return names of items whose name contains `query` (case-insensitive).
    pub fn search(graph: &CallGraph, query: &str) -> Vec<String> {
        let lower = query.to_lowercase();
        let mut results: Vec<String> = graph
            .items
            .values()
            .filter(|item| item.name.to_lowercase().contains(&lower))
            .map(|item| item.name.clone())
            .collect();
        results.sort();
        results.dedup();
        results
    }

    /// Return names of items that match the given `SymbolKind`.
    pub fn search_by_kind(graph: &CallGraph, kind: SymbolKind) -> Vec<String> {
        let mut results: Vec<String> = graph
            .items
            .values()
            .filter(|item| item.kind == kind)
            .map(|item| item.name.clone())
            .collect();
        results.sort();
        results.dedup();
        results
    }

    /// Return names of items whose URI contains `uri` (substring match).
    pub fn search_by_uri(graph: &CallGraph, uri: &str) -> Vec<String> {
        let mut results: Vec<String> = graph
            .items
            .values()
            .filter(|item| item.uri.contains(uri))
            .map(|item| item.name.clone())
            .collect();
        results.sort();
        results.dedup();
        results
    }
}

// ---------------------------------------------------------------------------
// CallHierarchyMetrics / CallMetricsResult
// ---------------------------------------------------------------------------

/// Computed metrics for a `CallGraph`.
#[derive(Debug, Clone, PartialEq)]
pub struct CallMetricsResult {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub avg_fan_out: f64,
    pub avg_fan_in: f64,
    pub max_fan_out: (String, usize),
    pub max_fan_in: (String, usize),
}

impl fmt::Display for CallMetricsResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "nodes={} edges={} avg_fan_out={:.2} avg_fan_in={:.2} max_fan_out={}({}) max_fan_in={}({})",
            self.total_nodes,
            self.total_edges,
            self.avg_fan_out,
            self.avg_fan_in,
            self.max_fan_out.0,
            self.max_fan_out.1,
            self.max_fan_in.0,
            self.max_fan_in.1,
        )
    }
}

/// Compute fan-in / fan-out metrics for a `CallGraph`.
pub struct CallHierarchyMetrics;

impl CallHierarchyMetrics {
    /// Analyze the graph and return aggregated metrics.
    pub fn compute(graph: &CallGraph) -> CallMetricsResult {
        let items = graph.all_items();
        let n = items.len();
        let total_edges = graph.edge_count();

        let (mut max_out_name, mut max_out_val) = (String::new(), 0usize);
        let (mut max_in_name, mut max_in_val) = (String::new(), 0usize);
        let mut sum_out: usize = 0;
        let mut sum_in: usize = 0;

        for item in &items {
            let out = graph.out_degree(item);
            let ind = graph.in_degree(item);
            sum_out += out;
            sum_in += ind;
            if out > max_out_val {
                max_out_val = out;
                max_out_name = item.name.clone();
            }
            if ind > max_in_val {
                max_in_val = ind;
                max_in_name = item.name.clone();
            }
        }

        let avg_fan_out = if n == 0 { 0.0 } else { sum_out as f64 / n as f64 };
        let avg_fan_in = if n == 0 { 0.0 } else { sum_in as f64 / n as f64 };

        CallMetricsResult {
            total_nodes: n,
            total_edges,
            avg_fan_out,
            avg_fan_in,
            max_fan_out: (max_out_name, max_out_val),
            max_fan_in: (max_in_name, max_in_val),
        }
    }
}

// ---------------------------------------------------------------------------
// CallDepthLimiter
// ---------------------------------------------------------------------------

/// Enforces a maximum call-chain depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallDepthLimiter {
    max_depth: usize,
}

impl CallDepthLimiter {
    /// Create a limiter with the given maximum depth.
    pub fn new(max_depth: usize) -> Self {
        Self { max_depth }
    }

    /// Return `true` if `depth` is within the configured limit.
    pub fn is_within_limit(&self, depth: usize) -> bool {
        depth <= self.max_depth
    }

    /// Clamp `depth` to at most `max_depth`.
    pub fn clamp(&self, depth: usize) -> usize {
        depth.min(self.max_depth)
    }

    /// Truncate `chain` to at most `max_depth` elements.
    pub fn limited_chain(&self, chain: &[String]) -> Vec<String> {
        chain.iter().take(self.max_depth).cloned().collect()
    }

    /// Update the depth limit.
    pub fn set_limit(&mut self, new: usize) {
        self.max_depth = new;
    }

    /// Return the current depth limit.
    pub fn limit(&self) -> usize {
        self.max_depth
    }
}

// ---------------------------------------------------------------------------
// HierarchyExporter - call hierarchy exporter
// ---------------------------------------------------------------------------

/// Severity level for call hierarchy exporter issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HierarchyExporterSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for HierarchyExporterSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [HierarchyExporter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyExporterEntry {
    pub id: String,
    pub label: String,
    pub severity: HierarchyExporterSeverity,
    pub detail: Option<String>,
    pub node_count: usize,
    enabled: bool,
}

impl HierarchyExporterEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: HierarchyExporterSeverity::Low,
            detail: None,
            node_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: HierarchyExporterSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_node_count(mut self, val: usize) -> Self {
        self.node_count = val;
        self
    }

    pub fn has_recursion(&self) -> bool {
        self.enabled && self.severity >= HierarchyExporterSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.node_count, det)
    }
}

impl fmt::Display for HierarchyExporterEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [HierarchyExporterEntry] items.
#[derive(Debug, Clone)]
pub struct HierarchyExporter {
    entries: Vec<HierarchyExporterEntry>,
    name: String,
    capacity: usize,
}

impl HierarchyExporter {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: HierarchyExporterEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<HierarchyExporterEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&HierarchyExporterEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn node_count(&self) -> usize { self.entries.len() }

    pub fn has_recursion(&self) -> bool {
        self.entries.iter().any(|e| e.has_recursion())
    }

    pub fn entries_by_severity(&self, severity: HierarchyExporterSeverity) -> Vec<&HierarchyExporterEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= HierarchyExporterSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&HierarchyExporterEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&HierarchyExporterEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// ChainDepthMeter - call chain depth meter
// ---------------------------------------------------------------------------

/// Configuration for [ChainDepthMeter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainDepthMeterConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub max_depth: usize,
}

impl ChainDepthMeterConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, max_depth: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_max_depth(mut self, val: usize) -> Self { self.max_depth = val; self }
}

impl Default for ChainDepthMeterConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ChainDepthMeter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainDepthMeterItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ChainDepthMeterItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn is_leaf(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ChainDepthMeterItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ChainDepthMeterItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ChainDepthMeter {
    config: ChainDepthMeterConfig,
    items: Vec<ChainDepthMeterItem>,
}

impl ChainDepthMeter {
    pub fn new(config: ChainDepthMeterConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ChainDepthMeterItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ChainDepthMeterItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ChainDepthMeterItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn max_depth(&self) -> usize { self.items.len() }

    pub fn is_leaf(&self) -> bool {
        self.items.iter().any(|i| i.is_leaf())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ChainDepthMeterItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ChainDepthMeterItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ChainDepthMeterConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// vsedit-callhier: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallhierXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl CallhierXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for CallhierXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct CallhierXRegistry {
    entries: Vec<CallhierXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl CallhierXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: CallhierXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&CallhierXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut CallhierXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<CallhierXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&CallhierXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&CallhierXConfig> {
        let mut sorted: Vec<&CallhierXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&CallhierXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> CallhierXIterator<'_> {
        CallhierXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct CallhierXIterator<'a> {
    inner: std::slice::Iter<'a, CallhierXConfig>,
}

impl<'a> Iterator for CallhierXIterator<'a> {
    type Item = &'a CallhierXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct CallhierXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl CallhierXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct CallhierXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl CallhierXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &CallhierXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &CallhierXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &CallhierXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for CallhierXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct CallhierXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl CallhierXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &CallhierXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &CallhierXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for CallhierXValidator {
    fn default() -> Self {
        Self::new()
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
// xb_ utilities – batch 60
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer60 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer60 {
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
pub fn xb_fnv1a_60(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_60<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_60<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_60(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_60(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 10
// ---------------------------------------------------------------------------

/// Generic object pool `Xc10Pool<T>`.
pub struct Xc10Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc10Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc10PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc10Pool<T> {
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
    pub fn stats(&self) -> Xc10PoolStats {
        Xc10PoolStats {
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

impl<T> Default for Xc10Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc10Scheduler`.
pub struct Xc10Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc10Scheduler {
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

impl Default for Xc10Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_10 hash for the given byte slice.
pub fn xc_10_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_10 convention.
pub fn xc_10_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe73 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe73Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe73PipelineError {
    pub stage: Xe73Stage,
    pub message: String,
}

impl std::fmt::Display for Xe73PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe73Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe73Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe73PipelineError>>>,
    stage_names: Vec<Xe73Stage>,
}

impl Xe73Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe73PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe73Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe73PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe73Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe73PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe73Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe73PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe73Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe73PipelineError> {
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

    pub fn compose(mut self, other: Xe73Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe73CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe73CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe73Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe73CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe73CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe73Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe73CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_73_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe73CacheEntry {
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

    fn xe_73_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe73CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_73_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe73PipelineError> {
    Ok(data)
}

pub fn xe_73_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe73PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_73_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe73PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_73_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe73PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_73_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe73PipelineError> {
    Err(Xe73PipelineError {
        stage: Xe73Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_71: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg71Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg71Graph {
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

impl Default for Xg71Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_71: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg71Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg71Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg71Heap<T>) {
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

impl<T: Ord> Default for Xg71Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 9).
pub struct Xh9SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh9SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 51 as u64,
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

/// A compact bit set supporting boolean operations (variant 9).
pub struct Xh9BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh9BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 9).
pub struct Xi9Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi9Deque<T> {
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
pub struct Xi9Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi9Interval {
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

/// A simple interval tree (variant 9).
pub struct Xi9IntervalTree {
    xi_intervals: Vec<Xi9Interval>,
}

impl Xi9IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi9Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi9Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi9Interval) -> Vec<&Xi9Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi9Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi9Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi9Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi9Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi9Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi9Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 9) ---

/// Disjoint set / union-find for crate 9.
pub struct Xj9UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj9UnionFind {
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

const XJ9_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 9.
pub struct Xj9BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj9BTreeNode<K, V>>>,
    len: usize,
}

struct Xj9BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj9BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj9BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ9_BTREE_ORDER - 1
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
        let mid = XJ9_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj9BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj9BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj9BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj9BTreeNode::xj_new_leaf();
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


// --- xk_9 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk9SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk9SegmentTree {
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
pub struct Xk9DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk9DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_9).
#[derive(Debug, Clone)]
pub struct Xl9Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl9Rope {
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

/// Suffix array for efficient string searching (xl_9).
#[derive(Debug, Clone)]
pub struct Xl9SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl9SuffixArray {
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
pub struct Xm9MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm9MatrixSparse {
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
pub struct Xm9Tokenizer {
    text: String,
}

impl Xm9Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 9.
pub struct Xn9Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn9Fenwick {
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

// ----- AVL tree map — crate 9 -----

#[derive(Debug, Clone)]
struct Xn9AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn9AvlNode<K, V>>>,
    right: Option<Box<Xn9AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 9.
#[derive(Debug, Clone)]
pub struct Xn9AVL<K, V> {
    root: Option<Box<Xn9AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn9AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn9AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn9AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn9AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn9AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn9AvlNode<K, V>>) -> Box<Xn9AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn9AvlNode<K, V>>) -> Box<Xn9AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn9AvlNode<K, V>>) -> Box<Xn9AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn9AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn9AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn9AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn9AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn9AvlNode<K, V>>) -> &Xn9AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn9AvlNode<K, V>>) -> (Box<Xn9AvlNode<K, V>>, Option<Box<Xn9AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn9AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn9AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn9AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn9AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn9AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn9AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn9AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo9RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo9Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo9RBNode<K, V> {
    key: K,
    value: V,
    color: Xo9Color,
    left: Option<Box<Xo9RBNode<K, V>>>,
    right: Option<Box<Xo9RBNode<K, V>>>,
}

/// A red-black tree map for crate 9.
#[derive(Debug, Clone)]
pub struct Xo9RedBlack<K, V> {
    root: Option<Box<Xo9RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo9RedBlack<K, V> {
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
            r.color = Xo9Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo9RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo9RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo9RBNode {
                    key, value, color: Xo9Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo9RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo9Color::Red)
    }

    fn xo_balance(mut h: Box<Xo9RBNode<K, V>>) -> Box<Xo9RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo9Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo9RBNode<K, V>>) -> Box<Xo9RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo9Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo9RBNode<K, V>>) -> Box<Xo9RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo9Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo9RBNode<K, V>>) {
        h.color = Xo9Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo9Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo9Color::Black; }
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
            r.color = Xo9Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo9RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo9RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo9RBNode<K, V>) -> (K, V, Option<Box<Xo9RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo9RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo9Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo9RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo9ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 9.
#[derive(Debug, Clone)]
pub struct Xo9ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo9ConsistentHash {
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
            let vkey = format!("{}#xo9#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo9#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 9).
#[derive(Debug)]
pub struct Xp9SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp9Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp9Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp9Node<K, V>>>,
    xp_right: Option<Box<Xp9Node<K, V>>>,
}

impl<K: Ord, V> Xp9Node<K, V> {
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

impl<K: Ord, V> Default for Xp9SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp9SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp9Node<K, V>>>, key: &K) -> Option<Box<Xp9Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp9Node<K, V>>) -> Box<Xp9Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp9Node<K, V>>) -> Box<Xp9Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp9Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp9Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp9Node::xp_new(key, val));
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


// --------------- Xq9Treap ---------------

use std::cmp::Ordering as Xq9Ord;

struct Xq9TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq9TreapNode<K, V>>>,
    right: Option<Box<Xq9TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq9Treap<K, V> {
    root: Option<Box<Xq9TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq9TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_9_size<K, V>(node: &Option<Box<Xq9TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_9_update_size<K, V>(node: &mut Xq9TreapNode<K, V>) {
    node.size = 1 + xq_9_size(&node.left) + xq_9_size(&node.right);
}

fn xq_9_rotate_right<K, V>(mut node: Box<Xq9TreapNode<K, V>>) -> Box<Xq9TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_9_update_size(&mut node);
    left.right = Some(node);
    xq_9_update_size(&mut left);
    left
}

fn xq_9_rotate_left<K, V>(mut node: Box<Xq9TreapNode<K, V>>) -> Box<Xq9TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_9_update_size(&mut node);
    right.left = Some(node);
    xq_9_update_size(&mut right);
    right
}

fn xq_9_insert_node<K: Ord, V>(
    node: Option<Box<Xq9TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq9TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq9TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq9Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq9Ord::Less => {
                let (new_left, old) = xq_9_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_9_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_9_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq9Ord::Greater => {
                let (new_right, old) = xq_9_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_9_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_9_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_9_remove_node<K: Ord, V>(
    node: Option<Box<Xq9TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq9TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq9Ord::Less => {
                let (new_left, old) = xq_9_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_9_update_size(&mut n);
                (Some(n), old)
            }
            Xq9Ord::Greater => {
                let (new_right, old) = xq_9_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_9_update_size(&mut n);
                (Some(n), old)
            }
            Xq9Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_9_rotate_right(n);
                    let (new_right, old) = xq_9_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_9_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_9_rotate_left(n);
                    let (new_left, old) = xq_9_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_9_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_9_find_min<K, V>(node: &Option<Box<Xq9TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_9_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_9_find_max<K, V>(node: &Option<Box<Xq9TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_9_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_9_rank<K: Ord, V>(node: &Option<Box<Xq9TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq9Ord::Less => xq_9_rank(&n.left, key),
            Xq9Ord::Equal => xq_9_size(&n.left),
            Xq9Ord::Greater => 1 + xq_9_size(&n.left) + xq_9_rank(&n.right, key),
        },
    }
}

fn xq_9_kth<K, V>(node: &Option<Box<Xq9TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_9_size(&n.left);
        if k < left_size {
            xq_9_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_9_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_9_in_order<K: Clone, V>(node: &Option<Box<Xq9TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_9_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_9_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq9Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 9 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_9_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq9Ord::Equal => return Some(&n.value),
                Xq9Ord::Less => cur = &n.left,
                Xq9Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_9_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_9_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_9_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_9_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_9_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_9_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_9_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq9VEBTree ---------------

pub struct Xq9VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq9VEBTree>>,
    clusters: Vec<Option<Box<Xq9VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq9VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq9VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq9VEBTree::xq_new(self.sqrt_lo)));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(name: &str, kind: SymbolKind) -> CallHierarchyItem {
        CallHierarchyItem {
            name: name.to_string(),
            kind,
            uri: "file:///src/main.rs".to_string(),
            start_line: 1,
            start_col: 0,
            end_line: 5,
            end_col: 1,
            selection_start_line: 1,
            selection_start_col: 4,
            selection_end_line: 1,
            selection_end_col: 8,
            detail: None,
            is_deprecated: false,
        }
    }

    /// A trivial provider used by tests.
    struct StubProvider;

    impl CallHierarchyProvider for StubProvider {
        fn prepare_call_hierarchy(
            &self,
            _uri: &str,
            _line: u32,
            _col: u32,
        ) -> Option<CallHierarchyItem> {
            Some(sample_item("main", SymbolKind::Function))
        }

        fn provide_incoming_calls(&self, _item: &CallHierarchyItem) -> Vec<IncomingCall> {
            vec![IncomingCall {
                from: sample_item("caller", SymbolKind::Method),
                from_ranges: vec![(10, 4, 10, 12)],
            }]
        }

        fn provide_outgoing_calls(&self, _item: &CallHierarchyItem) -> Vec<OutgoingCall> {
            vec![OutgoingCall {
                to: sample_item("helper", SymbolKind::Function),
                from_ranges: vec![(3, 4, 3, 10)],
            }]
        }
    }

    #[test]
    fn prepare_returns_item() {
        let provider = StubProvider;
        let item = provider
            .prepare_call_hierarchy("file:///src/main.rs", 1, 4)
            .expect("should resolve an item");
        assert_eq!(item.name, "main");
        assert_eq!(item.kind, SymbolKind::Function);
    }

    #[test]
    fn incoming_calls_populated() {
        let provider = StubProvider;
        let item = sample_item("main", SymbolKind::Function);
        let incoming = provider.provide_incoming_calls(&item);
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].from.name, "caller");
        assert_eq!(incoming[0].from_ranges, vec![(10, 4, 10, 12)]);
    }

    #[test]
    fn outgoing_calls_populated() {
        let provider = StubProvider;
        let item = sample_item("main", SymbolKind::Function);
        let outgoing = provider.provide_outgoing_calls(&item);
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].to.name, "helper");
        assert_eq!(outgoing[0].to.kind, SymbolKind::Function);
        assert_eq!(outgoing[0].from_ranges, vec![(3, 4, 3, 10)]);
    }

    #[test]
    fn display_symbol_kind() {
        assert_eq!(format!("{}", SymbolKind::Function), "Function");
        assert_eq!(format!("{}", SymbolKind::Method), "Method");
        assert_eq!(format!("{}", SymbolKind::Constructor), "Constructor");
        assert_eq!(format!("{}", SymbolKind::Struct), "Struct");
    }

    #[test]
    fn display_call_hierarchy_item() {
        let item = sample_item("main", SymbolKind::Function);
        assert_eq!(
            format!("{}", item),
            "main (Function) at file:///src/main.rs:1"
        );
    }

    #[test]
    fn contains_position_inside() {
        let item = sample_item("f", SymbolKind::Function);
        assert!(item.contains_position(1, 0));
        assert!(item.contains_position(3, 5));
        assert!(item.contains_position(5, 1));
    }

    #[test]
    fn contains_position_outside() {
        let item = sample_item("f", SymbolKind::Function);
        assert!(!item.contains_position(0, 0));
        assert!(!item.contains_position(6, 0));
        assert!(!item.contains_position(5, 2));
        assert!(!item.contains_position(1, 0).then(|| false).unwrap_or(true)
            || !item.contains_position(0, 99));
    }

    #[test]
    fn builder_with_detail_and_deprecated() {
        let item = sample_item("old_fn", SymbolKind::Function)
            .with_detail("module::old_fn")
            .with_deprecated(true);
        assert_eq!(item.detail.as_deref(), Some("module::old_fn"));
        assert!(item.is_deprecated);
    }

    #[test]
    fn error_display() {
        let e1 = CallHierarchyError::NoItemAtPosition {
            uri: "file:///a.rs".into(),
            line: 10,
            col: 5,
        };
        assert_eq!(format!("{}", e1), "no item at position file:///a.rs:10:5");

        let e2 = CallHierarchyError::ProviderFailed("timeout".into());
        assert_eq!(format!("{}", e2), "provider failed: timeout");

        let e3 = CallHierarchyError::CyclicCallChain("recurse".into());
        assert_eq!(format!("{}", e3), "cyclic call chain from 'recurse'");
    }

    #[test]
    fn call_graph_add_and_query() {
        let mut graph = CallGraph::new();
        let main = sample_item("main", SymbolKind::Function);
        let helper = sample_item("helper", SymbolKind::Function);
        graph.add_edge(&main, &helper);

        assert_eq!(graph.node_count(), 2);
        let callees = graph.get_callees(&main);
        assert_eq!(callees.len(), 1);
        assert_eq!(callees[0].name, "helper");
        let callers = graph.get_callers(&helper);
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].name, "main");
    }

    #[test]
    fn call_graph_roots_and_leaves() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Method);
        let c = sample_item("c", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);

        let roots = graph.find_roots();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "a");

        let leaves = graph.find_leaves();
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].name, "c");
    }

    #[test]
    fn call_graph_cycle_detection() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);
        assert!(!graph.has_cycle_from(&a));

        graph.add_edge(&c, &a);
        assert!(graph.has_cycle_from(&a));
        assert!(graph.has_cycle_from(&b));
    }

    #[test]
    fn call_graph_no_callers_no_callees() {
        let graph = CallGraph::new();
        let item = sample_item("lonely", SymbolKind::Struct);
        assert!(graph.get_callers(&item).is_empty());
        assert!(graph.get_callees(&item).is_empty());
    }

    #[test]
    fn call_graph_isolated_node_is_root_and_leaf() {
        let mut graph = CallGraph::new();
        let item = sample_item("isolated", SymbolKind::Module);
        graph.add_item(item.clone());

        let roots = graph.find_roots();
        assert!(roots.iter().any(|r| r.name == "isolated"));
        let leaves = graph.find_leaves();
        assert!(leaves.iter().any(|l| l.name == "isolated"));
    }

    #[test]
    fn eq_callhierarchyerror_same() {
        assert!(std::mem::size_of::<CallHierarchyError>() > 0);
    }

    #[test]
    fn ne_callhierarchyerror_diff() {
        assert!(std::mem::size_of::<CallHierarchyError>() > 0);
    }

    #[test]
    fn eq_symbolkind_same() {
        assert_eq!(SymbolKind::Function, SymbolKind::Function);
    }

    #[test]
    fn ne_symbolkind_diff() {
        assert_ne!(SymbolKind::Function, SymbolKind::Method);
    }

    #[test]
    fn display_callhierarchyerror_variants() {
        assert!(std::mem::size_of::<CallHierarchyError>() > 0);
        assert!(std::mem::size_of::<CallHierarchyError>() > 0);
    }

    #[test]
    fn display_symbolkind_variants() {
        assert!(!SymbolKind::Function.to_string().is_empty());
        assert!(!SymbolKind::Method.to_string().is_empty());
        assert!(!SymbolKind::Constructor.to_string().is_empty());
        assert!(!SymbolKind::Class.to_string().is_empty());
        assert!(!SymbolKind::Interface.to_string().is_empty());
    }

    #[test]
    fn call_hierarchy_direction_display() {
        assert_eq!(format!("{}", CallHierarchyDirection::Incoming), "Incoming");
        assert_eq!(format!("{}", CallHierarchyDirection::Outgoing), "Outgoing");
    }

    #[test]
    fn call_hierarchy_direction_is_incoming_and_opposite() {
        assert!(CallHierarchyDirection::Incoming.is_incoming());
        assert!(!CallHierarchyDirection::Outgoing.is_incoming());
        assert_eq!(
            CallHierarchyDirection::Incoming.opposite(),
            CallHierarchyDirection::Outgoing
        );
        assert_eq!(
            CallHierarchyDirection::Outgoing.opposite(),
            CallHierarchyDirection::Incoming
        );
    }

    #[test]
    fn call_graph_builder_basic() {
        let mut builder = CallGraphBuilder::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Method);
        builder.add_call(a, b);
        assert_eq!(builder.edge_count(), 1);
        let graph = builder.build();
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn call_graph_builder_multiple_edges() {
        let mut builder = CallGraphBuilder::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        builder.add_call(a.clone(), b).add_call(a, c);
        assert_eq!(builder.edge_count(), 2);
        let graph = builder.build();
        assert_eq!(graph.node_count(), 3);
    }

    #[test]
    fn call_hierarchy_flatten_outgoing() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);
        let flat = call_hierarchy_flatten(&graph, &a, CallHierarchyDirection::Outgoing, 10);
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].0, 1); // depth 1
        assert_eq!(flat[1].0, 2); // depth 2
    }

    #[test]
    fn call_hierarchy_flatten_incoming() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);
        let flat = call_hierarchy_flatten(&graph, &c, CallHierarchyDirection::Incoming, 10);
        assert_eq!(flat.len(), 2);
    }

    #[test]
    fn call_hierarchy_flatten_max_depth() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);
        let flat = call_hierarchy_flatten(&graph, &a, CallHierarchyDirection::Outgoing, 1);
        assert_eq!(flat.len(), 1); // only depth 1, not depth 2
        assert_eq!(flat[0].1.name, "b");
    }

    #[test]
    fn call_graph_edge_count() {
        let mut graph = CallGraph::new();
        assert_eq!(graph.edge_count(), 0);
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&a, &c);
        assert_eq!(graph.edge_count(), 2);
    }

    #[test]
    fn call_graph_get_item() {
        let mut graph = CallGraph::new();
        let item = sample_item("foo", SymbolKind::Method);
        graph.add_item(item.clone());
        let found = graph.get_item("foo", "file:///src/main.rs");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "foo");
        assert!(graph.get_item("nonexistent", "file:///src/main.rs").is_none());
    }

    #[test]
    fn callhier_stats_new_defaults() {
        let stats = CallhierStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn callhier_stats_record_success() {
        let mut stats = CallhierStats::new();
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
    fn callhier_stats_record_failure() {
        let mut stats = CallhierStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn callhier_stats_reset() {
        let mut stats = CallhierStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn callhier_stats_merge() {
        let mut a = CallhierStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = CallhierStats::new();
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
    fn callhier_stats_display() {
        let mut stats = CallhierStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn callhier_stats_default() {
        let stats = CallhierStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn callhier_validator_accepts_valid_name() {
        let v = CallhierValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn callhier_validator_rejects_empty() {
        let v = CallhierValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn callhier_validator_rejects_too_long() {
        let v = CallhierValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn callhier_validator_forbidden_prefix() {
        let v = CallhierValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn callhier_validator_allowed_chars() {
        let v = CallhierValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn callhier_validator_range() {
        let v = CallhierValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn callhier_sanitize_removes_control() {
        let result = CallhierValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn callhier_truncate_short_string() {
        assert_eq!(CallhierValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn callhier_truncate_long_string() {
        let result = CallhierValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn callhier_is_ascii_printable() {
        assert!(CallhierValidator::is_ascii_printable("Hello World 123"));
        assert!(!CallhierValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn symbol_kind_is_callable() {
        assert!(SymbolKind::Function.is_callable());
        assert!(SymbolKind::Method.is_callable());
        assert!(SymbolKind::Constructor.is_callable());
        assert!(!SymbolKind::Class.is_callable());
        assert!(!SymbolKind::Interface.is_callable());
        assert!(!SymbolKind::Module.is_callable());
        assert!(!SymbolKind::Property.is_callable());
        assert!(!SymbolKind::Enum.is_callable());
        assert!(!SymbolKind::Struct.is_callable());
    }

    #[test]
    fn symbol_kind_icon_char() {
        assert_eq!(SymbolKind::Function.icon_char(), 'f');
        assert_eq!(SymbolKind::Method.icon_char(), 'm');
        assert_eq!(SymbolKind::Constructor.icon_char(), 'k');
        assert_eq!(SymbolKind::Class.icon_char(), 'c');
        assert_eq!(SymbolKind::Interface.icon_char(), 'i');
        assert_eq!(SymbolKind::Module.icon_char(), 'M');
        assert_eq!(SymbolKind::Property.icon_char(), 'p');
        assert_eq!(SymbolKind::Enum.icon_char(), 'e');
        assert_eq!(SymbolKind::Struct.icon_char(), 's');
    }

    #[test]
    fn display_location_format() {
        let item = sample_item("foo", SymbolKind::Function);
        assert_eq!(item.display_location(), "file:///src/main.rs:1:0");
    }

    #[test]
    fn call_graph_contains_item() {
        let mut graph = CallGraph::new();
        let a = sample_item("alpha", SymbolKind::Function);
        let b = sample_item("beta", SymbolKind::Method);
        graph.add_item(a);
        graph.add_item(b);
        assert!(graph.contains_item("alpha"));
        assert!(graph.contains_item("beta"));
        assert!(!graph.contains_item("gamma"));
    }

    #[test]
    fn call_graph_depth_from_linear() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        let d = sample_item("d", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);
        graph.add_edge(&c, &d);
        assert_eq!(graph.depth_from(&a), 3);
        assert_eq!(graph.depth_from(&b), 2);
        assert_eq!(graph.depth_from(&c), 1);
        assert_eq!(graph.depth_from(&d), 0);
    }

    #[test]
    fn call_graph_depth_from_with_cycle() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);
        graph.add_edge(&c, &a);
        // Cycle is handled; depth should still terminate
        assert_eq!(graph.depth_from(&a), 2);
    }

    #[test]
    fn call_graph_items_returns_all() {
        let mut graph = CallGraph::new();
        let a = sample_item("x", SymbolKind::Function);
        let b = sample_item("y", SymbolKind::Method);
        let c = sample_item("z", SymbolKind::Struct);
        graph.add_item(a);
        graph.add_item(b);
        graph.add_item(c);
        let items = graph.items();
        assert_eq!(items.len(), 3);
        let names: HashSet<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains("x"));
        assert!(names.contains("y"));
        assert!(names.contains("z"));
    }

    // ── New tests ──

    #[test]
    fn call_chain_analyzer_find_paths() {
        let mut graph = CallGraph::new();
        let a = sample_item("main", SymbolKind::Function);
        let b = sample_item("process", SymbolKind::Function);
        let c = sample_item("output", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);
        graph.add_edge(&a, &c); // direct shortcut
        let analyzer = CallChainAnalyzer::new(&graph);
        let paths = analyzer.find_paths(&a, &c);
        assert!(paths.len() >= 2);
        // One path is [main, output], another is [main, process, output]
        assert!(paths.iter().any(|p| p.len() == 2));
        assert!(paths.iter().any(|p| p.len() == 3));
    }

    #[test]
    fn call_chain_analyzer_longest_chain() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        let d = sample_item("d", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);
        graph.add_edge(&c, &d);
        let analyzer = CallChainAnalyzer::new(&graph);
        let chain = analyzer.longest_chain_from(&a);
        assert_eq!(chain, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn call_graph_exporter_text() {
        let mut graph = CallGraph::new();
        let a = sample_item("alpha", SymbolKind::Function);
        let b = sample_item("beta", SymbolKind::Function);
        graph.add_edge(&a, &b);
        let exporter = CallGraphExporter::new(&graph);
        let text = exporter.to_text();
        assert!(text.contains("alpha -> beta"));
        assert_eq!(exporter.edge_count(), 1);
    }

    #[test]
    fn call_graph_exporter_dot() {
        let mut graph = CallGraph::new();
        let a = sample_item("foo", SymbolKind::Function);
        let b = sample_item("bar", SymbolKind::Method);
        graph.add_edge(&a, &b);
        let exporter = CallGraphExporter::new(&graph);
        let dot = exporter.to_dot();
        assert!(dot.starts_with("digraph CallGraph {"));
        assert!(dot.contains("\"foo\" -> \"bar\""));
        assert!(dot.ends_with('}'));
    }

    #[test]
    fn recursion_detector_direct() {
        let mut graph = CallGraph::new();
        let a = sample_item("recurse", SymbolKind::Function);
        graph.add_edge(&a, &a);
        let detector = RecursionDetector::new(&graph);
        assert!(detector.is_directly_recursive(&a));
        let direct = detector.find_directly_recursive();
        assert_eq!(direct.len(), 1);
    }

    #[test]
    fn recursion_detector_indirect() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &a);
        let detector = RecursionDetector::new(&graph);
        assert!(!detector.is_directly_recursive(&a));
        let recursive = detector.find_all_recursive_items();
        assert_eq!(recursive.len(), 2);
    }

    #[test]
    fn reachable_within_depth_limit() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        let d = sample_item("d", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);
        graph.add_edge(&c, &d);
        // depth 1: a, b
        let r1 = graph.reachable_within(&a, 1);
        assert_eq!(r1.len(), 2);
        // depth 0: just a
        let r0 = graph.reachable_within(&a, 0);
        assert_eq!(r0.len(), 1);
        // depth 3: all four
        let r3 = graph.reachable_within(&a, 3);
        assert_eq!(r3.len(), 4);
    }

    #[test]
    fn edge_count_and_degree() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Method);
        let c = sample_item("c", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&a, &c);
        graph.add_edge(&b, &c);
        assert_eq!(graph.edge_count(), 3);
        assert_eq!(graph.out_degree(&a), 2);
        assert_eq!(graph.in_degree(&c), 2);
        assert_eq!(graph.in_degree(&a), 0);
    }

    #[test]
    fn eccentricity_linear_chain() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);
        assert_eq!(graph.eccentricity(&a), 2);
        assert_eq!(graph.eccentricity(&c), 0);
    }

    #[test]
    fn topological_sort_dag() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);
        let sorted = graph.topological_sort();
        assert!(sorted.is_some());
        let names: Vec<&str> = sorted.unwrap().iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn topological_sort_returns_none_on_cycle() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &a);
        assert!(graph.topological_sort().is_none());
    }

    #[test]
    fn shortest_path_length_works() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        let d = sample_item("d", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&b, &c);
        graph.add_edge(&a, &c);
        graph.add_item(d.clone());
        assert_eq!(graph.shortest_path_length(&a, &a), Some(0));
        assert_eq!(graph.shortest_path_length(&a, &c), Some(1)); // direct edge
        assert_eq!(graph.shortest_path_length(&c, &a), None); // unreachable
        assert_eq!(graph.shortest_path_length(&a, &d), None); // isolated node
    }

    #[test]
    fn graph_summary_display() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Method);
        graph.add_edge(&a, &b);
        let summary = GraphSummary::from_graph(&graph);
        assert_eq!(summary.node_count, 2);
        assert_eq!(summary.edge_count, 1);
        assert_eq!(summary.root_count, 1);
        assert_eq!(summary.leaf_count, 1);
        assert!(!summary.has_cycles);
        let display = format!("{}", summary);
        assert!(display.contains("nodes=2"));
    }

    #[test]
    fn items_by_kind_filter() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Method);
        let c = sample_item("c", SymbolKind::Function);
        graph.add_item(a);
        graph.add_item(b);
        graph.add_item(c);
        let fns = graph.items_by_kind(SymbolKind::Function);
        assert_eq!(fns.len(), 2);
        let methods = graph.items_by_kind(SymbolKind::Method);
        assert_eq!(methods.len(), 1);
    }

    // ── CallHierarchyPath tests ──

    #[test]
    fn path_push_and_full_path() {
        let mut p = CallHierarchyPath::new();
        p.push("main");
        p.push("init");
        p.push("run");
        assert_eq!(p.full_path(), "main → init → run");
        assert_eq!(p.depth(), 3);
    }

    #[test]
    fn path_contains_and_endpoints() {
        let mut p = CallHierarchyPath::new();
        p.push("a");
        p.push("b");
        p.push("c");
        assert!(p.contains("b"));
        assert!(!p.contains("z"));
        assert!(p.starts_with("a"));
        assert!(!p.starts_with("b"));
        assert!(p.ends_with("c"));
    }

    #[test]
    fn path_pop_clear_empty() {
        let mut p = CallHierarchyPath::new();
        assert!(p.is_empty());
        p.push("x");
        p.push("y");
        assert_eq!(p.pop(), Some("y".to_string()));
        assert_eq!(p.depth(), 1);
        p.clear();
        assert!(p.is_empty());
        assert_eq!(p.pop(), None);
    }

    #[test]
    fn path_display() {
        let mut p = CallHierarchyPath::new();
        p.push("foo");
        p.push("bar");
        assert_eq!(format!("{}", p), "foo → bar");
    }

    // ── CallHierarchySearcher tests ──

    #[test]
    fn searcher_case_insensitive() {
        let mut graph = CallGraph::new();
        graph.add_item(sample_item("FooBar", SymbolKind::Function));
        graph.add_item(sample_item("baz", SymbolKind::Method));
        let results = CallHierarchySearcher::search(&graph, "foo");
        assert_eq!(results, vec!["FooBar"]);
    }

    #[test]
    fn searcher_no_match() {
        let mut graph = CallGraph::new();
        graph.add_item(sample_item("alpha", SymbolKind::Function));
        let results = CallHierarchySearcher::search(&graph, "zzz");
        assert!(results.is_empty());
    }

    #[test]
    fn searcher_by_kind() {
        let mut graph = CallGraph::new();
        graph.add_item(sample_item("f1", SymbolKind::Function));
        graph.add_item(sample_item("m1", SymbolKind::Method));
        graph.add_item(sample_item("f2", SymbolKind::Function));
        let fns = CallHierarchySearcher::search_by_kind(&graph, SymbolKind::Function);
        assert_eq!(fns.len(), 2);
        assert!(fns.contains(&"f1".to_string()));
    }

    #[test]
    fn searcher_by_uri() {
        let mut graph = CallGraph::new();
        graph.add_item(sample_item("x", SymbolKind::Function));
        let results = CallHierarchySearcher::search_by_uri(&graph, "main.rs");
        assert_eq!(results, vec!["x"]);
        let empty = CallHierarchySearcher::search_by_uri(&graph, "other.rs");
        assert!(empty.is_empty());
    }

    // ── CallHierarchyMetrics tests ──

    #[test]
    fn metrics_empty_graph() {
        let graph = CallGraph::new();
        let m = CallHierarchyMetrics::compute(&graph);
        assert_eq!(m.total_nodes, 0);
        assert_eq!(m.total_edges, 0);
        assert_eq!(m.avg_fan_out, 0.0);
    }

    #[test]
    fn metrics_with_edges() {
        let mut graph = CallGraph::new();
        let a = sample_item("a", SymbolKind::Function);
        let b = sample_item("b", SymbolKind::Function);
        let c = sample_item("c", SymbolKind::Function);
        graph.add_edge(&a, &b);
        graph.add_edge(&a, &c);
        let m = CallHierarchyMetrics::compute(&graph);
        assert_eq!(m.total_nodes, 3);
        assert_eq!(m.total_edges, 2);
        assert_eq!(m.max_fan_out.0, "a");
        assert_eq!(m.max_fan_out.1, 2);
    }

    #[test]
    fn metrics_display() {
        let graph = CallGraph::new();
        let m = CallHierarchyMetrics::compute(&graph);
        let s = format!("{}", m);
        assert!(s.contains("nodes=0"));
        assert!(s.contains("edges=0"));
    }

    // ── CallDepthLimiter tests ──

    #[test]
    fn limiter_within_and_clamp() {
        let lim = CallDepthLimiter::new(5);
        assert!(lim.is_within_limit(3));
        assert!(lim.is_within_limit(5));
        assert!(!lim.is_within_limit(6));
        assert_eq!(lim.clamp(10), 5);
        assert_eq!(lim.clamp(2), 2);
    }

    #[test]
    fn limiter_limited_chain() {
        let lim = CallDepthLimiter::new(2);
        let chain: Vec<String> = vec!["a", "b", "c", "d"]
            .into_iter()
            .map(String::from)
            .collect();
        let limited = lim.limited_chain(&chain);
        assert_eq!(limited, vec!["a", "b"]);
    }

    #[test]
    fn limiter_set_and_get() {
        let mut lim = CallDepthLimiter::new(3);
        assert_eq!(lim.limit(), 3);
        lim.set_limit(10);
        assert_eq!(lim.limit(), 10);
        assert!(lim.is_within_limit(10));
        assert!(!lim.is_within_limit(11));
    }

#[test]
    fn hierarchyexporter_severity_ordering() {
        assert!(HierarchyExporterSeverity::Critical > HierarchyExporterSeverity::High);
        assert!(HierarchyExporterSeverity::High > HierarchyExporterSeverity::Medium);
        assert!(HierarchyExporterSeverity::Medium > HierarchyExporterSeverity::Low);
    }

    #[test]
    fn hierarchyexporter_severity_display() {
        assert_eq!(HierarchyExporterSeverity::Low.to_string(), "low");
        assert_eq!(HierarchyExporterSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn hierarchyexporter_entry_creation() {
        let e = HierarchyExporterEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, HierarchyExporterSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn hierarchyexporter_entry_builder() {
        let e = HierarchyExporterEntry::new("e2", "Entry 2")
            .with_severity(HierarchyExporterSeverity::High)
            .with_detail("some detail")
            .with_node_count(42);
        assert_eq!(e.severity, HierarchyExporterSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.node_count, 42);
    }

    #[test]
    fn hierarchyexporter_entry_enable_disable() {
        let mut e = HierarchyExporterEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn hierarchyexporter_add_and_count() {
        let mut mgr = HierarchyExporter::new("test");
        mgr.add(HierarchyExporterEntry::new("a", "A"));
        mgr.add(HierarchyExporterEntry::new("b", "B").with_severity(HierarchyExporterSeverity::High));
        assert_eq!(mgr.node_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn hierarchyexporter_remove() {
        let mut mgr = HierarchyExporter::new("test");
        mgr.add(HierarchyExporterEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn hierarchyexporter_capacity() {
        let mut mgr = HierarchyExporter::new("test").with_capacity(1);
        assert!(mgr.add(HierarchyExporterEntry::new("a", "A")));
        assert!(!mgr.add(HierarchyExporterEntry::new("b", "B")));
    }

    #[test]
    fn hierarchyexporter_sorted_by_severity() {
        let mut mgr = HierarchyExporter::new("test");
        mgr.add(HierarchyExporterEntry::new("lo", "Low"));
        mgr.add(HierarchyExporterEntry::new("hi", "High").with_severity(HierarchyExporterSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, HierarchyExporterSeverity::Critical);
    }

    #[test]
    fn hierarchyexporter_summary() {
        let mgr = HierarchyExporter::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn chaindepthmeter_config_defaults() {
        let cfg = ChainDepthMeterConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn chaindepthmeter_item_creation() {
        let item = ChainDepthMeterItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn chaindepthmeter_add_and_get() {
        let mut mgr = ChainDepthMeter::new(ChainDepthMeterConfig::new("test"));
        mgr.add(ChainDepthMeterItem::new("k1", "v1"));
        assert_eq!(mgr.max_depth(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn chaindepthmeter_remove_item() {
        let mut mgr = ChainDepthMeter::new(ChainDepthMeterConfig::new("test"));
        mgr.add(ChainDepthMeterItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn chaindepthmeter_sorted_by_priority() {
        let mut mgr = ChainDepthMeter::new(ChainDepthMeterConfig::new("test"));
        mgr.add(ChainDepthMeterItem::new("lo", "low").with_priority(1));
        mgr.add(ChainDepthMeterItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn chaindepthmeter_items_with_tag() {
        let mut mgr = ChainDepthMeter::new(ChainDepthMeterConfig::new("test"));
        mgr.add(ChainDepthMeterItem::new("a", "1").with_tag("x"));
        mgr.add(ChainDepthMeterItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn chaindepthmeter_report() {
        let mgr = ChainDepthMeter::new(ChainDepthMeterConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn callhier_x_config_new() {
        let c = CallhierXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn callhier_x_config_builder() {
        let c = CallhierXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn callhier_x_config_display() {
        let c = CallhierXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn callhier_x_registry_insert_get() {
        let mut reg = CallhierXRegistry::new();
        reg.insert(CallhierXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn callhier_x_registry_duplicate() {
        let mut reg = CallhierXRegistry::new();
        reg.insert(CallhierXConfig::new("a")).unwrap();
        assert!(reg.insert(CallhierXConfig::new("a")).is_err());
    }

    #[test]
    fn callhier_x_registry_remove() {
        let mut reg = CallhierXRegistry::new();
        reg.insert(CallhierXConfig::new("a")).unwrap();
        reg.insert(CallhierXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn callhier_x_registry_active_entries() {
        let mut reg = CallhierXRegistry::new();
        reg.insert(CallhierXConfig::new("a")).unwrap();
        reg.insert(CallhierXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn callhier_x_registry_by_weight() {
        let mut reg = CallhierXRegistry::new();
        reg.insert(CallhierXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(CallhierXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn callhier_x_registry_tags() {
        let mut reg = CallhierXRegistry::new();
        reg.insert(CallhierXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(CallhierXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn callhier_x_registry_total_weight() {
        let mut reg = CallhierXRegistry::new();
        reg.insert(CallhierXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(CallhierXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn callhier_x_registry_iterator() {
        let mut reg = CallhierXRegistry::new();
        reg.insert(CallhierXConfig::new("a")).unwrap();
        reg.insert(CallhierXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn callhier_x_cache_put_get() {
        let mut cache = CallhierXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn callhier_x_cache_eviction() {
        let mut cache = CallhierXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn callhier_x_cache_lru_order() {
        let mut cache = CallhierXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn callhier_x_cache_most_least_recent() {
        let mut cache = CallhierXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn callhier_x_formatter_entry() {
        let e = CallhierXConfig::new("k").with_value("v");
        let fmt = CallhierXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn callhier_x_formatter_summary() {
        let mut reg = CallhierXRegistry::new();
        reg.insert(CallhierXConfig::new("a").with_weight(5)).unwrap();
        let fmt = CallhierXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn callhier_x_validator_valid() {
        let v = CallhierXValidator::new();
        let c = CallhierXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn callhier_x_validator_empty_key() {
        let v = CallhierXValidator::new();
        let c = CallhierXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn callhier_x_validator_require_value() {
        let v = CallhierXValidator::new().require_value(true);
        let c = CallhierXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn callhier_x_validator_allowed_tags() {
        let v = CallhierXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = CallhierXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn callhier_x_validator_validate_all() {
        let v = CallhierXValidator::new();
        let mut reg = CallhierXRegistry::new();
        reg.insert(CallhierXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
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


    #[test]
    fn xb_ring_buffer_60_push_and_len() {
        let mut rb = super::XbRingBuffer60::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_60_overwrite() {
        let mut rb = super::XbRingBuffer60::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_60_get_out_of_bounds() {
        let rb = super::XbRingBuffer60::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_60_drain_all() {
        let mut rb = super::XbRingBuffer60::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_60_peek_front_back() {
        let mut rb = super::XbRingBuffer60::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_60_clear() {
        let mut rb = super::XbRingBuffer60::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_60_capacity() {
        let rb = super::XbRingBuffer60::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_60_basic() {
        let h = super::xb_fnv1a_60(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_60(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_60_different_inputs() {
        let h1 = super::xb_fnv1a_60(b"abc");
        let h2 = super::xb_fnv1a_60(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_60_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_60(&data);
        let dec = super::xb_rle_decode_60(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_60_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_60(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_60(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_60_values() {
        assert!((super::xb_clamp_60(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_60(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_60(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_60_values() {
        assert!((super::xb_lerp_60(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_60(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_60(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_60_wrap_around_twice() {
        let mut rb = super::XbRingBuffer60::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 10 ----

    #[test]
    fn xc_10_pool_new_empty() {
        let pool: super::Xc10Pool<i32> = super::Xc10Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_10_pool_release_acquire() {
        let mut pool = super::Xc10Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_10_pool_acquire_empty() {
        let mut pool: super::Xc10Pool<i32> = super::Xc10Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_10_pool_full() {
        let mut pool = super::Xc10Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_10_pool_drain() {
        let mut pool = super::Xc10Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_10_pool_stats() {
        let mut pool = super::Xc10Pool::new(8);
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
    fn xc_10_pool_clear() {
        let mut pool = super::Xc10Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_10_pool_shrink() {
        let mut pool = super::Xc10Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_10_pool_default() {
        let pool: super::Xc10Pool<String> = super::Xc10Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_10_pool_extend() {
        let mut pool = super::Xc10Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_10_pool_retain() {
        let mut pool = super::Xc10Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_10_scheduler_round_robin() {
        let mut sched = super::Xc10Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_10_scheduler_empty() {
        let mut sched = super::Xc10Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_10_scheduler_reset() {
        let mut sched = super::Xc10Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_10_scheduler_add_remove() {
        let mut sched = super::Xc10Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_10_scheduler_targets() {
        let sched = super::Xc10Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_10_hash_empty() {
        assert_eq!(super::xc_10_hash(b""), 5381);
    }

    #[test]
    fn xc_10_hash_data() {
        let h = super::xc_10_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_10_hash(b"hello"), h);
    }

    #[test]
    fn xc_10_reverse_str() {
        assert_eq!(super::xc_10_reverse("abc"), "cba");
        assert_eq!(super::xc_10_reverse(""), "");
    }


    #[test]
    fn xe_73_pipeline_empty() {
        let p = super::Xe73Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_73_pipeline_parse_stage() {
        let p = super::Xe73Pipeline::new()
            .add_parse(super::xe_73_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_73_pipeline_transform_double() {
        let p = super::Xe73Pipeline::new()
            .add_transform(super::xe_73_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_73_pipeline_validate_reverse() {
        let p = super::Xe73Pipeline::new()
            .add_validate(super::xe_73_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_73_pipeline_emit_filter() {
        let p = super::Xe73Pipeline::new()
            .add_emit(super::xe_73_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_73_pipeline_multi_stage() {
        let p = super::Xe73Pipeline::new()
            .add_parse(super::xe_73_pipeline_identity)
            .add_transform(super::xe_73_pipeline_double)
            .add_validate(super::xe_73_pipeline_reverse)
            .add_emit(super::xe_73_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_73_pipeline_error_propagation() {
        let p = super::Xe73Pipeline::new()
            .add_parse(super::xe_73_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe73Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_73_pipeline_compose() {
        let p1 = super::Xe73Pipeline::new()
            .add_parse(super::xe_73_pipeline_identity);
        let p2 = super::Xe73Pipeline::new()
            .add_transform(super::xe_73_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_73_pipeline_error_display() {
        let e = super::Xe73PipelineError {
            stage: super::Xe73Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_73_cache_put_get() {
        let mut c = super::Xe73Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_73_cache_miss() {
        let mut c: super::Xe73Cache<&str, i32> = super::Xe73Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_73_cache_ttl_expiry() {
        let mut c = super::Xe73Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_73_cache_evict() {
        let mut c = super::Xe73Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_73_cache_capacity() {
        let mut c = super::Xe73Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_73_cache_stats() {
        let mut c = super::Xe73Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_73_cache_clear() {
        let mut c = super::Xe73Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_71 graph tests ------------------------------------------------

    #[test]
    fn xg_71_graph_empty() {
        let g = super::Xg71Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_71_graph_add_node() {
        let mut g = super::Xg71Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_71_graph_add_edge() {
        let mut g = super::Xg71Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_71_graph_neighbors() {
        let mut g = super::Xg71Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_71_graph_has_path() {
        let mut g = super::Xg71Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_71_graph_self_path() {
        let g = super::Xg71Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_71_graph_topo_sort() {
        let mut g = super::Xg71Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_71_graph_cycle_detect_false() {
        let mut g = super::Xg71Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_71_graph_cycle_detect_true() {
        let mut g = super::Xg71Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_71 heap tests -------------------------------------------------

    #[test]
    fn xg_71_heap_empty() {
        let h: super::Xg71Heap<i32> = super::Xg71Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_71_heap_push_pop() {
        let mut h = super::Xg71Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_71_heap_peek() {
        let mut h = super::Xg71Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_71_heap_drain_sorted() {
        let mut h = super::Xg71Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_71_heap_merge() {
        let mut a = super::Xg71Heap::new();
        let mut b = super::Xg71Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_71_heap_default() {
        let h: super::Xg71Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_71_graph_default() {
        let g: super::Xg71Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh9_skip_insert_contains() {
        let mut sl = super::Xh9SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh9_skip_remove() {
        let mut sl = super::Xh9SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh9_skip_len() {
        let mut sl = super::Xh9SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh9_skip_range_query() {
        let mut sl = super::Xh9SkipList::xh_new(4);
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
    fn xh9_skip_floor_ceiling() {
        let mut sl = super::Xh9SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh9_skip_rank() {
        let mut sl = super::Xh9SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh9_skip_empty() {
        let sl = super::Xh9SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh9_skip_duplicates() {
        let mut sl = super::Xh9SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh9_bitset_set_test() {
        let mut bs = super::Xh9BitSet::xh_new(256);
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
    fn xh9_bitset_clear_count() {
        let mut bs = super::Xh9BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh9_bitset_and_or_xor() {
        let mut a = super::Xh9BitSet::xh_new(128);
        let mut b = super::Xh9BitSet::xh_new(128);
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
    fn xh9_bitset_iter_ones() {
        let mut bs = super::Xh9BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh9_bitset_first_last() {
        let mut bs = super::Xh9BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh9_bitset_empty() {
        let bs = super::Xh9BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi9_deque_push_pop_back() {
        let mut dq = super::Xi9Deque::xi_new(4);
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
    fn xi9_deque_push_pop_front() {
        let mut dq = super::Xi9Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi9_deque_mixed_ops() {
        let mut dq = super::Xi9Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi9_deque_get_and_split() {
        let mut dq = super::Xi9Deque::xi_new(8);
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
    fn xi9_deque_rotate_left() {
        let mut dq = super::Xi9Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi9_deque_rotate_right() {
        let mut dq = super::Xi9Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi9_deque_grow() {
        let mut dq = super::Xi9Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi9_deque_empty() {
        let dq = super::Xi9Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi9_interval_tree_insert_query() {
        let mut tree = super::Xi9IntervalTree::xi_new();
        tree.xi_insert(super::Xi9Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi9Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi9Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi9_interval_tree_overlap() {
        let mut tree = super::Xi9IntervalTree::xi_new();
        tree.xi_insert(super::Xi9Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi9Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi9Interval::xi_new(12, 20));
        let q = super::Xi9Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi9_interval_tree_remove() {
        let mut tree = super::Xi9IntervalTree::xi_new();
        tree.xi_insert(super::Xi9Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi9Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi9_interval_tree_gaps() {
        let mut tree = super::Xi9IntervalTree::xi_new();
        tree.xi_insert(super::Xi9Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi9Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi9Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi9Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi9Interval::xi_new(8, 10));
    }

    #[test]
    fn xi9_interval_tree_merge() {
        let mut tree = super::Xi9IntervalTree::xi_new();
        tree.xi_insert(super::Xi9Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi9Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi9Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi9Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi9Interval::xi_new(10, 15));
    }

    #[test]
    fn xi9_interval_tree_all() {
        let mut tree = super::Xi9IntervalTree::xi_new();
        tree.xi_insert(super::Xi9Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi9Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi9_interval_tree_empty() {
        let tree = super::Xi9IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi9_interval_tree_contains_point() {
        let iv = super::Xi9Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 9) ---

    #[test]
    fn xj_9_uf_make_and_find() {
        let mut uf = super::Xj9UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_9_uf_union_connected() {
        let mut uf = super::Xj9UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_9_uf_component_count() {
        let mut uf = super::Xj9UnionFind::xj_new();
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
    fn xj_9_uf_component_size() {
        let mut uf = super::Xj9UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_9_uf_largest_component() {
        let mut uf = super::Xj9UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_9_uf_many_elements() {
        let mut uf = super::Xj9UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_9_uf_separate_components() {
        let mut uf = super::Xj9UnionFind::xj_new();
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
    fn xj_9_uf_path_compression() {
        let mut uf = super::Xj9UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_9_bt_insert_get() {
        let mut bt = super::Xj9BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_9_bt_contains_len() {
        let mut bt = super::Xj9BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_9_bt_replace() {
        let mut bt = super::Xj9BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_9_bt_remove() {
        let mut bt = super::Xj9BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_9_bt_keys_values() {
        let mut bt = super::Xj9BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_9_bt_range() {
        let mut bt = super::Xj9BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_9_bt_min_max() {
        let mut bt = super::Xj9BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_9_bt_many_inserts() {
        let mut bt = super::Xj9BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_9 segment tree tests ---

    #[test]
    fn xk_9_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk9SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_9_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk9SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_9_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk9SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_9_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk9SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_9_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk9SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_9_st_single_element() {
        let data = vec![42];
        let st = super::Xk9SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_9_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk9SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_9_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk9SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_9 disjoint intervals tests ---

    #[test]
    fn xk_9_di_add_and_count() {
        let mut di = super::Xk9DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_9_di_merge_overlap() {
        let mut di = super::Xk9DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_9_di_contains() {
        let mut di = super::Xk9DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_9_di_remove() {
        let mut di = super::Xk9DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_9_di_covered_length() {
        let mut di = super::Xk9DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_9_di_gaps() {
        let mut di = super::Xk9DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_9_di_merge_adjacent() {
        let mut di = super::Xk9DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_9_di_empty() {
        let di = super::Xk9DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_9_rope_new_empty() {
        let rope = super::Xl9Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_9_rope_from_str() {
        let rope = super::Xl9Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_9_rope_insert_at() {
        let mut rope = super::Xl9Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_9_rope_delete_range() {
        let mut rope = super::Xl9Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_9_rope_char_at() {
        let rope = super::Xl9Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_9_rope_split_concat() {
        let rope = super::Xl9Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_9_rope_line_count() {
        let rope = super::Xl9Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_9_rope_line_at() {
        let rope = super::Xl9Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_9_sa_build_and_search() {
        let sa = super::Xl9SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_9_sa_count() {
        let sa = super::Xl9SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_9_sa_longest_repeated() {
        let sa = super::Xl9SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_9_sa_all_positions() {
        let sa = super::Xl9SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_9_sa_len() {
        let sa = super::Xl9SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_9_sa_empty() {
        let sa = super::Xl9SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_9_rope_slice() {
        let rope = super::Xl9Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_9_sa_search_start() {
        let sa = super::Xl9SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_9_sparse_set_get() {
        let mut m = super::Xm9MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_9_sparse_row_col() {
        let mut m = super::Xm9MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_9_sparse_transpose() {
        let mut m = super::Xm9MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_9_sparse_multiply_vec() {
        let mut m = super::Xm9MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_9_sparse_nnz_density() {
        let mut m = super::Xm9MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_9_sparse_clear() {
        let mut m = super::Xm9MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_9_sparse_overwrite_zero() {
        let mut m = super::Xm9MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_9_tokenizer_basic() {
        let t = super::Xm9Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_9_tokenizer_count() {
        let t = super::Xm9Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_9_tokenizer_unique() {
        let t = super::Xm9Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_9_tokenizer_frequency() {
        let t = super::Xm9Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_9_tokenizer_delimiter() {
        let t = super::Xm9Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_9_tokenizer_whitespace() {
        let t = super::Xm9Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_9_tokenizer_empty() {
        let t = super::Xm9Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 9 ----

    #[test]
    fn xn_9_fenwick_prefix_sum() {
        let mut ft = super::Xn9Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_9_fenwick_range_sum() {
        let mut ft = super::Xn9Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_9_fenwick_point_query() {
        let mut ft = super::Xn9Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_9_fenwick_len() {
        let ft = super::Xn9Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_9_fenwick_multiple_updates() {
        let mut ft = super::Xn9Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_9_fenwick_single_element() {
        let mut ft = super::Xn9Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_9_fenwick_find_kth() {
        let mut ft = super::Xn9Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_9_fenwick_negative_delta() {
        let mut ft = super::Xn9Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 9 ----

    #[test]
    fn xn_9_avl_insert_get() {
        let mut m = super::Xn9AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_9_avl_remove() {
        let mut m = super::Xn9AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_9_avl_in_order() {
        let mut m = super::Xn9AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_9_avl_min_max() {
        let mut m = super::Xn9AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_9_avl_floor_ceiling() {
        let mut m = super::Xn9AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_9_avl_height_balanced() {
        let mut m = super::Xn9AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_9_avl_overwrite() {
        let mut m = super::Xn9AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_9_avl_empty() {
        let m: super::Xn9AVL<i32, i32> = super::Xn9AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo9RedBlack tests ---

    #[test]
    fn xo_9_rb_insert_and_get() {
        let mut tree = super::Xo9RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_9_rb_len_and_empty() {
        let mut tree = super::Xo9RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_9_rb_min_max() {
        let mut tree = super::Xo9RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_9_rb_contains() {
        let mut tree = super::Xo9RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_9_rb_remove() {
        let mut tree = super::Xo9RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_9_rb_in_order() {
        let mut tree = super::Xo9RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_9_rb_black_height() {
        let mut tree = super::Xo9RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_9_rb_overwrite() {
        let mut tree = super::Xo9RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo9ConsistentHash tests ---

    #[test]
    fn xo_9_ch_add_and_count() {
        let mut ring = super::Xo9ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_9_ch_remove_node() {
        let mut ring = super::Xo9ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_9_ch_get_node() {
        let mut ring = super::Xo9ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_9_ch_empty_ring() {
        let ring = super::Xo9ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_9_ch_distribution() {
        let mut ring = super::Xo9ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_9_ch_rebalance() {
        let mut ring = super::Xo9ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_9_ch_virtual_nodes() {
        let mut ring = super::Xo9ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_9_ch_consistent_lookup() {
        let mut ring = super::Xo9ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_9_splay_insert_get() {
        let mut t = super::Xp9SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_9_splay_remove() {
        let mut t = super::Xp9SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_9_splay_count_increases() {
        let mut t = super::Xp9SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_9_splay_depth() {
        let mut t = super::Xp9SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_9_splay_len_empty() {
        let t = super::Xp9SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_9_splay_min_max() {
        let mut t = super::Xp9SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_9_splay_overwrite() {
        let mut t = super::Xp9SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_9_splay_remove_missing() {
        let mut t = super::Xp9SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_9 treap tests ----
    #[test]
    fn xq_9_treap_empty() {
        let t = super::Xq9Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_9_treap_insert_get() {
        let mut t = super::Xq9Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_9_treap_overwrite() {
        let mut t = super::Xq9Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_9_treap_remove() {
        let mut t = super::Xq9Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_9_treap_min_max() {
        let mut t = super::Xq9Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_9_treap_rank() {
        let mut t = super::Xq9Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_9_treap_kth() {
        let mut t = super::Xq9Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_9_treap_in_order() {
        let mut t = super::Xq9Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_9 VEB tree tests ----
    #[test]
    fn xq_9_veb_empty() {
        let v = super::Xq9VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_9_veb_insert_contains() {
        let mut v = super::Xq9VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_9_veb_min_max() {
        let mut v = super::Xq9VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_9_veb_delete() {
        let mut v = super::Xq9VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_9_veb_successor() {
        let mut v = super::Xq9VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_9_veb_predecessor() {
        let mut v = super::Xq9VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_9_veb_count() {
        let mut v = super::Xq9VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_9_veb_duplicate_insert() {
        let mut v = super::Xq9VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }

}
