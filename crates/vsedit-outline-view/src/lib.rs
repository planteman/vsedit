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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    fn filter_by_kind() {
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
    fn breadcrumb_at_line() {
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
    fn sort_by_name() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("Zebra", OutlineKind::Struct, 1, 10));
        model.add_element(elem("alpha", OutlineKind::Function, 12, 20));
        model.add_element(elem("Beta", OutlineKind::Constant, 22, 25));
        model.sort_by_name();
        let names: Vec<_> = model.elements.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(names, vec!["alpha", "Beta", "Zebra"]);
    }

    #[test]
    fn sort_by_position() {
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
}
