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
}
