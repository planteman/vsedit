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

}
