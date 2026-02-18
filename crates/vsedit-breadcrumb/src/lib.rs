//! Path breadcrumb navigation.
//!
//! Provides breadcrumb data structures and a renderable navigation bar
//! with keyboard-navigable segments — rendered via ratatui.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

/// The kind of a breadcrumb element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreadcrumbKind {
    File,
    Folder,
    Symbol,
    Class,
    Function,
    Method,
    Property,
    Enum,
    Interface,
    Module,
}

/// A single element in a breadcrumb path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbElement {
    pub label: String,
    pub kind: BreadcrumbKind,
    pub uri: Option<String>,
    pub range_start_line: Option<u32>,
}

/// An ordered sequence of breadcrumb elements representing a navigation path.
#[derive(Debug, Clone, Default)]
pub struct BreadcrumbPath {
    pub elements: Vec<BreadcrumbElement>,
}

impl BreadcrumbPath {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }

    pub fn push(&mut self, element: BreadcrumbElement) {
        self.elements.push(element);
    }

    pub fn pop(&mut self) -> Option<BreadcrumbElement> {
        self.elements.pop()
    }

    pub fn last(&self) -> Option<&BreadcrumbElement> {
        self.elements.last()
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Join all element labels with `" > "`.
    pub fn to_path_string(&self) -> String {
        self.elements
            .iter()
            .map(|e| e.label.as_str())
            .collect::<Vec<_>>()
            .join(" > ")
    }
}

/// Trait for types that can produce breadcrumb paths for a given location.
pub trait BreadcrumbProvider {
    fn provide_breadcrumbs(&self, uri: &str, line: u32, col: u32) -> BreadcrumbPath;
}

// ---------------------------------------------------------------------------
// Renderable breadcrumb bar
// ---------------------------------------------------------------------------

/// A single segment in the breadcrumb navigation bar.
#[derive(Debug, Clone)]
pub struct BreadcrumbItem {
    pub label: String,
    pub icon: Option<char>,
    pub path: PathBuf,
    pub is_active: bool,
}

impl BreadcrumbItem {
    pub fn new(label: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            path: path.into(),
            is_active: false,
        }
    }

    pub fn with_icon(mut self, icon: char) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// Breadcrumb navigation bar with rendering support.
#[derive(Debug, Clone)]
pub struct BreadcrumbBar {
    pub items: Vec<BreadcrumbItem>,
    pub selected_index: usize,
    pub is_focused: bool,
}

impl BreadcrumbBar {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected_index: 0,
            is_focused: false,
        }
    }

    /// Replace all breadcrumb items.
    pub fn set_items(&mut self, items: Vec<BreadcrumbItem>) {
        self.items = items;
        if !self.items.is_empty() {
            self.selected_index = self.selected_index.min(self.items.len() - 1);
            if let Some(last) = self.items.last_mut() {
                last.is_active = true;
            }
        } else {
            self.selected_index = 0;
        }
    }

    /// Move selection to the next breadcrumb (right).
    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.items.len() - 1);
        }
    }

    /// Move selection to the previous breadcrumb (left).
    pub fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// Activate the currently selected item, returning its path.
    pub fn activate(&mut self) -> Option<PathBuf> {
        if self.items.is_empty() {
            return None;
        }
        for item in &mut self.items {
            item.is_active = false;
        }
        self.items[self.selected_index].is_active = true;
        Some(self.items[self.selected_index].path.clone())
    }

    /// Render the breadcrumb bar with `›` separators.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width < 3 || self.items.is_empty() {
            return;
        }

        let mut spans = Vec::new();
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    " › ",
                    Style::default().fg(Color::DarkGray),
                ));
            }

            let label = if let Some(icon) = item.icon {
                format!("{} {}", icon, item.label)
            } else {
                item.label.clone()
            };

            let style = if self.is_focused && i == self.selected_index {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::DarkGray)
            } else if item.is_active {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            spans.push(Span::styled(label, style));
        }

        let line = Line::from(spans);
        let render_area = Rect {
            height: 1,
            ..area
        };
        line.render(render_area, buf);
    }
}

impl Default for BreadcrumbBar {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Display implementations
// ---------------------------------------------------------------------------

impl std::fmt::Display for BreadcrumbKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BreadcrumbKind::File => "file",
            BreadcrumbKind::Folder => "folder",
            BreadcrumbKind::Symbol => "symbol",
            BreadcrumbKind::Class => "class",
            BreadcrumbKind::Function => "function",
            BreadcrumbKind::Method => "method",
            BreadcrumbKind::Property => "property",
            BreadcrumbKind::Enum => "enum",
            BreadcrumbKind::Interface => "interface",
            BreadcrumbKind::Module => "module",
        };
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for BreadcrumbElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.kind, self.label)
    }
}

// ---------------------------------------------------------------------------
// PartialEq for BreadcrumbBar
// ---------------------------------------------------------------------------

impl PartialEq for BreadcrumbBar {
    fn eq(&self, other: &Self) -> bool {
        self.items.len() == other.items.len()
            && self.selected_index == other.selected_index
            && self.is_focused == other.is_focused
    }
}

// ---------------------------------------------------------------------------
// BreadcrumbFilter
// ---------------------------------------------------------------------------

/// Filter breadcrumb elements by their kind.
#[derive(Debug, Clone)]
pub struct BreadcrumbFilter {
    pub allowed_kinds: Vec<BreadcrumbKind>,
}

impl BreadcrumbFilter {
    pub fn new() -> Self {
        Self {
            allowed_kinds: Vec::new(),
        }
    }

    pub fn allow(&mut self, kind: BreadcrumbKind) -> &mut Self {
        if !self.allowed_kinds.contains(&kind) {
            self.allowed_kinds.push(kind);
        }
        self
    }

    pub fn is_allowed(&self, kind: &BreadcrumbKind) -> bool {
        self.allowed_kinds.contains(kind)
    }

    pub fn filter_path(&self, path: &BreadcrumbPath) -> BreadcrumbPath {
        let elements = path
            .elements
            .iter()
            .filter(|e| self.is_allowed(&e.kind))
            .cloned()
            .collect();
        BreadcrumbPath { elements }
    }

    pub fn allowed_count(&self) -> usize {
        self.allowed_kinds.len()
    }
}

impl Default for BreadcrumbFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Additional BreadcrumbPath methods
// ---------------------------------------------------------------------------

impl BreadcrumbPath {
    /// Returns `true` if any element has the given kind.
    pub fn contains_kind(&self, kind: &BreadcrumbKind) -> bool {
        self.elements.iter().any(|e| &e.kind == kind)
    }

    /// Collect references to elements matching the given kind.
    pub fn elements_of_kind(&self, kind: &BreadcrumbKind) -> Vec<&BreadcrumbElement> {
        self.elements.iter().filter(|e| &e.kind == kind).collect()
    }

    /// Alias for `len()` — returns the depth of the path.
    pub fn depth(&self) -> usize {
        self.len()
    }

    /// Reverse the order of elements in-place.
    pub fn reverse(&mut self) {
        self.elements.reverse();
    }

    /// Truncate the path to at most `max_depth` elements.
    pub fn truncate(&mut self, max_depth: usize) {
        self.elements.truncate(max_depth);
    }
}

// ---------------------------------------------------------------------------
// Additional BreadcrumbBar methods
// ---------------------------------------------------------------------------

impl BreadcrumbBar {
    /// Return a reference to the currently selected item, if any.
    pub fn selected_item(&self) -> Option<&BreadcrumbItem> {
        self.items.get(self.selected_index)
    }

    /// Remove all items and reset selection.
    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_index = 0;
    }

    /// Number of items in the bar.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Select the first item.
    pub fn select_first(&mut self) {
        self.selected_index = 0;
    }

    /// Select the last item.
    pub fn select_last(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = self.items.len() - 1;
        }
    }
}

/// Accumulated statistics for breadcrumb operations.
#[derive(Debug, Clone, PartialEq)]
pub struct BreadcrumbStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl BreadcrumbStats {
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
    pub fn merge(&mut self, other: &BreadcrumbStats) {
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

impl Default for BreadcrumbStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BreadcrumbStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BreadcrumbStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for breadcrumb.
#[derive(Debug, Clone)]
pub struct BreadcrumbValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl BreadcrumbValidator {
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

impl Default for BreadcrumbValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// BreadcrumbDropdown — sibling navigation at each level
// ---------------------------------------------------------------------------

/// A dropdown showing sibling items at a particular breadcrumb level.
#[derive(Debug, Clone)]
pub struct BreadcrumbDropdown {
    /// The breadcrumb index this dropdown is anchored to.
    pub anchor_index: usize,
    /// Sibling items at this level.
    pub siblings: Vec<BreadcrumbElement>,
    /// Currently highlighted sibling.
    pub selected_index: usize,
    /// Whether the dropdown is visible.
    pub visible: bool,
}

impl BreadcrumbDropdown {
    pub fn new(anchor_index: usize, siblings: Vec<BreadcrumbElement>) -> Self {
        Self {
            anchor_index,
            siblings,
            selected_index: 0,
            visible: true,
        }
    }

    pub fn selected(&self) -> Option<&BreadcrumbElement> {
        self.siblings.get(self.selected_index)
    }

    pub fn select_next(&mut self) {
        if !self.siblings.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.siblings.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.siblings.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.siblings.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    pub fn accept(&self) -> Option<&BreadcrumbElement> {
        self.selected()
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn sibling_count(&self) -> usize {
        self.siblings.len()
    }
}

/// Update a `BreadcrumbPath` when the cursor moves to a new symbol.
///
/// `symbol_path` is the symbol hierarchy at the cursor position
/// (e.g. `["src", "main.rs", "MyStruct", "my_method"]`).
pub fn update_breadcrumbs_for_cursor(
    current: &mut BreadcrumbPath,
    symbol_path: &[(String, BreadcrumbKind)],
) {
    current.elements.clear();
    for (label, kind) in symbol_path {
        current.push(BreadcrumbElement {
            label: label.clone(),
            kind: kind.clone(),
            uri: None,
            range_start_line: None,
        });
    }
}

// ---------------------------------------------------------------------------
// Additional BreadcrumbPath methods
// ---------------------------------------------------------------------------

impl BreadcrumbPath {
    /// Returns an iterator over the elements of this path.
    pub fn iter(&self) -> std::slice::Iter<'_, BreadcrumbElement> {
        self.elements.iter()
    }

    /// Find the first element whose label matches the given string.
    pub fn find_by_label(&self, label: &str) -> Option<&BreadcrumbElement> {
        self.elements.iter().find(|e| e.label == label)
    }

    /// Collect all element labels into a `Vec`.
    pub fn labels(&self) -> Vec<&str> {
        self.elements.iter().map(|e| e.label.as_str()).collect()
    }
}

impl fmt::Display for BreadcrumbPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self
            .elements
            .iter()
            .map(|e| e.label.as_str())
            .collect::<Vec<_>>()
            .join(" > ");
        write!(f, "{}", path)
    }
}

// ---------------------------------------------------------------------------
// Additional BreadcrumbElement methods
// ---------------------------------------------------------------------------

impl BreadcrumbElement {
    /// Builder method to set the URI, consuming and returning self.
    pub fn with_uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    /// Builder method to set the range start line.
    pub fn with_range_start_line(mut self, line: u32) -> Self {
        self.range_start_line = Some(line);
        self
    }
}

// ---------------------------------------------------------------------------
// Additional BreadcrumbKind methods
// ---------------------------------------------------------------------------

impl BreadcrumbKind {
    /// Returns `true` for symbol-like kinds: Symbol, Function, Method,
    /// Property, Class, Enum, and Interface.
    pub fn is_symbol(&self) -> bool {
        matches!(
            self,
            BreadcrumbKind::Symbol
                | BreadcrumbKind::Function
                | BreadcrumbKind::Method
                | BreadcrumbKind::Property
                | BreadcrumbKind::Class
                | BreadcrumbKind::Enum
                | BreadcrumbKind::Interface
        )
    }
}

// ---------------------------------------------------------------------------
// Additional BreadcrumbBar methods
// ---------------------------------------------------------------------------

impl BreadcrumbBar {
    /// Collect the labels of all items in the bar.
    pub fn labels(&self) -> Vec<&str> {
        self.items.iter().map(|i| i.label.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// Additional BreadcrumbItem methods
// ---------------------------------------------------------------------------

impl BreadcrumbItem {
    /// Extract the file name component from the item's path, if present.
    pub fn file_name(&self) -> Option<&str> {
        self.path.file_name().and_then(|n| n.to_str())
    }
}

// ---------------------------------------------------------------------------
// Breadcrumb truncation
// ---------------------------------------------------------------------------

/// Truncate a breadcrumb path for display when it exceeds `max_segments`.
/// Keeps the first and last segments, replacing the middle with "…".
pub fn truncate_breadcrumb_path(path: &BreadcrumbPath, max_segments: usize) -> String {
    if max_segments < 2 || path.len() <= max_segments {
        return path.to_path_string();
    }
    let keep_start = 1;
    let keep_end = max_segments - 1;
    let labels: Vec<&str> = path.elements.iter().map(|e| e.label.as_str()).collect();
    let mut parts: Vec<&str> = Vec::with_capacity(max_segments + 1);
    for l in &labels[..keep_start] {
        parts.push(l);
    }
    parts.push("…");
    let end_start = labels.len().saturating_sub(keep_end);
    for l in &labels[end_start..] {
        parts.push(l);
    }
    parts.join(" > ")
}

// ---------------------------------------------------------------------------
// Breadcrumb comparison
// ---------------------------------------------------------------------------

impl BreadcrumbPath {
    /// Returns `true` if this path starts with the same elements as `prefix`.
    pub fn starts_with(&self, prefix: &BreadcrumbPath) -> bool {
        if prefix.len() > self.len() {
            return false;
        }
        self.elements[..prefix.len()]
            .iter()
            .zip(prefix.elements.iter())
            .all(|(a, b)| a.label == b.label && a.kind == b.kind)
    }

    /// Returns the common prefix of two paths.
    pub fn common_prefix(&self, other: &BreadcrumbPath) -> BreadcrumbPath {
        let mut result = BreadcrumbPath::new();
        for (a, b) in self.elements.iter().zip(other.elements.iter()) {
            if a.label == b.label && a.kind == b.kind {
                result.push(a.clone());
            } else {
                break;
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Breadcrumb serialization
// ---------------------------------------------------------------------------

impl BreadcrumbPath {
    /// Serialize to a simple colon-separated format: `kind:label;kind:label;...`
    pub fn serialize(&self) -> String {
        self.elements
            .iter()
            .map(|e| format!("{}:{}", kind_to_tag(&e.kind), e.label))
            .collect::<Vec<_>>()
            .join(";")
    }

    /// Deserialize from the format produced by [`serialize`](Self::serialize).
    pub fn deserialize(s: &str) -> Option<Self> {
        if s.is_empty() {
            return Some(BreadcrumbPath::new());
        }
        let mut path = BreadcrumbPath::new();
        for segment in s.split(';') {
            let (tag, label) = segment.split_once(':')?;
            let kind = tag_to_kind(tag)?;
            path.push(BreadcrumbElement {
                label: label.to_string(),
                kind,
                uri: None,
                range_start_line: None,
            });
        }
        Some(path)
    }
}

fn kind_to_tag(kind: &BreadcrumbKind) -> &'static str {
    match kind {
        BreadcrumbKind::File => "file",
        BreadcrumbKind::Folder => "folder",
        BreadcrumbKind::Symbol => "sym",
        BreadcrumbKind::Class => "class",
        BreadcrumbKind::Function => "fn",
        BreadcrumbKind::Method => "method",
        BreadcrumbKind::Property => "prop",
        BreadcrumbKind::Enum => "enum",
        BreadcrumbKind::Interface => "iface",
        BreadcrumbKind::Module => "mod",
    }
}

fn tag_to_kind(tag: &str) -> Option<BreadcrumbKind> {
    match tag {
        "file" => Some(BreadcrumbKind::File),
        "folder" => Some(BreadcrumbKind::Folder),
        "sym" => Some(BreadcrumbKind::Symbol),
        "class" => Some(BreadcrumbKind::Class),
        "fn" => Some(BreadcrumbKind::Function),
        "method" => Some(BreadcrumbKind::Method),
        "prop" => Some(BreadcrumbKind::Property),
        "enum" => Some(BreadcrumbKind::Enum),
        "iface" => Some(BreadcrumbKind::Interface),
        "mod" => Some(BreadcrumbKind::Module),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Breadcrumb navigation history (back/forward)
// ---------------------------------------------------------------------------

/// Navigation history supporting back/forward movement through breadcrumb paths.
#[derive(Debug, Clone)]
pub struct BreadcrumbHistory {
    entries: Vec<BreadcrumbPath>,
    cursor: usize,
    max_entries: usize,
}

impl BreadcrumbHistory {
    /// Create a new history with the given maximum capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            max_entries: max_entries.max(1),
        }
    }

    /// Push a new path, discarding any forward history.
    pub fn push(&mut self, path: BreadcrumbPath) {
        if self.cursor < self.entries.len() {
            self.entries.truncate(self.cursor);
        }
        self.entries.push(path);
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
        self.cursor = self.entries.len();
    }

    /// Navigate back, returning the previous path if available.
    pub fn back(&mut self) -> Option<&BreadcrumbPath> {
        if self.cursor > 1 {
            self.cursor -= 1;
            self.entries.get(self.cursor - 1)
        } else {
            None
        }
    }

    /// Navigate forward, returning the next path if available.
    pub fn forward(&mut self) -> Option<&BreadcrumbPath> {
        if self.cursor < self.entries.len() {
            self.cursor += 1;
            self.entries.get(self.cursor - 1)
        } else {
            None
        }
    }

    /// Return the current path without moving.
    pub fn current(&self) -> Option<&BreadcrumbPath> {
        if self.cursor == 0 {
            None
        } else {
            self.entries.get(self.cursor - 1)
        }
    }

    /// Whether back navigation is possible.
    pub fn can_go_back(&self) -> bool {
        self.cursor > 1
    }

    /// Whether forward navigation is possible.
    pub fn can_go_forward(&self) -> bool {
        self.cursor < self.entries.len()
    }

    /// Number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = 0;
    }
}

// ---------------------------------------------------------------------------
// Breadcrumb path collapsing
// ---------------------------------------------------------------------------

impl BreadcrumbPath {
    /// Collapse the path to show only "important" segments: the first folder,
    /// the file, and any symbol-like elements. Intermediate folders are replaced
    /// with a single `…` placeholder element.
    pub fn collapsed(&self) -> BreadcrumbPath {
        if self.len() <= 3 {
            return self.clone();
        }
        let mut result = BreadcrumbPath::new();
        let mut skipped_folders = false;
        for (i, elem) in self.elements.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == self.len() - 1;
            let is_important = elem.kind.is_symbol()
                || elem.kind == BreadcrumbKind::File
                || is_first
                || is_last;
            if is_important {
                if skipped_folders {
                    result.push(BreadcrumbElement {
                        label: "…".to_string(),
                        kind: BreadcrumbKind::Folder,
                        uri: None,
                        range_start_line: None,
                    });
                    skipped_folders = false;
                }
                result.push(elem.clone());
            } else {
                skipped_folders = true;
            }
        }
        result
    }

    /// Find the divergence index between two paths — the first index where
    /// the elements differ, or the length of the shorter path.
    pub fn divergence_index(&self, other: &BreadcrumbPath) -> usize {
        let mut idx = 0;
        for (a, b) in self.elements.iter().zip(other.elements.iter()) {
            if a.label != b.label || a.kind != b.kind {
                break;
            }
            idx += 1;
        }
        idx
    }

    /// Search within the path for elements whose label contains the query
    /// (case-insensitive), returning their indices.
    pub fn search(&self, query: &str) -> Vec<usize> {
        let query_lower = query.to_lowercase();
        self.elements
            .iter()
            .enumerate()
            .filter(|(_, e)| e.label.to_lowercase().contains(&query_lower))
            .map(|(i, _)| i)
            .collect()
    }

    /// Return a new path containing only the first `max_depth` elements.
    pub fn depth_limited(&self, max_depth: usize) -> BreadcrumbPath {
        BreadcrumbPath {
            elements: self.elements.iter().take(max_depth).cloned().collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Symbol breadcrumb generation from an outline
// ---------------------------------------------------------------------------

/// A symbol outline entry used to generate breadcrumbs.
#[derive(Debug, Clone)]
pub struct OutlineEntry {
    pub name: String,
    pub kind: BreadcrumbKind,
    pub start_line: u32,
    pub end_line: u32,
    pub children: Vec<OutlineEntry>,
}

/// Build a `BreadcrumbPath` from a document outline for a given cursor line.
/// Walks the outline tree and collects the chain of symbols that contain the
/// cursor position.
pub fn breadcrumbs_from_outline(outline: &[OutlineEntry], cursor_line: u32) -> BreadcrumbPath {
    let mut path = BreadcrumbPath::new();
    collect_outline_chain(outline, cursor_line, &mut path);
    path
}

fn collect_outline_chain(entries: &[OutlineEntry], line: u32, path: &mut BreadcrumbPath) {
    for entry in entries {
        if line >= entry.start_line && line <= entry.end_line {
            path.push(BreadcrumbElement {
                label: entry.name.clone(),
                kind: entry.kind.clone(),
                uri: None,
                range_start_line: Some(entry.start_line),
            });
            collect_outline_chain(&entry.children, line, path);
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// BreadcrumbPicker – dropdown selection at a breadcrumb segment
// ---------------------------------------------------------------------------

/// An item in the breadcrumb picker dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbPickerItem {
    pub label: String,
    pub kind: BreadcrumbKind,
    pub is_selected: bool,
}

/// Dropdown picker for selecting among siblings at a breadcrumb segment.
#[derive(Debug, Clone)]
pub struct BreadcrumbPicker {
    items: Vec<BreadcrumbPickerItem>,
    active_index: Option<usize>,
    filter_text: String,
}

impl BreadcrumbPicker {
    pub fn new(items: Vec<BreadcrumbPickerItem>) -> Self {
        Self {
            items,
            active_index: None,
            filter_text: String::new(),
        }
    }

    /// Set the filter text and return filtered indices.
    pub fn set_filter(&mut self, text: &str) -> usize {
        self.filter_text = text.to_lowercase();
        self.active_index = None;
        self.filtered_items().len()
    }

    /// Get items matching the current filter.
    pub fn filtered_items(&self) -> Vec<&BreadcrumbPickerItem> {
        if self.filter_text.is_empty() {
            self.items.iter().collect()
        } else {
            self.items.iter()
                .filter(|item| item.label.to_lowercase().contains(&self.filter_text))
                .collect()
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        let count = self.filtered_items().len();
        if count == 0 { return; }
        self.active_index = Some(match self.active_index {
            Some(i) => (i + 1) % count,
            None => 0,
        });
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        let count = self.filtered_items().len();
        if count == 0 { return; }
        self.active_index = Some(match self.active_index {
            Some(0) | None => count.saturating_sub(1),
            Some(i) => i - 1,
        });
    }

    /// Get the currently selected item.
    pub fn selected(&self) -> Option<&BreadcrumbPickerItem> {
        let filtered = self.filtered_items();
        self.active_index.and_then(|i| filtered.get(i).copied())
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl fmt::Display for BreadcrumbPicker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BreadcrumbPicker({} items)", self.items.len())
    }
}

// ---------------------------------------------------------------------------
// BreadcrumbSymbolResolver – resolves symbols for breadcrumb display
// ---------------------------------------------------------------------------

/// Resolves document symbols into breadcrumb elements.
#[derive(Debug, Clone)]
pub struct BreadcrumbSymbolResolver {
    /// Map from kind to icon character.
    icons: Vec<(BreadcrumbKind, char)>,
}

impl BreadcrumbSymbolResolver {
    pub fn new() -> Self {
        Self {
            icons: vec![
                (BreadcrumbKind::Function, 'ƒ'),
                (BreadcrumbKind::Class, '◆'),
                (BreadcrumbKind::Method, '▸'),
                (BreadcrumbKind::Module, '◇'),
                (BreadcrumbKind::Enum, '◈'),
                (BreadcrumbKind::Interface, '○'),
                (BreadcrumbKind::Property, '◻'),
            ],
        }
    }

    /// Get the icon for a given kind.
    pub fn icon_for(&self, kind: &BreadcrumbKind) -> char {
        self.icons.iter()
            .find(|(k, _)| k == kind)
            .map(|(_, icon)| *icon)
            .unwrap_or('·')
    }

    /// Resolve a symbol name and kind into a breadcrumb element.
    pub fn resolve(&self, name: &str, kind: BreadcrumbKind, line: Option<u32>) -> BreadcrumbElement {
        BreadcrumbElement {
            label: name.to_string(),
            kind,
            uri: None,
            range_start_line: line,
        }
    }

    /// Format a breadcrumb element with its icon.
    pub fn format_with_icon(&self, element: &BreadcrumbElement) -> String {
        let icon = self.icon_for(&element.kind);
        format!("{icon} {}", element.label)
    }
}

impl Default for BreadcrumbSymbolResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BreadcrumbSymbolResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BreadcrumbSymbolResolver({} icons)", self.icons.len())
    }
}

// ---------------------------------------------------------------------------
// Breadcrumb path truncation with ellipsis
// ---------------------------------------------------------------------------

/// Truncate a breadcrumb path to fit within `max_segments`, adding ellipsis element.
pub fn truncate_breadcrumb_elements(path: &BreadcrumbPath, max_segments: usize) -> BreadcrumbPath {
    if path.elements.len() <= max_segments || max_segments < 2 {
        return path.clone();
    }
    let mut result = BreadcrumbPath::new();
    // Keep first element
    result.push(path.elements[0].clone());
    // Add ellipsis element
    result.push(BreadcrumbElement {
        label: "…".to_string(),
        kind: BreadcrumbKind::File,
        uri: None,
        range_start_line: None,
    });
    // Keep last (max_segments - 2) elements
    let keep_from = path.elements.len() - (max_segments - 2);
    for elem in &path.elements[keep_from..] {
        result.push(elem.clone());
    }
    result
}

// ---------------------------------------------------------------------------
// Breadcrumb focus/keyboard navigation
// ---------------------------------------------------------------------------

/// Manages focus state for breadcrumb keyboard navigation.
#[derive(Debug, Clone)]
pub struct BreadcrumbFocusNavigator {
    segment_count: usize,
    focused_index: Option<usize>,
    is_picker_open: bool,
}

impl BreadcrumbFocusNavigator {
    pub fn new(segment_count: usize) -> Self {
        Self {
            segment_count,
            focused_index: None,
            is_picker_open: false,
        }
    }

    /// Focus the next segment (right arrow).
    pub fn focus_next(&mut self) {
        if self.segment_count == 0 { return; }
        self.focused_index = Some(match self.focused_index {
            Some(i) if i + 1 < self.segment_count => i + 1,
            _ => 0,
        });
    }

    /// Focus the previous segment (left arrow).
    pub fn focus_previous(&mut self) {
        if self.segment_count == 0 { return; }
        self.focused_index = Some(match self.focused_index {
            Some(0) | None => self.segment_count.saturating_sub(1),
            Some(i) => i - 1,
        });
    }

    /// Toggle the picker for the focused segment.
    pub fn toggle_picker(&mut self) {
        self.is_picker_open = !self.is_picker_open;
    }

    pub fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    pub fn is_picker_open(&self) -> bool {
        self.is_picker_open
    }

    /// Blur (unfocus) the breadcrumb bar.
    pub fn blur(&mut self) {
        self.focused_index = None;
        self.is_picker_open = false;
    }

    /// Whether any segment is focused.
    pub fn is_focused(&self) -> bool {
        self.focused_index.is_some()
    }

    /// Update the segment count (e.g., when path changes).
    pub fn update_count(&mut self, count: usize) {
        self.segment_count = count;
        if let Some(idx) = self.focused_index {
            if idx >= count {
                self.focused_index = if count > 0 { Some(count - 1) } else { None };
            }
        }
    }
}

impl fmt::Display for BreadcrumbFocusNavigator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BreadcrumbFocus(segments={}, focused={:?})",
            self.segment_count, self.focused_index
        )
    }
}

// ---------------------------------------------------------------------------
// BreadcrumbRenderer - breadcrumb dropdown renderer
// ---------------------------------------------------------------------------

/// Severity level for breadcrumb dropdown renderer issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BreadcrumbRendererSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for BreadcrumbRendererSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [BreadcrumbRenderer].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbRendererEntry {
    pub id: String,
    pub label: String,
    pub severity: BreadcrumbRendererSeverity,
    pub detail: Option<String>,
    pub item_count: usize,
    enabled: bool,
}

impl BreadcrumbRendererEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: BreadcrumbRendererSeverity::Low,
            detail: None,
            item_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: BreadcrumbRendererSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_item_count(mut self, val: usize) -> Self {
        self.item_count = val;
        self
    }

    pub fn is_expanded(&self) -> bool {
        self.enabled && self.severity >= BreadcrumbRendererSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.item_count, det)
    }
}

impl fmt::Display for BreadcrumbRendererEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [BreadcrumbRendererEntry] items.
#[derive(Debug, Clone)]
pub struct BreadcrumbRenderer {
    entries: Vec<BreadcrumbRendererEntry>,
    name: String,
    capacity: usize,
}

impl BreadcrumbRenderer {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: BreadcrumbRendererEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<BreadcrumbRendererEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&BreadcrumbRendererEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn item_count(&self) -> usize { self.entries.len() }

    pub fn is_expanded(&self) -> bool {
        self.entries.iter().any(|e| e.is_expanded())
    }

    pub fn entries_by_severity(&self, severity: BreadcrumbRendererSeverity) -> Vec<&BreadcrumbRendererEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= BreadcrumbRendererSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&BreadcrumbRendererEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&BreadcrumbRendererEntry> {
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
// PathShortener - breadcrumb path shortener
// ---------------------------------------------------------------------------

/// Configuration for [PathShortener].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathShortenerConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub max_depth: usize,
}

impl PathShortenerConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, max_depth: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_max_depth(mut self, val: usize) -> Self { self.max_depth = val; self }
}

impl Default for PathShortenerConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [PathShortener].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathShortenerItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl PathShortenerItem {
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

    pub fn needs_shortening(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for PathShortenerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [PathShortenerItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct PathShortener {
    config: PathShortenerConfig,
    items: Vec<PathShortenerItem>,
}

impl PathShortener {
    pub fn new(config: PathShortenerConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: PathShortenerItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<PathShortenerItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&PathShortenerItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn max_depth(&self) -> usize { self.items.len() }

    pub fn needs_shortening(&self) -> bool {
        self.items.iter().any(|i| i.needs_shortening())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&PathShortenerItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&PathShortenerItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &PathShortenerConfig {
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
// breadcrumb – Editor text helpers
// ---------------------------------------------------------------------------

/// A half-open range within a document `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XBreadcrumbTextSpan {
    pub start: usize,
    pub end: usize,
}

impl XBreadcrumbTextSpan {
    pub fn new(start: usize, end: usize) -> Self {
        let (s, e) = if start <= end { (start, end) } else { (end, start) };
        Self { start: s, end: e }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Extract the spanned slice from `text`.
    pub fn extract<'a>(&self, text: &'a str) -> &'a str {
        &text[self.start..self.end]
    }

    /// Returns true if `pos` is contained within this span.
    pub fn contains(&self, pos: usize) -> bool {
        pos >= self.start && pos < self.end
    }

    /// Returns the overlap with `other`, if any.
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let s = self.start.max(other.start);
        let e = self.end.min(other.end);
        if s < e { Some(Self { start: s, end: e }) } else { None }
    }

    /// Merge two spans into the smallest enclosing span.
    pub fn union(&self, other: &Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Shift the span by `delta` positions to the right.
    pub fn shift(&self, delta: usize) -> Self {
        Self { start: self.start + delta, end: self.end + delta }
    }
}

/// Count the number of lines in `text`.
pub fn x_breadcrumb_count_lines(text: &str) -> usize {
    if text.is_empty() { return 0; }
    text.lines().count()
}

/// Return the byte offset of the start of line `n` (0-based).
pub fn x_breadcrumb_line_start_offset(text: &str, line: usize) -> Option<usize> {
    let mut current = 0usize;
    for (i, l) in text.split('\n').enumerate() {
        if i == line { return Some(current); }
        current += l.len() + 1;
    }
    None
}

/// Compute the indentation level (number of leading spaces) of a line.
pub fn x_breadcrumb_indent_level(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

/// Trim trailing whitespace from every line in `text`.
pub fn x_breadcrumb_trim_trailing(text: &str) -> String {
    text.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Detect the dominant line ending in `text` (`"\n"` or `"\r\n"`).
pub fn x_breadcrumb_detect_eol(text: &str) -> &'static str {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count().saturating_sub(crlf);
    if crlf > lf { "\r\n" } else { "\n" }
}

/// Simple word-boundary based tokenizer: split on whitespace and punctuation.
pub fn x_breadcrumb_tokenize(text: &str) -> Vec<&str> {
    text.split(|c: char| c.is_whitespace() || ".,;:!?()[]{}".contains(c))
        .filter(|s| !s.is_empty())
        .collect()
}


/// Configuration manager for breadcrumb functionality.
pub struct BreadcrumbConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl BreadcrumbConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &BreadcrumbConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for breadcrumb operations.
pub struct BreadcrumbRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl BreadcrumbRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for breadcrumb.
pub struct BreadcrumbValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl BreadcrumbValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &BreadcrumbValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
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
// xa_ extended helpers for breadcrumb
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaBreadcrumbRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaBreadcrumbRingBuf {
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
pub struct XaBreadcrumbCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaBreadcrumbCounter {
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

impl Default for XaBreadcrumbCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 8
// ---------------------------------------------------------------------------

/// Generic object pool `Xc8Pool<T>`.
pub struct Xc8Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc8Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc8PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc8Pool<T> {
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
    pub fn stats(&self) -> Xc8PoolStats {
        Xc8PoolStats {
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

impl<T> Default for Xc8Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc8Scheduler`.
pub struct Xc8Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc8Scheduler {
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

impl Default for Xc8Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_8 hash for the given byte slice.
pub fn xc_8_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_8 convention.
pub fn xc_8_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_94 deepening: state machine + event bus ---

/// States for the Xd94 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd94State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd94State {
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
pub struct Xd94Transition {
    pub from: Xd94State,
    pub to: Xd94State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd94StateMachine {
    current: Xd94State,
    history: Vec<Xd94Transition>,
    step_counter: usize,
}

impl Xd94StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd94State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd94State {
        self.current
    }

    pub fn history(&self) -> &[Xd94Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd94State) -> Result<Xd94State, String> {
        let allowed = match (self.current, target) {
            (Xd94State::Idle, Xd94State::Running) => true,
            (Xd94State::Running, Xd94State::Paused) => true,
            (Xd94State::Running, Xd94State::Done) => true,
            (Xd94State::Paused, Xd94State::Running) => true,
            (Xd94State::Paused, Xd94State::Done) => true,
            (Xd94State::Done, Xd94State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_94: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd94Transition {
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
            "Xd94SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd94State> {
        let prefix = "Xd94SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd94State::Idle),
            "Running" => Some(Xd94State::Running),
            "Paused" => Some(Xd94State::Paused),
            "Done" => Some(Xd94State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd94State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd94 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd94Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd94Event {
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

type Xd94HandlerFn = Box<dyn Fn(&Xd94Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd94EventBus {
    handlers: Vec<(usize, Option<String>, Xd94HandlerFn)>,
    next_id: usize,
    published: Vec<Xd94Event>,
}

impl Xd94EventBus {
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
        F: Fn(&Xd94Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd94Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd94Event) {
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

    pub fn published_events(&self) -> &[Xd94Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_element(label: &str, kind: BreadcrumbKind) -> BreadcrumbElement {
        BreadcrumbElement {
            label: label.to_string(),
            kind,
            uri: None,
            range_start_line: None,
        }
    }

    fn sample_items() -> Vec<BreadcrumbItem> {
        vec![
            BreadcrumbItem::new("src", "/project/src").with_icon('📁'),
            BreadcrumbItem::new("lib.rs", "/project/src/lib.rs").with_icon('📄'),
            BreadcrumbItem::new("MyStruct", "/project/src/lib.rs#MyStruct"),
        ]
    }

    #[test]
    fn push_pop_and_len() {
        let mut path = BreadcrumbPath::new();
        assert!(path.is_empty());

        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("main.rs", BreadcrumbKind::File));
        assert_eq!(path.len(), 2);

        let popped = path.pop().unwrap();
        assert_eq!(popped.label, "main.rs");
        assert_eq!(path.len(), 1);
    }

    #[test]
    fn to_path_string_formatting() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("project", BreadcrumbKind::Folder));
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("lib.rs", BreadcrumbKind::File));
        assert_eq!(path.to_path_string(), "project > src > lib.rs");
    }

    #[test]
    fn last_returns_tail_element() {
        let mut path = BreadcrumbPath::new();
        assert!(path.last().is_none());

        path.push(sample_element("main", BreadcrumbKind::Function));
        assert_eq!(path.last().unwrap().label, "main");
        assert_eq!(path.last().unwrap().kind, BreadcrumbKind::Function);
    }

    #[test]
    fn empty_path_string() {
        let path = BreadcrumbPath::new();
        assert_eq!(path.to_path_string(), "");
    }

    #[test]
    fn bar_creation() {
        let bar = BreadcrumbBar::new();
        assert!(bar.items.is_empty());
        assert!(!bar.is_focused);
    }

    #[test]
    fn set_items_marks_last_active() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        assert_eq!(bar.items.len(), 3);
        assert!(bar.items.last().unwrap().is_active);
        assert!(!bar.items[0].is_active);
    }

    #[test]
    fn select_next_clamps() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        bar.select_next();
        assert_eq!(bar.selected_index, 1);
        bar.select_next();
        assert_eq!(bar.selected_index, 2);
        bar.select_next();
        assert_eq!(bar.selected_index, 2);
    }

    #[test]
    fn select_previous_clamps() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        bar.select_previous();
        assert_eq!(bar.selected_index, 0);
    }

    #[test]
    fn activate_returns_path() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        bar.select_next();
        let path = bar.activate();
        assert_eq!(path, Some(PathBuf::from("/project/src/lib.rs")));
        assert!(bar.items[1].is_active);
        assert!(!bar.items[2].is_active);
    }

    #[test]
    fn activate_empty_bar() {
        let mut bar = BreadcrumbBar::new();
        assert!(bar.activate().is_none());
    }

    #[test]
    fn render_does_not_panic() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        bar.is_focused = true;
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);
    }

    #[test]
    fn render_empty_no_panic() {
        let bar = BreadcrumbBar::new();
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);
    }

    #[test]
    fn default_impl() {
        let bar = BreadcrumbBar::default();
        assert!(bar.items.is_empty());
    }

    #[test]
    fn breadcrumb_item_with_icon() {
        let item = BreadcrumbItem::new("test", "/test").with_icon('📁');
        assert_eq!(item.icon, Some('📁'));
    }

    #[test]
    fn test_breadcrumb_kind_display() {
        assert_eq!(format!("{}", BreadcrumbKind::File), "file");
        assert_eq!(format!("{}", BreadcrumbKind::Folder), "folder");
        assert_eq!(format!("{}", BreadcrumbKind::Symbol), "symbol");
        assert_eq!(format!("{}", BreadcrumbKind::Class), "class");
        assert_eq!(format!("{}", BreadcrumbKind::Function), "function");
        assert_eq!(format!("{}", BreadcrumbKind::Method), "method");
        assert_eq!(format!("{}", BreadcrumbKind::Property), "property");
        assert_eq!(format!("{}", BreadcrumbKind::Enum), "enum");
        assert_eq!(format!("{}", BreadcrumbKind::Interface), "interface");
        assert_eq!(format!("{}", BreadcrumbKind::Module), "module");
    }

    #[test]
    fn test_breadcrumb_element_display() {
        let elem = sample_element("main.rs", BreadcrumbKind::File);
        assert_eq!(format!("{}", elem), "[file] main.rs");

        let elem2 = sample_element("MyClass", BreadcrumbKind::Class);
        assert_eq!(format!("{}", elem2), "[class] MyClass");
    }

    #[test]
    fn test_breadcrumb_filter_allow_and_check() {
        let mut filter = BreadcrumbFilter::new();
        assert!(!filter.is_allowed(&BreadcrumbKind::File));
        assert_eq!(filter.allowed_count(), 0);

        filter.allow(BreadcrumbKind::File);
        filter.allow(BreadcrumbKind::Folder);
        assert!(filter.is_allowed(&BreadcrumbKind::File));
        assert!(filter.is_allowed(&BreadcrumbKind::Folder));
        assert!(!filter.is_allowed(&BreadcrumbKind::Symbol));
        assert_eq!(filter.allowed_count(), 2);

        // Duplicate allow should not increase count.
        filter.allow(BreadcrumbKind::File);
        assert_eq!(filter.allowed_count(), 2);
    }

    #[test]
    fn test_breadcrumb_filter_filter_path() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("lib.rs", BreadcrumbKind::File));
        path.push(sample_element("MyStruct", BreadcrumbKind::Class));

        let mut filter = BreadcrumbFilter::new();
        filter.allow(BreadcrumbKind::Folder).allow(BreadcrumbKind::File);

        let filtered = filter.filter_path(&path);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered.elements[0].label, "src");
        assert_eq!(filtered.elements[1].label, "lib.rs");
    }

    #[test]
    fn test_breadcrumb_path_contains_kind() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("main.rs", BreadcrumbKind::File));

        assert!(path.contains_kind(&BreadcrumbKind::Folder));
        assert!(path.contains_kind(&BreadcrumbKind::File));
        assert!(!path.contains_kind(&BreadcrumbKind::Class));
    }

    #[test]
    fn test_breadcrumb_path_elements_of_kind() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("lib", BreadcrumbKind::Folder));
        path.push(sample_element("main.rs", BreadcrumbKind::File));

        let folders = path.elements_of_kind(&BreadcrumbKind::Folder);
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].label, "src");
        assert_eq!(folders[1].label, "lib");

        let classes = path.elements_of_kind(&BreadcrumbKind::Class);
        assert!(classes.is_empty());
    }

    #[test]
    fn test_breadcrumb_path_depth() {
        let mut path = BreadcrumbPath::new();
        assert_eq!(path.depth(), 0);
        path.push(sample_element("a", BreadcrumbKind::Folder));
        path.push(sample_element("b", BreadcrumbKind::File));
        assert_eq!(path.depth(), 2);
        assert_eq!(path.depth(), path.len());
    }

    #[test]
    fn test_breadcrumb_path_reverse() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("a", BreadcrumbKind::Folder));
        path.push(sample_element("b", BreadcrumbKind::File));
        path.push(sample_element("c", BreadcrumbKind::Class));

        path.reverse();
        assert_eq!(path.elements[0].label, "c");
        assert_eq!(path.elements[1].label, "b");
        assert_eq!(path.elements[2].label, "a");
    }

    #[test]
    fn test_breadcrumb_path_truncate() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("a", BreadcrumbKind::Folder));
        path.push(sample_element("b", BreadcrumbKind::File));
        path.push(sample_element("c", BreadcrumbKind::Class));

        path.truncate(2);
        assert_eq!(path.len(), 2);
        assert_eq!(path.elements[0].label, "a");
        assert_eq!(path.elements[1].label, "b");
    }

    #[test]
    fn test_bar_selected_item() {
        let mut bar = BreadcrumbBar::new();
        assert!(bar.selected_item().is_none());

        bar.set_items(sample_items());
        assert_eq!(bar.selected_item().unwrap().label, "src");

        bar.select_next();
        assert_eq!(bar.selected_item().unwrap().label, "lib.rs");
    }

    #[test]
    fn test_bar_clear() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        assert_eq!(bar.item_count(), 3);

        bar.clear();
        assert_eq!(bar.item_count(), 0);
        assert_eq!(bar.selected_index, 0);
        assert!(bar.selected_item().is_none());
    }

    #[test]
    fn test_bar_select_first_last() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());

        bar.select_last();
        assert_eq!(bar.selected_index, 2);
        assert_eq!(bar.selected_item().unwrap().label, "MyStruct");

        bar.select_first();
        assert_eq!(bar.selected_index, 0);
        assert_eq!(bar.selected_item().unwrap().label, "src");
    }

    #[test]
    fn test_bar_partial_eq() {
        let mut bar1 = BreadcrumbBar::new();
        let mut bar2 = BreadcrumbBar::new();
        assert_eq!(bar1, bar2);

        bar1.set_items(sample_items());
        bar2.set_items(sample_items());
        assert_eq!(bar1, bar2);

        bar1.select_next();
        assert_ne!(bar1, bar2);

        bar2.select_next();
        assert_eq!(bar1, bar2);

        bar1.is_focused = true;
        assert_ne!(bar1, bar2);
    }

    #[test]
    fn breadcrumb_stats_new_defaults() {
        let stats = BreadcrumbStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn breadcrumb_stats_record_success() {
        let mut stats = BreadcrumbStats::new();
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
    fn breadcrumb_stats_record_failure() {
        let mut stats = BreadcrumbStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn breadcrumb_stats_reset() {
        let mut stats = BreadcrumbStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn breadcrumb_stats_merge() {
        let mut a = BreadcrumbStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = BreadcrumbStats::new();
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
    fn breadcrumb_stats_display() {
        let mut stats = BreadcrumbStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn breadcrumb_stats_default() {
        let stats = BreadcrumbStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn breadcrumb_validator_accepts_and_rejects() {
        let mut v = BreadcrumbValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn breadcrumb_validator_warnings() {
        let mut v = BreadcrumbValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn breadcrumb_validator_clear_and_merge() {
        let mut v = BreadcrumbValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = BreadcrumbValidationCollector::new();
        a.add_error("a_err");
        let mut b = BreadcrumbValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    // --- BreadcrumbDropdown tests ---

    #[test]
    fn dropdown_creation() {
        let siblings = vec![
            sample_element("foo", BreadcrumbKind::Function),
            sample_element("bar", BreadcrumbKind::Function),
            sample_element("baz", BreadcrumbKind::Function),
        ];
        let dd = BreadcrumbDropdown::new(2, siblings);
        assert!(dd.visible);
        assert_eq!(dd.sibling_count(), 3);
        assert_eq!(dd.anchor_index, 2);
        assert_eq!(dd.selected().unwrap().label, "foo");
    }

    #[test]
    fn dropdown_navigation() {
        let siblings = vec![
            sample_element("a", BreadcrumbKind::Function),
            sample_element("b", BreadcrumbKind::Function),
        ];
        let mut dd = BreadcrumbDropdown::new(0, siblings);
        dd.select_next();
        assert_eq!(dd.selected().unwrap().label, "b");
        dd.select_next(); // wraps
        assert_eq!(dd.selected().unwrap().label, "a");
        dd.select_previous(); // wraps back
        assert_eq!(dd.selected().unwrap().label, "b");
    }

    #[test]
    fn dropdown_accept() {
        let siblings = vec![sample_element("target", BreadcrumbKind::Method)];
        let dd = BreadcrumbDropdown::new(0, siblings);
        assert_eq!(dd.accept().unwrap().label, "target");
    }

    #[test]
    fn dropdown_close() {
        let mut dd = BreadcrumbDropdown::new(0, vec![]);
        assert!(dd.visible);
        dd.close();
        assert!(!dd.visible);
    }

    // --- update_breadcrumbs_for_cursor tests ---

    #[test]
    fn update_breadcrumbs_for_cursor_replaces_path() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("old", BreadcrumbKind::File));

        update_breadcrumbs_for_cursor(&mut path, &[
            ("src".to_string(), BreadcrumbKind::Folder),
            ("main.rs".to_string(), BreadcrumbKind::File),
            ("MyStruct".to_string(), BreadcrumbKind::Class),
        ]);

        assert_eq!(path.len(), 3);
        assert_eq!(path.elements[0].label, "src");
        assert_eq!(path.elements[2].label, "MyStruct");
    }

    #[test]
    fn update_breadcrumbs_for_cursor_empty() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("old", BreadcrumbKind::File));
        update_breadcrumbs_for_cursor(&mut path, &[]);
        assert!(path.is_empty());
    }

    #[test]
    fn breadcrumb_path_iter() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("a", BreadcrumbKind::Folder));
        path.push(sample_element("b", BreadcrumbKind::File));
        let labels: Vec<&str> = path.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(labels, vec!["a", "b"]);
    }

    #[test]
    fn breadcrumb_path_find_by_label() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("main.rs", BreadcrumbKind::File));
        assert!(path.find_by_label("main.rs").is_some());
        assert_eq!(path.find_by_label("main.rs").unwrap().kind, BreadcrumbKind::File);
        assert!(path.find_by_label("missing").is_none());
    }

    #[test]
    fn breadcrumb_path_labels() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("x", BreadcrumbKind::Folder));
        path.push(sample_element("y", BreadcrumbKind::File));
        path.push(sample_element("z", BreadcrumbKind::Function));
        assert_eq!(path.labels(), vec!["x", "y", "z"]);
    }

    #[test]
    fn breadcrumb_path_display() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("a", BreadcrumbKind::Folder));
        path.push(sample_element("b", BreadcrumbKind::File));
        path.push(sample_element("c", BreadcrumbKind::Function));
        assert_eq!(format!("{path}"), "a > b > c");
    }

    #[test]
    fn breadcrumb_kind_is_symbol() {
        assert!(BreadcrumbKind::Function.is_symbol());
        assert!(BreadcrumbKind::Method.is_symbol());
        assert!(BreadcrumbKind::Class.is_symbol());
        assert!(BreadcrumbKind::Symbol.is_symbol());
        assert!(!BreadcrumbKind::File.is_symbol());
        assert!(!BreadcrumbKind::Folder.is_symbol());
        assert!(!BreadcrumbKind::Module.is_symbol());
    }

    #[test]
    fn breadcrumb_bar_labels() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        assert_eq!(bar.labels(), vec!["src", "lib.rs", "MyStruct"]);
    }

    #[test]
    fn breadcrumb_item_file_name() {
        let item = BreadcrumbItem::new("lib.rs", "/project/src/lib.rs");
        assert_eq!(item.file_name(), Some("lib.rs"));
    }

    #[test]
    fn breadcrumb_element_with_uri() {
        let elem = sample_element("func", BreadcrumbKind::Function)
            .with_uri("file:///a.rs");
        assert_eq!(elem.uri.as_deref(), Some("file:///a.rs"));
    }

    #[test]
    fn truncate_breadcrumb_short_path_unchanged() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("main.rs", BreadcrumbKind::File));
        assert_eq!(truncate_breadcrumb_path(&path, 5), "src > main.rs");
    }

    #[test]
    fn truncate_breadcrumb_long_path() {
        let mut path = BreadcrumbPath::new();
        for name in &["a", "b", "c", "d", "e", "f"] {
            path.push(sample_element(name, BreadcrumbKind::Folder));
        }
        let truncated = truncate_breadcrumb_path(&path, 3);
        assert!(truncated.contains("…"));
        assert!(truncated.starts_with("a"));
        assert!(truncated.ends_with("f"));
    }

    #[test]
    fn breadcrumb_starts_with_prefix() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("lib.rs", BreadcrumbKind::File));
        path.push(sample_element("MyFn", BreadcrumbKind::Function));

        let mut prefix = BreadcrumbPath::new();
        prefix.push(sample_element("src", BreadcrumbKind::Folder));
        prefix.push(sample_element("lib.rs", BreadcrumbKind::File));

        assert!(path.starts_with(&prefix));
        assert!(!prefix.starts_with(&path));
    }

    #[test]
    fn breadcrumb_common_prefix() {
        let mut a = BreadcrumbPath::new();
        a.push(sample_element("src", BreadcrumbKind::Folder));
        a.push(sample_element("lib.rs", BreadcrumbKind::File));
        a.push(sample_element("FnA", BreadcrumbKind::Function));

        let mut b = BreadcrumbPath::new();
        b.push(sample_element("src", BreadcrumbKind::Folder));
        b.push(sample_element("lib.rs", BreadcrumbKind::File));
        b.push(sample_element("FnB", BreadcrumbKind::Function));

        let common = a.common_prefix(&b);
        assert_eq!(common.len(), 2);
        assert_eq!(common.to_path_string(), "src > lib.rs");
    }

    #[test]
    fn breadcrumb_serialize_roundtrip() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("main.rs", BreadcrumbKind::File));
        path.push(sample_element("run", BreadcrumbKind::Function));
        let serialized = path.serialize();
        let deserialized = BreadcrumbPath::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.len(), 3);
        assert_eq!(deserialized.to_path_string(), path.to_path_string());
    }

    #[test]
    fn breadcrumb_serialize_empty() {
        let path = BreadcrumbPath::new();
        let serialized = path.serialize();
        assert_eq!(serialized, "");
        let deserialized = BreadcrumbPath::deserialize("").unwrap();
        assert!(deserialized.is_empty());
    }

    #[test]
    fn breadcrumb_deserialize_invalid_returns_none() {
        assert!(BreadcrumbPath::deserialize("no_colon").is_none());
        assert!(BreadcrumbPath::deserialize("badtag:label").is_none());
    }

    // -----------------------------------------------------------------------
    // Tests for new functionality
    // -----------------------------------------------------------------------

    #[test]
    fn history_back_and_forward() {
        let mut history = BreadcrumbHistory::new(10);
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());

        let mut p1 = BreadcrumbPath::new();
        p1.push(sample_element("src", BreadcrumbKind::Folder));
        let mut p2 = BreadcrumbPath::new();
        p2.push(sample_element("lib.rs", BreadcrumbKind::File));

        history.push(p1);
        history.push(p2);
        assert_eq!(history.len(), 2);
        assert_eq!(history.current().unwrap().to_path_string(), "lib.rs");

        // Go back
        assert!(history.can_go_back());
        let back = history.back().unwrap();
        assert_eq!(back.to_path_string(), "src");

        // Go forward
        assert!(history.can_go_forward());
        let fwd = history.forward().unwrap();
        assert_eq!(fwd.to_path_string(), "lib.rs");
        assert!(!history.can_go_forward());
    }

    #[test]
    fn history_push_truncates_forward() {
        let mut history = BreadcrumbHistory::new(10);
        let mut p1 = BreadcrumbPath::new();
        p1.push(sample_element("a", BreadcrumbKind::Folder));
        let mut p2 = BreadcrumbPath::new();
        p2.push(sample_element("b", BreadcrumbKind::Folder));
        let mut p3 = BreadcrumbPath::new();
        p3.push(sample_element("c", BreadcrumbKind::Folder));

        history.push(p1);
        history.push(p2);
        history.back(); // cursor at "a"
        // Pushing now should discard "b"
        history.push(p3);
        assert_eq!(history.len(), 2);
        assert_eq!(history.current().unwrap().to_path_string(), "c");
        assert!(!history.can_go_forward());
    }

    #[test]
    fn path_collapsed_short_path_unchanged() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("main.rs", BreadcrumbKind::File));
        let collapsed = path.collapsed();
        assert_eq!(collapsed.len(), 2);
        assert_eq!(collapsed.to_path_string(), "src > main.rs");
    }

    #[test]
    fn path_collapsed_replaces_middle_folders() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("project", BreadcrumbKind::Folder));
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("utils", BreadcrumbKind::Folder));
        path.push(sample_element("helpers", BreadcrumbKind::Folder));
        path.push(sample_element("lib.rs", BreadcrumbKind::File));
        path.push(sample_element("run", BreadcrumbKind::Function));

        let collapsed = path.collapsed();
        let labels: Vec<&str> = collapsed.labels();
        // first folder kept, middle folders collapsed to "…", file and symbol kept
        assert!(labels.contains(&"project"));
        assert!(labels.contains(&"…"));
        assert!(labels.contains(&"lib.rs"));
        assert!(labels.contains(&"run"));
        assert!(collapsed.len() < path.len());
    }

    #[test]
    fn divergence_index_finds_split() {
        let mut a = BreadcrumbPath::new();
        a.push(sample_element("src", BreadcrumbKind::Folder));
        a.push(sample_element("lib.rs", BreadcrumbKind::File));
        a.push(sample_element("foo", BreadcrumbKind::Function));

        let mut b = BreadcrumbPath::new();
        b.push(sample_element("src", BreadcrumbKind::Folder));
        b.push(sample_element("lib.rs", BreadcrumbKind::File));
        b.push(sample_element("bar", BreadcrumbKind::Function));

        assert_eq!(a.divergence_index(&b), 2);

        // Identical paths diverge at length
        assert_eq!(a.divergence_index(&a), 3);
    }

    #[test]
    fn search_finds_matching_elements() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("MyStruct", BreadcrumbKind::Class));
        path.push(sample_element("my_method", BreadcrumbKind::Method));

        let results = path.search("my");
        assert_eq!(results, vec![1, 2]);

        let empty = path.search("zzz");
        assert!(empty.is_empty());
    }

    #[test]
    fn depth_limited_returns_prefix() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("a", BreadcrumbKind::Folder));
        path.push(sample_element("b", BreadcrumbKind::Folder));
        path.push(sample_element("c", BreadcrumbKind::File));
        path.push(sample_element("d", BreadcrumbKind::Function));

        let limited = path.depth_limited(2);
        assert_eq!(limited.len(), 2);
        assert_eq!(limited.to_path_string(), "a > b");
    }

    #[test]
    fn breadcrumbs_from_outline_nested() {
        let outline = vec![OutlineEntry {
            name: "MyModule".into(),
            kind: BreadcrumbKind::Module,
            start_line: 0,
            end_line: 100,
            children: vec![OutlineEntry {
                name: "MyStruct".into(),
                kind: BreadcrumbKind::Class,
                start_line: 10,
                end_line: 50,
                children: vec![OutlineEntry {
                    name: "new".into(),
                    kind: BreadcrumbKind::Function,
                    start_line: 15,
                    end_line: 25,
                    children: vec![],
                }],
            }],
        }];

        let path = breadcrumbs_from_outline(&outline, 20);
        assert_eq!(path.len(), 3);
        assert_eq!(path.labels(), vec!["MyModule", "MyStruct", "new"]);

        // Cursor outside any symbol
        let empty = breadcrumbs_from_outline(&outline, 200);
        assert!(empty.is_empty());
    }

    // -- BreadcrumbPicker --------------------------------------------------

    #[test]
    fn picker_filter() {
        let items = vec![
            BreadcrumbPickerItem { label: "main.rs".into(), kind: BreadcrumbKind::File, is_selected: false },
            BreadcrumbPickerItem { label: "lib.rs".into(), kind: BreadcrumbKind::File, is_selected: false },
            BreadcrumbPickerItem { label: "mod.rs".into(), kind: BreadcrumbKind::File, is_selected: false },
        ];
        let mut picker = BreadcrumbPicker::new(items);
        let count = picker.set_filter("main");
        assert_eq!(count, 1);
        assert_eq!(picker.filtered_items()[0].label, "main.rs");
    }

    #[test]
    fn picker_navigation() {
        let items = vec![
            BreadcrumbPickerItem { label: "a".into(), kind: BreadcrumbKind::File, is_selected: false },
            BreadcrumbPickerItem { label: "b".into(), kind: BreadcrumbKind::File, is_selected: false },
        ];
        let mut picker = BreadcrumbPicker::new(items);
        picker.select_next();
        assert_eq!(picker.selected().unwrap().label, "a");
        picker.select_next();
        assert_eq!(picker.selected().unwrap().label, "b");
    }

    #[test]
    fn picker_display() {
        let picker = BreadcrumbPicker::new(Vec::new());
        assert!(format!("{picker}").contains("0 items"));
    }

    // -- BreadcrumbSymbolResolver ------------------------------------------

    #[test]
    fn symbol_resolver_icon() {
        let r = BreadcrumbSymbolResolver::new();
        assert_eq!(r.icon_for(&BreadcrumbKind::Function), 'ƒ');
        assert_eq!(r.icon_for(&BreadcrumbKind::File), '·'); // fallback
    }

    #[test]
    fn symbol_resolver_format() {
        let r = BreadcrumbSymbolResolver::new();
        let elem = r.resolve("main", BreadcrumbKind::Function, Some(1));
        let formatted = r.format_with_icon(&elem);
        assert!(formatted.contains("ƒ"));
        assert!(formatted.contains("main"));
    }

    #[test]
    fn symbol_resolver_display() {
        let r = BreadcrumbSymbolResolver::default();
        assert!(format!("{r}").contains("icons"));
    }

    // -- truncate_breadcrumb_elements --------------------------------------

    #[test]
    fn truncate_elements_short_enough() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("a", BreadcrumbKind::Folder));
        path.push(sample_element("b", BreadcrumbKind::File));
        let truncated = truncate_breadcrumb_elements(&path, 5);
        assert_eq!(truncated.len(), 2);
    }

    #[test]
    fn truncate_elements_with_ellipsis() {
        let mut path = BreadcrumbPath::new();
        for i in 0..6 {
            path.push(sample_element(&format!("seg{i}"), BreadcrumbKind::Folder));
        }
        let truncated = truncate_breadcrumb_elements(&path, 4);
        assert_eq!(truncated.elements[1].label, "…");
        assert_eq!(truncated.len(), 4);
    }

    // -- BreadcrumbFocusNavigator ------------------------------------------

    #[test]
    fn focus_navigator_next_previous() {
        let mut nav = BreadcrumbFocusNavigator::new(3);
        nav.focus_next();
        assert_eq!(nav.focused_index(), Some(0));
        nav.focus_next();
        assert_eq!(nav.focused_index(), Some(1));
        nav.focus_previous();
        assert_eq!(nav.focused_index(), Some(0));
    }

    #[test]
    fn focus_navigator_blur() {
        let mut nav = BreadcrumbFocusNavigator::new(3);
        nav.focus_next();
        nav.blur();
        assert!(!nav.is_focused());
    }

    #[test]
    fn focus_navigator_toggle_picker() {
        let mut nav = BreadcrumbFocusNavigator::new(3);
        assert!(!nav.is_picker_open());
        nav.toggle_picker();
        assert!(nav.is_picker_open());
    }

    #[test]
    fn focus_navigator_update_count() {
        let mut nav = BreadcrumbFocusNavigator::new(5);
        nav.focus_next();
        nav.focus_next();
        nav.focus_next();
        nav.update_count(2);
        assert_eq!(nav.focused_index(), Some(1));
    }

    #[test]
    fn focus_navigator_display() {
        let nav = BreadcrumbFocusNavigator::new(3);
        assert!(format!("{nav}").contains("segments=3"));
    }

#[test]
    fn breadcrumbrenderer_severity_ordering() {
        assert!(BreadcrumbRendererSeverity::Critical > BreadcrumbRendererSeverity::High);
        assert!(BreadcrumbRendererSeverity::High > BreadcrumbRendererSeverity::Medium);
        assert!(BreadcrumbRendererSeverity::Medium > BreadcrumbRendererSeverity::Low);
    }

    #[test]
    fn breadcrumbrenderer_severity_display() {
        assert_eq!(BreadcrumbRendererSeverity::Low.to_string(), "low");
        assert_eq!(BreadcrumbRendererSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn breadcrumbrenderer_entry_creation() {
        let e = BreadcrumbRendererEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, BreadcrumbRendererSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn breadcrumbrenderer_entry_builder() {
        let e = BreadcrumbRendererEntry::new("e2", "Entry 2")
            .with_severity(BreadcrumbRendererSeverity::High)
            .with_detail("some detail")
            .with_item_count(42);
        assert_eq!(e.severity, BreadcrumbRendererSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.item_count, 42);
    }

    #[test]
    fn breadcrumbrenderer_entry_enable_disable() {
        let mut e = BreadcrumbRendererEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn breadcrumbrenderer_add_and_count() {
        let mut mgr = BreadcrumbRenderer::new("test");
        mgr.add(BreadcrumbRendererEntry::new("a", "A"));
        mgr.add(BreadcrumbRendererEntry::new("b", "B").with_severity(BreadcrumbRendererSeverity::High));
        assert_eq!(mgr.item_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn breadcrumbrenderer_remove() {
        let mut mgr = BreadcrumbRenderer::new("test");
        mgr.add(BreadcrumbRendererEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn breadcrumbrenderer_capacity() {
        let mut mgr = BreadcrumbRenderer::new("test").with_capacity(1);
        assert!(mgr.add(BreadcrumbRendererEntry::new("a", "A")));
        assert!(!mgr.add(BreadcrumbRendererEntry::new("b", "B")));
    }

    #[test]
    fn breadcrumbrenderer_sorted_by_severity() {
        let mut mgr = BreadcrumbRenderer::new("test");
        mgr.add(BreadcrumbRendererEntry::new("lo", "Low"));
        mgr.add(BreadcrumbRendererEntry::new("hi", "High").with_severity(BreadcrumbRendererSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, BreadcrumbRendererSeverity::Critical);
    }

    #[test]
    fn breadcrumbrenderer_summary() {
        let mgr = BreadcrumbRenderer::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn pathshortener_config_defaults() {
        let cfg = PathShortenerConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn pathshortener_item_creation() {
        let item = PathShortenerItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn pathshortener_add_and_get() {
        let mut mgr = PathShortener::new(PathShortenerConfig::new("test"));
        mgr.add(PathShortenerItem::new("k1", "v1"));
        assert_eq!(mgr.max_depth(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn pathshortener_remove_item() {
        let mut mgr = PathShortener::new(PathShortenerConfig::new("test"));
        mgr.add(PathShortenerItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn pathshortener_sorted_by_priority() {
        let mut mgr = PathShortener::new(PathShortenerConfig::new("test"));
        mgr.add(PathShortenerItem::new("lo", "low").with_priority(1));
        mgr.add(PathShortenerItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn pathshortener_items_with_tag() {
        let mut mgr = PathShortener::new(PathShortenerConfig::new("test"));
        mgr.add(PathShortenerItem::new("a", "1").with_tag("x"));
        mgr.add(PathShortenerItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn pathshortener_report() {
        let mgr = PathShortener::new(PathShortenerConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    // -- breadcrumb additional tests -------------------------------------------

    #[test]
    fn x_breadcrumb_text_span_new_ordered() {
        let s = XBreadcrumbTextSpan::new(5, 10);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn x_breadcrumb_text_span_new_reversed() {
        let s = XBreadcrumbTextSpan::new(10, 5);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn x_breadcrumb_text_span_len() {
        assert_eq!(XBreadcrumbTextSpan::new(3, 7).len(), 4);
        assert_eq!(XBreadcrumbTextSpan::new(0, 0).len(), 0);
    }

    #[test]
    fn x_breadcrumb_text_span_extract() {
        let s = XBreadcrumbTextSpan::new(0, 5);
        assert_eq!(s.extract("hello world"), "hello");
    }

    #[test]
    fn x_breadcrumb_text_span_contains() {
        let s = XBreadcrumbTextSpan::new(2, 8);
        assert!(s.contains(2));
        assert!(s.contains(7));
        assert!(!s.contains(8));
    }

    #[test]
    fn x_breadcrumb_text_span_intersect() {
        let a = XBreadcrumbTextSpan::new(0, 10);
        let b = XBreadcrumbTextSpan::new(5, 15);
        let inter = a.intersect(&b).unwrap();
        assert_eq!(inter.start, 5);
        assert_eq!(inter.end, 10);
    }

    #[test]
    fn x_breadcrumb_text_span_intersect_none() {
        let a = XBreadcrumbTextSpan::new(0, 5);
        let b = XBreadcrumbTextSpan::new(5, 10);
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn x_breadcrumb_text_span_union() {
        let a = XBreadcrumbTextSpan::new(3, 7);
        let b = XBreadcrumbTextSpan::new(5, 12);
        let u = a.union(&b);
        assert_eq!(u.start, 3);
        assert_eq!(u.end, 12);
    }

    #[test]
    fn x_breadcrumb_count_lines_basic() {
        assert_eq!(x_breadcrumb_count_lines("a\nb\nc"), 3);
        assert_eq!(x_breadcrumb_count_lines(""), 0);
        assert_eq!(x_breadcrumb_count_lines("single"), 1);
    }

    #[test]
    fn x_breadcrumb_line_start_offset_basic() {
        assert_eq!(x_breadcrumb_line_start_offset("abc\ndef\nghi", 0), Some(0));
        assert_eq!(x_breadcrumb_line_start_offset("abc\ndef\nghi", 1), Some(4));
        assert_eq!(x_breadcrumb_line_start_offset("abc\ndef\nghi", 2), Some(8));
        assert_eq!(x_breadcrumb_line_start_offset("abc\ndef\nghi", 3), None);
    }

    #[test]
    fn x_breadcrumb_indent_level_basic() {
        assert_eq!(x_breadcrumb_indent_level("    hello"), 4);
        assert_eq!(x_breadcrumb_indent_level("hello"), 0);
        assert_eq!(x_breadcrumb_indent_level("  "), 2);
    }

    #[test]
    fn x_breadcrumb_trim_trailing_basic() {
        let input = "hello   \nworld  \n  foo  ";
        let result = x_breadcrumb_trim_trailing(input);
        assert_eq!(result, "hello\nworld\n  foo");
    }

    #[test]
    fn x_breadcrumb_detect_eol_lf() {
        assert_eq!(x_breadcrumb_detect_eol("a\nb\nc"), "\n");
    }

    #[test]
    fn x_breadcrumb_detect_eol_crlf() {
        assert_eq!(x_breadcrumb_detect_eol("a\r\nb\r\nc"), "\r\n");
    }

    #[test]
    fn x_breadcrumb_tokenize_basic() {
        let tokens = x_breadcrumb_tokenize("hello, world! foo");
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn x_breadcrumb_text_span_shift() {
        let s = XBreadcrumbTextSpan::new(2, 5).shift(10);
        assert_eq!(s.start, 12);
        assert_eq!(s.end, 15);
    }


    #[test]
    fn breadcrumb_config_new() {
        let cfg = BreadcrumbConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn breadcrumb_config_set_get() {
        let mut cfg = BreadcrumbConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn breadcrumb_config_remove() {
        let mut cfg = BreadcrumbConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn breadcrumb_config_keys_sorted() {
        let mut cfg = BreadcrumbConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn breadcrumb_config_bump_version() {
        let mut cfg = BreadcrumbConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn breadcrumb_config_clear() {
        let mut cfg = BreadcrumbConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn breadcrumb_config_merge() {
        let mut cfg1 = BreadcrumbConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = BreadcrumbConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn breadcrumb_config_disable() {
        let mut cfg = BreadcrumbConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn breadcrumb_rate_tracker_empty() {
        let rt = BreadcrumbRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn breadcrumb_rate_tracker_record() {
        let mut rt = BreadcrumbRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn breadcrumb_rate_tracker_prune() {
        let mut rt = BreadcrumbRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn breadcrumb_validator_valid() {
        let v = BreadcrumbValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn breadcrumb_validator_errors() {
        let mut v = BreadcrumbValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn breadcrumb_validator_clear() {
        let mut v = BreadcrumbValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn breadcrumb_validator_merge() {
        let mut v1 = BreadcrumbValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = BreadcrumbValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn breadcrumb_rate_tracker_clear() {
        let mut rt = BreadcrumbRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
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


    // xa_ extended tests for breadcrumb
    #[test]
    fn xa_breadcrumb_ring_new() {
        let rb = super::XaBreadcrumbRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_breadcrumb_ring_push_len() {
        let mut rb = super::XaBreadcrumbRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_breadcrumb_ring_wrap() {
        let mut rb = super::XaBreadcrumbRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_breadcrumb_ring_mean_empty() {
        let rb = super::XaBreadcrumbRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_breadcrumb_ring_mean_values() {
        let mut rb = super::XaBreadcrumbRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_breadcrumb_ring_min_max() {
        let mut rb = super::XaBreadcrumbRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_breadcrumb_ring_iter() {
        let mut rb = super::XaBreadcrumbRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_breadcrumb_counter_new() {
        let c = super::XaBreadcrumbCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_breadcrumb_counter_inc() {
        let mut c = super::XaBreadcrumbCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_breadcrumb_counter_inc_by() {
        let mut c = super::XaBreadcrumbCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_breadcrumb_counter_reset() {
        let mut c = super::XaBreadcrumbCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_breadcrumb_counter_clear() {
        let mut c = super::XaBreadcrumbCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_breadcrumb_counter_default() {
        let c = super::XaBreadcrumbCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 8 ----

    #[test]
    fn xc_8_pool_new_empty() {
        let pool: super::Xc8Pool<i32> = super::Xc8Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_8_pool_release_acquire() {
        let mut pool = super::Xc8Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_8_pool_acquire_empty() {
        let mut pool: super::Xc8Pool<i32> = super::Xc8Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_8_pool_full() {
        let mut pool = super::Xc8Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_8_pool_drain() {
        let mut pool = super::Xc8Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_8_pool_stats() {
        let mut pool = super::Xc8Pool::new(8);
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
    fn xc_8_pool_clear() {
        let mut pool = super::Xc8Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_8_pool_shrink() {
        let mut pool = super::Xc8Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_8_pool_default() {
        let pool: super::Xc8Pool<String> = super::Xc8Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_8_pool_extend() {
        let mut pool = super::Xc8Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_8_pool_retain() {
        let mut pool = super::Xc8Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_8_scheduler_round_robin() {
        let mut sched = super::Xc8Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_8_scheduler_empty() {
        let mut sched = super::Xc8Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_8_scheduler_reset() {
        let mut sched = super::Xc8Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_8_scheduler_add_remove() {
        let mut sched = super::Xc8Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_8_scheduler_targets() {
        let sched = super::Xc8Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_8_hash_empty() {
        assert_eq!(super::xc_8_hash(b""), 5381);
    }

    #[test]
    fn xc_8_hash_data() {
        let h = super::xc_8_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_8_hash(b"hello"), h);
    }

    #[test]
    fn xc_8_reverse_str() {
        assert_eq!(super::xc_8_reverse("abc"), "cba");
        assert_eq!(super::xc_8_reverse(""), "");
    }


    // --- xd_94 deepening tests ---

    #[test]
    fn xd_94_sm_initial_state() {
        let sm = Xd94StateMachine::new();
        assert_eq!(sm.current_state(), Xd94State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_94_sm_valid_idle_to_running() {
        let mut sm = Xd94StateMachine::new();
        assert!(sm.transition(Xd94State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd94State::Running);
    }

    #[test]
    fn xd_94_sm_valid_running_to_paused() {
        let mut sm = Xd94StateMachine::new();
        sm.transition(Xd94State::Running).unwrap();
        assert!(sm.transition(Xd94State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd94State::Paused);
    }

    #[test]
    fn xd_94_sm_valid_running_to_done() {
        let mut sm = Xd94StateMachine::new();
        sm.transition(Xd94State::Running).unwrap();
        assert!(sm.transition(Xd94State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd94State::Done);
    }

    #[test]
    fn xd_94_sm_valid_paused_to_running() {
        let mut sm = Xd94StateMachine::new();
        sm.transition(Xd94State::Running).unwrap();
        sm.transition(Xd94State::Paused).unwrap();
        assert!(sm.transition(Xd94State::Running).is_ok());
    }

    #[test]
    fn xd_94_sm_valid_done_to_idle() {
        let mut sm = Xd94StateMachine::new();
        sm.transition(Xd94State::Running).unwrap();
        sm.transition(Xd94State::Done).unwrap();
        assert!(sm.transition(Xd94State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd94State::Idle);
    }

    #[test]
    fn xd_94_sm_invalid_idle_to_done() {
        let mut sm = Xd94StateMachine::new();
        assert!(sm.transition(Xd94State::Done).is_err());
    }

    #[test]
    fn xd_94_sm_invalid_idle_to_paused() {
        let mut sm = Xd94StateMachine::new();
        assert!(sm.transition(Xd94State::Paused).is_err());
    }

    #[test]
    fn xd_94_sm_history_tracking() {
        let mut sm = Xd94StateMachine::new();
        sm.transition(Xd94State::Running).unwrap();
        sm.transition(Xd94State::Paused).unwrap();
        sm.transition(Xd94State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd94State::Idle);
        assert_eq!(sm.history()[0].to, Xd94State::Running);
        assert_eq!(sm.history()[1].from, Xd94State::Running);
        assert_eq!(sm.history()[2].to, Xd94State::Done);
    }

    #[test]
    fn xd_94_sm_serialize_deserialize() {
        let mut sm = Xd94StateMachine::new();
        sm.transition(Xd94State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd94StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd94State::Running));
    }

    #[test]
    fn xd_94_sm_deserialize_invalid() {
        assert_eq!(Xd94StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_94_sm_reset() {
        let mut sm = Xd94StateMachine::new();
        sm.transition(Xd94State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd94State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_94_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd94EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd94Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_94_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd94EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd94Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd94Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_94_bus_unsubscribe() {
        let mut bus = Xd94EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_94_event_kind_and_payload() {
        let e = Xd94Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd94Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_94_bus_clear_history() {
        let mut bus = Xd94EventBus::new();
        bus.publish(Xd94Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_94_sm_step_counter_increments() {
        let mut sm = Xd94StateMachine::new();
        sm.transition(Xd94State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd94State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }

}
