//! Accessibility features detection.

use std::collections::{HashMap, VecDeque};
use std::fmt;

/// Whether the platform has a screen reader active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilitySupport {
    Unknown,
    Disabled,
    Enabled,
}

/// High-contrast mode variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighContrastMode {
    None,
    Light,
    Dark,
}

/// Reduced-motion preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReducedMotionMode {
    NoPreference,
    Reduce,
}

/// Priority for screen reader announcements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncementPriority {
    /// Polite – can be interrupted.
    Polite,
    /// Assertive – interrupts current speech.
    Assertive,
}

/// A queued announcement for a screen reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    pub message: String,
    pub priority: AnnouncementPriority,
}

/// Queue of screen reader announcements (FIFO).
pub struct AnnouncementQueue {
    queue: VecDeque<Announcement>,
    max_size: usize,
}

impl AnnouncementQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            max_size,
        }
    }

    pub fn push(&mut self, message: String, priority: AnnouncementPriority) {
        if self.queue.len() >= self.max_size {
            self.queue.pop_front();
        }
        self.queue.push_back(Announcement { message, priority });
    }

    pub fn pop(&mut self) -> Option<Announcement> {
        self.queue.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

/// Configuration for accessibility features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibilityFeaturesConfig {
    pub high_contrast: HighContrastMode,
    pub reduced_motion: ReducedMotionMode,
    pub font_size_override: Option<u32>,
    pub cursor_blinking: bool,
    pub accessibility_support: AccessibilitySupport,
}

impl Default for AccessibilityFeaturesConfig {
    fn default() -> Self {
        Self {
            high_contrast: HighContrastMode::None,
            reduced_motion: ReducedMotionMode::NoPreference,
            font_size_override: None,
            cursor_blinking: true,
            accessibility_support: AccessibilitySupport::Unknown,
        }
    }
}

impl AccessibilityFeaturesConfig {
    /// Apply a high-contrast mode, disabling cursor blinking for better
    /// visibility when any high-contrast theme is active.
    pub fn apply_high_contrast(&mut self, mode: HighContrastMode) {
        self.high_contrast = mode;
        if mode != HighContrastMode::None {
            self.cursor_blinking = false;
        }
    }

    /// Apply a reduced-motion preference, disabling cursor blinking when
    /// motion reduction is requested.
    pub fn apply_reduced_motion(&mut self, mode: ReducedMotionMode) {
        self.reduced_motion = mode;
        if mode == ReducedMotionMode::Reduce {
            self.cursor_blinking = false;
        }
    }

    /// Detect high contrast from an environment variable (e.g. GTK_THEME).
    pub fn detect_high_contrast_from_env(env_value: Option<&str>) -> HighContrastMode {
        match env_value {
            Some(v) if v.contains("HighContrast") && v.contains("Inverse") => {
                HighContrastMode::Dark
            }
            Some(v) if v.contains("HighContrast") => HighContrastMode::Light,
            _ => HighContrastMode::None,
        }
    }

    /// Detect reduced motion preference from an environment variable.
    pub fn detect_reduced_motion_from_env(env_value: Option<&str>) -> ReducedMotionMode {
        match env_value {
            Some("1") | Some("true") | Some("reduce") => ReducedMotionMode::Reduce,
            _ => ReducedMotionMode::NoPreference,
        }
    }

    /// Returns true when a screen reader is known to be active.
    pub fn is_screen_reader_active(&self) -> bool {
        self.accessibility_support == AccessibilitySupport::Enabled
    }

    /// Returns the effective font size, using the override if set, otherwise
    /// falling back to the provided default.
    pub fn effective_font_size(&self, default: u32) -> u32 {
        self.font_size_override.unwrap_or(default)
    }

    /// Returns true when any accessibility accommodation is active (high
    /// contrast, reduced motion, or screen reader).
    pub fn has_any_accommodation(&self) -> bool {
        self.high_contrast != HighContrastMode::None
            || self.reduced_motion == ReducedMotionMode::Reduce
            || self.is_screen_reader_active()
    }

    /// Merge another config on top of this one.  Fields from `other` take
    /// precedence except `font_size_override` which is only overwritten when
    /// `other` supplies a `Some` value.
    pub fn merge(&mut self, other: &AccessibilityFeaturesConfig) {
        self.high_contrast = other.high_contrast;
        self.reduced_motion = other.reduced_motion;
        self.cursor_blinking = other.cursor_blinking;
        self.accessibility_support = other.accessibility_support;
        if other.font_size_override.is_some() {
            self.font_size_override = other.font_size_override;
        }
    }
}

impl fmt::Display for AccessibilityFeaturesConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "A11y[contrast={}, motion={}, font={}, cursor_blink={}, reader={}]",
            self.high_contrast,
            self.reduced_motion,
            match self.font_size_override {
                Some(s) => s.to_string(),
                None => "default".to_string(),
            },
            self.cursor_blinking,
            self.accessibility_support,
        )
    }
}

// ---------------------------------------------------------------------------
// Display impls for enums
// ---------------------------------------------------------------------------

impl fmt::Display for AccessibilitySupport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::Disabled => write!(f, "disabled"),
            Self::Enabled => write!(f, "enabled"),
        }
    }
}

impl fmt::Display for HighContrastMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Light => write!(f, "light"),
            Self::Dark => write!(f, "dark"),
        }
    }
}

impl fmt::Display for ReducedMotionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPreference => write!(f, "no-preference"),
            Self::Reduce => write!(f, "reduce"),
        }
    }
}

impl fmt::Display for AnnouncementPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Polite => write!(f, "polite"),
            Self::Assertive => write!(f, "assertive"),
        }
    }
}

impl fmt::Display for Announcement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.priority, self.message)
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when configuring accessibility features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessibilityError {
    /// Font size is outside the acceptable range.
    InvalidFontSize { size: u32, min: u32, max: u32 },
    /// Announcement message is empty.
    EmptyAnnouncement,
    /// Queue capacity must be at least 1.
    InvalidQueueCapacity,
}

impl fmt::Display for AccessibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFontSize { size, min, max } => {
                write!(f, "font size {size} is outside the valid range {min}..={max}")
            }
            Self::EmptyAnnouncement => write!(f, "announcement message must not be empty"),
            Self::InvalidQueueCapacity => write!(f, "queue capacity must be at least 1"),
        }
    }
}

impl std::error::Error for AccessibilityError {}

// ---------------------------------------------------------------------------
// Builder for AccessibilityFeaturesConfig
// ---------------------------------------------------------------------------

/// Builder for [`AccessibilityFeaturesConfig`] with validation.
#[derive(Debug, Clone)]
pub struct AccessibilityFeaturesConfigBuilder {
    config: AccessibilityFeaturesConfig,
    min_font_size: u32,
    max_font_size: u32,
}

impl Default for AccessibilityFeaturesConfigBuilder {
    fn default() -> Self {
        Self {
            config: AccessibilityFeaturesConfig::default(),
            min_font_size: 6,
            max_font_size: 72,
        }
    }
}

impl AccessibilityFeaturesConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn high_contrast(mut self, mode: HighContrastMode) -> Self {
        self.config.high_contrast = mode;
        self
    }

    pub fn reduced_motion(mut self, mode: ReducedMotionMode) -> Self {
        self.config.reduced_motion = mode;
        self
    }

    pub fn accessibility_support(mut self, support: AccessibilitySupport) -> Self {
        self.config.accessibility_support = support;
        self
    }

    pub fn cursor_blinking(mut self, enabled: bool) -> Self {
        self.config.cursor_blinking = enabled;
        self
    }

    pub fn font_size(mut self, size: u32) -> Self {
        self.config.font_size_override = Some(size);
        self
    }

    pub fn font_size_range(mut self, min: u32, max: u32) -> Self {
        self.min_font_size = min;
        self.max_font_size = max;
        self
    }

    /// Build the configuration, returning an error if validation fails.
    pub fn build(mut self) -> Result<AccessibilityFeaturesConfig, AccessibilityError> {
        if let Some(size) = self.config.font_size_override {
            if size < self.min_font_size || size > self.max_font_size {
                return Err(AccessibilityError::InvalidFontSize {
                    size,
                    min: self.min_font_size,
                    max: self.max_font_size,
                });
            }
        }
        // Automatically disable cursor blinking when accommodations require it.
        if self.config.high_contrast != HighContrastMode::None
            || self.config.reduced_motion == ReducedMotionMode::Reduce
        {
            self.config.cursor_blinking = false;
        }
        Ok(self.config)
    }
}

// ---------------------------------------------------------------------------
// Validated push for AnnouncementQueue
// ---------------------------------------------------------------------------

impl AnnouncementQueue {
    /// Create a queue with validation on the capacity.
    pub fn try_new(max_size: usize) -> Result<Self, AccessibilityError> {
        if max_size == 0 {
            return Err(AccessibilityError::InvalidQueueCapacity);
        }
        Ok(Self {
            queue: VecDeque::new(),
            max_size,
        })
    }

    /// Push an announcement with validation that the message is non-empty.
    pub fn try_push(
        &mut self,
        message: String,
        priority: AnnouncementPriority,
    ) -> Result<(), AccessibilityError> {
        if message.is_empty() {
            return Err(AccessibilityError::EmptyAnnouncement);
        }
        self.push(message, priority);
        Ok(())
    }

    /// Drain all assertive announcements, leaving polite ones in order.
    pub fn drain_assertive(&mut self) -> Vec<Announcement> {
        let mut assertive = Vec::new();
        let mut remaining = VecDeque::new();
        while let Some(a) = self.queue.pop_front() {
            if a.priority == AnnouncementPriority::Assertive {
                assertive.push(a);
            } else {
                remaining.push_back(a);
            }
        }
        self.queue = remaining;
        assertive
    }

    /// Returns the current max capacity of the queue.
    pub fn capacity(&self) -> usize {
        self.max_size
    }
}

impl fmt::Debug for AnnouncementQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnnouncementQueue")
            .field("len", &self.queue.len())
            .field("max_size", &self.max_size)
            .finish()
    }
}

/// Accumulated statistics for a11y-features operations.
#[derive(Debug, Clone, PartialEq)]
pub struct A11YFeaturesStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl A11YFeaturesStats {
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
    pub fn merge(&mut self, other: &A11YFeaturesStats) {
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

impl Default for A11YFeaturesStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for A11YFeaturesStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "A11YFeaturesStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for a11y-features.
#[derive(Debug, Clone)]
pub struct A11YFeaturesValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl A11YFeaturesValidator {
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

impl Default for A11YFeaturesValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ScreenReaderMode
// ---------------------------------------------------------------------------

/// Verbosity level for screen reader announcements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenReaderVerbosity {
    Minimal,
    Normal,
    Verbose,
}

/// Configuration for screen reader optimized rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenReaderMode {
    pub enabled: bool,
    pub line_by_line_navigation: bool,
    pub announce_cursor_position: bool,
    pub announce_selections: bool,
    pub announce_find_results: bool,
    pub verbosity: ScreenReaderVerbosity,
    pub page_size: u32,
}

impl ScreenReaderMode {
    pub fn new() -> Self {
        Self {
            enabled: false,
            line_by_line_navigation: true,
            announce_cursor_position: true,
            announce_selections: true,
            announce_find_results: true,
            verbosity: ScreenReaderVerbosity::Normal,
            page_size: 10,
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn set_verbosity(&mut self, v: ScreenReaderVerbosity) {
        self.verbosity = v;
    }

    pub fn is_verbose(&self) -> bool {
        self.verbosity == ScreenReaderVerbosity::Verbose
    }

    /// Returns whether the given event type should be announced.
    ///
    /// Known event types: `"cursor"`, `"selection"`, `"find"`.
    /// Unknown event types are announced when the mode is enabled.
    pub fn should_announce(&self, event_type: &str) -> bool {
        if !self.enabled {
            return false;
        }
        match event_type {
            "cursor" => self.announce_cursor_position,
            "selection" => self.announce_selections,
            "find" => self.announce_find_results,
            _ => true,
        }
    }

    /// Generate a cursor-position announcement suitable for a screen reader.
    pub fn generate_cursor_announcement(
        &self,
        line: u32,
        col: u32,
        line_content: &str,
    ) -> Option<String> {
        if !self.enabled || !self.announce_cursor_position {
            return None;
        }
        match self.verbosity {
            ScreenReaderVerbosity::Minimal => {
                Some(format!("Line {line}, Column {col}"))
            }
            ScreenReaderVerbosity::Normal => {
                Some(format!("Line {line}, Column {col}: {line_content}"))
            }
            ScreenReaderVerbosity::Verbose => {
                let char_count = line_content.len();
                Some(format!(
                    "Line {line}, Column {col} ({char_count} characters): {line_content}"
                ))
            }
        }
    }
}

impl Default for ScreenReaderMode {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AccessibilityHelp
// ---------------------------------------------------------------------------

/// An entry in the accessibility help dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibilityHelpEntry {
    pub shortcut: String,
    pub description: String,
    pub category: String,
}

/// Manages the accessibility help/keyboard shortcut reference.
pub struct AccessibilityHelp {
    entries: Vec<AccessibilityHelpEntry>,
}

impl AccessibilityHelp {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Create an `AccessibilityHelp` pre-populated with common editor shortcuts.
    pub fn with_defaults() -> Self {
        let mut help = Self::new();
        help.add_entry("Tab", "Indent", "Editing");
        help.add_entry("Shift+Tab", "Outdent", "Editing");
        help.add_entry("Ctrl+F", "Find", "Search");
        help.add_entry("Ctrl+H", "Replace", "Search");
        help.add_entry("Ctrl+G", "Go to Line", "Navigation");
        help.add_entry("Ctrl+/", "Toggle Comment", "Editing");
        help.add_entry("F1", "Command Palette", "General");
        help.add_entry("Escape", "Close Dialog", "General");
        help
    }

    pub fn add_entry(&mut self, shortcut: &str, description: &str, category: &str) {
        self.entries.push(AccessibilityHelpEntry {
            shortcut: shortcut.to_string(),
            description: description.to_string(),
            category: category.to_string(),
        });
    }

    pub fn entries(&self) -> &[AccessibilityHelpEntry] {
        &self.entries
    }

    pub fn entries_by_category(&self, category: &str) -> Vec<&AccessibilityHelpEntry> {
        self.entries
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Return a sorted, deduplicated list of categories.
    pub fn categories(&self) -> Vec<&str> {
        let mut cats: Vec<&str> = self.entries.iter().map(|e| e.category.as_str()).collect();
        cats.sort_unstable();
        cats.dedup();
        cats
    }

    /// Case-insensitive search across shortcut and description fields.
    pub fn search(&self, query: &str) -> Vec<&AccessibilityHelpEntry> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.shortcut.to_lowercase().contains(&q)
                    || e.description.to_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for AccessibilityHelp {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// A11yEvent & a11y_status_announcement
// ---------------------------------------------------------------------------

/// Accessibility events that should be announced to screen readers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum A11yEvent {
    FileOpened { filename: String },
    FileSaved { filename: String },
    FileClosed { filename: String },
    ErrorsChanged { error_count: usize, warning_count: usize },
    SearchResult { current: usize, total: usize, query: String },
    ModeChanged { mode: String },
    IndentationChanged { use_spaces: bool, size: u32 },
    LanguageChanged { language: String },
    LineCountChanged { count: u32 },
    SelectionChanged { lines: u32, chars: u32 },
}

/// Generate a screen reader announcement for important state changes.
/// Returns a formatted announcement string suitable for the announcement queue.
pub fn a11y_status_announcement(event: &A11yEvent) -> String {
    match event {
        A11yEvent::FileOpened { filename } => format!("Opened file: {filename}"),
        A11yEvent::FileSaved { filename } => format!("Saved file: {filename}"),
        A11yEvent::FileClosed { filename } => format!("Closed file: {filename}"),
        A11yEvent::ErrorsChanged {
            error_count,
            warning_count,
        } => format!("{error_count} errors, {warning_count} warnings"),
        A11yEvent::SearchResult {
            current,
            total,
            query,
        } => format!("Result {current} of {total} for \"{query}\""),
        A11yEvent::ModeChanged { mode } => format!("Mode changed to {mode}"),
        A11yEvent::IndentationChanged { use_spaces, size } => {
            let kind = if *use_spaces { "Spaces" } else { "Tabs" };
            format!("Indentation: {kind}, size {size}")
        }
        A11yEvent::LanguageChanged { language } => format!("Language changed to {language}"),
        A11yEvent::LineCountChanged { count } => format!("{count} lines"),
        A11yEvent::SelectionChanged { lines, chars } => {
            format!("Selected {lines} lines, {chars} characters")
        }
    }
}

// ---------------------------------------------------------------------------
// ARIA role/label management
// ---------------------------------------------------------------------------

/// Standard ARIA roles for UI components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AriaRole {
    Button,
    Dialog,
    Menu,
    MenuItem,
    Tab,
    TabPanel,
    TabList,
    TreeItem,
    Tree,
    Toolbar,
    Status,
    Alert,
    Region,
    Navigation,
    Complementary,
    Main,
}

impl fmt::Display for AriaRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Button => "button",
            Self::Dialog => "dialog",
            Self::Menu => "menu",
            Self::MenuItem => "menuitem",
            Self::Tab => "tab",
            Self::TabPanel => "tabpanel",
            Self::TabList => "tablist",
            Self::TreeItem => "treeitem",
            Self::Tree => "tree",
            Self::Toolbar => "toolbar",
            Self::Status => "status",
            Self::Alert => "alert",
            Self::Region => "region",
            Self::Navigation => "navigation",
            Self::Complementary => "complementary",
            Self::Main => "main",
        };
        write!(f, "{s}")
    }
}

/// ARIA attributes attached to a UI component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AriaAttributes {
    pub role: AriaRole,
    pub label: String,
    pub described_by: Option<String>,
    pub live: Option<AnnouncementPriority>,
    pub expanded: Option<bool>,
    pub hidden: bool,
}

impl AriaAttributes {
    pub fn new(role: AriaRole, label: impl Into<String>) -> Self {
        Self {
            role,
            label: label.into(),
            described_by: None,
            live: None,
            expanded: None,
            hidden: false,
        }
    }

    pub fn with_described_by(mut self, desc: impl Into<String>) -> Self {
        self.described_by = Some(desc.into());
        self
    }

    pub fn with_live(mut self, priority: AnnouncementPriority) -> Self {
        self.live = Some(priority);
        self
    }

    pub fn with_expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }
}

/// Registry mapping component IDs to their ARIA attributes.
#[derive(Debug, Clone, Default)]
pub struct AriaRegistry {
    components: HashMap<String, AriaAttributes>,
}

impl AriaRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, id: impl Into<String>, attrs: AriaAttributes) {
        self.components.insert(id.into(), attrs);
    }

    pub fn unregister(&mut self, id: &str) -> Option<AriaAttributes> {
        self.components.remove(id)
    }

    pub fn get(&self, id: &str) -> Option<&AriaAttributes> {
        self.components.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut AriaAttributes> {
        self.components.get_mut(id)
    }

    /// Find all components with a specific role.
    pub fn find_by_role(&self, role: AriaRole) -> Vec<(&str, &AriaAttributes)> {
        self.components
            .iter()
            .filter(|(_, attrs)| attrs.role == role)
            .map(|(id, attrs)| (id.as_str(), attrs))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.components.len()
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Focus trap management for modal dialogs
// ---------------------------------------------------------------------------

/// Represents a focusable element within a focus trap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusableElement {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

/// Manages focus trapping within a modal dialog or panel so that keyboard
/// navigation (Tab / Shift+Tab) cycles only within the trapped elements.
#[derive(Debug, Clone)]
pub struct FocusTrap {
    elements: Vec<FocusableElement>,
    active_index: Option<usize>,
    trap_active: bool,
}

impl FocusTrap {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            active_index: None,
            trap_active: false,
        }
    }

    pub fn activate(&mut self) {
        self.trap_active = true;
        if !self.focusable_indices().is_empty() && self.active_index.is_none() {
            self.active_index = self.focusable_indices().into_iter().next();
        }
    }

    pub fn deactivate(&mut self) {
        self.trap_active = false;
        self.active_index = None;
    }

    pub fn is_active(&self) -> bool {
        self.trap_active
    }

    pub fn add_element(&mut self, id: impl Into<String>, label: impl Into<String>) {
        self.elements.push(FocusableElement {
            id: id.into(),
            label: label.into(),
            enabled: true,
        });
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        if let Some(el) = self.elements.iter_mut().find(|e| e.id == id) {
            el.enabled = enabled;
        }
    }

    fn focusable_indices(&self) -> Vec<usize> {
        self.elements
            .iter()
            .enumerate()
            .filter(|(_, e)| e.enabled)
            .map(|(i, _)| i)
            .collect()
    }

    /// Move focus to the next enabled element, wrapping around.
    pub fn focus_next(&mut self) -> Option<&FocusableElement> {
        if !self.trap_active {
            return None;
        }
        let indices = self.focusable_indices();
        if indices.is_empty() {
            return None;
        }
        let next = match self.active_index {
            Some(cur) => {
                let pos = indices.iter().position(|&i| i == cur).unwrap_or(0);
                indices[(pos + 1) % indices.len()]
            }
            None => indices[0],
        };
        self.active_index = Some(next);
        Some(&self.elements[next])
    }

    /// Move focus to the previous enabled element, wrapping around.
    pub fn focus_prev(&mut self) -> Option<&FocusableElement> {
        if !self.trap_active {
            return None;
        }
        let indices = self.focusable_indices();
        if indices.is_empty() {
            return None;
        }
        let prev = match self.active_index {
            Some(cur) => {
                let pos = indices.iter().position(|&i| i == cur).unwrap_or(0);
                indices[(pos + indices.len() - 1) % indices.len()]
            }
            None => *indices.last().unwrap(),
        };
        self.active_index = Some(prev);
        Some(&self.elements[prev])
    }

    /// Return the currently focused element, if any.
    pub fn focused(&self) -> Option<&FocusableElement> {
        self.active_index
            .and_then(|i| self.elements.get(i))
            .filter(|_| self.trap_active)
    }

    pub fn element_count(&self) -> usize {
        self.elements.len()
    }
}

impl Default for FocusTrap {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Keyboard navigation path tracking
// ---------------------------------------------------------------------------

/// Records the sequence of UI regions the user has navigated through via
/// keyboard shortcuts, enabling breadcrumb-style announcements and
/// back-navigation.
#[derive(Debug, Clone)]
pub struct NavigationPath {
    path: Vec<String>,
    max_depth: usize,
}

impl NavigationPath {
    pub fn new(max_depth: usize) -> Self {
        Self {
            path: Vec::new(),
            max_depth,
        }
    }

    /// Push a new region onto the navigation stack. If the stack exceeds
    /// `max_depth`, the oldest entry is removed.
    pub fn push(&mut self, region: impl Into<String>) {
        if self.path.len() >= self.max_depth {
            self.path.remove(0);
        }
        self.path.push(region.into());
    }

    /// Pop the most recent region, returning it.
    pub fn pop(&mut self) -> Option<String> {
        self.path.pop()
    }

    /// Return the current (most recently pushed) region.
    pub fn current(&self) -> Option<&str> {
        self.path.last().map(|s| s.as_str())
    }

    /// Generate a breadcrumb string like "Editor > File Tree > Search".
    pub fn breadcrumb(&self) -> String {
        self.path.join(" > ")
    }

    pub fn depth(&self) -> usize {
        self.path.len()
    }

    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    pub fn clear(&mut self) {
        self.path.clear();
    }

    /// Return a screen-reader-friendly announcement describing the current
    /// navigation position.
    pub fn announce(&self) -> String {
        match self.current() {
            Some(region) => format!("Navigated to {region}. Path: {}", self.breadcrumb()),
            None => "No navigation history".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Screen-reader adapter
// ---------------------------------------------------------------------------

/// Identifies which screen-reader backend is in use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenReaderBackend {
    /// No screen reader detected.
    None,
    /// Orca (Linux/GNOME).
    Orca,
    /// NVDA (Windows).
    Nvda,
    /// VoiceOver (macOS / iOS).
    VoiceOver,
    /// A custom or third-party screen reader.
    Custom(String),
}

impl fmt::Display for ScreenReaderBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Orca => write!(f, "Orca"),
            Self::Nvda => write!(f, "NVDA"),
            Self::VoiceOver => write!(f, "VoiceOver"),
            Self::Custom(name) => write!(f, "{name}"),
        }
    }
}

/// Thin adapter that formats announcements for a specific screen-reader
/// backend.
#[derive(Debug, Clone)]
pub struct ScreenReaderAdapter {
    pub backend: ScreenReaderBackend,
}

impl ScreenReaderAdapter {
    /// Create a new adapter targeting `backend`.
    pub fn new(backend: ScreenReaderBackend) -> Self {
        Self { backend }
    }

    /// Format `msg` as an announcement string suitable for the current backend.
    pub fn announce(&self, msg: &str) -> String {
        match &self.backend {
            ScreenReaderBackend::None => String::new(),
            ScreenReaderBackend::Orca => format!("[Orca] {msg}"),
            ScreenReaderBackend::Nvda => format!("[NVDA] {msg}"),
            ScreenReaderBackend::VoiceOver => format!("[VoiceOver] {msg}"),
            ScreenReaderBackend::Custom(name) => format!("[{name}] {msg}"),
        }
    }

    /// Returns `true` when a real screen reader is configured.
    pub fn is_active(&self) -> bool {
        self.backend != ScreenReaderBackend::None
    }

    /// Human-readable name of the backend.
    pub fn backend_name(&self) -> &str {
        match &self.backend {
            ScreenReaderBackend::None => "None",
            ScreenReaderBackend::Orca => "Orca",
            ScreenReaderBackend::Nvda => "NVDA",
            ScreenReaderBackend::VoiceOver => "VoiceOver",
            ScreenReaderBackend::Custom(name) => name.as_str(),
        }
    }
}

// ---------------------------------------------------------------------------
// Focus ring
// ---------------------------------------------------------------------------

/// Visual focus-ring indicator used for keyboard-driven navigation.
#[derive(Debug, Clone)]
pub struct A11yFocusRing {
    pub visible: bool,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub thickness: u8,
    pub color: String,
}

impl A11yFocusRing {
    /// Create a focus ring at `(x, y)` with size `(w, h)` using sensible
    /// defaults for thickness and color.
    pub fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self {
            visible: true,
            x,
            y,
            width: w,
            height: h,
            thickness: 2,
            color: "#0078D4".to_string(),
        }
    }

    /// Move the ring to a new origin without changing its size.
    pub fn move_to(&mut self, x: u16, y: u16) {
        self.x = x;
        self.y = y;
    }

    /// Resize the ring without moving it.
    pub fn resize(&mut self, w: u16, h: u16) {
        self.width = w;
        self.height = h;
    }

    /// Make the focus ring visible.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the focus ring.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Returns `true` when the point `(px, py)` falls inside the ring
    /// bounding box.
    pub fn contains_point(&self, px: u16, py: u16) -> bool {
        px >= self.x
            && py >= self.y
            && px < self.x.saturating_add(self.width)
            && py < self.y.saturating_add(self.height)
    }

    /// Total area in pixels covered by the ring bounding box.
    pub fn area(&self) -> u32 {
        self.width as u32 * self.height as u32
    }
}

// ---------------------------------------------------------------------------
// Motion reduction
// ---------------------------------------------------------------------------

/// Controls whether (and how much) animations and transitions are reduced.
#[derive(Debug, Clone)]
pub struct A11yMotionReduction {
    pub enabled: bool,
    pub animation_scale: f32,
    pub transition_duration_ms: u32,
}

impl A11yMotionReduction {
    /// Default instance – animations play normally.
    pub fn new() -> Self {
        Self {
            enabled: false,
            animation_scale: 1.0,
            transition_duration_ms: 200,
        }
    }

    /// Convenience constructor that returns a fully-reduced instance (no
    /// animation, zero-duration transitions).
    pub fn with_reduced() -> Self {
        Self {
            enabled: true,
            animation_scale: 0.0,
            transition_duration_ms: 0,
        }
    }

    /// Returns `true` when animations should still play.
    pub fn should_animate(&self) -> bool {
        !self.enabled && self.animation_scale > 0.0
    }

    /// Compute the effective duration for a transition whose normal length is
    /// `base_ms` milliseconds.
    pub fn effective_duration(&self, base_ms: u32) -> u32 {
        if self.enabled {
            return 0;
        }
        (base_ms as f32 * self.animation_scale) as u32
    }
}

impl Default for A11yMotionReduction {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Keyboard navigation mode
// ---------------------------------------------------------------------------

/// Manages keyboard-driven focus traversal through a set of focusable
/// elements identified by string ids.
#[derive(Debug, Clone)]
pub struct KeyboardNavigationMode {
    pub enabled: bool,
    pub focus_visible: bool,
    pub tab_index: Vec<String>,
}

impl KeyboardNavigationMode {
    pub fn new() -> Self {
        Self {
            enabled: false,
            focus_visible: true,
            tab_index: Vec::new(),
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Register an element as focusable.
    pub fn add_focusable(&mut self, id: &str) {
        self.tab_index.push(id.to_string());
    }

    /// Return the id of the element that follows `current` in tab order, or
    /// `None` if `current` is not found or is the last element.
    pub fn next_focus(&self, current: &str) -> Option<&str> {
        self.tab_index
            .iter()
            .position(|id| id == current)
            .and_then(|i| self.tab_index.get(i + 1))
            .map(|s| s.as_str())
    }

    /// Return the id of the element that precedes `current` in tab order.
    pub fn prev_focus(&self, current: &str) -> Option<&str> {
        self.tab_index
            .iter()
            .position(|id| id == current)
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| self.tab_index.get(i))
            .map(|s| s.as_str())
    }

    /// Number of focusable elements registered.
    pub fn focus_count(&self) -> usize {
        self.tab_index.len()
    }
}

impl Default for KeyboardNavigationMode {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ContrastMode - high contrast mode detector
// ---------------------------------------------------------------------------

/// Severity level for high contrast mode detector issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContrastModeSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ContrastModeSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [ContrastMode].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContrastModeEntry {
    pub id: String,
    pub label: String,
    pub severity: ContrastModeSeverity,
    pub detail: Option<String>,
    pub contrast_level: usize,
    enabled: bool,
}

impl ContrastModeEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: ContrastModeSeverity::Low,
            detail: None,
            contrast_level: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: ContrastModeSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_contrast_level(mut self, val: usize) -> Self {
        self.contrast_level = val;
        self
    }

    pub fn is_high_contrast(&self) -> bool {
        self.enabled && self.severity >= ContrastModeSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.contrast_level, det)
    }
}

impl fmt::Display for ContrastModeEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [ContrastModeEntry] items.
#[derive(Debug, Clone)]
pub struct ContrastMode {
    entries: Vec<ContrastModeEntry>,
    name: String,
    capacity: usize,
}

impl ContrastMode {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: ContrastModeEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<ContrastModeEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&ContrastModeEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn contrast_level(&self) -> usize { self.entries.len() }

    pub fn is_high_contrast(&self) -> bool {
        self.entries.iter().any(|e| e.is_high_contrast())
    }

    pub fn entries_by_severity(&self, severity: ContrastModeSeverity) -> Vec<&ContrastModeEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= ContrastModeSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&ContrastModeEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&ContrastModeEntry> {
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
// ReducedMotionHandler - reduced motion handler
// ---------------------------------------------------------------------------

/// Configuration for [ReducedMotionHandler].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReducedMotionHandlerConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub motion_preference: usize,
}

impl ReducedMotionHandlerConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, motion_preference: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_motion_preference(mut self, val: usize) -> Self { self.motion_preference = val; self }
}

impl Default for ReducedMotionHandlerConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ReducedMotionHandler].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReducedMotionHandlerItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ReducedMotionHandlerItem {
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

    pub fn is_reduced_motion(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ReducedMotionHandlerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ReducedMotionHandlerItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ReducedMotionHandler {
    config: ReducedMotionHandlerConfig,
    items: Vec<ReducedMotionHandlerItem>,
}

impl ReducedMotionHandler {
    pub fn new(config: ReducedMotionHandlerConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ReducedMotionHandlerItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ReducedMotionHandlerItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ReducedMotionHandlerItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn motion_preference(&self) -> usize { self.items.len() }

    pub fn is_reduced_motion(&self) -> bool {
        self.items.iter().any(|i| i.is_reduced_motion())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ReducedMotionHandlerItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ReducedMotionHandlerItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ReducedMotionHandlerConfig {
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
// vsedit-a11y-features: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A11yFeaturesXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl A11yFeaturesXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for A11yFeaturesXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct A11yFeaturesXRegistry {
    entries: Vec<A11yFeaturesXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl A11yFeaturesXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: A11yFeaturesXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&A11yFeaturesXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut A11yFeaturesXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<A11yFeaturesXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&A11yFeaturesXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&A11yFeaturesXConfig> {
        let mut sorted: Vec<&A11yFeaturesXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&A11yFeaturesXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> A11yFeaturesXIterator<'_> {
        A11yFeaturesXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct A11yFeaturesXIterator<'a> {
    inner: std::slice::Iter<'a, A11yFeaturesXConfig>,
}

impl<'a> Iterator for A11yFeaturesXIterator<'a> {
    type Item = &'a A11yFeaturesXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct A11yFeaturesXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl A11yFeaturesXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct A11yFeaturesXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl A11yFeaturesXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &A11yFeaturesXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &A11yFeaturesXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &A11yFeaturesXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for A11yFeaturesXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct A11yFeaturesXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl A11yFeaturesXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &A11yFeaturesXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &A11yFeaturesXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for A11yFeaturesXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for a11y_features
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaA11yFeaturesRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaA11yFeaturesRingBuf {
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
pub struct XaA11yFeaturesCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaA11yFeaturesCounter {
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

impl Default for XaA11yFeaturesCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 1
// ---------------------------------------------------------------------------

/// Generic object pool `Xc1Pool<T>`.
pub struct Xc1Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc1Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc1PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc1Pool<T> {
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
    pub fn stats(&self) -> Xc1PoolStats {
        Xc1PoolStats {
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

impl<T> Default for Xc1Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc1Scheduler`.
pub struct Xc1Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc1Scheduler {
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

impl Default for Xc1Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_1 hash for the given byte slice.
pub fn xc_1_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_1 convention.
pub fn xc_1_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_29 deepening: state machine + event bus ---

/// States for the Xd29 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd29State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd29State {
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
pub struct Xd29Transition {
    pub from: Xd29State,
    pub to: Xd29State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd29StateMachine {
    current: Xd29State,
    history: Vec<Xd29Transition>,
    step_counter: usize,
}

impl Xd29StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd29State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd29State {
        self.current
    }

    pub fn history(&self) -> &[Xd29Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd29State) -> Result<Xd29State, String> {
        let allowed = match (self.current, target) {
            (Xd29State::Idle, Xd29State::Running) => true,
            (Xd29State::Running, Xd29State::Paused) => true,
            (Xd29State::Running, Xd29State::Done) => true,
            (Xd29State::Paused, Xd29State::Running) => true,
            (Xd29State::Paused, Xd29State::Done) => true,
            (Xd29State::Done, Xd29State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_29: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd29Transition {
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
            "Xd29SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd29State> {
        let prefix = "Xd29SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd29State::Idle),
            "Running" => Some(Xd29State::Running),
            "Paused" => Some(Xd29State::Paused),
            "Done" => Some(Xd29State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd29State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd29 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd29Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd29Event {
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

type Xd29HandlerFn = Box<dyn Fn(&Xd29Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd29EventBus {
    handlers: Vec<(usize, Option<String>, Xd29HandlerFn)>,
    next_id: usize,
    published: Vec<Xd29Event>,
}

impl Xd29EventBus {
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
        F: Fn(&Xd29Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd29Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd29Event) {
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

    pub fn published_events(&self) -> &[Xd29Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #27
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf27Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf27TrieNode {
    children: std::collections::HashMap<char, Xf27TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf27Trie {
    root: Xf27TrieNode,
    count: usize,
}

impl Xf27Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf27TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf27TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf27TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf27BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf27BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 0).
pub struct Xh0SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh0SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 42 as u64,
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

/// A compact bit set supporting boolean operations (variant 0).
pub struct Xh0BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh0BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 0).
pub struct Xi0Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi0Deque<T> {
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
pub struct Xi0Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi0Interval {
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

/// A simple interval tree (variant 0).
pub struct Xi0IntervalTree {
    xi_intervals: Vec<Xi0Interval>,
}

impl Xi0IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi0Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi0Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi0Interval) -> Vec<&Xi0Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi0Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi0Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi0Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi0Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi0Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi0Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 0) ---

/// Disjoint set / union-find for crate 0.
pub struct Xj0UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj0UnionFind {
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

const XJ0_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 0.
pub struct Xj0BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj0BTreeNode<K, V>>>,
    len: usize,
}

struct Xj0BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj0BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj0BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ0_BTREE_ORDER - 1
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
        let mid = XJ0_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj0BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj0BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj0BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj0BTreeNode::xj_new_leaf();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = AccessibilityFeaturesConfig::default();
        assert_eq!(cfg.high_contrast, HighContrastMode::None);
        assert_eq!(cfg.reduced_motion, ReducedMotionMode::NoPreference);
        assert!(cfg.font_size_override.is_none());
        assert!(cfg.cursor_blinking);
        assert_eq!(cfg.accessibility_support, AccessibilitySupport::Unknown);
    }

    #[test]
    fn apply_high_contrast_disables_cursor_blinking() {
        let mut cfg = AccessibilityFeaturesConfig::default();
        cfg.apply_high_contrast(HighContrastMode::Dark);
        assert_eq!(cfg.high_contrast, HighContrastMode::Dark);
        assert!(!cfg.cursor_blinking);

        cfg.apply_high_contrast(HighContrastMode::None);
        assert_eq!(cfg.high_contrast, HighContrastMode::None);
    }

    #[test]
    fn apply_reduced_motion_disables_cursor_blinking() {
        let mut cfg = AccessibilityFeaturesConfig::default();
        cfg.apply_reduced_motion(ReducedMotionMode::Reduce);
        assert_eq!(cfg.reduced_motion, ReducedMotionMode::Reduce);
        assert!(!cfg.cursor_blinking);
    }

    #[test]
    fn font_size_override() {
        let mut cfg = AccessibilityFeaturesConfig::default();
        assert!(cfg.font_size_override.is_none());
        cfg.font_size_override = Some(18);
        assert_eq!(cfg.font_size_override, Some(18));
    }

    #[test]
    fn announcement_queue_fifo() {
        let mut q = AnnouncementQueue::new(10);
        q.push("first".into(), AnnouncementPriority::Polite);
        q.push("second".into(), AnnouncementPriority::Assertive);
        assert_eq!(q.len(), 2);
        let a = q.pop().unwrap();
        assert_eq!(a.message, "first");
        assert_eq!(a.priority, AnnouncementPriority::Polite);
        let b = q.pop().unwrap();
        assert_eq!(b.message, "second");
        assert!(q.is_empty());
    }

    #[test]
    fn announcement_queue_overflow() {
        let mut q = AnnouncementQueue::new(2);
        q.push("a".into(), AnnouncementPriority::Polite);
        q.push("b".into(), AnnouncementPriority::Polite);
        q.push("c".into(), AnnouncementPriority::Polite);
        assert_eq!(q.len(), 2);
        assert_eq!(q.pop().unwrap().message, "b");
    }

    #[test]
    fn detect_high_contrast_from_env() {
        assert_eq!(
            AccessibilityFeaturesConfig::detect_high_contrast_from_env(Some("HighContrast")),
            HighContrastMode::Light
        );
        assert_eq!(
            AccessibilityFeaturesConfig::detect_high_contrast_from_env(Some(
                "HighContrastInverse"
            )),
            HighContrastMode::Dark
        );
        assert_eq!(
            AccessibilityFeaturesConfig::detect_high_contrast_from_env(Some("Adwaita")),
            HighContrastMode::None
        );
        assert_eq!(
            AccessibilityFeaturesConfig::detect_high_contrast_from_env(None),
            HighContrastMode::None
        );
    }

    #[test]
    fn detect_reduced_motion_from_env() {
        assert_eq!(
            AccessibilityFeaturesConfig::detect_reduced_motion_from_env(Some("1")),
            ReducedMotionMode::Reduce
        );
        assert_eq!(
            AccessibilityFeaturesConfig::detect_reduced_motion_from_env(Some("reduce")),
            ReducedMotionMode::Reduce
        );
        assert_eq!(
            AccessibilityFeaturesConfig::detect_reduced_motion_from_env(None),
            ReducedMotionMode::NoPreference
        );
    }

    #[test]
    fn screen_reader_detection() {
        let mut cfg = AccessibilityFeaturesConfig::default();
        assert!(!cfg.is_screen_reader_active());
        cfg.accessibility_support = AccessibilitySupport::Enabled;
        assert!(cfg.is_screen_reader_active());
    }

    #[test]
    fn effective_font_size_uses_override() {
        let mut cfg = AccessibilityFeaturesConfig::default();
        assert_eq!(cfg.effective_font_size(14), 14);
        cfg.font_size_override = Some(20);
        assert_eq!(cfg.effective_font_size(14), 20);
    }

    #[test]
    fn has_any_accommodation_reports_correctly() {
        let cfg = AccessibilityFeaturesConfig::default();
        assert!(!cfg.has_any_accommodation());

        let mut hc = AccessibilityFeaturesConfig::default();
        hc.high_contrast = HighContrastMode::Light;
        assert!(hc.has_any_accommodation());

        let mut rm = AccessibilityFeaturesConfig::default();
        rm.reduced_motion = ReducedMotionMode::Reduce;
        assert!(rm.has_any_accommodation());

        let mut sr = AccessibilityFeaturesConfig::default();
        sr.accessibility_support = AccessibilitySupport::Enabled;
        assert!(sr.has_any_accommodation());
    }

    #[test]
    fn merge_preserves_font_size_when_other_is_none() {
        let mut base = AccessibilityFeaturesConfig::default();
        base.font_size_override = Some(16);

        let overlay = AccessibilityFeaturesConfig {
            high_contrast: HighContrastMode::Dark,
            font_size_override: None,
            ..AccessibilityFeaturesConfig::default()
        };
        base.merge(&overlay);
        assert_eq!(base.high_contrast, HighContrastMode::Dark);
        assert_eq!(base.font_size_override, Some(16));
    }

    #[test]
    fn merge_overwrites_font_size_when_other_is_some() {
        let mut base = AccessibilityFeaturesConfig::default();
        base.font_size_override = Some(16);

        let overlay = AccessibilityFeaturesConfig {
            font_size_override: Some(24),
            ..AccessibilityFeaturesConfig::default()
        };
        base.merge(&overlay);
        assert_eq!(base.font_size_override, Some(24));
    }

    #[test]
    fn display_impls_produce_expected_strings() {
        assert_eq!(format!("{}", AccessibilitySupport::Enabled), "enabled");
        assert_eq!(format!("{}", HighContrastMode::Dark), "dark");
        assert_eq!(format!("{}", ReducedMotionMode::Reduce), "reduce");
        assert_eq!(format!("{}", AnnouncementPriority::Assertive), "assertive");

        let ann = Announcement {
            message: "hello".into(),
            priority: AnnouncementPriority::Polite,
        };
        assert_eq!(format!("{ann}"), "[polite] hello");
    }

    #[test]
    fn config_display() {
        let cfg = AccessibilityFeaturesConfig::default();
        let s = format!("{cfg}");
        assert!(s.starts_with("A11y["));
        assert!(s.contains("contrast=none"));
    }

    #[test]
    fn builder_happy_path() {
        let cfg = AccessibilityFeaturesConfigBuilder::new()
            .high_contrast(HighContrastMode::Light)
            .font_size(18)
            .build()
            .unwrap();
        assert_eq!(cfg.high_contrast, HighContrastMode::Light);
        assert_eq!(cfg.font_size_override, Some(18));
        // Builder should auto-disable cursor blinking for high contrast.
        assert!(!cfg.cursor_blinking);
    }

    #[test]
    fn builder_rejects_invalid_font_size() {
        let err = AccessibilityFeaturesConfigBuilder::new()
            .font_size(200)
            .build()
            .unwrap_err();
        assert_eq!(
            err,
            AccessibilityError::InvalidFontSize {
                size: 200,
                min: 6,
                max: 72
            }
        );
        // Display impl should be informative.
        let msg = format!("{err}");
        assert!(msg.contains("200"));
    }

    #[test]
    fn builder_custom_font_range() {
        let result = AccessibilityFeaturesConfigBuilder::new()
            .font_size_range(10, 30)
            .font_size(8)
            .build();
        assert!(result.is_err());

        let ok = AccessibilityFeaturesConfigBuilder::new()
            .font_size_range(10, 30)
            .font_size(15)
            .build();
        assert!(ok.is_ok());
    }

    #[test]
    fn queue_try_new_rejects_zero() {
        let err = AnnouncementQueue::try_new(0).unwrap_err();
        assert_eq!(err, AccessibilityError::InvalidQueueCapacity);
    }

    #[test]
    fn queue_try_push_rejects_empty_message() {
        let mut q = AnnouncementQueue::try_new(5).unwrap();
        let err = q
            .try_push(String::new(), AnnouncementPriority::Polite)
            .unwrap_err();
        assert_eq!(err, AccessibilityError::EmptyAnnouncement);
    }

    #[test]
    fn queue_drain_assertive() {
        let mut q = AnnouncementQueue::new(10);
        q.push("info".into(), AnnouncementPriority::Polite);
        q.push("alert1".into(), AnnouncementPriority::Assertive);
        q.push("note".into(), AnnouncementPriority::Polite);
        q.push("alert2".into(), AnnouncementPriority::Assertive);

        let assertive = q.drain_assertive();
        assert_eq!(assertive.len(), 2);
        assert_eq!(assertive[0].message, "alert1");
        assert_eq!(assertive[1].message, "alert2");
        // Only polite announcements remain.
        assert_eq!(q.len(), 2);
        assert_eq!(q.pop().unwrap().message, "info");
    }

    #[test]
    fn a11y_features_stats_new_defaults() {
        let stats = A11YFeaturesStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn a11y_features_stats_record_success() {
        let mut stats = A11YFeaturesStats::new();
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
    fn a11y_features_stats_record_failure() {
        let mut stats = A11YFeaturesStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn a11y_features_stats_reset() {
        let mut stats = A11YFeaturesStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn a11y_features_stats_merge() {
        let mut a = A11YFeaturesStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = A11YFeaturesStats::new();
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
    fn a11y_features_stats_display() {
        let mut stats = A11YFeaturesStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn a11y_features_stats_default() {
        let stats = A11YFeaturesStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn a11y_features_validator_accepts_valid_name() {
        let v = A11YFeaturesValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn a11y_features_validator_rejects_empty() {
        let v = A11YFeaturesValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn a11y_features_validator_rejects_too_long() {
        let v = A11YFeaturesValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn a11y_features_validator_forbidden_prefix() {
        let v = A11YFeaturesValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn a11y_features_validator_allowed_chars() {
        let v = A11YFeaturesValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn a11y_features_validator_range() {
        let v = A11YFeaturesValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn a11y_features_sanitize_removes_control() {
        let result = A11YFeaturesValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn a11y_features_truncate_short_string() {
        assert_eq!(A11YFeaturesValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn a11y_features_truncate_long_string() {
        let result = A11YFeaturesValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn a11y_features_is_ascii_printable() {
        assert!(A11YFeaturesValidator::is_ascii_printable("Hello World 123"));
        assert!(!A11YFeaturesValidator::is_ascii_printable("Hello\x00World"));
    }

    // -----------------------------------------------------------------------
    // ScreenReaderMode tests
    // -----------------------------------------------------------------------

    #[test]
    fn screen_reader_mode_defaults() {
        let mode = ScreenReaderMode::new();
        assert!(!mode.enabled);
        assert!(mode.line_by_line_navigation);
        assert!(mode.announce_cursor_position);
        assert!(mode.announce_selections);
        assert!(mode.announce_find_results);
        assert_eq!(mode.verbosity, ScreenReaderVerbosity::Normal);
        assert_eq!(mode.page_size, 10);
    }

    #[test]
    fn screen_reader_mode_enable_disable() {
        let mut mode = ScreenReaderMode::new();
        mode.enable();
        assert!(mode.enabled);
        mode.disable();
        assert!(!mode.enabled);
    }

    #[test]
    fn screen_reader_mode_set_verbosity() {
        let mut mode = ScreenReaderMode::new();
        mode.set_verbosity(ScreenReaderVerbosity::Verbose);
        assert!(mode.is_verbose());
        mode.set_verbosity(ScreenReaderVerbosity::Minimal);
        assert!(!mode.is_verbose());
    }

    #[test]
    fn screen_reader_mode_should_announce_disabled() {
        let mode = ScreenReaderMode::new(); // disabled by default
        assert!(!mode.should_announce("cursor"));
        assert!(!mode.should_announce("selection"));
        assert!(!mode.should_announce("find"));
    }

    #[test]
    fn screen_reader_mode_should_announce_enabled() {
        let mut mode = ScreenReaderMode::new();
        mode.enable();
        assert!(mode.should_announce("cursor"));
        assert!(mode.should_announce("selection"));
        assert!(mode.should_announce("find"));
        assert!(mode.should_announce("unknown_event"));
    }

    #[test]
    fn screen_reader_mode_should_announce_selective() {
        let mut mode = ScreenReaderMode::new();
        mode.enable();
        mode.announce_cursor_position = false;
        assert!(!mode.should_announce("cursor"));
        assert!(mode.should_announce("selection"));
    }

    #[test]
    fn screen_reader_mode_cursor_announcement_disabled() {
        let mode = ScreenReaderMode::new();
        assert!(mode.generate_cursor_announcement(1, 1, "hello").is_none());
    }

    #[test]
    fn screen_reader_mode_cursor_announcement_normal() {
        let mut mode = ScreenReaderMode::new();
        mode.enable();
        let ann = mode.generate_cursor_announcement(5, 3, "let x = 1;");
        assert_eq!(ann, Some("Line 5, Column 3: let x = 1;".to_string()));
    }

    #[test]
    fn screen_reader_mode_cursor_announcement_minimal() {
        let mut mode = ScreenReaderMode::new();
        mode.enable();
        mode.set_verbosity(ScreenReaderVerbosity::Minimal);
        let ann = mode.generate_cursor_announcement(2, 7, "fn main()");
        assert_eq!(ann, Some("Line 2, Column 7".to_string()));
    }

    #[test]
    fn screen_reader_mode_cursor_announcement_verbose() {
        let mut mode = ScreenReaderMode::new();
        mode.enable();
        mode.set_verbosity(ScreenReaderVerbosity::Verbose);
        let ann = mode.generate_cursor_announcement(10, 1, "abc").unwrap();
        assert!(ann.contains("3 characters"));
        assert!(ann.contains("Line 10, Column 1"));
    }

    // -----------------------------------------------------------------------
    // AccessibilityHelp tests
    // -----------------------------------------------------------------------

    #[test]
    fn accessibility_help_new_is_empty() {
        let help = AccessibilityHelp::new();
        assert_eq!(help.entry_count(), 0);
        assert!(help.entries().is_empty());
    }

    #[test]
    fn accessibility_help_with_defaults_has_entries() {
        let help = AccessibilityHelp::with_defaults();
        assert!(help.entry_count() >= 8);
    }

    #[test]
    fn accessibility_help_add_entry() {
        let mut help = AccessibilityHelp::new();
        help.add_entry("Ctrl+S", "Save File", "File");
        assert_eq!(help.entry_count(), 1);
        assert_eq!(help.entries()[0].shortcut, "Ctrl+S");
        assert_eq!(help.entries()[0].description, "Save File");
        assert_eq!(help.entries()[0].category, "File");
    }

    #[test]
    fn accessibility_help_entries_by_category() {
        let help = AccessibilityHelp::with_defaults();
        let editing = help.entries_by_category("Editing");
        assert!(editing.len() >= 2);
        for e in &editing {
            assert_eq!(e.category, "Editing");
        }
    }

    #[test]
    fn accessibility_help_categories_sorted_unique() {
        let help = AccessibilityHelp::with_defaults();
        let cats = help.categories();
        assert!(cats.len() >= 3);
        let mut sorted = cats.clone();
        sorted.sort_unstable();
        assert_eq!(cats, sorted);
        // no duplicates
        let mut deduped = cats.clone();
        deduped.dedup();
        assert_eq!(cats, deduped);
    }

    #[test]
    fn accessibility_help_search_case_insensitive() {
        let help = AccessibilityHelp::with_defaults();
        let results = help.search("find");
        assert!(!results.is_empty());
        let results_upper = help.search("FIND");
        assert_eq!(results.len(), results_upper.len());
    }

    #[test]
    fn accessibility_help_search_by_shortcut() {
        let help = AccessibilityHelp::with_defaults();
        let results = help.search("Ctrl+G");
        assert!(!results.is_empty());
        assert!(results.iter().any(|e| e.description == "Go to Line"));
    }

    #[test]
    fn accessibility_help_search_no_results() {
        let help = AccessibilityHelp::with_defaults();
        let results = help.search("zzzznonexistent");
        assert!(results.is_empty());
    }

    // -----------------------------------------------------------------------
    // A11yEvent & a11y_status_announcement tests
    // -----------------------------------------------------------------------

    #[test]
    fn a11y_event_file_opened() {
        let event = A11yEvent::FileOpened { filename: "main.rs".into() };
        assert_eq!(a11y_status_announcement(&event), "Opened file: main.rs");
    }

    #[test]
    fn a11y_event_file_saved() {
        let event = A11yEvent::FileSaved { filename: "lib.rs".into() };
        assert_eq!(a11y_status_announcement(&event), "Saved file: lib.rs");
    }

    #[test]
    fn a11y_event_file_closed() {
        let event = A11yEvent::FileClosed { filename: "test.rs".into() };
        assert_eq!(a11y_status_announcement(&event), "Closed file: test.rs");
    }

    #[test]
    fn a11y_event_errors_changed() {
        let event = A11yEvent::ErrorsChanged { error_count: 3, warning_count: 7 };
        assert_eq!(a11y_status_announcement(&event), "3 errors, 7 warnings");
    }

    #[test]
    fn a11y_event_search_result() {
        let event = A11yEvent::SearchResult {
            current: 2,
            total: 15,
            query: "foo".into(),
        };
        assert_eq!(a11y_status_announcement(&event), "Result 2 of 15 for \"foo\"");
    }

    #[test]
    fn a11y_event_mode_changed() {
        let event = A11yEvent::ModeChanged { mode: "Insert".into() };
        assert_eq!(a11y_status_announcement(&event), "Mode changed to Insert");
    }

    #[test]
    fn a11y_event_indentation_spaces() {
        let event = A11yEvent::IndentationChanged { use_spaces: true, size: 4 };
        assert_eq!(a11y_status_announcement(&event), "Indentation: Spaces, size 4");
    }

    #[test]
    fn a11y_event_indentation_tabs() {
        let event = A11yEvent::IndentationChanged { use_spaces: false, size: 2 };
        assert_eq!(a11y_status_announcement(&event), "Indentation: Tabs, size 2");
    }

    #[test]
    fn a11y_event_language_changed() {
        let event = A11yEvent::LanguageChanged { language: "Rust".into() };
        assert_eq!(a11y_status_announcement(&event), "Language changed to Rust");
    }

    #[test]
    fn a11y_event_line_count_changed() {
        let event = A11yEvent::LineCountChanged { count: 42 };
        assert_eq!(a11y_status_announcement(&event), "42 lines");
    }

    #[test]
    fn a11y_event_selection_changed() {
        let event = A11yEvent::SelectionChanged { lines: 3, chars: 120 };
        assert_eq!(a11y_status_announcement(&event), "Selected 3 lines, 120 characters");
    }

    // -----------------------------------------------------------------------
    // ARIA registry tests
    // -----------------------------------------------------------------------

    #[test]
    fn aria_registry_register_and_find_by_role() {
        let mut reg = AriaRegistry::new();
        reg.register("btn-save", AriaAttributes::new(AriaRole::Button, "Save"));
        reg.register("btn-cancel", AriaAttributes::new(AriaRole::Button, "Cancel"));
        reg.register("main-content", AriaAttributes::new(AriaRole::Main, "Editor"));

        assert_eq!(reg.len(), 3);

        let buttons = reg.find_by_role(AriaRole::Button);
        assert_eq!(buttons.len(), 2);

        let mains = reg.find_by_role(AriaRole::Main);
        assert_eq!(mains.len(), 1);
        assert_eq!(mains[0].1.label, "Editor");

        assert!(reg.find_by_role(AriaRole::Dialog).is_empty());
    }

    #[test]
    fn aria_attributes_builder_methods() {
        let attrs = AriaAttributes::new(AriaRole::Dialog, "Settings")
            .with_described_by("settings-desc")
            .with_live(AnnouncementPriority::Polite)
            .with_expanded(true);

        assert_eq!(attrs.role, AriaRole::Dialog);
        assert_eq!(attrs.label, "Settings");
        assert_eq!(attrs.described_by.as_deref(), Some("settings-desc"));
        assert_eq!(attrs.live, Some(AnnouncementPriority::Polite));
        assert_eq!(attrs.expanded, Some(true));
        assert!(!attrs.hidden);
    }

    // -----------------------------------------------------------------------
    // Focus trap tests
    // -----------------------------------------------------------------------

    #[test]
    fn focus_trap_cycles_through_elements() {
        let mut trap = FocusTrap::new();
        trap.add_element("ok", "OK");
        trap.add_element("cancel", "Cancel");
        trap.add_element("help", "Help");
        trap.activate();

        // First focus_next should land on element 0 (we started there via activate)
        // then advance to 1
        let el = trap.focus_next().unwrap();
        assert_eq!(el.id, "cancel");

        let el = trap.focus_next().unwrap();
        assert_eq!(el.id, "help");

        // Wrap around
        let el = trap.focus_next().unwrap();
        assert_eq!(el.id, "ok");
    }

    #[test]
    fn focus_trap_skips_disabled_elements() {
        let mut trap = FocusTrap::new();
        trap.add_element("a", "A");
        trap.add_element("b", "B");
        trap.add_element("c", "C");
        trap.set_enabled("b", false);
        trap.activate();

        // Starts on "a" (index 0), next skips "b" → "c"
        let el = trap.focus_next().unwrap();
        assert_eq!(el.id, "c");

        // Wraps back to "a"
        let el = trap.focus_next().unwrap();
        assert_eq!(el.id, "a");
    }

    #[test]
    fn focus_trap_prev_wraps() {
        let mut trap = FocusTrap::new();
        trap.add_element("x", "X");
        trap.add_element("y", "Y");
        trap.add_element("z", "Z");
        trap.activate();

        // Currently at index 0, prev wraps to last
        let el = trap.focus_prev().unwrap();
        assert_eq!(el.id, "z");

        let el = trap.focus_prev().unwrap();
        assert_eq!(el.id, "y");
    }

    #[test]
    fn focus_trap_inactive_returns_none() {
        let mut trap = FocusTrap::new();
        trap.add_element("a", "A");
        assert!(!trap.is_active());
        assert!(trap.focus_next().is_none());
        assert!(trap.focused().is_none());
    }

    // -----------------------------------------------------------------------
    // Navigation path tests
    // -----------------------------------------------------------------------

    #[test]
    fn navigation_path_breadcrumb_and_depth() {
        let mut nav = NavigationPath::new(10);
        assert!(nav.is_empty());
        assert_eq!(nav.announce(), "No navigation history");

        nav.push("Editor");
        nav.push("File Tree");
        nav.push("Search");

        assert_eq!(nav.depth(), 3);
        assert_eq!(nav.current(), Some("Search"));
        assert_eq!(nav.breadcrumb(), "Editor > File Tree > Search");
        assert_eq!(
            nav.announce(),
            "Navigated to Search. Path: Editor > File Tree > Search"
        );

        let popped = nav.pop();
        assert_eq!(popped.as_deref(), Some("Search"));
        assert_eq!(nav.current(), Some("File Tree"));
    }

    #[test]
    fn navigation_path_respects_max_depth() {
        let mut nav = NavigationPath::new(3);
        nav.push("A");
        nav.push("B");
        nav.push("C");
        nav.push("D"); // should evict "A"

        assert_eq!(nav.depth(), 3);
        assert_eq!(nav.breadcrumb(), "B > C > D");
    }

    // -----------------------------------------------------------------------
    // ScreenReaderAdapter tests
    // -----------------------------------------------------------------------

    #[test]
    fn screen_reader_none_is_inactive() {
        let adapter = ScreenReaderAdapter::new(ScreenReaderBackend::None);
        assert!(!adapter.is_active());
        assert_eq!(adapter.announce("hello"), "");
        assert_eq!(adapter.backend_name(), "None");
    }

    #[test]
    fn screen_reader_orca_announce() {
        let adapter = ScreenReaderAdapter::new(ScreenReaderBackend::Orca);
        assert!(adapter.is_active());
        assert_eq!(adapter.announce("Save complete"), "[Orca] Save complete");
        assert_eq!(adapter.backend_name(), "Orca");
    }

    #[test]
    fn screen_reader_custom_backend() {
        let adapter =
            ScreenReaderAdapter::new(ScreenReaderBackend::Custom("JAWS".to_string()));
        assert!(adapter.is_active());
        assert_eq!(adapter.announce("hi"), "[JAWS] hi");
        assert_eq!(adapter.backend_name(), "JAWS");
        assert_eq!(format!("{}", adapter.backend), "JAWS");
    }

    #[test]
    fn screen_reader_backend_display() {
        assert_eq!(format!("{}", ScreenReaderBackend::Nvda), "NVDA");
        assert_eq!(format!("{}", ScreenReaderBackend::VoiceOver), "VoiceOver");
        assert_eq!(format!("{}", ScreenReaderBackend::None), "None");
    }

    // -----------------------------------------------------------------------
    // A11yFocusRing tests
    // -----------------------------------------------------------------------

    #[test]
    fn focus_ring_defaults() {
        let ring = A11yFocusRing::new(10, 20, 100, 50);
        assert!(ring.visible);
        assert_eq!(ring.thickness, 2);
        assert_eq!(ring.area(), 5000);
    }

    #[test]
    fn focus_ring_move_and_resize() {
        let mut ring = A11yFocusRing::new(0, 0, 10, 10);
        ring.move_to(5, 5);
        assert_eq!((ring.x, ring.y), (5, 5));
        ring.resize(20, 30);
        assert_eq!(ring.area(), 600);
    }

    #[test]
    fn focus_ring_contains_point() {
        let ring = A11yFocusRing::new(10, 10, 20, 20);
        assert!(ring.contains_point(10, 10));
        assert!(ring.contains_point(29, 29));
        assert!(!ring.contains_point(30, 30));
        assert!(!ring.contains_point(9, 10));
    }

    #[test]
    fn focus_ring_show_hide() {
        let mut ring = A11yFocusRing::new(0, 0, 1, 1);
        ring.hide();
        assert!(!ring.visible);
        ring.show();
        assert!(ring.visible);
    }

    // -----------------------------------------------------------------------
    // A11yMotionReduction tests
    // -----------------------------------------------------------------------

    #[test]
    fn motion_reduction_default_animates() {
        let mr = A11yMotionReduction::new();
        assert!(mr.should_animate());
        assert_eq!(mr.effective_duration(300), 300);
    }

    #[test]
    fn motion_reduction_with_reduced() {
        let mr = A11yMotionReduction::with_reduced();
        assert!(!mr.should_animate());
        assert_eq!(mr.effective_duration(500), 0);
    }

    #[test]
    fn motion_reduction_partial_scale() {
        let mr = A11yMotionReduction {
            enabled: false,
            animation_scale: 0.5,
            transition_duration_ms: 100,
        };
        // scale > 0.0 AND not enabled → should animate
        assert!(mr.should_animate());
        assert_eq!(mr.effective_duration(200), 100);
    }

    // -----------------------------------------------------------------------
    // KeyboardNavigationMode tests
    // -----------------------------------------------------------------------

    #[test]
    fn keyboard_nav_enable_disable() {
        let mut nav = KeyboardNavigationMode::new();
        assert!(!nav.enabled);
        nav.enable();
        assert!(nav.enabled);
        nav.disable();
        assert!(!nav.enabled);
    }

    #[test]
    fn keyboard_nav_focus_traversal() {
        let mut nav = KeyboardNavigationMode::new();
        nav.add_focusable("btn-save");
        nav.add_focusable("btn-cancel");
        nav.add_focusable("input-name");
        assert_eq!(nav.focus_count(), 3);

        assert_eq!(nav.next_focus("btn-save"), Some("btn-cancel"));
        assert_eq!(nav.next_focus("btn-cancel"), Some("input-name"));
        assert_eq!(nav.next_focus("input-name"), None);

        assert_eq!(nav.prev_focus("input-name"), Some("btn-cancel"));
        assert_eq!(nav.prev_focus("btn-save"), None);
    }

    #[test]
    fn keyboard_nav_unknown_element() {
        let nav = KeyboardNavigationMode::new();
        assert_eq!(nav.next_focus("nonexistent"), None);
        assert_eq!(nav.prev_focus("nonexistent"), None);
    }

#[test]
    fn contrastmode_severity_ordering() {
        assert!(ContrastModeSeverity::Critical > ContrastModeSeverity::High);
        assert!(ContrastModeSeverity::High > ContrastModeSeverity::Medium);
        assert!(ContrastModeSeverity::Medium > ContrastModeSeverity::Low);
    }

    #[test]
    fn contrastmode_severity_display() {
        assert_eq!(ContrastModeSeverity::Low.to_string(), "low");
        assert_eq!(ContrastModeSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn contrastmode_entry_creation() {
        let e = ContrastModeEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, ContrastModeSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn contrastmode_entry_builder() {
        let e = ContrastModeEntry::new("e2", "Entry 2")
            .with_severity(ContrastModeSeverity::High)
            .with_detail("some detail")
            .with_contrast_level(42);
        assert_eq!(e.severity, ContrastModeSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.contrast_level, 42);
    }

    #[test]
    fn contrastmode_entry_enable_disable() {
        let mut e = ContrastModeEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn contrastmode_add_and_count() {
        let mut mgr = ContrastMode::new("test");
        mgr.add(ContrastModeEntry::new("a", "A"));
        mgr.add(ContrastModeEntry::new("b", "B").with_severity(ContrastModeSeverity::High));
        assert_eq!(mgr.contrast_level(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn contrastmode_remove() {
        let mut mgr = ContrastMode::new("test");
        mgr.add(ContrastModeEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn contrastmode_capacity() {
        let mut mgr = ContrastMode::new("test").with_capacity(1);
        assert!(mgr.add(ContrastModeEntry::new("a", "A")));
        assert!(!mgr.add(ContrastModeEntry::new("b", "B")));
    }

    #[test]
    fn contrastmode_sorted_by_severity() {
        let mut mgr = ContrastMode::new("test");
        mgr.add(ContrastModeEntry::new("lo", "Low"));
        mgr.add(ContrastModeEntry::new("hi", "High").with_severity(ContrastModeSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, ContrastModeSeverity::Critical);
    }

    #[test]
    fn contrastmode_summary() {
        let mgr = ContrastMode::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn reducedmotionhandler_config_defaults() {
        let cfg = ReducedMotionHandlerConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn reducedmotionhandler_item_creation() {
        let item = ReducedMotionHandlerItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn reducedmotionhandler_add_and_get() {
        let mut mgr = ReducedMotionHandler::new(ReducedMotionHandlerConfig::new("test"));
        mgr.add(ReducedMotionHandlerItem::new("k1", "v1"));
        assert_eq!(mgr.motion_preference(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn reducedmotionhandler_remove_item() {
        let mut mgr = ReducedMotionHandler::new(ReducedMotionHandlerConfig::new("test"));
        mgr.add(ReducedMotionHandlerItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn reducedmotionhandler_sorted_by_priority() {
        let mut mgr = ReducedMotionHandler::new(ReducedMotionHandlerConfig::new("test"));
        mgr.add(ReducedMotionHandlerItem::new("lo", "low").with_priority(1));
        mgr.add(ReducedMotionHandlerItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn reducedmotionhandler_items_with_tag() {
        let mut mgr = ReducedMotionHandler::new(ReducedMotionHandlerConfig::new("test"));
        mgr.add(ReducedMotionHandlerItem::new("a", "1").with_tag("x"));
        mgr.add(ReducedMotionHandlerItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn reducedmotionhandler_report() {
        let mgr = ReducedMotionHandler::new(ReducedMotionHandlerConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn a11yFeatures_x_config_new() {
        let c = A11yFeaturesXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn a11yFeatures_x_config_builder() {
        let c = A11yFeaturesXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn a11yFeatures_x_config_display() {
        let c = A11yFeaturesXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn a11yFeatures_x_registry_insert_get() {
        let mut reg = A11yFeaturesXRegistry::new();
        reg.insert(A11yFeaturesXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn a11yFeatures_x_registry_duplicate() {
        let mut reg = A11yFeaturesXRegistry::new();
        reg.insert(A11yFeaturesXConfig::new("a")).unwrap();
        assert!(reg.insert(A11yFeaturesXConfig::new("a")).is_err());
    }

    #[test]
    fn a11yFeatures_x_registry_remove() {
        let mut reg = A11yFeaturesXRegistry::new();
        reg.insert(A11yFeaturesXConfig::new("a")).unwrap();
        reg.insert(A11yFeaturesXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn a11yFeatures_x_registry_active_entries() {
        let mut reg = A11yFeaturesXRegistry::new();
        reg.insert(A11yFeaturesXConfig::new("a")).unwrap();
        reg.insert(A11yFeaturesXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn a11yFeatures_x_registry_by_weight() {
        let mut reg = A11yFeaturesXRegistry::new();
        reg.insert(A11yFeaturesXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(A11yFeaturesXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn a11yFeatures_x_registry_tags() {
        let mut reg = A11yFeaturesXRegistry::new();
        reg.insert(A11yFeaturesXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(A11yFeaturesXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn a11yFeatures_x_registry_total_weight() {
        let mut reg = A11yFeaturesXRegistry::new();
        reg.insert(A11yFeaturesXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(A11yFeaturesXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn a11yFeatures_x_registry_iterator() {
        let mut reg = A11yFeaturesXRegistry::new();
        reg.insert(A11yFeaturesXConfig::new("a")).unwrap();
        reg.insert(A11yFeaturesXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn a11yFeatures_x_cache_put_get() {
        let mut cache = A11yFeaturesXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn a11yFeatures_x_cache_eviction() {
        let mut cache = A11yFeaturesXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn a11yFeatures_x_cache_lru_order() {
        let mut cache = A11yFeaturesXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn a11yFeatures_x_cache_most_least_recent() {
        let mut cache = A11yFeaturesXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn a11yFeatures_x_formatter_entry() {
        let e = A11yFeaturesXConfig::new("k").with_value("v");
        let fmt = A11yFeaturesXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn a11yFeatures_x_formatter_summary() {
        let mut reg = A11yFeaturesXRegistry::new();
        reg.insert(A11yFeaturesXConfig::new("a").with_weight(5)).unwrap();
        let fmt = A11yFeaturesXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn a11yFeatures_x_validator_valid() {
        let v = A11yFeaturesXValidator::new();
        let c = A11yFeaturesXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn a11yFeatures_x_validator_empty_key() {
        let v = A11yFeaturesXValidator::new();
        let c = A11yFeaturesXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn a11yFeatures_x_validator_require_value() {
        let v = A11yFeaturesXValidator::new().require_value(true);
        let c = A11yFeaturesXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn a11yFeatures_x_validator_allowed_tags() {
        let v = A11yFeaturesXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = A11yFeaturesXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn a11yFeatures_x_validator_validate_all() {
        let v = A11yFeaturesXValidator::new();
        let mut reg = A11yFeaturesXRegistry::new();
        reg.insert(A11yFeaturesXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    // xa_ extended tests for a11y_features
    #[test]
    fn xa_a11y_features_ring_new() {
        let rb = super::XaA11yFeaturesRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_a11y_features_ring_push_len() {
        let mut rb = super::XaA11yFeaturesRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_a11y_features_ring_wrap() {
        let mut rb = super::XaA11yFeaturesRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_a11y_features_ring_mean_empty() {
        let rb = super::XaA11yFeaturesRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_a11y_features_ring_mean_values() {
        let mut rb = super::XaA11yFeaturesRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_a11y_features_ring_min_max() {
        let mut rb = super::XaA11yFeaturesRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_a11y_features_ring_iter() {
        let mut rb = super::XaA11yFeaturesRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_a11y_features_counter_new() {
        let c = super::XaA11yFeaturesCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_a11y_features_counter_inc() {
        let mut c = super::XaA11yFeaturesCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_a11y_features_counter_inc_by() {
        let mut c = super::XaA11yFeaturesCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_a11y_features_counter_reset() {
        let mut c = super::XaA11yFeaturesCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_a11y_features_counter_clear() {
        let mut c = super::XaA11yFeaturesCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_a11y_features_counter_default() {
        let c = super::XaA11yFeaturesCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 1 ----

    #[test]
    fn xc_1_pool_new_empty() {
        let pool: super::Xc1Pool<i32> = super::Xc1Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_1_pool_release_acquire() {
        let mut pool = super::Xc1Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_1_pool_acquire_empty() {
        let mut pool: super::Xc1Pool<i32> = super::Xc1Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_1_pool_full() {
        let mut pool = super::Xc1Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_1_pool_drain() {
        let mut pool = super::Xc1Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_1_pool_stats() {
        let mut pool = super::Xc1Pool::new(8);
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
    fn xc_1_pool_clear() {
        let mut pool = super::Xc1Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_1_pool_shrink() {
        let mut pool = super::Xc1Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_1_pool_default() {
        let pool: super::Xc1Pool<String> = super::Xc1Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_1_pool_extend() {
        let mut pool = super::Xc1Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_1_pool_retain() {
        let mut pool = super::Xc1Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_1_scheduler_round_robin() {
        let mut sched = super::Xc1Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_1_scheduler_empty() {
        let mut sched = super::Xc1Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_1_scheduler_reset() {
        let mut sched = super::Xc1Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_1_scheduler_add_remove() {
        let mut sched = super::Xc1Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_1_scheduler_targets() {
        let sched = super::Xc1Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_1_hash_empty() {
        assert_eq!(super::xc_1_hash(b""), 5381);
    }

    #[test]
    fn xc_1_hash_data() {
        let h = super::xc_1_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_1_hash(b"hello"), h);
    }

    #[test]
    fn xc_1_reverse_str() {
        assert_eq!(super::xc_1_reverse("abc"), "cba");
        assert_eq!(super::xc_1_reverse(""), "");
    }


    // --- xd_29 deepening tests ---

    #[test]
    fn xd_29_sm_initial_state() {
        let sm = Xd29StateMachine::new();
        assert_eq!(sm.current_state(), Xd29State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_29_sm_valid_idle_to_running() {
        let mut sm = Xd29StateMachine::new();
        assert!(sm.transition(Xd29State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd29State::Running);
    }

    #[test]
    fn xd_29_sm_valid_running_to_paused() {
        let mut sm = Xd29StateMachine::new();
        sm.transition(Xd29State::Running).unwrap();
        assert!(sm.transition(Xd29State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd29State::Paused);
    }

    #[test]
    fn xd_29_sm_valid_running_to_done() {
        let mut sm = Xd29StateMachine::new();
        sm.transition(Xd29State::Running).unwrap();
        assert!(sm.transition(Xd29State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd29State::Done);
    }

    #[test]
    fn xd_29_sm_valid_paused_to_running() {
        let mut sm = Xd29StateMachine::new();
        sm.transition(Xd29State::Running).unwrap();
        sm.transition(Xd29State::Paused).unwrap();
        assert!(sm.transition(Xd29State::Running).is_ok());
    }

    #[test]
    fn xd_29_sm_valid_done_to_idle() {
        let mut sm = Xd29StateMachine::new();
        sm.transition(Xd29State::Running).unwrap();
        sm.transition(Xd29State::Done).unwrap();
        assert!(sm.transition(Xd29State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd29State::Idle);
    }

    #[test]
    fn xd_29_sm_invalid_idle_to_done() {
        let mut sm = Xd29StateMachine::new();
        assert!(sm.transition(Xd29State::Done).is_err());
    }

    #[test]
    fn xd_29_sm_invalid_idle_to_paused() {
        let mut sm = Xd29StateMachine::new();
        assert!(sm.transition(Xd29State::Paused).is_err());
    }

    #[test]
    fn xd_29_sm_history_tracking() {
        let mut sm = Xd29StateMachine::new();
        sm.transition(Xd29State::Running).unwrap();
        sm.transition(Xd29State::Paused).unwrap();
        sm.transition(Xd29State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd29State::Idle);
        assert_eq!(sm.history()[0].to, Xd29State::Running);
        assert_eq!(sm.history()[1].from, Xd29State::Running);
        assert_eq!(sm.history()[2].to, Xd29State::Done);
    }

    #[test]
    fn xd_29_sm_serialize_deserialize() {
        let mut sm = Xd29StateMachine::new();
        sm.transition(Xd29State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd29StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd29State::Running));
    }

    #[test]
    fn xd_29_sm_deserialize_invalid() {
        assert_eq!(Xd29StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_29_sm_reset() {
        let mut sm = Xd29StateMachine::new();
        sm.transition(Xd29State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd29State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_29_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd29EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd29Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_29_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd29EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd29Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd29Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_29_bus_unsubscribe() {
        let mut bus = Xd29EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_29_event_kind_and_payload() {
        let e = Xd29Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd29Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_29_bus_clear_history() {
        let mut bus = Xd29EventBus::new();
        bus.publish(Xd29Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_29_sm_step_counter_increments() {
        let mut sm = Xd29StateMachine::new();
        sm.transition(Xd29State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd29State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #27 --

    #[test]
    fn xf27_trie_insert_search() {
        let mut t = Xf27Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf27_trie_starts_with() {
        let mut t = Xf27Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf27_trie_remove() {
        let mut t = Xf27Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf27_trie_word_count() {
        let mut t = Xf27Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf27_trie_longest_prefix() {
        let mut t = Xf27Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf27_trie_all_words() {
        let mut t = Xf27Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf27_trie_autocomplete() {
        let mut t = Xf27Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf27_trie_empty_search() {
        let t = Xf27Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf27_bloom_add_contains() {
        let mut bf = Xf27BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf27_bloom_probably_absent() {
        let bf = Xf27BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf27_bloom_false_positive_rate() {
        let mut bf = Xf27BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf27_bloom_clear() {
        let mut bf = Xf27BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf27_bloom_union() {
        let mut a = Xf27BloomFilter::xf_new(512, 2);
        let mut b = Xf27BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf27_bloom_intersection_estimate() {
        let mut a = Xf27BloomFilter::xf_new(512, 2);
        let mut b = Xf27BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf27_bloom_union_size_mismatch() {
        let a = Xf27BloomFilter::xf_new(256, 2);
        let b = Xf27BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh0_skip_insert_contains() {
        let mut sl = super::Xh0SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh0_skip_remove() {
        let mut sl = super::Xh0SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh0_skip_len() {
        let mut sl = super::Xh0SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh0_skip_range_query() {
        let mut sl = super::Xh0SkipList::xh_new(4);
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
    fn xh0_skip_floor_ceiling() {
        let mut sl = super::Xh0SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh0_skip_rank() {
        let mut sl = super::Xh0SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh0_skip_empty() {
        let sl = super::Xh0SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh0_skip_duplicates() {
        let mut sl = super::Xh0SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh0_bitset_set_test() {
        let mut bs = super::Xh0BitSet::xh_new(256);
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
    fn xh0_bitset_clear_count() {
        let mut bs = super::Xh0BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh0_bitset_and_or_xor() {
        let mut a = super::Xh0BitSet::xh_new(128);
        let mut b = super::Xh0BitSet::xh_new(128);
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
    fn xh0_bitset_iter_ones() {
        let mut bs = super::Xh0BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh0_bitset_first_last() {
        let mut bs = super::Xh0BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh0_bitset_empty() {
        let bs = super::Xh0BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi0_deque_push_pop_back() {
        let mut dq = super::Xi0Deque::xi_new(4);
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
    fn xi0_deque_push_pop_front() {
        let mut dq = super::Xi0Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi0_deque_mixed_ops() {
        let mut dq = super::Xi0Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi0_deque_get_and_split() {
        let mut dq = super::Xi0Deque::xi_new(8);
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
    fn xi0_deque_rotate_left() {
        let mut dq = super::Xi0Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi0_deque_rotate_right() {
        let mut dq = super::Xi0Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi0_deque_grow() {
        let mut dq = super::Xi0Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi0_deque_empty() {
        let dq = super::Xi0Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi0_interval_tree_insert_query() {
        let mut tree = super::Xi0IntervalTree::xi_new();
        tree.xi_insert(super::Xi0Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi0Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi0Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi0_interval_tree_overlap() {
        let mut tree = super::Xi0IntervalTree::xi_new();
        tree.xi_insert(super::Xi0Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi0Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi0Interval::xi_new(12, 20));
        let q = super::Xi0Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi0_interval_tree_remove() {
        let mut tree = super::Xi0IntervalTree::xi_new();
        tree.xi_insert(super::Xi0Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi0Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi0_interval_tree_gaps() {
        let mut tree = super::Xi0IntervalTree::xi_new();
        tree.xi_insert(super::Xi0Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi0Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi0Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi0Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi0Interval::xi_new(8, 10));
    }

    #[test]
    fn xi0_interval_tree_merge() {
        let mut tree = super::Xi0IntervalTree::xi_new();
        tree.xi_insert(super::Xi0Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi0Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi0Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi0Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi0Interval::xi_new(10, 15));
    }

    #[test]
    fn xi0_interval_tree_all() {
        let mut tree = super::Xi0IntervalTree::xi_new();
        tree.xi_insert(super::Xi0Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi0Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi0_interval_tree_empty() {
        let tree = super::Xi0IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi0_interval_tree_contains_point() {
        let iv = super::Xi0Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 0) ---

    #[test]
    fn xj_0_uf_make_and_find() {
        let mut uf = super::Xj0UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_0_uf_union_connected() {
        let mut uf = super::Xj0UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_0_uf_component_count() {
        let mut uf = super::Xj0UnionFind::xj_new();
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
    fn xj_0_uf_component_size() {
        let mut uf = super::Xj0UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_0_uf_largest_component() {
        let mut uf = super::Xj0UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_0_uf_many_elements() {
        let mut uf = super::Xj0UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_0_uf_separate_components() {
        let mut uf = super::Xj0UnionFind::xj_new();
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
    fn xj_0_uf_path_compression() {
        let mut uf = super::Xj0UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_0_bt_insert_get() {
        let mut bt = super::Xj0BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_0_bt_contains_len() {
        let mut bt = super::Xj0BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_0_bt_replace() {
        let mut bt = super::Xj0BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_0_bt_remove() {
        let mut bt = super::Xj0BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_0_bt_keys_values() {
        let mut bt = super::Xj0BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_0_bt_range() {
        let mut bt = super::Xj0BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_0_bt_min_max() {
        let mut bt = super::Xj0BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_0_bt_many_inserts() {
        let mut bt = super::Xj0BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }

}
