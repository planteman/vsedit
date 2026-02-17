//! Path breadcrumb navigation.
//!
//! Provides breadcrumb data structures and a renderable navigation bar
//! with keyboard-navigable segments — rendered via ratatui.

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
    fn breadcrumb_validator_accepts_valid_name() {
        let v = BreadcrumbValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn breadcrumb_validator_rejects_empty() {
        let v = BreadcrumbValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn breadcrumb_validator_rejects_too_long() {
        let v = BreadcrumbValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn breadcrumb_validator_forbidden_prefix() {
        let v = BreadcrumbValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn breadcrumb_validator_allowed_chars() {
        let v = BreadcrumbValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn breadcrumb_validator_range() {
        let v = BreadcrumbValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn breadcrumb_sanitize_removes_control() {
        let result = BreadcrumbValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn breadcrumb_truncate_short_string() {
        assert_eq!(BreadcrumbValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn breadcrumb_truncate_long_string() {
        let result = BreadcrumbValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn breadcrumb_is_ascii_printable() {
        assert!(BreadcrumbValidator::is_ascii_printable("Hello World 123"));
        assert!(!BreadcrumbValidator::is_ascii_printable("Hello\x00World"));
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
}
