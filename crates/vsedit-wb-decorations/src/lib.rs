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

use std::collections::HashMap;
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


// ---------------------------------------------------------------------------
// DecorationAnimFrame
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DecorationAnimFrame {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl DecorationAnimFrame {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for DecorationAnimFrame {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for DecorationAnimFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "DecorationAnimFrame({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// DecorationMergeOptimizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DecorationMergeOptimizer {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl DecorationMergeOptimizer {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for DecorationMergeOptimizer {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for DecorationMergeOptimizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "DecorationMergeOptimizer({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// DecorationAnimFrameSnapshot — point-in-time snapshot of DecorationAnimFrame state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DecorationAnimFrameSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl DecorationAnimFrameSnapshot {
    pub fn capture(source: &DecorationAnimFrame, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for DecorationAnimFrameSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// DecorationMergeOptimizerStats — aggregate statistics for DecorationMergeOptimizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct DecorationMergeOptimizerStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl DecorationMergeOptimizerStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for DecorationMergeOptimizerStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// DecorationAnimFrameConfig — configuration for DecorationAnimFrame
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DecorationAnimFrameConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl DecorationAnimFrameConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for DecorationAnimFrameConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for DecorationAnimFrameConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}


// ─── Decor Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for decoration updates.
#[derive(Debug, Clone)]
pub struct DecorRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> DecorRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for DecorRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DecorRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── Decor Formatter ───────────────────────────────────────

/// Formatting options for decoration output.
#[derive(Debug, Clone)]
pub struct DecorFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for DecorFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl DecorFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for decoration data.
pub struct DecorFmt {
    options: DecorFmtOpts,
}

impl DecorFmt {
    pub fn new(options: DecorFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: DecorFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}


/// Workbench decoration configuration manager.
#[derive(Debug, Clone)]
pub struct WbDecorationsConfig {
    entries: Vec<WbDecorationsEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workbench decoration entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WbDecorationsEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WbDecorationsEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl WbDecorationsConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WbDecorationsEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&WbDecorationsEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WbDecorationsEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WbDecorationsEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&WbDecorationsEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WbDecorationsEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<WbDecorationsEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Workbench resource decorations — extended utilities (xf)
// ---------------------------------------------------------------------------

/// Metric accumulator for wb_deco operations.
#[derive(Debug, Clone)]
pub struct XfMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XfMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for wb_deco.
#[derive(Debug, Clone)]
pub struct XfRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XfRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for wb_deco lookups.
#[derive(Debug, Clone)]
pub struct XfLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XfLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 22
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer22 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer22 {
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
pub fn xb_fnv1a_22(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_22<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_22<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_22(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_22(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 206
// ---------------------------------------------------------------------------

/// Generic object pool `Xc206Pool<T>`.
pub struct Xc206Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc206Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc206PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc206Pool<T> {
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
    pub fn stats(&self) -> Xc206PoolStats {
        Xc206PoolStats {
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

impl<T> Default for Xc206Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc206Scheduler`.
pub struct Xc206Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc206Scheduler {
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

impl Default for Xc206Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_206 hash for the given byte slice.
pub fn xc_206_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_206 convention.
pub fn xc_206_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe34 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe34Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe34PipelineError {
    pub stage: Xe34Stage,
    pub message: String,
}

impl std::fmt::Display for Xe34PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe34Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe34Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe34PipelineError>>>,
    stage_names: Vec<Xe34Stage>,
}

impl Xe34Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe34PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe34Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe34PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe34Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe34PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe34Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe34PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe34Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe34PipelineError> {
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

    pub fn compose(mut self, other: Xe34Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe34CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe34CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe34Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe34CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe34CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe34Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe34CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_34_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe34CacheEntry {
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

    fn xe_34_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe34CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_34_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe34PipelineError> {
    Ok(data)
}

pub fn xe_34_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe34PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_34_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe34PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_34_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe34PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_34_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe34PipelineError> {
    Err(Xe34PipelineError {
        stage: Xe34Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #120
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf120Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf120TrieNode {
    children: std::collections::HashMap<char, Xf120TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf120Trie {
    root: Xf120TrieNode,
    count: usize,
}

impl Xf120Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf120TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf120TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf120TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf120BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf120BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 205).
pub struct Xh205SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh205SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 247 as u64,
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

/// A compact bit set supporting boolean operations (variant 205).
pub struct Xh205BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh205BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 205).
pub struct Xi205Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi205Deque<T> {
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
pub struct Xi205Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi205Interval {
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

/// A simple interval tree (variant 205).
pub struct Xi205IntervalTree {
    xi_intervals: Vec<Xi205Interval>,
}

impl Xi205IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi205Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi205Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi205Interval) -> Vec<&Xi205Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi205Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi205Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi205Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi205Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi205Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi205Interval> = Vec::new();
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


    #[test] fn decorationAnimFrame_new() { let s = DecorationAnimFrame::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn decorationAnimFrame_add() { let mut s = DecorationAnimFrame::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn decorationAnimFrame_remove() { let mut s = DecorationAnimFrame::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn decorationAnimFrame_config() { let mut s = DecorationAnimFrame::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn decorationAnimFrame_nav() { let mut s = DecorationAnimFrame::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn decorationAnimFrame_filter() { let mut s = DecorationAnimFrame::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn decorationAnimFrame_display() { assert!(format!("{}", DecorationAnimFrame::new()).contains("DecorationAnimFrame")); }
    #[test] fn decorationMergeOptimizer_new() { let s = DecorationMergeOptimizer::new(); assert!(s.is_empty()); }
    #[test] fn decorationMergeOptimizer_add() { let mut s = DecorationMergeOptimizer::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn decorationMergeOptimizer_active() { let mut s = DecorationMergeOptimizer::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn decorationMergeOptimizer_error() { let mut s = DecorationMergeOptimizer::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn decorationMergeOptimizer_rm_group() { let mut s = DecorationMergeOptimizer::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn decorationMergeOptimizer_display() { assert!(format!("{}", DecorationMergeOptimizer::new()).contains("DecorationMergeOptimizer")); }


    #[test] fn decorationAnimFrame_snap_capture() {
        let s = DecorationAnimFrame::new();
        let snap = DecorationAnimFrameSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn decorationAnimFrame_snap_stale() {
        let s = DecorationAnimFrame::new();
        let snap = DecorationAnimFrameSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn decorationAnimFrame_snap_diff() {
        let s = DecorationAnimFrame::new();
        let s1v = DecorationAnimFrameSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn decorationAnimFrame_snap_display() {
        let s = DecorationAnimFrame::new();
        let snap = DecorationAnimFrameSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn decorationMergeOptimizer_stats_record() {
        let mut st = DecorationMergeOptimizerStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn decorationMergeOptimizer_stats_hit_ratio() {
        let mut st = DecorationMergeOptimizerStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn decorationMergeOptimizer_stats_merge() {
        let mut a = DecorationMergeOptimizerStats::new();
        a.total_adds = 5;
        let mut b = DecorationMergeOptimizerStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn decorationMergeOptimizer_stats_display() {
        let st = DecorationMergeOptimizerStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn decorationAnimFrame_config_default() {
        let c = DecorationAnimFrameConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn decorationAnimFrame_config_builder() {
        let c = DecorationAnimFrameConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn decorationAnimFrame_config_labels() {
        let mut c = DecorationAnimFrameConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn decorationAnimFrame_config_cleanup_threshold() {
        let c = DecorationAnimFrameConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn decorationAnimFrame_config_display() {
        assert!(format!("{}", DecorationAnimFrameConfig::new()).contains("Config"));
    }
    #[test] fn decorationMergeOptimizer_stats_peaks() {
        let mut st = DecorationMergeOptimizerStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }


    #[test]
    fn decor_ringbuf_push_get() {
        let mut rb = DecorRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn decor_ringbuf_overflow() {
        let mut rb = DecorRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn decor_ringbuf_clear() {
        let mut rb = DecorRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn decor_ringbuf_newest_oldest() {
        let mut rb = DecorRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn decor_ringbuf_to_vec() {
        let mut rb = DecorRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn decor_ringbuf_is_full() {
        let mut rb = DecorRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn decor_fmt_list() {
        let f = DecorFmt::new(DecorFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn decor_fmt_kv() {
        let f = DecorFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn decor_fmt_section() {
        let f = DecorFmt::new(DecorFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn decor_fmt_truncate() {
        let f = DecorFmt::new(DecorFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn decor_fmt_opts_defaults() {
        let o = DecorFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn wb_decorations_entry_creation() {
        let e = WbDecorationsEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wb_decorations_entry_with_priority() {
        let e = WbDecorationsEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wb_decorations_entry_metadata() {
        let e = WbDecorationsEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wb_decorations_entry_remove_meta() {
        let mut e = WbDecorationsEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wb_decorations_entry_activate_deactivate() {
        let mut e = WbDecorationsEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wb_decorations_config_add_sorted() {
        let mut c = WbDecorationsConfig::new(10);
        c.add(WbDecorationsEntry::new("lo", "Lo").with_priority(1));
        c.add(WbDecorationsEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wb_decorations_config_capacity() {
        let mut c = WbDecorationsConfig::new(1);
        assert!(c.add(WbDecorationsEntry::new("a", "A")));
        assert!(!c.add(WbDecorationsEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wb_decorations_config_remove() {
        let mut c = WbDecorationsConfig::new(10);
        c.add(WbDecorationsEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wb_decorations_config_get() {
        let mut c = WbDecorationsConfig::new(10);
        c.add(WbDecorationsEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wb_decorations_config_active_entries() {
        let mut c = WbDecorationsConfig::new(10);
        c.add(WbDecorationsEntry::new("a", "A"));
        c.add(WbDecorationsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wb_decorations_config_enable_disable() {
        let mut c = WbDecorationsConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wb_decorations_config_clear() {
        let mut c = WbDecorationsConfig::new(10);
        c.add(WbDecorationsEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wb_decorations_config_find_by_label() {
        let mut c = WbDecorationsConfig::new(10);
        c.add(WbDecorationsEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wb_decorations_config_top_n() {
        let mut c = WbDecorationsConfig::new(10);
        c.add(WbDecorationsEntry::new("a", "A").with_priority(1));
        c.add(WbDecorationsEntry::new("b", "B").with_priority(2));
        c.add(WbDecorationsEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wb_decorations_config_deactivate_activate_all() {
        let mut c = WbDecorationsConfig::new(10);
        c.add(WbDecorationsEntry::new("a", "A"));
        c.add(WbDecorationsEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wb_decorations_config_highest_priority() {
        let mut c = WbDecorationsConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WbDecorationsEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wb_decorations_config_contains() {
        let mut c = WbDecorationsConfig::new(10);
        c.add(WbDecorationsEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wb_decorations_config_labels() {
        let mut c = WbDecorationsConfig::new(10);
        c.add(WbDecorationsEntry::new("a", "Alpha"));
        c.add(WbDecorationsEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wb_decorations_config_drain_inactive() {
        let mut c = WbDecorationsConfig::new(10);
        c.add(WbDecorationsEntry::new("a", "A"));
        c.add(WbDecorationsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn xf_metrics_empty() {
        let m = XfMetrics::new("wb_deco");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xf_metrics_record_and_mean() {
        let mut m = XfMetrics::new("wb_deco");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xf_metrics_min_max() {
        let mut m = XfMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xf_metrics_variance_and_std() {
        let mut m = XfMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn xf_metrics_percentile() {
        let mut m = XfMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xf_metrics_merge() {
        let mut a = XfMetrics::new("a");
        a.record(1.0);
        let mut b = XfMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xf_metrics_reset() {
        let mut m = XfMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xf_rate_window_empty() {
        let rw = XfRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xf_rate_window_tick_and_rate() {
        let mut rw = XfRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xf_lru_cache_basic() {
        let mut c = XfLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xf_lru_cache_contains_and_keys() {
        let mut c = XfLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xf_lru_cache_remove() {
        let mut c = XfLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xf_metrics_sum() {
        let mut m = XfMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xf_metrics_label() {
        let m = XfMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xf_lru_cache_clear() {
        let mut c = XfLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_22_push_and_len() {
        let mut rb = super::XbRingBuffer22::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_22_overwrite() {
        let mut rb = super::XbRingBuffer22::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_22_get_out_of_bounds() {
        let rb = super::XbRingBuffer22::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_22_drain_all() {
        let mut rb = super::XbRingBuffer22::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_22_peek_front_back() {
        let mut rb = super::XbRingBuffer22::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_22_clear() {
        let mut rb = super::XbRingBuffer22::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_22_capacity() {
        let rb = super::XbRingBuffer22::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_22_basic() {
        let h = super::xb_fnv1a_22(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_22(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_22_different_inputs() {
        let h1 = super::xb_fnv1a_22(b"abc");
        let h2 = super::xb_fnv1a_22(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_22_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_22(&data);
        let dec = super::xb_rle_decode_22(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_22_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_22(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_22(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_22_values() {
        assert!((super::xb_clamp_22(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_22(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_22(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_22_values() {
        assert!((super::xb_lerp_22(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_22(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_22(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_22_wrap_around_twice() {
        let mut rb = super::XbRingBuffer22::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 206 ----

    #[test]
    fn xc_206_pool_new_empty() {
        let pool: super::Xc206Pool<i32> = super::Xc206Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_206_pool_release_acquire() {
        let mut pool = super::Xc206Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_206_pool_acquire_empty() {
        let mut pool: super::Xc206Pool<i32> = super::Xc206Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_206_pool_full() {
        let mut pool = super::Xc206Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_206_pool_drain() {
        let mut pool = super::Xc206Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_206_pool_stats() {
        let mut pool = super::Xc206Pool::new(8);
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
    fn xc_206_pool_clear() {
        let mut pool = super::Xc206Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_206_pool_shrink() {
        let mut pool = super::Xc206Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_206_pool_default() {
        let pool: super::Xc206Pool<String> = super::Xc206Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_206_pool_extend() {
        let mut pool = super::Xc206Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_206_pool_retain() {
        let mut pool = super::Xc206Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_206_scheduler_round_robin() {
        let mut sched = super::Xc206Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_206_scheduler_empty() {
        let mut sched = super::Xc206Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_206_scheduler_reset() {
        let mut sched = super::Xc206Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_206_scheduler_add_remove() {
        let mut sched = super::Xc206Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_206_scheduler_targets() {
        let sched = super::Xc206Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_206_hash_empty() {
        assert_eq!(super::xc_206_hash(b""), 5381);
    }

    #[test]
    fn xc_206_hash_data() {
        let h = super::xc_206_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_206_hash(b"hello"), h);
    }

    #[test]
    fn xc_206_reverse_str() {
        assert_eq!(super::xc_206_reverse("abc"), "cba");
        assert_eq!(super::xc_206_reverse(""), "");
    }


    #[test]
    fn xe_34_pipeline_empty() {
        let p = super::Xe34Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_34_pipeline_parse_stage() {
        let p = super::Xe34Pipeline::new()
            .add_parse(super::xe_34_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_34_pipeline_transform_double() {
        let p = super::Xe34Pipeline::new()
            .add_transform(super::xe_34_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_34_pipeline_validate_reverse() {
        let p = super::Xe34Pipeline::new()
            .add_validate(super::xe_34_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_34_pipeline_emit_filter() {
        let p = super::Xe34Pipeline::new()
            .add_emit(super::xe_34_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_34_pipeline_multi_stage() {
        let p = super::Xe34Pipeline::new()
            .add_parse(super::xe_34_pipeline_identity)
            .add_transform(super::xe_34_pipeline_double)
            .add_validate(super::xe_34_pipeline_reverse)
            .add_emit(super::xe_34_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_34_pipeline_error_propagation() {
        let p = super::Xe34Pipeline::new()
            .add_parse(super::xe_34_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe34Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_34_pipeline_compose() {
        let p1 = super::Xe34Pipeline::new()
            .add_parse(super::xe_34_pipeline_identity);
        let p2 = super::Xe34Pipeline::new()
            .add_transform(super::xe_34_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_34_pipeline_error_display() {
        let e = super::Xe34PipelineError {
            stage: super::Xe34Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_34_cache_put_get() {
        let mut c = super::Xe34Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_34_cache_miss() {
        let mut c: super::Xe34Cache<&str, i32> = super::Xe34Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_34_cache_ttl_expiry() {
        let mut c = super::Xe34Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_34_cache_evict() {
        let mut c = super::Xe34Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_34_cache_capacity() {
        let mut c = super::Xe34Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_34_cache_stats() {
        let mut c = super::Xe34Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_34_cache_clear() {
        let mut c = super::Xe34Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #120 --

    #[test]
    fn xf120_trie_insert_search() {
        let mut t = Xf120Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf120_trie_starts_with() {
        let mut t = Xf120Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf120_trie_remove() {
        let mut t = Xf120Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf120_trie_word_count() {
        let mut t = Xf120Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf120_trie_longest_prefix() {
        let mut t = Xf120Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf120_trie_all_words() {
        let mut t = Xf120Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf120_trie_autocomplete() {
        let mut t = Xf120Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf120_trie_empty_search() {
        let t = Xf120Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf120_bloom_add_contains() {
        let mut bf = Xf120BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf120_bloom_probably_absent() {
        let bf = Xf120BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf120_bloom_false_positive_rate() {
        let mut bf = Xf120BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf120_bloom_clear() {
        let mut bf = Xf120BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf120_bloom_union() {
        let mut a = Xf120BloomFilter::xf_new(512, 2);
        let mut b = Xf120BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf120_bloom_intersection_estimate() {
        let mut a = Xf120BloomFilter::xf_new(512, 2);
        let mut b = Xf120BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf120_bloom_union_size_mismatch() {
        let a = Xf120BloomFilter::xf_new(256, 2);
        let b = Xf120BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh205_skip_insert_contains() {
        let mut sl = super::Xh205SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh205_skip_remove() {
        let mut sl = super::Xh205SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh205_skip_len() {
        let mut sl = super::Xh205SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh205_skip_range_query() {
        let mut sl = super::Xh205SkipList::xh_new(4);
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
    fn xh205_skip_floor_ceiling() {
        let mut sl = super::Xh205SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh205_skip_rank() {
        let mut sl = super::Xh205SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh205_skip_empty() {
        let sl = super::Xh205SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh205_skip_duplicates() {
        let mut sl = super::Xh205SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh205_bitset_set_test() {
        let mut bs = super::Xh205BitSet::xh_new(256);
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
    fn xh205_bitset_clear_count() {
        let mut bs = super::Xh205BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh205_bitset_and_or_xor() {
        let mut a = super::Xh205BitSet::xh_new(128);
        let mut b = super::Xh205BitSet::xh_new(128);
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
    fn xh205_bitset_iter_ones() {
        let mut bs = super::Xh205BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh205_bitset_first_last() {
        let mut bs = super::Xh205BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh205_bitset_empty() {
        let bs = super::Xh205BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi205_deque_push_pop_back() {
        let mut dq = super::Xi205Deque::xi_new(4);
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
    fn xi205_deque_push_pop_front() {
        let mut dq = super::Xi205Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi205_deque_mixed_ops() {
        let mut dq = super::Xi205Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi205_deque_get_and_split() {
        let mut dq = super::Xi205Deque::xi_new(8);
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
    fn xi205_deque_rotate_left() {
        let mut dq = super::Xi205Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi205_deque_rotate_right() {
        let mut dq = super::Xi205Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi205_deque_grow() {
        let mut dq = super::Xi205Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi205_deque_empty() {
        let dq = super::Xi205Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi205_interval_tree_insert_query() {
        let mut tree = super::Xi205IntervalTree::xi_new();
        tree.xi_insert(super::Xi205Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi205Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi205Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi205_interval_tree_overlap() {
        let mut tree = super::Xi205IntervalTree::xi_new();
        tree.xi_insert(super::Xi205Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi205Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi205Interval::xi_new(12, 20));
        let q = super::Xi205Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi205_interval_tree_remove() {
        let mut tree = super::Xi205IntervalTree::xi_new();
        tree.xi_insert(super::Xi205Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi205Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi205_interval_tree_gaps() {
        let mut tree = super::Xi205IntervalTree::xi_new();
        tree.xi_insert(super::Xi205Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi205Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi205Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi205Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi205Interval::xi_new(8, 10));
    }

    #[test]
    fn xi205_interval_tree_merge() {
        let mut tree = super::Xi205IntervalTree::xi_new();
        tree.xi_insert(super::Xi205Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi205Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi205Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi205Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi205Interval::xi_new(10, 15));
    }

    #[test]
    fn xi205_interval_tree_all() {
        let mut tree = super::Xi205IntervalTree::xi_new();
        tree.xi_insert(super::Xi205Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi205Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi205_interval_tree_empty() {
        let tree = super::Xi205IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi205_interval_tree_contains_point() {
        let iv = super::Xi205Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}