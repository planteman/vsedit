//! Ext API: Decorations.
//!
//! RPC bridge between the extension host and the main thread for editor decorations.

use std::collections::HashMap;
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



/// Configuration manager for ext_decorations functionality.
pub struct ExtDecorationsConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ExtDecorationsConfig {
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

    pub fn merge(&mut self, other: &ExtDecorationsConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for ext_decorations operations.
pub struct ExtDecorationsRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ExtDecorationsRateTracker {
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

/// Validation result collector for ext_decorations.
pub struct ExtDecorationsValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ExtDecorationsValidationCollector {
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

    pub fn merge(&mut self, other: &ExtDecorationsValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Text decoration ranges and styling — extended utilities (zx)
// ---------------------------------------------------------------------------

/// Metric accumulator for ext_deco operations.
#[derive(Debug, Clone)]
pub struct ZxMetrics {
    samples: Vec<f64>,
    label: String,
}

impl ZxMetrics {
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

/// Sliding-window rate counter for ext_deco.
#[derive(Debug, Clone)]
pub struct ZxRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl ZxRateWindow {
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

/// A small LRU-style cache for ext_deco lookups.
#[derive(Debug, Clone)]
pub struct ZxLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZxLruCache {
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
// xa_ extended helpers for ext_decorations
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtDecorationsRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtDecorationsRingBuf {
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
pub struct XaExtDecorationsCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtDecorationsCounter {
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

impl Default for XaExtDecorationsCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 55
// ---------------------------------------------------------------------------

/// Generic object pool `Xc55Pool<T>`.
pub struct Xc55Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc55Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc55PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc55Pool<T> {
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
    pub fn stats(&self) -> Xc55PoolStats {
        Xc55PoolStats {
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

impl<T> Default for Xc55Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc55Scheduler`.
pub struct Xc55Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc55Scheduler {
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

impl Default for Xc55Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_55 hash for the given byte slice.
pub fn xc_55_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_55 convention.
pub fn xc_55_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_42 deepening: state machine + event bus ---

/// States for the Xd42 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd42State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd42State {
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
pub struct Xd42Transition {
    pub from: Xd42State,
    pub to: Xd42State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd42StateMachine {
    current: Xd42State,
    history: Vec<Xd42Transition>,
    step_counter: usize,
}

impl Xd42StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd42State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd42State {
        self.current
    }

    pub fn history(&self) -> &[Xd42Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd42State) -> Result<Xd42State, String> {
        let allowed = match (self.current, target) {
            (Xd42State::Idle, Xd42State::Running) => true,
            (Xd42State::Running, Xd42State::Paused) => true,
            (Xd42State::Running, Xd42State::Done) => true,
            (Xd42State::Paused, Xd42State::Running) => true,
            (Xd42State::Paused, Xd42State::Done) => true,
            (Xd42State::Done, Xd42State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_42: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd42Transition {
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
            "Xd42SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd42State> {
        let prefix = "Xd42SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd42State::Idle),
            "Running" => Some(Xd42State::Running),
            "Paused" => Some(Xd42State::Paused),
            "Done" => Some(Xd42State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd42State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd42 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd42Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd42Event {
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

type Xd42HandlerFn = Box<dyn Fn(&Xd42Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd42EventBus {
    handlers: Vec<(usize, Option<String>, Xd42HandlerFn)>,
    next_id: usize,
    published: Vec<Xd42Event>,
}

impl Xd42EventBus {
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
        F: Fn(&Xd42Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd42Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd42Event) {
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

    pub fn published_events(&self) -> &[Xd42Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #40
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf40Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf40TrieNode {
    children: std::collections::HashMap<char, Xf40TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf40Trie {
    root: Xf40TrieNode,
    count: usize,
}

impl Xf40Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf40TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf40TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf40TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf40BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf40BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 54).
pub struct Xh54SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh54SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 96 as u64,
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

/// A compact bit set supporting boolean operations (variant 54).
pub struct Xh54BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh54BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 54).
pub struct Xi54Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi54Deque<T> {
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
pub struct Xi54Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi54Interval {
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

/// A simple interval tree (variant 54).
pub struct Xi54IntervalTree {
    xi_intervals: Vec<Xi54Interval>,
}

impl Xi54IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi54Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi54Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi54Interval) -> Vec<&Xi54Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi54Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi54Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi54Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi54Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi54Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi54Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 54) ---

/// Disjoint set / union-find for crate 54.
pub struct Xj54UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj54UnionFind {
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

const XJ54_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 54.
pub struct Xj54BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj54BTreeNode<K, V>>>,
    len: usize,
}

struct Xj54BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj54BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj54BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ54_BTREE_ORDER - 1
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
        let mid = XJ54_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj54BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj54BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj54BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj54BTreeNode::xj_new_leaf();
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


// --- xk_54 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk54SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk54SegmentTree {
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
pub struct Xk54DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk54DisjointIntervals {
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
    fn extdecorations_validator_accepts_and_rejects() {
        let mut v = ExtDecorationsValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn extdecorations_validator_warnings() {
        let mut v = ExtDecorationsValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn extdecorations_validator_clear_and_merge() {
        let mut v = ExtDecorationsValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = ExtDecorationsValidationCollector::new();
        a.add_error("a_err");
        let mut b = ExtDecorationsValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
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

    #[test]
    fn ext_decorations_config_new() {
        let cfg = ExtDecorationsConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn ext_decorations_config_set_get() {
        let mut cfg = ExtDecorationsConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn ext_decorations_config_remove() {
        let mut cfg = ExtDecorationsConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn ext_decorations_config_keys_sorted() {
        let mut cfg = ExtDecorationsConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn ext_decorations_config_bump_version() {
        let mut cfg = ExtDecorationsConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn ext_decorations_config_clear() {
        let mut cfg = ExtDecorationsConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn ext_decorations_config_merge() {
        let mut cfg1 = ExtDecorationsConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ExtDecorationsConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn ext_decorations_config_disable() {
        let mut cfg = ExtDecorationsConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn ext_decorations_rate_tracker_empty() {
        let rt = ExtDecorationsRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn ext_decorations_rate_tracker_record() {
        let mut rt = ExtDecorationsRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn ext_decorations_rate_tracker_prune() {
        let mut rt = ExtDecorationsRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn ext_decorations_validator_valid() {
        let v = ExtDecorationsValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn ext_decorations_validator_errors() {
        let mut v = ExtDecorationsValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn ext_decorations_validator_clear() {
        let mut v = ExtDecorationsValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn ext_decorations_validator_merge() {
        let mut v1 = ExtDecorationsValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = ExtDecorationsValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn ext_decorations_rate_tracker_clear() {
        let mut rt = ExtDecorationsRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zx_metrics_empty() {
        let m = ZxMetrics::new("ext_deco");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zx_metrics_record_and_mean() {
        let mut m = ZxMetrics::new("ext_deco");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zx_metrics_min_max() {
        let mut m = ZxMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zx_metrics_variance_and_std() {
        let mut m = ZxMetrics::new("v");
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
    fn zx_metrics_percentile() {
        let mut m = ZxMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn zx_metrics_merge() {
        let mut a = ZxMetrics::new("a");
        a.record(1.0);
        let mut b = ZxMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn zx_metrics_reset() {
        let mut m = ZxMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn zx_rate_window_empty() {
        let rw = ZxRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn zx_rate_window_tick_and_rate() {
        let mut rw = ZxRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn zx_lru_cache_basic() {
        let mut c = ZxLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn zx_lru_cache_contains_and_keys() {
        let mut c = ZxLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn zx_lru_cache_remove() {
        let mut c = ZxLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn zx_metrics_sum() {
        let mut m = ZxMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zx_metrics_label() {
        let m = ZxMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn zx_lru_cache_clear() {
        let mut c = ZxLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for ext_decorations
    #[test]
    fn xa_ext_decorations_ring_new() {
        let rb = super::XaExtDecorationsRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_decorations_ring_push_len() {
        let mut rb = super::XaExtDecorationsRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_decorations_ring_wrap() {
        let mut rb = super::XaExtDecorationsRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_decorations_ring_mean_empty() {
        let rb = super::XaExtDecorationsRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_decorations_ring_mean_values() {
        let mut rb = super::XaExtDecorationsRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_decorations_ring_min_max() {
        let mut rb = super::XaExtDecorationsRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_decorations_ring_iter() {
        let mut rb = super::XaExtDecorationsRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_decorations_counter_new() {
        let c = super::XaExtDecorationsCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_decorations_counter_inc() {
        let mut c = super::XaExtDecorationsCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_decorations_counter_inc_by() {
        let mut c = super::XaExtDecorationsCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_decorations_counter_reset() {
        let mut c = super::XaExtDecorationsCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_decorations_counter_clear() {
        let mut c = super::XaExtDecorationsCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_decorations_counter_default() {
        let c = super::XaExtDecorationsCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 55 ----

    #[test]
    fn xc_55_pool_new_empty() {
        let pool: super::Xc55Pool<i32> = super::Xc55Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_55_pool_release_acquire() {
        let mut pool = super::Xc55Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_55_pool_acquire_empty() {
        let mut pool: super::Xc55Pool<i32> = super::Xc55Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_55_pool_full() {
        let mut pool = super::Xc55Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_55_pool_drain() {
        let mut pool = super::Xc55Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_55_pool_stats() {
        let mut pool = super::Xc55Pool::new(8);
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
    fn xc_55_pool_clear() {
        let mut pool = super::Xc55Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_55_pool_shrink() {
        let mut pool = super::Xc55Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_55_pool_default() {
        let pool: super::Xc55Pool<String> = super::Xc55Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_55_pool_extend() {
        let mut pool = super::Xc55Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_55_pool_retain() {
        let mut pool = super::Xc55Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_55_scheduler_round_robin() {
        let mut sched = super::Xc55Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_55_scheduler_empty() {
        let mut sched = super::Xc55Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_55_scheduler_reset() {
        let mut sched = super::Xc55Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_55_scheduler_add_remove() {
        let mut sched = super::Xc55Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_55_scheduler_targets() {
        let sched = super::Xc55Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_55_hash_empty() {
        assert_eq!(super::xc_55_hash(b""), 5381);
    }

    #[test]
    fn xc_55_hash_data() {
        let h = super::xc_55_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_55_hash(b"hello"), h);
    }

    #[test]
    fn xc_55_reverse_str() {
        assert_eq!(super::xc_55_reverse("abc"), "cba");
        assert_eq!(super::xc_55_reverse(""), "");
    }


    // --- xd_42 deepening tests ---

    #[test]
    fn xd_42_sm_initial_state() {
        let sm = Xd42StateMachine::new();
        assert_eq!(sm.current_state(), Xd42State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_42_sm_valid_idle_to_running() {
        let mut sm = Xd42StateMachine::new();
        assert!(sm.transition(Xd42State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd42State::Running);
    }

    #[test]
    fn xd_42_sm_valid_running_to_paused() {
        let mut sm = Xd42StateMachine::new();
        sm.transition(Xd42State::Running).unwrap();
        assert!(sm.transition(Xd42State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd42State::Paused);
    }

    #[test]
    fn xd_42_sm_valid_running_to_done() {
        let mut sm = Xd42StateMachine::new();
        sm.transition(Xd42State::Running).unwrap();
        assert!(sm.transition(Xd42State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd42State::Done);
    }

    #[test]
    fn xd_42_sm_valid_paused_to_running() {
        let mut sm = Xd42StateMachine::new();
        sm.transition(Xd42State::Running).unwrap();
        sm.transition(Xd42State::Paused).unwrap();
        assert!(sm.transition(Xd42State::Running).is_ok());
    }

    #[test]
    fn xd_42_sm_valid_done_to_idle() {
        let mut sm = Xd42StateMachine::new();
        sm.transition(Xd42State::Running).unwrap();
        sm.transition(Xd42State::Done).unwrap();
        assert!(sm.transition(Xd42State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd42State::Idle);
    }

    #[test]
    fn xd_42_sm_invalid_idle_to_done() {
        let mut sm = Xd42StateMachine::new();
        assert!(sm.transition(Xd42State::Done).is_err());
    }

    #[test]
    fn xd_42_sm_invalid_idle_to_paused() {
        let mut sm = Xd42StateMachine::new();
        assert!(sm.transition(Xd42State::Paused).is_err());
    }

    #[test]
    fn xd_42_sm_history_tracking() {
        let mut sm = Xd42StateMachine::new();
        sm.transition(Xd42State::Running).unwrap();
        sm.transition(Xd42State::Paused).unwrap();
        sm.transition(Xd42State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd42State::Idle);
        assert_eq!(sm.history()[0].to, Xd42State::Running);
        assert_eq!(sm.history()[1].from, Xd42State::Running);
        assert_eq!(sm.history()[2].to, Xd42State::Done);
    }

    #[test]
    fn xd_42_sm_serialize_deserialize() {
        let mut sm = Xd42StateMachine::new();
        sm.transition(Xd42State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd42StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd42State::Running));
    }

    #[test]
    fn xd_42_sm_deserialize_invalid() {
        assert_eq!(Xd42StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_42_sm_reset() {
        let mut sm = Xd42StateMachine::new();
        sm.transition(Xd42State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd42State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_42_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd42EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd42Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_42_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd42EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd42Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd42Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_42_bus_unsubscribe() {
        let mut bus = Xd42EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_42_event_kind_and_payload() {
        let e = Xd42Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd42Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_42_bus_clear_history() {
        let mut bus = Xd42EventBus::new();
        bus.publish(Xd42Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_42_sm_step_counter_increments() {
        let mut sm = Xd42StateMachine::new();
        sm.transition(Xd42State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd42State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #40 --

    #[test]
    fn xf40_trie_insert_search() {
        let mut t = Xf40Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf40_trie_starts_with() {
        let mut t = Xf40Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf40_trie_remove() {
        let mut t = Xf40Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf40_trie_word_count() {
        let mut t = Xf40Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf40_trie_longest_prefix() {
        let mut t = Xf40Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf40_trie_all_words() {
        let mut t = Xf40Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf40_trie_autocomplete() {
        let mut t = Xf40Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf40_trie_empty_search() {
        let t = Xf40Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf40_bloom_add_contains() {
        let mut bf = Xf40BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf40_bloom_probably_absent() {
        let bf = Xf40BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf40_bloom_false_positive_rate() {
        let mut bf = Xf40BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf40_bloom_clear() {
        let mut bf = Xf40BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf40_bloom_union() {
        let mut a = Xf40BloomFilter::xf_new(512, 2);
        let mut b = Xf40BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf40_bloom_intersection_estimate() {
        let mut a = Xf40BloomFilter::xf_new(512, 2);
        let mut b = Xf40BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf40_bloom_union_size_mismatch() {
        let a = Xf40BloomFilter::xf_new(256, 2);
        let b = Xf40BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh54_skip_insert_contains() {
        let mut sl = super::Xh54SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh54_skip_remove() {
        let mut sl = super::Xh54SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh54_skip_len() {
        let mut sl = super::Xh54SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh54_skip_range_query() {
        let mut sl = super::Xh54SkipList::xh_new(4);
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
    fn xh54_skip_floor_ceiling() {
        let mut sl = super::Xh54SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh54_skip_rank() {
        let mut sl = super::Xh54SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh54_skip_empty() {
        let sl = super::Xh54SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh54_skip_duplicates() {
        let mut sl = super::Xh54SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh54_bitset_set_test() {
        let mut bs = super::Xh54BitSet::xh_new(256);
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
    fn xh54_bitset_clear_count() {
        let mut bs = super::Xh54BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh54_bitset_and_or_xor() {
        let mut a = super::Xh54BitSet::xh_new(128);
        let mut b = super::Xh54BitSet::xh_new(128);
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
    fn xh54_bitset_iter_ones() {
        let mut bs = super::Xh54BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh54_bitset_first_last() {
        let mut bs = super::Xh54BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh54_bitset_empty() {
        let bs = super::Xh54BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi54_deque_push_pop_back() {
        let mut dq = super::Xi54Deque::xi_new(4);
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
    fn xi54_deque_push_pop_front() {
        let mut dq = super::Xi54Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi54_deque_mixed_ops() {
        let mut dq = super::Xi54Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi54_deque_get_and_split() {
        let mut dq = super::Xi54Deque::xi_new(8);
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
    fn xi54_deque_rotate_left() {
        let mut dq = super::Xi54Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi54_deque_rotate_right() {
        let mut dq = super::Xi54Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi54_deque_grow() {
        let mut dq = super::Xi54Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi54_deque_empty() {
        let dq = super::Xi54Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi54_interval_tree_insert_query() {
        let mut tree = super::Xi54IntervalTree::xi_new();
        tree.xi_insert(super::Xi54Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi54Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi54Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi54_interval_tree_overlap() {
        let mut tree = super::Xi54IntervalTree::xi_new();
        tree.xi_insert(super::Xi54Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi54Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi54Interval::xi_new(12, 20));
        let q = super::Xi54Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi54_interval_tree_remove() {
        let mut tree = super::Xi54IntervalTree::xi_new();
        tree.xi_insert(super::Xi54Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi54Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi54_interval_tree_gaps() {
        let mut tree = super::Xi54IntervalTree::xi_new();
        tree.xi_insert(super::Xi54Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi54Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi54Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi54Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi54Interval::xi_new(8, 10));
    }

    #[test]
    fn xi54_interval_tree_merge() {
        let mut tree = super::Xi54IntervalTree::xi_new();
        tree.xi_insert(super::Xi54Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi54Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi54Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi54Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi54Interval::xi_new(10, 15));
    }

    #[test]
    fn xi54_interval_tree_all() {
        let mut tree = super::Xi54IntervalTree::xi_new();
        tree.xi_insert(super::Xi54Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi54Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi54_interval_tree_empty() {
        let tree = super::Xi54IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi54_interval_tree_contains_point() {
        let iv = super::Xi54Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 54) ---

    #[test]
    fn xj_54_uf_make_and_find() {
        let mut uf = super::Xj54UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_54_uf_union_connected() {
        let mut uf = super::Xj54UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_54_uf_component_count() {
        let mut uf = super::Xj54UnionFind::xj_new();
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
    fn xj_54_uf_component_size() {
        let mut uf = super::Xj54UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_54_uf_largest_component() {
        let mut uf = super::Xj54UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_54_uf_many_elements() {
        let mut uf = super::Xj54UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_54_uf_separate_components() {
        let mut uf = super::Xj54UnionFind::xj_new();
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
    fn xj_54_uf_path_compression() {
        let mut uf = super::Xj54UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_54_bt_insert_get() {
        let mut bt = super::Xj54BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_54_bt_contains_len() {
        let mut bt = super::Xj54BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_54_bt_replace() {
        let mut bt = super::Xj54BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_54_bt_remove() {
        let mut bt = super::Xj54BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_54_bt_keys_values() {
        let mut bt = super::Xj54BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_54_bt_range() {
        let mut bt = super::Xj54BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_54_bt_min_max() {
        let mut bt = super::Xj54BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_54_bt_many_inserts() {
        let mut bt = super::Xj54BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_54 segment tree tests ---

    #[test]
    fn xk_54_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk54SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_54_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk54SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_54_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk54SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_54_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk54SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_54_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk54SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_54_st_single_element() {
        let data = vec![42];
        let st = super::Xk54SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_54_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk54SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_54_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk54SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_54 disjoint intervals tests ---

    #[test]
    fn xk_54_di_add_and_count() {
        let mut di = super::Xk54DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_54_di_merge_overlap() {
        let mut di = super::Xk54DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_54_di_contains() {
        let mut di = super::Xk54DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_54_di_remove() {
        let mut di = super::Xk54DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_54_di_covered_length() {
        let mut di = super::Xk54DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_54_di_gaps() {
        let mut di = super::Xk54DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_54_di_merge_adjacent() {
        let mut di = super::Xk54DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_54_di_empty() {
        let di = super::Xk54DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
