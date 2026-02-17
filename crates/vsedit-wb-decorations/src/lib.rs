//! File decorations.

/// Defines the visual style of a decoration.
#[derive(Debug, Clone)]
pub struct DecorationType {
    pub id: String,
    pub background_color: Option<String>,
    pub border: Option<String>,
    pub outline: Option<String>,
    pub gutter_icon: Option<String>,
    pub is_whole_line: bool,
    pub after_text: Option<String>,
    pub before_text: Option<String>,
}

/// A range within a document where a decoration applies.
#[derive(Debug, Clone, PartialEq)]
pub struct DecorationRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub hover_message: Option<String>,
}

/// A set of decoration ranges applied to a specific document.
#[derive(Debug, Clone)]
pub struct DecorationSet {
    pub type_id: String,
    pub uri: String,
    pub ranges: Vec<DecorationRange>,
}

/// Service for managing decorations across documents.
pub struct DecorationService {
    pub types: Vec<DecorationType>,
    pub sets: Vec<DecorationSet>,
}

impl DecorationService {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            sets: Vec::new(),
        }
    }

    /// Register a new decoration type.
    pub fn register_type(&mut self, dt: DecorationType) {
        self.types.push(dt);
    }

    /// Set decorations for a given type and URI, replacing any existing set
    /// with the same type_id and uri.
    pub fn set_decorations(
        &mut self,
        type_id: String,
        uri: String,
        ranges: Vec<DecorationRange>,
    ) {
        self.sets
            .retain(|s| !(s.type_id == type_id && s.uri == uri));
        self.sets.push(DecorationSet {
            type_id,
            uri,
            ranges,
        });
    }

    /// Get all decoration sets for a given URI.
    pub fn get_decorations(&self, uri: &str) -> Vec<&DecorationSet> {
        self.sets.iter().filter(|s| s.uri == uri).collect()
    }

    /// Remove decorations for a specific type and URI.
    pub fn remove_decorations(&mut self, type_id: &str, uri: &str) {
        self.sets
            .retain(|s| !(s.type_id == type_id && s.uri == uri));
    }

    /// Clear all decoration types and sets.
    pub fn clear_all(&mut self) {
        self.types.clear();
        self.sets.clear();
    }

    pub fn get_type(&self, id: &str) -> Option<&DecorationType> {
        self.types.iter().find(|t| t.id == id)
    }

    pub fn has_type(&self, id: &str) -> bool {
        self.types.iter().any(|t| t.id == id)
    }

    pub fn unregister_type(&mut self, id: &str) -> bool {
        let len = self.types.len();
        self.types.retain(|t| t.id != id);
        self.sets.retain(|s| s.type_id != id);
        self.types.len() != len
    }

    pub fn get_all_uris(&self) -> Vec<&str> {
        let mut uris: Vec<&str> = self.sets.iter().map(|s| s.uri.as_str()).collect();
        uris.sort();
        uris.dedup();
        uris
    }

    pub fn decoration_count(&self) -> usize {
        self.sets.iter().map(|s| s.ranges.len()).sum()
    }
}

impl Default for DecorationService {
    fn default() -> Self {
        Self::new()
    }
}

/// Merge overlapping or adjacent ranges on the same line into combined ranges.
pub fn merge_ranges(mut ranges: Vec<DecorationRange>) -> Vec<DecorationRange> {
    if ranges.is_empty() {
        return ranges;
    }
    ranges.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.start_col.cmp(&b.start_col))
    });
    let mut merged: Vec<DecorationRange> = Vec::new();
    merged.push(ranges[0].clone());
    for r in ranges.into_iter().skip(1) {
        let last = merged.last_mut().unwrap();
        if r.start_line <= last.end_line && r.start_col <= last.end_col {
            if r.end_line > last.end_line || (r.end_line == last.end_line && r.end_col > last.end_col) {
                last.end_line = r.end_line;
                last.end_col = r.end_col;
            }
        } else {
            merged.push(r);
        }
    }
    merged
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeBehavior {
    OpenOpen,
    ClosedClosed,
    OpenClosed,
    ClosedOpen,
}

#[derive(Debug, Clone)]
pub struct DecorationRenderOptions {
    pub background_color: Option<String>,
    pub border_color: Option<String>,
    pub border_width: Option<String>,
    pub border_style: Option<String>,
    pub font_weight: Option<String>,
    pub font_style: Option<String>,
    pub opacity: Option<f32>,
    pub range_behavior: RangeBehavior,
}

impl Default for DecorationRenderOptions {
    fn default() -> Self {
        Self {
            background_color: None,
            border_color: None,
            border_width: None,
            border_style: None,
            font_weight: None,
            font_style: None,
            opacity: None,
            range_behavior: RangeBehavior::OpenOpen,
        }
    }
}

pub trait DecorationProvider {
    fn provide_decorations(&self, uri: &str) -> Vec<DecorationRange>;

    fn event_uri_filter(&self) -> Option<Vec<String>> {
        None
    }
}

use std::fmt;

/// Priority level for decorations, ordered from Low to Critical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DecorationPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// A decoration range paired with a priority and type identifier.
#[derive(Debug, Clone)]
pub struct PrioritizedDecoration {
    pub range: DecorationRange,
    pub priority: DecorationPriority,
    pub type_id: String,
}

/// Utility struct with static methods for sorting and filtering decoration ranges.
pub struct DecorationSorter;

impl DecorationSorter {
    /// Sort ranges by start_line, then by start_col.
    pub fn sort_by_line(ranges: &mut Vec<DecorationRange>) {
        ranges.sort_by(|a, b| {
            a.start_line
                .cmp(&b.start_line)
                .then(a.start_col.cmp(&b.start_col))
        });
    }

    /// Sort prioritized decorations by priority descending (Critical first).
    pub fn sort_by_priority(items: &mut Vec<PrioritizedDecoration>) {
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Return ranges that overlap with the inclusive line range [start, end].
    pub fn filter_by_line_range(
        ranges: &[DecorationRange],
        start: u32,
        end: u32,
    ) -> Vec<DecorationRange> {
        ranges
            .iter()
            .filter(|r| r.start_line <= end && r.end_line >= start)
            .cloned()
            .collect()
    }

    /// Count how many decoration ranges touch each line.
    pub fn count_by_line(ranges: &[DecorationRange]) -> std::collections::HashMap<u32, usize> {
        let mut counts = std::collections::HashMap::new();
        for r in ranges {
            for line in r.start_line..=r.end_line {
                *counts.entry(line).or_insert(0) += 1;
            }
        }
        counts
    }
}

impl fmt::Display for DecorationRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "L{}:{}-L{}:{}",
            self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
}

impl fmt::Display for DecorationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DecorationType({})", self.id)
    }
}

impl DecorationService {
    /// Get a slice of all registered decoration types.
    pub fn get_types(&self) -> &[DecorationType] {
        &self.types
    }

    /// Return the number of registered decoration types.
    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    /// Return the number of decoration sets.
    pub fn set_count(&self) -> usize {
        self.sets.len()
    }

    /// Get all decoration sets whose type_id matches `type_id`.
    pub fn get_decorations_by_type(&self, type_id: &str) -> Vec<&DecorationSet> {
        self.sets.iter().filter(|s| s.type_id == type_id).collect()
    }

    /// Remove all decoration sets for the given URI.
    pub fn remove_all_for_uri(&mut self, uri: &str) {
        self.sets.retain(|s| s.uri != uri);
    }

    /// Check whether any decoration set targets the given URI.
    pub fn has_decorations(&self, uri: &str) -> bool {
        self.sets.iter().any(|s| s.uri == uri)
    }
}

/// Accumulated statistics for wb-decorations operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbDecorationsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbDecorationsStats {
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
    pub fn merge(&mut self, other: &WbDecorationsStats) {
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

impl Default for WbDecorationsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbDecorationsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbDecorationsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-decorations.
#[derive(Debug, Clone)]
pub struct WbDecorationsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbDecorationsValidator {
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

impl Default for WbDecorationsValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Gutter indicators
// ---------------------------------------------------------------------------

/// The visual type of a gutter indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GutterIndicatorKind {
    /// Breakpoint (red dot).
    Breakpoint,
    /// Error indicator (red circle).
    Error,
    /// Warning indicator (yellow triangle).
    Warning,
    /// Git addition (green bar).
    GitAdd,
    /// Git modification (blue bar).
    GitModify,
    /// Git deletion (red bar).
    GitDelete,
    /// Custom kind identified by a string stored elsewhere.
    Custom,
}

/// A single gutter decoration on a specific line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GutterDecoration {
    /// The 1-based line number.
    pub line: u32,
    /// Icon name or path.
    pub icon: Option<String>,
    /// Color string (e.g. "red", "#ff0000").
    pub color: Option<String>,
    /// Tooltip shown on hover.
    pub tooltip: Option<String>,
    /// The kind of indicator.
    pub kind: GutterIndicatorKind,
}

/// Service for managing gutter decorations across documents.
pub struct GutterIndicatorService {
    decorations: Vec<(String, Vec<GutterDecoration>)>,
}

impl GutterIndicatorService {
    pub fn new() -> Self {
        Self {
            decorations: Vec::new(),
        }
    }

    /// Set gutter decorations for a URI, replacing any previous set.
    pub fn set_decorations(&mut self, uri: impl Into<String>, decs: Vec<GutterDecoration>) {
        let uri = uri.into();
        self.decorations.retain(|(u, _)| u != &uri);
        if !decs.is_empty() {
            self.decorations.push((uri, decs));
        }
    }

    /// Get gutter decorations for a given URI.
    pub fn get_decorations(&self, uri: &str) -> &[GutterDecoration] {
        self.decorations
            .iter()
            .find(|(u, _)| u == uri)
            .map(|(_, d)| d.as_slice())
            .unwrap_or(&[])
    }

    /// Get decorations for a specific line in a URI.
    pub fn decorations_for_line(&self, uri: &str, line: u32) -> Vec<&GutterDecoration> {
        self.get_decorations(uri)
            .iter()
            .filter(|d| d.line == line)
            .collect()
    }

    /// Clear all gutter decorations for a URI.
    pub fn clear(&mut self, uri: &str) {
        self.decorations.retain(|(u, _)| u != uri);
    }

    /// Clear everything.
    pub fn clear_all(&mut self) {
        self.decorations.clear();
    }

    /// Total number of gutter decorations across all URIs.
    pub fn total_count(&self) -> usize {
        self.decorations.iter().map(|(_, d)| d.len()).sum()
    }
}

impl Default for GutterIndicatorService {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for GutterIndicatorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Breakpoint => write!(f, "breakpoint"),
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::GitAdd => write!(f, "git-add"),
            Self::GitModify => write!(f, "git-modify"),
            Self::GitDelete => write!(f, "git-delete"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl fmt::Display for GutterDecoration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L{} [{}]", self.line, self.kind)
    }
}

// ---------------------------------------------------------------------------
// DecorationRange helpers
// ---------------------------------------------------------------------------

impl DecorationRange {
    /// Create a single-line range.
    pub fn single_line(line: u32, start_col: u32, end_col: u32) -> Self {
        Self {
            start_line: line,
            start_col,
            end_line: line,
            end_col,
            hover_message: None,
        }
    }

    /// Create a whole-line range.
    pub fn whole_line(line: u32) -> Self {
        Self {
            start_line: line,
            start_col: 0,
            end_line: line,
            end_col: u32::MAX,
            hover_message: None,
        }
    }

    /// Set the hover message.
    pub fn with_hover(mut self, msg: impl Into<String>) -> Self {
        self.hover_message = Some(msg.into());
        self
    }

    /// Number of lines this range spans.
    pub fn line_span(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// Returns true if the range spans multiple lines.
    pub fn is_multiline(&self) -> bool {
        self.start_line != self.end_line
    }

    /// Returns true if this range contains the given position.
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

    /// Returns true if two ranges overlap.
    pub fn overlaps(&self, other: &DecorationRange) -> bool {
        if self.end_line < other.start_line || other.end_line < self.start_line {
            return false;
        }
        if self.end_line == other.start_line && self.end_col <= other.start_col {
            return false;
        }
        if other.end_line == self.start_line && other.end_col <= self.start_col {
            return false;
        }
        true
    }
}

impl Default for DecorationRange {
    fn default() -> Self {
        Self::single_line(0, 0, 0)
    }
}

// ---------------------------------------------------------------------------
// DecorationType builder
// ---------------------------------------------------------------------------

impl DecorationType {
    /// Create a minimal decoration type with just an id.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            background_color: None,
            border: None,
            outline: None,
            gutter_icon: None,
            is_whole_line: false,
            after_text: None,
            before_text: None,
        }
    }

    /// Set background color.
    pub fn with_background(mut self, color: impl Into<String>) -> Self {
        self.background_color = Some(color.into());
        self
    }

    /// Set as whole line decoration.
    pub fn whole_line(mut self) -> Self {
        self.is_whole_line = true;
        self
    }

    /// Set after text.
    pub fn with_after_text(mut self, text: impl Into<String>) -> Self {
        self.after_text = Some(text.into());
        self
    }
}

// ---------------------------------------------------------------------------
// GutterIndicatorKind helpers
// ---------------------------------------------------------------------------

impl GutterIndicatorKind {
    /// Returns all gutter indicator variants.
    pub fn all() -> Vec<Self> {
        vec![
            Self::Breakpoint,
            Self::Error,
            Self::Warning,
            Self::GitAdd,
            Self::GitModify,
            Self::GitDelete,
            Self::Custom,
        ]
    }

    /// Returns true if this is a diff-related indicator.
    pub fn is_diff(&self) -> bool {
        matches!(self, Self::GitAdd | Self::GitModify | Self::GitDelete)
    }

    /// Returns true if this is a diagnostic indicator.
    pub fn is_diagnostic(&self) -> bool {
        matches!(self, Self::Error | Self::Warning)
    }
}

/// Count decorations by type across all sets.
pub fn count_by_type(service: &DecorationService) -> std::collections::HashMap<String, usize> {
    let mut map = std::collections::HashMap::new();
    for set in &service.sets {
        *map.entry(set.type_id.clone()).or_insert(0) += set.ranges.len();
    }
    map
}

/// Find all decorations at a given line.
pub fn decorations_at_line(service: &DecorationService, line: u32) -> Vec<(&DecorationSet, &DecorationRange)> {
    let mut results = Vec::new();
    for set in &service.sets {
        for range in &set.ranges {
            if range.start_line <= line && range.end_line >= line {
                results.push((set, range));
            }
        }
    }
    results
}

// ---------------------------------------------------------------------------
// Decoration style merging
// ---------------------------------------------------------------------------

/// Resolved style properties for rendering a decoration, produced by merging
/// multiple `DecorationRenderOptions` in priority order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResolvedStyle {
    pub background_color: Option<String>,
    pub border_color: Option<String>,
    pub border_width: Option<String>,
    pub border_style: Option<String>,
    pub font_weight: Option<String>,
    pub font_style: Option<String>,
    pub opacity: Option<f32>,
}

impl ResolvedStyle {
    /// Merge `other` into `self`. Fields in `other` override only if they are
    /// `Some` and the corresponding field in `self` is `None`.
    pub fn merge_base(&mut self, other: &DecorationRenderOptions) {
        if self.background_color.is_none() {
            self.background_color.clone_from(&other.background_color);
        }
        if self.border_color.is_none() {
            self.border_color.clone_from(&other.border_color);
        }
        if self.border_width.is_none() {
            self.border_width.clone_from(&other.border_width);
        }
        if self.border_style.is_none() {
            self.border_style.clone_from(&other.border_style);
        }
        if self.font_weight.is_none() {
            self.font_weight.clone_from(&other.font_weight);
        }
        if self.font_style.is_none() {
            self.font_style.clone_from(&other.font_style);
        }
        if self.opacity.is_none() {
            self.opacity = other.opacity;
        }
    }

    /// Force-merge `other` into `self`; `Some` fields in `other` always win.
    pub fn merge_override(&mut self, other: &DecorationRenderOptions) {
        if other.background_color.is_some() {
            self.background_color.clone_from(&other.background_color);
        }
        if other.border_color.is_some() {
            self.border_color.clone_from(&other.border_color);
        }
        if other.border_width.is_some() {
            self.border_width.clone_from(&other.border_width);
        }
        if other.border_style.is_some() {
            self.border_style.clone_from(&other.border_style);
        }
        if other.font_weight.is_some() {
            self.font_weight.clone_from(&other.font_weight);
        }
        if other.font_style.is_some() {
            self.font_style.clone_from(&other.font_style);
        }
        if other.opacity.is_some() {
            self.opacity = other.opacity;
        }
    }
}

/// Merge a list of render options in order (first = lowest priority).
/// Later entries override earlier ones.
pub fn merge_render_options(options: &[DecorationRenderOptions]) -> ResolvedStyle {
    let mut resolved = ResolvedStyle::default();
    for opt in options {
        resolved.merge_override(opt);
    }
    resolved
}

// ---------------------------------------------------------------------------
// Decoration inheritance (child inherits parent unless overridden)
// ---------------------------------------------------------------------------

/// A node in a decoration inheritance tree.  Children inherit the parent's
/// `DecorationRenderOptions` but may override individual fields.
#[derive(Debug, Clone)]
pub struct DecorationNode {
    pub id: String,
    pub options: DecorationRenderOptions,
    pub children: Vec<DecorationNode>,
}

impl DecorationNode {
    pub fn new(id: impl Into<String>, options: DecorationRenderOptions) -> Self {
        Self {
            id: id.into(),
            options,
            children: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child: DecorationNode) {
        self.children.push(child);
    }

    /// Resolve the effective style for this node by merging a parent style (as
    /// base) with this node's own options (as overrides).
    pub fn resolve(&self, parent_style: &ResolvedStyle) -> ResolvedStyle {
        let mut style = parent_style.clone();
        style.merge_override(&self.options);
        style
    }

    /// Collect `(id, ResolvedStyle)` pairs for this node and all descendants.
    pub fn resolve_tree(&self, parent_style: &ResolvedStyle) -> Vec<(String, ResolvedStyle)> {
        let my_style = self.resolve(parent_style);
        let mut out = vec![(self.id.clone(), my_style.clone())];
        for child in &self.children {
            out.extend(child.resolve_tree(&my_style));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Batch decoration updates with change detection
// ---------------------------------------------------------------------------

/// Represents a pending change to a decoration set.
#[derive(Debug, Clone, PartialEq)]
pub enum DecorationChange {
    /// A decoration set was added or replaced.
    Set {
        type_id: String,
        uri: String,
        ranges: Vec<DecorationRange>,
    },
    /// A decoration set was removed.
    Remove { type_id: String, uri: String },
    /// All decorations for a URI were removed.
    RemoveUri { uri: String },
}

/// Accumulates decoration mutations and applies them as a batch, returning
/// only the changes that actually modified state.
pub struct BatchDecorationUpdate {
    changes: Vec<DecorationChange>,
}

impl BatchDecorationUpdate {
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
        }
    }

    pub fn set(
        &mut self,
        type_id: impl Into<String>,
        uri: impl Into<String>,
        ranges: Vec<DecorationRange>,
    ) {
        self.changes.push(DecorationChange::Set {
            type_id: type_id.into(),
            uri: uri.into(),
            ranges,
        });
    }

    pub fn remove(&mut self, type_id: impl Into<String>, uri: impl Into<String>) {
        self.changes.push(DecorationChange::Remove {
            type_id: type_id.into(),
            uri: uri.into(),
        });
    }

    pub fn remove_uri(&mut self, uri: impl Into<String>) {
        self.changes.push(DecorationChange::RemoveUri {
            uri: uri.into(),
        });
    }

    /// Apply all accumulated changes to `service`. Returns the changes that
    /// actually modified state (i.e. were not no-ops).
    pub fn apply(self, service: &mut DecorationService) -> Vec<DecorationChange> {
        let mut effective: Vec<DecorationChange> = Vec::new();
        for change in self.changes {
            match &change {
                DecorationChange::Set {
                    type_id,
                    uri,
                    ranges,
                } => {
                    let existing: Vec<DecorationRange> = service
                        .get_decorations(uri)
                        .iter()
                        .filter(|s| s.type_id == *type_id)
                        .flat_map(|s| s.ranges.clone())
                        .collect();
                    if existing != *ranges {
                        service.set_decorations(
                            type_id.clone(),
                            uri.clone(),
                            ranges.clone(),
                        );
                        effective.push(change);
                    }
                }
                DecorationChange::Remove { type_id, uri } => {
                    if service
                        .sets
                        .iter()
                        .any(|s| s.type_id == *type_id && s.uri == *uri)
                    {
                        service.remove_decorations(type_id, uri);
                        effective.push(change);
                    }
                }
                DecorationChange::RemoveUri { uri } => {
                    if service.has_decorations(uri) {
                        service.remove_all_for_uri(uri);
                        effective.push(change);
                    }
                }
            }
        }
        effective
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.changes.len()
    }
}

impl Default for BatchDecorationUpdate {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Decoration filtering by type / source
// ---------------------------------------------------------------------------

/// Source annotation for a decoration, allowing filtering by originator.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DecorationSource {
    /// Built-in editor decoration (e.g. search highlight).
    BuiltIn,
    /// Decoration from an extension, identified by extension id.
    Extension(String),
    /// Decoration from a language server.
    LanguageServer,
    /// Decoration from a debugger.
    Debugger,
}

impl fmt::Display for DecorationSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuiltIn => write!(f, "built-in"),
            Self::Extension(id) => write!(f, "extension:{id}"),
            Self::LanguageServer => write!(f, "language-server"),
            Self::Debugger => write!(f, "debugger"),
        }
    }
}

/// A decoration set annotated with its source and priority, enabling
/// filtering and ordering in multi-provider scenarios.
#[derive(Debug, Clone)]
pub struct SourcedDecorationSet {
    pub set: DecorationSet,
    pub source: DecorationSource,
    pub priority: DecorationPriority,
}

/// Filter and query a collection of `SourcedDecorationSet` values.
pub struct DecorationFilter;

impl DecorationFilter {
    /// Keep only sets from the given source.
    pub fn by_source<'a>(
        sets: &'a [SourcedDecorationSet],
        source: &DecorationSource,
    ) -> Vec<&'a SourcedDecorationSet> {
        sets.iter().filter(|s| &s.source == source).collect()
    }

    /// Keep only sets with priority >= `min`.
    pub fn by_min_priority(
        sets: &[SourcedDecorationSet],
        min: DecorationPriority,
    ) -> Vec<&SourcedDecorationSet> {
        sets.iter().filter(|s| s.priority >= min).collect()
    }

    /// Keep only sets targeting the given URI.
    pub fn by_uri<'a>(
        sets: &'a [SourcedDecorationSet],
        uri: &str,
    ) -> Vec<&'a SourcedDecorationSet> {
        sets.iter().filter(|s| s.set.uri == uri).collect()
    }

    /// Keep only sets whose type_id matches.
    pub fn by_type_id<'a>(
        sets: &'a [SourcedDecorationSet],
        type_id: &str,
    ) -> Vec<&'a SourcedDecorationSet> {
        sets.iter().filter(|s| s.set.type_id == type_id).collect()
    }

    /// Combine multiple filters: source + minimum priority + URI.
    pub fn query<'a>(
        sets: &'a [SourcedDecorationSet],
        source: Option<&DecorationSource>,
        min_priority: Option<DecorationPriority>,
        uri: Option<&str>,
    ) -> Vec<&'a SourcedDecorationSet> {
        sets.iter()
            .filter(|s| source.map_or(true, |src| &s.source == src))
            .filter(|s| min_priority.map_or(true, |mp| s.priority >= mp))
            .filter(|s| uri.map_or(true, |u| s.set.uri == u))
            .collect()
    }
}


// ---------------------------------------------------------------------------
// DecorationLayerOrder
// ---------------------------------------------------------------------------

pub struct DecorationLayerOrder;

impl DecorationLayerOrder {
    pub fn compare(a: DecorationPriority, b: DecorationPriority) -> std::cmp::Ordering {
        let rank = |p: &DecorationPriority| match p {
            DecorationPriority::Low => 0,
            DecorationPriority::Normal => 1,
            DecorationPriority::High => 2,
            DecorationPriority::Critical => 3,
        };
        rank(&a).cmp(&rank(&b))
    }

    pub fn is_higher(a: DecorationPriority, b: DecorationPriority) -> bool {
        Self::compare(a, b) == std::cmp::Ordering::Greater
    }
}

// ---------------------------------------------------------------------------
// DecorationPerformanceTracker
// ---------------------------------------------------------------------------

pub struct DecorationPerformanceTracker {
    update_times_ms: Vec<u64>,
    total_updates: u64,
}

impl DecorationPerformanceTracker {
    pub fn new() -> Self { Self { update_times_ms: Vec::new(), total_updates: 0 } }

    pub fn record_update(&mut self, duration_ms: u64) {
        self.update_times_ms.push(duration_ms);
        self.total_updates += 1;
    }

    pub fn average_update_ms(&self) -> Option<u64> {
        if self.update_times_ms.is_empty() { None }
        else { Some(self.update_times_ms.iter().sum::<u64>() / self.update_times_ms.len() as u64) }
    }

    pub fn max_update_ms(&self) -> Option<u64> { self.update_times_ms.iter().copied().max() }
    pub fn total_updates(&self) -> u64 { self.total_updates }
    pub fn reset(&mut self) { self.update_times_ms.clear(); self.total_updates = 0; }

    pub fn slow_updates(&self, threshold_ms: u64) -> usize {
        self.update_times_ms.iter().filter(|&&t| t > threshold_ms).count()
    }
}

impl Default for DecorationPerformanceTracker { fn default() -> Self { Self::new() } }

impl fmt::Display for DecorationPerformanceTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PerfTracker({} updates)", self.total_updates)
    }
}

// ---------------------------------------------------------------------------
// DecorationBulkUpdater
// ---------------------------------------------------------------------------

pub struct DecorationBulkUpdater {
    pending: Vec<(String, String, Vec<DecorationRange>)>,
}

impl DecorationBulkUpdater {
    pub fn new() -> Self { Self { pending: Vec::new() } }

    pub fn queue(&mut self, type_id: impl Into<String>, uri: impl Into<String>, ranges: Vec<DecorationRange>) {
        self.pending.push((type_id.into(), uri.into(), ranges));
    }

    pub fn apply(&mut self, service: &mut DecorationService) {
        for (type_id, uri, ranges) in self.pending.drain(..) {
            service.set_decorations(type_id, uri, ranges);
        }
    }

    pub fn pending_count(&self) -> usize { self.pending.len() }
    pub fn is_empty(&self) -> bool { self.pending.is_empty() }
    pub fn clear(&mut self) { self.pending.clear(); }
}

impl Default for DecorationBulkUpdater { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// DecorationColorTheme
// ---------------------------------------------------------------------------

pub struct DecorationColorTheme {
    colors: std::collections::HashMap<String, String>,
}

impl DecorationColorTheme {
    pub fn new() -> Self { Self { colors: std::collections::HashMap::new() } }

    pub fn set_color(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.colors.insert(key.into(), value.into());
    }

    pub fn get_color(&self, key: &str) -> Option<&str> {
        self.colors.get(key).map(|s| s.as_str())
    }

    pub fn with_defaults() -> Self {
        let mut theme = Self::new();
        theme.set_color("error", "#ff0000");
        theme.set_color("warning", "#ffaa00");
        theme.set_color("info", "#0088ff");
        theme.set_color("hint", "#888888");
        theme
    }

    pub fn len(&self) -> usize { self.colors.len() }
    pub fn is_empty(&self) -> bool { self.colors.is_empty() }
}

impl Default for DecorationColorTheme { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_type(id: &str) -> DecorationType {
        DecorationType {
            id: id.to_string(),
            background_color: None,
            border: None,
            outline: None,
            gutter_icon: None,
            is_whole_line: false,
            after_text: None,
            before_text: None,
        }
    }

    fn sample_range(line: u32) -> DecorationRange {
        DecorationRange {
            start_line: line,
            start_col: 0,
            end_line: line,
            end_col: 10,
            hover_message: None,
        }
    }

    fn sample_range_cols(line: u32, start: u32, end: u32) -> DecorationRange {
        DecorationRange {
            start_line: line,
            start_col: start,
            end_line: line,
            end_col: end,
            hover_message: None,
        }
    }

    #[test]
    fn register_and_set_decorations() {
        let mut svc = DecorationService::new();
        svc.register_type(sample_type("highlight"));
        svc.set_decorations(
            "highlight".into(),
            "file:///a.rs".into(),
            vec![sample_range(1), sample_range(5)],
        );

        let decs = svc.get_decorations("file:///a.rs");
        assert_eq!(decs.len(), 1);
        assert_eq!(decs[0].ranges.len(), 2);
        assert_eq!(decs[0].ranges[0].start_line, 1);
    }

    #[test]
    fn remove_decorations() {
        let mut svc = DecorationService::new();
        svc.set_decorations(
            "err".into(),
            "file:///b.rs".into(),
            vec![sample_range(3)],
        );
        assert_eq!(svc.get_decorations("file:///b.rs").len(), 1);

        svc.remove_decorations("err", "file:///b.rs");
        assert!(svc.get_decorations("file:///b.rs").is_empty());
    }

    #[test]
    fn clear_all() {
        let mut svc = DecorationService::new();
        svc.register_type(sample_type("a"));
        svc.register_type(sample_type("b"));
        svc.set_decorations("a".into(), "file:///x.rs".into(), vec![sample_range(1)]);
        svc.set_decorations("b".into(), "file:///y.rs".into(), vec![sample_range(2)]);

        svc.clear_all();
        assert!(svc.types.is_empty());
        assert!(svc.sets.is_empty());
    }

    #[test]
    fn set_decorations_replaces_existing() {
        let mut svc = DecorationService::new();
        svc.set_decorations("t".into(), "file:///c.rs".into(), vec![sample_range(1)]);
        svc.set_decorations("t".into(), "file:///c.rs".into(), vec![sample_range(9)]);

        let decs = svc.get_decorations("file:///c.rs");
        assert_eq!(decs.len(), 1);
        assert_eq!(decs[0].ranges[0].start_line, 9);
    }

    #[test]
    fn get_type_and_has_type() {
        let mut svc = DecorationService::new();
        svc.register_type(sample_type("err"));
        assert!(svc.has_type("err"));
        assert!(!svc.has_type("warn"));
        assert_eq!(svc.get_type("err").unwrap().id, "err");
        assert!(svc.get_type("warn").is_none());
    }

    #[test]
    fn unregister_type_removes_sets() {
        let mut svc = DecorationService::new();
        svc.register_type(sample_type("err"));
        svc.set_decorations("err".into(), "file:///a.rs".into(), vec![sample_range(1)]);
        assert!(svc.unregister_type("err"));
        assert!(!svc.has_type("err"));
        assert!(svc.get_decorations("file:///a.rs").is_empty());
        assert!(!svc.unregister_type("err"));
    }

    #[test]
    fn get_all_uris_deduplicates() {
        let mut svc = DecorationService::new();
        svc.set_decorations("a".into(), "file:///x.rs".into(), vec![sample_range(1)]);
        svc.set_decorations("b".into(), "file:///x.rs".into(), vec![sample_range(2)]);
        svc.set_decorations("a".into(), "file:///y.rs".into(), vec![sample_range(3)]);
        let uris = svc.get_all_uris();
        assert_eq!(uris.len(), 2);
        assert!(uris.contains(&"file:///x.rs"));
        assert!(uris.contains(&"file:///y.rs"));
    }

    #[test]
    fn decoration_count_sums_ranges() {
        let mut svc = DecorationService::new();
        svc.set_decorations("a".into(), "file:///a.rs".into(), vec![sample_range(1), sample_range(2)]);
        svc.set_decorations("b".into(), "file:///b.rs".into(), vec![sample_range(3)]);
        assert_eq!(svc.decoration_count(), 3);
    }

    #[test]
    fn merge_ranges_combines_overlapping() {
        let ranges = vec![
            sample_range_cols(1, 0, 5),
            sample_range_cols(1, 3, 8),
            sample_range_cols(1, 10, 15),
        ];
        let merged = merge_ranges(ranges);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].start_col, 0);
        assert_eq!(merged[0].end_col, 8);
        assert_eq!(merged[1].start_col, 10);
    }

    #[test]
    fn merge_ranges_empty() {
        let merged = merge_ranges(vec![]);
        assert!(merged.is_empty());
    }

    #[test]
    fn decoration_render_options_default() {
        let opts = DecorationRenderOptions::default();
        assert!(opts.background_color.is_none());
        assert_eq!(opts.range_behavior, RangeBehavior::OpenOpen);
        assert!(opts.opacity.is_none());
    }

    #[test]
    fn range_behavior_variants() {
        assert_ne!(RangeBehavior::OpenOpen, RangeBehavior::ClosedClosed);
        assert_ne!(RangeBehavior::OpenClosed, RangeBehavior::ClosedOpen);
    }

    #[test]
    fn decoration_provider_default_filter() {
        struct TestProvider;
        impl DecorationProvider for TestProvider {
            fn provide_decorations(&self, _uri: &str) -> Vec<DecorationRange> {
                vec![sample_range(1)]
            }
        }
        let provider = TestProvider;
        assert!(provider.event_uri_filter().is_none());
        assert_eq!(provider.provide_decorations("file:///a.rs").len(), 1);
    }

    #[test]
    fn test_decoration_priority_ordering() {
        assert!(DecorationPriority::Low < DecorationPriority::Normal);
        assert!(DecorationPriority::Normal < DecorationPriority::High);
        assert!(DecorationPriority::High < DecorationPriority::Critical);
        assert!(DecorationPriority::Low < DecorationPriority::Critical);
    }

    #[test]
    fn test_sort_by_line() {
        let mut ranges = vec![
            sample_range_cols(5, 3, 10),
            sample_range_cols(1, 0, 5),
            sample_range_cols(1, 2, 8),
            sample_range_cols(3, 0, 4),
        ];
        DecorationSorter::sort_by_line(&mut ranges);
        assert_eq!(ranges[0].start_line, 1);
        assert_eq!(ranges[0].start_col, 0);
        assert_eq!(ranges[1].start_line, 1);
        assert_eq!(ranges[1].start_col, 2);
        assert_eq!(ranges[2].start_line, 3);
        assert_eq!(ranges[3].start_line, 5);
    }

    #[test]
    fn test_sort_by_priority() {
        let mut items = vec![
            PrioritizedDecoration {
                range: sample_range(1),
                priority: DecorationPriority::Low,
                type_id: "a".into(),
            },
            PrioritizedDecoration {
                range: sample_range(2),
                priority: DecorationPriority::Critical,
                type_id: "b".into(),
            },
            PrioritizedDecoration {
                range: sample_range(3),
                priority: DecorationPriority::Normal,
                type_id: "c".into(),
            },
        ];
        DecorationSorter::sort_by_priority(&mut items);
        assert_eq!(items[0].priority, DecorationPriority::Critical);
        assert_eq!(items[1].priority, DecorationPriority::Normal);
        assert_eq!(items[2].priority, DecorationPriority::Low);
    }

    #[test]
    fn test_filter_by_line_range() {
        let ranges = vec![
            sample_range(1),
            sample_range(5),
            sample_range(10),
            sample_range(15),
        ];
        let filtered = DecorationSorter::filter_by_line_range(&ranges, 4, 11);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].start_line, 5);
        assert_eq!(filtered[1].start_line, 10);
    }

    #[test]
    fn test_count_by_line() {
        let ranges = vec![
            sample_range(1),
            sample_range(1),
            sample_range(3),
        ];
        let counts = DecorationSorter::count_by_line(&ranges);
        assert_eq!(counts[&1], 2);
        assert_eq!(counts[&3], 1);
        assert!(!counts.contains_key(&2));
    }

    #[test]
    fn test_decoration_range_display() {
        let r = DecorationRange {
            start_line: 10,
            start_col: 5,
            end_line: 12,
            end_col: 20,
            hover_message: None,
        };
        assert_eq!(format!("{}", r), "L10:5-L12:20");
    }

    #[test]
    fn test_decoration_type_display() {
        let dt = sample_type("error");
        assert_eq!(format!("{}", dt), "DecorationType(error)");
    }

    #[test]
    fn test_get_types() {
        let mut svc = DecorationService::new();
        svc.register_type(sample_type("a"));
        svc.register_type(sample_type("b"));
        let types = svc.get_types();
        assert_eq!(types.len(), 2);
        assert_eq!(types[0].id, "a");
        assert_eq!(types[1].id, "b");
    }

    #[test]
    fn test_type_count_and_set_count() {
        let mut svc = DecorationService::new();
        assert_eq!(svc.type_count(), 0);
        assert_eq!(svc.set_count(), 0);
        svc.register_type(sample_type("x"));
        svc.register_type(sample_type("y"));
        svc.set_decorations("x".into(), "file:///a.rs".into(), vec![sample_range(1)]);
        assert_eq!(svc.type_count(), 2);
        assert_eq!(svc.set_count(), 1);
    }

    #[test]
    fn test_get_decorations_by_type() {
        let mut svc = DecorationService::new();
        svc.set_decorations("err".into(), "file:///a.rs".into(), vec![sample_range(1)]);
        svc.set_decorations("warn".into(), "file:///a.rs".into(), vec![sample_range(2)]);
        svc.set_decorations("err".into(), "file:///b.rs".into(), vec![sample_range(3)]);
        let err_sets = svc.get_decorations_by_type("err");
        assert_eq!(err_sets.len(), 2);
        let warn_sets = svc.get_decorations_by_type("warn");
        assert_eq!(warn_sets.len(), 1);
    }

    #[test]
    fn test_remove_all_for_uri() {
        let mut svc = DecorationService::new();
        svc.set_decorations("a".into(), "file:///x.rs".into(), vec![sample_range(1)]);
        svc.set_decorations("b".into(), "file:///x.rs".into(), vec![sample_range(2)]);
        svc.set_decorations("a".into(), "file:///y.rs".into(), vec![sample_range(3)]);
        svc.remove_all_for_uri("file:///x.rs");
        assert!(svc.get_decorations("file:///x.rs").is_empty());
        assert_eq!(svc.get_decorations("file:///y.rs").len(), 1);
    }

    #[test]
    fn test_has_decorations() {
        let mut svc = DecorationService::new();
        assert!(!svc.has_decorations("file:///a.rs"));
        svc.set_decorations("t".into(), "file:///a.rs".into(), vec![sample_range(1)]);
        assert!(svc.has_decorations("file:///a.rs"));
        assert!(!svc.has_decorations("file:///b.rs"));
    }

    #[test]
    fn test_decoration_range_partial_eq() {
        let r1 = sample_range_cols(1, 0, 10);
        let r2 = sample_range_cols(1, 0, 10);
        let r3 = sample_range_cols(2, 0, 10);
        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
    }

    #[test]
    fn wb_decorations_stats_new_defaults() {
        let stats = WbDecorationsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_decorations_stats_record_success() {
        let mut stats = WbDecorationsStats::new();
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
    fn wb_decorations_stats_record_failure() {
        let mut stats = WbDecorationsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_decorations_stats_reset() {
        let mut stats = WbDecorationsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_decorations_stats_merge() {
        let mut a = WbDecorationsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbDecorationsStats::new();
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
    fn wb_decorations_stats_display() {
        let mut stats = WbDecorationsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_decorations_stats_default() {
        let stats = WbDecorationsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_decorations_validator_accepts_valid_name() {
        let v = WbDecorationsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_decorations_validator_rejects_empty() {
        let v = WbDecorationsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_decorations_validator_rejects_too_long() {
        let v = WbDecorationsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_decorations_validator_forbidden_prefix() {
        let v = WbDecorationsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_decorations_validator_allowed_chars() {
        let v = WbDecorationsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_decorations_validator_range() {
        let v = WbDecorationsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_decorations_sanitize_removes_control() {
        let result = WbDecorationsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_decorations_truncate_short_string() {
        assert_eq!(WbDecorationsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_decorations_truncate_long_string() {
        let result = WbDecorationsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_decorations_is_ascii_printable() {
        assert!(WbDecorationsValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbDecorationsValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- gutter indicator tests ---------------------------------------------

    #[test]
    fn gutter_service_set_and_get() {
        let mut svc = GutterIndicatorService::new();
        svc.set_decorations("file:///a.rs", vec![
            GutterDecoration {
                line: 5,
                icon: None,
                color: Some("red".into()),
                tooltip: Some("breakpoint".into()),
                kind: GutterIndicatorKind::Breakpoint,
            },
        ]);
        assert_eq!(svc.get_decorations("file:///a.rs").len(), 1);
        assert_eq!(svc.get_decorations("file:///b.rs").len(), 0);
    }

    #[test]
    fn gutter_service_decorations_for_line() {
        let mut svc = GutterIndicatorService::new();
        svc.set_decorations("f", vec![
            GutterDecoration {
                line: 1,
                icon: None,
                color: None,
                tooltip: None,
                kind: GutterIndicatorKind::Error,
            },
            GutterDecoration {
                line: 2,
                icon: None,
                color: None,
                tooltip: None,
                kind: GutterIndicatorKind::Warning,
            },
            GutterDecoration {
                line: 1,
                icon: None,
                color: None,
                tooltip: None,
                kind: GutterIndicatorKind::GitAdd,
            },
        ]);
        assert_eq!(svc.decorations_for_line("f", 1).len(), 2);
        assert_eq!(svc.decorations_for_line("f", 2).len(), 1);
        assert_eq!(svc.decorations_for_line("f", 3).len(), 0);
    }

    #[test]
    fn gutter_service_clear() {
        let mut svc = GutterIndicatorService::new();
        svc.set_decorations("f", vec![GutterDecoration {
            line: 1,
            icon: None,
            color: None,
            tooltip: None,
            kind: GutterIndicatorKind::Custom,
        }]);
        assert_eq!(svc.total_count(), 1);
        svc.clear("f");
        assert_eq!(svc.total_count(), 0);
    }

    #[test]
    fn gutter_service_replace_on_set() {
        let mut svc = GutterIndicatorService::new();
        svc.set_decorations("f", vec![GutterDecoration {
            line: 1,
            icon: None,
            color: None,
            tooltip: None,
            kind: GutterIndicatorKind::Error,
        }]);
        svc.set_decorations("f", vec![
            GutterDecoration {
                line: 2,
                icon: None,
                color: None,
                tooltip: None,
                kind: GutterIndicatorKind::Warning,
            },
            GutterDecoration {
                line: 3,
                icon: None,
                color: None,
                tooltip: None,
                kind: GutterIndicatorKind::GitModify,
            },
        ]);
        assert_eq!(svc.total_count(), 2);
        assert_eq!(svc.get_decorations("f").len(), 2);
    }

    #[test]
    fn gutter_indicator_kind_display() {
        assert_eq!(format!("{}", GutterIndicatorKind::Breakpoint), "breakpoint");
        assert_eq!(format!("{}", GutterIndicatorKind::GitDelete), "git-delete");
    }

    #[test]
    fn gutter_decoration_display() {
        let dec = GutterDecoration {
            line: 42,
            icon: None,
            color: None,
            tooltip: None,
            kind: GutterIndicatorKind::Error,
        };
        assert_eq!(format!("{dec}"), "L42 [error]");
    }

    #[test]
    fn test_decoration_range_single_line() {
        let r = DecorationRange::single_line(5, 0, 10);
        assert_eq!(r.line_span(), 1);
        assert!(!r.is_multiline());
    }

    #[test]
    fn test_decoration_range_whole_line() {
        let r = DecorationRange::whole_line(3);
        assert_eq!(r.start_line, 3);
        assert_eq!(r.end_col, u32::MAX);
    }

    #[test]
    fn test_decoration_range_with_hover() {
        let r = DecorationRange::single_line(1, 0, 5).with_hover("hint");
        assert_eq!(r.hover_message.as_deref(), Some("hint"));
    }

    #[test]
    fn test_decoration_range_contains_position() {
        let r = DecorationRange { start_line: 1, start_col: 5, end_line: 3, end_col: 10, hover_message: None };
        assert!(r.contains_position(2, 0));
        assert!(r.contains_position(1, 5));
        assert!(!r.contains_position(0, 0));
        assert!(!r.contains_position(3, 11));
    }

    #[test]
    fn test_decoration_range_overlaps() {
        let a = DecorationRange::single_line(5, 0, 10);
        let b = DecorationRange::single_line(5, 5, 15);
        let c = DecorationRange::single_line(6, 0, 5);
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn test_decoration_type_builder() {
        let dt = DecorationType::new("test")
            .with_background("#FF0000")
            .whole_line()
            .with_after_text("suffix");
        assert_eq!(dt.id, "test");
        assert_eq!(dt.background_color.as_deref(), Some("#FF0000"));
        assert!(dt.is_whole_line);
        assert_eq!(dt.after_text.as_deref(), Some("suffix"));
    }

    #[test]
    fn test_gutter_indicator_kind_all() {
        assert_eq!(GutterIndicatorKind::all().len(), 7);
    }

    #[test]
    fn test_gutter_indicator_kind_is_diff_diagnostic() {
        assert!(GutterIndicatorKind::GitAdd.is_diff());
        assert!(!GutterIndicatorKind::Error.is_diff());
        assert!(GutterIndicatorKind::Error.is_diagnostic());
        assert!(!GutterIndicatorKind::GitAdd.is_diagnostic());
    }

    #[test]
    fn test_count_by_type() {
        let svc = DecorationService {
            types: vec![],
            sets: vec![
                DecorationSet {
                    type_id: "highlight".into(),
                    uri: "file.rs".into(),
                    ranges: vec![DecorationRange::single_line(1, 0, 5), DecorationRange::single_line(2, 0, 5)],
                },
            ],
        };
        let counts = count_by_type(&svc);
        assert_eq!(counts["highlight"], 2);
    }

    #[test]
    fn test_decorations_at_line() {
        let svc = DecorationService {
            types: vec![],
            sets: vec![
                DecorationSet {
                    type_id: "hl".into(),
                    uri: "f.rs".into(),
                    ranges: vec![DecorationRange::single_line(5, 0, 10)],
                },
            ],
        };
        assert_eq!(decorations_at_line(&svc, 5).len(), 1);
        assert_eq!(decorations_at_line(&svc, 6).len(), 0);
    }

    // -- style merging tests ------------------------------------------------

    #[test]
    fn test_resolved_style_merge_base_does_not_override() {
        let mut style = ResolvedStyle {
            background_color: Some("red".into()),
            ..Default::default()
        };
        let opts = DecorationRenderOptions {
            background_color: Some("blue".into()),
            font_weight: Some("bold".into()),
            ..Default::default()
        };
        style.merge_base(&opts);
        // background_color already set, should keep "red"
        assert_eq!(style.background_color.as_deref(), Some("red"));
        // font_weight was None, should be filled
        assert_eq!(style.font_weight.as_deref(), Some("bold"));
    }

    #[test]
    fn test_resolved_style_merge_override_wins() {
        let mut style = ResolvedStyle {
            background_color: Some("red".into()),
            ..Default::default()
        };
        let opts = DecorationRenderOptions {
            background_color: Some("blue".into()),
            opacity: Some(0.5),
            ..Default::default()
        };
        style.merge_override(&opts);
        assert_eq!(style.background_color.as_deref(), Some("blue"));
        assert_eq!(style.opacity, Some(0.5));
    }

    #[test]
    fn test_merge_render_options_layered() {
        let base = DecorationRenderOptions {
            background_color: Some("white".into()),
            border_color: Some("gray".into()),
            ..Default::default()
        };
        let overlay = DecorationRenderOptions {
            background_color: Some("yellow".into()),
            font_style: Some("italic".into()),
            ..Default::default()
        };
        let resolved = merge_render_options(&[base, overlay]);
        assert_eq!(resolved.background_color.as_deref(), Some("yellow"));
        assert_eq!(resolved.border_color.as_deref(), Some("gray"));
        assert_eq!(resolved.font_style.as_deref(), Some("italic"));
    }

    // -- inheritance tests --------------------------------------------------

    #[test]
    fn test_decoration_node_inherits_parent_style() {
        let parent = DecorationNode::new(
            "parent",
            DecorationRenderOptions {
                background_color: Some("red".into()),
                font_weight: Some("bold".into()),
                ..Default::default()
            },
        );
        let child = DecorationNode::new(
            "child",
            DecorationRenderOptions {
                background_color: Some("blue".into()),
                ..Default::default()
            },
        );
        let root_style = ResolvedStyle::default();
        let parent_resolved = parent.resolve(&root_style);
        let child_resolved = child.resolve(&parent_resolved);
        // child overrides background
        assert_eq!(child_resolved.background_color.as_deref(), Some("blue"));
        // child inherits font_weight from parent
        assert_eq!(child_resolved.font_weight.as_deref(), Some("bold"));
    }

    #[test]
    fn test_decoration_node_resolve_tree() {
        let mut root = DecorationNode::new(
            "root",
            DecorationRenderOptions {
                background_color: Some("white".into()),
                font_weight: Some("normal".into()),
                ..Default::default()
            },
        );
        let mut mid = DecorationNode::new(
            "mid",
            DecorationRenderOptions {
                font_weight: Some("bold".into()),
                ..Default::default()
            },
        );
        let leaf = DecorationNode::new(
            "leaf",
            DecorationRenderOptions {
                background_color: Some("green".into()),
                ..Default::default()
            },
        );
        mid.add_child(leaf);
        root.add_child(mid);

        let styles = root.resolve_tree(&ResolvedStyle::default());
        assert_eq!(styles.len(), 3);
        // root
        assert_eq!(styles[0].0, "root");
        assert_eq!(styles[0].1.background_color.as_deref(), Some("white"));
        // mid inherits background from root, overrides font_weight
        assert_eq!(styles[1].0, "mid");
        assert_eq!(styles[1].1.background_color.as_deref(), Some("white"));
        assert_eq!(styles[1].1.font_weight.as_deref(), Some("bold"));
        // leaf overrides background, inherits font_weight from mid
        assert_eq!(styles[2].0, "leaf");
        assert_eq!(styles[2].1.background_color.as_deref(), Some("green"));
        assert_eq!(styles[2].1.font_weight.as_deref(), Some("bold"));
    }

    // -- batch update tests -------------------------------------------------

    #[test]
    fn test_batch_update_detects_no_op() {
        let mut svc = DecorationService::new();
        svc.set_decorations("t".into(), "f.rs".into(), vec![sample_range(1)]);

        let mut batch = BatchDecorationUpdate::new();
        // Same data – should be a no-op
        batch.set("t", "f.rs", vec![sample_range(1)]);
        let effective = batch.apply(&mut svc);
        assert!(effective.is_empty());
    }

    #[test]
    fn test_batch_update_applies_real_changes() {
        let mut svc = DecorationService::new();
        let mut batch = BatchDecorationUpdate::new();
        batch.set("err", "a.rs", vec![sample_range(1)]);
        batch.set("warn", "a.rs", vec![sample_range(2)]);
        assert_eq!(batch.len(), 2);

        let effective = batch.apply(&mut svc);
        assert_eq!(effective.len(), 2);
        assert_eq!(svc.decoration_count(), 2);
    }

    #[test]
    fn test_batch_update_remove_noop_and_real() {
        let mut svc = DecorationService::new();
        svc.set_decorations("t".into(), "f.rs".into(), vec![sample_range(5)]);

        let mut batch = BatchDecorationUpdate::new();
        batch.remove("nonexistent", "f.rs");
        batch.remove("t", "f.rs");
        let effective = batch.apply(&mut svc);
        assert_eq!(effective.len(), 1);
        assert!(svc.get_decorations("f.rs").is_empty());
    }

    // -- source filtering tests ---------------------------------------------

    fn make_sourced(
        type_id: &str,
        uri: &str,
        source: DecorationSource,
        priority: DecorationPriority,
    ) -> SourcedDecorationSet {
        SourcedDecorationSet {
            set: DecorationSet {
                type_id: type_id.into(),
                uri: uri.into(),
                ranges: vec![sample_range(1)],
            },
            source,
            priority,
        }
    }

    #[test]
    fn test_filter_by_source() {
        let sets = vec![
            make_sourced("a", "f.rs", DecorationSource::BuiltIn, DecorationPriority::Normal),
            make_sourced("b", "f.rs", DecorationSource::Debugger, DecorationPriority::High),
            make_sourced("c", "g.rs", DecorationSource::BuiltIn, DecorationPriority::Low),
        ];
        let filtered = DecorationFilter::by_source(&sets, &DecorationSource::BuiltIn);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_by_min_priority() {
        let sets = vec![
            make_sourced("a", "f.rs", DecorationSource::BuiltIn, DecorationPriority::Low),
            make_sourced("b", "f.rs", DecorationSource::Debugger, DecorationPriority::High),
            make_sourced("c", "g.rs", DecorationSource::BuiltIn, DecorationPriority::Critical),
        ];
        let filtered = DecorationFilter::by_min_priority(&sets, DecorationPriority::High);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|s| s.priority >= DecorationPriority::High));
    }

    #[test]
    fn test_filter_query_combined() {
        let sets = vec![
            make_sourced("a", "f.rs", DecorationSource::BuiltIn, DecorationPriority::Normal),
            make_sourced("b", "f.rs", DecorationSource::Debugger, DecorationPriority::High),
            make_sourced("c", "g.rs", DecorationSource::BuiltIn, DecorationPriority::High),
        ];
        let filtered = DecorationFilter::query(
            &sets,
            Some(&DecorationSource::BuiltIn),
            Some(DecorationPriority::High),
            None,
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].set.uri, "g.rs");
    }

    #[test]
    fn test_decoration_source_display() {
        assert_eq!(DecorationSource::BuiltIn.to_string(), "built-in");
        assert_eq!(
            DecorationSource::Extension("rust-analyzer".into()).to_string(),
            "extension:rust-analyzer"
        );
        assert_eq!(DecorationSource::LanguageServer.to_string(), "language-server");
        assert_eq!(DecorationSource::Debugger.to_string(), "debugger");
    }


    #[test]
    fn layer_order_compare() {
        assert!(DecorationLayerOrder::is_higher(DecorationPriority::High, DecorationPriority::Low));
        assert!(!DecorationLayerOrder::is_higher(DecorationPriority::Low, DecorationPriority::High));
    }

    #[test]
    fn perf_tracker_basic() {
        let mut t = DecorationPerformanceTracker::new();
        t.record_update(10);
        t.record_update(20);
        t.record_update(30);
        assert_eq!(t.average_update_ms(), Some(20));
        assert_eq!(t.max_update_ms(), Some(30));
        assert_eq!(t.total_updates(), 3);
    }

    #[test]
    fn perf_tracker_slow() {
        let mut t = DecorationPerformanceTracker::new();
        t.record_update(5);
        t.record_update(50);
        t.record_update(100);
        assert_eq!(t.slow_updates(20), 2);
    }

    #[test]
    fn perf_tracker_reset() {
        let mut t = DecorationPerformanceTracker::new();
        t.record_update(10);
        t.reset();
        assert_eq!(t.total_updates(), 0);
        assert_eq!(t.average_update_ms(), None);
    }

    #[test]
    fn bulk_updater_basic() {
        let mut updater = DecorationBulkUpdater::new();
        updater.queue("type1", "file://a.rs", vec![]);
        assert_eq!(updater.pending_count(), 1);
        let mut svc = DecorationService::new();
        svc.register_type(sample_type("type1"));
        updater.apply(&mut svc);
        assert!(updater.is_empty());
    }

    #[test]
    fn bulk_updater_clear() {
        let mut u = DecorationBulkUpdater::new();
        u.queue("t", "u", vec![]);
        u.clear();
        assert!(u.is_empty());
    }

    #[test]
    fn color_theme_defaults() {
        let theme = DecorationColorTheme::with_defaults();
        assert_eq!(theme.get_color("error"), Some("#ff0000"));
        assert!(theme.len() >= 4);
    }

    #[test]
    fn color_theme_custom() {
        let mut theme = DecorationColorTheme::new();
        theme.set_color("custom", "#123456");
        assert_eq!(theme.get_color("custom"), Some("#123456"));
    }

    #[test]
    fn perf_tracker_display() {
        let t = DecorationPerformanceTracker::new();
        assert!(format!("{t}").contains("0 updates"));
    }

    #[test]
    fn layer_order_equal() {
        assert!(!DecorationLayerOrder::is_higher(DecorationPriority::Normal, DecorationPriority::Normal));
    }

    #[test]
    fn color_theme_missing() {
        let theme = DecorationColorTheme::new();
        assert_eq!(theme.get_color("nonexistent"), None);
    }

}
