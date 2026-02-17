//! Ext API: Decorations.
//!
//! RPC bridge between the extension host and the main thread for editor decorations.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_decorations";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DecorationMessage {
    RegisterType {
        key: String,
        options: DecorationRenderOptions,
    },
    UnregisterType {
        key: String,
    },
    SetDecorations {
        key: String,
        uri: String,
        ranges: Vec<DecorationOptions>,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecorationRenderOptions {
    pub background_color: Option<String>,
    pub border: Option<String>,
    pub color: Option<String>,
    pub font_style: Option<String>,
    pub font_weight: Option<String>,
    pub is_whole_line: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecorationType {
    pub key: String,
    pub options: DecorationRenderOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecorationOptions {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
    pub hover_message: Option<String>,
}

// ── Bridge ──

pub struct DecorationBridge {
    types: Vec<DecorationType>,
    applied: Vec<(String, String, Vec<DecorationOptions>)>,
}

impl DecorationBridge {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            applied: Vec::new(),
        }
    }

    pub fn register_type(&mut self, key: &str, options: DecorationRenderOptions) {
        if !self.types.iter().any(|t| t.key == key) {
            self.types.push(DecorationType {
                key: key.to_string(),
                options,
            });
        }
    }

    pub fn unregister_type(&mut self, key: &str) {
        self.types.retain(|t| t.key != key);
        self.applied.retain(|(k, _, _)| k != key);
    }

    pub fn set_decorations(&mut self, key: &str, uri: &str, ranges: Vec<DecorationOptions>) {
        self.applied.retain(|(k, u, _)| !(k == key && u == uri));
        if !ranges.is_empty() {
            self.applied.push((key.to_string(), uri.to_string(), ranges));
        }
    }

    pub fn has_type(&self, key: &str) -> bool {
        self.types.iter().any(|t| t.key == key)
    }

    pub fn handle_message(&mut self, msg: &DecorationMessage) -> serde_json::Value {
        match msg {
            DecorationMessage::RegisterType { key, options } => {
                self.register_type(key, options.clone());
                serde_json::json!({"registered": true})
            }
            DecorationMessage::UnregisterType { key } => {
                self.unregister_type(key);
                serde_json::json!({"unregistered": true})
            }
            DecorationMessage::SetDecorations { key, uri, ranges } => {
                self.set_decorations(key, uri, ranges.clone());
                serde_json::json!({"set": ranges.len()})
            }
        }
    }
}

impl Default for DecorationBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ── Error Types ──

/// Errors that can occur when working with decorations.
#[derive(Debug, Clone, PartialEq)]
pub enum DecorationError {
    /// The decoration type key was not registered.
    UnknownType(String),
    /// A decoration range is invalid (start after end).
    InvalidRange {
        start_line: u32,
        start_character: u32,
        end_line: u32,
        end_character: u32,
    },
    /// The decoration type key is empty.
    EmptyKey,
    /// The URI is empty or invalid.
    InvalidUri(String),
}

impl fmt::Display for DecorationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownType(key) => write!(f, "unknown decoration type: {key}"),
            Self::InvalidRange {
                start_line,
                start_character,
                end_line,
                end_character,
            } => write!(
                f,
                "invalid range: ({start_line}:{start_character}) > ({end_line}:{end_character})"
            ),
            Self::EmptyKey => write!(f, "decoration type key must not be empty"),
            Self::InvalidUri(uri) => write!(f, "invalid uri: {uri}"),
        }
    }
}

impl std::error::Error for DecorationError {}

// ── Display implementations ──

impl fmt::Display for DecorationRenderOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RenderOptions(whole_line={}", self.is_whole_line)?;
        if let Some(bg) = &self.background_color {
            write!(f, ", bg={bg}")?;
        }
        if let Some(c) = &self.color {
            write!(f, ", color={c}")?;
        }
        write!(f, ")")
    }
}

impl fmt::Display for DecorationOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}-{}:{}",
            self.start_line, self.start_character, self.end_line, self.end_character
        )
    }
}

impl fmt::Display for DecorationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DecorationType({}, {})", self.key, self.options)
    }
}

// ── Builder for DecorationRenderOptions ──

/// Fluent builder for constructing `DecorationRenderOptions`.
#[derive(Debug, Clone, Default)]
pub struct RenderOptionsBuilder {
    background_color: Option<String>,
    border: Option<String>,
    color: Option<String>,
    font_style: Option<String>,
    font_weight: Option<String>,
    is_whole_line: bool,
}

impl RenderOptionsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn background_color(mut self, color: impl Into<String>) -> Self {
        self.background_color = Some(color.into());
        self
    }

    pub fn border(mut self, border: impl Into<String>) -> Self {
        self.border = Some(border.into());
        self
    }

    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn font_style(mut self, style: impl Into<String>) -> Self {
        self.font_style = Some(style.into());
        self
    }

    pub fn font_weight(mut self, weight: impl Into<String>) -> Self {
        self.font_weight = Some(weight.into());
        self
    }

    pub fn whole_line(mut self, whole: bool) -> Self {
        self.is_whole_line = whole;
        self
    }

    pub fn build(self) -> DecorationRenderOptions {
        DecorationRenderOptions {
            background_color: self.background_color,
            border: self.border,
            color: self.color,
            font_style: self.font_style,
            font_weight: self.font_weight,
            is_whole_line: self.is_whole_line,
        }
    }
}

// ── Validation & helpers ──

impl DecorationOptions {
    /// Validate that the range is well-formed (start <= end).
    pub fn validate(&self) -> Result<(), DecorationError> {
        if self.start_line > self.end_line
            || (self.start_line == self.end_line
                && self.start_character > self.end_character)
        {
            return Err(DecorationError::InvalidRange {
                start_line: self.start_line,
                start_character: self.start_character,
                end_line: self.end_line,
                end_character: self.end_character,
            });
        }
        Ok(())
    }

    /// Returns the number of lines this decoration spans (at least 1).
    pub fn line_span(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Returns true if this decoration covers only a single line.
    pub fn is_single_line(&self) -> bool {
        self.start_line == self.end_line
    }

    /// Returns true if this range overlaps with `other`.
    pub fn overlaps(&self, other: &DecorationOptions) -> bool {
        if self.end_line < other.start_line || self.start_line > other.end_line {
            return false;
        }
        if self.end_line == other.start_line
            && self.end_character <= other.start_character
        {
            return false;
        }
        if self.start_line == other.end_line
            && self.start_character >= other.end_character
        {
            return false;
        }
        true
    }
}

impl DecorationBridge {
    /// Validate a key before use.
    fn validate_key(key: &str) -> Result<(), DecorationError> {
        if key.is_empty() {
            return Err(DecorationError::EmptyKey);
        }
        Ok(())
    }

    /// Register a type with validation.
    pub fn try_register_type(
        &mut self,
        key: &str,
        options: DecorationRenderOptions,
    ) -> Result<bool, DecorationError> {
        Self::validate_key(key)?;
        if self.types.iter().any(|t| t.key == key) {
            return Ok(false);
        }
        self.types.push(DecorationType {
            key: key.to_string(),
            options,
        });
        Ok(true)
    }

    /// Set decorations with validation: the type must be registered and
    /// all ranges must be well-formed.
    pub fn try_set_decorations(
        &mut self,
        key: &str,
        uri: &str,
        ranges: Vec<DecorationOptions>,
    ) -> Result<usize, DecorationError> {
        Self::validate_key(key)?;
        if !self.has_type(key) {
            return Err(DecorationError::UnknownType(key.to_string()));
        }
        if uri.is_empty() {
            return Err(DecorationError::InvalidUri(uri.to_string()));
        }
        for r in &ranges {
            r.validate()?;
        }
        let count = ranges.len();
        self.set_decorations(key, uri, ranges);
        Ok(count)
    }

    /// Return how many decoration types are registered.
    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    /// Return how many (key, uri) decoration sets are currently applied.
    pub fn applied_count(&self) -> usize {
        self.applied.len()
    }

    /// Total number of individual decoration ranges across all applied sets.
    pub fn total_ranges(&self) -> usize {
        self.applied.iter().map(|(_, _, r)| r.len()).sum()
    }

    /// Get all decoration ranges applied for a given URI.
    pub fn decorations_for_uri(&self, uri: &str) -> Vec<(&str, &[DecorationOptions])> {
        self.applied
            .iter()
            .filter(|(_, u, _)| u == uri)
            .map(|(k, _, r)| (k.as_str(), r.as_slice()))
            .collect()
    }

    /// Lookup the render options for a given decoration type key.
    pub fn get_render_options(&self, key: &str) -> Option<&DecorationRenderOptions> {
        self.types.iter().find(|t| t.key == key).map(|t| &t.options)
    }

    /// Remove all applied decorations for a given URI across all types.
    pub fn clear_uri(&mut self, uri: &str) {
        self.applied.retain(|(_, u, _)| u != uri);
    }

    /// Returns `true` when no decoration types are registered.
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Lookup a registered decoration type by key.
    pub fn get_type(&self, key: &str) -> Option<&DecorationType> {
        self.types.iter().find(|t| t.key == key)
    }

    /// Remove all registered types and applied decorations.
    pub fn clear_all(&mut self) {
        self.types.clear();
        self.applied.clear();
    }
}

impl fmt::Display for DecorationBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.types.len();
        if n == 1 {
            write!(f, "1 decoration type")
        } else {
            write!(f, "{n} decoration types")
        }
    }
}

impl DecorationRenderOptions {
    /// Returns `true` if a background color is set.
    pub fn has_background(&self) -> bool {
        self.background_color.is_some()
    }

    /// Returns `true` if a border is set.
    pub fn has_border(&self) -> bool {
        self.border.is_some()
    }
}

impl DecorationType {
    /// Returns `true` if this decoration type applies to whole lines.
    pub fn is_whole_line(&self) -> bool {
        self.options.is_whole_line
    }
}

/// Initialize the decorations extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Accumulated statistics for ext-decorations operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtDecorationsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtDecorationsStats {
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
    pub fn merge(&mut self, other: &ExtDecorationsStats) {
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

impl Default for ExtDecorationsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtDecorationsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtDecorationsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-decorations.
#[derive(Debug, Clone)]
pub struct ExtDecorationsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtDecorationsValidator {
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

impl Default for ExtDecorationsValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Extended decoration options — after/before content, overview ruler
// ---------------------------------------------------------------------------

/// Content rendered inline after or before the decorated range.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemableDecorationAttachment {
    /// Text to render.
    pub content_text: Option<String>,
    /// Foreground color.
    pub color: Option<String>,
    /// Background color.
    pub background_color: Option<String>,
    /// Font style (e.g. "italic").
    pub font_style: Option<String>,
}

impl Default for ThemableDecorationAttachment {
    fn default() -> Self {
        Self {
            content_text: None,
            color: None,
            background_color: None,
            font_style: None,
        }
    }
}

/// Overview ruler lane for decoration markers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OverviewRulerLane {
    Left,
    Center,
    Right,
    Full,
}

/// Overview ruler configuration for a decoration type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OverviewRulerOptions {
    pub color: String,
    pub lane: OverviewRulerLane,
}

/// Extended decoration options with after/before content and overview ruler.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtendedDecorationOptions {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
    pub hover_message: Option<String>,
    pub after_content: Option<ThemableDecorationAttachment>,
    pub before_content: Option<ThemableDecorationAttachment>,
}

/// Extended render options including after/before and overview ruler.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtendedRenderOptions {
    #[serde(flatten)]
    pub base: DecorationRenderOptions,
    pub after: Option<ThemableDecorationAttachment>,
    pub before: Option<ThemableDecorationAttachment>,
    pub overview_ruler: Option<OverviewRulerOptions>,
}

/// A fully-typed decoration type with a unique key and extended options.
#[derive(Debug, Clone, PartialEq)]
pub struct TextEditorDecorationType {
    pub key: String,
    pub options: ExtendedRenderOptions,
}

impl TextEditorDecorationType {
    pub fn new(key: impl Into<String>, options: ExtendedRenderOptions) -> Self {
        Self {
            key: key.into(),
            options,
        }
    }
}

impl fmt::Display for TextEditorDecorationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TextEditorDecorationType({})", self.key)
    }
}

// ── DecorationPriority ──

/// Priority ordering for overlapping decorations. Higher values take precedence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DecorationPriority {
    pub priority: i32,
    pub key: String,
}

impl DecorationPriority {
    pub fn new(key: impl Into<String>, priority: i32) -> Self {
        Self { priority, key: key.into() }
    }
}

impl fmt::Display for DecorationPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(priority={})", self.key, self.priority)
    }
}

/// Registry of decoration priorities for resolving overlaps.
#[derive(Debug, Clone, Default)]
pub struct DecorationPriorityRegistry {
    entries: Vec<DecorationPriority>,
}

impl DecorationPriorityRegistry {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn register(&mut self, key: impl Into<String>, priority: i32) {
        let key = key.into();
        if let Some(existing) = self.entries.iter_mut().find(|e| e.key == key) {
            existing.priority = priority;
        } else {
            self.entries.push(DecorationPriority::new(key, priority));
        }
    }

    /// Returns keys sorted by priority (highest first).
    pub fn sorted_keys(&self) -> Vec<String> {
        let mut sorted = self.entries.clone();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted.into_iter().map(|e| e.key).collect()
    }

    pub fn get_priority(&self, key: &str) -> Option<i32> {
        self.entries.iter().find(|e| e.key == key).map(|e| e.priority)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ── DecorationMerger ──

/// Merge overlapping decoration ranges of the same type.
pub struct DecorationMerger;

impl DecorationMerger {
    /// Merge a list of `DecorationOptions` by combining overlapping ranges.
    /// Assumes all ranges are on the same URI and same decoration type.
    pub fn merge(mut ranges: Vec<DecorationOptions>) -> Vec<DecorationOptions> {
        if ranges.len() <= 1 {
            return ranges;
        }
        ranges.sort_by(|a, b| {
            a.start_line.cmp(&b.start_line)
                .then(a.start_character.cmp(&b.start_character))
        });
        let mut merged: Vec<DecorationOptions> = Vec::new();
        for r in ranges {
            if let Some(last) = merged.last_mut() {
                if r.start_line < last.end_line
                    || (r.start_line == last.end_line && r.start_character <= last.end_character)
                {
                    // Extend the current range
                    if r.end_line > last.end_line
                        || (r.end_line == last.end_line && r.end_character > last.end_character)
                    {
                        last.end_line = r.end_line;
                        last.end_character = r.end_character;
                    }
                    // Merge hover messages
                    if last.hover_message.is_none() {
                        last.hover_message = r.hover_message;
                    }
                    continue;
                }
            }
            merged.push(r);
        }
        merged
    }
}

// ── DecorationAnimator ──

/// Animate decoration transitions between two states.
#[derive(Debug, Clone)]
pub struct DecorationAnimator {
    pub from_color: Option<String>,
    pub to_color: Option<String>,
    pub steps: u32,
}

impl DecorationAnimator {
    pub fn new(steps: u32) -> Self {
        Self {
            from_color: None,
            to_color: None,
            steps: steps.max(1),
        }
    }

    pub fn from_color(mut self, color: impl Into<String>) -> Self {
        self.from_color = Some(color.into());
        self
    }

    pub fn to_color(mut self, color: impl Into<String>) -> Self {
        self.to_color = Some(color.into());
        self
    }

    /// Generate intermediate render options for each animation step.
    /// Returns `steps` render options interpolating between from and to.
    pub fn generate_steps(&self) -> Vec<DecorationRenderOptions> {
        (0..self.steps)
            .map(|i| {
                let progress = if self.steps <= 1 { 1.0 } else { i as f64 / (self.steps - 1) as f64 };
                let bg = if progress < 0.5 {
                    self.from_color.clone()
                } else {
                    self.to_color.clone()
                };
                DecorationRenderOptions {
                    background_color: bg,
                    border: None,
                    color: None,
                    font_style: None,
                    font_weight: None,
                    is_whole_line: false,
                }
            })
            .collect()
    }

    /// Returns the number of animation steps.
    pub fn step_count(&self) -> u32 {
        self.steps
    }
}

// ── Batch operations on DecorationBridge ──

impl DecorationBridge {
    /// Register multiple decoration types at once.
    pub fn register_types_batch(&mut self, types: Vec<(&str, DecorationRenderOptions)>) -> usize {
        let mut count = 0;
        for (key, options) in types {
            if !self.has_type(key) {
                self.register_type(key, options);
                count += 1;
            }
        }
        count
    }

    /// Unregister multiple decoration types at once.
    pub fn unregister_types_batch(&mut self, keys: &[&str]) -> usize {
        let before = self.type_count();
        for key in keys {
            self.unregister_type(key);
        }
        before - self.type_count()
    }

    /// Clear all decorations for multiple URIs at once.
    pub fn clear_uris_batch(&mut self, uris: &[&str]) {
        for uri in uris {
            self.clear_uri(uri);
        }
    }

    /// Returns all registered type keys.
    pub fn type_keys(&self) -> Vec<&str> {
        self.types.iter().map(|t| t.key.as_str()).collect()
    }
}

/// Count total decoration ranges across all applied entries for a given type key.
pub fn count_ranges_for_type(bridge: &DecorationBridge, key: &str) -> usize {
    bridge
        .decorations_for_uri("")
        .iter()
        .count();
    // Actually count across all applied
    let mut total = 0;
    for (k, _uri, ranges) in &bridge.applied {
        if k == key {
            total += ranges.len();
        }
    }
    total
}

/// Return the set of unique URIs that have decorations applied.
pub fn decorated_uris(bridge: &DecorationBridge) -> Vec<String> {
    let mut uris: Vec<String> = bridge
        .applied
        .iter()
        .map(|(_, uri, _)| uri.clone())
        .collect();
    uris.sort();
    uris.dedup();
    uris
}

/// Check if a decoration options range covers a specific line.
pub fn decoration_covers_line(opt: &DecorationOptions, line: u32) -> bool {
    opt.start_line <= line && opt.end_line >= line
}

/// Filter decoration options to only those covering a specific line.
pub fn decorations_at_line(options: &[DecorationOptions], line: u32) -> Vec<&DecorationOptions> {
    options
        .iter()
        .filter(|o| decoration_covers_line(o, line))
        .collect()
}

/// Compute a summary of render options: how many style properties are set.
pub fn render_options_style_count(opts: &DecorationRenderOptions) -> usize {
    let mut count = 0;
    if opts.background_color.is_some() { count += 1; }
    if opts.border.is_some() { count += 1; }
    if opts.color.is_some() { count += 1; }
    if opts.font_style.is_some() { count += 1; }
    if opts.font_weight.is_some() { count += 1; }
    if opts.is_whole_line { count += 1; }
    count
}

/// Merge two DecorationRenderOptions, with `overlay` values taking precedence.
pub fn merge_render_options(
    base: &DecorationRenderOptions,
    overlay: &DecorationRenderOptions,
) -> DecorationRenderOptions {
    DecorationRenderOptions {
        background_color: overlay.background_color.clone().or_else(|| base.background_color.clone()),
        border: overlay.border.clone().or_else(|| base.border.clone()),
        color: overlay.color.clone().or_else(|| base.color.clone()),
        font_style: overlay.font_style.clone().or_else(|| base.font_style.clone()),
        font_weight: overlay.font_weight.clone().or_else(|| base.font_weight.clone()),
        is_whole_line: overlay.is_whole_line || base.is_whole_line,
    }
}

// ── Decoration Hit Testing ──

/// Result of a hit test against decoration ranges.
#[derive(Debug, Clone, PartialEq)]
pub struct DecorationHit {
    /// Index of the decoration in the original slice.
    pub index: usize,
    /// The matched decoration range.
    pub range: DecorationOptions,
    /// The decoration type key.
    pub type_key: String,
}

/// Test whether a cursor position (line, character) falls inside any decoration range.
pub fn hit_test(
    decorations: &[(&str, &[DecorationOptions])],
    line: u32,
    character: u32,
) -> Vec<DecorationHit> {
    let mut hits = Vec::new();
    for &(key, ranges) in decorations {
        for (i, r) in ranges.iter().enumerate() {
            if point_in_range(line, character, r) {
                hits.push(DecorationHit {
                    index: i,
                    range: r.clone(),
                    type_key: key.to_string(),
                });
            }
        }
    }
    hits
}

/// Check if a point (line, character) is inside a decoration range (inclusive on both ends).
fn point_in_range(line: u32, character: u32, r: &DecorationOptions) -> bool {
    if line < r.start_line || line > r.end_line {
        return false;
    }
    if line == r.start_line && character < r.start_character {
        return false;
    }
    if line == r.end_line && character > r.end_character {
        return false;
    }
    true
}

// ── Decoration Range Splitting ──

impl DecorationOptions {
    /// Split a multi-line decoration into per-line decorations.
    /// Each resulting decoration covers exactly one line: from the appropriate
    /// start character to `line_length` (or the original end character for the last line).
    /// `line_lengths` maps line number → length; lines not in the map use `default_length`.
    pub fn split_by_line(
        &self,
        line_lengths: &std::collections::HashMap<u32, u32>,
        default_length: u32,
    ) -> Vec<DecorationOptions> {
        if self.start_line == self.end_line {
            return vec![self.clone()];
        }
        let mut result = Vec::new();
        for line in self.start_line..=self.end_line {
            let start_ch = if line == self.start_line {
                self.start_character
            } else {
                0
            };
            let end_ch = if line == self.end_line {
                self.end_character
            } else {
                *line_lengths.get(&line).unwrap_or(&default_length)
            };
            result.push(DecorationOptions {
                start_line: line,
                start_character: start_ch,
                end_line: line,
                end_character: end_ch,
                hover_message: self.hover_message.clone(),
            });
        }
        result
    }

    /// Returns `true` if this range fully contains `other`.
    pub fn contains(&self, other: &DecorationOptions) -> bool {
        let self_start = (self.start_line, self.start_character);
        let self_end = (self.end_line, self.end_character);
        let other_start = (other.start_line, other.start_character);
        let other_end = (other.end_line, other.end_character);
        self_start <= other_start && self_end >= other_end
    }

    /// Compute the intersection of two ranges, if they overlap.
    pub fn intersect(&self, other: &DecorationOptions) -> Option<DecorationOptions> {
        if !self.overlaps(other) {
            return None;
        }
        let start = if (self.start_line, self.start_character)
            > (other.start_line, other.start_character)
        {
            (self.start_line, self.start_character)
        } else {
            (other.start_line, other.start_character)
        };
        let end = if (self.end_line, self.end_character) < (other.end_line, other.end_character) {
            (self.end_line, self.end_character)
        } else {
            (other.end_line, other.end_character)
        };
        Some(DecorationOptions {
            start_line: start.0,
            start_character: start.1,
            end_line: end.0,
            end_character: end.1,
            hover_message: None,
        })
    }
}

// ── Decoration Diff Computation ──

/// Describes the difference between two decoration sets.
#[derive(Debug, Clone, PartialEq)]
pub struct DecorationDiff {
    /// Ranges present in `new` but not in `old`.
    pub added: Vec<DecorationOptions>,
    /// Ranges present in `old` but not in `new`.
    pub removed: Vec<DecorationOptions>,
    /// Ranges unchanged between `old` and `new`.
    pub unchanged: Vec<DecorationOptions>,
}

/// Compute the diff between an old and new set of decoration ranges.
pub fn diff_decorations(
    old: &[DecorationOptions],
    new: &[DecorationOptions],
) -> DecorationDiff {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut unchanged = Vec::new();

    for o in old {
        if new.contains(o) {
            unchanged.push(o.clone());
        } else {
            removed.push(o.clone());
        }
    }
    for n in new {
        if !old.contains(n) {
            added.push(n.clone());
        }
    }
    DecorationDiff {
        added,
        removed,
        unchanged,
    }
}

// ── Decoration Filtering ──

/// Category for filtering decorations by their visual properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationCategory {
    /// Has a background color set.
    Background,
    /// Has a border set.
    Border,
    /// Has a foreground color set.
    Foreground,
    /// Applies to the whole line.
    WholeLine,
}

/// Filter registered decoration types by category.
pub fn filter_types_by_category<'a>(
    types: &'a [DecorationType],
    category: DecorationCategory,
) -> Vec<&'a DecorationType> {
    types
        .iter()
        .filter(|t| match category {
            DecorationCategory::Background => t.options.background_color.is_some(),
            DecorationCategory::Border => t.options.border.is_some(),
            DecorationCategory::Foreground => t.options.color.is_some(),
            DecorationCategory::WholeLine => t.options.is_whole_line,
        })
        .collect()
}

// ── Computed Style ──

/// A fully resolved computed style combining multiple decoration render options
/// according to priority order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ComputedStyle {
    pub background_color: Option<String>,
    pub border: Option<String>,
    pub color: Option<String>,
    pub font_style: Option<String>,
    pub font_weight: Option<String>,
    pub is_whole_line: bool,
}

/// Compute the final style for a position by layering render options in priority order.
/// `layers` must be sorted highest-priority-first; the first layer with a value wins.
pub fn compute_style(layers: &[&DecorationRenderOptions]) -> ComputedStyle {
    let mut style = ComputedStyle::default();
    for layer in layers {
        if style.background_color.is_none() {
            style.background_color = layer.background_color.clone();
        }
        if style.border.is_none() {
            style.border = layer.border.clone();
        }
        if style.color.is_none() {
            style.color = layer.color.clone();
        }
        if style.font_style.is_none() {
            style.font_style = layer.font_style.clone();
        }
        if style.font_weight.is_none() {
            style.font_weight = layer.font_weight.clone();
        }
        if layer.is_whole_line {
            style.is_whole_line = true;
        }
    }
    style
}

// ── Decoration Lifecycle Manager ──

/// Tracks the lifecycle state of a decoration type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Created,
    Active,
    Paused,
    Disposed,
}

/// Manages decoration type lifecycle transitions.
#[derive(Debug, Clone)]
pub struct DecorationLifecycleManager {
    states: Vec<(String, LifecycleState)>,
}

impl DecorationLifecycleManager {
    pub fn new() -> Self {
        Self {
            states: Vec::new(),
        }
    }

    /// Register a new decoration type in Created state.
    pub fn create(&mut self, key: impl Into<String>) {
        let key = key.into();
        if !self.states.iter().any(|(k, _)| k == &key) {
            self.states.push((key, LifecycleState::Created));
        }
    }

    /// Transition a decoration type to Active state.
    pub fn activate(&mut self, key: &str) -> Result<(), String> {
        self.transition(key, &[LifecycleState::Created, LifecycleState::Paused], LifecycleState::Active)
    }

    /// Transition a decoration type to Paused state.
    pub fn pause(&mut self, key: &str) -> Result<(), String> {
        self.transition(key, &[LifecycleState::Active], LifecycleState::Paused)
    }

    /// Transition a decoration type to Disposed state (terminal).
    pub fn dispose(&mut self, key: &str) -> Result<(), String> {
        self.transition(
            key,
            &[LifecycleState::Created, LifecycleState::Active, LifecycleState::Paused],
            LifecycleState::Disposed,
        )
    }

    /// Get the current state of a decoration type.
    pub fn state(&self, key: &str) -> Option<LifecycleState> {
        self.states.iter().find(|(k, _)| k == key).map(|(_, s)| *s)
    }

    /// Return all keys currently in Active state.
    pub fn active_keys(&self) -> Vec<&str> {
        self.states
            .iter()
            .filter(|(_, s)| *s == LifecycleState::Active)
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Return the total number of tracked decoration types.
    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    fn transition(
        &mut self,
        key: &str,
        valid_from: &[LifecycleState],
        to: LifecycleState,
    ) -> Result<(), String> {
        let entry = self
            .states
            .iter_mut()
            .find(|(k, _)| k == key)
            .ok_or_else(|| format!("unknown key: {key}"))?;
        if !valid_from.contains(&entry.1) {
            return Err(format!(
                "cannot transition {key} from {:?} to {to:?}",
                entry.1
            ));
        }
        entry.1 = to;
        Ok(())
    }
}

impl Default for DecorationLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Decoration Serialization Snapshot ──

/// A serializable snapshot of the entire decoration bridge state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecorationSnapshot {
    pub types: Vec<DecorationType>,
    pub applied: Vec<AppliedDecorationSet>,
}

/// A single applied decoration set entry for serialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppliedDecorationSet {
    pub key: String,
    pub uri: String,
    pub ranges: Vec<DecorationOptions>,
}

impl DecorationBridge {
    /// Serialize the current state into a snapshot.
    pub fn snapshot(&self) -> DecorationSnapshot {
        DecorationSnapshot {
            types: self.types.clone(),
            applied: self
                .applied
                .iter()
                .map(|(k, u, r)| AppliedDecorationSet {
                    key: k.clone(),
                    uri: u.clone(),
                    ranges: r.clone(),
                })
                .collect(),
        }
    }

    /// Restore state from a snapshot, replacing current state entirely.
    pub fn restore(&mut self, snapshot: DecorationSnapshot) {
        self.types = snapshot.types;
        self.applied = snapshot
            .applied
            .into_iter()
            .map(|a| (a.key, a.uri, a.ranges))
            .collect();
    }

    /// Collect all decorations at a specific line across all URIs and types,
    /// returning (type_key, uri, &DecorationOptions) triples.
    pub fn all_decorations_at_line(&self, line: u32) -> Vec<(&str, &str, &DecorationOptions)> {
        let mut result = Vec::new();
        for (key, uri, ranges) in &self.applied {
            for r in ranges {
                if decoration_covers_line(r, line) {
                    result.push((key.as_str(), uri.as_str(), r));
                }
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// DecorationRangeMerger - decoration range merger
// ---------------------------------------------------------------------------

/// Severity level for decoration range merger issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DecorationRangeMergerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for DecorationRangeMergerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [DecorationRangeMerger].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecorationRangeMergerEntry {
    pub id: String,
    pub label: String,
    pub severity: DecorationRangeMergerSeverity,
    pub detail: Option<String>,
    pub range_count: usize,
    enabled: bool,
}

impl DecorationRangeMergerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: DecorationRangeMergerSeverity::Low,
            detail: None,
            range_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: DecorationRangeMergerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_range_count(mut self, val: usize) -> Self {
        self.range_count = val;
        self
    }

    pub fn can_merge(&self) -> bool {
        self.enabled && self.severity >= DecorationRangeMergerSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.range_count, det)
    }
}

impl fmt::Display for DecorationRangeMergerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [DecorationRangeMergerEntry] items.
#[derive(Debug, Clone)]
pub struct DecorationRangeMerger {
    entries: Vec<DecorationRangeMergerEntry>,
    name: String,
    capacity: usize,
}

impl DecorationRangeMerger {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: DecorationRangeMergerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<DecorationRangeMergerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&DecorationRangeMergerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn range_count(&self) -> usize { self.entries.len() }

    pub fn can_merge(&self) -> bool {
        self.entries.iter().any(|e| e.can_merge())
    }

    pub fn entries_by_severity(&self, severity: DecorationRangeMergerSeverity) -> Vec<&DecorationRangeMergerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= DecorationRangeMergerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&DecorationRangeMergerEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&DecorationRangeMergerEntry> {
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
// DecorationPriorityResolver - decoration priority resolver
// ---------------------------------------------------------------------------

/// Configuration for [DecorationPriorityResolver].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecorationPriorityResolverConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub priority_count: usize,
}

impl DecorationPriorityResolverConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, priority_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_priority_count(mut self, val: usize) -> Self { self.priority_count = val; self }
}

impl Default for DecorationPriorityResolverConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [DecorationPriorityResolver].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecorationPriorityResolverItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl DecorationPriorityResolverItem {
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

    pub fn has_conflict(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for DecorationPriorityResolverItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [DecorationPriorityResolverItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct DecorationPriorityResolver {
    config: DecorationPriorityResolverConfig,
    items: Vec<DecorationPriorityResolverItem>,
}

impl DecorationPriorityResolver {
    pub fn new(config: DecorationPriorityResolverConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: DecorationPriorityResolverItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<DecorationPriorityResolverItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&DecorationPriorityResolverItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn priority_count(&self) -> usize { self.items.len() }

    pub fn has_conflict(&self) -> bool {
        self.items.iter().any(|i| i.has_conflict())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&DecorationPriorityResolverItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&DecorationPriorityResolverItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &DecorationPriorityResolverConfig {
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

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = DecorationMessage::RegisterType {
            key: "highlight".into(),
            options: DecorationRenderOptions {
                background_color: Some("yellow".into()),
                border: None,
                color: None,
                font_style: None,
                font_weight: None,
                is_whole_line: false,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: DecorationMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn decoration_options_serialization() {
        let opt = DecorationOptions {
            start_line: 1,
            start_character: 0,
            end_line: 1,
            end_character: 10,
            hover_message: Some("error here".into()),
        };
        let json = serde_json::to_string(&opt).unwrap();
        let back: DecorationOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opt, back);
    }

    #[test]
    fn bridge_register_and_unregister() {
        let mut bridge = DecorationBridge::new();
        bridge.register_type(
            "err",
            DecorationRenderOptions {
                background_color: Some("red".into()),
                border: None,
                color: None,
                font_style: None,
                font_weight: None,
                is_whole_line: true,
            },
        );
        assert!(bridge.has_type("err"));
        bridge.unregister_type("err");
        assert!(!bridge.has_type("err"));
    }

    #[test]
    fn bridge_set_clears_old() {
        let mut bridge = DecorationBridge::new();
        let opts = vec![DecorationOptions {
            start_line: 1,
            start_character: 0,
            end_line: 1,
            end_character: 5,
            hover_message: None,
        }];
        bridge.set_decorations("k", "file:///a", opts.clone());
        bridge.set_decorations("k", "file:///a", vec![]);
        assert!(bridge.applied.is_empty());
    }

    #[test]
    fn bridge_unregister_cleans_applied() {
        let mut bridge = DecorationBridge::new();
        let render = DecorationRenderOptions {
            background_color: None,
            border: None,
            color: None,
            font_style: None,
            font_weight: None,
            is_whole_line: false,
        };
        bridge.register_type("k", render);
        bridge.set_decorations(
            "k",
            "file:///a",
            vec![DecorationOptions {
                start_line: 1,
                start_character: 0,
                end_line: 1,
                end_character: 5,
                hover_message: None,
            }],
        );
        bridge.unregister_type("k");
        assert!(bridge.applied.is_empty());
    }

    #[test]
    fn render_options_builder() {
        let opts = RenderOptionsBuilder::new()
            .background_color("red")
            .color("white")
            .font_weight("bold")
            .whole_line(true)
            .build();
        assert_eq!(opts.background_color.as_deref(), Some("red"));
        assert_eq!(opts.color.as_deref(), Some("white"));
        assert_eq!(opts.font_weight.as_deref(), Some("bold"));
        assert!(opts.is_whole_line);
        assert!(opts.border.is_none());
    }

    #[test]
    fn decoration_options_validate_ok() {
        let opt = DecorationOptions {
            start_line: 1,
            start_character: 0,
            end_line: 1,
            end_character: 5,
            hover_message: None,
        };
        assert!(opt.validate().is_ok());
    }

    #[test]
    fn decoration_options_validate_bad_range() {
        let opt = DecorationOptions {
            start_line: 5,
            start_character: 0,
            end_line: 3,
            end_character: 0,
            hover_message: None,
        };
        assert!(matches!(
            opt.validate(),
            Err(DecorationError::InvalidRange { .. })
        ));
    }

    #[test]
    fn decoration_options_line_span() {
        let opt = DecorationOptions {
            start_line: 2,
            start_character: 0,
            end_line: 5,
            end_character: 10,
            hover_message: None,
        };
        assert_eq!(opt.line_span(), 4);
        assert!(!opt.is_single_line());
    }

    #[test]
    fn decoration_options_overlap_detection() {
        let a = DecorationOptions {
            start_line: 1,
            start_character: 0,
            end_line: 1,
            end_character: 10,
            hover_message: None,
        };
        let b = DecorationOptions {
            start_line: 1,
            start_character: 5,
            end_line: 1,
            end_character: 15,
            hover_message: None,
        };
        let c = DecorationOptions {
            start_line: 2,
            start_character: 0,
            end_line: 2,
            end_character: 5,
            hover_message: None,
        };
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn try_register_rejects_empty_key() {
        let mut bridge = DecorationBridge::new();
        let res = bridge.try_register_type("", RenderOptionsBuilder::new().build());
        assert_eq!(res, Err(DecorationError::EmptyKey));
    }

    #[test]
    fn try_set_decorations_rejects_unknown_type() {
        let mut bridge = DecorationBridge::new();
        let res = bridge.try_set_decorations("missing", "file:///a", vec![]);
        assert_eq!(res, Err(DecorationError::UnknownType("missing".into())));
    }

    #[test]
    fn try_set_decorations_validates_ranges() {
        let mut bridge = DecorationBridge::new();
        bridge
            .try_register_type("k", RenderOptionsBuilder::new().build())
            .unwrap();
        let bad_range = DecorationOptions {
            start_line: 10,
            start_character: 0,
            end_line: 5,
            end_character: 0,
            hover_message: None,
        };
        let res = bridge.try_set_decorations("k", "file:///a", vec![bad_range]);
        assert!(matches!(res, Err(DecorationError::InvalidRange { .. })));
    }

    #[test]
    fn bridge_counts_and_clear_uri() {
        let mut bridge = DecorationBridge::new();
        bridge
            .try_register_type("k", RenderOptionsBuilder::new().build())
            .unwrap();
        bridge
            .try_register_type("k2", RenderOptionsBuilder::new().color("blue").build())
            .unwrap();
        assert_eq!(bridge.type_count(), 2);

        let r = DecorationOptions {
            start_line: 0,
            start_character: 0,
            end_line: 0,
            end_character: 1,
            hover_message: None,
        };
        bridge.try_set_decorations("k", "file:///a", vec![r.clone()]).unwrap();
        bridge.try_set_decorations("k2", "file:///a", vec![r.clone()]).unwrap();
        bridge.try_set_decorations("k", "file:///b", vec![r]).unwrap();
        assert_eq!(bridge.applied_count(), 3);
        assert_eq!(bridge.total_ranges(), 3);
        assert_eq!(bridge.decorations_for_uri("file:///a").len(), 2);

        bridge.clear_uri("file:///a");
        assert_eq!(bridge.applied_count(), 1);
    }

    #[test]
    fn display_implementations() {
        let opts = RenderOptionsBuilder::new()
            .background_color("yellow")
            .color("black")
            .build();
        let display = format!("{opts}");
        assert!(display.contains("bg=yellow"));
        assert!(display.contains("color=black"));

        let dec = DecorationOptions {
            start_line: 1,
            start_character: 2,
            end_line: 3,
            end_character: 4,
            hover_message: None,
        };
        assert_eq!(format!("{dec}"), "1:2-3:4");

        let dt = DecorationType {
            key: "err".into(),
            options: opts,
        };
        let dt_display = format!("{dt}");
        assert!(dt_display.contains("err"));
    }

    #[test]
    fn error_display() {
        let e = DecorationError::EmptyKey;
        assert_eq!(format!("{e}"), "decoration type key must not be empty");

        let e2 = DecorationError::UnknownType("foo".into());
        assert!(format!("{e2}").contains("foo"));
    }

    #[test]
    fn get_render_options() {
        let mut bridge = DecorationBridge::new();
        let opts = RenderOptionsBuilder::new().background_color("green").build();
        bridge.try_register_type("k", opts.clone()).unwrap();
        let retrieved = bridge.get_render_options("k").unwrap();
        assert_eq!(retrieved, &opts);
        assert!(bridge.get_render_options("missing").is_none());
    }

    #[test]
    fn ext_decorations_stats_new_defaults() {
        let stats = ExtDecorationsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_decorations_stats_record_success() {
        let mut stats = ExtDecorationsStats::new();
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
    fn ext_decorations_stats_record_failure() {
        let mut stats = ExtDecorationsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_decorations_stats_reset() {
        let mut stats = ExtDecorationsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_decorations_stats_merge() {
        let mut a = ExtDecorationsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtDecorationsStats::new();
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
    fn ext_decorations_stats_display() {
        let mut stats = ExtDecorationsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_decorations_stats_default() {
        let stats = ExtDecorationsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_decorations_validator_accepts_valid_name() {
        let v = ExtDecorationsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_decorations_validator_rejects_empty() {
        let v = ExtDecorationsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_decorations_validator_rejects_too_long() {
        let v = ExtDecorationsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_decorations_validator_forbidden_prefix() {
        let v = ExtDecorationsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_decorations_validator_allowed_chars() {
        let v = ExtDecorationsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_decorations_validator_range() {
        let v = ExtDecorationsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_decorations_sanitize_removes_control() {
        let result = ExtDecorationsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_decorations_truncate_short_string() {
        assert_eq!(ExtDecorationsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_decorations_truncate_long_string() {
        let result = ExtDecorationsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_decorations_is_ascii_printable() {
        assert!(ExtDecorationsValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtDecorationsValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- extended decoration tests ------------------------------------------

    #[test]
    fn text_editor_decoration_type_display() {
        let dt = TextEditorDecorationType::new(
            "errorHighlight",
            ExtendedRenderOptions {
                base: RenderOptionsBuilder::new()
                    .background_color("rgba(255,0,0,0.3)")
                    .build(),
                after: None,
                before: None,
                overview_ruler: Some(OverviewRulerOptions {
                    color: "red".into(),
                    lane: OverviewRulerLane::Right,
                }),
            },
        );
        assert_eq!(
            format!("{dt}"),
            "TextEditorDecorationType(errorHighlight)"
        );
        assert!(dt.options.overview_ruler.is_some());
    }

    #[test]
    fn extended_decoration_options_after_before() {
        let opt = ExtendedDecorationOptions {
            start_line: 1,
            start_character: 0,
            end_line: 1,
            end_character: 10,
            hover_message: None,
            after_content: Some(ThemableDecorationAttachment {
                content_text: Some(" // inferred".into()),
                color: Some("gray".into()),
                ..Default::default()
            }),
            before_content: None,
        };
        assert_eq!(
            opt.after_content.as_ref().unwrap().content_text.as_deref(),
            Some(" // inferred")
        );
    }

    #[test]
    fn overview_ruler_lane_variants() {
        assert_ne!(OverviewRulerLane::Left, OverviewRulerLane::Right);
        assert_ne!(OverviewRulerLane::Center, OverviewRulerLane::Full);
    }

    // -- new functionality tests --------------------------------------------

    #[test]
    fn bridge_is_empty() {
        let bridge = DecorationBridge::new();
        assert!(bridge.is_empty());
        let mut bridge2 = DecorationBridge::new();
        bridge2.register_type("k", RenderOptionsBuilder::new().build());
        assert!(!bridge2.is_empty());
    }

    #[test]
    fn bridge_get_type() {
        let mut bridge = DecorationBridge::new();
        let opts = RenderOptionsBuilder::new().background_color("red").build();
        bridge.register_type("err", opts.clone());
        let dt = bridge.get_type("err").unwrap();
        assert_eq!(dt.key, "err");
        assert_eq!(dt.options, opts);
        assert!(bridge.get_type("missing").is_none());
    }

    #[test]
    fn bridge_clear_all() {
        let mut bridge = DecorationBridge::new();
        bridge.register_type("a", RenderOptionsBuilder::new().build());
        bridge.register_type("b", RenderOptionsBuilder::new().build());
        bridge.set_decorations("a", "file:///x", vec![DecorationOptions {
            start_line: 0, start_character: 0, end_line: 0, end_character: 1,
            hover_message: None,
        }]);
        assert_eq!(bridge.type_count(), 2);
        assert_eq!(bridge.applied_count(), 1);
        bridge.clear_all();
        assert!(bridge.is_empty());
        assert_eq!(bridge.applied_count(), 0);
    }

    #[test]
    fn render_options_has_background() {
        let with_bg = RenderOptionsBuilder::new().background_color("red").build();
        assert!(with_bg.has_background());
        let without = RenderOptionsBuilder::new().build();
        assert!(!without.has_background());
    }

    #[test]
    fn render_options_has_border() {
        let with_border = RenderOptionsBuilder::new().border("1px solid").build();
        assert!(with_border.has_border());
        let without = RenderOptionsBuilder::new().build();
        assert!(!without.has_border());
    }

    #[test]
    fn decoration_type_is_whole_line() {
        let whole = DecorationType {
            key: "k".into(),
            options: RenderOptionsBuilder::new().whole_line(true).build(),
        };
        assert!(whole.is_whole_line());
        let partial = DecorationType {
            key: "k2".into(),
            options: RenderOptionsBuilder::new().whole_line(false).build(),
        };
        assert!(!partial.is_whole_line());
    }

    #[test]
    fn bridge_display() {
        let bridge = DecorationBridge::new();
        assert_eq!(format!("{bridge}"), "0 decoration types");

        let mut bridge1 = DecorationBridge::new();
        bridge1.register_type("a", RenderOptionsBuilder::new().build());
        assert_eq!(format!("{bridge1}"), "1 decoration type");

        let mut bridge2 = DecorationBridge::new();
        bridge2.register_type("a", RenderOptionsBuilder::new().build());
        bridge2.register_type("b", RenderOptionsBuilder::new().build());
        assert_eq!(format!("{bridge2}"), "2 decoration types");
    }

    // ── New tests ──

    #[test]
    fn priority_registry_sorted() {
        let mut reg = DecorationPriorityRegistry::new();
        reg.register("error", 100);
        reg.register("warning", 50);
        reg.register("info", 10);
        let keys = reg.sorted_keys();
        assert_eq!(keys, vec!["error", "warning", "info"]);
        assert_eq!(reg.get_priority("error"), Some(100));
        assert_eq!(reg.len(), 3);
    }

    #[test]
    fn priority_registry_update() {
        let mut reg = DecorationPriorityRegistry::new();
        reg.register("err", 10);
        reg.register("err", 99);
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.get_priority("err"), Some(99));
    }

    #[test]
    fn merger_non_overlapping() {
        let ranges = vec![
            DecorationOptions { start_line: 1, start_character: 0, end_line: 1, end_character: 5, hover_message: None },
            DecorationOptions { start_line: 3, start_character: 0, end_line: 3, end_character: 5, hover_message: None },
        ];
        let merged = DecorationMerger::merge(ranges);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merger_overlapping() {
        let ranges = vec![
            DecorationOptions { start_line: 1, start_character: 0, end_line: 1, end_character: 10, hover_message: None },
            DecorationOptions { start_line: 1, start_character: 5, end_line: 1, end_character: 15, hover_message: None },
            DecorationOptions { start_line: 1, start_character: 12, end_line: 2, end_character: 3, hover_message: None },
        ];
        let merged = DecorationMerger::merge(ranges);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].start_line, 1);
        assert_eq!(merged[0].end_line, 2);
        assert_eq!(merged[0].end_character, 3);
    }

    #[test]
    fn animator_generate_steps() {
        let animator = DecorationAnimator::new(4)
            .from_color("red")
            .to_color("blue");
        let steps = animator.generate_steps();
        assert_eq!(steps.len(), 4);
        assert_eq!(steps[0].background_color, Some("red".into()));
        assert_eq!(steps[3].background_color, Some("blue".into()));
        assert_eq!(animator.step_count(), 4);
    }

    #[test]
    fn animator_single_step() {
        let animator = DecorationAnimator::new(1).to_color("green");
        let steps = animator.generate_steps();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].background_color, Some("green".into()));
    }

    #[test]
    fn bridge_batch_register() {
        let mut bridge = DecorationBridge::new();
        let opts = RenderOptionsBuilder::new().build();
        let count = bridge.register_types_batch(vec![
            ("a", opts.clone()),
            ("b", opts.clone()),
            ("c", opts),
        ]);
        assert_eq!(count, 3);
        assert_eq!(bridge.type_count(), 3);
        let keys = bridge.type_keys();
        assert!(keys.contains(&"a"));
        assert!(keys.contains(&"b"));
    }

    #[test]
    fn bridge_batch_unregister() {
        let mut bridge = DecorationBridge::new();
        let opts = RenderOptionsBuilder::new().build();
        bridge.register_type("a", opts.clone());
        bridge.register_type("b", opts.clone());
        bridge.register_type("c", opts);
        let removed = bridge.unregister_types_batch(&["a", "c"]);
        assert_eq!(removed, 2);
        assert_eq!(bridge.type_count(), 1);
        assert!(bridge.has_type("b"));
    }

    #[test]
    fn bridge_clear_uris_batch() {
        let mut bridge = DecorationBridge::new();
        let opts = RenderOptionsBuilder::new().build();
        bridge.register_type("hl", opts);
        let r = DecorationOptions { start_line: 1, start_character: 0, end_line: 1, end_character: 5, hover_message: None };
        bridge.set_decorations("hl", "file:///a.rs", vec![r.clone()]);
        bridge.set_decorations("hl", "file:///b.rs", vec![r]);
        assert_eq!(bridge.applied_count(), 2);
        bridge.clear_uris_batch(&["file:///a.rs", "file:///b.rs"]);
        assert_eq!(bridge.applied_count(), 0);
    }

    #[test]
    fn count_ranges_for_type_basic() {
        let mut bridge = DecorationBridge::new();
        let opts = RenderOptionsBuilder::new().build();
        bridge.register_type("hl", opts);
        let r = DecorationOptions {
            start_line: 0, start_character: 0,
            end_line: 0, end_character: 5,
            hover_message: None,
        };
        bridge.set_decorations("hl", "file:///a.rs", vec![r.clone(), r.clone()]);
        bridge.set_decorations("hl", "file:///b.rs", vec![r]);
        assert_eq!(count_ranges_for_type(&bridge, "hl"), 3);
    }

    #[test]
    fn count_ranges_for_type_unknown_key() {
        let bridge = DecorationBridge::new();
        assert_eq!(count_ranges_for_type(&bridge, "nope"), 0);
    }

    #[test]
    fn decorated_uris_deduplicates() {
        let mut bridge = DecorationBridge::new();
        let opts = RenderOptionsBuilder::new().build();
        bridge.register_type("a", opts.clone());
        bridge.register_type("b", opts);
        let r = DecorationOptions {
            start_line: 0, start_character: 0,
            end_line: 0, end_character: 1,
            hover_message: None,
        };
        bridge.set_decorations("a", "file:///x.rs", vec![r.clone()]);
        bridge.set_decorations("b", "file:///x.rs", vec![r]);
        let uris = decorated_uris(&bridge);
        assert_eq!(uris.len(), 1);
        assert_eq!(uris[0], "file:///x.rs");
    }

    #[test]
    fn decorated_uris_empty() {
        let bridge = DecorationBridge::new();
        assert!(decorated_uris(&bridge).is_empty());
    }

    #[test]
    fn decoration_covers_line_true() {
        let opt = DecorationOptions {
            start_line: 2, start_character: 0,
            end_line: 5, end_character: 10,
            hover_message: None,
        };
        assert!(decoration_covers_line(&opt, 3));
        assert!(decoration_covers_line(&opt, 2));
        assert!(decoration_covers_line(&opt, 5));
    }

    #[test]
    fn decoration_covers_line_false() {
        let opt = DecorationOptions {
            start_line: 2, start_character: 0,
            end_line: 5, end_character: 10,
            hover_message: None,
        };
        assert!(!decoration_covers_line(&opt, 0));
        assert!(!decoration_covers_line(&opt, 6));
    }

    #[test]
    fn decorations_at_line_filters() {
        let opts = vec![
            DecorationOptions { start_line: 0, start_character: 0, end_line: 0, end_character: 5, hover_message: None },
            DecorationOptions { start_line: 1, start_character: 0, end_line: 3, end_character: 5, hover_message: None },
            DecorationOptions { start_line: 5, start_character: 0, end_line: 5, end_character: 5, hover_message: None },
        ];
        let at1 = decorations_at_line(&opts, 2);
        assert_eq!(at1.len(), 1);
        assert_eq!(at1[0].start_line, 1);
    }

    #[test]
    fn render_options_style_count_all_set() {
        let opts = RenderOptionsBuilder::new()
            .background_color("red")
            .border("1px solid")
            .color("blue")
            .font_style("italic")
            .font_weight("bold")
            .whole_line(true)
            .build();
        assert_eq!(render_options_style_count(&opts), 6);
    }

    #[test]
    fn render_options_style_count_none_set() {
        let opts = RenderOptionsBuilder::new().build();
        assert_eq!(render_options_style_count(&opts), 0);
    }

    #[test]
    fn merge_render_options_overlay_wins() {
        let base = RenderOptionsBuilder::new().background_color("red").build();
        let overlay = RenderOptionsBuilder::new().background_color("blue").color("green").build();
        let merged = merge_render_options(&base, &overlay);
        assert_eq!(merged.background_color.as_deref(), Some("blue"));
        assert_eq!(merged.color.as_deref(), Some("green"));
    }

    #[test]
    fn merge_render_options_base_fills_gaps() {
        let base = RenderOptionsBuilder::new().font_style("italic").build();
        let overlay = RenderOptionsBuilder::new().build();
        let merged = merge_render_options(&base, &overlay);
        assert_eq!(merged.font_style.as_deref(), Some("italic"));
    }

    // ── Hit testing tests ──

    #[test]
    fn hit_test_finds_matching_decorations() {
        let ranges = vec![
            DecorationOptions { start_line: 1, start_character: 0, end_line: 1, end_character: 10, hover_message: None },
            DecorationOptions { start_line: 3, start_character: 5, end_line: 3, end_character: 15, hover_message: None },
        ];
        let decorations = vec![("err", ranges.as_slice())];
        let hits = hit_test(&decorations, 1, 5);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].type_key, "err");
        assert_eq!(hits[0].index, 0);

        let hits_miss = hit_test(&decorations, 2, 0);
        assert!(hits_miss.is_empty());
    }

    #[test]
    fn hit_test_multiple_types_overlap() {
        let err_ranges = vec![
            DecorationOptions { start_line: 5, start_character: 0, end_line: 5, end_character: 20, hover_message: None },
        ];
        let warn_ranges = vec![
            DecorationOptions { start_line: 5, start_character: 10, end_line: 5, end_character: 30, hover_message: None },
        ];
        let decorations = vec![("err", err_ranges.as_slice()), ("warn", warn_ranges.as_slice())];
        let hits = hit_test(&decorations, 5, 15);
        assert_eq!(hits.len(), 2);
        let keys: Vec<&str> = hits.iter().map(|h| h.type_key.as_str()).collect();
        assert!(keys.contains(&"err"));
        assert!(keys.contains(&"warn"));
    }

    // ── Range splitting tests ──

    #[test]
    fn split_single_line_returns_self() {
        let opt = DecorationOptions {
            start_line: 3, start_character: 5, end_line: 3, end_character: 15,
            hover_message: Some("msg".into()),
        };
        let result = opt.split_by_line(&std::collections::HashMap::new(), 80);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], opt);
    }

    #[test]
    fn split_multi_line_creates_per_line() {
        let opt = DecorationOptions {
            start_line: 1, start_character: 5, end_line: 3, end_character: 10,
            hover_message: None,
        };
        let mut lengths = std::collections::HashMap::new();
        lengths.insert(1, 40);
        lengths.insert(2, 30);
        let result = opt.split_by_line(&lengths, 80);
        assert_eq!(result.len(), 3);
        // First line: keep original start
        assert_eq!(result[0].start_character, 5);
        assert_eq!(result[0].end_character, 40);
        // Middle line: full line
        assert_eq!(result[1].start_character, 0);
        assert_eq!(result[1].end_character, 30);
        // Last line: keep original end
        assert_eq!(result[2].start_character, 0);
        assert_eq!(result[2].end_character, 10);
    }

    // ── Contains / intersect tests ──

    #[test]
    fn contains_range() {
        let outer = DecorationOptions { start_line: 1, start_character: 0, end_line: 5, end_character: 20, hover_message: None };
        let inner = DecorationOptions { start_line: 2, start_character: 3, end_line: 4, end_character: 10, hover_message: None };
        let outside = DecorationOptions { start_line: 6, start_character: 0, end_line: 7, end_character: 5, hover_message: None };
        assert!(outer.contains(&inner));
        assert!(!inner.contains(&outer));
        assert!(!outer.contains(&outside));
    }

    #[test]
    fn intersect_overlapping() {
        let a = DecorationOptions { start_line: 1, start_character: 5, end_line: 1, end_character: 20, hover_message: None };
        let b = DecorationOptions { start_line: 1, start_character: 10, end_line: 1, end_character: 30, hover_message: None };
        let inter = a.intersect(&b).unwrap();
        assert_eq!(inter.start_line, 1);
        assert_eq!(inter.start_character, 10);
        assert_eq!(inter.end_line, 1);
        assert_eq!(inter.end_character, 20);
    }

    #[test]
    fn intersect_non_overlapping() {
        let a = DecorationOptions { start_line: 1, start_character: 0, end_line: 1, end_character: 5, hover_message: None };
        let b = DecorationOptions { start_line: 2, start_character: 0, end_line: 2, end_character: 5, hover_message: None };
        assert!(a.intersect(&b).is_none());
    }

    // ── Diff tests ──

    #[test]
    fn diff_decorations_detects_changes() {
        let r1 = DecorationOptions { start_line: 1, start_character: 0, end_line: 1, end_character: 5, hover_message: None };
        let r2 = DecorationOptions { start_line: 2, start_character: 0, end_line: 2, end_character: 5, hover_message: None };
        let r3 = DecorationOptions { start_line: 3, start_character: 0, end_line: 3, end_character: 5, hover_message: None };
        let old = vec![r1.clone(), r2.clone()];
        let new = vec![r2.clone(), r3.clone()];
        let diff = diff_decorations(&old, &new);
        assert_eq!(diff.added, vec![r3]);
        assert_eq!(diff.removed, vec![r1]);
        assert_eq!(diff.unchanged, vec![r2]);
    }

    // ── Filter by category tests ──

    #[test]
    fn filter_types_by_category_background() {
        let types = vec![
            DecorationType { key: "a".into(), options: RenderOptionsBuilder::new().background_color("red").build() },
            DecorationType { key: "b".into(), options: RenderOptionsBuilder::new().color("blue").build() },
            DecorationType { key: "c".into(), options: RenderOptionsBuilder::new().background_color("green").border("1px").build() },
        ];
        let bg = filter_types_by_category(&types, DecorationCategory::Background);
        assert_eq!(bg.len(), 2);
        assert_eq!(bg[0].key, "a");
        assert_eq!(bg[1].key, "c");

        let fg = filter_types_by_category(&types, DecorationCategory::Foreground);
        assert_eq!(fg.len(), 1);
        assert_eq!(fg[0].key, "b");

        let border = filter_types_by_category(&types, DecorationCategory::Border);
        assert_eq!(border.len(), 1);
        assert_eq!(border[0].key, "c");
    }

    // ── Computed style tests ──

    #[test]
    fn compute_style_layers_priority() {
        let high = RenderOptionsBuilder::new().background_color("red").color("white").build();
        let low = RenderOptionsBuilder::new().background_color("blue").font_style("italic").build();
        let style = compute_style(&[&high, &low]);
        // High priority wins for background
        assert_eq!(style.background_color.as_deref(), Some("red"));
        assert_eq!(style.color.as_deref(), Some("white"));
        // Low priority fills in missing
        assert_eq!(style.font_style.as_deref(), Some("italic"));
    }

    // ── Lifecycle manager tests ──

    #[test]
    fn lifecycle_state_transitions() {
        let mut mgr = DecorationLifecycleManager::new();
        mgr.create("highlight");
        assert_eq!(mgr.state("highlight"), Some(LifecycleState::Created));
        assert_eq!(mgr.len(), 1);

        mgr.activate("highlight").unwrap();
        assert_eq!(mgr.state("highlight"), Some(LifecycleState::Active));
        assert_eq!(mgr.active_keys(), vec!["highlight"]);

        mgr.pause("highlight").unwrap();
        assert_eq!(mgr.state("highlight"), Some(LifecycleState::Paused));
        assert!(mgr.active_keys().is_empty());

        mgr.activate("highlight").unwrap();
        assert_eq!(mgr.state("highlight"), Some(LifecycleState::Active));

        mgr.dispose("highlight").unwrap();
        assert_eq!(mgr.state("highlight"), Some(LifecycleState::Disposed));
    }

    #[test]
    fn lifecycle_invalid_transition_rejected() {
        let mut mgr = DecorationLifecycleManager::new();
        mgr.create("x");
        // Cannot pause from Created
        assert!(mgr.pause("x").is_err());
        // Cannot transition unknown key
        assert!(mgr.activate("unknown").is_err());
        // Dispose then try to reactivate
        mgr.dispose("x").unwrap();
        assert!(mgr.activate("x").is_err());
    }

    // ── Snapshot serialization tests ──

    #[test]
    fn snapshot_roundtrip() {
        let mut bridge = DecorationBridge::new();
        let opts = RenderOptionsBuilder::new().background_color("red").build();
        bridge.register_type("err", opts);
        bridge.set_decorations("err", "file:///a.rs", vec![
            DecorationOptions { start_line: 1, start_character: 0, end_line: 1, end_character: 10, hover_message: Some("oops".into()) },
        ]);

        let snap = bridge.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        let restored_snap: DecorationSnapshot = serde_json::from_str(&json).unwrap();

        let mut bridge2 = DecorationBridge::new();
        bridge2.restore(restored_snap);
        assert_eq!(bridge2.type_count(), 1);
        assert!(bridge2.has_type("err"));
        assert_eq!(bridge2.applied_count(), 1);
        assert_eq!(bridge2.total_ranges(), 1);
    }

    // ── all_decorations_at_line tests ──

    #[test]
    fn all_decorations_at_line_across_types() {
        let mut bridge = DecorationBridge::new();
        bridge.register_type("err", RenderOptionsBuilder::new().background_color("red").build());
        bridge.register_type("warn", RenderOptionsBuilder::new().background_color("yellow").build());
        bridge.set_decorations("err", "file:///a.rs", vec![
            DecorationOptions { start_line: 5, start_character: 0, end_line: 5, end_character: 20, hover_message: None },
        ]);
        bridge.set_decorations("warn", "file:///b.rs", vec![
            DecorationOptions { start_line: 5, start_character: 3, end_line: 7, end_character: 10, hover_message: None },
        ]);
        let at5 = bridge.all_decorations_at_line(5);
        assert_eq!(at5.len(), 2);
        let at6 = bridge.all_decorations_at_line(6);
        assert_eq!(at6.len(), 1);
        assert_eq!(at6[0].0, "warn");
    }

#[test]
    fn decorationrangemerger_severity_ordering() {
        assert!(DecorationRangeMergerSeverity::Critical > DecorationRangeMergerSeverity::High);
        assert!(DecorationRangeMergerSeverity::High > DecorationRangeMergerSeverity::Medium);
        assert!(DecorationRangeMergerSeverity::Medium > DecorationRangeMergerSeverity::Low);
    }

    #[test]
    fn decorationrangemerger_severity_display() {
        assert_eq!(DecorationRangeMergerSeverity::Low.to_string(), "low");
        assert_eq!(DecorationRangeMergerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn decorationrangemerger_entry_creation() {
        let e = DecorationRangeMergerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, DecorationRangeMergerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn decorationrangemerger_entry_builder() {
        let e = DecorationRangeMergerEntry::new("e2", "Entry 2")
            .with_severity(DecorationRangeMergerSeverity::High)
            .with_detail("some detail")
            .with_range_count(42);
        assert_eq!(e.severity, DecorationRangeMergerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.range_count, 42);
    }

    #[test]
    fn decorationrangemerger_entry_enable_disable() {
        let mut e = DecorationRangeMergerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn decorationrangemerger_add_and_count() {
        let mut mgr = DecorationRangeMerger::new("test");
        mgr.add(DecorationRangeMergerEntry::new("a", "A"));
        mgr.add(DecorationRangeMergerEntry::new("b", "B").with_severity(DecorationRangeMergerSeverity::High));
        assert_eq!(mgr.range_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn decorationrangemerger_remove() {
        let mut mgr = DecorationRangeMerger::new("test");
        mgr.add(DecorationRangeMergerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn decorationrangemerger_capacity() {
        let mut mgr = DecorationRangeMerger::new("test").with_capacity(1);
        assert!(mgr.add(DecorationRangeMergerEntry::new("a", "A")));
        assert!(!mgr.add(DecorationRangeMergerEntry::new("b", "B")));
    }

    #[test]
    fn decorationrangemerger_sorted_by_severity() {
        let mut mgr = DecorationRangeMerger::new("test");
        mgr.add(DecorationRangeMergerEntry::new("lo", "Low"));
        mgr.add(DecorationRangeMergerEntry::new("hi", "High").with_severity(DecorationRangeMergerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, DecorationRangeMergerSeverity::Critical);
    }

    #[test]
    fn decorationrangemerger_summary() {
        let mgr = DecorationRangeMerger::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn decorationpriorityresolver_config_defaults() {
        let cfg = DecorationPriorityResolverConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn decorationpriorityresolver_item_creation() {
        let item = DecorationPriorityResolverItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn decorationpriorityresolver_add_and_get() {
        let mut mgr = DecorationPriorityResolver::new(DecorationPriorityResolverConfig::new("test"));
        mgr.add(DecorationPriorityResolverItem::new("k1", "v1"));
        assert_eq!(mgr.priority_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn decorationpriorityresolver_remove_item() {
        let mut mgr = DecorationPriorityResolver::new(DecorationPriorityResolverConfig::new("test"));
        mgr.add(DecorationPriorityResolverItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn decorationpriorityresolver_sorted_by_priority() {
        let mut mgr = DecorationPriorityResolver::new(DecorationPriorityResolverConfig::new("test"));
        mgr.add(DecorationPriorityResolverItem::new("lo", "low").with_priority(1));
        mgr.add(DecorationPriorityResolverItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn decorationpriorityresolver_items_with_tag() {
        let mut mgr = DecorationPriorityResolver::new(DecorationPriorityResolverConfig::new("test"));
        mgr.add(DecorationPriorityResolverItem::new("a", "1").with_tag("x"));
        mgr.add(DecorationPriorityResolverItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn decorationpriorityresolver_report() {
        let mgr = DecorationPriorityResolver::new(DecorationPriorityResolverConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }
}
