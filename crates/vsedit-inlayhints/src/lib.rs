//! Inlay type and parameter annotations for inline editor hints.

use std::collections::HashMap;
use std::fmt;
/// The kind of inlay hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintKind {
    /// A type annotation hint (e.g. `: i32`).
    Type,
    /// A parameter name hint (e.g. `name:`).
    Parameter,
    /// Any other hint.
    Other,
}

/// A single labeled segment of an inlay hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHintLabelPart {
    pub value: String,
    pub tooltip: Option<String>,
    pub command: Option<String>,
}

/// An inlay hint displayed inline in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHint {
    pub position_line: u32,
    pub position_col: u32,
    pub label: Vec<InlayHintLabelPart>,
    pub kind: InlayHintKind,
    pub padding_left: bool,
    pub padding_right: bool,
    pub tooltip: Option<String>,
}

impl InlayHint {
    /// Create a simple hint with a single label part and no tooltip or command.
    pub fn simple(
        position_line: u32,
        position_col: u32,
        text: impl Into<String>,
        kind: InlayHintKind,
    ) -> Self {
        Self {
            position_line,
            position_col,
            label: vec![InlayHintLabelPart {
                value: text.into(),
                tooltip: None,
                command: None,
            }],
            kind,
            padding_left: false,
            padding_right: false,
            tooltip: None,
        }
    }
}

/// Trait for types that can provide inlay hints for a document region.
pub trait InlayHintsProvider {
    /// Return inlay hints for the given URI within the line range `[start_line, end_line]`.
    fn provide_inlay_hints(
        &self,
        uri: &str,
        start_line: u32,
        end_line: u32,
    ) -> Vec<InlayHint>;
}

impl std::fmt::Display for InlayHintKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InlayHintKind::Type => write!(f, "Type"),
            InlayHintKind::Parameter => write!(f, "Parameter"),
            InlayHintKind::Other => write!(f, "Other"),
        }
    }
}

impl std::fmt::Display for InlayHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for part in &self.label {
            write!(f, "{}", part.value)?;
        }
        Ok(())
    }
}

/// Errors that can occur when constructing or resolving inlay hints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlayHintError {
    /// The specified position is invalid (e.g. out of document bounds).
    InvalidPosition { line: u32, col: u32 },
    /// The hint label is empty; at least one label part is required.
    EmptyLabel,
    /// No provider was found with the given name.
    ProviderNotFound(String),
}

impl std::fmt::Display for InlayHintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InlayHintError::InvalidPosition { line, col } => {
                write!(f, "invalid position: line {line}, col {col}")
            }
            InlayHintError::EmptyLabel => write!(f, "hint label must not be empty"),
            InlayHintError::ProviderNotFound(name) => {
                write!(f, "provider not found: {name}")
            }
        }
    }
}

impl std::error::Error for InlayHintError {}

/// Builder for constructing [`InlayHint`] instances step-by-step.
#[derive(Debug, Clone)]
pub struct InlayHintBuilder {
    position_line: Option<u32>,
    position_col: Option<u32>,
    label: Vec<InlayHintLabelPart>,
    kind: InlayHintKind,
    padding_left: bool,
    padding_right: bool,
    tooltip: Option<String>,
}

impl InlayHintBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self {
            position_line: None,
            position_col: None,
            label: Vec::new(),
            kind: InlayHintKind::Other,
            padding_left: false,
            padding_right: false,
            tooltip: None,
        }
    }

    /// Set the position (line and column) of the hint.
    pub fn position(mut self, line: u32, col: u32) -> Self {
        self.position_line = Some(line);
        self.position_col = Some(col);
        self
    }

    /// Append a label part to the hint.
    pub fn add_label_part(mut self, value: impl Into<String>) -> Self {
        self.label.push(InlayHintLabelPart {
            value: value.into(),
            tooltip: None,
            command: None,
        });
        self
    }

    /// Set the kind of hint.
    pub fn kind(mut self, kind: InlayHintKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set left and right padding.
    pub fn padding(mut self, left: bool, right: bool) -> Self {
        self.padding_left = left;
        self.padding_right = right;
        self
    }

    /// Set the tooltip for the hint.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Build the [`InlayHint`], returning an error if required fields are missing.
    pub fn build(self) -> Result<InlayHint, InlayHintError> {
        let line = self.position_line.ok_or(InlayHintError::InvalidPosition {
            line: u32::MAX,
            col: u32::MAX,
        })?;
        let col = self.position_col.ok_or(InlayHintError::InvalidPosition {
            line,
            col: u32::MAX,
        })?;
        if self.label.is_empty() {
            return Err(InlayHintError::EmptyLabel);
        }
        Ok(InlayHint {
            position_line: line,
            position_col: col,
            label: self.label,
            kind: self.kind,
            padding_left: self.padding_left,
            padding_right: self.padding_right,
            tooltip: self.tooltip,
        })
    }
}

impl Default for InlayHintBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Data attached to a hint for lazy resolution (e.g. deferred tooltip or command).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHintResolveData {
    /// An identifier the server can use to look up additional information.
    pub resolve_id: String,
    /// Additional tooltip text loaded on demand.
    pub tooltip: Option<String>,
    /// Command identifier to execute when the hint is clicked.
    pub command_id: Option<String>,
    /// Human-readable command title.
    pub command_title: Option<String>,
}

impl InlayHintResolveData {
    /// Create resolve data with only an identifier.
    pub fn new(resolve_id: impl Into<String>) -> Self {
        Self {
            resolve_id: resolve_id.into(),
            tooltip: None,
            command_id: None,
            command_title: None,
        }
    }

    /// Attach a lazily-resolved tooltip.
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Attach a command.
    pub fn with_command(mut self, id: impl Into<String>, title: impl Into<String>) -> Self {
        self.command_id = Some(id.into());
        self.command_title = Some(title.into());
        self
    }
}

impl InlayHint {
    /// Merge adjacent hints that share the same line into a single hint.
    ///
    /// Hints are considered adjacent when they are on the same line.  The
    /// merged hint keeps the position of the first hint in the group, and
    /// all label parts are concatenated in order.  The kind is taken from
    /// the first hint.
    pub fn merge_adjacent(mut hints: Vec<InlayHint>) -> Vec<InlayHint> {
        if hints.len() <= 1 {
            return hints;
        }
        hints.sort_by(|a, b| {
            a.position_line
                .cmp(&b.position_line)
                .then(a.position_col.cmp(&b.position_col))
        });

        let mut merged: Vec<InlayHint> = Vec::new();
        for hint in hints {
            if let Some(last) = merged.last_mut() {
                if last.position_line == hint.position_line {
                    last.label.extend(hint.label);
                    continue;
                }
            }
            merged.push(hint);
        }
        merged
    }
}

/// Configuration for inlay hints display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHintsConfig {
    pub enabled: bool,
    pub font_size: Option<u32>,
    pub font_family: Option<String>,
    /// Maximum display length (in characters) for a single hint label.
    pub max_length: Option<u32>,
}

impl Default for InlayHintsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            font_size: None,
            font_family: None,
            max_length: None,
        }
    }
}

/// A registry that stores multiple named [`InlayHintsProvider`] instances.
///
/// Querying the registry collects hints from every registered provider and
/// returns them sorted by position.
pub struct InlayHintsRegistry {
    providers: Vec<(String, Box<dyn InlayHintsProvider>)>,
}

impl InlayHintsRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a provider under the given name.
    pub fn register(&mut self, name: impl Into<String>, provider: Box<dyn InlayHintsProvider>) {
        self.providers.push((name.into(), provider));
    }

    /// Remove a provider by name. Returns `true` if it was found.
    pub fn unregister(&mut self, name: &str) -> bool {
        let before = self.providers.len();
        self.providers.retain(|(n, _)| n != name);
        self.providers.len() < before
    }

    /// Query all registered providers for the given range, merge and sort.
    pub fn provide_all(
        &self,
        uri: &str,
        start_line: u32,
        end_line: u32,
    ) -> Vec<InlayHint> {
        let mut all: Vec<InlayHint> = self
            .providers
            .iter()
            .flat_map(|(_, p)| p.provide_inlay_hints(uri, start_line, end_line))
            .collect();
        all.sort_by(|a, b| {
            a.position_line
                .cmp(&b.position_line)
                .then(a.position_col.cmp(&b.position_col))
        });
        all
    }

    /// Query a single provider by name.
    pub fn provide_by_name(
        &self,
        name: &str,
        uri: &str,
        start_line: u32,
        end_line: u32,
    ) -> Result<Vec<InlayHint>, InlayHintError> {
        self.providers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, p)| p.provide_inlay_hints(uri, start_line, end_line))
            .ok_or_else(|| InlayHintError::ProviderNotFound(name.to_string()))
    }

    /// Return the number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Return whether the registry has no providers.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl Default for InlayHintsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics summarizing the composition of a collection of inlay hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InlayHintStats {
    /// Total number of hints.
    pub total_hints: usize,
    /// Number of [`InlayHintKind::Type`] hints.
    pub type_hints: usize,
    /// Number of [`InlayHintKind::Parameter`] hints.
    pub parameter_hints: usize,
    /// Number of [`InlayHintKind::Other`] hints.
    pub other_hints: usize,
}

/// Compute aggregate statistics for a slice of inlay hints.
pub fn compute_hint_stats(hints: &[InlayHint]) -> InlayHintStats {
    let mut type_hints: usize = 0;
    let mut parameter_hints: usize = 0;
    let mut other_hints: usize = 0;
    for hint in hints {
        match hint.kind {
            InlayHintKind::Type => type_hints += 1,
            InlayHintKind::Parameter => parameter_hints += 1,
            InlayHintKind::Other => other_hints += 1,
        }
    }
    InlayHintStats {
        total_hints: hints.len(),
        type_hints,
        parameter_hints,
        other_hints,
    }
}

/// Accumulated statistics for inlayhints operations.
#[derive(Debug, Clone, PartialEq)]
pub struct InlayhintsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl InlayhintsStats {
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
    pub fn merge(&mut self, other: &InlayhintsStats) {
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

impl Default for InlayhintsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for InlayhintsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InlayhintsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for inlayhints.
#[derive(Debug, Clone)]
pub struct InlayhintsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl InlayhintsValidator {
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

impl Default for InlayhintsValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Inlay-hint display mode (on / off / offUnlessPressed)
// ---------------------------------------------------------------------------

/// Controls when inlay hints are visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintDisplayMode {
    /// Always show inlay hints.
    On,
    /// Never show inlay hints.
    Off,
    /// Hide by default; show while Ctrl+Alt is held.
    OffUnlessPressed,
}

impl InlayHintDisplayMode {
    /// Whether hints should be rendered given the current modifier state.
    pub fn should_show(&self, modifier_held: bool) -> bool {
        match self {
            Self::On => true,
            Self::Off => false,
            Self::OffUnlessPressed => modifier_held,
        }
    }

    /// Cycle through modes: On → Off → OffUnlessPressed → On.
    pub fn toggle(self) -> Self {
        match self {
            Self::On => Self::Off,
            Self::Off => Self::OffUnlessPressed,
            Self::OffUnlessPressed => Self::On,
        }
    }
}

impl fmt::Display for InlayHintDisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::On => write!(f, "on"),
            Self::Off => write!(f, "off"),
            Self::OffUnlessPressed => write!(f, "offUnlessPressed"),
        }
    }
}

// ---------------------------------------------------------------------------
// Inline rendering helper
// ---------------------------------------------------------------------------

/// Style descriptor passed into [`render_line_with_inlay_hints`].
#[derive(Debug, Clone)]
pub struct InlayHintStyle {
    /// Prefix inserted before hint text (e.g. ANSI dim-on sequence).
    pub prefix: String,
    /// Suffix inserted after hint text (e.g. ANSI reset sequence).
    pub suffix: String,
}

impl Default for InlayHintStyle {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            suffix: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// InlayHintFilter – select hints by kind or line range
// ---------------------------------------------------------------------------

/// Configurable filter for selecting a subset of inlay hints.
#[derive(Debug, Clone)]
pub struct InlayHintFilter {
    /// If set, only hints of these kinds are kept.
    pub kinds: Option<Vec<InlayHintKind>>,
    /// If set, only hints on lines `>= min_line` are kept.
    pub min_line: Option<u32>,
    /// If set, only hints on lines `<= max_line` are kept.
    pub max_line: Option<u32>,
}

impl InlayHintFilter {
    /// Create a filter that accepts everything.
    pub fn accept_all() -> Self {
        Self {
            kinds: None,
            min_line: None,
            max_line: None,
        }
    }

    /// Restrict to the given kinds.
    pub fn with_kinds(mut self, kinds: Vec<InlayHintKind>) -> Self {
        self.kinds = Some(kinds);
        self
    }

    /// Restrict to a line range (inclusive).
    pub fn with_line_range(mut self, min: u32, max: u32) -> Self {
        self.min_line = Some(min);
        self.max_line = Some(max);
        self
    }

    /// Return `true` if `hint` passes the filter.
    pub fn matches(&self, hint: &InlayHint) -> bool {
        if let Some(ref kinds) = self.kinds {
            if !kinds.contains(&hint.kind) {
                return false;
            }
        }
        if let Some(min) = self.min_line {
            if hint.position_line < min {
                return false;
            }
        }
        if let Some(max) = self.max_line {
            if hint.position_line > max {
                return false;
            }
        }
        true
    }

    /// Apply the filter to a slice, returning only matching hints.
    pub fn apply(&self, hints: &[InlayHint]) -> Vec<InlayHint> {
        hints.iter().filter(|h| self.matches(h)).cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// InlayHintCache – per-URI, per-range caching of computed hints
// ---------------------------------------------------------------------------

/// Key identifying a cached hint region.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(dead_code)]
struct CacheKey {
    uri: String,
    start_line: u32,
    end_line: u32,
}

/// Simple cache for inlay hints keyed by `(uri, start_line, end_line)`.
#[derive(Debug)]
pub struct InlayHintCache {
    entries: HashMap<(String, u32, u32), Vec<InlayHint>>,
}

impl InlayHintCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Look up cached hints for the given region.
    pub fn get(&self, uri: &str, start_line: u32, end_line: u32) -> Option<&Vec<InlayHint>> {
        self.entries.get(&(uri.to_string(), start_line, end_line))
    }

    /// Insert hints for a region, replacing any previous entry.
    pub fn insert(&mut self, uri: &str, start_line: u32, end_line: u32, hints: Vec<InlayHint>) {
        self.entries
            .insert((uri.to_string(), start_line, end_line), hints);
    }

    /// Invalidate all cached entries for a given URI.
    pub fn invalidate_uri(&mut self, uri: &str) {
        self.entries.retain(|(u, _, _), _| u != uri);
    }

    /// Invalidate entries that overlap with the given line range for a URI.
    pub fn invalidate_range(&mut self, uri: &str, start_line: u32, end_line: u32) {
        self.entries.retain(|(u, s, e), _| {
            if u != uri {
                return true;
            }
            // Keep entries that don't overlap
            *e < start_line || *s > end_line
        });
    }

    /// Clear all cached entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return the number of cached regions.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for InlayHintCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// InlayHintDiff – compare two hint sets
// ---------------------------------------------------------------------------

/// Describes differences between two hint sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHintDiff {
    /// Hints present in the new set but not in the old set.
    pub added: Vec<InlayHint>,
    /// Hints present in the old set but not in the new set.
    pub removed: Vec<InlayHint>,
    /// Hints present in both sets (unchanged).
    pub unchanged: Vec<InlayHint>,
}

impl InlayHintDiff {
    /// Compute the diff between `old` and `new` hint sets.
    ///
    /// Uses structural equality ([`PartialEq`]) to classify each hint.
    pub fn compute(old: &[InlayHint], new: &[InlayHint]) -> Self {
        let mut added: Vec<InlayHint> = Vec::new();
        let mut removed: Vec<InlayHint> = Vec::new();
        let mut unchanged: Vec<InlayHint> = Vec::new();

        // Track which old hints have been matched
        let mut old_matched = vec![false; old.len()];

        for new_hint in new {
            let mut found = false;
            for (i, old_hint) in old.iter().enumerate() {
                if !old_matched[i] && new_hint == old_hint {
                    old_matched[i] = true;
                    unchanged.push(new_hint.clone());
                    found = true;
                    break;
                }
            }
            if !found {
                added.push(new_hint.clone());
            }
        }

        for (i, old_hint) in old.iter().enumerate() {
            if !old_matched[i] {
                removed.push(old_hint.clone());
            }
        }

        Self {
            added,
            removed,
            unchanged,
        }
    }

    /// Return `true` if there are no differences.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    /// Total number of changed hints (added + removed).
    pub fn change_count(&self) -> usize {
        self.added.len() + self.removed.len()
    }
}

/// Render a single editor line with inlay hints spliced in.
///
/// `hints` must contain only hints whose `position_line` matches the line
/// being rendered and must be sorted by `position_col` ascending.  Each hint
/// is inserted at the corresponding column offset, wrapped in the style
/// prefix/suffix.
pub fn render_line_with_inlay_hints(
    line: &str,
    hints: &[InlayHint],
    style: &InlayHintStyle,
) -> String {
    if hints.is_empty() {
        return line.to_string();
    }

    let mut result = String::with_capacity(line.len() + hints.len() * 16);
    let mut col: u32 = 0;
    let mut hint_idx = 0;

    for ch in line.chars() {
        // Insert any hints at this column position.
        while hint_idx < hints.len() && hints[hint_idx].position_col == col {
            let hint = &hints[hint_idx];
            if hint.padding_left {
                result.push(' ');
            }
            result.push_str(&style.prefix);
            for part in &hint.label {
                result.push_str(&part.value);
            }
            result.push_str(&style.suffix);
            if hint.padding_right {
                result.push(' ');
            }
            hint_idx += 1;
        }
        result.push(ch);
        col += 1;
    }

    // Hints that appear past the end of the line.
    while hint_idx < hints.len() {
        let hint = &hints[hint_idx];
        if hint.padding_left {
            result.push(' ');
        }
        result.push_str(&style.prefix);
        for part in &hint.label {
            result.push_str(&part.value);
        }
        result.push_str(&style.suffix);
        if hint.padding_right {
            result.push(' ');
        }
        hint_idx += 1;
    }

    result
}

// ---------------------------------------------------------------------------
// Inlay-hint analysis utilities
// ---------------------------------------------------------------------------

/// Count the total number of label parts across all hints.
pub fn total_label_parts(hints: &[InlayHint]) -> usize {
    hints.iter().map(|h| h.label.len()).sum()
}

/// Return the set of unique lines that have at least one inlay hint.
pub fn hint_lines(hints: &[InlayHint]) -> Vec<u32> {
    let mut lines: Vec<u32> = hints.iter().map(|h| h.position_line).collect();
    lines.sort();
    lines.dedup();
    lines
}

/// Return the maximum number of hints on any single line.
pub fn max_hints_per_line(hints: &[InlayHint]) -> usize {
    if hints.is_empty() {
        return 0;
    }
    let mut counts = std::collections::HashMap::<u32, usize>::new();
    for h in hints {
        *counts.entry(h.position_line).or_insert(0) += 1;
    }
    counts.values().copied().max().unwrap_or(0)
}

/// Concatenate all label text from a hint into a single string.
pub fn hint_full_label(hint: &InlayHint) -> String {
    hint.label.iter().map(|p| p.value.as_str()).collect::<Vec<_>>().join("")
}

/// Return hints sorted by position (line first, then column).
pub fn sort_hints_by_position(hints: &mut [InlayHint]) {
    hints.sort_by(|a, b| {
        a.position_line
            .cmp(&b.position_line)
            .then(a.position_col.cmp(&b.position_col))
    });
}

/// Partition hints into type hints and parameter hints.
pub fn partition_hints_by_kind(hints: &[InlayHint]) -> (Vec<&InlayHint>, Vec<&InlayHint>) {
    let mut types = Vec::new();
    let mut params = Vec::new();
    for h in hints {
        match h.kind {
            InlayHintKind::Type => types.push(h),
            InlayHintKind::Parameter => params.push(h),
            InlayHintKind::Other => {}
        }
    }
    (types, params)
}

/// Find all hints whose label contains the given substring.
pub fn search_hints_by_label<'a>(hints: &'a [InlayHint], query: &str) -> Vec<&'a InlayHint> {
    let q = query.to_lowercase();
    hints
        .iter()
        .filter(|h| hint_full_label(h).to_lowercase().contains(&q))
        .collect()
}

// ---------------------------------------------------------------------------
// Hint density & line analysis
// ---------------------------------------------------------------------------

/// Return a map of line number to count of hints on that line.
pub fn hints_per_line(hints: &[InlayHint]) -> HashMap<u32, usize> {
    let mut counts = HashMap::new();
    for h in hints {
        *counts.entry(h.position_line).or_insert(0) += 1;
    }
    counts
}

/// Return lines that have more hints than the given threshold.
pub fn dense_lines(hints: &[InlayHint], threshold: usize) -> Vec<u32> {
    let counts = hints_per_line(hints);
    let mut lines: Vec<u32> = counts
        .into_iter()
        .filter(|(_, count)| *count > threshold)
        .map(|(line, _)| line)
        .collect();
    lines.sort();
    lines
}

/// Return hints that have a tooltip set on any of their label parts.
pub fn hints_with_label_tooltips<'a>(hints: &'a [InlayHint]) -> Vec<&'a InlayHint> {
    hints
        .iter()
        .filter(|h| h.label.iter().any(|p| p.tooltip.is_some()))
        .collect()
}

/// Return the total character length of all hint labels combined.
pub fn combined_label_length(hints: &[InlayHint]) -> usize {
    hints
        .iter()
        .map(|h| hint_full_label(h).len())
        .sum()
}

/// Return hints that have a command attached to any label part.
pub fn hints_having_commands<'a>(hints: &'a [InlayHint]) -> Vec<&'a InlayHint> {
    hints
        .iter()
        .filter(|h| h.label.iter().any(|p| p.command.is_some()))
        .collect()
}

/// Return the average number of label parts per hint (0.0 if empty).
pub fn avg_label_parts(hints: &[InlayHint]) -> f64 {
    if hints.is_empty() {
        return 0.0;
    }
    let total: usize = hints.iter().map(|h| h.label.len()).sum();
    total as f64 / hints.len() as f64
}

/// Merge two sets of hints, deduplicating by position and full label text.
pub fn merge_hint_sets(a: &[InlayHint], b: &[InlayHint]) -> Vec<InlayHint> {
    let mut result: Vec<InlayHint> = a.to_vec();
    let mut seen: std::collections::HashSet<(u32, u32, String)> = a
        .iter()
        .map(|h| (h.position_line, h.position_col, hint_full_label(h)))
        .collect();
    for h in b {
        let key = (h.position_line, h.position_col, hint_full_label(h));
        if seen.insert(key) {
            result.push(h.clone());
        }
    }
    result
}

/// Group hints by their kind into separate vectors.
pub fn classify_hints(hints: &[InlayHint]) -> (Vec<&InlayHint>, Vec<&InlayHint>, Vec<&InlayHint>) {
    let mut types = Vec::new();
    let mut params = Vec::new();
    let mut others = Vec::new();
    for h in hints {
        match h.kind {
            InlayHintKind::Type => types.push(h),
            InlayHintKind::Parameter => params.push(h),
            InlayHintKind::Other => others.push(h),
        }
    }
    (types, params, others)
}

/// Return hints that have multi-part labels (more than one label part).
pub fn multi_segment_hints<'a>(hints: &'a [InlayHint]) -> Vec<&'a InlayHint> {
    hints.iter().filter(|h| h.label.len() > 1).collect()
}

// ---------------------------------------------------------------------------
// InlayHintInteraction – click/hover events
// ---------------------------------------------------------------------------

/// The kind of interaction on an inlay hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintInteractionKind {
    Click,
    Hover,
    DoubleClick,
}

/// An interaction event on an inlay hint.
#[derive(Debug, Clone)]
pub struct InlayHintInteraction {
    pub kind: InlayHintInteractionKind,
    pub hint_line: u32,
    pub hint_col: u32,
    pub label_part_index: usize,
    pub command: Option<String>,
}

impl InlayHintInteraction {
    pub fn click(hint: &InlayHint, label_part_index: usize) -> Self {
        let cmd = hint.label.get(label_part_index).and_then(|p| p.command.clone());
        Self {
            kind: InlayHintInteractionKind::Click,
            hint_line: hint.position_line,
            hint_col: hint.position_col,
            label_part_index,
            command: cmd,
        }
    }

    pub fn hover(hint: &InlayHint, label_part_index: usize) -> Self {
        Self {
            kind: InlayHintInteractionKind::Hover,
            hint_line: hint.position_line,
            hint_col: hint.position_col,
            label_part_index,
            command: None,
        }
    }

    /// Whether this interaction should trigger a command.
    pub fn has_command(&self) -> bool {
        self.command.is_some()
    }
}

// ---------------------------------------------------------------------------
// InlayHintPadding – spacing rules
// ---------------------------------------------------------------------------

/// Padding rules for an inlay hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InlayHintPadding {
    pub left: u8,
    pub right: u8,
}

impl InlayHintPadding {
    pub const NONE: Self = Self { left: 0, right: 0 };
    pub const TYPE_HINT: Self = Self { left: 1, right: 0 };
    pub const PARAM_HINT: Self = Self { left: 0, right: 1 };
    pub const BOTH: Self = Self { left: 1, right: 1 };

    /// Derive padding from a hint's existing flags.
    pub fn from_hint(hint: &InlayHint) -> Self {
        Self {
            left: if hint.padding_left { 1 } else { 0 },
            right: if hint.padding_right { 1 } else { 0 },
        }
    }

    /// Total padding width.
    pub fn total(&self) -> u8 {
        self.left + self.right
    }
}

// ---------------------------------------------------------------------------
// InlayHintTheme – custom styling
// ---------------------------------------------------------------------------

/// Styling properties for inlay hints.
#[derive(Debug, Clone, PartialEq)]
pub struct InlayHintTheme {
    pub foreground: String,
    pub background: String,
    pub font_size_factor: f32,
    pub font_style: String,
    pub border_radius: u8,
}

impl InlayHintTheme {
    pub fn default_light() -> Self {
        Self {
            foreground: "#747474".into(),
            background: "#e0e0e0".into(),
            font_size_factor: 0.9,
            font_style: "normal".into(),
            border_radius: 3,
        }
    }

    pub fn default_dark() -> Self {
        Self {
            foreground: "#969696".into(),
            background: "#3a3a3a".into(),
            font_size_factor: 0.9,
            font_style: "normal".into(),
            border_radius: 3,
        }
    }

    /// Generate a simple CSS-like style string.
    pub fn to_css(&self) -> String {
        format!(
            "color: {}; background: {}; font-size: {}em; font-style: {}; border-radius: {}px",
            self.foreground, self.background, self.font_size_factor, self.font_style, self.border_radius
        )
    }
}

// ---------------------------------------------------------------------------
// Inlay hint toggle by kind
// ---------------------------------------------------------------------------

/// Visibility settings per hint kind.
#[derive(Debug, Clone)]
pub struct InlayHintVisibility {
    pub show_types: bool,
    pub show_parameters: bool,
    pub show_other: bool,
}

impl InlayHintVisibility {
    /// All hints visible.
    pub fn all() -> Self {
        Self { show_types: true, show_parameters: true, show_other: true }
    }

    /// No hints visible.
    pub fn none() -> Self {
        Self { show_types: false, show_parameters: false, show_other: false }
    }

    /// Check if a hint kind is visible.
    pub fn is_visible(&self, kind: InlayHintKind) -> bool {
        match kind {
            InlayHintKind::Type => self.show_types,
            InlayHintKind::Parameter => self.show_parameters,
            InlayHintKind::Other => self.show_other,
        }
    }

    /// Filter hints to only visible ones.
    pub fn filter<'a>(&self, hints: &'a [InlayHint]) -> Vec<&'a InlayHint> {
        hints.iter().filter(|h| self.is_visible(h.kind)).collect()
    }

    /// Toggle visibility for a specific kind.
    pub fn toggle(&mut self, kind: InlayHintKind) {
        match kind {
            InlayHintKind::Type => self.show_types = !self.show_types,
            InlayHintKind::Parameter => self.show_parameters = !self.show_parameters,
            InlayHintKind::Other => self.show_other = !self.show_other,
        }
    }
}


// ---------------------------------------------------------------------------
// Inlay hint animation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationEasing { Linear, EaseIn, EaseOut, EaseInOut }
impl Default for AnimationEasing { fn default() -> Self { Self::EaseOut } }
impl AnimationEasing {
    pub fn apply(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t, Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 },
        }
    }
}
impl fmt::Display for AnimationEasing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self { Self::Linear => write!(f, "linear"), Self::EaseIn => write!(f, "ease-in"), Self::EaseOut => write!(f, "ease-out"), Self::EaseInOut => write!(f, "ease-in-out") }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlayHintAnimation {
    pub hint_id: String, pub opacity: f64, pub target_opacity: f64,
    pub duration_ms: u64, pub elapsed_ms: u64, pub easing: AnimationEasing,
}
impl InlayHintAnimation {
    pub fn fade_in(id: impl Into<String>, dur: u64) -> Self { Self { hint_id: id.into(), opacity: 0.0, target_opacity: 1.0, duration_ms: dur, elapsed_ms: 0, easing: AnimationEasing::default() } }
    pub fn fade_out(id: impl Into<String>, dur: u64) -> Self { Self { hint_id: id.into(), opacity: 1.0, target_opacity: 0.0, duration_ms: dur, elapsed_ms: 0, easing: AnimationEasing::default() } }
    pub fn with_easing(mut self, e: AnimationEasing) -> Self { self.easing = e; self }
    pub fn tick(&mut self, delta: u64) {
        self.elapsed_ms = (self.elapsed_ms + delta).min(self.duration_ms);
        let p = if self.duration_ms == 0 { 1.0 } else { self.elapsed_ms as f64 / self.duration_ms as f64 };
        let e = self.easing.apply(p);
        let s = if self.target_opacity > 0.5 { 0.0 } else { 1.0 };
        self.opacity = s + (self.target_opacity - s) * e;
    }
    pub fn is_complete(&self) -> bool { self.elapsed_ms >= self.duration_ms }
    pub fn progress(&self) -> f64 { if self.duration_ms == 0 { 1.0 } else { self.elapsed_ms as f64 / self.duration_ms as f64 } }
    pub fn reset(&mut self) { self.elapsed_ms = 0; self.opacity = if self.target_opacity > 0.5 { 0.0 } else { 1.0 }; }
}
impl fmt::Display for InlayHintAnimation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Anim({}, {:.0}%->{:.0}%, {})", self.hint_id, self.opacity*100.0, self.target_opacity*100.0, self.easing) }
}

// ---------------------------------------------------------------------------
// Inlay hint editor integration
// ---------------------------------------------------------------------------

pub struct InlayHintEditorIntegration {
    uri: String, hints: Vec<InlayHint>, animations: Vec<InlayHintAnimation>,
    enabled: bool, visible_range: (u32, u32), anim_dur_ms: u64,
}
impl InlayHintEditorIntegration {
    pub fn new(uri: impl Into<String>) -> Self { Self { uri: uri.into(), hints: Vec::new(), animations: Vec::new(), enabled: true, visible_range: (0, 100), anim_dur_ms: 150 } }
    pub fn uri(&self) -> &str { &self.uri }
    pub fn set_visible_range(&mut self, s: u32, e: u32) { self.visible_range = (s, e); }
    pub fn visible_range(&self) -> (u32, u32) { self.visible_range }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn update_hints(&mut self, new: Vec<InlayHint>) {
        let d = self.anim_dur_ms;
        self.animations = new.iter().enumerate().map(|(i, h)| InlayHintAnimation::fade_in(format!("{}:{}:{}", h.position_line, h.position_col, i), d)).collect();
        self.hints = new;
    }
    pub fn hints_in_range(&self) -> Vec<&InlayHint> { let (s, e) = self.visible_range; self.hints.iter().filter(|h| h.position_line >= s && h.position_line <= e).collect() }
    pub fn hint_count(&self) -> usize { self.hints.len() }
    pub fn visible_hint_count(&self) -> usize { self.hints_in_range().len() }
    pub fn tick_animations(&mut self, d: u64) { for a in &mut self.animations { a.tick(d); } self.animations.retain(|a| !a.is_complete() || a.target_opacity > 0.5); }
    pub fn active_animation_count(&self) -> usize { self.animations.iter().filter(|a| !a.is_complete()).count() }
    pub fn set_animation_duration(&mut self, ms: u64) { self.anim_dur_ms = ms; }
    pub fn clear(&mut self) { self.hints.clear(); self.animations.clear(); }
}
impl fmt::Display for InlayHintEditorIntegration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "InlayHintEditor({}, {} hints, enabled={})", self.uri, self.hints.len(), self.enabled) }
}
impl Default for InlayHintEditorIntegration { fn default() -> Self { Self::new("untitled") } }


// ---------------------------------------------------------------------------
// InlayHintAnimationConfig — configuration for InlayHintAnimation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct InlayHintAnimationConfig {
    pub max_entries: usize,
    pub auto_refresh: bool,
    pub refresh_interval_ms: u64,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl InlayHintAnimationConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_refresh(mut self, a: bool) -> Self { self.auto_refresh = a; self }
    pub fn with_refresh_interval(mut self, ms: u64) -> Self { self.refresh_interval_ms = ms; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn is_refresh_due(&self, elapsed_ms: u64) -> bool { self.auto_refresh && elapsed_ms >= self.refresh_interval_ms }
}

impl Default for InlayHintAnimationConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_refresh: true, refresh_interval_ms: 5000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for InlayHintAnimationConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_refresh={}, interval={}ms)", self.max_entries, self.auto_refresh, self.refresh_interval_ms)
    }
}

// ---------------------------------------------------------------------------
// InlayHintEditorIntegrationStats — statistics tracker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct InlayHintEditorIntegrationStats {
    pub total_operations: u64,
    pub successful: u64,
    pub failed: u64,
    pub total_duration_ms: u64,
    pub peak_concurrent: usize,
    pub current_concurrent: usize,
}

impl InlayHintEditorIntegrationStats {
    pub fn new() -> Self { Self::default() }
    pub fn record_success(&mut self, duration_ms: u64) {
        self.total_operations += 1; self.successful += 1; self.total_duration_ms += duration_ms;
    }
    pub fn record_failure(&mut self, duration_ms: u64) {
        self.total_operations += 1; self.failed += 1; self.total_duration_ms += duration_ms;
    }
    pub fn success_rate(&self) -> f64 { if self.total_operations == 0 { 0.0 } else { self.successful as f64 / self.total_operations as f64 } }
    pub fn avg_duration_ms(&self) -> f64 { if self.total_operations == 0 { 0.0 } else { self.total_duration_ms as f64 / self.total_operations as f64 } }
    pub fn update_concurrent(&mut self, current: usize) {
        self.current_concurrent = current;
        if current > self.peak_concurrent { self.peak_concurrent = current; }
    }
    pub fn reset(&mut self) { *self = Self::default(); }
    pub fn merge(&mut self, other: &Self) {
        self.total_operations += other.total_operations;
        self.successful += other.successful;
        self.failed += other.failed;
        self.total_duration_ms += other.total_duration_ms;
        if other.peak_concurrent > self.peak_concurrent { self.peak_concurrent = other.peak_concurrent; }
    }
}

impl fmt::Display for InlayHintEditorIntegrationStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(ops={}, success={:.1}%, avg={:.1}ms)", self.total_operations, self.success_rate() * 100.0, self.avg_duration_ms())
    }
}

// ---------------------------------------------------------------------------
// InlayHintAnimationEventKind — event types for InlayHintAnimation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintAnimationEventKind {
    Created,
    Updated,
    Deleted,
    Refreshed,
    Error,
}

impl fmt::Display for InlayHintAnimationEventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Updated => write!(f, "updated"),
            Self::Deleted => write!(f, "deleted"),
            Self::Refreshed => write!(f, "refreshed"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A recorded event in the InlayHintAnimation lifecycle.
#[derive(Debug, Clone)]
pub struct InlayHintAnimationEvent {
    pub kind: InlayHintAnimationEventKind,
    pub timestamp: u64,
    pub detail: Option<String>,
}

impl InlayHintAnimationEvent {
    pub fn new(kind: InlayHintAnimationEventKind, timestamp: u64) -> Self {
        Self { kind, timestamp, detail: None }
    }
    pub fn with_detail(mut self, d: impl Into<String>) -> Self { self.detail = Some(d.into()); self }
    pub fn is_error(&self) -> bool { self.kind == InlayHintAnimationEventKind::Error }
}

impl fmt::Display for InlayHintAnimationEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Event({}, t={})", self.kind, self.timestamp)
    }
}

// ---------------------------------------------------------------------------
// InlayHintLayoutCalculator
// ---------------------------------------------------------------------------

/// Computes layout offsets for inlay hints on a line.
pub struct InlayHintLayoutCalculator {
    spacing: u32,
}

impl InlayHintLayoutCalculator {
    pub fn new(spacing: u32) -> Self {
        Self { spacing }
    }

    /// Compute the x offset for a new hint inserted at position `col` on a line
    /// that already has `existing_hints` hints placed.
    pub fn x_offset(&self, col: u32, existing_hints_width: u32) -> u32 {
        col + existing_hints_width + self.spacing * if existing_hints_width > 0 { 1 } else { 0 }
    }

    /// Total display width consumed by hints on one line.
    pub fn total_width_on_line(&self, hint_widths: &[u32]) -> u32 {
        if hint_widths.is_empty() {
            return 0;
        }
        let widths_sum: u32 = hint_widths.iter().sum();
        widths_sum + self.spacing * (hint_widths.len() as u32 - 1)
    }
}

// ---------------------------------------------------------------------------
// InlayHintPredicateFilter
// ---------------------------------------------------------------------------

/// Filters inlay hints by various criteria using a builder pattern.
pub struct InlayHintPredicateFilter {
    kind: Option<InlayHintKind>,
    line_range: Option<(u32, u32)>,
    min_label_length: Option<usize>,
}

impl InlayHintPredicateFilter {
    pub fn new() -> Self {
        Self { kind: None, line_range: None, min_label_length: None }
    }

    pub fn with_kind(mut self, kind: InlayHintKind) -> Self {
        self.kind = Some(kind);
        self
    }

    pub fn with_line_range(mut self, start: u32, end: u32) -> Self {
        self.line_range = Some((start, end));
        self
    }

    pub fn with_min_label_length(mut self, len: usize) -> Self {
        self.min_label_length = Some(len);
        self
    }

    pub fn matches(&self, hint: &InlayHint) -> bool {
        if let Some(k) = self.kind {
            if hint.kind != k {
                return false;
            }
        }
        if let Some((start, end)) = self.line_range {
            if hint.position_line < start || hint.position_line > end {
                return false;
            }
        }
        if let Some(min_len) = self.min_label_length {
            let total_len: usize = hint.label.iter().map(|p| p.value.len()).sum();
            if total_len < min_len {
                return false;
            }
        }
        true
    }

    pub fn filter_hints<'a>(&self, hints: &'a [InlayHint]) -> Vec<&'a InlayHint> {
        hints.iter().filter(|h| self.matches(h)).collect()
    }

    pub fn filtered_count(&self, hints: &[InlayHint]) -> usize {
        hints.iter().filter(|h| self.matches(h)).count()
    }
}

// ---------------------------------------------------------------------------
// InlayHintVersionedCache
// ---------------------------------------------------------------------------

/// Stores inlay hints per file per version, with eviction support.
pub struct InlayHintVersionedCache {
    entries: HashMap<String, (u64, Vec<InlayHint>)>,
}

impl InlayHintVersionedCache {
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    pub fn set(&mut self, file: &str, version: u64, hints: Vec<InlayHint>) {
        self.entries.insert(file.to_string(), (version, hints));
    }

    pub fn get(&self, file: &str, version: u64) -> Option<&[InlayHint]> {
        self.entries.get(file).and_then(|(v, h)| {
            if *v == version { Some(h.as_slice()) } else { None }
        })
    }

    pub fn invalidate(&mut self, file: &str) -> bool {
        self.entries.remove(file).is_some()
    }

    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }

    pub fn cache_size(&self) -> usize {
        self.entries.len()
    }

    pub fn evict_oldest(&mut self) {
        if let Some(oldest) = self.entries.iter().min_by_key(|(_, (v, _))| *v).map(|(k, _)| k.clone()) {
            self.entries.remove(&oldest);
        }
    }
}


// ---------------------------------------------------------------------------
// inlayhints – Editor text helpers
// ---------------------------------------------------------------------------

/// A half-open range within a document `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XInlayhintsTextSpan {
    pub start: usize,
    pub end: usize,
}

impl XInlayhintsTextSpan {
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
pub fn x_inlayhints_count_lines(text: &str) -> usize {
    if text.is_empty() { return 0; }
    text.lines().count()
}

/// Return the byte offset of the start of line `n` (0-based).
pub fn x_inlayhints_line_start_offset(text: &str, line: usize) -> Option<usize> {
    let mut current = 0usize;
    for (i, l) in text.split('\n').enumerate() {
        if i == line { return Some(current); }
        current += l.len() + 1;
    }
    None
}

/// Compute the indentation level (number of leading spaces) of a line.
pub fn x_inlayhints_indent_level(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

/// Trim trailing whitespace from every line in `text`.
pub fn x_inlayhints_trim_trailing(text: &str) -> String {
    text.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Detect the dominant line ending in `text` (`"\n"` or `"\r\n"`).
pub fn x_inlayhints_detect_eol(text: &str) -> &'static str {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count().saturating_sub(crlf);
    if crlf > lf { "\r\n" } else { "\n" }
}

/// Simple word-boundary based tokenizer: split on whitespace and punctuation.
pub fn x_inlayhints_tokenize(text: &str) -> Vec<&str> {
    text.split(|c: char| c.is_whitespace() || ".,;:!?()[]{}".contains(c))
        .filter(|s| !s.is_empty())
        .collect()
}



// ---------------------------------------------------------------------------
// inlayhints – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for inlay hints rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YInlayhintsInlayHintPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl YInlayhintsInlayHintPriority {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Low => 0,
            Self::Normal => 1,
            Self::High => 2,
            Self::Critical => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Normal => "Normal",
            Self::High => "High",
            Self::Critical => "Critical",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YInlayhintsInlayHintPriority] {
        &[
            YInlayhintsInlayHintPriority::Low,
            YInlayhintsInlayHintPriority::Normal,
            YInlayhintsInlayHintPriority::High,
            YInlayhintsInlayHintPriority::Critical,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YInlayhintsInlayHintPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks hint cache data.
#[derive(Debug, Clone)]
pub struct YInlayhintsInlayHintCache {
    pub hints: Vec<(u32, String)>,
    pub max_size: usize,
    pub hits: u64,
}

impl YInlayhintsInlayHintCache {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            hints: Vec::new(),
            max_size: 0,
            hits: 0,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.hints.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.hints.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.hints.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YInlayhintsInlayHintCache({}: {:?})", "hints", self.hints)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_inlayhints_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_inlayhints_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_inlayhints_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_inlayhints_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_inlayhints_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_inlayhints_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_inlayhints_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_inlayhints_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// inlayhints – Extended inlay hint throttle helpers
// ---------------------------------------------------------------------------

/// Priority levels for inlay hint throttle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZInlayhintsPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZInlayhintsPriority {
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
    pub fn all_asc() -> [ZInlayhintsPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZInlayhintsPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks inlay hint throttle data.
#[derive(Debug, Clone)]
pub struct ZInlayhintsInlayHintThrottler {
    pub pending_ranges: Vec<(u32, u32)>,
    pub cooldown_ms: u64,
    pub active: bool,
}

impl ZInlayhintsInlayHintThrottler {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            pending_ranges: Vec::new(),
            cooldown_ms: 0,
            active: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.pending_ranges.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.pending_ranges.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.pending_ranges.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZInlayhintsInlayHintThrottler[cooldown_ms={:?}, active={:?}]", self.cooldown_ms, self.active)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.active = !c.active;
        c
    }
}

/// Compute a simple rolling hash for inlay hint throttle.
pub fn z_inlayhints_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_inlayhints_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_inlayhints_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_inlayhints_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_inlayhints_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_inlayhints_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_inlayhints_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 65
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer65 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer65 {
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
pub fn xb_fnv1a_65(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_65<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_65<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_65(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_65(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 93
// ---------------------------------------------------------------------------

/// Generic object pool `Xc93Pool<T>`.
pub struct Xc93Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc93Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc93PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc93Pool<T> {
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
    pub fn stats(&self) -> Xc93PoolStats {
        Xc93PoolStats {
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

impl<T> Default for Xc93Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc93Scheduler`.
pub struct Xc93Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc93Scheduler {
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

impl Default for Xc93Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_93 hash for the given byte slice.
pub fn xc_93_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_93 convention.
pub fn xc_93_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe78 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe78Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe78PipelineError {
    pub stage: Xe78Stage,
    pub message: String,
}

impl std::fmt::Display for Xe78PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe78Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe78Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe78PipelineError>>>,
    stage_names: Vec<Xe78Stage>,
}

impl Xe78Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe78PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe78Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe78PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe78Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe78PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe78Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe78PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe78Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe78PipelineError> {
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

    pub fn compose(mut self, other: Xe78Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe78CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe78CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe78Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe78CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe78CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe78Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe78CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_78_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe78CacheEntry {
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

    fn xe_78_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe78CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_78_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe78PipelineError> {
    Ok(data)
}

pub fn xe_78_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe78PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_78_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe78PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_78_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe78PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_78_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe78PipelineError> {
    Err(Xe78PipelineError {
        stage: Xe78Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_76: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg76Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg76Graph {
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

impl Default for Xg76Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_76: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg76Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg76Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg76Heap<T>) {
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

impl<T: Ord> Default for Xg76Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 92).
pub struct Xh92SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh92SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 134 as u64,
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

/// A compact bit set supporting boolean operations (variant 92).
pub struct Xh92BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh92BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 92).
pub struct Xi92Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi92Deque<T> {
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
pub struct Xi92Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi92Interval {
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

/// A simple interval tree (variant 92).
pub struct Xi92IntervalTree {
    xi_intervals: Vec<Xi92Interval>,
}

impl Xi92IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi92Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi92Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi92Interval) -> Vec<&Xi92Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi92Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi92Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi92Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi92Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi92Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi92Interval> = Vec::new();
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

    #[test]
    fn simple_hint_construction() {
        let hint = InlayHint::simple(10, 5, ": i32", InlayHintKind::Type);
        assert_eq!(hint.position_line, 10);
        assert_eq!(hint.position_col, 5);
        assert_eq!(hint.kind, InlayHintKind::Type);
        assert_eq!(hint.label.len(), 1);
        assert_eq!(hint.label[0].value, ": i32");
        assert!(!hint.padding_left);
        assert!(!hint.padding_right);
        assert!(hint.tooltip.is_none());
    }

    #[test]
    fn provider_returns_hints_in_range() {
        struct TestProvider;

        impl InlayHintsProvider for TestProvider {
            fn provide_inlay_hints(
                &self,
                _uri: &str,
                start_line: u32,
                end_line: u32,
            ) -> Vec<InlayHint> {
                (start_line..=end_line)
                    .map(|line| InlayHint::simple(line, 0, "hint", InlayHintKind::Other))
                    .collect()
            }
        }

        let provider = TestProvider;
        let hints = provider.provide_inlay_hints("file:///test.rs", 3, 5);
        assert_eq!(hints.len(), 3);
        assert_eq!(hints[0].position_line, 3);
        assert_eq!(hints[2].position_line, 5);
    }

    #[test]
    fn default_config_is_enabled() {
        let config = InlayHintsConfig::default();
        assert!(config.enabled);
        assert!(config.font_size.is_none());
        assert!(config.font_family.is_none());
        assert!(config.max_length.is_none());
    }

    #[test]
    fn builder_valid_hint() {
        let hint = InlayHintBuilder::new()
            .position(1, 2)
            .add_label_part(": u64")
            .kind(InlayHintKind::Type)
            .padding(true, false)
            .tooltip("unsigned 64-bit integer")
            .build()
            .expect("should build successfully");

        assert_eq!(hint.position_line, 1);
        assert_eq!(hint.position_col, 2);
        assert_eq!(hint.kind, InlayHintKind::Type);
        assert!(hint.padding_left);
        assert!(!hint.padding_right);
        assert_eq!(hint.tooltip.as_deref(), Some("unsigned 64-bit integer"));
        assert_eq!(hint.label[0].value, ": u64");
    }

    #[test]
    fn builder_missing_position() {
        let result = InlayHintBuilder::new()
            .add_label_part("text")
            .build();
        assert!(matches!(result, Err(InlayHintError::InvalidPosition { .. })));
    }

    #[test]
    fn builder_empty_label() {
        let result = InlayHintBuilder::new()
            .position(0, 0)
            .build();
        assert_eq!(result, Err(InlayHintError::EmptyLabel));
    }

    #[test]
    fn display_inlay_hint_kind() {
        assert_eq!(format!("{}", InlayHintKind::Type), "Type");
        assert_eq!(format!("{}", InlayHintKind::Parameter), "Parameter");
        assert_eq!(format!("{}", InlayHintKind::Other), "Other");
    }

    #[test]
    fn display_inlay_hint_concatenates_labels() {
        let hint = InlayHintBuilder::new()
            .position(0, 0)
            .add_label_part("name")
            .add_label_part(": ")
            .add_label_part("String")
            .kind(InlayHintKind::Type)
            .build()
            .unwrap();
        assert_eq!(format!("{hint}"), "name: String");
    }

    #[test]
    fn error_display_messages() {
        let e1 = InlayHintError::InvalidPosition { line: 5, col: 10 };
        assert_eq!(format!("{e1}"), "invalid position: line 5, col 10");

        let e2 = InlayHintError::EmptyLabel;
        assert_eq!(format!("{e2}"), "hint label must not be empty");

        let e3 = InlayHintError::ProviderNotFound("foo".into());
        assert_eq!(format!("{e3}"), "provider not found: foo");
    }

    #[test]
    fn registry_multiple_providers() {
        struct TypeHinter;
        impl InlayHintsProvider for TypeHinter {
            fn provide_inlay_hints(&self, _uri: &str, _s: u32, _e: u32) -> Vec<InlayHint> {
                vec![InlayHint::simple(2, 10, ": i32", InlayHintKind::Type)]
            }
        }

        struct ParamHinter;
        impl InlayHintsProvider for ParamHinter {
            fn provide_inlay_hints(&self, _uri: &str, _s: u32, _e: u32) -> Vec<InlayHint> {
                vec![InlayHint::simple(1, 5, "name:", InlayHintKind::Parameter)]
            }
        }

        let mut registry = InlayHintsRegistry::new();
        assert!(registry.is_empty());
        registry.register("types", Box::new(TypeHinter));
        registry.register("params", Box::new(ParamHinter));
        assert_eq!(registry.len(), 2);

        let hints = registry.provide_all("file:///test.rs", 0, 10);
        assert_eq!(hints.len(), 2);
        // Should be sorted by position: line 1 before line 2.
        assert_eq!(hints[0].position_line, 1);
        assert_eq!(hints[1].position_line, 2);
    }

    #[test]
    fn registry_provide_by_name_not_found() {
        let registry = InlayHintsRegistry::new();
        let result = registry.provide_by_name("missing", "file:///x", 0, 10);
        assert_eq!(
            result,
            Err(InlayHintError::ProviderNotFound("missing".into()))
        );
    }

    #[test]
    fn registry_unregister() {
        struct Dummy;
        impl InlayHintsProvider for Dummy {
            fn provide_inlay_hints(&self, _: &str, _: u32, _: u32) -> Vec<InlayHint> {
                vec![]
            }
        }

        let mut registry = InlayHintsRegistry::new();
        registry.register("dummy", Box::new(Dummy));
        assert_eq!(registry.len(), 1);
        assert!(registry.unregister("dummy"));
        assert!(registry.is_empty());
        assert!(!registry.unregister("dummy"));
    }

    #[test]
    fn merge_adjacent_same_line() {
        let hints = vec![
            InlayHint::simple(5, 10, ": i32", InlayHintKind::Type),
            InlayHint::simple(5, 20, ": u8", InlayHintKind::Type),
            InlayHint::simple(7, 3, "x:", InlayHintKind::Parameter),
        ];
        let merged = InlayHint::merge_adjacent(hints);
        assert_eq!(merged.len(), 2);
        // First merged hint should have two label parts from line 5.
        assert_eq!(merged[0].position_line, 5);
        assert_eq!(merged[0].label.len(), 2);
        assert_eq!(merged[0].label[0].value, ": i32");
        assert_eq!(merged[0].label[1].value, ": u8");
        // Second hint is on line 7.
        assert_eq!(merged[1].position_line, 7);
        assert_eq!(merged[1].label.len(), 1);
    }

    #[test]
    fn merge_adjacent_empty_and_single() {
        assert!(InlayHint::merge_adjacent(vec![]).is_empty());
        let single = vec![InlayHint::simple(0, 0, "x", InlayHintKind::Other)];
        let result = InlayHint::merge_adjacent(single);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn resolve_data_construction() {
        let data = InlayHintResolveData::new("hint-42")
            .with_tooltip("Full type: std::string::String")
            .with_command("editor.action.showType", "Show Full Type");

        assert_eq!(data.resolve_id, "hint-42");
        assert_eq!(
            data.tooltip.as_deref(),
            Some("Full type: std::string::String")
        );
        assert_eq!(data.command_id.as_deref(), Some("editor.action.showType"));
        assert_eq!(data.command_title.as_deref(), Some("Show Full Type"));
    }

    #[test]
    fn stats_empty_hints() {
        let stats = compute_hint_stats(&[]);
        assert_eq!(stats.total_hints, 0);
        assert_eq!(stats.type_hints, 0);
        assert_eq!(stats.parameter_hints, 0);
        assert_eq!(stats.other_hints, 0);
    }

    #[test]
    fn stats_mixed_hints() {
        let hints = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(2, 0, "name:", InlayHintKind::Parameter),
            InlayHint::simple(3, 0, ": bool", InlayHintKind::Type),
            InlayHint::simple(4, 0, "debug", InlayHintKind::Other),
            InlayHint::simple(5, 0, "x:", InlayHintKind::Parameter),
        ];
        let stats = compute_hint_stats(&hints);
        assert_eq!(stats.total_hints, 5);
        assert_eq!(stats.type_hints, 2);
        assert_eq!(stats.parameter_hints, 2);
        assert_eq!(stats.other_hints, 1);
    }

    #[test]
    fn stats_all_same_kind() {
        let hints: Vec<InlayHint> = (0..4)
            .map(|i| InlayHint::simple(i, 0, "p:", InlayHintKind::Parameter))
            .collect();
        let stats = compute_hint_stats(&hints);
        assert_eq!(stats.total_hints, 4);
        assert_eq!(stats.type_hints, 0);
        assert_eq!(stats.parameter_hints, 4);
        assert_eq!(stats.other_hints, 0);
    }

    #[test]
    fn stats_counts_sum_to_total() {
        let hints = vec![
            InlayHint::simple(0, 0, ": u8", InlayHintKind::Type),
            InlayHint::simple(1, 0, "flag:", InlayHintKind::Parameter),
            InlayHint::simple(2, 0, "note", InlayHintKind::Other),
        ];
        let stats = compute_hint_stats(&hints);
        assert_eq!(
            stats.type_hints + stats.parameter_hints + stats.other_hints,
            stats.total_hints
        );
    }

    #[test]
    fn stats_struct_is_copy() {
        let hints = vec![InlayHint::simple(0, 0, ": f64", InlayHintKind::Type)];
        let stats = compute_hint_stats(&hints);
        let copy = stats;
        assert_eq!(stats, copy);
    }

    #[test]
    fn inlayhints_stats_new_defaults() {
        let stats = InlayhintsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn inlayhints_stats_record_success() {
        let mut stats = InlayhintsStats::new();
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
    fn inlayhints_stats_record_failure() {
        let mut stats = InlayhintsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn inlayhints_stats_reset() {
        let mut stats = InlayhintsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn inlayhints_stats_merge() {
        let mut a = InlayhintsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = InlayhintsStats::new();
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
    fn inlayhints_stats_display() {
        let mut stats = InlayhintsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn inlayhints_stats_default() {
        let stats = InlayhintsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn inlayhints_validator_accepts_valid_name() {
        let v = InlayhintsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn inlayhints_validator_rejects_empty() {
        let v = InlayhintsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn inlayhints_validator_rejects_too_long() {
        let v = InlayhintsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn inlayhints_validator_forbidden_prefix() {
        let v = InlayhintsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn inlayhints_validator_allowed_chars() {
        let v = InlayhintsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn inlayhints_validator_range() {
        let v = InlayhintsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn inlayhints_sanitize_removes_control() {
        let result = InlayhintsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn inlayhints_truncate_short_string() {
        assert_eq!(InlayhintsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn inlayhints_truncate_long_string() {
        let result = InlayhintsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn inlayhints_is_ascii_printable() {
        assert!(InlayhintsValidator::is_ascii_printable("Hello World 123"));
        assert!(!InlayhintsValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- render & display-mode tests ----------------------------------------

    #[test]
    fn display_mode_should_show() {
        assert!(InlayHintDisplayMode::On.should_show(false));
        assert!(InlayHintDisplayMode::On.should_show(true));
        assert!(!InlayHintDisplayMode::Off.should_show(false));
        assert!(!InlayHintDisplayMode::Off.should_show(true));
        assert!(!InlayHintDisplayMode::OffUnlessPressed.should_show(false));
        assert!(InlayHintDisplayMode::OffUnlessPressed.should_show(true));
    }

    #[test]
    fn display_mode_toggle_cycle() {
        let m = InlayHintDisplayMode::On;
        let m = m.toggle();
        assert_eq!(m, InlayHintDisplayMode::Off);
        let m = m.toggle();
        assert_eq!(m, InlayHintDisplayMode::OffUnlessPressed);
        let m = m.toggle();
        assert_eq!(m, InlayHintDisplayMode::On);
    }

    #[test]
    fn render_line_no_hints() {
        let line = "let x = 42;";
        let result = render_line_with_inlay_hints(line, &[], &InlayHintStyle::default());
        assert_eq!(result, line);
    }

    #[test]
    fn render_line_with_type_hint() {
        let line = "let x = 42;";
        let hint = InlayHint::simple(0, 5, ": i32", InlayHintKind::Type);
        let style = InlayHintStyle {
            prefix: "[".into(),
            suffix: "]".into(),
        };
        let result = render_line_with_inlay_hints(line, &[hint], &style);
        assert_eq!(result, "let x[: i32] = 42;");
    }

    #[test]
    fn render_line_hint_with_padding() {
        let line = "foo";
        let mut hint = InlayHint::simple(0, 3, ": T", InlayHintKind::Type);
        hint.padding_left = true;
        hint.padding_right = true;
        let result = render_line_with_inlay_hints(line, &[hint], &InlayHintStyle::default());
        assert_eq!(result, "foo : T ");
    }

    #[test]
    fn render_line_multiple_hints() {
        let line = "ab";
        let hints = vec![
            InlayHint::simple(0, 1, "X", InlayHintKind::Other),
            InlayHint::simple(0, 2, "Y", InlayHintKind::Other),
        ];
        let result = render_line_with_inlay_hints(line, &hints, &InlayHintStyle::default());
        assert_eq!(result, "aXbY");
    }

    #[test]
    fn render_line_hint_past_end() {
        let line = "ab";
        let hint = InlayHint::simple(0, 5, "Z", InlayHintKind::Other);
        let result = render_line_with_inlay_hints(line, &[hint], &InlayHintStyle::default());
        assert_eq!(result, "abZ");
    }

    // -- filter tests -------------------------------------------------------

    #[test]
    fn filter_accept_all_keeps_everything() {
        let hints = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(5, 0, "x:", InlayHintKind::Parameter),
            InlayHint::simple(10, 0, "note", InlayHintKind::Other),
        ];
        let filter = InlayHintFilter::accept_all();
        let result = filter.apply(&hints);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn filter_by_kind() {
        let hints = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(2, 0, "x:", InlayHintKind::Parameter),
            InlayHint::simple(3, 0, ": bool", InlayHintKind::Type),
        ];
        let filter = InlayHintFilter::accept_all().with_kinds(vec![InlayHintKind::Type]);
        let result = filter.apply(&hints);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|h| h.kind == InlayHintKind::Type));
    }

    #[test]
    fn filter_by_line_range() {
        let hints: Vec<InlayHint> = (0..20)
            .map(|i| InlayHint::simple(i, 0, "h", InlayHintKind::Other))
            .collect();
        let filter = InlayHintFilter::accept_all().with_line_range(5, 10);
        let result = filter.apply(&hints);
        assert_eq!(result.len(), 6); // lines 5..=10
        assert!(result.iter().all(|h| h.position_line >= 5 && h.position_line <= 10));
    }

    #[test]
    fn filter_combined_kind_and_range() {
        let hints = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(5, 0, "x:", InlayHintKind::Parameter),
            InlayHint::simple(8, 0, ": bool", InlayHintKind::Type),
            InlayHint::simple(12, 0, ": u8", InlayHintKind::Type),
        ];
        let filter = InlayHintFilter::accept_all()
            .with_kinds(vec![InlayHintKind::Type])
            .with_line_range(3, 10);
        let result = filter.apply(&hints);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].position_line, 8);
    }

    // -- cache tests --------------------------------------------------------

    #[test]
    fn cache_insert_and_get() {
        let mut cache = InlayHintCache::new();
        assert!(cache.is_empty());
        let hints = vec![InlayHint::simple(1, 0, ": i32", InlayHintKind::Type)];
        cache.insert("file:///a.rs", 0, 10, hints.clone());
        assert_eq!(cache.len(), 1);
        let got = cache.get("file:///a.rs", 0, 10).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].label[0].value, ": i32");
    }

    #[test]
    fn cache_invalidate_uri() {
        let mut cache = InlayHintCache::new();
        cache.insert("file:///a.rs", 0, 10, vec![]);
        cache.insert("file:///a.rs", 11, 20, vec![]);
        cache.insert("file:///b.rs", 0, 5, vec![]);
        assert_eq!(cache.len(), 3);
        cache.invalidate_uri("file:///a.rs");
        assert_eq!(cache.len(), 1);
        assert!(cache.get("file:///b.rs", 0, 5).is_some());
    }

    #[test]
    fn cache_invalidate_range() {
        let mut cache = InlayHintCache::new();
        cache.insert("file:///a.rs", 0, 10, vec![]);
        cache.insert("file:///a.rs", 15, 25, vec![]);
        cache.insert("file:///a.rs", 30, 40, vec![]);
        // Invalidate lines 8..=20 – overlaps entries [0,10] and [15,25]
        cache.invalidate_range("file:///a.rs", 8, 20);
        assert_eq!(cache.len(), 1);
        assert!(cache.get("file:///a.rs", 30, 40).is_some());
    }

    // -- diff tests ---------------------------------------------------------

    #[test]
    fn diff_identical_sets() {
        let hints = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(2, 5, "x:", InlayHintKind::Parameter),
        ];
        let diff = InlayHintDiff::compute(&hints, &hints);
        assert!(diff.is_empty());
        assert_eq!(diff.unchanged.len(), 2);
        assert_eq!(diff.change_count(), 0);
    }

    #[test]
    fn diff_all_new() {
        let old: Vec<InlayHint> = vec![];
        let new = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type),
        ];
        let diff = InlayHintDiff::compute(&old, &new);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 0);
        assert_eq!(diff.unchanged.len(), 0);
        assert_eq!(diff.change_count(), 1);
    }

    #[test]
    fn diff_all_removed() {
        let old = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type),
        ];
        let new: Vec<InlayHint> = vec![];
        let diff = InlayHintDiff::compute(&old, &new);
        assert_eq!(diff.added.len(), 0);
        assert_eq!(diff.removed.len(), 1);
        assert!(!diff.is_empty());
    }

    #[test]
    fn diff_mixed_changes() {
        let old = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(2, 0, "x:", InlayHintKind::Parameter),
        ];
        let new = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type), // unchanged
            InlayHint::simple(3, 0, ": bool", InlayHintKind::Type), // added
        ];
        let diff = InlayHintDiff::compute(&old, &new);
        assert_eq!(diff.unchanged.len(), 1);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].label[0].value, ": bool");
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].label[0].value, "x:");
        assert_eq!(diff.change_count(), 2);
    }

    #[test]
    fn total_label_parts_counts() {
        let hints = vec![
            InlayHint::simple(0, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(1, 0, "x:", InlayHintKind::Parameter),
        ];
        assert_eq!(total_label_parts(&hints), 2);
    }

    #[test]
    fn total_label_parts_empty() {
        assert_eq!(total_label_parts(&[]), 0);
    }

    #[test]
    fn hint_lines_unique_sorted() {
        let hints = vec![
            InlayHint::simple(5, 0, "a", InlayHintKind::Type),
            InlayHint::simple(3, 0, "b", InlayHintKind::Type),
            InlayHint::simple(5, 5, "c", InlayHintKind::Parameter),
        ];
        assert_eq!(hint_lines(&hints), vec![3, 5]);
    }

    #[test]
    fn max_hints_per_line_calculation() {
        let hints = vec![
            InlayHint::simple(1, 0, "a", InlayHintKind::Type),
            InlayHint::simple(1, 5, "b", InlayHintKind::Type),
            InlayHint::simple(2, 0, "c", InlayHintKind::Parameter),
        ];
        assert_eq!(max_hints_per_line(&hints), 2);
        assert_eq!(max_hints_per_line(&[]), 0);
    }

    #[test]
    fn hint_full_label_concatenates() {
        let mut hint = InlayHint::simple(0, 0, "x:", InlayHintKind::Parameter);
        hint.label.push(InlayHintLabelPart {
            value: " i32".to_string(),
            tooltip: None,
            command: None,
        });
        assert_eq!(hint_full_label(&hint), "x: i32");
    }

    #[test]
    fn sort_hints_by_position_orders() {
        let mut hints = vec![
            InlayHint::simple(3, 10, "a", InlayHintKind::Type),
            InlayHint::simple(1, 5, "b", InlayHintKind::Type),
            InlayHint::simple(3, 2, "c", InlayHintKind::Type),
        ];
        sort_hints_by_position(&mut hints);
        assert_eq!(hints[0].position_line, 1);
        assert_eq!(hints[1].position_col, 2);
        assert_eq!(hints[2].position_col, 10);
    }

    #[test]
    fn partition_hints_by_kind_separates() {
        let hints = vec![
            InlayHint::simple(0, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(0, 5, "x:", InlayHintKind::Parameter),
            InlayHint::simple(1, 0, ": bool", InlayHintKind::Type),
        ];
        let (types, params) = partition_hints_by_kind(&hints);
        assert_eq!(types.len(), 2);
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn search_hints_by_label_finds_matches() {
        let hints = vec![
            InlayHint::simple(0, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(1, 0, ": String", InlayHintKind::Type),
            InlayHint::simple(2, 0, "x:", InlayHintKind::Parameter),
        ];
        let found = search_hints_by_label(&hints, "string");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].position_line, 1);
    }

    #[test]
    fn search_hints_by_label_no_match() {
        let hints = vec![InlayHint::simple(0, 0, ": i32", InlayHintKind::Type)];
        assert!(search_hints_by_label(&hints, "nope").is_empty());
    }

    #[test]
    fn hints_per_line_counts() {
        let hints = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(1, 5, "x:", InlayHintKind::Parameter),
            InlayHint::simple(3, 0, ": bool", InlayHintKind::Type),
        ];
        let counts = hints_per_line(&hints);
        assert_eq!(counts.get(&1), Some(&2));
        assert_eq!(counts.get(&3), Some(&1));
        assert_eq!(counts.get(&2), None);
    }

    #[test]
    fn dense_lines_filters_above_threshold() {
        let hints = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(1, 5, "x:", InlayHintKind::Parameter),
            InlayHint::simple(1, 10, "y:", InlayHintKind::Parameter),
            InlayHint::simple(3, 0, ": bool", InlayHintKind::Type),
        ];
        let dense = dense_lines(&hints, 2);
        assert_eq!(dense, vec![1]);
    }

    #[test]
    fn dense_lines_empty_when_below_threshold() {
        let hints = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type),
        ];
        assert!(dense_lines(&hints, 5).is_empty());
    }

    #[test]
    fn hints_with_label_tooltips_filters() {
        let mut h1 = InlayHint::simple(0, 0, ": i32", InlayHintKind::Type);
        h1.label[0].tooltip = Some("integer type".into());
        let h2 = InlayHint::simple(1, 0, "x:", InlayHintKind::Parameter);
        let hints = vec![h1, h2];
        let result = hints_with_label_tooltips(&hints);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn combined_label_length_sums() {
        let hints = vec![
            InlayHint::simple(0, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(1, 0, "x:", InlayHintKind::Parameter),
        ];
        assert_eq!(combined_label_length(&hints), 7);
    }

    #[test]
    fn hints_having_commands_filters() {
        let mut h1 = InlayHint::simple(0, 0, ": i32", InlayHintKind::Type);
        h1.label[0].command = Some("goto.definition".into());
        let h2 = InlayHint::simple(1, 0, "x:", InlayHintKind::Parameter);
        let hints = vec![h1, h2];
        let result = hints_having_commands(&hints);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn avg_label_parts_computes() {
        let h1 = InlayHintBuilder::new()
            .position(0, 0)
            .add_label_part(": ")
            .add_label_part("i32")
            .kind(InlayHintKind::Type)
            .build()
            .unwrap();
        let h2 = InlayHint::simple(1, 0, "x:", InlayHintKind::Parameter);
        let avg = avg_label_parts(&[h1, h2]);
        assert!((avg - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn avg_label_parts_empty() {
        let hints: Vec<InlayHint> = vec![];
        assert!((avg_label_parts(&hints) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn merge_hint_sets_deduplicates() {
        let a = vec![
            InlayHint::simple(0, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(1, 5, "x:", InlayHintKind::Parameter),
        ];
        let b = vec![
            InlayHint::simple(0, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(2, 0, ": bool", InlayHintKind::Type),
        ];
        let merged = merge_hint_sets(&a, &b);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn classify_hints_groups() {
        let hints = vec![
            InlayHint::simple(0, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(1, 0, "x:", InlayHintKind::Parameter),
            InlayHint::simple(2, 0, ": bool", InlayHintKind::Type),
        ];
        let (types, params, others) = classify_hints(&hints);
        assert_eq!(types.len(), 2);
        assert_eq!(params.len(), 1);
        assert_eq!(others.len(), 0);
    }

    #[test]
    fn multi_segment_hints_filters() {
        let h1 = InlayHintBuilder::new()
            .position(0, 0)
            .add_label_part(": ")
            .add_label_part("i32")
            .kind(InlayHintKind::Type)
            .build()
            .unwrap();
        let h2 = InlayHint::simple(1, 0, "x:", InlayHintKind::Parameter);
        let hints = vec![h1, h2];
        let result = multi_segment_hints(&hints);
        assert_eq!(result.len(), 1);
    }

    // -- InlayHintInteraction tests --

    #[test]
    fn interaction_click_with_command() {
        let mut hint = InlayHint::simple(5, 10, ": i32", InlayHintKind::Type);
        hint.label[0].command = Some("goToDefinition".into());
        let interaction = InlayHintInteraction::click(&hint, 0);
        assert_eq!(interaction.kind, InlayHintInteractionKind::Click);
        assert!(interaction.has_command());
        assert_eq!(interaction.command.as_deref(), Some("goToDefinition"));
    }

    #[test]
    fn interaction_hover_no_command() {
        let hint = InlayHint::simple(1, 0, "param:", InlayHintKind::Parameter);
        let interaction = InlayHintInteraction::hover(&hint, 0);
        assert_eq!(interaction.kind, InlayHintInteractionKind::Hover);
        assert!(!interaction.has_command());
    }

    // -- InlayHintPadding tests --

    #[test]
    fn padding_from_hint() {
        let mut hint = InlayHint::simple(0, 0, "x", InlayHintKind::Type);
        hint.padding_left = true;
        hint.padding_right = false;
        let pad = InlayHintPadding::from_hint(&hint);
        assert_eq!(pad, InlayHintPadding::TYPE_HINT);
        assert_eq!(pad.total(), 1);
    }

    #[test]
    fn padding_constants() {
        assert_eq!(InlayHintPadding::NONE.total(), 0);
        assert_eq!(InlayHintPadding::BOTH.total(), 2);
    }

    // -- InlayHintTheme tests --

    #[test]
    fn theme_light_css() {
        let theme = InlayHintTheme::default_light();
        let css = theme.to_css();
        assert!(css.contains("#747474"));
        assert!(css.contains("0.9em"));
    }

    #[test]
    fn theme_dark_css() {
        let theme = InlayHintTheme::default_dark();
        let css = theme.to_css();
        assert!(css.contains("#969696"));
        assert!(css.contains("#3a3a3a"));
    }

    // -- InlayHintVisibility tests --

    #[test]
    fn visibility_all() {
        let vis = InlayHintVisibility::all();
        assert!(vis.is_visible(InlayHintKind::Type));
        assert!(vis.is_visible(InlayHintKind::Parameter));
        assert!(vis.is_visible(InlayHintKind::Other));
    }

    #[test]
    fn visibility_none() {
        let vis = InlayHintVisibility::none();
        assert!(!vis.is_visible(InlayHintKind::Type));
    }

    #[test]
    fn visibility_toggle() {
        let mut vis = InlayHintVisibility::all();
        vis.toggle(InlayHintKind::Type);
        assert!(!vis.show_types);
        vis.toggle(InlayHintKind::Type);
        assert!(vis.show_types);
    }

    #[test]
    fn visibility_filter() {
        let mut vis = InlayHintVisibility::all();
        vis.show_parameters = false;
        let hints = vec![
            InlayHint::simple(0, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(0, 5, "x:", InlayHintKind::Parameter),
        ];
        let filtered = vis.filter(&hints);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].kind, InlayHintKind::Type);
    }

    #[test]
    fn interaction_click_out_of_bounds() {
        let hint = InlayHint::simple(0, 0, "x", InlayHintKind::Type);
        let interaction = InlayHintInteraction::click(&hint, 5);
        assert!(!interaction.has_command());
    }

    #[test] fn anim_easing_linear() { assert!((AnimationEasing::Linear.apply(0.5) - 0.5).abs() < 1e-9); }
    #[test] fn anim_easing_ease_in() { assert!((AnimationEasing::EaseIn.apply(0.5) - 0.25).abs() < 1e-9); }
    #[test] fn anim_easing_bounds() { assert!((AnimationEasing::EaseOut.apply(0.0)).abs() < 1e-9); assert!((AnimationEasing::EaseOut.apply(1.0) - 1.0).abs() < 1e-9); }
    #[test] fn anim_easing_clamp() { assert!((AnimationEasing::EaseInOut.apply(-1.0)).abs() < 1e-9); }
    #[test] fn anim_tick() { let mut a = InlayHintAnimation::fade_in("h1", 100); a.tick(50); assert!(!a.is_complete()); a.tick(50); assert!(a.is_complete()); }
    #[test] fn anim_fade_out_init() { let a = InlayHintAnimation::fade_out("h1", 200); assert!((a.opacity - 1.0).abs() < 1e-9); }
    #[test] fn anim_reset_st() { let mut a = InlayHintAnimation::fade_in("h1", 100); a.tick(100); a.reset(); assert_eq!(a.elapsed_ms, 0); }
    #[test] fn anim_display() { assert!(format!("{}", InlayHintAnimation::fade_in("h1", 100)).contains("h1")); }
    #[test] fn ed_integ_update() { let mut e = InlayHintEditorIntegration::new("f"); e.update_hints(vec![InlayHint::simple(5,0,": i32",InlayHintKind::Type)]); assert_eq!(e.hint_count(), 1); }
    #[test] fn ed_integ_range() { let mut e = InlayHintEditorIntegration::new("f"); e.update_hints(vec![InlayHint::simple(5,0,"a",InlayHintKind::Type), InlayHint::simple(200,0,"b",InlayHintKind::Type)]); e.set_visible_range(0,100); assert_eq!(e.visible_hint_count(), 1); }
    #[test] fn ed_integ_toggle() { let mut e = InlayHintEditorIntegration::default(); e.set_enabled(false); assert!(!e.is_enabled()); }
    #[test] fn ed_integ_clear() { let mut e = InlayHintEditorIntegration::default(); e.update_hints(vec![InlayHint::simple(0,0,"x",InlayHintKind::Other)]); e.clear(); assert_eq!(e.hint_count(), 0); }
    #[test] fn ed_integ_display() { assert!(format!("{}", InlayHintEditorIntegration::default()).contains("untitled")); }


    #[test] fn inlayHintAnimation_cfg_default() {
        let c = InlayHintAnimationConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_refresh);
    }
    #[test] fn inlayHintAnimation_cfg_builder() {
        let c = InlayHintAnimationConfig::new().with_max_entries(500).with_auto_refresh(false);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_refresh);
    }
    #[test] fn inlayHintAnimation_cfg_labels() {
        let mut c = InlayHintAnimationConfig::new();
        c.set_label("x", "y");
        assert_eq!(c.get_label("x"), Some("y"));
    }
    #[test] fn inlayHintAnimation_cfg_refresh_due() {
        let c = InlayHintAnimationConfig::new();
        assert!(!c.is_refresh_due(1000));
        assert!(c.is_refresh_due(6000));
    }
    #[test] fn inlayHintAnimation_cfg_display() {
        assert!(format!("{}", InlayHintAnimationConfig::new()).contains("Config"));
    }
    #[test] fn inlayHintEditorIntegration_stats_success() {
        let mut st = InlayHintEditorIntegrationStats::new();
        st.record_success(10);
        st.record_success(20);
        st.record_failure(5);
        assert_eq!(st.total_operations, 3);
        assert!((st.success_rate() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn inlayHintEditorIntegration_stats_avg_dur() {
        let mut st = InlayHintEditorIntegrationStats::new();
        st.record_success(10);
        st.record_success(30);
        assert!((st.avg_duration_ms() - 20.0).abs() < 1e-9);
    }
    #[test] fn inlayHintEditorIntegration_stats_merge() {
        let mut a = InlayHintEditorIntegrationStats::new();
        a.record_success(10);
        let mut b = InlayHintEditorIntegrationStats::new();
        b.record_success(20);
        a.merge(&b);
        assert_eq!(a.total_operations, 2);
    }
    #[test] fn inlayHintEditorIntegration_stats_concurrent() {
        let mut st = InlayHintEditorIntegrationStats::new();
        st.update_concurrent(5);
        st.update_concurrent(3);
        assert_eq!(st.peak_concurrent, 5);
    }
    #[test] fn inlayHintEditorIntegration_stats_display() {
        assert!(format!("{}", InlayHintEditorIntegrationStats::new()).contains("Stats"));
    }
    #[test] fn inlayHintAnimation_event_new() {
        let e = InlayHintAnimationEvent::new(InlayHintAnimationEventKind::Created, 100);
        assert_eq!(e.kind, InlayHintAnimationEventKind::Created);
        assert!(!e.is_error());
    }
    #[test] fn inlayHintAnimation_event_detail() {
        let e = InlayHintAnimationEvent::new(InlayHintAnimationEventKind::Error, 0).with_detail("oops");
        assert!(e.is_error());
        assert_eq!(e.detail.unwrap(), "oops");
    }
    #[test] fn inlayHintAnimation_event_display() {
        let e = InlayHintAnimationEvent::new(InlayHintAnimationEventKind::Updated, 50);
        assert!(format!("{}", e).contains("updated"));
    }
    #[test] fn inlayHintAnimation_event_kind_display() {
        assert_eq!(format!("{}", InlayHintAnimationEventKind::Refreshed), "refreshed");
    }

    // -- InlayHintLayoutCalculator tests --

    #[test]
    fn layout_x_offset_no_existing() {
        let calc = InlayHintLayoutCalculator::new(2);
        assert_eq!(calc.x_offset(10, 0), 10);
    }

    #[test]
    fn layout_x_offset_with_existing() {
        let calc = InlayHintLayoutCalculator::new(2);
        assert_eq!(calc.x_offset(10, 5), 17);
    }

    #[test]
    fn layout_total_width() {
        let calc = InlayHintLayoutCalculator::new(2);
        assert_eq!(calc.total_width_on_line(&[3, 4, 5]), 16);
    }

    #[test]
    fn layout_total_width_empty() {
        let calc = InlayHintLayoutCalculator::new(2);
        assert_eq!(calc.total_width_on_line(&[]), 0);
    }

    // -- InlayHintPredicateFilter tests --

    #[test]
    fn predicate_filter_by_kind() {
        let hints = vec![
            InlayHint::simple(1, 0, ": i32", InlayHintKind::Type),
            InlayHint::simple(2, 0, "name:", InlayHintKind::Parameter),
        ];
        let f = InlayHintPredicateFilter::new().with_kind(InlayHintKind::Type);
        assert_eq!(f.filtered_count(&hints), 1);
    }

    #[test]
    fn predicate_filter_by_line_range() {
        let hints = vec![
            InlayHint::simple(5, 0, "a", InlayHintKind::Type),
            InlayHint::simple(15, 0, "b", InlayHintKind::Type),
        ];
        let f = InlayHintPredicateFilter::new().with_line_range(1, 10);
        assert_eq!(f.filtered_count(&hints), 1);
    }

    #[test]
    fn predicate_filter_by_min_label_length() {
        let hints = vec![
            InlayHint::simple(1, 0, "ab", InlayHintKind::Type),
            InlayHint::simple(2, 0, "abcdef", InlayHintKind::Type),
        ];
        let f = InlayHintPredicateFilter::new().with_min_label_length(4);
        assert_eq!(f.filtered_count(&hints), 1);
    }

    #[test]
    fn predicate_filter_matches_all() {
        let hint = InlayHint::simple(5, 0, ": i32", InlayHintKind::Type);
        let f = InlayHintPredicateFilter::new();
        assert!(f.matches(&hint));
    }

    // -- InlayHintVersionedCache tests --

    #[test]
    fn versioned_cache_set_and_get() {
        let mut c = InlayHintVersionedCache::new();
        c.set("a.rs", 1, vec![InlayHint::simple(1, 0, "x", InlayHintKind::Type)]);
        assert!(c.get("a.rs", 1).is_some());
        assert!(c.get("a.rs", 2).is_none());
    }

    #[test]
    fn versioned_cache_invalidate() {
        let mut c = InlayHintVersionedCache::new();
        c.set("a.rs", 1, vec![]);
        assert!(c.invalidate("a.rs"));
        assert_eq!(c.cache_size(), 0);
    }

    #[test]
    fn versioned_cache_evict_oldest() {
        let mut c = InlayHintVersionedCache::new();
        c.set("a.rs", 1, vec![]);
        c.set("b.rs", 2, vec![]);
        c.evict_oldest();
        assert_eq!(c.cache_size(), 1);
        assert!(c.get("b.rs", 2).is_some());
    }

    #[test]
    fn versioned_cache_invalidate_all() {
        let mut c = InlayHintVersionedCache::new();
        c.set("a.rs", 1, vec![]);
        c.set("b.rs", 2, vec![]);
        c.invalidate_all();
        assert_eq!(c.cache_size(), 0);
    }


    // -- inlayhints additional tests -------------------------------------------

    #[test]
    fn x_inlayhints_text_span_new_ordered() {
        let s = XInlayhintsTextSpan::new(5, 10);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn x_inlayhints_text_span_new_reversed() {
        let s = XInlayhintsTextSpan::new(10, 5);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn x_inlayhints_text_span_len() {
        assert_eq!(XInlayhintsTextSpan::new(3, 7).len(), 4);
        assert_eq!(XInlayhintsTextSpan::new(0, 0).len(), 0);
    }

    #[test]
    fn x_inlayhints_text_span_extract() {
        let s = XInlayhintsTextSpan::new(0, 5);
        assert_eq!(s.extract("hello world"), "hello");
    }

    #[test]
    fn x_inlayhints_text_span_contains() {
        let s = XInlayhintsTextSpan::new(2, 8);
        assert!(s.contains(2));
        assert!(s.contains(7));
        assert!(!s.contains(8));
    }

    #[test]
    fn x_inlayhints_text_span_intersect() {
        let a = XInlayhintsTextSpan::new(0, 10);
        let b = XInlayhintsTextSpan::new(5, 15);
        let inter = a.intersect(&b).unwrap();
        assert_eq!(inter.start, 5);
        assert_eq!(inter.end, 10);
    }

    #[test]
    fn x_inlayhints_text_span_intersect_none() {
        let a = XInlayhintsTextSpan::new(0, 5);
        let b = XInlayhintsTextSpan::new(5, 10);
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn x_inlayhints_text_span_union() {
        let a = XInlayhintsTextSpan::new(3, 7);
        let b = XInlayhintsTextSpan::new(5, 12);
        let u = a.union(&b);
        assert_eq!(u.start, 3);
        assert_eq!(u.end, 12);
    }

    #[test]
    fn x_inlayhints_count_lines_basic() {
        assert_eq!(x_inlayhints_count_lines("a\nb\nc"), 3);
        assert_eq!(x_inlayhints_count_lines(""), 0);
        assert_eq!(x_inlayhints_count_lines("single"), 1);
    }

    #[test]
    fn x_inlayhints_line_start_offset_basic() {
        assert_eq!(x_inlayhints_line_start_offset("abc\ndef\nghi", 0), Some(0));
        assert_eq!(x_inlayhints_line_start_offset("abc\ndef\nghi", 1), Some(4));
        assert_eq!(x_inlayhints_line_start_offset("abc\ndef\nghi", 2), Some(8));
        assert_eq!(x_inlayhints_line_start_offset("abc\ndef\nghi", 3), None);
    }

    #[test]
    fn x_inlayhints_indent_level_basic() {
        assert_eq!(x_inlayhints_indent_level("    hello"), 4);
        assert_eq!(x_inlayhints_indent_level("hello"), 0);
        assert_eq!(x_inlayhints_indent_level("  "), 2);
    }

    #[test]
    fn x_inlayhints_trim_trailing_basic() {
        let input = "hello   \nworld  \n  foo  ";
        let result = x_inlayhints_trim_trailing(input);
        assert_eq!(result, "hello\nworld\n  foo");
    }

    #[test]
    fn x_inlayhints_detect_eol_lf() {
        assert_eq!(x_inlayhints_detect_eol("a\nb\nc"), "\n");
    }

    #[test]
    fn x_inlayhints_detect_eol_crlf() {
        assert_eq!(x_inlayhints_detect_eol("a\r\nb\r\nc"), "\r\n");
    }

    #[test]
    fn x_inlayhints_tokenize_basic() {
        let tokens = x_inlayhints_tokenize("hello, world! foo");
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn x_inlayhints_text_span_shift() {
        let s = XInlayhintsTextSpan::new(2, 5).shift(10);
        assert_eq!(s.start, 12);
        assert_eq!(s.end, 15);
    }


    // -- inlayhints extended domain tests ----------------------------------------

    #[test]
    fn y_inlayhints_enum_index() {
        assert_eq!(YInlayhintsInlayHintPriority::Low.index(), 0);
        assert_eq!(YInlayhintsInlayHintPriority::Normal.index(), 1);
        assert_eq!(YInlayhintsInlayHintPriority::High.index(), 2);
        assert_eq!(YInlayhintsInlayHintPriority::Critical.index(), 3);
    }

    #[test]
    fn y_inlayhints_enum_label() {
        assert_eq!(YInlayhintsInlayHintPriority::Low.label(), "Low");
        assert_eq!(YInlayhintsInlayHintPriority::Normal.label(), "Normal");
        assert_eq!(YInlayhintsInlayHintPriority::High.label(), "High");
        assert_eq!(YInlayhintsInlayHintPriority::Critical.label(), "Critical");
    }

    #[test]
    fn y_inlayhints_enum_all() {
        let all = YInlayhintsInlayHintPriority::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_inlayhints_enum_is_default() {
        assert!(YInlayhintsInlayHintPriority::Low.is_default());
        assert!(!YInlayhintsInlayHintPriority::Critical.is_default());
    }

    #[test]
    fn y_inlayhints_enum_display() {
        assert_eq!(format!("{}", YInlayhintsInlayHintPriority::Low), "Low");
    }

    #[test]
    fn y_inlayhints_struct_new() {
        let s = YInlayhintsInlayHintCache::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_inlayhints_struct_clear() {
        let mut s = YInlayhintsInlayHintCache::new();
        s.hints.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_inlayhints_fingerprint_deterministic() {
        let h1 = y_inlayhints_fingerprint("hello");
        let h2 = y_inlayhints_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_inlayhints_fingerprint("a"), y_inlayhints_fingerprint("b"));
    }

    #[test]
    fn y_inlayhints_truncate_short() {
        assert_eq!(y_inlayhints_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_inlayhints_truncate_long() {
        let r = y_inlayhints_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_inlayhints_normalize_key_basic() {
        assert_eq!(y_inlayhints_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_inlayhints_split_path_basic() {
        let parts = y_inlayhints_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_inlayhints_count_occurrences_basic() {
        assert_eq!(y_inlayhints_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_inlayhints_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_inlayhints_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_inlayhints_in_range_basic() {
        assert!(y_inlayhints_in_range(5, 1, 10));
        assert!(y_inlayhints_in_range(1, 1, 10));
        assert!(y_inlayhints_in_range(10, 1, 10));
        assert!(!y_inlayhints_in_range(0, 1, 10));
        assert!(!y_inlayhints_in_range(11, 1, 10));
    }

    #[test]
    fn y_inlayhints_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_inlayhints_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_inlayhints_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_inlayhints_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- inlayhints Z-extended tests -----------------------------------------------

    #[test]
    fn z_inlayhints_priority_weight() {
        assert_eq!(ZInlayhintsPriority::Idle.weight(), 0);
        assert_eq!(ZInlayhintsPriority::Normal.weight(), 2);
        assert_eq!(ZInlayhintsPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_inlayhints_priority_label() {
        assert_eq!(ZInlayhintsPriority::Low.label(), "low");
        assert_eq!(ZInlayhintsPriority::High.label(), "high");
    }

    #[test]
    fn z_inlayhints_priority_is_elevated() {
        assert!(!ZInlayhintsPriority::Normal.is_elevated());
        assert!(ZInlayhintsPriority::High.is_elevated());
        assert!(ZInlayhintsPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_inlayhints_priority_display() {
        assert_eq!(format!("{}", ZInlayhintsPriority::Idle), "idle");
    }

    #[test]
    fn z_inlayhints_priority_all_asc() {
        let all = ZInlayhintsPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZInlayhintsPriority::Idle);
        assert_eq!(all[4], ZInlayhintsPriority::Realtime);
    }

    #[test]
    fn z_inlayhints_struct_new() {
        let s = ZInlayhintsInlayHintThrottler::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_inlayhints_struct_toggled_clone() {
        let s = ZInlayhintsInlayHintThrottler::new();
        let t = s.toggled_clone();
        assert_ne!(s.active, t.active);
    }

    #[test]
    fn z_inlayhints_rolling_hash_deterministic() {
        let h1 = z_inlayhints_rolling_hash(b"test");
        let h2 = z_inlayhints_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_inlayhints_rolling_hash(b"a"), z_inlayhints_rolling_hash(b"b"));
    }

    #[test]
    fn z_inlayhints_pad_to_basic() {
        assert_eq!(z_inlayhints_pad_to("hi", 5), "hi   ");
        assert_eq!(z_inlayhints_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_inlayhints_is_identifier_basic() {
        assert!(z_inlayhints_is_identifier("foo_bar"));
        assert!(z_inlayhints_is_identifier("abc123"));
        assert!(!z_inlayhints_is_identifier(""));
        assert!(!z_inlayhints_is_identifier("has space"));
    }

    #[test]
    fn z_inlayhints_levenshtein_basic() {
        assert_eq!(z_inlayhints_levenshtein("", ""), 0);
        assert_eq!(z_inlayhints_levenshtein("abc", "abc"), 0);
        assert_eq!(z_inlayhints_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_inlayhints_unique_words_basic() {
        let w = z_inlayhints_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_inlayhints_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_inlayhints_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_inlayhints_common_prefix_basic() {
        assert_eq!(z_inlayhints_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_inlayhints_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_inlayhints_struct_clear() {
        let mut s = ZInlayhintsInlayHintThrottler::new();
        s.pending_ranges.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_inlayhints_rolling_hash_empty() {
        let h = z_inlayhints_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_65_push_and_len() {
        let mut rb = super::XbRingBuffer65::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_65_overwrite() {
        let mut rb = super::XbRingBuffer65::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_65_get_out_of_bounds() {
        let rb = super::XbRingBuffer65::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_65_drain_all() {
        let mut rb = super::XbRingBuffer65::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_65_peek_front_back() {
        let mut rb = super::XbRingBuffer65::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_65_clear() {
        let mut rb = super::XbRingBuffer65::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_65_capacity() {
        let rb = super::XbRingBuffer65::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_65_basic() {
        let h = super::xb_fnv1a_65(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_65(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_65_different_inputs() {
        let h1 = super::xb_fnv1a_65(b"abc");
        let h2 = super::xb_fnv1a_65(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_65_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_65(&data);
        let dec = super::xb_rle_decode_65(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_65_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_65(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_65(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_65_values() {
        assert!((super::xb_clamp_65(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_65(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_65(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_65_values() {
        assert!((super::xb_lerp_65(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_65(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_65(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_65_wrap_around_twice() {
        let mut rb = super::XbRingBuffer65::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 93 ----

    #[test]
    fn xc_93_pool_new_empty() {
        let pool: super::Xc93Pool<i32> = super::Xc93Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_93_pool_release_acquire() {
        let mut pool = super::Xc93Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_93_pool_acquire_empty() {
        let mut pool: super::Xc93Pool<i32> = super::Xc93Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_93_pool_full() {
        let mut pool = super::Xc93Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_93_pool_drain() {
        let mut pool = super::Xc93Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_93_pool_stats() {
        let mut pool = super::Xc93Pool::new(8);
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
    fn xc_93_pool_clear() {
        let mut pool = super::Xc93Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_93_pool_shrink() {
        let mut pool = super::Xc93Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_93_pool_default() {
        let pool: super::Xc93Pool<String> = super::Xc93Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_93_pool_extend() {
        let mut pool = super::Xc93Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_93_pool_retain() {
        let mut pool = super::Xc93Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_93_scheduler_round_robin() {
        let mut sched = super::Xc93Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_93_scheduler_empty() {
        let mut sched = super::Xc93Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_93_scheduler_reset() {
        let mut sched = super::Xc93Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_93_scheduler_add_remove() {
        let mut sched = super::Xc93Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_93_scheduler_targets() {
        let sched = super::Xc93Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_93_hash_empty() {
        assert_eq!(super::xc_93_hash(b""), 5381);
    }

    #[test]
    fn xc_93_hash_data() {
        let h = super::xc_93_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_93_hash(b"hello"), h);
    }

    #[test]
    fn xc_93_reverse_str() {
        assert_eq!(super::xc_93_reverse("abc"), "cba");
        assert_eq!(super::xc_93_reverse(""), "");
    }


    #[test]
    fn xe_78_pipeline_empty() {
        let p = super::Xe78Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_78_pipeline_parse_stage() {
        let p = super::Xe78Pipeline::new()
            .add_parse(super::xe_78_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_78_pipeline_transform_double() {
        let p = super::Xe78Pipeline::new()
            .add_transform(super::xe_78_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_78_pipeline_validate_reverse() {
        let p = super::Xe78Pipeline::new()
            .add_validate(super::xe_78_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_78_pipeline_emit_filter() {
        let p = super::Xe78Pipeline::new()
            .add_emit(super::xe_78_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_78_pipeline_multi_stage() {
        let p = super::Xe78Pipeline::new()
            .add_parse(super::xe_78_pipeline_identity)
            .add_transform(super::xe_78_pipeline_double)
            .add_validate(super::xe_78_pipeline_reverse)
            .add_emit(super::xe_78_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_78_pipeline_error_propagation() {
        let p = super::Xe78Pipeline::new()
            .add_parse(super::xe_78_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe78Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_78_pipeline_compose() {
        let p1 = super::Xe78Pipeline::new()
            .add_parse(super::xe_78_pipeline_identity);
        let p2 = super::Xe78Pipeline::new()
            .add_transform(super::xe_78_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_78_pipeline_error_display() {
        let e = super::Xe78PipelineError {
            stage: super::Xe78Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_78_cache_put_get() {
        let mut c = super::Xe78Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_78_cache_miss() {
        let mut c: super::Xe78Cache<&str, i32> = super::Xe78Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_78_cache_ttl_expiry() {
        let mut c = super::Xe78Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_78_cache_evict() {
        let mut c = super::Xe78Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_78_cache_capacity() {
        let mut c = super::Xe78Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_78_cache_stats() {
        let mut c = super::Xe78Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_78_cache_clear() {
        let mut c = super::Xe78Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_76 graph tests ------------------------------------------------

    #[test]
    fn xg_76_graph_empty() {
        let g = super::Xg76Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_76_graph_add_node() {
        let mut g = super::Xg76Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_76_graph_add_edge() {
        let mut g = super::Xg76Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_76_graph_neighbors() {
        let mut g = super::Xg76Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_76_graph_has_path() {
        let mut g = super::Xg76Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_76_graph_self_path() {
        let g = super::Xg76Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_76_graph_topo_sort() {
        let mut g = super::Xg76Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_76_graph_cycle_detect_false() {
        let mut g = super::Xg76Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_76_graph_cycle_detect_true() {
        let mut g = super::Xg76Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_76 heap tests -------------------------------------------------

    #[test]
    fn xg_76_heap_empty() {
        let h: super::Xg76Heap<i32> = super::Xg76Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_76_heap_push_pop() {
        let mut h = super::Xg76Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_76_heap_peek() {
        let mut h = super::Xg76Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_76_heap_drain_sorted() {
        let mut h = super::Xg76Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_76_heap_merge() {
        let mut a = super::Xg76Heap::new();
        let mut b = super::Xg76Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_76_heap_default() {
        let h: super::Xg76Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_76_graph_default() {
        let g: super::Xg76Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh92_skip_insert_contains() {
        let mut sl = super::Xh92SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh92_skip_remove() {
        let mut sl = super::Xh92SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh92_skip_len() {
        let mut sl = super::Xh92SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh92_skip_range_query() {
        let mut sl = super::Xh92SkipList::xh_new(4);
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
    fn xh92_skip_floor_ceiling() {
        let mut sl = super::Xh92SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh92_skip_rank() {
        let mut sl = super::Xh92SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh92_skip_empty() {
        let sl = super::Xh92SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh92_skip_duplicates() {
        let mut sl = super::Xh92SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh92_bitset_set_test() {
        let mut bs = super::Xh92BitSet::xh_new(256);
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
    fn xh92_bitset_clear_count() {
        let mut bs = super::Xh92BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh92_bitset_and_or_xor() {
        let mut a = super::Xh92BitSet::xh_new(128);
        let mut b = super::Xh92BitSet::xh_new(128);
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
    fn xh92_bitset_iter_ones() {
        let mut bs = super::Xh92BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh92_bitset_first_last() {
        let mut bs = super::Xh92BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh92_bitset_empty() {
        let bs = super::Xh92BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi92_deque_push_pop_back() {
        let mut dq = super::Xi92Deque::xi_new(4);
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
    fn xi92_deque_push_pop_front() {
        let mut dq = super::Xi92Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi92_deque_mixed_ops() {
        let mut dq = super::Xi92Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi92_deque_get_and_split() {
        let mut dq = super::Xi92Deque::xi_new(8);
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
    fn xi92_deque_rotate_left() {
        let mut dq = super::Xi92Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi92_deque_rotate_right() {
        let mut dq = super::Xi92Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi92_deque_grow() {
        let mut dq = super::Xi92Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi92_deque_empty() {
        let dq = super::Xi92Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi92_interval_tree_insert_query() {
        let mut tree = super::Xi92IntervalTree::xi_new();
        tree.xi_insert(super::Xi92Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi92Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi92Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi92_interval_tree_overlap() {
        let mut tree = super::Xi92IntervalTree::xi_new();
        tree.xi_insert(super::Xi92Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi92Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi92Interval::xi_new(12, 20));
        let q = super::Xi92Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi92_interval_tree_remove() {
        let mut tree = super::Xi92IntervalTree::xi_new();
        tree.xi_insert(super::Xi92Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi92Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi92_interval_tree_gaps() {
        let mut tree = super::Xi92IntervalTree::xi_new();
        tree.xi_insert(super::Xi92Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi92Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi92Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi92Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi92Interval::xi_new(8, 10));
    }

    #[test]
    fn xi92_interval_tree_merge() {
        let mut tree = super::Xi92IntervalTree::xi_new();
        tree.xi_insert(super::Xi92Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi92Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi92Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi92Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi92Interval::xi_new(10, 15));
    }

    #[test]
    fn xi92_interval_tree_all() {
        let mut tree = super::Xi92IntervalTree::xi_new();
        tree.xi_insert(super::Xi92Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi92Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi92_interval_tree_empty() {
        let tree = super::Xi92IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi92_interval_tree_contains_point() {
        let iv = super::Xi92Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
