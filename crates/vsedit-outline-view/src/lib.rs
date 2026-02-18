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


// ---------------------------------------------------------------------------
// xb_ utilities – batch 67
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer67 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer67 {
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
pub fn xb_fnv1a_67(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_67<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_67<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_67(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_67(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 133
// ---------------------------------------------------------------------------

/// Generic object pool `Xc133Pool<T>`.
pub struct Xc133Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc133Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc133PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc133Pool<T> {
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
    pub fn stats(&self) -> Xc133PoolStats {
        Xc133PoolStats {
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

impl<T> Default for Xc133Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc133Scheduler`.
pub struct Xc133Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc133Scheduler {
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

impl Default for Xc133Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_133 hash for the given byte slice.
pub fn xc_133_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_133 convention.
pub fn xc_133_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe80 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe80Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe80PipelineError {
    pub stage: Xe80Stage,
    pub message: String,
}

impl std::fmt::Display for Xe80PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe80Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe80Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe80PipelineError>>>,
    stage_names: Vec<Xe80Stage>,
}

impl Xe80Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe80PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe80Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe80PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe80Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe80PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe80Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe80PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe80Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe80PipelineError> {
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

    pub fn compose(mut self, other: Xe80Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe80CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe80CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe80Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe80CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe80CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe80Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe80CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_80_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe80CacheEntry {
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

    fn xe_80_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe80CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_80_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe80PipelineError> {
    Ok(data)
}

pub fn xe_80_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe80PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_80_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe80PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_80_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe80PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_80_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe80PipelineError> {
    Err(Xe80PipelineError {
        stage: Xe80Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_78: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg78Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg78Graph {
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

impl Default for Xg78Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_78: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg78Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg78Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg78Heap<T>) {
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

impl<T: Ord> Default for Xg78Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 132).
pub struct Xh132SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh132SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 174 as u64,
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

/// A compact bit set supporting boolean operations (variant 132).
pub struct Xh132BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh132BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 132).
pub struct Xi132Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi132Deque<T> {
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
pub struct Xi132Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi132Interval {
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

/// A simple interval tree (variant 132).
pub struct Xi132IntervalTree {
    xi_intervals: Vec<Xi132Interval>,
}

impl Xi132IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi132Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi132Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi132Interval) -> Vec<&Xi132Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi132Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi132Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi132Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi132Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi132Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi132Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 132) ---

/// Disjoint set / union-find for crate 132.
pub struct Xj132UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj132UnionFind {
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

const XJ132_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 132.
pub struct Xj132BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj132BTreeNode<K, V>>>,
    len: usize,
}

struct Xj132BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj132BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj132BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ132_BTREE_ORDER - 1
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
        let mid = XJ132_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj132BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj132BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj132BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj132BTreeNode::xj_new_leaf();
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


// --- xk_132 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk132SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk132SegmentTree {
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
pub struct Xk132DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk132DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_132).
#[derive(Debug, Clone)]
pub struct Xl132Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl132Rope {
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

/// Suffix array for efficient string searching (xl_132).
#[derive(Debug, Clone)]
pub struct Xl132SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl132SuffixArray {
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

    #[test]
    fn xb_ring_buffer_67_push_and_len() {
        let mut rb = super::XbRingBuffer67::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_67_overwrite() {
        let mut rb = super::XbRingBuffer67::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_67_get_out_of_bounds() {
        let rb = super::XbRingBuffer67::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_67_drain_all() {
        let mut rb = super::XbRingBuffer67::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_67_peek_front_back() {
        let mut rb = super::XbRingBuffer67::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_67_clear() {
        let mut rb = super::XbRingBuffer67::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_67_capacity() {
        let rb = super::XbRingBuffer67::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_67_basic() {
        let h = super::xb_fnv1a_67(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_67(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_67_different_inputs() {
        let h1 = super::xb_fnv1a_67(b"abc");
        let h2 = super::xb_fnv1a_67(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_67_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_67(&data);
        let dec = super::xb_rle_decode_67(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_67_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_67(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_67(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_67_values() {
        assert!((super::xb_clamp_67(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_67(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_67(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_67_values() {
        assert!((super::xb_lerp_67(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_67(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_67(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_67_wrap_around_twice() {
        let mut rb = super::XbRingBuffer67::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 133 ----

    #[test]
    fn xc_133_pool_new_empty() {
        let pool: super::Xc133Pool<i32> = super::Xc133Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_133_pool_release_acquire() {
        let mut pool = super::Xc133Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_133_pool_acquire_empty() {
        let mut pool: super::Xc133Pool<i32> = super::Xc133Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_133_pool_full() {
        let mut pool = super::Xc133Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_133_pool_drain() {
        let mut pool = super::Xc133Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_133_pool_stats() {
        let mut pool = super::Xc133Pool::new(8);
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
    fn xc_133_pool_clear() {
        let mut pool = super::Xc133Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_133_pool_shrink() {
        let mut pool = super::Xc133Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_133_pool_default() {
        let pool: super::Xc133Pool<String> = super::Xc133Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_133_pool_extend() {
        let mut pool = super::Xc133Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_133_pool_retain() {
        let mut pool = super::Xc133Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_133_scheduler_round_robin() {
        let mut sched = super::Xc133Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_133_scheduler_empty() {
        let mut sched = super::Xc133Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_133_scheduler_reset() {
        let mut sched = super::Xc133Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_133_scheduler_add_remove() {
        let mut sched = super::Xc133Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_133_scheduler_targets() {
        let sched = super::Xc133Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_133_hash_empty() {
        assert_eq!(super::xc_133_hash(b""), 5381);
    }

    #[test]
    fn xc_133_hash_data() {
        let h = super::xc_133_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_133_hash(b"hello"), h);
    }

    #[test]
    fn xc_133_reverse_str() {
        assert_eq!(super::xc_133_reverse("abc"), "cba");
        assert_eq!(super::xc_133_reverse(""), "");
    }


    #[test]
    fn xe_80_pipeline_empty() {
        let p = super::Xe80Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_80_pipeline_parse_stage() {
        let p = super::Xe80Pipeline::new()
            .add_parse(super::xe_80_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_80_pipeline_transform_double() {
        let p = super::Xe80Pipeline::new()
            .add_transform(super::xe_80_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_80_pipeline_validate_reverse() {
        let p = super::Xe80Pipeline::new()
            .add_validate(super::xe_80_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_80_pipeline_emit_filter() {
        let p = super::Xe80Pipeline::new()
            .add_emit(super::xe_80_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_80_pipeline_multi_stage() {
        let p = super::Xe80Pipeline::new()
            .add_parse(super::xe_80_pipeline_identity)
            .add_transform(super::xe_80_pipeline_double)
            .add_validate(super::xe_80_pipeline_reverse)
            .add_emit(super::xe_80_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_80_pipeline_error_propagation() {
        let p = super::Xe80Pipeline::new()
            .add_parse(super::xe_80_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe80Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_80_pipeline_compose() {
        let p1 = super::Xe80Pipeline::new()
            .add_parse(super::xe_80_pipeline_identity);
        let p2 = super::Xe80Pipeline::new()
            .add_transform(super::xe_80_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_80_pipeline_error_display() {
        let e = super::Xe80PipelineError {
            stage: super::Xe80Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_80_cache_put_get() {
        let mut c = super::Xe80Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_80_cache_miss() {
        let mut c: super::Xe80Cache<&str, i32> = super::Xe80Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_80_cache_ttl_expiry() {
        let mut c = super::Xe80Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_80_cache_evict() {
        let mut c = super::Xe80Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_80_cache_capacity() {
        let mut c = super::Xe80Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_80_cache_stats() {
        let mut c = super::Xe80Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_80_cache_clear() {
        let mut c = super::Xe80Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_78 graph tests ------------------------------------------------

    #[test]
    fn xg_78_graph_empty() {
        let g = super::Xg78Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_78_graph_add_node() {
        let mut g = super::Xg78Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_78_graph_add_edge() {
        let mut g = super::Xg78Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_78_graph_neighbors() {
        let mut g = super::Xg78Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_78_graph_has_path() {
        let mut g = super::Xg78Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_78_graph_self_path() {
        let g = super::Xg78Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_78_graph_topo_sort() {
        let mut g = super::Xg78Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_78_graph_cycle_detect_false() {
        let mut g = super::Xg78Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_78_graph_cycle_detect_true() {
        let mut g = super::Xg78Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_78 heap tests -------------------------------------------------

    #[test]
    fn xg_78_heap_empty() {
        let h: super::Xg78Heap<i32> = super::Xg78Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_78_heap_push_pop() {
        let mut h = super::Xg78Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_78_heap_peek() {
        let mut h = super::Xg78Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_78_heap_drain_sorted() {
        let mut h = super::Xg78Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_78_heap_merge() {
        let mut a = super::Xg78Heap::new();
        let mut b = super::Xg78Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_78_heap_default() {
        let h: super::Xg78Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_78_graph_default() {
        let g: super::Xg78Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh132_skip_insert_contains() {
        let mut sl = super::Xh132SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh132_skip_remove() {
        let mut sl = super::Xh132SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh132_skip_len() {
        let mut sl = super::Xh132SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh132_skip_range_query() {
        let mut sl = super::Xh132SkipList::xh_new(4);
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
    fn xh132_skip_floor_ceiling() {
        let mut sl = super::Xh132SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh132_skip_rank() {
        let mut sl = super::Xh132SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh132_skip_empty() {
        let sl = super::Xh132SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh132_skip_duplicates() {
        let mut sl = super::Xh132SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh132_bitset_set_test() {
        let mut bs = super::Xh132BitSet::xh_new(256);
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
    fn xh132_bitset_clear_count() {
        let mut bs = super::Xh132BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh132_bitset_and_or_xor() {
        let mut a = super::Xh132BitSet::xh_new(128);
        let mut b = super::Xh132BitSet::xh_new(128);
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
    fn xh132_bitset_iter_ones() {
        let mut bs = super::Xh132BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh132_bitset_first_last() {
        let mut bs = super::Xh132BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh132_bitset_empty() {
        let bs = super::Xh132BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi132_deque_push_pop_back() {
        let mut dq = super::Xi132Deque::xi_new(4);
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
    fn xi132_deque_push_pop_front() {
        let mut dq = super::Xi132Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi132_deque_mixed_ops() {
        let mut dq = super::Xi132Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi132_deque_get_and_split() {
        let mut dq = super::Xi132Deque::xi_new(8);
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
    fn xi132_deque_rotate_left() {
        let mut dq = super::Xi132Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi132_deque_rotate_right() {
        let mut dq = super::Xi132Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi132_deque_grow() {
        let mut dq = super::Xi132Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi132_deque_empty() {
        let dq = super::Xi132Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi132_interval_tree_insert_query() {
        let mut tree = super::Xi132IntervalTree::xi_new();
        tree.xi_insert(super::Xi132Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi132Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi132Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi132_interval_tree_overlap() {
        let mut tree = super::Xi132IntervalTree::xi_new();
        tree.xi_insert(super::Xi132Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi132Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi132Interval::xi_new(12, 20));
        let q = super::Xi132Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi132_interval_tree_remove() {
        let mut tree = super::Xi132IntervalTree::xi_new();
        tree.xi_insert(super::Xi132Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi132Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi132_interval_tree_gaps() {
        let mut tree = super::Xi132IntervalTree::xi_new();
        tree.xi_insert(super::Xi132Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi132Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi132Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi132Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi132Interval::xi_new(8, 10));
    }

    #[test]
    fn xi132_interval_tree_merge() {
        let mut tree = super::Xi132IntervalTree::xi_new();
        tree.xi_insert(super::Xi132Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi132Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi132Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi132Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi132Interval::xi_new(10, 15));
    }

    #[test]
    fn xi132_interval_tree_all() {
        let mut tree = super::Xi132IntervalTree::xi_new();
        tree.xi_insert(super::Xi132Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi132Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi132_interval_tree_empty() {
        let tree = super::Xi132IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi132_interval_tree_contains_point() {
        let iv = super::Xi132Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 132) ---

    #[test]
    fn xj_132_uf_make_and_find() {
        let mut uf = super::Xj132UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_132_uf_union_connected() {
        let mut uf = super::Xj132UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_132_uf_component_count() {
        let mut uf = super::Xj132UnionFind::xj_new();
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
    fn xj_132_uf_component_size() {
        let mut uf = super::Xj132UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_132_uf_largest_component() {
        let mut uf = super::Xj132UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_132_uf_many_elements() {
        let mut uf = super::Xj132UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_132_uf_separate_components() {
        let mut uf = super::Xj132UnionFind::xj_new();
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
    fn xj_132_uf_path_compression() {
        let mut uf = super::Xj132UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_132_bt_insert_get() {
        let mut bt = super::Xj132BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_132_bt_contains_len() {
        let mut bt = super::Xj132BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_132_bt_replace() {
        let mut bt = super::Xj132BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_132_bt_remove() {
        let mut bt = super::Xj132BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_132_bt_keys_values() {
        let mut bt = super::Xj132BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_132_bt_range() {
        let mut bt = super::Xj132BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_132_bt_min_max() {
        let mut bt = super::Xj132BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_132_bt_many_inserts() {
        let mut bt = super::Xj132BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_132 segment tree tests ---

    #[test]
    fn xk_132_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk132SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_132_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk132SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_132_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk132SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_132_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk132SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_132_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk132SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_132_st_single_element() {
        let data = vec![42];
        let st = super::Xk132SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_132_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk132SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_132_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk132SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_132 disjoint intervals tests ---

    #[test]
    fn xk_132_di_add_and_count() {
        let mut di = super::Xk132DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_132_di_merge_overlap() {
        let mut di = super::Xk132DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_132_di_contains() {
        let mut di = super::Xk132DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_132_di_remove() {
        let mut di = super::Xk132DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_132_di_covered_length() {
        let mut di = super::Xk132DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_132_di_gaps() {
        let mut di = super::Xk132DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_132_di_merge_adjacent() {
        let mut di = super::Xk132DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_132_di_empty() {
        let di = super::Xk132DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_132_rope_new_empty() {
        let rope = super::Xl132Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_132_rope_from_str() {
        let rope = super::Xl132Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_132_rope_insert_at() {
        let mut rope = super::Xl132Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_132_rope_delete_range() {
        let mut rope = super::Xl132Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_132_rope_char_at() {
        let rope = super::Xl132Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_132_rope_split_concat() {
        let rope = super::Xl132Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_132_rope_line_count() {
        let rope = super::Xl132Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_132_rope_line_at() {
        let rope = super::Xl132Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_132_sa_build_and_search() {
        let sa = super::Xl132SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_132_sa_count() {
        let sa = super::Xl132SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_132_sa_longest_repeated() {
        let sa = super::Xl132SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_132_sa_all_positions() {
        let sa = super::Xl132SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_132_sa_len() {
        let sa = super::Xl132SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_132_sa_empty() {
        let sa = super::Xl132SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_132_rope_slice() {
        let rope = super::Xl132Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_132_sa_search_start() {
        let sa = super::Xl132SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
