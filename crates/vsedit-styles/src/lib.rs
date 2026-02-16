//! Theme color system mapping VS Code colors to terminal colors.

use std::collections::HashMap;
use std::fmt;

// Re-export ratatui colors
pub use vsedit_tui::{Color, Modifier, Style};

/// A named color token from a VS Code theme.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThemeColor(pub String);

impl ThemeColor {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Construct from a string slice.
    pub fn from_str(id: &str) -> Self {
        Self(id.to_string())
    }
}

impl fmt::Display for ThemeColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Default theme colors for the terminal.
pub struct ThemeDefaults;

impl ThemeDefaults {
    pub fn editor_background() -> Color {
        Color::Reset
    }
    pub fn editor_foreground() -> Color {
        Color::Reset
    }
    pub fn editor_line_number() -> Color {
        Color::DarkGray
    }
    pub fn editor_cursor() -> Color {
        Color::White
    }
    pub fn editor_selection() -> Color {
        Color::Rgb(38, 79, 120)
    }
    pub fn sidebar_background() -> Color {
        Color::Rgb(37, 37, 38)
    }
    pub fn sidebar_foreground() -> Color {
        Color::Rgb(204, 204, 204)
    }
    pub fn statusbar_background() -> Color {
        Color::Rgb(0, 122, 204)
    }
    pub fn statusbar_foreground() -> Color {
        Color::White
    }
    pub fn tab_active_background() -> Color {
        Color::Rgb(30, 30, 30)
    }
    pub fn tab_inactive_background() -> Color {
        Color::Rgb(45, 45, 45)
    }
    pub fn panel_background() -> Color {
        Color::Rgb(30, 30, 30)
    }
    pub fn error_foreground() -> Color {
        Color::Red
    }
    pub fn warning_foreground() -> Color {
        Color::Yellow
    }
    pub fn info_foreground() -> Color {
        Color::Blue
    }
    pub fn accent() -> Color {
        Color::Rgb(0, 122, 204)
    }
    pub fn border() -> Color {
        Color::Rgb(68, 68, 68)
    }
}

/// Build a Style from theme defaults.
pub fn editor_style() -> Style {
    Style::default()
        .fg(ThemeDefaults::editor_foreground())
        .bg(ThemeDefaults::editor_background())
}

pub fn line_number_style() -> Style {
    Style::default().fg(ThemeDefaults::editor_line_number())
}

pub fn selection_style() -> Style {
    Style::default().bg(ThemeDefaults::editor_selection())
}

pub fn statusbar_style() -> Style {
    Style::default()
        .fg(ThemeDefaults::statusbar_foreground())
        .bg(ThemeDefaults::statusbar_background())
}

pub fn sidebar_style() -> Style {
    Style::default()
        .fg(ThemeDefaults::sidebar_foreground())
        .bg(ThemeDefaults::sidebar_background())
}

pub fn error_style() -> Style {
    Style::default().fg(ThemeDefaults::error_foreground())
}

pub fn warning_style() -> Style {
    Style::default().fg(ThemeDefaults::warning_foreground())
}

pub fn info_style() -> Style {
    Style::default().fg(ThemeDefaults::info_foreground())
}

pub fn panel_style() -> Style {
    Style::default()
        .fg(ThemeDefaults::editor_foreground())
        .bg(ThemeDefaults::panel_background())
}

pub fn tab_active_style() -> Style {
    Style::default()
        .fg(ThemeDefaults::editor_foreground())
        .bg(ThemeDefaults::tab_active_background())
        .add_modifier(Modifier::BOLD)
}

pub fn tab_inactive_style() -> Style {
    Style::default()
        .fg(ThemeDefaults::sidebar_foreground())
        .bg(ThemeDefaults::tab_inactive_background())
}

pub fn cursor_style() -> Style {
    Style::default()
        .fg(ThemeDefaults::editor_background())
        .bg(ThemeDefaults::editor_cursor())
}

pub fn border_style() -> Style {
    Style::default().fg(ThemeDefaults::border())
}

pub fn accent_style() -> Style {
    Style::default().fg(ThemeDefaults::accent())
}

/// Maps `ThemeColor` tokens to resolved `Style` values.
pub struct ThemeColorResolver {
    map: HashMap<ThemeColor, Style>,
}

impl ThemeColorResolver {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Register a style for a given theme color token.
    pub fn register(&mut self, color: ThemeColor, style: Style) {
        self.map.insert(color, style);
    }

    /// Resolve a theme color to its registered style, falling back to default.
    pub fn resolve(&self, color: &ThemeColor) -> Style {
        self.map.get(color).copied().unwrap_or_default()
    }

    /// Number of registered mappings.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns true if no mappings are registered.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl Default for ThemeColorResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Incremental builder for constructing a `Style`.
pub struct StyleBuilder {
    style: Style,
}

impl StyleBuilder {
    pub fn new() -> Self {
        Self {
            style: Style::default(),
        }
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.style = self.style.fg(color);
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.style = self.style.bg(color);
        self
    }

    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.style = self.style.add_modifier(modifier);
        self
    }

    pub fn build(self) -> Style {
        self.style
    }
}

impl Default for StyleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ThemeColorResolver extras
// ---------------------------------------------------------------------------

impl ThemeColorResolver {
    /// Register multiple color-style mappings at once.
    pub fn register_many(&mut self, mappings: Vec<(ThemeColor, Style)>) {
        for (color, style) in mappings {
            self.register(color, style);
        }
    }

    /// Check whether a given theme color has a registered style.
    pub fn contains(&self, color: &ThemeColor) -> bool {
        self.map.contains_key(color)
    }

    /// Remove a registered mapping. Returns the removed style if it existed.
    pub fn unregister(&mut self, color: &ThemeColor) -> Option<Style> {
        self.map.remove(color)
    }

    /// Get all registered theme color tokens.
    pub fn tokens(&self) -> Vec<&ThemeColor> {
        self.map.keys().collect()
    }

    /// Clear all registered mappings.
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

// ---------------------------------------------------------------------------
// ColorPalette — named palette of Color values
// ---------------------------------------------------------------------------

/// A named collection of colors for easy theming.
#[derive(Debug, Clone)]
pub struct ColorPalette {
    colors: HashMap<String, Color>,
}

impl ColorPalette {
    pub fn new() -> Self {
        Self {
            colors: HashMap::new(),
        }
    }

    /// Insert a named color.
    pub fn set(&mut self, name: impl Into<String>, color: Color) {
        self.colors.insert(name.into(), color);
    }

    /// Get a named color.
    pub fn get(&self, name: &str) -> Option<Color> {
        self.colors.get(name).copied()
    }

    /// Number of colors in the palette.
    pub fn len(&self) -> usize {
        self.colors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    /// Get all color names.
    pub fn names(&self) -> Vec<&String> {
        self.colors.keys().collect()
    }

    /// Create a default dark palette.
    pub fn dark_default() -> Self {
        let mut p = Self::new();
        p.set("background", Color::Rgb(30, 30, 30));
        p.set("foreground", Color::Rgb(212, 212, 212));
        p.set("accent", Color::Rgb(0, 122, 204));
        p.set("error", Color::Red);
        p.set("warning", Color::Yellow);
        p.set("info", Color::Blue);
        p.set("success", Color::Green);
        p.set("border", Color::Rgb(68, 68, 68));
        p
    }

    /// Create a default light palette.
    pub fn light_default() -> Self {
        let mut p = Self::new();
        p.set("background", Color::Rgb(255, 255, 255));
        p.set("foreground", Color::Rgb(30, 30, 30));
        p.set("accent", Color::Rgb(0, 102, 204));
        p.set("error", Color::Red);
        p.set("warning", Color::Rgb(191, 134, 0));
        p.set("info", Color::Rgb(0, 102, 204));
        p.set("success", Color::Rgb(22, 130, 22));
        p.set("border", Color::Rgb(200, 200, 200));
        p
    }
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self::dark_default()
    }
}

// ---------------------------------------------------------------------------
// Helpers for parsing hex colors
// ---------------------------------------------------------------------------

/// Parse a hex color string like "#rrggbb" into a Color::Rgb.
/// Returns None on invalid input.
pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

/// Blend two RGB colors by a factor (0.0 = all a, 1.0 = all b).
/// Returns None if either color is not Rgb.
pub fn blend_colors(a: Color, b: Color, factor: f32) -> Option<Color> {
    if let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (a, b) {
        let f = factor.clamp(0.0, 1.0);
        let r = (r1 as f32 * (1.0 - f) + r2 as f32 * f) as u8;
        let g = (g1 as f32 * (1.0 - f) + g2 as f32 * f) as u8;
        let bl = (b1 as f32 * (1.0 - f) + b2 as f32 * f) as u8;
        Some(Color::Rgb(r, g, bl))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// StyleProperty, StyleRule, StyleSheet — CSS-like style system
// ---------------------------------------------------------------------------

/// A single style property value.
#[derive(Debug, Clone, PartialEq)]
pub enum StyleProperty {
    ColorValue(Color),
    ModifierValue(Modifier),
    StringValue(String),
    NumberValue(f64),
}

/// A style rule binding a selector to a set of properties.
#[derive(Debug, Clone)]
pub struct StyleRule {
    pub selector: String,
    pub properties: HashMap<String, StyleProperty>,
}

impl StyleRule {
    pub fn new(selector: impl Into<String>) -> Self {
        Self {
            selector: selector.into(),
            properties: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, value: StyleProperty) {
        self.properties.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&StyleProperty> {
        self.properties.get(key)
    }

    pub fn has(&self, key: &str) -> bool {
        self.properties.contains_key(key)
    }

    pub fn property_count(&self) -> usize {
        self.properties.len()
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.properties.remove(key).is_some()
    }
}

/// A collection of style rules, keyed by selector.
#[derive(Debug, Clone)]
pub struct StyleSheet {
    pub rules: Vec<StyleRule>,
}

impl StyleSheet {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: StyleRule) {
        self.rules.push(rule);
    }

    pub fn find_rule(&self, selector: &str) -> Option<&StyleRule> {
        self.rules.iter().find(|r| r.selector == selector)
    }

    pub fn find_rule_mut(&mut self, selector: &str) -> Option<&mut StyleRule> {
        self.rules.iter_mut().find(|r| r.selector == selector)
    }

    pub fn remove_rule(&mut self, selector: &str) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.selector != selector);
        self.rules.len() < before
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn selectors(&self) -> Vec<&str> {
        self.rules.iter().map(|r| r.selector.as_str()).collect()
    }

    /// Merge another stylesheet into this one. Rules with matching selectors
    /// have their properties overwritten; new selectors are appended.
    pub fn merge(&mut self, other: &StyleSheet) {
        for other_rule in &other.rules {
            if let Some(existing) = self.find_rule_mut(&other_rule.selector) {
                for (k, v) in &other_rule.properties {
                    existing.set(k.clone(), v.clone());
                }
            } else {
                self.rules.push(other_rule.clone());
            }
        }
    }
}

impl Default for StyleSheet {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve styles by cascading multiple sheets: later sheets override earlier
/// ones for the same selector. Returns the merged properties.
pub fn style_cascade(sheets: &[&StyleSheet], selector: &str) -> HashMap<String, StyleProperty> {
    let mut merged = HashMap::new();
    for sheet in sheets {
        if let Some(rule) = sheet.find_rule(selector) {
            for (k, v) in &rule.properties {
                merged.insert(k.clone(), v.clone());
            }
        }
    }
    merged
}

/// Compare two style rules and return the differences.
/// Each entry is (property_name, old_value, new_value). Only properties that
/// differ between the two rules are included.
pub fn style_diff(
    old: &StyleRule,
    new: &StyleRule,
) -> Vec<(String, Option<StyleProperty>, Option<StyleProperty>)> {
    let mut result = Vec::new();
    let mut all_keys: Vec<&String> = old.properties.keys().collect();
    for k in new.properties.keys() {
        if !all_keys.contains(&k) {
            all_keys.push(k);
        }
    }
    all_keys.sort();
    for key in all_keys {
        let old_val = old.properties.get(key);
        let new_val = new.properties.get(key);
        if old_val != new_val {
            result.push((key.clone(), old_val.cloned(), new_val.cloned()));
        }
    }
    result
}

/// Accumulated statistics for styles operations.
#[derive(Debug, Clone, PartialEq)]
pub struct StylesStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl StylesStats {
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
    pub fn merge(&mut self, other: &StylesStats) {
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

impl Default for StylesStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StylesStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StylesStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for styles.
#[derive(Debug, Clone)]
pub struct StylesValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl StylesValidator {
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

impl Default for StylesValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// StyleInheritance — style inheritance chain resolution
// ---------------------------------------------------------------------------

/// Resolves styles through an inheritance chain (child extends parent).
#[derive(Debug, Clone, Default)]
pub struct StyleInheritance {
    /// Maps selector -> parent selector.
    parent_map: HashMap<String, String>,
}

impl StyleInheritance {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register that `child` inherits from `parent`.
    pub fn set_parent(&mut self, child: impl Into<String>, parent: impl Into<String>) {
        self.parent_map.insert(child.into(), parent.into());
    }

    /// Get the parent of a selector.
    pub fn parent_of(&self, selector: &str) -> Option<&str> {
        self.parent_map.get(selector).map(|s| s.as_str())
    }

    /// Walk the full inheritance chain, from the selector up to the root.
    /// Returns an empty vec if the selector has no parent.
    pub fn chain(&self, selector: &str) -> Vec<&str> {
        let mut chain = Vec::new();
        let mut current = selector;
        while let Some(parent) = self.parent_map.get(current) {
            // Guard against cycles
            if chain.contains(&parent.as_str()) {
                break;
            }
            chain.push(parent.as_str());
            current = parent;
        }
        chain
    }

    /// Resolve all properties for a selector by cascading from root ancestor to child.
    pub fn resolve(&self, sheet: &StyleSheet, selector: &str) -> HashMap<String, StyleProperty> {
        let mut chain = self.chain(selector);
        chain.reverse(); // root first
        chain.push(selector); // self last
        let mut merged = HashMap::new();
        for sel in chain {
            if let Some(rule) = sheet.find_rule(sel) {
                for (k, v) in &rule.properties {
                    merged.insert(k.clone(), v.clone());
                }
            }
        }
        merged
    }

    /// Depth of the inheritance chain for a selector (0 = no parent).
    pub fn depth(&self, selector: &str) -> usize {
        self.chain(selector).len()
    }
}

// ---------------------------------------------------------------------------
// ContrastChecker — WCAG-like contrast checking for accessibility
// ---------------------------------------------------------------------------

/// Checks color contrast ratios for accessibility.
pub struct ContrastChecker;

impl ContrastChecker {
    /// Relative luminance of an RGB color per WCAG 2.0.
    pub fn luminance(r: u8, g: u8, b: u8) -> f64 {
        let rs = Self::srgb_component(r);
        let gs = Self::srgb_component(g);
        let bs = Self::srgb_component(b);
        0.2126 * rs + 0.7152 * gs + 0.0722 * bs
    }

    fn srgb_component(c: u8) -> f64 {
        let s = c as f64 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }

    /// Contrast ratio between two luminance values.
    pub fn contrast_ratio(l1: f64, l2: f64) -> f64 {
        let lighter = l1.max(l2);
        let darker = l1.min(l2);
        (lighter + 0.05) / (darker + 0.05)
    }

    /// Contrast ratio between two RGB colors.
    pub fn color_contrast(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> f64 {
        let l1 = Self::luminance(r1, g1, b1);
        let l2 = Self::luminance(r2, g2, b2);
        Self::contrast_ratio(l1, l2)
    }

    /// Check if a contrast ratio meets WCAG AA for normal text (>= 4.5).
    pub fn meets_aa(ratio: f64) -> bool {
        ratio >= 4.5
    }

    /// Check if a contrast ratio meets WCAG AAA for normal text (>= 7.0).
    pub fn meets_aaa(ratio: f64) -> bool {
        ratio >= 7.0
    }

    /// Check if a contrast ratio meets WCAG AA for large text (>= 3.0).
    pub fn meets_aa_large(ratio: f64) -> bool {
        ratio >= 3.0
    }
}

// ---------------------------------------------------------------------------
// StyleTransition — interpolation between two styles
// ---------------------------------------------------------------------------

/// Represents a transition between two color values with a duration.
#[derive(Debug, Clone)]
pub struct StyleTransition {
    pub property: String,
    pub from: Color,
    pub to: Color,
    pub duration_ms: u64,
    pub elapsed_ms: u64,
}

impl StyleTransition {
    pub fn new(property: impl Into<String>, from: Color, to: Color, duration_ms: u64) -> Self {
        Self {
            property: property.into(),
            from,
            to,
            duration_ms,
            elapsed_ms: 0,
        }
    }

    /// Advance elapsed time by delta.
    pub fn tick(&mut self, delta_ms: u64) {
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms).min(self.duration_ms);
    }

    /// Progress ratio in [0.0, 1.0].
    pub fn progress(&self) -> f32 {
        if self.duration_ms == 0 {
            return 1.0;
        }
        (self.elapsed_ms as f32 / self.duration_ms as f32).min(1.0)
    }

    pub fn is_complete(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }

    /// Interpolated color at the current progress.
    pub fn current_color(&self) -> Option<Color> {
        blend_colors(self.from, self.to, self.progress())
    }
}

/// Manages multiple simultaneous style transitions.
#[derive(Debug, Clone, Default)]
pub struct TransitionManager {
    transitions: Vec<StyleTransition>,
}

impl TransitionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, transition: StyleTransition) {
        self.transitions.push(transition);
    }

    /// Tick all transitions and remove completed ones. Returns count removed.
    pub fn tick(&mut self, delta_ms: u64) -> usize {
        for t in &mut self.transitions {
            t.tick(delta_ms);
        }
        let before = self.transitions.len();
        self.transitions.retain(|t| !t.is_complete());
        before - self.transitions.len()
    }

    pub fn active_count(&self) -> usize {
        self.transitions.len()
    }

    pub fn is_idle(&self) -> bool {
        self.transitions.is_empty()
    }

    /// Get the current interpolated color for a property, if a transition exists.
    pub fn current_color(&self, property: &str) -> Option<Color> {
        self.transitions
            .iter()
            .find(|t| t.property == property)
            .and_then(|t| t.current_color())
    }
}

// ---------------------------------------------------------------------------
// StyleSheet — conditional (media-query-like) styles
// ---------------------------------------------------------------------------

/// A condition for applying a style rule.
#[derive(Debug, Clone, PartialEq)]
pub enum StyleCondition {
    MinWidth(u16),
    MaxWidth(u16),
    DarkTheme,
    LightTheme,
}

impl StyleCondition {
    /// Evaluate the condition against current context.
    pub fn matches(&self, width: u16, is_dark: bool) -> bool {
        match self {
            StyleCondition::MinWidth(w) => width >= *w,
            StyleCondition::MaxWidth(w) => width <= *w,
            StyleCondition::DarkTheme => is_dark,
            StyleCondition::LightTheme => !is_dark,
        }
    }
}

/// A style rule with an optional condition.
#[derive(Debug, Clone)]
pub struct ConditionalStyleRule {
    pub rule: StyleRule,
    pub condition: Option<StyleCondition>,
}

impl ConditionalStyleRule {
    pub fn new(rule: StyleRule) -> Self {
        Self { rule, condition: None }
    }

    pub fn with_condition(mut self, condition: StyleCondition) -> Self {
        self.condition = Some(condition);
        self
    }

    pub fn matches_context(&self, width: u16, is_dark: bool) -> bool {
        self.condition.as_ref().map_or(true, |c| c.matches(width, is_dark))
    }
}

impl StyleSheet {
    /// Resolve properties for a selector, filtered by conditional rules.
    pub fn resolve_conditional(
        &self,
        selector: &str,
        conditionals: &[ConditionalStyleRule],
        width: u16,
        is_dark: bool,
    ) -> HashMap<String, StyleProperty> {
        let mut merged = HashMap::new();
        // Base rules first
        if let Some(rule) = self.find_rule(selector) {
            for (k, v) in &rule.properties {
                merged.insert(k.clone(), v.clone());
            }
        }
        // Conditional overrides
        for cond in conditionals {
            if cond.rule.selector == selector && cond.matches_context(width, is_dark) {
                for (k, v) in &cond.rule.properties {
                    merged.insert(k.clone(), v.clone());
                }
            }
        }
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_color_creation() {
        let tc = ThemeColor::new("editor.background");
        assert_eq!(tc.0, "editor.background");
    }

    #[test]
    fn theme_color_equality() {
        let a = ThemeColor::new("editor.foreground");
        let b = ThemeColor::new("editor.foreground");
        assert_eq!(a, b);
    }

    #[test]
    fn statusbar_style_has_colored_background() {
        let style = statusbar_style();
        // statusbar background is Rgb(0,122,204), not the default
        assert_ne!(style, Style::default());
    }

    #[test]
    fn sidebar_style_has_colored_foreground() {
        let style = sidebar_style();
        assert_ne!(style, Style::default());
    }

    #[test]
    fn selection_style_has_colored_background() {
        let style = selection_style();
        assert_ne!(style, Style::default());
    }

    #[test]
    fn line_number_style_is_not_default() {
        let style = line_number_style();
        assert_ne!(style, Style::default());
    }

    #[test]
    fn theme_color_display() {
        let tc = ThemeColor::new("editor.background");
        assert_eq!(format!("{tc}"), "editor.background");
    }

    #[test]
    fn theme_color_from_str() {
        let tc = ThemeColor::from_str("editor.foreground");
        assert_eq!(tc.0, "editor.foreground");
    }

    #[test]
    fn error_style_is_red() {
        let style = error_style();
        assert_eq!(style, Style::default().fg(Color::Red));
    }

    #[test]
    fn warning_style_is_yellow() {
        let style = warning_style();
        assert_eq!(style, Style::default().fg(Color::Yellow));
    }

    #[test]
    fn info_style_is_blue() {
        let style = info_style();
        assert_eq!(style, Style::default().fg(Color::Blue));
    }

    #[test]
    fn panel_style_has_background() {
        let style = panel_style();
        assert_ne!(style, Style::default());
    }

    #[test]
    fn tab_active_vs_inactive_differ() {
        assert_ne!(tab_active_style(), tab_inactive_style());
    }

    #[test]
    fn cursor_style_has_colors() {
        let style = cursor_style();
        assert_ne!(style, Style::default());
    }

    #[test]
    fn border_style_uses_border_color() {
        let style = border_style();
        assert_eq!(style, Style::default().fg(ThemeDefaults::border()));
    }

    #[test]
    fn accent_style_uses_accent_color() {
        let style = accent_style();
        assert_eq!(style, Style::default().fg(ThemeDefaults::accent()));
    }

    #[test]
    fn resolver_register_and_resolve() {
        let mut resolver = ThemeColorResolver::new();
        let tc = ThemeColor::new("test.color");
        let style = Style::default().fg(Color::Red);
        resolver.register(tc.clone(), style);
        assert_eq!(resolver.resolve(&tc), style);
        assert_eq!(resolver.len(), 1);
    }

    #[test]
    fn resolver_missing_returns_default() {
        let resolver = ThemeColorResolver::new();
        let tc = ThemeColor::new("missing");
        assert_eq!(resolver.resolve(&tc), Style::default());
        assert!(resolver.is_empty());
    }

    #[test]
    fn style_builder_fg_bg_modifier() {
        let style = StyleBuilder::new()
            .fg(Color::Red)
            .bg(Color::Blue)
            .modifier(Modifier::BOLD)
            .build();
        let expected = Style::default()
            .fg(Color::Red)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD);
        assert_eq!(style, expected);
    }

    #[test]
    fn style_builder_default() {
        let style = StyleBuilder::default().build();
        assert_eq!(style, Style::default());
    }

    #[test]
    fn resolver_register_many() {
        let mut resolver = ThemeColorResolver::new();
        let mappings = vec![
            (ThemeColor::new("a"), Style::default().fg(Color::Red)),
            (ThemeColor::new("b"), Style::default().fg(Color::Blue)),
        ];
        resolver.register_many(mappings);
        assert_eq!(resolver.len(), 2);
        assert!(resolver.contains(&ThemeColor::new("a")));
        assert!(resolver.contains(&ThemeColor::new("b")));
    }

    #[test]
    fn resolver_unregister() {
        let mut resolver = ThemeColorResolver::new();
        let tc = ThemeColor::new("test");
        let s = Style::default().fg(Color::Green);
        resolver.register(tc.clone(), s);
        let removed = resolver.unregister(&tc);
        assert_eq!(removed, Some(s));
        assert!(!resolver.contains(&tc));
    }

    #[test]
    fn resolver_tokens() {
        let mut resolver = ThemeColorResolver::new();
        resolver.register(ThemeColor::new("x"), Style::default());
        resolver.register(ThemeColor::new("y"), Style::default());
        let tokens = resolver.tokens();
        assert_eq!(tokens.len(), 2);
    }

    #[test]
    fn resolver_clear() {
        let mut resolver = ThemeColorResolver::new();
        resolver.register(ThemeColor::new("x"), Style::default());
        resolver.clear();
        assert!(resolver.is_empty());
    }

    #[test]
    fn color_palette_set_and_get() {
        let mut palette = ColorPalette::new();
        palette.set("primary", Color::Cyan);
        assert_eq!(palette.get("primary"), Some(Color::Cyan));
        assert!(palette.get("missing").is_none());
    }

    #[test]
    fn color_palette_dark_default() {
        let p = ColorPalette::dark_default();
        assert!(p.len() >= 7);
        assert!(p.get("background").is_some());
        assert!(p.get("error").is_some());
    }

    #[test]
    fn color_palette_light_default() {
        let p = ColorPalette::light_default();
        assert!(p.len() >= 7);
        assert_eq!(p.get("background"), Some(Color::Rgb(255, 255, 255)));
    }

    #[test]
    fn color_palette_names() {
        let mut p = ColorPalette::new();
        p.set("a", Color::Red);
        p.set("b", Color::Blue);
        let names = p.names();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn parse_hex_color_valid() {
        assert_eq!(parse_hex_color("#ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_hex_color("00ff00"), Some(Color::Rgb(0, 255, 0)));
        assert_eq!(parse_hex_color("#0000ff"), Some(Color::Rgb(0, 0, 255)));
    }

    #[test]
    fn parse_hex_color_invalid() {
        assert!(parse_hex_color("").is_none());
        assert!(parse_hex_color("#fff").is_none());
        assert!(parse_hex_color("#gggggg").is_none());
    }

    #[test]
    fn blend_colors_midpoint() {
        let a = Color::Rgb(0, 0, 0);
        let b = Color::Rgb(200, 100, 50);
        let blended = blend_colors(a, b, 0.5).unwrap();
        if let Color::Rgb(r, g, bl) = blended {
            assert_eq!(r, 100);
            assert_eq!(g, 50);
            assert_eq!(bl, 25);
        } else {
            panic!("Expected Rgb");
        }
    }

    #[test]
    fn blend_colors_extremes() {
        let a = Color::Rgb(255, 0, 0);
        let b = Color::Rgb(0, 255, 0);
        assert_eq!(blend_colors(a, b, 0.0), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(blend_colors(a, b, 1.0), Some(Color::Rgb(0, 255, 0)));
    }

    #[test]
    fn blend_colors_non_rgb() {
        assert!(blend_colors(Color::Red, Color::Rgb(0, 0, 0), 0.5).is_none());
    }

    #[test]
    fn test_style_property_eq() {
        assert_eq!(
            StyleProperty::ColorValue(Color::Red),
            StyleProperty::ColorValue(Color::Red)
        );
        assert_ne!(
            StyleProperty::ColorValue(Color::Red),
            StyleProperty::ColorValue(Color::Blue)
        );
        assert_eq!(
            StyleProperty::NumberValue(1.5),
            StyleProperty::NumberValue(1.5)
        );
        assert_eq!(
            StyleProperty::StringValue("a".into()),
            StyleProperty::StringValue("a".into())
        );
        assert_eq!(
            StyleProperty::ModifierValue(Modifier::BOLD),
            StyleProperty::ModifierValue(Modifier::BOLD)
        );
    }

    #[test]
    fn test_style_rule_set_get() {
        let mut rule = StyleRule::new("editor.background");
        rule.set("color", StyleProperty::ColorValue(Color::Red));
        assert!(rule.has("color"));
        assert_eq!(
            rule.get("color"),
            Some(&StyleProperty::ColorValue(Color::Red))
        );
        assert_eq!(rule.property_count(), 1);
        assert!(!rule.has("missing"));
        assert_eq!(rule.get("missing"), None);
    }

    #[test]
    fn test_style_rule_remove() {
        let mut rule = StyleRule::new("test");
        rule.set("a", StyleProperty::NumberValue(1.0));
        assert!(rule.remove("a"));
        assert!(!rule.has("a"));
        assert!(!rule.remove("a"));
        assert_eq!(rule.property_count(), 0);
    }

    #[test]
    fn test_stylesheet_add_find() {
        let mut sheet = StyleSheet::new();
        let mut rule = StyleRule::new("editor.bg");
        rule.set("color", StyleProperty::ColorValue(Color::Black));
        sheet.add_rule(rule);
        assert_eq!(sheet.rule_count(), 1);
        assert!(sheet.find_rule("editor.bg").is_some());
        assert!(sheet.find_rule("missing").is_none());
    }

    #[test]
    fn test_stylesheet_remove_rule() {
        let mut sheet = StyleSheet::new();
        sheet.add_rule(StyleRule::new("a"));
        sheet.add_rule(StyleRule::new("b"));
        assert!(sheet.remove_rule("a"));
        assert_eq!(sheet.rule_count(), 1);
        assert!(!sheet.remove_rule("a"));
    }

    #[test]
    fn test_stylesheet_merge() {
        let mut base = StyleSheet::new();
        let mut r1 = StyleRule::new("editor.bg");
        r1.set("color", StyleProperty::ColorValue(Color::Black));
        r1.set("opacity", StyleProperty::NumberValue(1.0));
        base.add_rule(r1);

        let mut overlay = StyleSheet::new();
        let mut r2 = StyleRule::new("editor.bg");
        r2.set("color", StyleProperty::ColorValue(Color::White));
        overlay.add_rule(r2);
        overlay.add_rule(StyleRule::new("sidebar.fg"));

        base.merge(&overlay);
        assert_eq!(base.rule_count(), 2);
        let merged = base.find_rule("editor.bg").unwrap();
        assert_eq!(
            merged.get("color"),
            Some(&StyleProperty::ColorValue(Color::White))
        );
        assert_eq!(
            merged.get("opacity"),
            Some(&StyleProperty::NumberValue(1.0))
        );
    }

    #[test]
    fn test_stylesheet_selectors() {
        let mut sheet = StyleSheet::new();
        sheet.add_rule(StyleRule::new("a"));
        sheet.add_rule(StyleRule::new("b"));
        let mut sels = sheet.selectors();
        sels.sort();
        assert_eq!(sels, vec!["a", "b"]);
    }

    #[test]
    fn test_style_cascade_single_sheet() {
        let mut sheet = StyleSheet::new();
        let mut rule = StyleRule::new("editor.bg");
        rule.set("color", StyleProperty::ColorValue(Color::Red));
        sheet.add_rule(rule);

        let result = style_cascade(&[&sheet], "editor.bg");
        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("color"),
            Some(&StyleProperty::ColorValue(Color::Red))
        );
    }

    #[test]
    fn test_style_cascade_override() {
        let mut s1 = StyleSheet::new();
        let mut r1 = StyleRule::new("bg");
        r1.set("color", StyleProperty::ColorValue(Color::Red));
        r1.set("size", StyleProperty::NumberValue(10.0));
        s1.add_rule(r1);

        let mut s2 = StyleSheet::new();
        let mut r2 = StyleRule::new("bg");
        r2.set("color", StyleProperty::ColorValue(Color::Blue));
        s2.add_rule(r2);

        let result = style_cascade(&[&s1, &s2], "bg");
        assert_eq!(
            result.get("color"),
            Some(&StyleProperty::ColorValue(Color::Blue))
        );
        assert_eq!(
            result.get("size"),
            Some(&StyleProperty::NumberValue(10.0))
        );
    }

    #[test]
    fn test_style_cascade_missing_selector() {
        let sheet = StyleSheet::new();
        let result = style_cascade(&[&sheet], "nonexistent");
        assert!(result.is_empty());
    }

    #[test]
    fn test_style_diff_same_rule() {
        let mut a = StyleRule::new("x");
        a.set("color", StyleProperty::ColorValue(Color::Red));
        let b = a.clone();
        let diff = style_diff(&a, &b);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_style_diff_changed_properties() {
        let mut old = StyleRule::new("x");
        old.set("color", StyleProperty::ColorValue(Color::Red));
        let mut new = StyleRule::new("x");
        new.set("color", StyleProperty::ColorValue(Color::Blue));
        let diff = style_diff(&old, &new);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].0, "color");
        assert_eq!(diff[0].1, Some(StyleProperty::ColorValue(Color::Red)));
        assert_eq!(diff[0].2, Some(StyleProperty::ColorValue(Color::Blue)));
    }

    #[test]
    fn test_style_diff_added_removed_props() {
        let mut old = StyleRule::new("x");
        old.set("a", StyleProperty::NumberValue(1.0));
        let mut new = StyleRule::new("x");
        new.set("b", StyleProperty::StringValue("hello".into()));
        let diff = style_diff(&old, &new);
        assert_eq!(diff.len(), 2);
        let a_entry = diff.iter().find(|d| d.0 == "a").unwrap();
        assert_eq!(a_entry.1, Some(StyleProperty::NumberValue(1.0)));
        assert_eq!(a_entry.2, None);
        let b_entry = diff.iter().find(|d| d.0 == "b").unwrap();
        assert_eq!(b_entry.1, None);
        assert_eq!(b_entry.2, Some(StyleProperty::StringValue("hello".into())));
    }

    #[test]
    fn styles_stats_new_defaults() {
        let stats = StylesStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn styles_stats_record_success() {
        let mut stats = StylesStats::new();
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
    fn styles_stats_record_failure() {
        let mut stats = StylesStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn styles_stats_reset() {
        let mut stats = StylesStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn styles_stats_merge() {
        let mut a = StylesStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = StylesStats::new();
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
    fn styles_stats_display() {
        let mut stats = StylesStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn styles_stats_default() {
        let stats = StylesStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn styles_validator_accepts_valid_name() {
        let v = StylesValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn styles_validator_rejects_empty() {
        let v = StylesValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn styles_validator_rejects_too_long() {
        let v = StylesValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn styles_validator_forbidden_prefix() {
        let v = StylesValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn styles_validator_allowed_chars() {
        let v = StylesValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn styles_validator_range() {
        let v = StylesValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn styles_sanitize_removes_control() {
        let result = StylesValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn styles_truncate_short_string() {
        assert_eq!(StylesValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn styles_truncate_long_string() {
        let result = StylesValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn styles_is_ascii_printable() {
        assert!(StylesValidator::is_ascii_printable("Hello World 123"));
        assert!(!StylesValidator::is_ascii_printable("Hello\x00World"));
    }

    // --- New tests ---

    #[test]
    fn style_inheritance_chain_resolution() {
        let mut inh = StyleInheritance::new();
        inh.set_parent("button.primary", "button");
        inh.set_parent("button", "base");

        assert_eq!(inh.parent_of("button.primary"), Some("button"));
        assert_eq!(inh.depth("button.primary"), 2);
        let chain = inh.chain("button.primary");
        assert_eq!(chain, vec!["button", "base"]);
    }

    #[test]
    fn style_inheritance_resolve_cascade() {
        let mut sheet = StyleSheet::new();
        let mut base = StyleRule::new("base");
        base.set("color", StyleProperty::StringValue("white".into()));
        base.set("size", StyleProperty::NumberValue(12.0));
        sheet.add_rule(base);

        let mut button = StyleRule::new("button");
        button.set("color", StyleProperty::StringValue("blue".into()));
        sheet.add_rule(button);

        let mut inh = StyleInheritance::new();
        inh.set_parent("button", "base");

        let resolved = inh.resolve(&sheet, "button");
        assert_eq!(resolved.get("color"), Some(&StyleProperty::StringValue("blue".into())));
        assert_eq!(resolved.get("size"), Some(&StyleProperty::NumberValue(12.0)));
    }

    #[test]
    fn contrast_checker_black_white() {
        let ratio = ContrastChecker::color_contrast(0, 0, 0, 255, 255, 255);
        assert!(ratio > 20.0);
        assert!(ContrastChecker::meets_aa(ratio));
        assert!(ContrastChecker::meets_aaa(ratio));
    }

    #[test]
    fn contrast_checker_similar_colors_fail() {
        let ratio = ContrastChecker::color_contrast(200, 200, 200, 210, 210, 210);
        assert!(!ContrastChecker::meets_aa(ratio));
        assert!(!ContrastChecker::meets_aaa(ratio));
    }

    #[test]
    fn contrast_checker_large_text() {
        let ratio = ContrastChecker::color_contrast(100, 100, 100, 200, 200, 200);
        assert!(ContrastChecker::meets_aa_large(ratio));
    }

    #[test]
    fn style_transition_interpolation() {
        let mut t = StyleTransition::new(
            "bg",
            Color::Rgb(0, 0, 0),
            Color::Rgb(255, 255, 255),
            100,
        );
        assert_eq!(t.progress(), 0.0);
        t.tick(50);
        assert!((t.progress() - 0.5).abs() < 0.01);
        let mid = t.current_color().unwrap();
        if let Color::Rgb(r, _, _) = mid {
            assert!(r > 100 && r < 200);
        } else {
            panic!("Expected Rgb");
        }
        t.tick(50);
        assert!(t.is_complete());
    }

    #[test]
    fn transition_manager_tick_and_remove() {
        let mut mgr = TransitionManager::new();
        mgr.add(StyleTransition::new("a", Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255), 50));
        mgr.add(StyleTransition::new("b", Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255), 100));
        assert_eq!(mgr.active_count(), 2);

        let removed = mgr.tick(50);
        assert_eq!(removed, 1);
        assert_eq!(mgr.active_count(), 1);

        assert!(mgr.current_color("b").is_some());
        assert!(mgr.current_color("a").is_none()); // already removed
    }

    #[test]
    fn conditional_style_rule_matching() {
        let cond_dark = StyleCondition::DarkTheme;
        assert!(cond_dark.matches(80, true));
        assert!(!cond_dark.matches(80, false));

        let cond_min = StyleCondition::MinWidth(100);
        assert!(cond_min.matches(120, true));
        assert!(!cond_min.matches(80, true));
    }

    #[test]
    fn stylesheet_resolve_conditional() {
        let mut sheet = StyleSheet::new();
        let mut rule = StyleRule::new("panel");
        rule.set("bg", StyleProperty::StringValue("dark".into()));
        sheet.add_rule(rule);

        let mut light_rule = StyleRule::new("panel");
        light_rule.set("bg", StyleProperty::StringValue("white".into()));
        let conditionals = vec![
            ConditionalStyleRule::new(light_rule).with_condition(StyleCondition::LightTheme),
        ];

        let dark_result = sheet.resolve_conditional("panel", &conditionals, 80, true);
        assert_eq!(dark_result.get("bg"), Some(&StyleProperty::StringValue("dark".into())));

        let light_result = sheet.resolve_conditional("panel", &conditionals, 80, false);
        assert_eq!(light_result.get("bg"), Some(&StyleProperty::StringValue("white".into())));
    }

    #[test]
    fn style_inheritance_cycle_guard() {
        let mut inh = StyleInheritance::new();
        inh.set_parent("a", "b");
        inh.set_parent("b", "a");
        let chain = inh.chain("a");
        // Should not infinite loop, chain should stop
        assert!(chain.len() <= 2);
    }
}
