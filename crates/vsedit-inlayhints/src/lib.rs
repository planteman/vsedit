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
}
