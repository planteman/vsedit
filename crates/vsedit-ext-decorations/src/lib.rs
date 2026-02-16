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
}
