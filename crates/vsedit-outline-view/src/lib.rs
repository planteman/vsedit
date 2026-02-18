//! Outline view (document structure).

use std::fmt;

/// Errors that can occur when operating on an outline model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutlineError {
    ElementNotFound(String),
    EmptyModel,
    InvalidRange { start: u32, end: u32 },
}

impl fmt::Display for OutlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutlineError::ElementNotFound(label) => write!(f, "element not found: {label}"),
            OutlineError::EmptyModel => write!(f, "outline model is empty"),
            OutlineError::InvalidRange { start, end } => {
                write!(f, "invalid range: {start}..{end}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutlineKind {
    File,
    Module,
    Namespace,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Struct,
    Event,
}

impl fmt::Display for OutlineKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            OutlineKind::File => "File",
            OutlineKind::Module => "Module",
            OutlineKind::Namespace => "Namespace",
            OutlineKind::Class => "Class",
            OutlineKind::Method => "Method",
            OutlineKind::Property => "Property",
            OutlineKind::Field => "Field",
            OutlineKind::Constructor => "Constructor",
            OutlineKind::Enum => "Enum",
            OutlineKind::Interface => "Interface",
            OutlineKind::Function => "Function",
            OutlineKind::Variable => "Variable",
            OutlineKind::Constant => "Constant",
            OutlineKind::String => "String",
            OutlineKind::Number => "Number",
            OutlineKind::Boolean => "Boolean",
            OutlineKind::Array => "Array",
            OutlineKind::Object => "Object",
            OutlineKind::Key => "Key",
            OutlineKind::Struct => "Struct",
            OutlineKind::Event => "Event",
        };
        write!(f, "{name}")
    }
}

#[derive(Debug, Clone)]
pub struct OutlineElement {
    pub label: String,
    pub detail: Option<String>,
    pub kind: OutlineKind,
    pub range_start_line: u32,
    pub range_end_line: u32,
    pub children: Vec<OutlineElement>,
}

impl fmt::Display for OutlineElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}) [{}-{}]",
            self.label, self.kind, self.range_start_line, self.range_end_line
        )
    }
}

impl OutlineElement {
    /// Builder method: append a child element and return self.
    pub fn with_child(mut self, child: OutlineElement) -> Self {
        self.children.push(child);
        self
    }

    /// Builder method: set the detail string and return self.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Clone)]
pub struct OutlineModel {
    pub elements: Vec<OutlineElement>,
    pub uri: String,
}

impl OutlineModel {
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            elements: Vec::new(),
            uri: uri.into(),
        }
    }

    pub fn add_element(&mut self, elem: OutlineElement) {
        self.elements.push(elem);
    }

    /// Returns all elements and their descendants in a flat list (pre-order).
    pub fn flatten(&self) -> Vec<&OutlineElement> {
        let mut result = Vec::new();
        fn collect<'a>(elems: &'a [OutlineElement], out: &mut Vec<&'a OutlineElement>) {
            for e in elems {
                out.push(e);
                collect(&e.children, out);
            }
        }
        collect(&self.elements, &mut result);
        result
    }

    /// Find the deepest element whose range contains the given line.
    pub fn find_at_line(&self, line: u32) -> Option<&OutlineElement> {
        fn search(elems: &[OutlineElement], line: u32) -> Option<&OutlineElement> {
            for e in elems {
                if line >= e.range_start_line && line <= e.range_end_line {
                    if let Some(child) = search(&e.children, line) {
                        return Some(child);
                    }
                    return Some(e);
                }
            }
            None
        }
        search(&self.elements, line)
    }

    /// Total count of all elements including nested children.
    pub fn element_count(&self) -> usize {
        self.flatten().len()
    }

    /// Return all elements (and descendants) matching a specific kind.
    pub fn filter_by_kind(&self, kind: OutlineKind) -> Vec<&OutlineElement> {
        self.flatten().into_iter().filter(|e| e.kind == kind).collect()
    }

    /// Find elements whose label contains `query` (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&OutlineElement> {
        let q = query.to_lowercase();
        self.flatten()
            .into_iter()
            .filter(|e| e.label.to_lowercase().contains(&q))
            .collect()
    }

    /// Maximum nesting depth (0 if the model is empty).
    pub fn depth(&self) -> usize {
        fn max_depth(elems: &[OutlineElement], current: usize) -> usize {
            let mut best = if elems.is_empty() { 0 } else { current };
            for e in elems {
                best = best.max(max_depth(&e.children, current + 1));
            }
            best
        }
        max_depth(&self.elements, 1)
    }

    /// Returns the path from root to deepest element containing `line`.
    pub fn breadcrumb_at_line(&self, line: u32) -> Vec<&OutlineElement> {
        fn collect_path<'a>(
            elems: &'a [OutlineElement],
            line: u32,
            path: &mut Vec<&'a OutlineElement>,
        ) -> bool {
            for e in elems {
                if line >= e.range_start_line && line <= e.range_end_line {
                    path.push(e);
                    collect_path(&e.children, line, path);
                    return true;
                }
            }
            false
        }
        let mut path = Vec::new();
        collect_path(&self.elements, line, &mut path);
        path
    }

    /// Sort top-level elements alphabetically by label.
    pub fn sort_by_name(&mut self) {
        fn sort_recursive(elems: &mut [OutlineElement]) {
            elems.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
            for e in elems.iter_mut() {
                sort_recursive(&mut e.children);
            }
        }
        sort_recursive(&mut self.elements);
    }

    /// Sort top-level elements by their start line.
    pub fn sort_by_position(&mut self) {
        fn sort_recursive(elems: &mut [OutlineElement]) {
            elems.sort_by_key(|e| e.range_start_line);
            for e in elems.iter_mut() {
                sort_recursive(&mut e.children);
            }
        }
        sort_recursive(&mut self.elements);
    }

    /// Returns true if elements is empty.
    pub fn is_elements_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Get the first element, if any.
    pub fn first_element(&self) -> Option<&OutlineElement> {
        self.elements.first()
    }

    /// Get the last element, if any.
    pub fn last_element(&self) -> Option<&OutlineElement> {
        self.elements.last()
    }

    /// Retain only elements matching the predicate.
    pub fn retain_elements(&mut self, f: impl Fn(&OutlineElement) -> bool) {
        self.elements.retain(|item| f(item));
    }
}

/// Accumulated statistics for outline-view operations.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineViewStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl OutlineViewStats {
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
    pub fn merge(&mut self, other: &OutlineViewStats) {
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

impl Default for OutlineViewStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OutlineViewStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OutlineViewStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for outline-view.
#[derive(Debug, Clone)]
pub struct OutlineViewValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl OutlineViewValidator {
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

impl Default for OutlineViewValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// DocumentSymbolTree
// ---------------------------------------------------------------------------

/// Renders an outline model as an indented text tree.
pub struct DocumentSymbolTree {
    lines: Vec<(String, OutlineKind, usize)>,
}

impl DocumentSymbolTree {
    /// Build a tree from an outline model.
    pub fn new(model: &OutlineModel) -> Self {
        let mut lines = Vec::new();
        fn collect(elems: &[OutlineElement], depth: usize, out: &mut Vec<(String, OutlineKind, usize)>) {
            for e in elems {
                out.push((e.label.clone(), e.kind, depth));
                collect(&e.children, depth + 1, out);
            }
        }
        collect(&model.elements, 0, &mut lines);
        Self { lines }
    }

    /// Render the tree with indentation (2 spaces per depth level).
    pub fn render(&self) -> String {
        self.lines
            .iter()
            .map(|(label, kind, depth)| self.render_line(label, *kind, *depth))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Render a single line with indentation.
    pub fn render_line(&self, label: &str, kind: OutlineKind, depth: usize) -> String {
        let indent = "  ".repeat(depth);
        format!("{indent}{label} ({kind})")
    }

    /// Total number of nodes in the tree.
    pub fn total_nodes(&self) -> usize {
        self.lines.len()
    }
}

// ---------------------------------------------------------------------------
// outline_filter / outline_exclude
// ---------------------------------------------------------------------------

/// Return elements matching any of the given kinds.
pub fn outline_filter<'a>(model: &'a OutlineModel, kinds: &[OutlineKind]) -> Vec<&'a OutlineElement> {
    model.flatten().into_iter().filter(|e| kinds.contains(&e.kind)).collect()
}

/// Return elements NOT matching any of the given kinds.
pub fn outline_exclude<'a>(model: &'a OutlineModel, kinds: &[OutlineKind]) -> Vec<&'a OutlineElement> {
    model.flatten().into_iter().filter(|e| !kinds.contains(&e.kind)).collect()
}

// ---------------------------------------------------------------------------
// outline_breadcrumb
// ---------------------------------------------------------------------------

/// Returns a formatted breadcrumb string like "Class > Method > Variable".
pub fn outline_breadcrumb(model: &OutlineModel, line: u32) -> String {
    outline_breadcrumb_labels(model, line).join(" > ")
}

/// Returns just the labels of the breadcrumb path.
pub fn outline_breadcrumb_labels(model: &OutlineModel, line: u32) -> Vec<String> {
    model
        .breadcrumb_at_line(line)
        .iter()
        .map(|e| e.label.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// OutlineModel extensions
// ---------------------------------------------------------------------------

impl OutlineModel {
    /// Merge elements from another model into this one.
    pub fn merge(&mut self, other: &OutlineModel) {
        self.elements.extend(other.elements.iter().cloned());
    }

    /// Find the first element with an exact label match (searches all levels).
    pub fn find_by_label(&self, label: &str) -> Option<&OutlineElement> {
        self.flatten().into_iter().find(|e| e.label == label)
    }

    /// Return unique kinds present in the model.
    pub fn kinds_present(&self) -> Vec<OutlineKind> {
        let mut kinds: Vec<OutlineKind> = self.flatten().iter().map(|e| e.kind).collect();
        kinds.sort_by_key(|k| format!("{k:?}"));
        kinds.dedup();
        kinds
    }
}

// ---------------------------------------------------------------------------
// outline_sort_by_position
// ---------------------------------------------------------------------------

/// Sort order for outline elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineSortOrder {
    /// Sort by start line (ascending).
    ByPosition,
    /// Sort alphabetically by label.
    ByName,
    /// Sort by symbol kind (grouping similar kinds together).
    ByKind,
}

/// Sort outline elements by their start-line position (ascending).
/// Children within each element are also sorted recursively.
pub fn outline_sort_by_position(elements: &mut [OutlineElement]) {
    elements.sort_by_key(|e| e.range_start_line);
    for elem in elements.iter_mut() {
        outline_sort_by_position(&mut elem.children);
    }
}

/// Sort outline elements alphabetically by label (case-insensitive).
/// Children within each element are also sorted recursively.
pub fn outline_sort_by_name(elements: &mut [OutlineElement]) {
    elements.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    for elem in elements.iter_mut() {
        outline_sort_by_name(&mut elem.children);
    }
}

/// Sort outline elements by kind ordinal, then by label within same kind.
pub fn outline_sort_by_kind(elements: &mut [OutlineElement]) {
    elements.sort_by(|a, b| {
        let ka = format!("{:?}", a.kind);
        let kb = format!("{:?}", b.kind);
        ka.cmp(&kb).then_with(|| a.label.cmp(&b.label))
    });
    for elem in elements.iter_mut() {
        outline_sort_by_kind(&mut elem.children);
    }
}

/// Apply the specified sort order to outline elements.
pub fn outline_sort(elements: &mut [OutlineElement], order: OutlineSortOrder) {
    match order {
        OutlineSortOrder::ByPosition => outline_sort_by_position(elements),
        OutlineSortOrder::ByName => outline_sort_by_name(elements),
        OutlineSortOrder::ByKind => outline_sort_by_kind(elements),
    }
}

impl OutlineModel {
    /// Return a clone of this model with elements sorted by position.
    pub fn sorted_by_position(&self) -> Self {
        let mut clone = self.clone();
        outline_sort_by_position(&mut clone.elements);
        clone
    }

    /// Return a clone of this model with elements sorted by name.
    pub fn sorted_by_name(&self) -> Self {
        let mut clone = self.clone();
        outline_sort_by_name(&mut clone.elements);
        clone
    }

    /// Return a clone with the specified sort order applied.
    pub fn sorted(&self, order: OutlineSortOrder) -> Self {
        let mut clone = self.clone();
        outline_sort(&mut clone.elements, order);
        clone
    }
}

// ---------------------------------------------------------------------------
// OutlineModel – additional query helpers
// ---------------------------------------------------------------------------

impl OutlineModel {
    /// Return labels of all top-level and nested elements (pre-order).
    pub fn labels(&self) -> Vec<&str> {
        self.flatten().into_iter().map(|e| e.label.as_str()).collect()
    }

    /// Find the first element (at any depth) whose label matches `name` exactly.
    pub fn find_by_name(&self, name: &str) -> Option<&OutlineElement> {
        self.flatten().into_iter().find(|e| e.label == name)
    }

    /// Count how many elements (at any depth) have the given kind.
    pub fn kind_count(&self, kind: OutlineKind) -> usize {
        self.flatten().into_iter().filter(|e| e.kind == kind).count()
    }

    /// Return a one-line summary of the model.
    pub fn summary(&self) -> String {
        let total = self.element_count();
        let top = self.elements.len();
        let depth = self.depth();
        format!(
            "OutlineModel(uri={}, top_level={}, total={}, depth={})",
            self.uri, top, total, depth
        )
    }
}

impl fmt::Display for OutlineModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OutlineModel(uri={}, elements={})",
            self.uri,
            self.element_count()
        )
    }
}

// ---------------------------------------------------------------------------
// OutlineElement – query helpers
// ---------------------------------------------------------------------------

impl OutlineElement {
    /// Returns `true` if this element has any children.
    pub fn is_container(&self) -> bool {
        !self.children.is_empty()
    }

    /// Return the number of direct children.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }
}

// ---------------------------------------------------------------------------
// OutlineKind – classification helpers
// ---------------------------------------------------------------------------

impl OutlineKind {
    /// Returns `true` for type-like kinds: Class, Struct, Interface, Enum.
    pub fn is_type(&self) -> bool {
        matches!(
            self,
            OutlineKind::Class | OutlineKind::Struct | OutlineKind::Interface | OutlineKind::Enum
        )
    }

    /// Returns `true` for callable kinds: Function, Method, Constructor.
    pub fn is_callable(&self) -> bool {
        matches!(
            self,
            OutlineKind::Function | OutlineKind::Method | OutlineKind::Constructor
        )
    }
}

// ---------------------------------------------------------------------------
// Symbol filtering by kind (multiple kinds)
// ---------------------------------------------------------------------------

impl OutlineModel {
    /// Return all elements matching any of the given kinds.
    pub fn filter_by_kinds(&self, kinds: &[OutlineKind]) -> Vec<&OutlineElement> {
        self.flatten()
            .into_iter()
            .filter(|e| kinds.contains(&e.kind))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Outline flattening with depth info
// ---------------------------------------------------------------------------

/// A flattened outline entry carrying its nesting depth.
#[derive(Debug, Clone)]
pub struct FlatOutlineEntry<'a> {
    /// Reference to the original element.
    pub element: &'a OutlineElement,
    /// Nesting depth (0 for top-level).
    pub depth: usize,
}

/// Flatten an outline model, attaching depth information to each entry.
pub fn flatten_with_depth(model: &OutlineModel) -> Vec<FlatOutlineEntry<'_>> {
    let mut result = Vec::new();
    fn collect<'a>(elems: &'a [OutlineElement], depth: usize, out: &mut Vec<FlatOutlineEntry<'a>>) {
        for e in elems {
            out.push(FlatOutlineEntry { element: e, depth });
            collect(&e.children, depth + 1, out);
        }
    }
    collect(&model.elements, 0, &mut result);
    result
}

// ---------------------------------------------------------------------------
// Outline path computation (breadcrumb)
// ---------------------------------------------------------------------------

impl OutlineModel {
    /// Compute the path of labels from root to the element containing the
    /// given line. Returns an empty vec if no element spans that line.
    pub fn path_at_line(&self, line: u32) -> Vec<String> {
        let mut path = Vec::new();
        fn search(elems: &[OutlineElement], line: u32, path: &mut Vec<String>) -> bool {
            for e in elems {
                if line >= e.range_start_line && line <= e.range_end_line {
                    path.push(e.label.clone());
                    search(&e.children, line, path);
                    return true;
                }
            }
            false
        }
        search(&self.elements, line, &mut path);
        path
    }
}

// ---------------------------------------------------------------------------
// Symbol range overlap detection
// ---------------------------------------------------------------------------

impl OutlineElement {
    /// Returns `true` if this element's line range overlaps with another's.
    pub fn overlaps(&self, other: &OutlineElement) -> bool {
        self.range_start_line <= other.range_end_line && other.range_start_line <= self.range_end_line
    }

    /// Returns `true` if this element fully contains `other`.
    pub fn contains_element(&self, other: &OutlineElement) -> bool {
        self.range_start_line <= other.range_start_line
            && self.range_end_line >= other.range_end_line
    }

    /// Line span of this element (inclusive).
    pub fn line_span(&self) -> u32 {
        if self.range_end_line >= self.range_start_line {
            self.range_end_line - self.range_start_line + 1
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Outline diff – detect structural changes between two outline snapshots
// ---------------------------------------------------------------------------

/// Describes a difference between two outline snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutlineDiff {
    /// A symbol was added (label, kind).
    Added(String, OutlineKind),
    /// A symbol was removed (label, kind).
    Removed(String, OutlineKind),
    /// A symbol's line range changed (label, old_start, old_end, new_start, new_end).
    Moved(String, u32, u32, u32, u32),
}

impl fmt::Display for OutlineDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutlineDiff::Added(label, kind) => write!(f, "+ {label} ({kind})"),
            OutlineDiff::Removed(label, kind) => write!(f, "- {label} ({kind})"),
            OutlineDiff::Moved(label, os, oe, ns, ne) => {
                write!(f, "~ {label} [{os}-{oe}] -> [{ns}-{ne}]")
            }
        }
    }
}

/// Compute a flat diff between two outline models.
///
/// Compares elements by (label, kind) identity. Reports additions, removals,
/// and range changes.
pub fn outline_diff(old: &OutlineModel, new: &OutlineModel) -> Vec<OutlineDiff> {
    let old_flat = old.flatten();
    let new_flat = new.flatten();

    let old_map: std::collections::HashMap<(&str, OutlineKind), (u32, u32)> = old_flat
        .iter()
        .map(|e| ((e.label.as_str(), e.kind), (e.range_start_line, e.range_end_line)))
        .collect();
    let new_map: std::collections::HashMap<(&str, OutlineKind), (u32, u32)> = new_flat
        .iter()
        .map(|e| ((e.label.as_str(), e.kind), (e.range_start_line, e.range_end_line)))
        .collect();

    let mut diffs = Vec::new();

    for (&(label, kind), &(os, oe)) in &old_map {
        match new_map.get(&(label, kind)) {
            None => diffs.push(OutlineDiff::Removed(label.to_string(), kind)),
            Some(&(ns, ne)) if (os, oe) != (ns, ne) => {
                diffs.push(OutlineDiff::Moved(label.to_string(), os, oe, ns, ne));
            }
            _ => {}
        }
    }
    for (&(label, kind), _) in &new_map {
        if !old_map.contains_key(&(label, kind)) {
            diffs.push(OutlineDiff::Added(label.to_string(), kind));
        }
    }

    diffs.sort_by(|a, b| {
        let key = |d: &OutlineDiff| match d {
            OutlineDiff::Removed(l, _) => (0, l.clone()),
            OutlineDiff::Moved(l, _, _, _, _) => (1, l.clone()),
            OutlineDiff::Added(l, _) => (2, l.clone()),
        };
        key(a).cmp(&key(b))
    });

    diffs
}

// ---------------------------------------------------------------------------
// Outline symbol range validation
// ---------------------------------------------------------------------------

impl OutlineModel {
    /// Detect sibling elements at any level whose ranges overlap.
    ///
    /// Returns pairs of labels that overlap within the same parent scope.
    pub fn find_overlapping_siblings(&self) -> Vec<(String, String)> {
        let mut result = Vec::new();
        fn check(elems: &[OutlineElement], out: &mut Vec<(String, String)>) {
            for i in 0..elems.len() {
                for j in (i + 1)..elems.len() {
                    if elems[i].overlaps(&elems[j]) {
                        out.push((elems[i].label.clone(), elems[j].label.clone()));
                    }
                }
                check(&elems[i].children, out);
            }
        }
        check(&self.elements, &mut result);
        result
    }

    /// Validate that every child element's range is fully contained within
    /// its parent's range. Returns labels of violating children.
    pub fn find_range_violations(&self) -> Vec<String> {
        let mut violations = Vec::new();
        fn check(elems: &[OutlineElement], parent: Option<&OutlineElement>, out: &mut Vec<String>) {
            for e in elems {
                if let Some(p) = parent {
                    if e.range_start_line < p.range_start_line
                        || e.range_end_line > p.range_end_line
                    {
                        out.push(e.label.clone());
                    }
                }
                check(&e.children, Some(e), out);
            }
        }
        check(&self.elements, None, &mut violations);
        violations
    }
}

// ---------------------------------------------------------------------------
// Outline navigation – next/previous sibling at cursor
// ---------------------------------------------------------------------------

impl OutlineModel {
    /// Find the element immediately after the one containing `line` at the
    /// same nesting level.  Returns `None` if the cursor is in the last
    /// sibling or outside any element.
    pub fn next_sibling_at_line(&self, line: u32) -> Option<&OutlineElement> {
        fn search<'a>(elems: &'a [OutlineElement], line: u32) -> Option<&'a OutlineElement> {
            for (i, e) in elems.iter().enumerate() {
                if line >= e.range_start_line && line <= e.range_end_line {
                    // Try deeper first
                    if let Some(found) = search(&e.children, line) {
                        return Some(found);
                    }
                    // Return next sibling at this level
                    return elems.get(i + 1);
                }
            }
            None
        }
        search(&self.elements, line)
    }

    /// Find the element immediately before the one containing `line` at the
    /// same nesting level.
    pub fn prev_sibling_at_line(&self, line: u32) -> Option<&OutlineElement> {
        fn search<'a>(elems: &'a [OutlineElement], line: u32) -> Option<&'a OutlineElement> {
            for (i, e) in elems.iter().enumerate() {
                if line >= e.range_start_line && line <= e.range_end_line {
                    if let Some(found) = search(&e.children, line) {
                        return Some(found);
                    }
                    return if i > 0 { Some(&elems[i - 1]) } else { None };
                }
            }
            None
        }
        search(&self.elements, line)
    }
}

// ---------------------------------------------------------------------------
// Outline statistics summary
// ---------------------------------------------------------------------------

/// Per-kind count entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindCount {
    pub kind: OutlineKind,
    pub count: usize,
}

impl OutlineModel {
    /// Return a breakdown of element counts grouped by kind, sorted
    /// descending by count.
    pub fn kind_histogram(&self) -> Vec<KindCount> {
        let mut map: std::collections::HashMap<OutlineKind, usize> =
            std::collections::HashMap::new();
        for e in self.flatten() {
            *map.entry(e.kind).or_insert(0) += 1;
        }
        let mut counts: Vec<KindCount> = map
            .into_iter()
            .map(|(kind, count)| KindCount { kind, count })
            .collect();
        counts.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| format!("{:?}", a.kind).cmp(&format!("{:?}", b.kind))));
        counts
    }
}

// ---------------------------------------------------------------------------
// Outline hierarchy builder – reconstruct tree from a flat list
// ---------------------------------------------------------------------------

/// A flat symbol entry used as input for hierarchy reconstruction.
#[derive(Debug, Clone)]
pub struct FlatSymbol {
    pub label: String,
    pub kind: OutlineKind,
    pub start_line: u32,
    pub end_line: u32,
    pub detail: Option<String>,
}

/// Build a hierarchical `OutlineModel` from a flat list of symbols.
///
/// Symbols whose ranges are fully contained within another symbol become
/// children of the innermost enclosing symbol. The input does not need
/// to be sorted; the function sorts by start line internally.
pub fn build_hierarchy(uri: impl Into<String>, symbols: &[FlatSymbol]) -> OutlineModel {
    let mut sorted: Vec<&FlatSymbol> = symbols.iter().collect();
    sorted.sort_by(|a, b| a.start_line.cmp(&b.start_line).then(b.end_line.cmp(&a.end_line)));

    let mut root_elements: Vec<OutlineElement> = Vec::new();

    fn insert_into(target: &mut Vec<OutlineElement>, sym: &FlatSymbol) {
        // Try to insert into the last element if it fully contains the symbol.
        if let Some(last) = target.last_mut() {
            if last.range_start_line <= sym.start_line && last.range_end_line >= sym.end_line {
                insert_into(&mut last.children, sym);
                return;
            }
        }
        target.push(OutlineElement {
            label: sym.label.clone(),
            detail: sym.detail.clone(),
            kind: sym.kind,
            range_start_line: sym.start_line,
            range_end_line: sym.end_line,
            children: Vec::new(),
        });
    }

    for sym in &sorted {
        insert_into(&mut root_elements, sym);
    }

    OutlineModel {
        elements: root_elements,
        uri: uri.into(),
    }
}

// ---------------------------------------------------------------------------
// Symbol navigation – go to parent / first child
// ---------------------------------------------------------------------------

impl OutlineModel {
    /// Find the parent element of the deepest element containing `line`.
    /// Returns `None` if the cursor is at a top-level element or outside
    /// any element.
    pub fn parent_at_line(&self, line: u32) -> Option<&OutlineElement> {
        fn search<'a>(
            elems: &'a [OutlineElement],
            line: u32,
            parent: Option<&'a OutlineElement>,
        ) -> Option<&'a OutlineElement> {
            for e in elems {
                if line >= e.range_start_line && line <= e.range_end_line {
                    if let Some(found) = search(&e.children, line, Some(e)) {
                        return Some(found);
                    }
                    return parent;
                }
            }
            None
        }
        search(&self.elements, line, None)
    }

    /// Return the first child of the deepest element containing `line`,
    /// or `None` if that element has no children.
    pub fn first_child_at_line(&self, line: u32) -> Option<&OutlineElement> {
        self.find_at_line(line).and_then(|e| e.children.first())
    }
}

// ---------------------------------------------------------------------------
// Outline walking / visiting
// ---------------------------------------------------------------------------

impl OutlineModel {
    /// Walk all elements in pre-order, calling `visitor` with each element
    /// and its depth. If the visitor returns `false`, traversal of that
    /// subtree is skipped.
    pub fn walk<F>(&self, mut visitor: F)
    where
        F: FnMut(&OutlineElement, usize) -> bool,
    {
        fn walk_inner<F: FnMut(&OutlineElement, usize) -> bool>(
            elems: &[OutlineElement],
            depth: usize,
            visitor: &mut F,
        ) {
            for e in elems {
                if visitor(e, depth) {
                    walk_inner(&e.children, depth + 1, visitor);
                }
            }
        }
        walk_inner(&self.elements, 0, &mut visitor);
    }

    /// Collect all elements at a specific nesting depth (0 = top-level).
    pub fn elements_at_depth(&self, target_depth: usize) -> Vec<&OutlineElement> {
        let mut out = Vec::new();
        fn collect<'a>(elems: &'a [OutlineElement], depth: usize, target: usize, out: &mut Vec<&'a OutlineElement>) {
            for e in elems {
                if depth == target {
                    out.push(e);
                } else if depth < target {
                    collect(&e.children, depth + 1, target, out);
                }
            }
        }
        collect(&self.elements, 0, target_depth, &mut out);
        out
    }

    /// Return all leaf elements (elements with no children).
    pub fn leaves(&self) -> Vec<&OutlineElement> {
        self.flatten().into_iter().filter(|e| e.children.is_empty()).collect()
    }

    /// Return the total line span covered by all top-level elements.
    /// This is the difference between the smallest start line and the
    /// largest end line across all elements, or 0 if the model is empty.
    pub fn total_line_span(&self) -> u32 {
        let flat = self.flatten();
        if flat.is_empty() {
            return 0;
        }
        let min_start = flat.iter().map(|e| e.range_start_line).min().unwrap();
        let max_end = flat.iter().map(|e| e.range_end_line).max().unwrap();
        max_end - min_start + 1
    }
}

// ---------------------------------------------------------------------------
// OutlineElement – ancestry / descendant queries
// ---------------------------------------------------------------------------

impl OutlineElement {
    /// Total count of all descendants (children, grandchildren, etc.).
    pub fn descendant_count(&self) -> usize {
        fn count(elems: &[OutlineElement]) -> usize {
            elems.iter().map(|e| 1 + count(&e.children)).sum()
        }
        count(&self.children)
    }

    /// Maximum nesting depth from this element (1 if leaf).
    pub fn subtree_depth(&self) -> usize {
        if self.children.is_empty() {
            return 1;
        }
        1 + self.children.iter().map(|c| c.subtree_depth()).max().unwrap_or(0)
    }

    /// Returns `true` if this element's range contains the given line.
    pub fn contains_line(&self, line: u32) -> bool {
        line >= self.range_start_line && line <= self.range_end_line
    }

    /// Collect all unique kinds in this element's subtree (including self).
    pub fn subtree_kinds(&self) -> Vec<OutlineKind> {
        let mut kinds = vec![self.kind];
        fn collect(elems: &[OutlineElement], out: &mut Vec<OutlineKind>) {
            for e in elems {
                if !out.contains(&e.kind) {
                    out.push(e.kind);
                }
                collect(&e.children, out);
            }
        }
        collect(&self.children, &mut kinds);
        kinds
    }
}

// ---------------------------------------------------------------------------
// Outline model – range-based queries
// ---------------------------------------------------------------------------

impl OutlineModel {
    /// Return all elements (at any depth) whose range intersects [start, end].
    pub fn elements_in_range(&self, start: u32, end: u32) -> Vec<&OutlineElement> {
        self.flatten()
            .into_iter()
            .filter(|e| e.range_start_line <= end && e.range_end_line >= start)
            .collect()
    }

    /// Return the element with the largest line span.
    pub fn largest_element(&self) -> Option<&OutlineElement> {
        self.flatten().into_iter().max_by_key(|e| e.line_span())
    }

    /// Return the element with the smallest line span.
    pub fn smallest_element(&self) -> Option<&OutlineElement> {
        self.flatten().into_iter().min_by_key(|e| e.line_span())
    }
}

// ---------------------------------------------------------------------------
// Outline model – structural comparison helpers
// ---------------------------------------------------------------------------

impl OutlineModel {
    /// Returns `true` if two models have the same set of (label, kind) pairs
    /// regardless of position.
    pub fn structurally_equal(&self, other: &OutlineModel) -> bool {
        let mut self_sigs: Vec<(String, OutlineKind)> = self
            .flatten()
            .iter()
            .map(|e| (e.label.clone(), e.kind))
            .collect();
        let mut other_sigs: Vec<(String, OutlineKind)> = other
            .flatten()
            .iter()
            .map(|e| (e.label.clone(), e.kind))
            .collect();
        self_sigs.sort_by(|a, b| a.0.cmp(&b.0).then(format!("{:?}", a.1).cmp(&format!("{:?}", b.1))));
        other_sigs.sort_by(|a, b| a.0.cmp(&b.0).then(format!("{:?}", a.1).cmp(&format!("{:?}", b.1))));
        self_sigs == other_sigs
    }
}

// ---------------------------------------------------------------------------
// OutlineKind – additional classification helpers
// ---------------------------------------------------------------------------

impl OutlineKind {
    /// Returns `true` for value-like kinds: Variable, Constant, Field, Property.
    pub fn is_value(&self) -> bool {
        matches!(
            self,
            OutlineKind::Variable | OutlineKind::Constant | OutlineKind::Field | OutlineKind::Property
        )
    }

    /// Returns `true` for container-like kinds: File, Module, Namespace.
    pub fn is_container_kind(&self) -> bool {
        matches!(
            self,
            OutlineKind::File | OutlineKind::Module | OutlineKind::Namespace
        )
    }
}


// ---------------------------------------------------------------------------
// OutlineSortToggler
// ---------------------------------------------------------------------------

/// Available sort orders for outline elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineSortMode {
    /// Sort by position in the document.
    ByPosition,
    /// Sort by name alphabetically.
    ByName,
    /// Sort by kind (group by symbol kind, then by position).
    ByKind,
    /// Sort by name length.
    ByNameLength,
}

/// Manages outline sort order with toggle cycling.
#[derive(Debug, Clone)]
pub struct OutlineSortToggler {
    modes: Vec<OutlineSortMode>,
    current_index: usize,
    toggle_count: u64,
}

impl OutlineSortToggler {
    /// Create a new sort toggler with default mode order.
    pub fn new() -> Self {
        Self {
            modes: vec![
                OutlineSortMode::ByPosition,
                OutlineSortMode::ByName,
                OutlineSortMode::ByKind,
                OutlineSortMode::ByNameLength,
            ],
            current_index: 0,
            toggle_count: 0,
        }
    }

    /// Create a toggler with custom modes.
    pub fn with_modes(modes: Vec<OutlineSortMode>) -> Self {
        Self {
            modes,
            current_index: 0,
            toggle_count: 0,
        }
    }

    /// Get the current sort mode.
    pub fn current(&self) -> OutlineSortMode {
        self.modes[self.current_index]
    }

    /// Toggle to the next sort mode, returning the new mode.
    pub fn toggle(&mut self) -> OutlineSortMode {
        self.current_index = (self.current_index + 1) % self.modes.len();
        self.toggle_count += 1;
        self.current()
    }

    /// Toggle to the previous sort mode.
    pub fn toggle_back(&mut self) -> OutlineSortMode {
        if self.current_index == 0 {
            self.current_index = self.modes.len() - 1;
        } else {
            self.current_index -= 1;
        }
        self.toggle_count += 1;
        self.current()
    }

    /// Set the mode directly, returns true if found.
    pub fn set_mode(&mut self, mode: OutlineSortMode) -> bool {
        if let Some(idx) = self.modes.iter().position(|m| *m == mode) {
            self.current_index = idx;
            true
        } else {
            false
        }
    }

    /// Number of available modes.
    pub fn mode_count(&self) -> usize {
        self.modes.len()
    }

    /// Number of times toggled.
    pub fn toggle_count(&self) -> u64 {
        self.toggle_count
    }

    /// Apply current sort to a mutable slice of outline elements.
    pub fn sort_elements(&self, elements: &mut [OutlineElement]) {
        match self.current() {
            OutlineSortMode::ByPosition => {
                elements.sort_by_key(|e| (e.range_start_line, e.range_end_line));
            }
            OutlineSortMode::ByName => {
                elements.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
            }
            OutlineSortMode::ByKind => {
                elements.sort_by(|a, b| {
                    let ka = format!("{:?}", a.kind);
                    let kb = format!("{:?}", b.kind);
                    ka.cmp(&kb).then(a.range_start_line.cmp(&b.range_start_line))
                });
            }
            OutlineSortMode::ByNameLength => {
                elements.sort_by_key(|e| e.label.len());
            }
        }
    }

    /// Reset to first mode.
    pub fn reset(&mut self) {
        self.current_index = 0;
        self.toggle_count = 0;
    }
}

impl fmt::Display for OutlineSortToggler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SortToggler({:?}, toggled {} times)", self.current(), self.toggle_count)
    }
}

// ---------------------------------------------------------------------------
// OutlineFilterByKind
// ---------------------------------------------------------------------------

/// Filters outline elements by symbol kind.
#[derive(Debug, Clone)]
pub struct OutlineFilterByKind {
    /// Allowed kinds. If empty, all kinds are shown.
    allowed: Vec<OutlineKind>,
    /// Excluded kinds.
    excluded: Vec<OutlineKind>,
    /// Number of times the filter has been applied.
    apply_count: u64,
}

impl OutlineFilterByKind {
    /// Create a new filter that allows all kinds.
    pub fn new() -> Self {
        Self {
            allowed: Vec::new(),
            excluded: Vec::new(),
            apply_count: 0,
        }
    }

    /// Create a filter that only shows the specified kinds.
    pub fn only(kinds: Vec<OutlineKind>) -> Self {
        Self {
            allowed: kinds,
            excluded: Vec::new(),
            apply_count: 0,
        }
    }

    /// Create a filter that excludes the specified kinds.
    pub fn excluding(kinds: Vec<OutlineKind>) -> Self {
        Self {
            allowed: Vec::new(),
            excluded: kinds,
            apply_count: 0,
        }
    }

    /// Add a kind to the allowed list.
    pub fn allow_kind(&mut self, kind: OutlineKind) {
        if !self.allowed.contains(&kind) {
            self.allowed.push(kind);
        }
    }

    /// Add a kind to the excluded list.
    pub fn exclude_kind(&mut self, kind: OutlineKind) {
        if !self.excluded.contains(&kind) {
            self.excluded.push(kind);
        }
    }

    /// Check if a kind passes this filter.
    pub fn accepts(&self, kind: OutlineKind) -> bool {
        if self.excluded.contains(&kind) {
            return false;
        }
        if self.allowed.is_empty() {
            return true;
        }
        self.allowed.contains(&kind)
    }

    /// Filter a slice of elements, returning only those whose kind is accepted.
    pub fn apply<'a>(&mut self, elements: &'a [OutlineElement]) -> Vec<&'a OutlineElement> {
        self.apply_count += 1;
        elements.iter().filter(|e| self.accepts(e.kind)).collect()
    }

    /// Count how many elements would pass.
    pub fn count_matching(&self, elements: &[OutlineElement]) -> usize {
        elements.iter().filter(|e| self.accepts(e.kind)).count()
    }

    /// Count how many elements would be filtered out.
    pub fn count_filtered_out(&self, elements: &[OutlineElement]) -> usize {
        elements.iter().filter(|e| !self.accepts(e.kind)).count()
    }

    /// Number of times apply was called.
    pub fn apply_count(&self) -> u64 {
        self.apply_count
    }

    /// Number of allowed kinds.
    pub fn allowed_count(&self) -> usize {
        self.allowed.len()
    }

    /// Number of excluded kinds.
    pub fn excluded_count(&self) -> usize {
        self.excluded.len()
    }

    /// Reset the filter to accept all kinds.
    pub fn reset(&mut self) {
        self.allowed.clear();
        self.excluded.clear();
        self.apply_count = 0;
    }

    /// Check if filter is in default (accept-all) state.
    pub fn is_default(&self) -> bool {
        self.allowed.is_empty() && self.excluded.is_empty()
    }
}

impl fmt::Display for OutlineFilterByKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FilterByKind(allowed={}, excluded={}, applied {} times)",
            self.allowed_count(),
            self.excluded_count(),
            self.apply_count
        )
    }
}



// ---------------------------------------------------------------------------
// outline_view – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XOutlineViewLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XOutlineViewPanelState {
    pub region: XOutlineViewLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XOutlineViewPanelState {
    pub fn new(region: XOutlineViewLayoutRegion, label: impl Into<String>) -> Self {
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
pub fn x_outline_view_total_visible_area(panels: &[XOutlineViewPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_outline_view_count_in_region(
    panels: &[XOutlineViewPanelState],
    region: XOutlineViewLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_outline_view_widest_panel(panels: &[XOutlineViewPanelState]) -> Option<&XOutlineViewPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_outline_view_collapse_region(
    panels: &mut [XOutlineViewPanelState],
    region: XOutlineViewLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XOutlineViewLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XOutlineViewLayoutConstraint {
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
// outline_view – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for document outline tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YOutlineViewOutlineSortOrder {
    ByPosition,
    ByName,
    ByKind,
    ByCategory,
}

impl YOutlineViewOutlineSortOrder {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::ByPosition => 0,
            Self::ByName => 1,
            Self::ByKind => 2,
            Self::ByCategory => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::ByPosition => "ByPosition",
            Self::ByName => "ByName",
            Self::ByKind => "ByKind",
            Self::ByCategory => "ByCategory",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YOutlineViewOutlineSortOrder] {
        &[
            YOutlineViewOutlineSortOrder::ByPosition,
            YOutlineViewOutlineSortOrder::ByName,
            YOutlineViewOutlineSortOrder::ByKind,
            YOutlineViewOutlineSortOrder::ByCategory,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YOutlineViewOutlineSortOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks outline filter data.
#[derive(Debug, Clone)]
pub struct YOutlineViewOutlineFilter {
    pub patterns: Vec<String>,
    pub case_sensitive: bool,
    pub include_detail: bool,
}

impl YOutlineViewOutlineFilter {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            case_sensitive: false,
            include_detail: false,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.patterns.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YOutlineViewOutlineFilter({}: {:?})", "patterns", self.patterns)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_outline_view_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_outline_view_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_outline_view_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_outline_view_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_outline_view_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_outline_view_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_outline_view_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_outline_view_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// outline_view – Extended outline breadcrumb helpers
// ---------------------------------------------------------------------------

/// Priority levels for outline breadcrumb.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZOutlineViewPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZOutlineViewPriority {
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
    pub fn all_asc() -> [ZOutlineViewPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZOutlineViewPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks outline breadcrumb data.
#[derive(Debug, Clone)]
pub struct ZOutlineViewOutlineBreadcrumb {
    pub segments: Vec<String>,
    pub separator: String,
    pub max_depth: usize,
}

impl ZOutlineViewOutlineBreadcrumb {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            separator: String::new(),
            max_depth: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.segments.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZOutlineViewOutlineBreadcrumb[separator={:?}, max_depth={:?}]", self.separator, self.max_depth)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for outline breadcrumb.
pub fn z_outline_view_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_outline_view_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_outline_view_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_outline_view_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_outline_view_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_outline_view_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_outline_view_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elem(label: &str, kind: OutlineKind, start: u32, end: u32) -> OutlineElement {
        OutlineElement {
            label: label.into(),
            detail: None,
            kind,
            range_start_line: start,
            range_end_line: end,
            children: Vec::new(),
        }
    }

    #[test]
    fn add_and_count() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("main", OutlineKind::Function, 1, 10));
        model.add_element(elem("Foo", OutlineKind::Struct, 12, 20));
        assert_eq!(model.element_count(), 2);
        assert_eq!(model.uri, "file.rs");
    }

    #[test]
    fn flatten_includes_children() {
        let mut model = OutlineModel::new("file.rs");
        let mut parent = elem("MyStruct", OutlineKind::Struct, 1, 30);
        parent.children.push(elem("field_a", OutlineKind::Field, 2, 2));
        parent.children.push(elem("method_b", OutlineKind::Method, 4, 10));
        model.add_element(parent);
        let flat = model.flatten();
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].label, "MyStruct");
        assert_eq!(flat[1].label, "field_a");
    }

    #[test]
    fn find_at_line_deepest() {
        let mut model = OutlineModel::new("file.rs");
        let mut parent = elem("Outer", OutlineKind::Class, 1, 50);
        parent.children.push(elem("inner", OutlineKind::Method, 10, 20));
        model.add_element(parent);
        model.add_element(elem("standalone", OutlineKind::Function, 55, 60));

        let found = model.find_at_line(15).unwrap();
        assert_eq!(found.label, "inner");

        let found = model.find_at_line(5).unwrap();
        assert_eq!(found.label, "Outer");

        let found = model.find_at_line(57).unwrap();
        assert_eq!(found.label, "standalone");

        assert!(model.find_at_line(100).is_none());
    }

    #[test]
    fn outline_error_display() {
        let e = OutlineError::ElementNotFound("foo".into());
        assert_eq!(e.to_string(), "element not found: foo");
        assert_eq!(OutlineError::EmptyModel.to_string(), "outline model is empty");
        let e = OutlineError::InvalidRange { start: 5, end: 2 };
        assert_eq!(e.to_string(), "invalid range: 5..2");
    }

    #[test]
    fn outline_kind_display() {
        assert_eq!(OutlineKind::Function.to_string(), "Function");
        assert_eq!(OutlineKind::Struct.to_string(), "Struct");
        assert_eq!(OutlineKind::Event.to_string(), "Event");
    }

    #[test]
    fn outline_element_display() {
        let e = elem("main", OutlineKind::Function, 1, 10);
        assert_eq!(e.to_string(), "main (Function) [1-10]");
    }

    #[test]
    fn with_child_builder() {
        let e = elem("Parent", OutlineKind::Class, 1, 50)
            .with_child(elem("child_a", OutlineKind::Field, 2, 2))
            .with_child(elem("child_b", OutlineKind::Method, 4, 10));
        assert_eq!(e.children.len(), 2);
        assert_eq!(e.children[0].label, "child_a");
    }

    #[test]
    fn with_detail_builder() {
        let e = elem("foo", OutlineKind::Function, 1, 5).with_detail("returns i32");
        assert_eq!(e.detail.as_deref(), Some("returns i32"));
    }

    #[test]
    fn filter_by_kind_works() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("main", OutlineKind::Function, 1, 10));
        model.add_element(
            elem("MyStruct", OutlineKind::Struct, 12, 30)
                .with_child(elem("new", OutlineKind::Function, 13, 20)),
        );
        model.add_element(elem("FOO", OutlineKind::Constant, 32, 32));
        let fns = model.filter_by_kind(OutlineKind::Function);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].label, "main");
        assert_eq!(fns[1].label, "new");
    }

    #[test]
    fn search_case_insensitive() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("MyStruct", OutlineKind::Struct, 1, 20));
        model.add_element(elem("my_func", OutlineKind::Function, 22, 30));
        model.add_element(elem("OTHER", OutlineKind::Constant, 32, 32));
        let results = model.search("my");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn depth_empty_model() {
        let model = OutlineModel::new("file.rs");
        assert_eq!(model.depth(), 0);
    }

    #[test]
    fn depth_nested() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("A", OutlineKind::Class, 1, 50)
                .with_child(
                    elem("B", OutlineKind::Method, 2, 40)
                        .with_child(elem("C", OutlineKind::Variable, 3, 3)),
                ),
        );
        model.add_element(elem("flat", OutlineKind::Function, 52, 60));
        assert_eq!(model.depth(), 3);
    }

    #[test]
    fn breadcrumb_at_line_works() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("Outer", OutlineKind::Class, 1, 50)
                .with_child(elem("inner", OutlineKind::Method, 10, 20)),
        );
        let crumbs = model.breadcrumb_at_line(15);
        assert_eq!(crumbs.len(), 2);
        assert_eq!(crumbs[0].label, "Outer");
        assert_eq!(crumbs[1].label, "inner");

        let crumbs = model.breadcrumb_at_line(5);
        assert_eq!(crumbs.len(), 1);
        assert_eq!(crumbs[0].label, "Outer");

        assert!(model.breadcrumb_at_line(100).is_empty());
    }

    #[test]
    fn sort_by_name_works() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("Zebra", OutlineKind::Struct, 1, 10));
        model.add_element(elem("alpha", OutlineKind::Function, 12, 20));
        model.add_element(elem("Beta", OutlineKind::Constant, 22, 25));
        model.sort_by_name();
        let names: Vec<_> = model.elements.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(names, vec!["alpha", "Beta", "Zebra"]);
    }

    #[test]
    fn sort_by_position_works() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("late", OutlineKind::Function, 50, 60));
        model.add_element(elem("early", OutlineKind::Function, 1, 10));
        model.add_element(elem("mid", OutlineKind::Function, 20, 30));
        model.sort_by_position();
        let starts: Vec<_> = model.elements.iter().map(|e| e.range_start_line).collect();
        assert_eq!(starts, vec![1, 20, 50]);
    }

    #[test]
    fn eq_outlineerror_same() {
        assert_eq!(OutlineError::EmptyModel, OutlineError::EmptyModel);
    }

    #[test]
    fn ne_outlineerror_diff() {
        assert_ne!(OutlineError::EmptyModel, OutlineError::ElementNotFound("x".into()));
    }

    #[test]
    fn eq_outlinekind_same() {
        assert_eq!(OutlineKind::File, OutlineKind::File);
    }

    #[test]
    fn ne_outlinekind_diff() {
        assert_ne!(OutlineKind::File, OutlineKind::Module);
    }

    #[test]
    fn display_outlineerror_variants() {
        assert!(!OutlineError::EmptyModel.to_string().is_empty());
        assert!(!OutlineError::EmptyModel.to_string().is_empty());
    }

    #[test]
    fn display_outlinekind_variants() {
        assert!(!OutlineKind::File.to_string().is_empty());
        assert!(!OutlineKind::Module.to_string().is_empty());
        assert!(!OutlineKind::Namespace.to_string().is_empty());
        assert!(!OutlineKind::Class.to_string().is_empty());
        assert!(!OutlineKind::Method.to_string().is_empty());
    }

    #[test]
    fn document_symbol_tree_render() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("MyClass", OutlineKind::Class, 1, 50)
                .with_child(elem("method_a", OutlineKind::Method, 5, 20))
                .with_child(elem("field_x", OutlineKind::Field, 22, 22)),
        );
        model.add_element(elem("helper", OutlineKind::Function, 52, 60));
        let tree = DocumentSymbolTree::new(&model);
        assert_eq!(tree.total_nodes(), 4);
        let rendered = tree.render();
        assert!(rendered.contains("MyClass (Class)"));
        assert!(rendered.contains("  method_a (Method)"));
        assert!(rendered.contains("  field_x (Field)"));
        assert!(rendered.contains("helper (Function)"));
    }

    #[test]
    fn document_symbol_tree_empty() {
        let model = OutlineModel::new("empty.rs");
        let tree = DocumentSymbolTree::new(&model);
        assert_eq!(tree.total_nodes(), 0);
        assert_eq!(tree.render(), "");
    }

    #[test]
    fn outline_filter_by_kinds() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("main", OutlineKind::Function, 1, 10));
        model.add_element(elem("Foo", OutlineKind::Struct, 12, 30));
        model.add_element(elem("BAR", OutlineKind::Constant, 32, 32));
        let fns = outline_filter(&model, &[OutlineKind::Function, OutlineKind::Constant]);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].label, "main");
        assert_eq!(fns[1].label, "BAR");
    }

    #[test]
    fn outline_exclude_by_kinds() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("main", OutlineKind::Function, 1, 10));
        model.add_element(elem("Foo", OutlineKind::Struct, 12, 30));
        model.add_element(elem("BAR", OutlineKind::Constant, 32, 32));
        let not_fns = outline_exclude(&model, &[OutlineKind::Function]);
        assert_eq!(not_fns.len(), 2);
        assert_eq!(not_fns[0].label, "Foo");
        assert_eq!(not_fns[1].label, "BAR");
    }

    #[test]
    fn outline_breadcrumb_formatted() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("MyClass", OutlineKind::Class, 1, 50)
                .with_child(
                    elem("my_method", OutlineKind::Method, 10, 40)
                        .with_child(elem("local_var", OutlineKind::Variable, 15, 15)),
                ),
        );
        let bc = outline_breadcrumb(&model, 15);
        assert_eq!(bc, "MyClass > my_method > local_var");

        let labels = outline_breadcrumb_labels(&model, 15);
        assert_eq!(labels, vec!["MyClass", "my_method", "local_var"]);
    }

    #[test]
    fn outline_breadcrumb_empty_for_unknown_line() {
        let model = OutlineModel::new("file.rs");
        assert_eq!(outline_breadcrumb(&model, 99), "");
        assert!(outline_breadcrumb_labels(&model, 99).is_empty());
    }

    #[test]
    fn model_merge() {
        let mut m1 = OutlineModel::new("a.rs");
        m1.add_element(elem("foo", OutlineKind::Function, 1, 10));
        let mut m2 = OutlineModel::new("b.rs");
        m2.add_element(elem("bar", OutlineKind::Function, 1, 5));
        m2.add_element(elem("Baz", OutlineKind::Struct, 6, 20));
        m1.merge(&m2);
        assert_eq!(m1.element_count(), 3);
    }

    #[test]
    fn model_find_by_label() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("Parent", OutlineKind::Class, 1, 50)
                .with_child(elem("nested_child", OutlineKind::Method, 5, 20)),
        );
        model.add_element(elem("top_level", OutlineKind::Function, 52, 60));

        let found = model.find_by_label("nested_child").unwrap();
        assert_eq!(found.kind, OutlineKind::Method);

        let found2 = model.find_by_label("top_level").unwrap();
        assert_eq!(found2.kind, OutlineKind::Function);

        assert!(model.find_by_label("nonexistent").is_none());
    }

    #[test]
    fn model_kinds_present() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("main", OutlineKind::Function, 1, 10));
        model.add_element(elem("Foo", OutlineKind::Struct, 12, 30));
        model.add_element(elem("helper", OutlineKind::Function, 32, 40));
        let kinds = model.kinds_present();
        assert!(kinds.contains(&OutlineKind::Function));
        assert!(kinds.contains(&OutlineKind::Struct));
        assert_eq!(kinds.len(), 2); // deduped
    }

    #[test]
    fn render_line_indentation() {
        let model = OutlineModel::new("file.rs");
        let tree = DocumentSymbolTree::new(&model);
        let line = tree.render_line("foo", OutlineKind::Function, 0);
        assert_eq!(line, "foo (Function)");
        let line2 = tree.render_line("bar", OutlineKind::Method, 2);
        assert_eq!(line2, "    bar (Method)");
    }

    #[test]
    fn outline_view_stats_new_defaults() {
        let stats = OutlineViewStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn outline_view_stats_record_success() {
        let mut stats = OutlineViewStats::new();
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
    fn outline_view_stats_record_failure() {
        let mut stats = OutlineViewStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn outline_view_stats_reset() {
        let mut stats = OutlineViewStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn outline_view_stats_merge() {
        let mut a = OutlineViewStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = OutlineViewStats::new();
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
    fn outline_view_stats_display() {
        let mut stats = OutlineViewStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn outline_view_stats_default() {
        let stats = OutlineViewStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn outline_view_validator_accepts_valid_name() {
        let v = OutlineViewValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn outline_view_validator_rejects_empty() {
        let v = OutlineViewValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn outline_view_validator_rejects_too_long() {
        let v = OutlineViewValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn outline_view_validator_forbidden_prefix() {
        let v = OutlineViewValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn outline_view_validator_allowed_chars() {
        let v = OutlineViewValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn outline_view_validator_range() {
        let v = OutlineViewValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn outline_view_sanitize_removes_control() {
        let result = OutlineViewValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn outline_view_truncate_short_string() {
        assert_eq!(OutlineViewValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn outline_view_truncate_long_string() {
        let result = OutlineViewValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn outline_view_is_ascii_printable() {
        assert!(OutlineViewValidator::is_ascii_printable("Hello World 123"));
        assert!(!OutlineViewValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- outline_sort_by_position tests -------------------------------------

    #[test]
    fn sort_by_position_reorders_by_start_line() {
        let mut elements = vec![
            elem("gamma", OutlineKind::Function, 30, 40),
            elem("alpha", OutlineKind::Function, 1, 10),
            elem("beta", OutlineKind::Function, 15, 25),
        ];
        outline_sort_by_position(&mut elements);
        assert_eq!(elements[0].label, "alpha");
        assert_eq!(elements[1].label, "beta");
        assert_eq!(elements[2].label, "gamma");
    }

    #[test]
    fn sort_by_position_recursive() {
        let mut parent = elem("Parent", OutlineKind::Class, 1, 50);
        parent.children = vec![
            elem("z_method", OutlineKind::Method, 30, 40),
            elem("a_method", OutlineKind::Method, 5, 15),
        ];
        let mut elements = vec![parent];
        outline_sort_by_position(&mut elements);
        assert_eq!(elements[0].children[0].label, "a_method");
        assert_eq!(elements[0].children[1].label, "z_method");
    }

    #[test]
    fn sort_by_name_alphabetical() {
        let mut elements = vec![
            elem("Zebra", OutlineKind::Class, 1, 10),
            elem("apple", OutlineKind::Class, 11, 20),
            elem("Mango", OutlineKind::Class, 21, 30),
        ];
        outline_sort_by_name(&mut elements);
        assert_eq!(elements[0].label, "apple");
        assert_eq!(elements[1].label, "Mango");
        assert_eq!(elements[2].label, "Zebra");
    }

    #[test]
    fn sort_by_kind_groups_same_kinds() {
        let mut elements = vec![
            elem("b_var", OutlineKind::Variable, 20, 21),
            elem("a_fn", OutlineKind::Function, 1, 10),
            elem("a_var", OutlineKind::Variable, 15, 16),
            elem("b_fn", OutlineKind::Function, 11, 20),
        ];
        outline_sort_by_kind(&mut elements);
        assert_eq!(elements[0].kind, elements[1].kind);
        assert_eq!(elements[2].kind, elements[3].kind);
    }

    #[test]
    fn model_sorted_by_position() {
        let model = OutlineModel {
            elements: vec![
                elem("second", OutlineKind::Function, 20, 30),
                elem("first", OutlineKind::Function, 1, 10),
            ],
            uri: String::from("test://file"),
        };
        let sorted = model.sorted_by_position();
        assert_eq!(sorted.elements[0].label, "first");
        assert_eq!(sorted.elements[1].label, "second");
        // Original unchanged
        assert_eq!(model.elements[0].label, "second");
    }

    #[test]
    fn outline_sort_dispatch() {
        let mut elements = vec![
            elem("z", OutlineKind::Variable, 1, 2),
            elem("a", OutlineKind::Variable, 3, 4),
        ];
        outline_sort(&mut elements, OutlineSortOrder::ByName);
        assert_eq!(elements[0].label, "a");
    }

    // -- New tests ----------------------------------------------------------

    #[test]
    fn model_labels() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("Foo", OutlineKind::Struct, 1, 20)
                .with_child(elem("bar", OutlineKind::Method, 5, 10)),
        );
        model.add_element(elem("main", OutlineKind::Function, 22, 30));
        let labels = model.labels();
        assert_eq!(labels, vec!["Foo", "bar", "main"]);
    }

    #[test]
    fn model_find_by_name() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("MyClass", OutlineKind::Class, 1, 50)
                .with_child(elem("do_thing", OutlineKind::Method, 5, 20)),
        );
        assert!(model.find_by_name("do_thing").is_some());
        assert_eq!(model.find_by_name("do_thing").unwrap().kind, OutlineKind::Method);
        assert!(model.find_by_name("nonexistent").is_none());
    }

    #[test]
    fn model_kind_count() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("a", OutlineKind::Function, 1, 5));
        model.add_element(elem("b", OutlineKind::Function, 6, 10));
        model.add_element(elem("c", OutlineKind::Struct, 11, 20));
        assert_eq!(model.kind_count(OutlineKind::Function), 2);
        assert_eq!(model.kind_count(OutlineKind::Struct), 1);
        assert_eq!(model.kind_count(OutlineKind::Enum), 0);
    }

    #[test]
    fn model_summary() {
        let mut model = OutlineModel::new("test.rs");
        model.add_element(elem("main", OutlineKind::Function, 1, 10));
        let s = model.summary();
        assert!(s.contains("test.rs"));
        assert!(s.contains("total=1"));
    }

    #[test]
    fn model_display() {
        let mut model = OutlineModel::new("test.rs");
        model.add_element(elem("main", OutlineKind::Function, 1, 10));
        let s = format!("{model}");
        assert!(s.contains("OutlineModel"));
        assert!(s.contains("test.rs"));
        assert!(s.contains("elements=1"));
    }

    #[test]
    fn element_is_container_and_child_count() {
        let leaf = elem("x", OutlineKind::Variable, 1, 1);
        assert!(!leaf.is_container());
        assert_eq!(leaf.child_count(), 0);

        let parent = elem("Foo", OutlineKind::Class, 1, 50)
            .with_child(elem("a", OutlineKind::Field, 2, 2))
            .with_child(elem("b", OutlineKind::Method, 5, 10));
        assert!(parent.is_container());
        assert_eq!(parent.child_count(), 2);
    }

    #[test]
    fn outline_kind_is_type() {
        assert!(OutlineKind::Class.is_type());
        assert!(OutlineKind::Struct.is_type());
        assert!(OutlineKind::Interface.is_type());
        assert!(OutlineKind::Enum.is_type());
        assert!(!OutlineKind::Function.is_type());
        assert!(!OutlineKind::Variable.is_type());
    }

    #[test]
    fn outline_kind_is_callable() {
        assert!(OutlineKind::Function.is_callable());
        assert!(OutlineKind::Method.is_callable());
        assert!(OutlineKind::Constructor.is_callable());
        assert!(!OutlineKind::Class.is_callable());
        assert!(!OutlineKind::Field.is_callable());
    }

    #[test]
    fn filter_by_kind_returns_matching() {
        let mut model = OutlineModel::new("test.rs");
        model.add_element(elem("main", OutlineKind::Function, 1, 10));
        model.add_element(elem("Foo", OutlineKind::Class, 12, 30)
            .with_child(elem("bar", OutlineKind::Method, 13, 20))
            .with_child(elem("x", OutlineKind::Field, 22, 22)));
        let funcs = model.filter_by_kind(OutlineKind::Function);
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].label, "main");
        let methods = model.filter_by_kind(OutlineKind::Method);
        assert_eq!(methods.len(), 1);
    }

    #[test]
    fn flatten_with_depth_assigns_correct_depths() {
        let mut model = OutlineModel::new("test.rs");
        model.add_element(
            elem("Foo", OutlineKind::Class, 1, 50)
                .with_child(elem("bar", OutlineKind::Method, 2, 10))
        );
        let flat = flatten_with_depth(&model);
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].depth, 0);
        assert_eq!(flat[0].element.label, "Foo");
        assert_eq!(flat[1].depth, 1);
        assert_eq!(flat[1].element.label, "bar");
    }

    #[test]
    fn search_case_insensitive_matches_all() {
        let mut model = OutlineModel::new("test.rs");
        model.add_element(elem("MyFunction", OutlineKind::Function, 1, 5));
        model.add_element(elem("my_var", OutlineKind::Variable, 7, 7));
        let results = model.search("my");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn path_at_line_computes_breadcrumb() {
        let mut model = OutlineModel::new("test.rs");
        model.add_element(
            elem("Foo", OutlineKind::Class, 1, 50)
                .with_child(elem("bar", OutlineKind::Method, 10, 20))
        );
        let path = model.path_at_line(15);
        assert_eq!(path, vec!["Foo", "bar"]);
        let path_root = model.path_at_line(5);
        assert_eq!(path_root, vec!["Foo"]);
        let path_none = model.path_at_line(100);
        assert!(path_none.is_empty());
    }

    #[test]
    fn element_overlaps_and_contains() {
        let a = elem("a", OutlineKind::Class, 1, 50);
        let b = elem("b", OutlineKind::Method, 10, 20);
        let c = elem("c", OutlineKind::Function, 60, 70);
        assert!(a.overlaps(&b));
        assert!(a.contains_element(&b));
        assert!(!a.overlaps(&c));
        assert!(!b.contains_element(&a));
    }

    #[test]
    fn element_line_span() {
        let e = elem("x", OutlineKind::Variable, 5, 5);
        assert_eq!(e.line_span(), 1);
        let e2 = elem("y", OutlineKind::Function, 1, 10);
        assert_eq!(e2.line_span(), 10);
    }

    #[test]
    fn outline_diff_detects_added_and_removed() {
        let mut old = OutlineModel::new("file.rs");
        old.add_element(elem("foo", OutlineKind::Function, 1, 10));
        old.add_element(elem("bar", OutlineKind::Struct, 12, 20));

        let mut new = OutlineModel::new("file.rs");
        new.add_element(elem("foo", OutlineKind::Function, 1, 10));
        new.add_element(elem("baz", OutlineKind::Enum, 12, 25));

        let diffs = outline_diff(&old, &new);
        assert!(diffs.iter().any(|d| matches!(d, OutlineDiff::Removed(l, _) if l == "bar")));
        assert!(diffs.iter().any(|d| matches!(d, OutlineDiff::Added(l, _) if l == "baz")));
        // foo is unchanged, so no diff entry for it
        assert!(!diffs.iter().any(|d| match d {
            OutlineDiff::Added(l, _) | OutlineDiff::Removed(l, _) => l == "foo",
            _ => false,
        }));
    }

    #[test]
    fn outline_diff_detects_moved() {
        let mut old = OutlineModel::new("file.rs");
        old.add_element(elem("main", OutlineKind::Function, 1, 10));

        let mut new = OutlineModel::new("file.rs");
        new.add_element(elem("main", OutlineKind::Function, 5, 15));

        let diffs = outline_diff(&old, &new);
        assert_eq!(diffs.len(), 1);
        assert!(matches!(&diffs[0], OutlineDiff::Moved(l, 1, 10, 5, 15) if l == "main"));
    }

    #[test]
    fn outline_diff_display() {
        let d = OutlineDiff::Added("foo".into(), OutlineKind::Function);
        assert_eq!(d.to_string(), "+ foo (Function)");
        let d = OutlineDiff::Removed("bar".into(), OutlineKind::Struct);
        assert_eq!(d.to_string(), "- bar (Struct)");
        let d = OutlineDiff::Moved("baz".into(), 1, 10, 5, 15);
        assert_eq!(d.to_string(), "~ baz [1-10] -> [5-15]");
    }

    #[test]
    fn find_overlapping_siblings_detects_overlap() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("a", OutlineKind::Function, 1, 15));
        model.add_element(elem("b", OutlineKind::Function, 10, 25));
        model.add_element(elem("c", OutlineKind::Function, 30, 40));
        let overlaps = model.find_overlapping_siblings();
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0], ("a".to_string(), "b".to_string()));
    }

    #[test]
    fn find_range_violations_detects_child_outside_parent() {
        let mut model = OutlineModel::new("file.rs");
        let mut parent = elem("Parent", OutlineKind::Class, 10, 30);
        parent.children.push(elem("ok_child", OutlineKind::Method, 12, 25));
        parent.children.push(elem("bad_child", OutlineKind::Method, 5, 15));
        model.add_element(parent);
        let violations = model.find_range_violations();
        assert_eq!(violations, vec!["bad_child"]);
    }

    #[test]
    fn next_prev_sibling_navigation() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("first", OutlineKind::Function, 1, 10));
        model.add_element(elem("second", OutlineKind::Function, 12, 20));
        model.add_element(elem("third", OutlineKind::Function, 22, 30));

        let next = model.next_sibling_at_line(5).unwrap();
        assert_eq!(next.label, "second");

        let next = model.next_sibling_at_line(15).unwrap();
        assert_eq!(next.label, "third");

        assert!(model.next_sibling_at_line(25).is_none());

        let prev = model.prev_sibling_at_line(15).unwrap();
        assert_eq!(prev.label, "first");

        assert!(model.prev_sibling_at_line(5).is_none());
    }

    #[test]
    fn kind_histogram_counts_and_sorts() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("a", OutlineKind::Function, 1, 5));
        model.add_element(elem("b", OutlineKind::Function, 6, 10));
        model.add_element(elem("c", OutlineKind::Function, 11, 15));
        model.add_element(elem("D", OutlineKind::Struct, 16, 25));
        model.add_element(elem("E", OutlineKind::Constant, 26, 26));
        let hist = model.kind_histogram();
        assert_eq!(hist[0].kind, OutlineKind::Function);
        assert_eq!(hist[0].count, 3);
        assert_eq!(hist.len(), 3);
    }

    // -- build_hierarchy tests ----------------------------------------------

    #[test]
    fn build_hierarchy_nests_contained_symbols() {
        let symbols = vec![
            FlatSymbol { label: "MyClass".into(), kind: OutlineKind::Class, start_line: 1, end_line: 50, detail: None },
            FlatSymbol { label: "method_a".into(), kind: OutlineKind::Method, start_line: 5, end_line: 20, detail: None },
            FlatSymbol { label: "local_var".into(), kind: OutlineKind::Variable, start_line: 10, end_line: 10, detail: None },
            FlatSymbol { label: "standalone".into(), kind: OutlineKind::Function, start_line: 55, end_line: 65, detail: None },
        ];
        let model = build_hierarchy("test.rs", &symbols);
        assert_eq!(model.elements.len(), 2);
        assert_eq!(model.elements[0].label, "MyClass");
        assert_eq!(model.elements[0].children.len(), 1);
        assert_eq!(model.elements[0].children[0].label, "method_a");
        assert_eq!(model.elements[0].children[0].children.len(), 1);
        assert_eq!(model.elements[0].children[0].children[0].label, "local_var");
        assert_eq!(model.elements[1].label, "standalone");
    }

    #[test]
    fn build_hierarchy_empty_input() {
        let model = build_hierarchy("empty.rs", &[]);
        assert!(model.is_elements_empty());
        assert_eq!(model.uri, "empty.rs");
    }

    // -- parent_at_line / first_child_at_line tests -------------------------

    #[test]
    fn parent_at_line_returns_enclosing_element() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("Outer", OutlineKind::Class, 1, 50)
                .with_child(elem("inner", OutlineKind::Method, 10, 20)),
        );
        let parent = model.parent_at_line(15).unwrap();
        assert_eq!(parent.label, "Outer");
        // At the outer level, no parent exists
        assert!(model.parent_at_line(5).is_none());
        // Outside all elements
        assert!(model.parent_at_line(100).is_none());
    }

    #[test]
    fn first_child_at_line_returns_first_child() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("Cls", OutlineKind::Class, 1, 50)
                .with_child(elem("alpha", OutlineKind::Method, 5, 10))
                .with_child(elem("beta", OutlineKind::Method, 12, 20)),
        );
        // Cursor on Cls but not inside a child
        let child = model.first_child_at_line(3).unwrap();
        assert_eq!(child.label, "alpha");
        // Cursor inside alpha (leaf) – no children
        assert!(model.first_child_at_line(7).is_none());
    }

    // -- elements_at_depth tests --------------------------------------------

    #[test]
    fn elements_at_depth_returns_correct_level() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("Root", OutlineKind::Class, 1, 50)
                .with_child(
                    elem("Mid", OutlineKind::Method, 5, 40)
                        .with_child(elem("Leaf", OutlineKind::Variable, 10, 10)),
                ),
        );
        model.add_element(elem("TopFn", OutlineKind::Function, 55, 65));

        let d0 = model.elements_at_depth(0);
        assert_eq!(d0.len(), 2);

        let d1 = model.elements_at_depth(1);
        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0].label, "Mid");

        let d2 = model.elements_at_depth(2);
        assert_eq!(d2.len(), 1);
        assert_eq!(d2[0].label, "Leaf");

        let d3 = model.elements_at_depth(3);
        assert!(d3.is_empty());
    }

    // -- leaves tests -------------------------------------------------------

    #[test]
    fn leaves_returns_only_childless_elements() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("Parent", OutlineKind::Class, 1, 50)
                .with_child(elem("child_a", OutlineKind::Field, 2, 2))
                .with_child(elem("child_b", OutlineKind::Method, 5, 10)),
        );
        model.add_element(elem("lonely", OutlineKind::Function, 55, 60));
        let lvs = model.leaves();
        let labels: Vec<&str> = lvs.iter().map(|e| e.label.as_str()).collect();
        assert!(labels.contains(&"child_a"));
        assert!(labels.contains(&"child_b"));
        assert!(labels.contains(&"lonely"));
        assert!(!labels.contains(&"Parent"));
    }

    // -- total_line_span tests ----------------------------------------------

    #[test]
    fn total_line_span_covers_full_range() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("a", OutlineKind::Function, 5, 15));
        model.add_element(elem("b", OutlineKind::Function, 20, 30));
        assert_eq!(model.total_line_span(), 26); // 30 - 5 + 1
    }

    #[test]
    fn total_line_span_empty_model() {
        let model = OutlineModel::new("empty.rs");
        assert_eq!(model.total_line_span(), 0);
    }

    // -- descendant_count / subtree_depth tests -----------------------------

    #[test]
    fn element_descendant_count() {
        let e = elem("Root", OutlineKind::Class, 1, 50)
            .with_child(
                elem("Mid", OutlineKind::Method, 5, 40)
                    .with_child(elem("Leaf1", OutlineKind::Variable, 10, 10))
                    .with_child(elem("Leaf2", OutlineKind::Variable, 15, 15)),
            )
            .with_child(elem("Other", OutlineKind::Field, 42, 42));
        assert_eq!(e.descendant_count(), 4);
    }

    #[test]
    fn element_subtree_depth() {
        let leaf = elem("x", OutlineKind::Variable, 1, 1);
        assert_eq!(leaf.subtree_depth(), 1);

        let nested = elem("A", OutlineKind::Class, 1, 50)
            .with_child(
                elem("B", OutlineKind::Method, 5, 40)
                    .with_child(elem("C", OutlineKind::Variable, 10, 10)),
            );
        assert_eq!(nested.subtree_depth(), 3);
    }

    // -- contains_line / subtree_kinds tests --------------------------------

    #[test]
    fn element_contains_line() {
        let e = elem("fn", OutlineKind::Function, 10, 20);
        assert!(e.contains_line(10));
        assert!(e.contains_line(15));
        assert!(e.contains_line(20));
        assert!(!e.contains_line(9));
        assert!(!e.contains_line(21));
    }

    #[test]
    fn element_subtree_kinds() {
        let e = elem("Cls", OutlineKind::Class, 1, 50)
            .with_child(elem("m", OutlineKind::Method, 5, 20))
            .with_child(elem("f", OutlineKind::Field, 22, 22));
        let kinds = e.subtree_kinds();
        assert!(kinds.contains(&OutlineKind::Class));
        assert!(kinds.contains(&OutlineKind::Method));
        assert!(kinds.contains(&OutlineKind::Field));
        assert_eq!(kinds.len(), 3);
    }

    // -- elements_in_range tests --------------------------------------------

    #[test]
    fn elements_in_range_returns_intersecting() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("a", OutlineKind::Function, 1, 10));
        model.add_element(elem("b", OutlineKind::Function, 15, 25));
        model.add_element(elem("c", OutlineKind::Function, 30, 40));
        let result = model.elements_in_range(8, 20);
        let labels: Vec<&str> = result.iter().map(|e| e.label.as_str()).collect();
        assert!(labels.contains(&"a"));
        assert!(labels.contains(&"b"));
        assert!(!labels.contains(&"c"));
    }

    // -- largest / smallest element tests -----------------------------------

    #[test]
    fn largest_and_smallest_element() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("small", OutlineKind::Variable, 1, 1));
        model.add_element(elem("big", OutlineKind::Function, 5, 50));
        model.add_element(elem("mid", OutlineKind::Struct, 55, 65));

        assert_eq!(model.largest_element().unwrap().label, "big");
        assert_eq!(model.smallest_element().unwrap().label, "small");
    }

    // -- structurally_equal tests -------------------------------------------

    #[test]
    fn structurally_equal_same_symbols() {
        let mut a = OutlineModel::new("a.rs");
        a.add_element(elem("foo", OutlineKind::Function, 1, 10));
        a.add_element(elem("Bar", OutlineKind::Struct, 12, 20));

        let mut b = OutlineModel::new("b.rs");
        b.add_element(elem("foo", OutlineKind::Function, 5, 15));
        b.add_element(elem("Bar", OutlineKind::Struct, 20, 30));

        assert!(a.structurally_equal(&b));
    }

    #[test]
    fn structurally_equal_different_symbols() {
        let mut a = OutlineModel::new("a.rs");
        a.add_element(elem("foo", OutlineKind::Function, 1, 10));

        let mut b = OutlineModel::new("b.rs");
        b.add_element(elem("bar", OutlineKind::Function, 1, 10));

        assert!(!a.structurally_equal(&b));
    }

    // -- OutlineKind::is_value / is_container_kind tests --------------------

    #[test]
    fn outline_kind_is_value() {
        assert!(OutlineKind::Variable.is_value());
        assert!(OutlineKind::Constant.is_value());
        assert!(OutlineKind::Field.is_value());
        assert!(OutlineKind::Property.is_value());
        assert!(!OutlineKind::Function.is_value());
        assert!(!OutlineKind::Class.is_value());
    }

    #[test]
    fn outline_kind_is_container_kind() {
        assert!(OutlineKind::File.is_container_kind());
        assert!(OutlineKind::Module.is_container_kind());
        assert!(OutlineKind::Namespace.is_container_kind());
        assert!(!OutlineKind::Class.is_container_kind());
        assert!(!OutlineKind::Function.is_container_kind());
    }

    // -- walk tests ---------------------------------------------------------

    #[test]
    fn walk_visits_all_elements() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("A", OutlineKind::Class, 1, 50)
                .with_child(elem("B", OutlineKind::Method, 5, 20)),
        );
        model.add_element(elem("C", OutlineKind::Function, 55, 60));
        let mut visited = Vec::new();
        model.walk(|e, d| {
            visited.push((e.label.clone(), d));
            true
        });
        assert_eq!(visited, vec![
            ("A".to_string(), 0),
            ("B".to_string(), 1),
            ("C".to_string(), 0),
        ]);
    }

    #[test]
    fn sort_toggler_default_is_position() {
        let toggler = OutlineSortToggler::new();
        assert_eq!(toggler.current(), OutlineSortMode::ByPosition);
    }

    #[test]
    fn sort_toggler_toggle_cycles() {
        let mut toggler = OutlineSortToggler::new();
        assert_eq!(toggler.toggle(), OutlineSortMode::ByName);
        assert_eq!(toggler.toggle(), OutlineSortMode::ByKind);
        assert_eq!(toggler.toggle(), OutlineSortMode::ByNameLength);
        assert_eq!(toggler.toggle(), OutlineSortMode::ByPosition);
    }

    #[test]
    fn sort_toggler_toggle_back() {
        let mut toggler = OutlineSortToggler::new();
        assert_eq!(toggler.toggle_back(), OutlineSortMode::ByNameLength);
        assert_eq!(toggler.toggle_back(), OutlineSortMode::ByKind);
    }

    #[test]
    fn sort_toggler_set_mode() {
        let mut toggler = OutlineSortToggler::new();
        assert!(toggler.set_mode(OutlineSortMode::ByKind));
        assert_eq!(toggler.current(), OutlineSortMode::ByKind);
    }

    #[test]
    fn sort_toggler_toggle_count() {
        let mut toggler = OutlineSortToggler::new();
        toggler.toggle();
        toggler.toggle();
        assert_eq!(toggler.toggle_count(), 2);
    }

    #[test]
    fn sort_toggler_sort_by_name() {
        let mut toggler = OutlineSortToggler::new();
        toggler.set_mode(OutlineSortMode::ByName);
        let mut elems = vec![
            OutlineElement { label: "Zeta".into(), kind: OutlineKind::Function, range_start_line: 1, range_end_line: 10, detail: None, children: vec![] },
            OutlineElement { label: "Alpha".into(), kind: OutlineKind::Function, range_start_line: 20, range_end_line: 30, detail: None, children: vec![] },
        ];
        toggler.sort_elements(&mut elems);
        assert_eq!(elems[0].label, "Alpha");
        assert_eq!(elems[1].label, "Zeta");
    }

    #[test]
    fn sort_toggler_sort_by_position() {
        let mut toggler = OutlineSortToggler::new();
        let mut elems = vec![
            OutlineElement { label: "B".into(), kind: OutlineKind::Function, range_start_line: 20, range_end_line: 30, detail: None, children: vec![] },
            OutlineElement { label: "A".into(), kind: OutlineKind::Function, range_start_line: 1, range_end_line: 10, detail: None, children: vec![] },
        ];
        toggler.sort_elements(&mut elems);
        assert_eq!(elems[0].label, "A");
    }

    #[test]
    fn sort_toggler_display() {
        let toggler = OutlineSortToggler::new();
        let s = format!("{toggler}");
        assert!(s.contains("ByPosition"));
        assert!(s.contains("0 times"));
    }

    #[test]
    fn sort_toggler_reset() {
        let mut toggler = OutlineSortToggler::new();
        toggler.toggle();
        toggler.toggle();
        toggler.reset();
        assert_eq!(toggler.current(), OutlineSortMode::ByPosition);
        assert_eq!(toggler.toggle_count(), 0);
    }

    #[test]
    fn filter_by_kind_accepts_all_by_default() {
        let filter = OutlineFilterByKind::new();
        assert!(filter.accepts(OutlineKind::Function));
        assert!(filter.accepts(OutlineKind::Class));
        assert!(filter.is_default());
    }

    #[test]
    fn filter_by_kind_only() {
        let filter = OutlineFilterByKind::only(vec![OutlineKind::Function, OutlineKind::Method]);
        assert!(filter.accepts(OutlineKind::Function));
        assert!(filter.accepts(OutlineKind::Method));
        assert!(!filter.accepts(OutlineKind::Class));
    }

    #[test]
    fn filter_by_kind_excluding() {
        let filter = OutlineFilterByKind::excluding(vec![OutlineKind::Variable]);
        assert!(filter.accepts(OutlineKind::Function));
        assert!(!filter.accepts(OutlineKind::Variable));
    }

    #[test]
    fn filter_by_kind_apply() {
        let mut filter = OutlineFilterByKind::only(vec![OutlineKind::Function]);
        let elems = vec![
            OutlineElement { label: "foo".into(), kind: OutlineKind::Function, range_start_line: 1, range_end_line: 10, detail: None, children: vec![] },
            OutlineElement { label: "Bar".into(), kind: OutlineKind::Class, range_start_line: 20, range_end_line: 30, detail: None, children: vec![] },
            OutlineElement { label: "baz".into(), kind: OutlineKind::Function, range_start_line: 40, range_end_line: 50, detail: None, children: vec![] },
        ];
        let result = filter.apply(&elems);
        assert_eq!(result.len(), 2);
        assert_eq!(filter.apply_count(), 1);
    }

    #[test]
    fn filter_by_kind_count_matching() {
        let filter = OutlineFilterByKind::excluding(vec![OutlineKind::Variable]);
        let elems = vec![
            OutlineElement { label: "x".into(), kind: OutlineKind::Variable, range_start_line: 1, range_end_line: 2, detail: None, children: vec![] },
            OutlineElement { label: "f".into(), kind: OutlineKind::Function, range_start_line: 3, range_end_line: 4, detail: None, children: vec![] },
        ];
        assert_eq!(filter.count_matching(&elems), 1);
        assert_eq!(filter.count_filtered_out(&elems), 1);
    }

    #[test]
    fn filter_by_kind_reset() {
        let mut filter = OutlineFilterByKind::only(vec![OutlineKind::Function]);
        filter.reset();
        assert!(filter.is_default());
    }

    #[test]
    fn filter_by_kind_display() {
        let filter = OutlineFilterByKind::new();
        let s = format!("{filter}");
        assert!(s.contains("allowed=0"));
        assert!(s.contains("excluded=0"));
    }



    // -- outline_view additional tests -------------------------------------------

    #[test]
    fn x_outline_view_panel_state_new() {
        let p = XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XOutlineViewLayoutRegion::Sidebar);
    }

    #[test]
    fn x_outline_view_panel_area() {
        let p = XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_outline_view_panel_toggle() {
        let mut p = XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_outline_view_panel_resize() {
        let mut p = XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_outline_view_panel_is_narrow() {
        let mut p = XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_outline_view_total_visible_area_basic() {
        let panels = vec![
            XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Sidebar, "a"),
            XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_outline_view_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_outline_view_total_visible_area_hidden() {
        let mut panels = vec![
            XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Sidebar, "a"),
            XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_outline_view_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_outline_view_count_in_region_basic() {
        let panels = vec![
            XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Sidebar, "a"),
            XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Sidebar, "b"),
            XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_outline_view_count_in_region(&panels, XOutlineViewLayoutRegion::Sidebar), 2);
        assert_eq!(x_outline_view_count_in_region(&panels, XOutlineViewLayoutRegion::Editor), 1);
        assert_eq!(x_outline_view_count_in_region(&panels, XOutlineViewLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_outline_view_widest_panel_basic() {
        let mut panels = vec![
            XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Sidebar, "narrow"),
            XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_outline_view_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_outline_view_collapse_region_basic() {
        let mut panels = vec![
            XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Sidebar, "a"),
            XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Sidebar, "b"),
            XOutlineViewPanelState::new(XOutlineViewLayoutRegion::Editor, "c"),
        ];
        x_outline_view_collapse_region(&mut panels, XOutlineViewLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_outline_view_layout_constraint_clamp() {
        let lc = XOutlineViewLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_outline_view_layout_constraint_satisfied() {
        let lc = XOutlineViewLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_outline_view_widest_panel_empty() {
        let panels: Vec<XOutlineViewPanelState> = vec![];
        assert!(x_outline_view_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_outline_view_layout_region_eq() {
        assert_eq!(XOutlineViewLayoutRegion::Sidebar, XOutlineViewLayoutRegion::Sidebar);
        assert_ne!(XOutlineViewLayoutRegion::Sidebar, XOutlineViewLayoutRegion::Panel);
    }


    // -- outline_view extended domain tests ----------------------------------------

    #[test]
    fn y_outline_view_enum_index() {
        assert_eq!(YOutlineViewOutlineSortOrder::ByPosition.index(), 0);
        assert_eq!(YOutlineViewOutlineSortOrder::ByName.index(), 1);
        assert_eq!(YOutlineViewOutlineSortOrder::ByKind.index(), 2);
        assert_eq!(YOutlineViewOutlineSortOrder::ByCategory.index(), 3);
    }

    #[test]
    fn y_outline_view_enum_label() {
        assert_eq!(YOutlineViewOutlineSortOrder::ByPosition.label(), "ByPosition");
        assert_eq!(YOutlineViewOutlineSortOrder::ByName.label(), "ByName");
        assert_eq!(YOutlineViewOutlineSortOrder::ByKind.label(), "ByKind");
        assert_eq!(YOutlineViewOutlineSortOrder::ByCategory.label(), "ByCategory");
    }

    #[test]
    fn y_outline_view_enum_all() {
        let all = YOutlineViewOutlineSortOrder::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_outline_view_enum_is_default() {
        assert!(YOutlineViewOutlineSortOrder::ByPosition.is_default());
        assert!(!YOutlineViewOutlineSortOrder::ByCategory.is_default());
    }

    #[test]
    fn y_outline_view_enum_display() {
        assert_eq!(format!("{}", YOutlineViewOutlineSortOrder::ByPosition), "ByPosition");
    }

    #[test]
    fn y_outline_view_struct_new() {
        let s = YOutlineViewOutlineFilter::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_outline_view_struct_clear() {
        let mut s = YOutlineViewOutlineFilter::new();
        s.patterns.push("test".into());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_outline_view_fingerprint_deterministic() {
        let h1 = y_outline_view_fingerprint("hello");
        let h2 = y_outline_view_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_outline_view_fingerprint("a"), y_outline_view_fingerprint("b"));
    }

    #[test]
    fn y_outline_view_truncate_short() {
        assert_eq!(y_outline_view_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_outline_view_truncate_long() {
        let r = y_outline_view_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_outline_view_normalize_key_basic() {
        assert_eq!(y_outline_view_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_outline_view_split_path_basic() {
        let parts = y_outline_view_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_outline_view_count_occurrences_basic() {
        assert_eq!(y_outline_view_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_outline_view_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_outline_view_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_outline_view_in_range_basic() {
        assert!(y_outline_view_in_range(5, 1, 10));
        assert!(y_outline_view_in_range(1, 1, 10));
        assert!(y_outline_view_in_range(10, 1, 10));
        assert!(!y_outline_view_in_range(0, 1, 10));
        assert!(!y_outline_view_in_range(11, 1, 10));
    }

    #[test]
    fn y_outline_view_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_outline_view_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_outline_view_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_outline_view_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- outline_view Z-extended tests -----------------------------------------------

    #[test]
    fn z_outline_view_priority_weight() {
        assert_eq!(ZOutlineViewPriority::Idle.weight(), 0);
        assert_eq!(ZOutlineViewPriority::Normal.weight(), 2);
        assert_eq!(ZOutlineViewPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_outline_view_priority_label() {
        assert_eq!(ZOutlineViewPriority::Low.label(), "low");
        assert_eq!(ZOutlineViewPriority::High.label(), "high");
    }

    #[test]
    fn z_outline_view_priority_is_elevated() {
        assert!(!ZOutlineViewPriority::Normal.is_elevated());
        assert!(ZOutlineViewPriority::High.is_elevated());
        assert!(ZOutlineViewPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_outline_view_priority_display() {
        assert_eq!(format!("{}", ZOutlineViewPriority::Idle), "idle");
    }

    #[test]
    fn z_outline_view_priority_all_asc() {
        let all = ZOutlineViewPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZOutlineViewPriority::Idle);
        assert_eq!(all[4], ZOutlineViewPriority::Realtime);
    }

    #[test]
    fn z_outline_view_struct_new() {
        let s = ZOutlineViewOutlineBreadcrumb::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_outline_view_struct_toggled_clone() {
        let s = ZOutlineViewOutlineBreadcrumb::new();
        let t = s.toggled_clone();
        let _ = t.max_depth;
    }

    #[test]
    fn z_outline_view_rolling_hash_deterministic() {
        let h1 = z_outline_view_rolling_hash(b"test");
        let h2 = z_outline_view_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_outline_view_rolling_hash(b"a"), z_outline_view_rolling_hash(b"b"));
    }

    #[test]
    fn z_outline_view_pad_to_basic() {
        assert_eq!(z_outline_view_pad_to("hi", 5), "hi   ");
        assert_eq!(z_outline_view_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_outline_view_is_identifier_basic() {
        assert!(z_outline_view_is_identifier("foo_bar"));
        assert!(z_outline_view_is_identifier("abc123"));
        assert!(!z_outline_view_is_identifier(""));
        assert!(!z_outline_view_is_identifier("has space"));
    }

    #[test]
    fn z_outline_view_levenshtein_basic() {
        assert_eq!(z_outline_view_levenshtein("", ""), 0);
        assert_eq!(z_outline_view_levenshtein("abc", "abc"), 0);
        assert_eq!(z_outline_view_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_outline_view_unique_words_basic() {
        let w = z_outline_view_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_outline_view_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_outline_view_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_outline_view_common_prefix_basic() {
        assert_eq!(z_outline_view_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_outline_view_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_outline_view_struct_clear() {
        let mut s = ZOutlineViewOutlineBreadcrumb::new();
        s.segments.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_outline_view_rolling_hash_empty() {
        let h = z_outline_view_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }
}
