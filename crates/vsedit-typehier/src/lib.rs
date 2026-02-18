//! Type hierarchy view.

use std::collections::{HashMap, HashSet};
use std::fmt;

/// The kind of a symbol in the type hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Class,
    Interface,
    Struct,
    Enum,
    TypeParameter,
    Module,
}

/// A tag that can be applied to a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolTag {
    Deprecated,
}

/// An item in the type hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeHierarchyItem {
    pub name: String,
    pub kind: SymbolKind,
    pub uri: String,
    pub range_start_line: u32,
    pub range_start_col: u32,
    pub range_end_line: u32,
    pub range_end_col: u32,
    pub detail: Option<String>,
    pub tags: Vec<SymbolTag>,
}

impl TypeHierarchyItem {
    pub fn new(
        name: String,
        kind: SymbolKind,
        uri: String,
        range_start_line: u32,
        range_start_col: u32,
        range_end_line: u32,
        range_end_col: u32,
    ) -> Self {
        Self {
            name,
            kind,
            uri,
            range_start_line,
            range_start_col,
            range_end_line,
            range_end_col,
            detail: None,
            tags: Vec::new(),
        }
    }
}

/// Provides type hierarchy information for symbols.
pub trait TypeHierarchyProvider {
    /// Prepare the type hierarchy at the given position.
    fn prepare(&self, uri: &str, line: u32, col: u32) -> Option<Vec<TypeHierarchyItem>>;

    /// Return the supertypes of the given item.
    fn supertypes(&self, item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem>;

    /// Return the subtypes of the given item.
    fn subtypes(&self, item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem>;
}

// ---------------------------------------------------------------------------
// Display implementations
// ---------------------------------------------------------------------------

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            SymbolKind::Class => "Class",
            SymbolKind::Interface => "Interface",
            SymbolKind::Struct => "Struct",
            SymbolKind::Enum => "Enum",
            SymbolKind::TypeParameter => "TypeParameter",
            SymbolKind::Module => "Module",
        };
        f.write_str(name)
    }
}

impl fmt::Display for SymbolTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymbolTag::Deprecated => f.write_str("Deprecated"),
        }
    }
}

impl fmt::Display for TypeHierarchyItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} at {}", self.name, self.kind, self.uri)
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when resolving a type hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeHierarchyError {
    /// No type symbol was found at the requested position.
    NoTypeAtPosition,
    /// The underlying provider failed.
    ProviderFailed(String),
    /// A circular reference was detected in the hierarchy.
    CircularHierarchy(String),
}

impl fmt::Display for TypeHierarchyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeHierarchyError::NoTypeAtPosition => {
                write!(f, "no type symbol found at the given position")
            }
            TypeHierarchyError::ProviderFailed(msg) => {
                write!(f, "type hierarchy provider failed: {msg}")
            }
            TypeHierarchyError::CircularHierarchy(msg) => {
                write!(f, "circular type hierarchy detected: {msg}")
            }
        }
    }
}

impl std::error::Error for TypeHierarchyError {}

// ---------------------------------------------------------------------------
// Builder-style helpers and queries on TypeHierarchyItem
// ---------------------------------------------------------------------------

impl TypeHierarchyItem {
    /// Builder-style method to set the detail field.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Builder-style method to add a tag.
    pub fn with_tag(mut self, tag: SymbolTag) -> Self {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
        self
    }

    /// Returns `true` if the item is marked as deprecated.
    pub fn is_deprecated(&self) -> bool {
        self.tags.contains(&SymbolTag::Deprecated)
    }

    /// Returns `true` if the given `(line, col)` falls within this item's range.
    pub fn contains_position(&self, line: u32, col: u32) -> bool {
        if line < self.range_start_line || line > self.range_end_line {
            return false;
        }
        if line == self.range_start_line && col < self.range_start_col {
            return false;
        }
        if line == self.range_end_line && col > self.range_end_col {
            return false;
        }
        true
    }
}

// ---------------------------------------------------------------------------
// TypeTree – full hierarchy graph built from a provider
// ---------------------------------------------------------------------------

/// A graph-based representation of a full type hierarchy.
///
/// Types are identified by their `TypeHierarchyItem`. The tree stores
/// supertype and subtype edges and can answer ancestry / descendant queries.
#[derive(Debug, Clone)]
pub struct TypeTree {
    items: Vec<TypeHierarchyItem>,
    /// Maps item index → set of supertype indices.
    supertype_edges: HashMap<usize, HashSet<usize>>,
    /// Maps item index → set of subtype indices.
    subtype_edges: HashMap<usize, HashSet<usize>>,
}

impl TypeTree {
    /// Create an empty `TypeTree`.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            supertype_edges: HashMap::new(),
            subtype_edges: HashMap::new(),
        }
    }

    /// Add a type to the tree. Returns the index of the (possibly existing) item.
    pub fn add_type(&mut self, item: TypeHierarchyItem) -> usize {
        if let Some(idx) = self.items.iter().position(|i| i == &item) {
            return idx;
        }
        let idx = self.items.len();
        self.items.push(item);
        idx
    }

    /// Record that `supertype_idx` is a supertype of `item_idx`.
    pub fn add_supertype_edge(&mut self, item_idx: usize, supertype_idx: usize) {
        self.supertype_edges
            .entry(item_idx)
            .or_default()
            .insert(supertype_idx);
        self.subtype_edges
            .entry(supertype_idx)
            .or_default()
            .insert(item_idx);
    }

    /// Record that `subtype_idx` is a subtype of `item_idx`.
    pub fn add_subtype_edge(&mut self, item_idx: usize, subtype_idx: usize) {
        self.subtype_edges
            .entry(item_idx)
            .or_default()
            .insert(subtype_idx);
        self.supertype_edges
            .entry(subtype_idx)
            .or_default()
            .insert(item_idx);
    }

    /// Direct supertypes of the item at `idx`.
    pub fn get_supertypes(&self, idx: usize) -> Vec<&TypeHierarchyItem> {
        self.supertype_edges
            .get(&idx)
            .map(|set| set.iter().filter_map(|&i| self.items.get(i)).collect())
            .unwrap_or_default()
    }

    /// Direct subtypes of the item at `idx`.
    pub fn get_subtypes(&self, idx: usize) -> Vec<&TypeHierarchyItem> {
        self.subtype_edges
            .get(&idx)
            .map(|set| set.iter().filter_map(|&i| self.items.get(i)).collect())
            .unwrap_or_default()
    }

    /// All transitive ancestors (supertypes of supertypes …).
    pub fn all_ancestors(&self, idx: usize) -> Vec<&TypeHierarchyItem> {
        let mut visited = HashSet::new();
        let mut stack = vec![idx];
        while let Some(cur) = stack.pop() {
            if let Some(parents) = self.supertype_edges.get(&cur) {
                for &p in parents {
                    if visited.insert(p) {
                        stack.push(p);
                    }
                }
            }
        }
        visited
            .iter()
            .filter_map(|&i| self.items.get(i))
            .collect()
    }

    /// All transitive descendants (subtypes of subtypes …).
    pub fn all_descendants(&self, idx: usize) -> Vec<&TypeHierarchyItem> {
        let mut visited = HashSet::new();
        let mut stack = vec![idx];
        while let Some(cur) = stack.pop() {
            if let Some(children) = self.subtype_edges.get(&cur) {
                for &c in children {
                    if visited.insert(c) {
                        stack.push(c);
                    }
                }
            }
        }
        visited
            .iter()
            .filter_map(|&i| self.items.get(i))
            .collect()
    }

    /// Maximum depth from `idx` down through subtypes. A leaf has depth 0.
    pub fn depth(&self, idx: usize) -> usize {
        match self.subtype_edges.get(&idx) {
            Some(set) if !set.is_empty() => {}
            _ => return 0,
        }
        let mut seen = HashSet::new();
        seen.insert(idx);
        self.depth_inner(idx, &mut seen)
    }

    fn depth_inner(&self, idx: usize, seen: &mut HashSet<usize>) -> usize {
        let children = match self.subtype_edges.get(&idx) {
            Some(set) if !set.is_empty() => set,
            _ => return 0,
        };
        let mut max_child = 0;
        for &c in children {
            if seen.contains(&c) {
                continue; // avoid infinite recursion on cycles
            }
            seen.insert(c);
            let d = 1 + self.depth_inner(c, seen);
            if d > max_child {
                max_child = d;
            }
        }
        max_child
    }

    /// Return a reference to the item at `idx`, if it exists.
    pub fn get_item(&self, idx: usize) -> Option<&TypeHierarchyItem> {
        self.items.get(idx)
    }

    /// Find the index of a given item. Returns `None` if not found.
    pub fn find_index(&self, item: &TypeHierarchyItem) -> Option<usize> {
        self.items.iter().position(|i| i == item)
    }

    /// Return references to all items.
    pub fn all_items(&self) -> Vec<&TypeHierarchyItem> {
        self.items.iter().collect()
    }

    /// Return the total number of types in the tree.
    pub fn type_count(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if there is a cycle reachable from `idx` through
    /// supertype edges.
    pub fn has_circular_reference(&self, idx: usize) -> bool {
        let mut visited = HashSet::new();
        let mut stack = vec![idx];
        while let Some(cur) = stack.pop() {
            if !visited.insert(cur) {
                return true;
            }
            if let Some(parents) = self.supertype_edges.get(&cur) {
                for &p in parents {
                    stack.push(p);
                }
            }
        }
        false
    }
}

impl Default for TypeTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for typehier operations.
#[derive(Debug, Clone, PartialEq)]
pub struct TypehierStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl TypehierStats {
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
    pub fn merge(&mut self, other: &TypehierStats) {
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

impl Default for TypehierStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TypehierStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TypehierStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for typehier.
#[derive(Debug, Clone)]
pub struct TypehierValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl TypehierValidator {
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

impl Default for TypehierValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Renders a type hierarchy tree as indented text for display.
pub struct TypeHierarchyTree;

impl TypeHierarchyTree {
    /// Render the subtypes tree rooted at `idx` as an indented string.
    pub fn render_subtypes(tree: &TypeTree, idx: usize) -> String {
        let mut out = String::new();
        if let Some(item) = tree.get_item(idx) {
            out.push_str(&item.name);
            out.push('\n');
            Self::render_subtypes_inner(tree, idx, 1, &mut out, &mut HashSet::new());
        }
        out
    }

    fn render_subtypes_inner(
        tree: &TypeTree,
        idx: usize,
        depth: usize,
        out: &mut String,
        visited: &mut HashSet<usize>,
    ) {
        visited.insert(idx);
        let children = tree.get_subtypes(idx);
        for child in &children {
            if let Some(child_idx) = tree.find_index(child) {
                if visited.contains(&child_idx) {
                    continue;
                }
                for _ in 0..depth {
                    out.push_str("  ");
                }
                out.push_str(&child.name);
                out.push('\n');
                Self::render_subtypes_inner(tree, child_idx, depth + 1, out, visited);
            }
        }
    }

    /// Render the supertypes chain rooted at `idx` as an indented string.
    pub fn render_supertypes(tree: &TypeTree, idx: usize) -> String {
        let mut out = String::new();
        if let Some(item) = tree.get_item(idx) {
            out.push_str(&item.name);
            out.push('\n');
            Self::render_supertypes_inner(tree, idx, 1, &mut out, &mut HashSet::new());
        }
        out
    }

    fn render_supertypes_inner(
        tree: &TypeTree,
        idx: usize,
        depth: usize,
        out: &mut String,
        visited: &mut HashSet<usize>,
    ) {
        visited.insert(idx);
        let parents = tree.get_supertypes(idx);
        for parent in &parents {
            if let Some(parent_idx) = tree.find_index(parent) {
                if visited.contains(&parent_idx) {
                    continue;
                }
                for _ in 0..depth {
                    out.push_str("  ");
                }
                out.push_str(&parent.name);
                out.push('\n');
                Self::render_supertypes_inner(tree, parent_idx, depth + 1, out, visited);
            }
        }
    }
}

/// Walk the supertype chain from `idx` upward, returning items in order from child to root.
/// Stops if a cycle is detected.
pub fn resolve_type_chain(tree: &TypeTree, idx: usize) -> Vec<&TypeHierarchyItem> {
    let mut chain = Vec::new();
    let mut visited = HashSet::new();
    let mut current = idx;
    loop {
        if !visited.insert(current) {
            break; // cycle
        }
        if let Some(item) = tree.get_item(current) {
            chain.push(item);
        }
        let parents = tree.get_supertypes(current);
        if parents.is_empty() {
            break;
        }
        // Follow the first supertype (primary inheritance)
        match tree.find_index(parents[0]) {
            Some(pidx) => current = pidx,
            None => break,
        }
    }
    chain
}

/// Return a flat list of all items in the tree, sorted by name.
pub fn type_hierarchy_flatten(tree: &TypeTree) -> Vec<&TypeHierarchyItem> {
    let mut items = tree.all_items();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items
}

/// Return a flat list of all root types (types with no supertypes).
pub fn type_hierarchy_roots(tree: &TypeTree) -> Vec<&TypeHierarchyItem> {
    tree.all_items()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| tree.get_supertypes(*i).is_empty())
        .map(|(_, item)| item)
        .collect()
}

// ---------------------------------------------------------------------------
// SymbolKind extensions
// ---------------------------------------------------------------------------

impl SymbolKind {
    pub fn is_type(&self) -> bool {
        matches!(
            self,
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Enum | SymbolKind::Struct
        )
    }

    pub fn is_container(&self) -> bool {
        matches!(self, SymbolKind::Module)
    }

    pub fn label(&self) -> &'static str {
        match self {
            SymbolKind::Class => "class",
            SymbolKind::Interface => "interface",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::TypeParameter => "type parameter",
            SymbolKind::Module => "module",
        }
    }
}

// ---------------------------------------------------------------------------
// TypeHierarchyItem kind predicates
// ---------------------------------------------------------------------------

impl TypeHierarchyItem {
    pub fn is_class(&self) -> bool {
        self.kind == SymbolKind::Class
    }

    pub fn is_interface(&self) -> bool {
        self.kind == SymbolKind::Interface
    }

    pub fn is_struct(&self) -> bool {
        self.kind == SymbolKind::Struct
    }

    pub fn is_enum(&self) -> bool {
        self.kind == SymbolKind::Enum
    }

    pub fn is_module(&self) -> bool {
        self.kind == SymbolKind::Module
    }

    pub fn has_tags(&self) -> bool {
        !self.tags.is_empty()
    }

    pub fn has_detail(&self) -> bool {
        self.detail.is_some()
    }

    pub fn line_span(&self) -> u32 {
        self.range_end_line.saturating_sub(self.range_start_line)
    }
}

// ---------------------------------------------------------------------------
// TypeTree extensions
// ---------------------------------------------------------------------------

impl TypeTree {
    pub fn find_by_name(&self, name: &str) -> Option<(usize, &TypeHierarchyItem)> {
        self.items
            .iter()
            .enumerate()
            .find(|(_, item)| item.name == name)
    }

    pub fn flatten(&self) -> Vec<&TypeHierarchyItem> {
        self.items.iter().collect()
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn leaf_indices(&self) -> Vec<usize> {
        (0..self.items.len())
            .filter(|i| {
                self.subtype_edges
                    .get(i)
                    .map_or(true, |set| set.is_empty())
            })
            .collect()
    }

    pub fn leaf_count(&self) -> usize {
        self.leaf_indices().len()
    }

    pub fn root_indices(&self) -> Vec<usize> {
        (0..self.items.len())
            .filter(|i| {
                self.supertype_edges
                    .get(i)
                    .map_or(true, |set| set.is_empty())
            })
            .collect()
    }

    pub fn roots(&self) -> Vec<&TypeHierarchyItem> {
        self.root_indices()
            .iter()
            .filter_map(|&i| self.items.get(i))
            .collect()
    }

    pub fn leaves(&self) -> Vec<&TypeHierarchyItem> {
        self.leaf_indices()
            .iter()
            .filter_map(|&i| self.items.get(i))
            .collect()
    }

    pub fn edge_count(&self) -> usize {
        self.subtype_edges.values().map(|s| s.len()).sum()
    }

    pub fn bfs(&self, start: usize) -> Vec<&TypeHierarchyItem> {
        let mut visited = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut result = Vec::new();
        if start >= self.items.len() {
            return result;
        }
        visited.insert(start);
        queue.push_back(start);
        while let Some(cur) = queue.pop_front() {
            if let Some(item) = self.items.get(cur) {
                result.push(item);
            }
            if let Some(children) = self.subtype_edges.get(&cur) {
                let mut sorted: Vec<usize> = children.iter().copied().collect();
                sorted.sort_unstable();
                for c in sorted {
                    if visited.insert(c) {
                        queue.push_back(c);
                    }
                }
            }
        }
        result
    }

    pub fn contains_name(&self, name: &str) -> bool {
        self.items.iter().any(|item| item.name == name)
    }
}

// ---------------------------------------------------------------------------
// TypeHierarchyTree extensions
// ---------------------------------------------------------------------------

impl TypeHierarchyTree {
    pub fn leaf_count(tree: &TypeTree) -> usize {
        tree.leaf_count()
    }

    pub fn root(tree: &TypeTree) -> Option<&TypeHierarchyItem> {
        let roots = tree.roots();
        roots.into_iter().next()
    }
}

// ---------------------------------------------------------------------------
// TypehierStats extensions
// ---------------------------------------------------------------------------

impl TypehierStats {
    pub fn summary(&self) -> String {
        format!(
            "{} ops ({} ok, {} err) avg={} ns",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }

    pub fn has_failures(&self) -> bool {
        self.failed_operations > 0
    }

    pub fn is_empty(&self) -> bool {
        self.total_operations == 0
    }
}

// ---------------------------------------------------------------------------
// Display for TypeTree
// ---------------------------------------------------------------------------

impl fmt::Display for TypeTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TypeTree({} types, {} edges)",
            self.count(),
            self.edge_count()
        )
    }
}

// ---------------------------------------------------------------------------
// Shortest path between two types (BFS on undirected edges)
// ---------------------------------------------------------------------------

/// Find the shortest path between two types in the hierarchy, treating
/// supertype and subtype edges as undirected. Returns `None` if no path
/// exists. The returned vector includes both endpoints.
pub fn shortest_path(tree: &TypeTree, from: usize, to: usize) -> Option<Vec<usize>> {
    if from == to {
        return Some(vec![from]);
    }
    let n = tree.type_count();
    if from >= n || to >= n {
        return None;
    }
    let mut visited = HashSet::new();
    let mut parent: HashMap<usize, usize> = HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    visited.insert(from);
    queue.push_back(from);

    while let Some(cur) = queue.pop_front() {
        let mut neighbors = HashSet::new();
        if let Some(sups) = tree.supertype_edges.get(&cur) {
            neighbors.extend(sups);
        }
        if let Some(subs) = tree.subtype_edges.get(&cur) {
            neighbors.extend(subs);
        }
        for &nb in &neighbors {
            if visited.insert(nb) {
                parent.insert(nb, cur);
                if nb == to {
                    // reconstruct path
                    let mut path = vec![to];
                    let mut c = to;
                    while let Some(&p) = parent.get(&c) {
                        path.push(p);
                        c = p;
                    }
                    path.reverse();
                    return Some(path);
                }
                queue.push_back(nb);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Diamond inheritance detection
// ---------------------------------------------------------------------------

impl TypeTree {
    /// Detect diamond inheritance: a type that has two or more supertypes
    /// sharing a common ancestor. Returns indices of types involved in
    /// diamond patterns.
    pub fn detect_diamonds(&self) -> Vec<usize> {
        let mut diamonds = Vec::new();
        for idx in 0..self.items.len() {
            let direct_supers: Vec<usize> = self
                .supertype_edges
                .get(&idx)
                .map(|s| s.iter().copied().collect())
                .unwrap_or_default();
            if direct_supers.len() < 2 {
                continue;
            }
            // Collect transitive ancestors for each direct supertype
            let ancestor_sets: Vec<HashSet<usize>> = direct_supers
                .iter()
                .map(|&s| {
                    let mut anc = HashSet::new();
                    let mut stack = vec![s];
                    while let Some(c) = stack.pop() {
                        if let Some(parents) = self.supertype_edges.get(&c) {
                            for &p in parents {
                                if anc.insert(p) {
                                    stack.push(p);
                                }
                            }
                        }
                    }
                    anc.insert(s);
                    anc
                })
                .collect();
            // Check if any two ancestor sets share a common type
            'outer: for i in 0..ancestor_sets.len() {
                for j in (i + 1)..ancestor_sets.len() {
                    if !ancestor_sets[i].is_disjoint(&ancestor_sets[j]) {
                        diamonds.push(idx);
                        break 'outer;
                    }
                }
            }
        }
        diamonds
    }

    /// Count how many concrete types (classes/structs) implement each
    /// interface in the tree. Returns a map from interface index to count.
    pub fn interface_implementor_counts(&self) -> HashMap<usize, usize> {
        let mut counts = HashMap::new();
        for (idx, item) in self.items.iter().enumerate() {
            if item.kind != SymbolKind::Interface {
                continue;
            }
            let descendants = self.all_descendants(idx);
            let impl_count = descendants
                .iter()
                .filter(|d| d.kind != SymbolKind::Interface)
                .count();
            counts.insert(idx, impl_count);
        }
        counts
    }

    /// Compute breadth and depth statistics for the tree.
    /// Returns `(max_depth, max_breadth, avg_breadth)` where breadth is
    /// measured as the number of direct subtypes per node.
    pub fn depth_breadth_stats(&self) -> (usize, usize, f64) {
        let root_indices = self.root_indices();
        let mut max_depth: usize = 0;
        for &r in &root_indices {
            let d = self.depth(r);
            if d > max_depth {
                max_depth = d;
            }
        }

        let mut max_breadth: usize = 0;
        let mut total_breadth: usize = 0;
        let mut nodes_with_children: usize = 0;
        for idx in 0..self.items.len() {
            let b = self
                .subtype_edges
                .get(&idx)
                .map_or(0, |s| s.len());
            if b > 0 {
                nodes_with_children += 1;
                total_breadth += b;
                if b > max_breadth {
                    max_breadth = b;
                }
            }
        }
        let avg_breadth = if nodes_with_children > 0 {
            total_breadth as f64 / nodes_with_children as f64
        } else {
            0.0
        };
        (max_depth, max_breadth, avg_breadth)
    }

    /// Flatten the hierarchy into a topologically sorted list (supertypes
    /// before subtypes). Uses Kahn's algorithm. Returns `None` if the
    /// graph contains a cycle.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let n = self.items.len();
        let mut in_degree = vec![0usize; n];
        for (_, subs) in &self.subtype_edges {
            for &s in subs {
                if s < n {
                    in_degree[s] += 1;
                }
            }
        }
        let mut queue: std::collections::VecDeque<usize> =
            (0..n).filter(|&i| in_degree[i] == 0).collect();
        let mut sorted = Vec::with_capacity(n);
        while let Some(cur) = queue.pop_front() {
            sorted.push(cur);
            if let Some(subs) = self.subtype_edges.get(&cur) {
                for &s in subs {
                    if s < n {
                        in_degree[s] -= 1;
                        if in_degree[s] == 0 {
                            queue.push_back(s);
                        }
                    }
                }
            }
        }
        if sorted.len() == n {
            Some(sorted)
        } else {
            None // cycle detected
        }
    }

    /// Return all types that are isolated (no supertype or subtype edges).
    pub fn isolated_types(&self) -> Vec<usize> {
        (0..self.items.len())
            .filter(|i| {
                let no_sup = self
                    .supertype_edges
                    .get(i)
                    .map_or(true, |s| s.is_empty());
                let no_sub = self
                    .subtype_edges
                    .get(i)
                    .map_or(true, |s| s.is_empty());
                no_sup && no_sub
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// TypeHierarchySearch – query helpers for filtering items in a TypeTree
// ---------------------------------------------------------------------------

/// Provides search and filtering operations over a [`TypeTree`].
pub struct TypeHierarchySearch;

impl TypeHierarchySearch {
    /// Return indices of items whose name contains `query` (case-insensitive).
    pub fn search(tree: &TypeTree, query: &str) -> Vec<usize> {
        let lower = query.to_lowercase();
        tree.all_items()
            .iter()
            .enumerate()
            .filter(|(_, item)| item.name.to_lowercase().contains(&lower))
            .map(|(i, _)| i)
            .collect()
    }

    /// Return indices of items that match the given [`SymbolKind`].
    pub fn search_by_kind(tree: &TypeTree, kind: SymbolKind) -> Vec<usize> {
        tree.all_items()
            .iter()
            .enumerate()
            .filter(|(_, item)| item.kind == kind)
            .map(|(i, _)| i)
            .collect()
    }

    /// Return indices of items marked as deprecated.
    pub fn search_deprecated(tree: &TypeTree) -> Vec<usize> {
        tree.all_items()
            .iter()
            .enumerate()
            .filter(|(_, item)| item.is_deprecated())
            .map(|(i, _)| i)
            .collect()
    }

    /// Return indices of items that have a `detail` value.
    pub fn search_with_detail(tree: &TypeTree) -> Vec<usize> {
        tree.all_items()
            .iter()
            .enumerate()
            .filter(|(_, item)| item.detail.is_some())
            .map(|(i, _)| i)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// TypeHierarchyBreadcrumb – navigation path tracker
// ---------------------------------------------------------------------------

/// Tracks a navigation path through the type hierarchy as a stack of names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeHierarchyBreadcrumb {
    path: Vec<String>,
}

impl TypeHierarchyBreadcrumb {
    /// Create an empty breadcrumb trail.
    pub fn new() -> Self {
        Self { path: Vec::new() }
    }

    /// Push a name onto the breadcrumb trail.
    pub fn push(&mut self, name: &str) {
        self.path.push(name.to_string());
    }

    /// Pop the last name from the trail.
    pub fn pop(&mut self) -> Option<String> {
        self.path.pop()
    }

    /// Return the current (last) name in the trail.
    pub fn current(&self) -> Option<&str> {
        self.path.last().map(|s| s.as_str())
    }

    /// Return the depth of the breadcrumb trail.
    pub fn depth(&self) -> usize {
        self.path.len()
    }

    /// Return the full path joined with ` > `.
    pub fn full_path(&self) -> String {
        self.path.join(" > ")
    }

    /// Clear the breadcrumb trail.
    pub fn clear(&mut self) {
        self.path.clear();
    }

    /// Returns `true` if the breadcrumb trail is empty.
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }
}

impl Default for TypeHierarchyBreadcrumb {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TypeHierarchyBreadcrumb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.full_path())
    }
}

// ---------------------------------------------------------------------------
// TypeHierarchyStatistics – aggregate metrics for a TypeTree
// ---------------------------------------------------------------------------

/// Aggregate metrics computed from a [`TypeTree`].
#[derive(Debug, Clone, PartialEq)]
pub struct TypeHierarchyStatsResult {
    pub total_types: usize,
    pub class_count: usize,
    pub interface_count: usize,
    pub enum_count: usize,
    pub struct_count: usize,
    pub max_depth: usize,
    pub avg_children: f64,
    pub leaf_count: usize,
    pub root_count: usize,
}

/// Computes aggregate statistics for a [`TypeTree`].
pub struct TypeHierarchyStatistics;

impl TypeHierarchyStatistics {
    /// Compute statistics over the given tree.
    pub fn compute(tree: &TypeTree) -> TypeHierarchyStatsResult {
        let items = tree.all_items();
        let total_types = items.len();

        let mut class_count: usize = 0;
        let mut interface_count: usize = 0;
        let mut enum_count: usize = 0;
        let mut struct_count: usize = 0;

        for item in &items {
            match item.kind {
                SymbolKind::Class => class_count += 1,
                SymbolKind::Interface => interface_count += 1,
                SymbolKind::Enum => enum_count += 1,
                SymbolKind::Struct => struct_count += 1,
                _ => {}
            }
        }

        let mut max_depth: usize = 0;
        let mut total_children: usize = 0;
        let mut leaf_count: usize = 0;
        let mut root_count: usize = 0;

        for idx in 0..total_types {
            let subtypes = tree.get_subtypes(idx);
            let supertypes = tree.get_supertypes(idx);
            total_children += subtypes.len();
            if subtypes.is_empty() {
                leaf_count += 1;
            }
            if supertypes.is_empty() {
                root_count += 1;
            }
            let d = tree.depth(idx);
            if d > max_depth {
                max_depth = d;
            }
        }

        let avg_children = if total_types == 0 {
            0.0
        } else {
            total_children as f64 / total_types as f64
        };

        TypeHierarchyStatsResult {
            total_types,
            class_count,
            interface_count,
            enum_count,
            struct_count,
            max_depth,
            avg_children,
            leaf_count,
            root_count,
        }
    }
}

// ---------------------------------------------------------------------------
// TypeHierarchyExporter – render a TypeTree in various formats
// ---------------------------------------------------------------------------

/// Renders a [`TypeTree`] in various output formats.
pub struct TypeHierarchyExporter;

impl TypeHierarchyExporter {
    /// Render the tree as indented plain text.
    ///
    /// Root types (no supertypes) are printed at indentation level 0 and their
    /// subtypes are indented recursively.
    pub fn to_text(tree: &TypeTree) -> String {
        let items = tree.all_items();
        let count = items.len();
        if count == 0 {
            return String::new();
        }

        // Find root indices (items with no supertypes).
        let roots: Vec<usize> = (0..count)
            .filter(|&i| tree.get_supertypes(i).is_empty())
            .collect();

        let mut out = String::new();
        let mut visited = HashSet::new();
        for root in roots {
            Self::write_text_node(tree, root, 0, &mut visited, &mut out);
        }
        out
    }

    fn write_text_node(
        tree: &TypeTree,
        idx: usize,
        indent: usize,
        visited: &mut HashSet<usize>,
        out: &mut String,
    ) {
        if !visited.insert(idx) {
            return;
        }
        if let Some(item) = tree.get_item(idx) {
            for _ in 0..indent {
                out.push_str("  ");
            }
            out.push_str(&format!("{}: {}\n", item.kind, item.name));
            let mut children: Vec<usize> = tree
                .get_subtypes(idx)
                .iter()
                .filter_map(|child| tree.find_index(child))
                .collect();
            children.sort_unstable();
            for child_idx in children {
                Self::write_text_node(tree, child_idx, indent + 1, visited, out);
            }
        }
    }

    /// Render the tree as a Graphviz DOT digraph.
    pub fn to_dot(tree: &TypeTree) -> String {
        let items = tree.all_items();
        let mut out = String::from("digraph TypeHierarchy {\n");
        out.push_str("  rankdir=BT;\n");
        out.push_str("  node [shape=box];\n");

        for (i, item) in items.iter().enumerate() {
            let label = format!("{} ({})", item.name, item.kind);
            out.push_str(&format!("  n{i} [label=\"{label}\"];\n"));
        }

        for (i, _) in items.iter().enumerate() {
            for sup in tree.get_supertypes(i) {
                if let Some(si) = tree.find_index(sup) {
                    out.push_str(&format!("  n{i} -> n{si};\n"));
                }
            }
        }

        out.push_str("}\n");
        out
    }

    /// Return a flat list of `"Kind: Name"` strings for every item.
    pub fn to_list(tree: &TypeTree) -> Vec<String> {
        tree.all_items()
            .iter()
            .map(|item| format!("{}: {}", item.kind, item.name))
            .collect()
    }
}


// === Type Hierarchy Graph Layout ===

/// Type Hierarchy Graph Layout implementation.
#[derive(Debug, Clone)]
pub struct TypeHierarchyGraphLayout {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: TypeHierarchyGraphLayoutStats,
}

/// Statistics for TypeHierarchyGraphLayout.
#[derive(Debug, Clone, Default)]
pub struct TypeHierarchyGraphLayoutStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl TypeHierarchyGraphLayoutStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl TypeHierarchyGraphLayout {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: TypeHierarchyGraphLayoutStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &TypeHierarchyGraphLayoutStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for TypeHierarchyGraphLayout {
    fn default() -> Self {
        Self::new()
    }
}

// === Type Hierarchy Search Filter ===

/// Priority level for TypeHierarchySearchFilter items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TypeHierarchySearchFilterPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TypeHierarchySearchFilterPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for TypeHierarchySearchFilterPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Type Hierarchy Search Filter implementation.
#[derive(Debug, Clone)]
pub struct TypeHierarchySearchFilter {
    items: Vec<TypeHierarchySearchFilterItem>,
    max_items: usize,
    default_priority: TypeHierarchySearchFilterPriority,
}

/// A single item in TypeHierarchySearchFilter.
#[derive(Debug, Clone)]
pub struct TypeHierarchySearchFilterItem {
    pub id: String,
    pub label: String,
    pub priority: TypeHierarchySearchFilterPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl TypeHierarchySearchFilterItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: TypeHierarchySearchFilterPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: TypeHierarchySearchFilterPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl TypeHierarchySearchFilter {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: TypeHierarchySearchFilterPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: TypeHierarchySearchFilterItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<TypeHierarchySearchFilterItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&TypeHierarchySearchFilterItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: TypeHierarchySearchFilterPriority) -> Vec<&TypeHierarchySearchFilterItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TypeHierarchySearchFilterItem> {
        let mut sorted: Vec<&TypeHierarchySearchFilterItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&TypeHierarchySearchFilterItem> {
        let mut sorted: Vec<&TypeHierarchySearchFilterItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&TypeHierarchySearchFilterItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: TypeHierarchySearchFilterPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> TypeHierarchySearchFilterPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &TypeHierarchySearchFilterItem> {
        self.items.iter()
    }
}

impl Default for TypeHierarchySearchFilter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// vsedit-typehier: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypehierXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl TypehierXConfig {
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

impl std::fmt::Display for TypehierXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct TypehierXRegistry {
    entries: Vec<TypehierXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl TypehierXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: TypehierXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&TypehierXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut TypehierXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<TypehierXConfig> {
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

    pub fn active_entries(&self) -> Vec<&TypehierXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&TypehierXConfig> {
        let mut sorted: Vec<&TypehierXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&TypehierXConfig> {
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

    pub fn iter(&self) -> TypehierXIterator<'_> {
        TypehierXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct TypehierXIterator<'a> {
    inner: std::slice::Iter<'a, TypehierXConfig>,
}

impl<'a> Iterator for TypehierXIterator<'a> {
    type Item = &'a TypehierXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct TypehierXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl TypehierXCache {
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
pub struct TypehierXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl TypehierXFormatter {
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

    pub fn format_entry(&self, entry: &TypehierXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &TypehierXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &TypehierXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for TypehierXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct TypehierXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl TypehierXValidator {
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

    pub fn validate(&self, entry: &TypehierXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &TypehierXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for TypehierXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for typehier
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaTypehierRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaTypehierRingBuf {
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
pub struct XaTypehierCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaTypehierCounter {
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

impl Default for XaTypehierCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 188
// ---------------------------------------------------------------------------

/// Generic object pool `Xc188Pool<T>`.
pub struct Xc188Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc188Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc188PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc188Pool<T> {
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
    pub fn stats(&self) -> Xc188PoolStats {
        Xc188PoolStats {
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

impl<T> Default for Xc188Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc188Scheduler`.
pub struct Xc188Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc188Scheduler {
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

impl Default for Xc188Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_188 hash for the given byte slice.
pub fn xc_188_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_188 convention.
pub fn xc_188_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_28 deepening: state machine + event bus ---

/// States for the Xd28 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd28State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd28State {
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
pub struct Xd28Transition {
    pub from: Xd28State,
    pub to: Xd28State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd28StateMachine {
    current: Xd28State,
    history: Vec<Xd28Transition>,
    step_counter: usize,
}

impl Xd28StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd28State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd28State {
        self.current
    }

    pub fn history(&self) -> &[Xd28Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd28State) -> Result<Xd28State, String> {
        let allowed = match (self.current, target) {
            (Xd28State::Idle, Xd28State::Running) => true,
            (Xd28State::Running, Xd28State::Paused) => true,
            (Xd28State::Running, Xd28State::Done) => true,
            (Xd28State::Paused, Xd28State::Running) => true,
            (Xd28State::Paused, Xd28State::Done) => true,
            (Xd28State::Done, Xd28State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_28: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd28Transition {
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
            "Xd28SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd28State> {
        let prefix = "Xd28SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd28State::Idle),
            "Running" => Some(Xd28State::Running),
            "Paused" => Some(Xd28State::Paused),
            "Done" => Some(Xd28State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd28State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd28 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd28Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd28Event {
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

type Xd28HandlerFn = Box<dyn Fn(&Xd28Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd28EventBus {
    handlers: Vec<(usize, Option<String>, Xd28HandlerFn)>,
    next_id: usize,
    published: Vec<Xd28Event>,
}

impl Xd28EventBus {
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
        F: Fn(&Xd28Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd28Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd28Event) {
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

    pub fn published_events(&self) -> &[Xd28Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #26
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf26Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf26TrieNode {
    children: std::collections::HashMap<char, Xf26TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf26Trie {
    root: Xf26TrieNode,
    count: usize,
}

impl Xf26Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf26TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf26TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf26TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf26BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf26BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 187).
pub struct Xh187SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh187SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 229 as u64,
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

/// A compact bit set supporting boolean operations (variant 187).
pub struct Xh187BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh187BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 187).
pub struct Xi187Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi187Deque<T> {
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
pub struct Xi187Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi187Interval {
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

/// A simple interval tree (variant 187).
pub struct Xi187IntervalTree {
    xi_intervals: Vec<Xi187Interval>,
}

impl Xi187IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi187Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi187Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi187Interval) -> Vec<&Xi187Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi187Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi187Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi187Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi187Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi187Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi187Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 187) ---

/// Disjoint set / union-find for crate 187.
pub struct Xj187UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj187UnionFind {
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

const XJ187_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 187.
pub struct Xj187BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj187BTreeNode<K, V>>>,
    len: usize,
}

struct Xj187BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj187BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj187BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ187_BTREE_ORDER - 1
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
        let mid = XJ187_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj187BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj187BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj187BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj187BTreeNode::xj_new_leaf();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item() -> TypeHierarchyItem {
        TypeHierarchyItem::new(
            "MyClass".into(),
            SymbolKind::Class,
            "file:///src/main.rs".into(),
            10,
            0,
            20,
            1,
        )
    }

    #[test]
    fn new_item_has_defaults() {
        let item = sample_item();
        assert_eq!(item.name, "MyClass");
        assert_eq!(item.kind, SymbolKind::Class);
        assert!(item.detail.is_none());
        assert!(item.tags.is_empty());
    }

    #[test]
    fn item_with_detail_and_tags() {
        let mut item = sample_item();
        item.detail = Some("module::MyClass".into());
        item.tags.push(SymbolTag::Deprecated);
        assert_eq!(item.detail.as_deref(), Some("module::MyClass"));
        assert_eq!(item.tags, vec![SymbolTag::Deprecated]);
    }

    struct DummyProvider;

    impl TypeHierarchyProvider for DummyProvider {
        fn prepare(&self, _uri: &str, _line: u32, _col: u32) -> Option<Vec<TypeHierarchyItem>> {
            Some(vec![sample_item()])
        }

        fn supertypes(&self, _item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem> {
            vec![TypeHierarchyItem::new(
                "BaseClass".into(),
                SymbolKind::Class,
                "file:///src/base.rs".into(),
                1,
                0,
                5,
                1,
            )]
        }

        fn subtypes(&self, _item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem> {
            vec![]
        }
    }

    #[test]
    fn provider_prepare_and_supertypes() {
        let provider = DummyProvider;
        let items = provider.prepare("file:///src/main.rs", 10, 0).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "MyClass");

        let supers = provider.supertypes(&items[0]);
        assert_eq!(supers.len(), 1);
        assert_eq!(supers[0].name, "BaseClass");

        let subs = provider.subtypes(&items[0]);
        assert!(subs.is_empty());
    }

    // -----------------------------------------------------------------------
    // Display impls
    // -----------------------------------------------------------------------

    #[test]
    fn display_symbol_kind() {
        assert_eq!(SymbolKind::Class.to_string(), "Class");
        assert_eq!(SymbolKind::Interface.to_string(), "Interface");
        assert_eq!(SymbolKind::Struct.to_string(), "Struct");
        assert_eq!(SymbolKind::Enum.to_string(), "Enum");
        assert_eq!(SymbolKind::TypeParameter.to_string(), "TypeParameter");
        assert_eq!(SymbolKind::Module.to_string(), "Module");
    }

    #[test]
    fn display_symbol_tag() {
        assert_eq!(SymbolTag::Deprecated.to_string(), "Deprecated");
    }

    #[test]
    fn display_type_hierarchy_item() {
        let item = sample_item();
        assert_eq!(item.to_string(), "MyClass: Class at file:///src/main.rs");
    }

    // -----------------------------------------------------------------------
    // Error display
    // -----------------------------------------------------------------------

    #[test]
    fn display_errors() {
        let e1 = TypeHierarchyError::NoTypeAtPosition;
        assert_eq!(e1.to_string(), "no type symbol found at the given position");

        let e2 = TypeHierarchyError::ProviderFailed("timeout".into());
        assert_eq!(e2.to_string(), "type hierarchy provider failed: timeout");

        let e3 = TypeHierarchyError::CircularHierarchy("A -> B -> A".into());
        assert_eq!(
            e3.to_string(),
            "circular type hierarchy detected: A -> B -> A"
        );
    }

    // -----------------------------------------------------------------------
    // Builder methods and queries
    // -----------------------------------------------------------------------

    #[test]
    fn builder_with_detail() {
        let item = sample_item().with_detail("some::path::MyClass");
        assert_eq!(item.detail.as_deref(), Some("some::path::MyClass"));
    }

    #[test]
    fn builder_with_tag_and_is_deprecated() {
        let item = sample_item().with_tag(SymbolTag::Deprecated);
        assert!(item.is_deprecated());

        let plain = sample_item();
        assert!(!plain.is_deprecated());
    }

    #[test]
    fn with_tag_does_not_duplicate() {
        let item = sample_item()
            .with_tag(SymbolTag::Deprecated)
            .with_tag(SymbolTag::Deprecated);
        assert_eq!(item.tags.len(), 1);
    }

    #[test]
    fn contains_position_inside() {
        let item = sample_item(); // lines 10..20
        assert!(item.contains_position(10, 0));
        assert!(item.contains_position(15, 5));
        assert!(item.contains_position(20, 1));
    }

    #[test]
    fn contains_position_outside() {
        let item = sample_item();
        assert!(!item.contains_position(9, 0));
        assert!(!item.contains_position(21, 0));
        assert!(!item.contains_position(10, 0).then(|| ()).is_none()); // inside, double-check
        // before start col on start line
        let item2 = TypeHierarchyItem::new("X".into(), SymbolKind::Struct, "f".into(), 5, 3, 5, 10);
        assert!(!item2.contains_position(5, 2));
        // after end col on end line
        assert!(!item2.contains_position(5, 11));
    }

    // -----------------------------------------------------------------------
    // TypeTree
    // -----------------------------------------------------------------------

    fn make_item(name: &str, kind: SymbolKind) -> TypeHierarchyItem {
        TypeHierarchyItem::new(name.into(), kind, "file:///test".into(), 0, 0, 0, 0)
    }

    #[test]
    fn type_tree_add_and_query() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("A", SymbolKind::Class));
        let b = tree.add_type(make_item("B", SymbolKind::Class));
        let c = tree.add_type(make_item("C", SymbolKind::Class));

        // B extends A, C extends B
        tree.add_supertype_edge(b, a);
        tree.add_supertype_edge(c, b);

        assert_eq!(tree.get_supertypes(b).len(), 1);
        assert_eq!(tree.get_supertypes(b)[0].name, "A");

        assert_eq!(tree.get_subtypes(a).len(), 1);
        assert_eq!(tree.get_subtypes(a)[0].name, "B");
    }

    #[test]
    fn type_tree_all_ancestors_and_descendants() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("A", SymbolKind::Interface));
        let b = tree.add_type(make_item("B", SymbolKind::Class));
        let c = tree.add_type(make_item("C", SymbolKind::Class));
        let d = tree.add_type(make_item("D", SymbolKind::Class));

        tree.add_supertype_edge(b, a);
        tree.add_supertype_edge(c, b);
        tree.add_subtype_edge(c, d);

        let ancestors_of_c: Vec<String> =
            tree.all_ancestors(c).iter().map(|i| i.name.clone()).collect();
        assert!(ancestors_of_c.contains(&"A".to_string()));
        assert!(ancestors_of_c.contains(&"B".to_string()));
        assert_eq!(ancestors_of_c.len(), 2);

        let descendants_of_a: Vec<String> = tree
            .all_descendants(a)
            .iter()
            .map(|i| i.name.clone())
            .collect();
        assert!(descendants_of_a.contains(&"B".to_string()));
        assert!(descendants_of_a.contains(&"C".to_string()));
        // D is a subtype of C, so also a descendant of A
        assert!(descendants_of_a.contains(&"D".to_string()));
    }

    #[test]
    fn type_tree_depth() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("Root", SymbolKind::Class));
        let b = tree.add_type(make_item("Mid", SymbolKind::Class));
        let c = tree.add_type(make_item("Leaf", SymbolKind::Class));

        tree.add_subtype_edge(a, b);
        tree.add_subtype_edge(b, c);

        assert_eq!(tree.depth(a), 2);
        assert_eq!(tree.depth(b), 1);
        assert_eq!(tree.depth(c), 0);
    }

    #[test]
    fn type_tree_circular_detection() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("A", SymbolKind::Class));
        let b = tree.add_type(make_item("B", SymbolKind::Class));

        tree.add_supertype_edge(a, b);
        tree.add_supertype_edge(b, a); // cycle!

        assert!(tree.has_circular_reference(a));
        assert!(tree.has_circular_reference(b));

        // A tree without cycles
        let mut clean = TypeTree::new();
        let x = clean.add_type(make_item("X", SymbolKind::Struct));
        let y = clean.add_type(make_item("Y", SymbolKind::Struct));
        clean.add_supertype_edge(y, x);
        assert!(!clean.has_circular_reference(y));
        assert!(!clean.has_circular_reference(x));
    }

    #[test]
    fn typehier_stats_new_defaults() {
        let stats = TypehierStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn typehier_stats_record_success() {
        let mut stats = TypehierStats::new();
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
    fn typehier_stats_record_failure() {
        let mut stats = TypehierStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn typehier_stats_reset() {
        let mut stats = TypehierStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn typehier_stats_merge() {
        let mut a = TypehierStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = TypehierStats::new();
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
    fn typehier_stats_display() {
        let mut stats = TypehierStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn typehier_stats_default() {
        let stats = TypehierStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn typehier_validator_accepts_valid_name() {
        let v = TypehierValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn typehier_validator_rejects_empty() {
        let v = TypehierValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn typehier_validator_rejects_too_long() {
        let v = TypehierValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn typehier_validator_forbidden_prefix() {
        let v = TypehierValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn typehier_validator_allowed_chars() {
        let v = TypehierValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn typehier_validator_range() {
        let v = TypehierValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn typehier_sanitize_removes_control() {
        let result = TypehierValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn typehier_truncate_short_string() {
        assert_eq!(TypehierValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn typehier_truncate_long_string() {
        let result = TypehierValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn typehier_is_ascii_printable() {
        assert!(TypehierValidator::is_ascii_printable("Hello World 123"));
        assert!(!TypehierValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn type_tree_get_item() {
        let mut tree = TypeTree::new();
        let idx = tree.add_type(make_item("A", SymbolKind::Class));
        assert_eq!(tree.get_item(idx).unwrap().name, "A");
        assert!(tree.get_item(999).is_none());
    }

    #[test]
    fn type_tree_find_index() {
        let mut tree = TypeTree::new();
        let item = make_item("B", SymbolKind::Class);
        let idx = tree.add_type(item.clone());
        assert_eq!(tree.find_index(&item), Some(idx));
    }

    #[test]
    fn type_tree_all_items() {
        let mut tree = TypeTree::new();
        tree.add_type(make_item("A", SymbolKind::Class));
        tree.add_type(make_item("B", SymbolKind::Class));
        assert_eq!(tree.all_items().len(), 2);
        assert_eq!(tree.type_count(), 2);
    }

    #[test]
    fn render_subtypes_tree() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("Animal", SymbolKind::Class));
        let d = tree.add_type(make_item("Dog", SymbolKind::Class));
        let c = tree.add_type(make_item("Cat", SymbolKind::Class));
        tree.add_subtype_edge(a, d);
        tree.add_subtype_edge(a, c);
        let output = TypeHierarchyTree::render_subtypes(&tree, a);
        assert!(output.contains("Animal"));
        assert!(output.contains("  Dog") || output.contains("  Cat"));
    }

    #[test]
    fn render_supertypes_tree() {
        let mut tree = TypeTree::new();
        let base = tree.add_type(make_item("Object", SymbolKind::Class));
        let mid = tree.add_type(make_item("Animal", SymbolKind::Class));
        let leaf = tree.add_type(make_item("Dog", SymbolKind::Class));
        tree.add_supertype_edge(mid, base);
        tree.add_supertype_edge(leaf, mid);
        let output = TypeHierarchyTree::render_supertypes(&tree, leaf);
        assert!(output.contains("Dog"));
        assert!(output.contains("  Animal"));
    }

    #[test]
    fn resolve_type_chain_linear() {
        let mut tree = TypeTree::new();
        let obj = tree.add_type(make_item("Object", SymbolKind::Class));
        let animal = tree.add_type(make_item("Animal", SymbolKind::Class));
        let dog = tree.add_type(make_item("Dog", SymbolKind::Class));
        tree.add_supertype_edge(animal, obj);
        tree.add_supertype_edge(dog, animal);
        let chain = resolve_type_chain(&tree, dog);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].name, "Dog");
        assert_eq!(chain[1].name, "Animal");
        assert_eq!(chain[2].name, "Object");
    }

    #[test]
    fn type_hierarchy_flatten_sorted() {
        let mut tree = TypeTree::new();
        tree.add_type(make_item("Zebra", SymbolKind::Class));
        tree.add_type(make_item("Apple", SymbolKind::Class));
        tree.add_type(make_item("Mango", SymbolKind::Class));
        let flat = type_hierarchy_flatten(&tree);
        assert_eq!(flat[0].name, "Apple");
        assert_eq!(flat[1].name, "Mango");
        assert_eq!(flat[2].name, "Zebra");
    }

    #[test]
    fn type_hierarchy_roots_detection() {
        let mut tree = TypeTree::new();
        let root = tree.add_type(make_item("Object", SymbolKind::Class));
        let child = tree.add_type(make_item("Animal", SymbolKind::Class));
        tree.add_supertype_edge(child, root);
        let roots = type_hierarchy_roots(&tree);
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "Object");
    }

    // -----------------------------------------------------------------------
    // New functionality tests
    // -----------------------------------------------------------------------

    #[test]
    fn symbol_kind_is_type_and_label() {
        assert!(SymbolKind::Class.is_type());
        assert!(SymbolKind::Interface.is_type());
        assert!(SymbolKind::Struct.is_type());
        assert!(SymbolKind::Enum.is_type());
        assert!(!SymbolKind::Module.is_type());
        assert!(!SymbolKind::TypeParameter.is_type());

        assert!(SymbolKind::Module.is_container());
        assert!(!SymbolKind::Class.is_container());

        assert_eq!(SymbolKind::Class.label(), "class");
        assert_eq!(SymbolKind::Interface.label(), "interface");
        assert_eq!(SymbolKind::Struct.label(), "struct");
        assert_eq!(SymbolKind::Enum.label(), "enum");
        assert_eq!(SymbolKind::TypeParameter.label(), "type parameter");
        assert_eq!(SymbolKind::Module.label(), "module");
    }

    #[test]
    fn item_kind_predicates() {
        let cls = make_item("C", SymbolKind::Class);
        assert!(cls.is_class());
        assert!(!cls.is_interface());
        assert!(!cls.is_struct());
        assert!(!cls.is_enum());
        assert!(!cls.is_module());

        let iface = make_item("I", SymbolKind::Interface);
        assert!(iface.is_interface());

        let st = make_item("S", SymbolKind::Struct);
        assert!(st.is_struct());

        let en = make_item("E", SymbolKind::Enum);
        assert!(en.is_enum());

        let m = make_item("M", SymbolKind::Module);
        assert!(m.is_module());
    }

    #[test]
    fn item_has_tags_and_detail() {
        let plain = make_item("X", SymbolKind::Class);
        assert!(!plain.has_tags());
        assert!(!plain.has_detail());
        assert_eq!(plain.line_span(), 0);

        let tagged = plain.with_tag(SymbolTag::Deprecated).with_detail("detail");
        assert!(tagged.has_tags());
        assert!(tagged.has_detail());
    }

    #[test]
    fn type_tree_find_by_name_and_contains() {
        let mut tree = TypeTree::new();
        tree.add_type(make_item("Alpha", SymbolKind::Class));
        tree.add_type(make_item("Beta", SymbolKind::Interface));

        let (idx, item) = tree.find_by_name("Beta").unwrap();
        assert_eq!(idx, 1);
        assert_eq!(item.name, "Beta");

        assert!(tree.find_by_name("Gamma").is_none());
        assert!(tree.contains_name("Alpha"));
        assert!(!tree.contains_name("Gamma"));
    }

    #[test]
    fn type_tree_leaves_roots_and_bfs() {
        let mut tree = TypeTree::new();
        let root = tree.add_type(make_item("Root", SymbolKind::Class));
        let mid = tree.add_type(make_item("Mid", SymbolKind::Class));
        let leaf1 = tree.add_type(make_item("Leaf1", SymbolKind::Class));
        let leaf2 = tree.add_type(make_item("Leaf2", SymbolKind::Class));
        tree.add_subtype_edge(root, mid);
        tree.add_subtype_edge(mid, leaf1);
        tree.add_subtype_edge(mid, leaf2);

        assert_eq!(tree.leaf_count(), 2);
        let leaf_names: Vec<&str> = tree.leaves().iter().map(|i| i.name.as_str()).collect();
        assert!(leaf_names.contains(&"Leaf1"));
        assert!(leaf_names.contains(&"Leaf2"));

        let root_items = tree.roots();
        assert_eq!(root_items.len(), 1);
        assert_eq!(root_items[0].name, "Root");

        let bfs_items = tree.bfs(root);
        assert_eq!(bfs_items.len(), 4);
        assert_eq!(bfs_items[0].name, "Root");

        assert_eq!(tree.edge_count(), 3);
        assert!(!tree.is_empty());
        assert_eq!(tree.count(), 4);
    }

    #[test]
    fn type_tree_display_and_flatten() {
        let mut tree = TypeTree::new();
        tree.add_type(make_item("A", SymbolKind::Class));
        tree.add_type(make_item("B", SymbolKind::Struct));
        assert_eq!(format!("{tree}"), "TypeTree(2 types, 0 edges)");

        let flat = tree.flatten();
        assert_eq!(flat.len(), 2);

        assert_eq!(TypeHierarchyTree::leaf_count(&tree), 2);
        let root = TypeHierarchyTree::root(&tree);
        assert!(root.is_some());
    }

    #[test]
    fn typehier_stats_summary_and_flags() {
        let mut stats = TypehierStats::new();
        assert!(stats.is_empty());
        assert!(!stats.has_failures());

        stats.record_success(100);
        stats.record_failure(200);
        assert!(!stats.is_empty());
        assert!(stats.has_failures());

        let summary = stats.summary();
        assert!(summary.contains("2 ops"));
        assert!(summary.contains("1 ok"));
        assert!(summary.contains("1 err"));
    }

    // -----------------------------------------------------------------------
    // Shortest path, diamond detection, stats, topo sort, isolated types
    // -----------------------------------------------------------------------

    #[test]
    fn shortest_path_direct_edge() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("A", SymbolKind::Class));
        let b = tree.add_type(make_item("B", SymbolKind::Class));
        let c = tree.add_type(make_item("C", SymbolKind::Class));
        tree.add_supertype_edge(b, a);
        tree.add_supertype_edge(c, b);

        // A-B direct
        let path = shortest_path(&tree, a, b).unwrap();
        assert_eq!(path, vec![a, b]);

        // A-C through B
        let path2 = shortest_path(&tree, a, c).unwrap();
        assert_eq!(path2, vec![a, b, c]);

        // same node
        let path3 = shortest_path(&tree, a, a).unwrap();
        assert_eq!(path3, vec![a]);

        // unreachable
        let d = tree.add_type(make_item("D", SymbolKind::Class));
        assert!(shortest_path(&tree, a, d).is_none());

        // out of bounds
        assert!(shortest_path(&tree, 999, 0).is_none());
    }

    #[test]
    fn detect_diamond_inheritance() {
        //     Base
        //    /    \
        //  Left  Right
        //    \    /
        //    Bottom
        let mut tree = TypeTree::new();
        let base = tree.add_type(make_item("Base", SymbolKind::Class));
        let left = tree.add_type(make_item("Left", SymbolKind::Class));
        let right = tree.add_type(make_item("Right", SymbolKind::Class));
        let bottom = tree.add_type(make_item("Bottom", SymbolKind::Class));

        tree.add_supertype_edge(left, base);
        tree.add_supertype_edge(right, base);
        tree.add_supertype_edge(bottom, left);
        tree.add_supertype_edge(bottom, right);

        let diamonds = tree.detect_diamonds();
        assert!(diamonds.contains(&bottom));
        // Base, Left, Right have <2 supertypes so they are not diamond nodes
        assert!(!diamonds.contains(&base));
    }

    #[test]
    fn interface_implementor_counts() {
        let mut tree = TypeTree::new();
        let iface = tree.add_type(make_item("Drawable", SymbolKind::Interface));
        let cls1 = tree.add_type(make_item("Circle", SymbolKind::Class));
        let cls2 = tree.add_type(make_item("Square", SymbolKind::Class));
        let cls3 = tree.add_type(make_item("Triangle", SymbolKind::Class));

        tree.add_subtype_edge(iface, cls1);
        tree.add_subtype_edge(iface, cls2);
        tree.add_subtype_edge(iface, cls3);

        let counts = tree.interface_implementor_counts();
        assert_eq!(counts[&iface], 3);

        // A class should not appear in the counts map
        assert!(!counts.contains_key(&cls1));
    }

    #[test]
    fn depth_breadth_statistics() {
        //  Root -> Mid -> Leaf1
        //              -> Leaf2
        //              -> Leaf3
        let mut tree = TypeTree::new();
        let root = tree.add_type(make_item("Root", SymbolKind::Class));
        let mid = tree.add_type(make_item("Mid", SymbolKind::Class));
        let l1 = tree.add_type(make_item("L1", SymbolKind::Class));
        let l2 = tree.add_type(make_item("L2", SymbolKind::Class));
        let l3 = tree.add_type(make_item("L3", SymbolKind::Class));
        tree.add_subtype_edge(root, mid);
        tree.add_subtype_edge(mid, l1);
        tree.add_subtype_edge(mid, l2);
        tree.add_subtype_edge(mid, l3);

        let (max_depth, max_breadth, avg_breadth) = tree.depth_breadth_stats();
        assert_eq!(max_depth, 2);
        assert_eq!(max_breadth, 3); // Mid has 3 children
        // Root has 1 child, Mid has 3 → avg = (1+3)/2 = 2.0
        assert!((avg_breadth - 2.0).abs() < f64::EPSILON);

        // empty tree
        let empty = TypeTree::new();
        let (d, b, a) = empty.depth_breadth_stats();
        assert_eq!(d, 0);
        assert_eq!(b, 0);
        assert!((a - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn topological_sort_acyclic() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("A", SymbolKind::Class));
        let b = tree.add_type(make_item("B", SymbolKind::Class));
        let c = tree.add_type(make_item("C", SymbolKind::Class));
        tree.add_subtype_edge(a, b);
        tree.add_subtype_edge(b, c);

        let sorted = tree.topological_sort().unwrap();
        assert_eq!(sorted.len(), 3);
        // A must come before B, B before C
        let pos_a = sorted.iter().position(|&x| x == a).unwrap();
        let pos_b = sorted.iter().position(|&x| x == b).unwrap();
        let pos_c = sorted.iter().position(|&x| x == c).unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn topological_sort_detects_cycle() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("A", SymbolKind::Class));
        let b = tree.add_type(make_item("B", SymbolKind::Class));
        tree.add_subtype_edge(a, b);
        tree.add_subtype_edge(b, a);

        assert!(tree.topological_sort().is_none());
    }

    #[test]
    fn isolated_types_detection() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("Connected1", SymbolKind::Class));
        let b = tree.add_type(make_item("Connected2", SymbolKind::Class));
        let c = tree.add_type(make_item("Isolated", SymbolKind::Struct));
        tree.add_subtype_edge(a, b);

        let isolated = tree.isolated_types();
        assert_eq!(isolated, vec![c]);
    }

    // -----------------------------------------------------------------------
    // Tests for TypeHierarchySearch
    // -----------------------------------------------------------------------

    #[test]
    fn search_by_name_case_insensitive() {
        let mut tree = TypeTree::new();
        tree.add_type(make_item("FooBar", SymbolKind::Class));
        tree.add_type(make_item("Baz", SymbolKind::Interface));
        tree.add_type(make_item("fooQux", SymbolKind::Struct));

        let results = TypeHierarchySearch::search(&tree, "foo");
        assert_eq!(results, vec![0, 2]);
    }

    #[test]
    fn search_by_kind() {
        let mut tree = TypeTree::new();
        tree.add_type(make_item("A", SymbolKind::Class));
        tree.add_type(make_item("B", SymbolKind::Interface));
        tree.add_type(make_item("C", SymbolKind::Class));

        let classes = TypeHierarchySearch::search_by_kind(&tree, SymbolKind::Class);
        assert_eq!(classes, vec![0, 2]);
    }

    #[test]
    fn search_deprecated_items() {
        let mut tree = TypeTree::new();
        tree.add_type(make_item("Fresh", SymbolKind::Class));
        tree.add_type(make_item("Old", SymbolKind::Class).with_tag(SymbolTag::Deprecated));

        let deprecated = TypeHierarchySearch::search_deprecated(&tree);
        assert_eq!(deprecated, vec![1]);
    }

    #[test]
    fn search_with_detail() {
        let mut tree = TypeTree::new();
        tree.add_type(make_item("NoDetail", SymbolKind::Struct));
        tree.add_type(make_item("HasDetail", SymbolKind::Struct).with_detail("some info"));

        let detailed = TypeHierarchySearch::search_with_detail(&tree);
        assert_eq!(detailed, vec![1]);
    }

    // -----------------------------------------------------------------------
    // Tests for TypeHierarchyBreadcrumb
    // -----------------------------------------------------------------------

    #[test]
    fn breadcrumb_push_pop_current() {
        let mut bc = TypeHierarchyBreadcrumb::new();
        assert!(bc.is_empty());
        assert_eq!(bc.depth(), 0);
        assert_eq!(bc.current(), None);

        bc.push("Root");
        bc.push("Child");
        assert_eq!(bc.depth(), 2);
        assert_eq!(bc.current(), Some("Child"));

        assert_eq!(bc.pop(), Some("Child".to_string()));
        assert_eq!(bc.current(), Some("Root"));
    }

    #[test]
    fn breadcrumb_full_path_and_display() {
        let mut bc = TypeHierarchyBreadcrumb::new();
        bc.push("Object");
        bc.push("Animal");
        bc.push("Cat");

        assert_eq!(bc.full_path(), "Object > Animal > Cat");
        assert_eq!(format!("{bc}"), "Object > Animal > Cat");
    }

    #[test]
    fn breadcrumb_clear() {
        let mut bc = TypeHierarchyBreadcrumb::new();
        bc.push("A");
        bc.push("B");
        bc.clear();
        assert!(bc.is_empty());
        assert_eq!(bc.full_path(), "");
    }

    // -----------------------------------------------------------------------
    // Tests for TypeHierarchyStatistics
    // -----------------------------------------------------------------------

    #[test]
    fn statistics_empty_tree() {
        let tree = TypeTree::new();
        let stats = TypeHierarchyStatistics::compute(&tree);
        assert_eq!(stats.total_types, 0);
        assert_eq!(stats.leaf_count, 0);
        assert_eq!(stats.root_count, 0);
        assert!((stats.avg_children - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn statistics_mixed_tree() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("Base", SymbolKind::Class));
        let b = tree.add_type(make_item("Iface", SymbolKind::Interface));
        let c = tree.add_type(make_item("Leaf", SymbolKind::Enum));
        tree.add_subtype_edge(a, b);
        tree.add_subtype_edge(b, c);

        let stats = TypeHierarchyStatistics::compute(&tree);
        assert_eq!(stats.total_types, 3);
        assert_eq!(stats.class_count, 1);
        assert_eq!(stats.interface_count, 1);
        assert_eq!(stats.enum_count, 1);
        assert_eq!(stats.struct_count, 0);
        assert_eq!(stats.max_depth, 2);
        assert_eq!(stats.leaf_count, 1); // only "Leaf"
        assert_eq!(stats.root_count, 1); // only "Base"
    }

    // -----------------------------------------------------------------------
    // Tests for TypeHierarchyExporter
    // -----------------------------------------------------------------------

    #[test]
    fn exporter_to_list() {
        let mut tree = TypeTree::new();
        tree.add_type(make_item("Alpha", SymbolKind::Class));
        tree.add_type(make_item("Beta", SymbolKind::Interface));

        let list = TypeHierarchyExporter::to_list(&tree);
        assert_eq!(list, vec!["Class: Alpha", "Interface: Beta"]);
    }

    #[test]
    fn exporter_to_text() {
        let mut tree = TypeTree::new();
        let root = tree.add_type(make_item("Root", SymbolKind::Class));
        let child = tree.add_type(make_item("Child", SymbolKind::Struct));
        tree.add_subtype_edge(root, child);

        let text = TypeHierarchyExporter::to_text(&tree);
        assert!(text.contains("Class: Root"));
        assert!(text.contains("  Struct: Child"));
    }

    #[test]
    fn exporter_to_dot() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("A", SymbolKind::Class));
        let b = tree.add_type(make_item("B", SymbolKind::Class));
        tree.add_subtype_edge(a, b);

        let dot = TypeHierarchyExporter::to_dot(&tree);
        assert!(dot.starts_with("digraph TypeHierarchy {"));
        assert!(dot.contains("n0 [label=\"A (Class)\"]"));
        assert!(dot.contains("n1 -> n0;"));
        assert!(dot.ends_with("}\n"));
    }

    #[test]
    fn exporter_empty_tree() {
        let tree = TypeTree::new();
        assert_eq!(TypeHierarchyExporter::to_text(&tree), "");
        assert!(TypeHierarchyExporter::to_list(&tree).is_empty());
    }

    #[test]
    fn typeHierarchyGraphLayout_new() {
        let s = TypeHierarchyGraphLayout::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn typeHierarchyGraphLayout_add_contains() {
        let mut s = TypeHierarchyGraphLayout::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn typeHierarchyGraphLayout_add_duplicate() {
        let mut s = TypeHierarchyGraphLayout::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn typeHierarchyGraphLayout_remove() {
        let mut s = TypeHierarchyGraphLayout::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn typeHierarchyGraphLayout_capacity() {
        let s = TypeHierarchyGraphLayout::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn typeHierarchyGraphLayout_search() {
        let mut s = TypeHierarchyGraphLayout::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn typeHierarchyGraphLayout_stats() {
        let mut s = TypeHierarchyGraphLayout::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn typeHierarchySearchFilter_new() {
        let m = TypeHierarchySearchFilter::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn typeHierarchySearchFilter_add_find() {
        let mut m = TypeHierarchySearchFilter::new();
        m.add(TypeHierarchySearchFilterItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn typeHierarchySearchFilter_priority_filter() {
        let mut m = TypeHierarchySearchFilter::new();
        m.add(TypeHierarchySearchFilterItem::new("a", "A").with_priority(TypeHierarchySearchFilterPriority::High));
        m.add(TypeHierarchySearchFilterItem::new("b", "B").with_priority(TypeHierarchySearchFilterPriority::Low));
        m.add(TypeHierarchySearchFilterItem::new("c", "C").with_priority(TypeHierarchySearchFilterPriority::High));
        assert_eq!(m.by_priority(TypeHierarchySearchFilterPriority::High).len(), 2);
    }

    #[test]
    fn typeHierarchySearchFilter_remove() {
        let mut m = TypeHierarchySearchFilter::new();
        m.add(TypeHierarchySearchFilterItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn typeHierarchySearchFilter_search() {
        let mut m = TypeHierarchySearchFilter::new();
        m.add(TypeHierarchySearchFilterItem::new("id1", "Hello World"));
        m.add(TypeHierarchySearchFilterItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn typeHierarchySearchFilter_total_weight() {
        let mut m = TypeHierarchySearchFilter::new();
        m.add(TypeHierarchySearchFilterItem::new("a", "A").with_priority(TypeHierarchySearchFilterPriority::Critical));
        m.add(TypeHierarchySearchFilterItem::new("b", "B").with_priority(TypeHierarchySearchFilterPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn typeHierarchySearchFilter_capacity_limit() {
        let mut m = TypeHierarchySearchFilter::new().with_max_items(2);
        m.add(TypeHierarchySearchFilterItem::new("1", "one"));
        m.add(TypeHierarchySearchFilterItem::new("2", "two"));
        assert!(!m.add(TypeHierarchySearchFilterItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn typeHierarchySearchFilter_sorted_by_priority() {
        let mut m = TypeHierarchySearchFilter::new();
        m.add(TypeHierarchySearchFilterItem::new("lo", "Low").with_priority(TypeHierarchySearchFilterPriority::Low));
        m.add(TypeHierarchySearchFilterItem::new("hi", "High").with_priority(TypeHierarchySearchFilterPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn typeHierarchySearchFilter_item_metadata() {
        let mut item = TypeHierarchySearchFilterItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn typeHierarchyGraphLayout_enabled_toggle() {
        let mut s = TypeHierarchyGraphLayout::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn typeHierarchySearchFilter_priority_display() {
        assert_eq!(format!("{}", TypeHierarchySearchFilterPriority::High), "high");
        assert_eq!(format!("{}", TypeHierarchySearchFilterPriority::Low), "low");
    }


    #[test]
    fn typehier_x_config_new() {
        let c = TypehierXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn typehier_x_config_builder() {
        let c = TypehierXConfig::new("k")
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
    fn typehier_x_config_display() {
        let c = TypehierXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn typehier_x_registry_insert_get() {
        let mut reg = TypehierXRegistry::new();
        reg.insert(TypehierXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn typehier_x_registry_duplicate() {
        let mut reg = TypehierXRegistry::new();
        reg.insert(TypehierXConfig::new("a")).unwrap();
        assert!(reg.insert(TypehierXConfig::new("a")).is_err());
    }

    #[test]
    fn typehier_x_registry_remove() {
        let mut reg = TypehierXRegistry::new();
        reg.insert(TypehierXConfig::new("a")).unwrap();
        reg.insert(TypehierXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn typehier_x_registry_active_entries() {
        let mut reg = TypehierXRegistry::new();
        reg.insert(TypehierXConfig::new("a")).unwrap();
        reg.insert(TypehierXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn typehier_x_registry_by_weight() {
        let mut reg = TypehierXRegistry::new();
        reg.insert(TypehierXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(TypehierXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn typehier_x_registry_tags() {
        let mut reg = TypehierXRegistry::new();
        reg.insert(TypehierXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(TypehierXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn typehier_x_registry_total_weight() {
        let mut reg = TypehierXRegistry::new();
        reg.insert(TypehierXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(TypehierXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn typehier_x_registry_iterator() {
        let mut reg = TypehierXRegistry::new();
        reg.insert(TypehierXConfig::new("a")).unwrap();
        reg.insert(TypehierXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn typehier_x_cache_put_get() {
        let mut cache = TypehierXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn typehier_x_cache_eviction() {
        let mut cache = TypehierXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn typehier_x_cache_lru_order() {
        let mut cache = TypehierXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn typehier_x_cache_most_least_recent() {
        let mut cache = TypehierXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn typehier_x_formatter_entry() {
        let e = TypehierXConfig::new("k").with_value("v");
        let fmt = TypehierXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn typehier_x_formatter_summary() {
        let mut reg = TypehierXRegistry::new();
        reg.insert(TypehierXConfig::new("a").with_weight(5)).unwrap();
        let fmt = TypehierXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn typehier_x_validator_valid() {
        let v = TypehierXValidator::new();
        let c = TypehierXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn typehier_x_validator_empty_key() {
        let v = TypehierXValidator::new();
        let c = TypehierXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn typehier_x_validator_require_value() {
        let v = TypehierXValidator::new().require_value(true);
        let c = TypehierXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn typehier_x_validator_allowed_tags() {
        let v = TypehierXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = TypehierXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn typehier_x_validator_validate_all() {
        let v = TypehierXValidator::new();
        let mut reg = TypehierXRegistry::new();
        reg.insert(TypehierXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    // xa_ extended tests for typehier
    #[test]
    fn xa_typehier_ring_new() {
        let rb = super::XaTypehierRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_typehier_ring_push_len() {
        let mut rb = super::XaTypehierRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_typehier_ring_wrap() {
        let mut rb = super::XaTypehierRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_typehier_ring_mean_empty() {
        let rb = super::XaTypehierRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_typehier_ring_mean_values() {
        let mut rb = super::XaTypehierRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_typehier_ring_min_max() {
        let mut rb = super::XaTypehierRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_typehier_ring_iter() {
        let mut rb = super::XaTypehierRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_typehier_counter_new() {
        let c = super::XaTypehierCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_typehier_counter_inc() {
        let mut c = super::XaTypehierCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_typehier_counter_inc_by() {
        let mut c = super::XaTypehierCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_typehier_counter_reset() {
        let mut c = super::XaTypehierCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_typehier_counter_clear() {
        let mut c = super::XaTypehierCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_typehier_counter_default() {
        let c = super::XaTypehierCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 188 ----

    #[test]
    fn xc_188_pool_new_empty() {
        let pool: super::Xc188Pool<i32> = super::Xc188Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_188_pool_release_acquire() {
        let mut pool = super::Xc188Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_188_pool_acquire_empty() {
        let mut pool: super::Xc188Pool<i32> = super::Xc188Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_188_pool_full() {
        let mut pool = super::Xc188Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_188_pool_drain() {
        let mut pool = super::Xc188Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_188_pool_stats() {
        let mut pool = super::Xc188Pool::new(8);
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
    fn xc_188_pool_clear() {
        let mut pool = super::Xc188Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_188_pool_shrink() {
        let mut pool = super::Xc188Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_188_pool_default() {
        let pool: super::Xc188Pool<String> = super::Xc188Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_188_pool_extend() {
        let mut pool = super::Xc188Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_188_pool_retain() {
        let mut pool = super::Xc188Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_188_scheduler_round_robin() {
        let mut sched = super::Xc188Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_188_scheduler_empty() {
        let mut sched = super::Xc188Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_188_scheduler_reset() {
        let mut sched = super::Xc188Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_188_scheduler_add_remove() {
        let mut sched = super::Xc188Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_188_scheduler_targets() {
        let sched = super::Xc188Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_188_hash_empty() {
        assert_eq!(super::xc_188_hash(b""), 5381);
    }

    #[test]
    fn xc_188_hash_data() {
        let h = super::xc_188_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_188_hash(b"hello"), h);
    }

    #[test]
    fn xc_188_reverse_str() {
        assert_eq!(super::xc_188_reverse("abc"), "cba");
        assert_eq!(super::xc_188_reverse(""), "");
    }


    // --- xd_28 deepening tests ---

    #[test]
    fn xd_28_sm_initial_state() {
        let sm = Xd28StateMachine::new();
        assert_eq!(sm.current_state(), Xd28State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_28_sm_valid_idle_to_running() {
        let mut sm = Xd28StateMachine::new();
        assert!(sm.transition(Xd28State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd28State::Running);
    }

    #[test]
    fn xd_28_sm_valid_running_to_paused() {
        let mut sm = Xd28StateMachine::new();
        sm.transition(Xd28State::Running).unwrap();
        assert!(sm.transition(Xd28State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd28State::Paused);
    }

    #[test]
    fn xd_28_sm_valid_running_to_done() {
        let mut sm = Xd28StateMachine::new();
        sm.transition(Xd28State::Running).unwrap();
        assert!(sm.transition(Xd28State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd28State::Done);
    }

    #[test]
    fn xd_28_sm_valid_paused_to_running() {
        let mut sm = Xd28StateMachine::new();
        sm.transition(Xd28State::Running).unwrap();
        sm.transition(Xd28State::Paused).unwrap();
        assert!(sm.transition(Xd28State::Running).is_ok());
    }

    #[test]
    fn xd_28_sm_valid_done_to_idle() {
        let mut sm = Xd28StateMachine::new();
        sm.transition(Xd28State::Running).unwrap();
        sm.transition(Xd28State::Done).unwrap();
        assert!(sm.transition(Xd28State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd28State::Idle);
    }

    #[test]
    fn xd_28_sm_invalid_idle_to_done() {
        let mut sm = Xd28StateMachine::new();
        assert!(sm.transition(Xd28State::Done).is_err());
    }

    #[test]
    fn xd_28_sm_invalid_idle_to_paused() {
        let mut sm = Xd28StateMachine::new();
        assert!(sm.transition(Xd28State::Paused).is_err());
    }

    #[test]
    fn xd_28_sm_history_tracking() {
        let mut sm = Xd28StateMachine::new();
        sm.transition(Xd28State::Running).unwrap();
        sm.transition(Xd28State::Paused).unwrap();
        sm.transition(Xd28State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd28State::Idle);
        assert_eq!(sm.history()[0].to, Xd28State::Running);
        assert_eq!(sm.history()[1].from, Xd28State::Running);
        assert_eq!(sm.history()[2].to, Xd28State::Done);
    }

    #[test]
    fn xd_28_sm_serialize_deserialize() {
        let mut sm = Xd28StateMachine::new();
        sm.transition(Xd28State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd28StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd28State::Running));
    }

    #[test]
    fn xd_28_sm_deserialize_invalid() {
        assert_eq!(Xd28StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_28_sm_reset() {
        let mut sm = Xd28StateMachine::new();
        sm.transition(Xd28State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd28State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_28_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd28EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd28Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_28_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd28EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd28Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd28Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_28_bus_unsubscribe() {
        let mut bus = Xd28EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_28_event_kind_and_payload() {
        let e = Xd28Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd28Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_28_bus_clear_history() {
        let mut bus = Xd28EventBus::new();
        bus.publish(Xd28Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_28_sm_step_counter_increments() {
        let mut sm = Xd28StateMachine::new();
        sm.transition(Xd28State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd28State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #26 --

    #[test]
    fn xf26_trie_insert_search() {
        let mut t = Xf26Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf26_trie_starts_with() {
        let mut t = Xf26Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf26_trie_remove() {
        let mut t = Xf26Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf26_trie_word_count() {
        let mut t = Xf26Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf26_trie_longest_prefix() {
        let mut t = Xf26Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf26_trie_all_words() {
        let mut t = Xf26Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf26_trie_autocomplete() {
        let mut t = Xf26Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf26_trie_empty_search() {
        let t = Xf26Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf26_bloom_add_contains() {
        let mut bf = Xf26BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf26_bloom_probably_absent() {
        let bf = Xf26BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf26_bloom_false_positive_rate() {
        let mut bf = Xf26BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf26_bloom_clear() {
        let mut bf = Xf26BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf26_bloom_union() {
        let mut a = Xf26BloomFilter::xf_new(512, 2);
        let mut b = Xf26BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf26_bloom_intersection_estimate() {
        let mut a = Xf26BloomFilter::xf_new(512, 2);
        let mut b = Xf26BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf26_bloom_union_size_mismatch() {
        let a = Xf26BloomFilter::xf_new(256, 2);
        let b = Xf26BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh187_skip_insert_contains() {
        let mut sl = super::Xh187SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh187_skip_remove() {
        let mut sl = super::Xh187SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh187_skip_len() {
        let mut sl = super::Xh187SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh187_skip_range_query() {
        let mut sl = super::Xh187SkipList::xh_new(4);
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
    fn xh187_skip_floor_ceiling() {
        let mut sl = super::Xh187SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh187_skip_rank() {
        let mut sl = super::Xh187SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh187_skip_empty() {
        let sl = super::Xh187SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh187_skip_duplicates() {
        let mut sl = super::Xh187SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh187_bitset_set_test() {
        let mut bs = super::Xh187BitSet::xh_new(256);
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
    fn xh187_bitset_clear_count() {
        let mut bs = super::Xh187BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh187_bitset_and_or_xor() {
        let mut a = super::Xh187BitSet::xh_new(128);
        let mut b = super::Xh187BitSet::xh_new(128);
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
    fn xh187_bitset_iter_ones() {
        let mut bs = super::Xh187BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh187_bitset_first_last() {
        let mut bs = super::Xh187BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh187_bitset_empty() {
        let bs = super::Xh187BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi187_deque_push_pop_back() {
        let mut dq = super::Xi187Deque::xi_new(4);
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
    fn xi187_deque_push_pop_front() {
        let mut dq = super::Xi187Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi187_deque_mixed_ops() {
        let mut dq = super::Xi187Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi187_deque_get_and_split() {
        let mut dq = super::Xi187Deque::xi_new(8);
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
    fn xi187_deque_rotate_left() {
        let mut dq = super::Xi187Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi187_deque_rotate_right() {
        let mut dq = super::Xi187Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi187_deque_grow() {
        let mut dq = super::Xi187Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi187_deque_empty() {
        let dq = super::Xi187Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi187_interval_tree_insert_query() {
        let mut tree = super::Xi187IntervalTree::xi_new();
        tree.xi_insert(super::Xi187Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi187Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi187Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi187_interval_tree_overlap() {
        let mut tree = super::Xi187IntervalTree::xi_new();
        tree.xi_insert(super::Xi187Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi187Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi187Interval::xi_new(12, 20));
        let q = super::Xi187Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi187_interval_tree_remove() {
        let mut tree = super::Xi187IntervalTree::xi_new();
        tree.xi_insert(super::Xi187Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi187Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi187_interval_tree_gaps() {
        let mut tree = super::Xi187IntervalTree::xi_new();
        tree.xi_insert(super::Xi187Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi187Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi187Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi187Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi187Interval::xi_new(8, 10));
    }

    #[test]
    fn xi187_interval_tree_merge() {
        let mut tree = super::Xi187IntervalTree::xi_new();
        tree.xi_insert(super::Xi187Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi187Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi187Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi187Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi187Interval::xi_new(10, 15));
    }

    #[test]
    fn xi187_interval_tree_all() {
        let mut tree = super::Xi187IntervalTree::xi_new();
        tree.xi_insert(super::Xi187Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi187Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi187_interval_tree_empty() {
        let tree = super::Xi187IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi187_interval_tree_contains_point() {
        let iv = super::Xi187Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 187) ---

    #[test]
    fn xj_187_uf_make_and_find() {
        let mut uf = super::Xj187UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_187_uf_union_connected() {
        let mut uf = super::Xj187UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_187_uf_component_count() {
        let mut uf = super::Xj187UnionFind::xj_new();
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
    fn xj_187_uf_component_size() {
        let mut uf = super::Xj187UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_187_uf_largest_component() {
        let mut uf = super::Xj187UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_187_uf_many_elements() {
        let mut uf = super::Xj187UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_187_uf_separate_components() {
        let mut uf = super::Xj187UnionFind::xj_new();
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
    fn xj_187_uf_path_compression() {
        let mut uf = super::Xj187UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_187_bt_insert_get() {
        let mut bt = super::Xj187BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_187_bt_contains_len() {
        let mut bt = super::Xj187BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_187_bt_replace() {
        let mut bt = super::Xj187BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_187_bt_remove() {
        let mut bt = super::Xj187BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_187_bt_keys_values() {
        let mut bt = super::Xj187BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_187_bt_range() {
        let mut bt = super::Xj187BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_187_bt_min_max() {
        let mut bt = super::Xj187BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_187_bt_many_inserts() {
        let mut bt = super::Xj187BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }

}
