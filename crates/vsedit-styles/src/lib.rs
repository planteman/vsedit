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

// ---------------------------------------------------------------------------
// Color manipulation utilities
// ---------------------------------------------------------------------------

/// Extract RGB components from a `Color::Rgb`. Returns `None` for non-RGB variants.
pub fn color_to_rgb(color: Color) -> Option<(u8, u8, u8)> {
    if let Color::Rgb(r, g, b) = color {
        Some((r, g, b))
    } else {
        None
    }
}

/// Convert an RGB `Color` to a hex string like `"#rrggbb"`.
/// Returns `None` for non-RGB colors.
pub fn color_to_hex(color: Color) -> Option<String> {
    color_to_rgb(color).map(|(r, g, b)| format!("#{:02x}{:02x}{:02x}", r, g, b))
}

/// Lighten an RGB color by a factor in `[0.0, 1.0]`.
/// A factor of `0.0` returns the original color; `1.0` returns white.
pub fn lighten(color: Color, factor: f32) -> Option<Color> {
    let (r, g, b) = color_to_rgb(color)?;
    let f = factor.clamp(0.0, 1.0);
    let lr = (r as f32 + (255.0 - r as f32) * f) as u8;
    let lg = (g as f32 + (255.0 - g as f32) * f) as u8;
    let lb = (b as f32 + (255.0 - b as f32) * f) as u8;
    Some(Color::Rgb(lr, lg, lb))
}

/// Darken an RGB color by a factor in `[0.0, 1.0]`.
/// A factor of `0.0` returns the original color; `1.0` returns black.
pub fn darken(color: Color, factor: f32) -> Option<Color> {
    let (r, g, b) = color_to_rgb(color)?;
    let f = factor.clamp(0.0, 1.0);
    let dr = (r as f32 * (1.0 - f)) as u8;
    let dg = (g as f32 * (1.0 - f)) as u8;
    let db = (b as f32 * (1.0 - f)) as u8;
    Some(Color::Rgb(dr, dg, db))
}

/// Invert an RGB color.
pub fn invert_color(color: Color) -> Option<Color> {
    let (r, g, b) = color_to_rgb(color)?;
    Some(Color::Rgb(255 - r, 255 - g, 255 - b))
}

/// Compute a grayscale version of an RGB color using luminance weights.
pub fn grayscale(color: Color) -> Option<Color> {
    let (r, g, b) = color_to_rgb(color)?;
    let gray = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) as u8;
    Some(Color::Rgb(gray, gray, gray))
}

// ---------------------------------------------------------------------------
// ThemeScope — semantic scope classification
// ---------------------------------------------------------------------------

/// Semantic scope for theme color tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeScope {
    Editor,
    Sidebar,
    StatusBar,
    Panel,
    Tab,
    Dialog,
    Notification,
    Menu,
}

impl ThemeScope {
    /// Return the prefix string used in color token IDs for this scope.
    pub fn prefix(&self) -> &'static str {
        match self {
            ThemeScope::Editor => "editor",
            ThemeScope::Sidebar => "sidebar",
            ThemeScope::StatusBar => "statusBar",
            ThemeScope::Panel => "panel",
            ThemeScope::Tab => "tab",
            ThemeScope::Dialog => "dialog",
            ThemeScope::Notification => "notification",
            ThemeScope::Menu => "menu",
        }
    }

    /// Build a `ThemeColor` with this scope's prefix.
    pub fn color(&self, suffix: &str) -> ThemeColor {
        ThemeColor::new(format!("{}.{}", self.prefix(), suffix))
    }
}

impl ThemeColor {
    /// Return the scope of this color token based on its prefix, if recognized.
    pub fn scope(&self) -> Option<ThemeScope> {
        let id = &self.0;
        if id.starts_with("editor.") {
            Some(ThemeScope::Editor)
        } else if id.starts_with("sidebar.") {
            Some(ThemeScope::Sidebar)
        } else if id.starts_with("statusBar.") {
            Some(ThemeScope::StatusBar)
        } else if id.starts_with("panel.") {
            Some(ThemeScope::Panel)
        } else if id.starts_with("tab.") {
            Some(ThemeScope::Tab)
        } else if id.starts_with("dialog.") {
            Some(ThemeScope::Dialog)
        } else if id.starts_with("notification.") {
            Some(ThemeScope::Notification)
        } else if id.starts_with("menu.") {
            Some(ThemeScope::Menu)
        } else {
            None
        }
    }

    /// Return the suffix after the scope prefix (e.g. `"background"` from `"editor.background"`).
    pub fn suffix(&self) -> Option<&str> {
        self.0.split_once('.').map(|(_, s)| s)
    }
}

// ---------------------------------------------------------------------------
// StyleOverrideStack — layered style overrides
// ---------------------------------------------------------------------------

/// A stack of named style layers. Later (higher) layers override earlier ones.
#[derive(Debug, Clone)]
pub struct StyleOverrideStack {
    layers: Vec<(String, Style)>,
}

impl StyleOverrideStack {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Push a named style layer onto the stack.
    pub fn push(&mut self, name: impl Into<String>, style: Style) {
        self.layers.push((name.into(), style));
    }

    /// Remove the topmost layer matching `name`. Returns `true` if found.
    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(pos) = self.layers.iter().rposition(|l| l.0 == name) {
            self.layers.remove(pos);
            true
        } else {
            false
        }
    }

    /// Flatten all layers into a single `Style` by patching each layer on top.
    pub fn flatten(&self) -> Style {
        let mut result = Style::default();
        for (_, layer) in &self.layers {
            result = result.patch(*layer);
        }
        result
    }

    /// Number of layers currently on the stack.
    pub fn depth(&self) -> usize {
        self.layers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Clear all layers.
    pub fn clear(&mut self) {
        self.layers.clear();
    }
}

impl Default for StyleOverrideStack {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ColorPalette extensions
// ---------------------------------------------------------------------------

impl ColorPalette {
    /// Derive a lighter variant of a named color. Returns `None` if the name
    /// is missing or the color is not RGB.
    pub fn lighter(&self, name: &str, factor: f32) -> Option<Color> {
        self.get(name).and_then(|c| lighten(c, factor))
    }

    /// Derive a darker variant of a named color.
    pub fn darker(&self, name: &str, factor: f32) -> Option<Color> {
        self.get(name).and_then(|c| darken(c, factor))
    }

    /// Check whether two named colors have sufficient contrast for WCAG AA.
    pub fn check_contrast_aa(&self, name_a: &str, name_b: &str) -> Option<bool> {
        let a = self.get(name_a)?;
        let b = self.get(name_b)?;
        let (r1, g1, b1) = color_to_rgb(a)?;
        let (r2, g2, b2) = color_to_rgb(b)?;
        let ratio = ContrastChecker::color_contrast(r1, g1, b1, r2, g2, b2);
        Some(ContrastChecker::meets_aa(ratio))
    }

    /// Merge another palette into this one; existing names are overwritten.
    pub fn merge(&mut self, other: &ColorPalette) {
        for (name, color) in &other.colors {
            self.colors.insert(name.clone(), *color);
        }
    }

    /// Remove a named color. Returns `true` if it existed.
    pub fn remove(&mut self, name: &str) -> bool {
        self.colors.remove(name).is_some()
    }
}


// ---------------------------------------------------------------------------
// StyleCascade -- cascading style resolution
// ---------------------------------------------------------------------------

pub struct StyleCascade {
    layers: Vec<(String, Style)>,
}

impl StyleCascade {
    pub fn new() -> Self { Self { layers: Vec::new() } }

    pub fn push(&mut self, name: impl Into<String>, style: Style) {
        self.layers.push((name.into(), style));
    }

    pub fn resolve(&self) -> Style {
        let mut result = Style::default();
        for (_, style) in &self.layers {
            if style.fg.is_some() { result.fg = style.fg; }
            if style.bg.is_some() { result.bg = style.bg; }
            result = result.patch(*style);
        }
        result
    }

    pub fn len(&self) -> usize { self.layers.len() }
    pub fn is_empty(&self) -> bool { self.layers.is_empty() }
    pub fn clear(&mut self) { self.layers.clear(); }

    pub fn remove_layer(&mut self, name: &str) -> bool {
        if let Some(i) = self.layers.iter().position(|(n, _)| n == name) { self.layers.remove(i); true } else { false }
    }
}

impl Default for StyleCascade { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// StyleMediaQuery -- terminal capability detection
// ---------------------------------------------------------------------------

pub struct StyleMediaQuery {
    pub supports_256_colors: bool,
    pub supports_true_color: bool,
    pub supports_bold: bool,
    pub supports_italic: bool,
    pub terminal_width: u16,
}

impl StyleMediaQuery {
    pub fn basic() -> Self {
        Self { supports_256_colors: false, supports_true_color: false, supports_bold: true, supports_italic: false, terminal_width: 80 }
    }

    pub fn full() -> Self {
        Self { supports_256_colors: true, supports_true_color: true, supports_bold: true, supports_italic: true, terminal_width: 120 }
    }

    pub fn best_color_depth(&self) -> u32 {
        if self.supports_true_color { 24 }
        else if self.supports_256_colors { 8 }
        else { 4 }
    }

    pub fn is_wide(&self) -> bool { self.terminal_width >= 120 }
    pub fn is_narrow(&self) -> bool { self.terminal_width < 80 }
}

impl Default for StyleMediaQuery { fn default() -> Self { Self::basic() } }

impl std::fmt::Display for StyleMediaQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MediaQuery({}bit, {}cols)", self.best_color_depth(), self.terminal_width)
    }
}

// ---------------------------------------------------------------------------
// StyleVariableResolver
// ---------------------------------------------------------------------------

pub struct StyleVariableResolver {
    variables: std::collections::HashMap<String, String>,
}

impl StyleVariableResolver {
    pub fn new() -> Self { Self { variables: std::collections::HashMap::new() } }

    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(name.into(), value.into());
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.variables.get(name).map(|s| s.as_str())
    }

    pub fn resolve_string(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (name, value) in &self.variables {
            let var_ref = format!("${{{}}}", name);
            result = result.replace(&var_ref, value);
        }
        result
    }

    pub fn len(&self) -> usize { self.variables.len() }
    pub fn is_empty(&self) -> bool { self.variables.is_empty() }
    pub fn clear(&mut self) { self.variables.clear(); }
}

impl Default for StyleVariableResolver { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// StyleDiffComparison
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum StyleDiffChange {
    FgChanged,
    BgChanged,
    ModifierChanged,
    NoChange,
}

pub struct StyleDiffComparison;

impl StyleDiffComparison {
    pub fn compare(a: &Style, b: &Style) -> Vec<StyleDiffChange> {
        let mut changes = Vec::new();
        if a.fg != b.fg { changes.push(StyleDiffChange::FgChanged); }
        if a.bg != b.bg { changes.push(StyleDiffChange::BgChanged); }
        if a.add_modifier != b.add_modifier || a.sub_modifier != b.sub_modifier {
            changes.push(StyleDiffChange::ModifierChanged);
        }
        if changes.is_empty() { changes.push(StyleDiffChange::NoChange); }
        changes
    }

    pub fn are_equal(a: &Style, b: &Style) -> bool {
        Self::compare(a, b) == vec![StyleDiffChange::NoChange]
    }
}


// === Style Variable Resolver ===

/// Resolves style variables like `${color.primary}` in style strings.
#[derive(Debug, Clone)]
pub struct StyleVarResolver {
    variables: HashMap<String, String>,
    fallbacks: HashMap<String, String>,
    resolution_cache: HashMap<String, String>,
    max_depth: usize,
}

impl StyleVarResolver {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            fallbacks: HashMap::new(),
            resolution_cache: HashMap::new(),
            max_depth: 10,
        }
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn set_variable(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        self.resolution_cache.remove(&name);
        self.variables.insert(name, value.into());
    }

    pub fn set_fallback(&mut self, name: impl Into<String>, fallback: impl Into<String>) {
        self.fallbacks.insert(name.into(), fallback.into());
    }

    pub fn resolve(&mut self, input: &str) -> String {
        if let Some(cached) = self.resolution_cache.get(input) {
            return cached.clone();
        }
        let result = self.resolve_recursive(input, 0);
        self.resolution_cache.insert(input.to_string(), result.clone());
        result
    }

    fn resolve_recursive(&self, input: &str, depth: usize) -> String {
        if depth >= self.max_depth {
            return input.to_string();
        }
        let mut result = input.to_string();
        let mut start = 0;
        while let Some(var_start) = result[start..].find("${") {
            let abs_start = start + var_start;
            if let Some(var_end) = result[abs_start..].find('}') {
                let abs_end = abs_start + var_end;
                let var_name = &result[abs_start + 2..abs_end];
                let replacement = self.variables.get(var_name)
                    .or_else(|| self.fallbacks.get(var_name))
                    .cloned()
                    .unwrap_or_else(|| format!("${{{}}}", var_name));
                let resolved = self.resolve_recursive(&replacement, depth + 1);
                result = format!("{}{}{}", &result[..abs_start], resolved, &result[abs_end + 1..]);
                start = abs_start + resolved.len();
            } else {
                break;
            }
        }
        result
    }

    pub fn clear_cache(&mut self) {
        self.resolution_cache.clear();
    }

    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    pub fn remove_variable(&mut self, name: &str) -> Option<String> {
        self.resolution_cache.remove(name);
        self.variables.remove(name)
    }

    pub fn all_variable_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.variables.keys().cloned().collect();
        names.sort();
        names
    }
}

impl Default for StyleVarResolver {
    fn default() -> Self {
        Self::new()
    }
}

// === Style Theme Switcher ===

/// A named style theme with key-value style pairs.
#[derive(Debug, Clone)]
pub struct StyleThemeEntry {
    pub name: String,
    pub styles: HashMap<String, Style>,
    pub is_dark: bool,
    pub priority: u32,
}

impl StyleThemeEntry {
    pub fn new(name: impl Into<String>, is_dark: bool) -> Self {
        Self {
            name: name.into(),
            styles: HashMap::new(),
            is_dark,
            priority: 0,
        }
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn set_style(&mut self, key: impl Into<String>, style: Style) {
        self.styles.insert(key.into(), style);
    }

    pub fn get_style(&self, key: &str) -> Option<&Style> {
        self.styles.get(key)
    }

    pub fn style_count(&self) -> usize {
        self.styles.len()
    }

    pub fn merge_from(&mut self, other: &StyleThemeEntry) {
        for (k, v) in &other.styles {
            self.styles.entry(k.clone()).or_insert(*v);
        }
    }
}

/// Manages switching between multiple themes.
#[derive(Debug)]
pub struct StyleThemeSwitcher {
    themes: Vec<StyleThemeEntry>,
    active_index: usize,
    history: Vec<usize>,
    max_history: usize,
}

impl StyleThemeSwitcher {
    pub fn new() -> Self {
        Self {
            themes: Vec::new(),
            active_index: 0,
            history: Vec::new(),
            max_history: 20,
        }
    }

    pub fn add_theme(&mut self, theme: StyleThemeEntry) {
        self.themes.push(theme);
    }

    pub fn switch_to(&mut self, index: usize) -> bool {
        if index < self.themes.len() {
            self.history.push(self.active_index);
            if self.history.len() > self.max_history {
                self.history.remove(0);
            }
            self.active_index = index;
            true
        } else {
            false
        }
    }

    pub fn switch_by_name(&mut self, name: &str) -> bool {
        if let Some(idx) = self.themes.iter().position(|t| t.name == name) {
            self.switch_to(idx)
        } else {
            false
        }
    }

    pub fn active_theme(&self) -> Option<&StyleThemeEntry> {
        self.themes.get(self.active_index)
    }

    pub fn active_theme_name(&self) -> Option<&str> {
        self.active_theme().map(|t| t.name.as_str())
    }

    pub fn switch_to_previous(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            self.active_index = prev;
            true
        } else {
            false
        }
    }

    pub fn theme_count(&self) -> usize {
        self.themes.len()
    }

    pub fn dark_themes(&self) -> Vec<&StyleThemeEntry> {
        self.themes.iter().filter(|t| t.is_dark).collect()
    }

    pub fn light_themes(&self) -> Vec<&StyleThemeEntry> {
        self.themes.iter().filter(|t| !t.is_dark).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&StyleThemeEntry> {
        let mut sorted: Vec<&StyleThemeEntry> = self.themes.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn remove_theme(&mut self, name: &str) -> bool {
        if let Some(idx) = self.themes.iter().position(|t| t.name == name) {
            self.themes.remove(idx);
            if self.active_index >= self.themes.len() && !self.themes.is_empty() {
                self.active_index = self.themes.len() - 1;
            }
            true
        } else {
            false
        }
    }
}

impl Default for StyleThemeSwitcher {
    fn default() -> Self {
        Self::new()
    }
}


// ─── StyleB Builder & Validator ─────────────────────────────

/// Builder for constructing style configurations.
#[derive(Debug, Clone)]
pub struct StyleBBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl StyleBBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<StyleBCfg, StyleBBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(StyleBBuildErr { errors }); }
        Ok(StyleBCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated style configuration.
#[derive(Debug, Clone)]
pub struct StyleBCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl StyleBCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &StyleBCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for StyleBCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StyleBCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct StyleBBuildErr { pub errors: Vec<String> }

impl fmt::Display for StyleBBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StyleBBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for StyleBBuildErr {}

// ─── StyleF Formatter ───────────────────────────────────────

/// Formatting options for style output.
#[derive(Debug, Clone)]
pub struct StyleFFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for StyleFFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl StyleFFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for style data.
pub struct StyleFFmt {
    options: StyleFFmtOpts,
}

impl StyleFFmt {
    pub fn new(options: StyleFFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: StyleFFmtOpts::default() } }

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


/// Configuration manager for styles functionality.
pub struct StylesConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl StylesConfig {
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

    pub fn merge(&mut self, other: &StylesConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for styles operations.
pub struct StylesRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl StylesRateTracker {
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

/// Validation result collector for styles.
pub struct StylesValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl StylesValidationCollector {
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

    pub fn merge(&mut self, other: &StylesValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Theme and CSS variable management — extended utilities (yt)
// ---------------------------------------------------------------------------

/// Metric accumulator for styles operations.
#[derive(Debug, Clone)]
pub struct YtMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YtMetrics {
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

/// Sliding-window rate counter for styles.
#[derive(Debug, Clone)]
pub struct YtRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YtRateWindow {
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

/// A small LRU-style cache for styles lookups.
#[derive(Debug, Clone)]
pub struct YtLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YtLruCache {
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
// xa_ extended helpers for styles
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaStylesRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaStylesRingBuf {
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
pub struct XaStylesCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaStylesCounter {
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

impl Default for XaStylesCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 169
// ---------------------------------------------------------------------------

/// Generic object pool `Xc169Pool<T>`.
pub struct Xc169Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc169Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc169PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc169Pool<T> {
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
    pub fn stats(&self) -> Xc169PoolStats {
        Xc169PoolStats {
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

impl<T> Default for Xc169Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc169Scheduler`.
pub struct Xc169Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc169Scheduler {
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

impl Default for Xc169Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_169 hash for the given byte slice.
pub fn xc_169_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_169 convention.
pub fn xc_169_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_105 deepening: state machine + event bus ---

/// States for the Xd105 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd105State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd105State {
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
pub struct Xd105Transition {
    pub from: Xd105State,
    pub to: Xd105State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd105StateMachine {
    current: Xd105State,
    history: Vec<Xd105Transition>,
    step_counter: usize,
}

impl Xd105StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd105State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd105State {
        self.current
    }

    pub fn history(&self) -> &[Xd105Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd105State) -> Result<Xd105State, String> {
        let allowed = match (self.current, target) {
            (Xd105State::Idle, Xd105State::Running) => true,
            (Xd105State::Running, Xd105State::Paused) => true,
            (Xd105State::Running, Xd105State::Done) => true,
            (Xd105State::Paused, Xd105State::Running) => true,
            (Xd105State::Paused, Xd105State::Done) => true,
            (Xd105State::Done, Xd105State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_105: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd105Transition {
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
            "Xd105SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd105State> {
        let prefix = "Xd105SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd105State::Idle),
            "Running" => Some(Xd105State::Running),
            "Paused" => Some(Xd105State::Paused),
            "Done" => Some(Xd105State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd105State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd105 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd105Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd105Event {
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

type Xd105HandlerFn = Box<dyn Fn(&Xd105Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd105EventBus {
    handlers: Vec<(usize, Option<String>, Xd105HandlerFn)>,
    next_id: usize,
    published: Vec<Xd105Event>,
}

impl Xd105EventBus {
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
        F: Fn(&Xd105Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd105Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd105Event) {
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

    pub fn published_events(&self) -> &[Xd105Event] {
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
// xg_29: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg29Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg29Graph {
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

impl Default for Xg29Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_29: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg29Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg29Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg29Heap<T>) {
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

impl<T: Ord> Default for Xg29Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 168).
pub struct Xh168SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh168SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 210 as u64,
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

/// A compact bit set supporting boolean operations (variant 168).
pub struct Xh168BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh168BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 168).
pub struct Xi168Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi168Deque<T> {
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
pub struct Xi168Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi168Interval {
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

/// A simple interval tree (variant 168).
pub struct Xi168IntervalTree {
    xi_intervals: Vec<Xi168Interval>,
}

impl Xi168IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi168Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi168Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi168Interval) -> Vec<&Xi168Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi168Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi168Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi168Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi168Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi168Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi168Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 168) ---

/// Disjoint set / union-find for crate 168.
pub struct Xj168UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj168UnionFind {
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

const XJ168_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 168.
pub struct Xj168BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj168BTreeNode<K, V>>>,
    len: usize,
}

struct Xj168BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj168BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj168BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ168_BTREE_ORDER - 1
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
        let mid = XJ168_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj168BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj168BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj168BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj168BTreeNode::xj_new_leaf();
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


// --- xk_168 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk168SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk168SegmentTree {
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
pub struct Xk168DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk168DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_168).
#[derive(Debug, Clone)]
pub struct Xl168Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl168Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_168).
#[derive(Debug, Clone)]
pub struct Xl168SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl168SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm168MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm168MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm168Tokenizer {
    text: String,
}

impl Xm168Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 168.
pub struct Xn168Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn168Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 168 -----

#[derive(Debug, Clone)]
struct Xn168AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn168AvlNode<K, V>>>,
    right: Option<Box<Xn168AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 168.
#[derive(Debug, Clone)]
pub struct Xn168AVL<K, V> {
    root: Option<Box<Xn168AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn168AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn168AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn168AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn168AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn168AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn168AvlNode<K, V>>) -> Box<Xn168AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn168AvlNode<K, V>>) -> Box<Xn168AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn168AvlNode<K, V>>) -> Box<Xn168AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn168AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn168AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn168AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn168AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn168AvlNode<K, V>>) -> &Xn168AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn168AvlNode<K, V>>) -> (Box<Xn168AvlNode<K, V>>, Option<Box<Xn168AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn168AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn168AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn168AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn168AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn168AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn168AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn168AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo168RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo168Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo168RBNode<K, V> {
    key: K,
    value: V,
    color: Xo168Color,
    left: Option<Box<Xo168RBNode<K, V>>>,
    right: Option<Box<Xo168RBNode<K, V>>>,
}

/// A red-black tree map for crate 168.
#[derive(Debug, Clone)]
pub struct Xo168RedBlack<K, V> {
    root: Option<Box<Xo168RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo168RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo168Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo168RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo168RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo168RBNode {
                    key, value, color: Xo168Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo168RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo168Color::Red)
    }

    fn xo_balance(mut h: Box<Xo168RBNode<K, V>>) -> Box<Xo168RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo168Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo168RBNode<K, V>>) -> Box<Xo168RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo168Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo168RBNode<K, V>>) -> Box<Xo168RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo168Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo168RBNode<K, V>>) {
        h.color = Xo168Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo168Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo168Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo168Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo168RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo168RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo168RBNode<K, V>) -> (K, V, Option<Box<Xo168RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo168RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo168Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo168RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo168ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 168.
#[derive(Debug, Clone)]
pub struct Xo168ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo168ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo168#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo168#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 168).
#[derive(Debug)]
pub struct Xp168SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp168Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp168Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp168Node<K, V>>>,
    xp_right: Option<Box<Xp168Node<K, V>>>,
}

impl<K: Ord, V> Xp168Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp168SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp168SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp168Node<K, V>>>, key: &K) -> Option<Box<Xp168Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp168Node<K, V>>) -> Box<Xp168Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp168Node<K, V>>) -> Box<Xp168Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp168Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp168Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp168Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq168Treap ---------------

use std::cmp::Ordering as Xq168Ord;

struct Xq168TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq168TreapNode<K, V>>>,
    right: Option<Box<Xq168TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq168Treap<K, V> {
    root: Option<Box<Xq168TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq168TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_168_size<K, V>(node: &Option<Box<Xq168TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_168_update_size<K, V>(node: &mut Xq168TreapNode<K, V>) {
    node.size = 1 + xq_168_size(&node.left) + xq_168_size(&node.right);
}

fn xq_168_rotate_right<K, V>(mut node: Box<Xq168TreapNode<K, V>>) -> Box<Xq168TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_168_update_size(&mut node);
    left.right = Some(node);
    xq_168_update_size(&mut left);
    left
}

fn xq_168_rotate_left<K, V>(mut node: Box<Xq168TreapNode<K, V>>) -> Box<Xq168TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_168_update_size(&mut node);
    right.left = Some(node);
    xq_168_update_size(&mut right);
    right
}

fn xq_168_insert_node<K: Ord, V>(
    node: Option<Box<Xq168TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq168TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq168TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq168Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq168Ord::Less => {
                let (new_left, old) = xq_168_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_168_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_168_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq168Ord::Greater => {
                let (new_right, old) = xq_168_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_168_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_168_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_168_remove_node<K: Ord, V>(
    node: Option<Box<Xq168TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq168TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq168Ord::Less => {
                let (new_left, old) = xq_168_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_168_update_size(&mut n);
                (Some(n), old)
            }
            Xq168Ord::Greater => {
                let (new_right, old) = xq_168_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_168_update_size(&mut n);
                (Some(n), old)
            }
            Xq168Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_168_rotate_right(n);
                    let (new_right, old) = xq_168_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_168_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_168_rotate_left(n);
                    let (new_left, old) = xq_168_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_168_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_168_find_min<K, V>(node: &Option<Box<Xq168TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_168_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_168_find_max<K, V>(node: &Option<Box<Xq168TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_168_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_168_rank<K: Ord, V>(node: &Option<Box<Xq168TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq168Ord::Less => xq_168_rank(&n.left, key),
            Xq168Ord::Equal => xq_168_size(&n.left),
            Xq168Ord::Greater => 1 + xq_168_size(&n.left) + xq_168_rank(&n.right, key),
        },
    }
}

fn xq_168_kth<K, V>(node: &Option<Box<Xq168TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_168_size(&n.left);
        if k < left_size {
            xq_168_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_168_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_168_in_order<K: Clone, V>(node: &Option<Box<Xq168TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_168_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_168_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq168Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 168 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_168_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq168Ord::Equal => return Some(&n.value),
                Xq168Ord::Less => cur = &n.left,
                Xq168Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_168_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_168_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_168_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_168_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_168_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_168_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_168_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq168VEBTree ---------------

pub struct Xq168VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq168VEBTree>>,
    clusters: Vec<Option<Box<Xq168VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq168VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq168VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq168VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr168KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr168KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr168BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr168KDNode {
    xr_point: Xr168KDPoint,
    xr_left: Option<Box<Xr168KDNode>>,
    xr_right: Option<Box<Xr168KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr168KDTree {
    xr_root: Option<Box<Xr168KDNode>>,
    xr_size: usize,
}

impl Xr168KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr168KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr168KDNode>>,
        point: Xr168KDPoint,
        depth: usize,
    ) -> Box<Xr168KDNode> {
        match node {
            None => Box::new(Xr168KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr168KDPoint) -> Option<Xr168KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr168KDNode>,
        query: &Xr168KDPoint,
        depth: usize,
        best: &mut Xr168KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr168KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr168KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr168KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr168KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr168KDNode>>, pts: &mut Vec<Xr168KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr168KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr168BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr168BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs168PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs168PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs168PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs168PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs168ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs168ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs168ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs168RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs168RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs168RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs168CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs168CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs168CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}

/// Auxiliary statistics tracker for xs_168 data structures.
#[derive(Debug, Clone)]
pub struct Xs168StatsTracker {
    xs_samples: Vec<f64>,
    xs_sorted: bool,
}

impl Xs168StatsTracker {
    /// Create a new stats tracker.
    pub fn xs_new() -> Self {
        Xs168StatsTracker {
            xs_samples: Vec::new(),
            xs_sorted: true,
        }
    }

    /// Add a sample value.
    pub fn xs_add(&mut self, value: f64) {
        self.xs_samples.push(value);
        self.xs_sorted = false;
    }

    /// Return the number of samples.
    pub fn xs_count(&self) -> usize {
        self.xs_samples.len()
    }

    /// Return the mean of all samples.
    pub fn xs_mean(&self) -> f64 {
        if self.xs_samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.xs_samples.iter().sum();
        sum / self.xs_samples.len() as f64
    }

    /// Return the minimum value.
    pub fn xs_min(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::min)
    }

    /// Return the maximum value.
    pub fn xs_max(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::max)
    }

    /// Return the variance of all samples.
    pub fn xs_variance(&self) -> f64 {
        if self.xs_samples.len() < 2 {
            return 0.0;
        }
        let mean = self.xs_mean();
        let sum_sq: f64 = self.xs_samples.iter()
            .map(|x| (x - mean) * (x - mean))
            .sum();
        sum_sq / (self.xs_samples.len() - 1) as f64
    }

    /// Return the standard deviation.
    pub fn xs_std_dev(&self) -> f64 {
        self.xs_variance().sqrt()
    }

    /// Return the median value.
    pub fn xs_median(&mut self) -> Option<f64> {
        if self.xs_samples.is_empty() {
            return None;
        }
        if !self.xs_sorted {
            self.xs_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.xs_sorted = true;
        }
        let mid = self.xs_samples.len() / 2;
        if self.xs_samples.len() % 2 == 0 {
            Some((self.xs_samples[mid - 1] + self.xs_samples[mid]) / 2.0)
        } else {
            Some(self.xs_samples[mid])
        }
    }

    /// Check if the tracker is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_samples.is_empty()
    }

    /// Clear all samples.
    pub fn xs_clear(&mut self) {
        self.xs_samples.clear();
        self.xs_sorted = true;
    }

    /// Return the range (max - min).
    pub fn xs_range(&self) -> f64 {
        match (self.xs_min(), self.xs_max()) {
            (Some(min), Some(max)) => max - min,
            _ => 0.0,
        }
    }

    /// Return the sum of all samples.
    pub fn xs_sum(&self) -> f64 {
        self.xs_samples.iter().sum()
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
    fn styles_validator_accepts_and_rejects() {
        let mut v = StylesValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn styles_validator_warnings() {
        let mut v = StylesValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn styles_validator_clear_and_merge() {
        let mut v = StylesValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = StylesValidationCollector::new();
        a.add_error("a_err");
        let mut b = StylesValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
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

    // -----------------------------------------------------------------------
    // New tests — color utilities, ThemeScope, StyleOverrideStack, palette
    // -----------------------------------------------------------------------

    #[test]
    fn color_to_hex_roundtrip() {
        let color = Color::Rgb(0, 122, 204);
        let hex = color_to_hex(color).unwrap();
        assert_eq!(hex, "#007acc");
        assert_eq!(parse_hex_color(&hex), Some(color));
    }

    #[test]
    fn lighten_and_darken() {
        let base = Color::Rgb(100, 100, 100);
        let lighter = lighten(base, 0.5).unwrap();
        let darker = darken(base, 0.5).unwrap();
        let (lr, _, _) = color_to_rgb(lighter).unwrap();
        let (dr, _, _) = color_to_rgb(darker).unwrap();
        assert!(lr > 100, "lighten should increase component");
        assert!(dr < 100, "darken should decrease component");
        // Extremes
        assert_eq!(lighten(base, 1.0), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(darken(base, 1.0), Some(Color::Rgb(0, 0, 0)));
        // Non-RGB returns None
        assert!(lighten(Color::Red, 0.5).is_none());
    }

    #[test]
    fn invert_and_grayscale() {
        assert_eq!(invert_color(Color::Rgb(0, 0, 0)), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(invert_color(Color::Rgb(255, 255, 255)), Some(Color::Rgb(0, 0, 0)));
        let gray = grayscale(Color::Rgb(255, 0, 0)).unwrap();
        if let Color::Rgb(r, g, b) = gray {
            assert_eq!(r, g);
            assert_eq!(g, b);
            assert!(r > 0 && r < 255);
        } else {
            panic!("expected Rgb");
        }
    }

    #[test]
    fn theme_scope_prefix_and_color() {
        let tc = ThemeScope::Editor.color("background");
        assert_eq!(tc.0, "editor.background");
        assert_eq!(tc.scope(), Some(ThemeScope::Editor));
        assert_eq!(tc.suffix(), Some("background"));

        let tc2 = ThemeScope::StatusBar.color("foreground");
        assert_eq!(tc2.0, "statusBar.foreground");
        assert_eq!(tc2.scope(), Some(ThemeScope::StatusBar));
    }

    #[test]
    fn theme_color_scope_unrecognized() {
        let tc = ThemeColor::new("custom.something");
        assert_eq!(tc.scope(), None);
        assert_eq!(tc.suffix(), Some("something"));
    }

    #[test]
    fn style_override_stack_flatten() {
        let mut stack = StyleOverrideStack::new();
        assert!(stack.is_empty());

        stack.push("base", Style::default().fg(Color::White).bg(Color::Black));
        stack.push("highlight", Style::default().fg(Color::Yellow));

        let flat = stack.flatten();
        let expected = Style::default().fg(Color::Yellow).bg(Color::Black);
        assert_eq!(flat, expected);
        assert_eq!(stack.depth(), 2);

        assert!(stack.remove("highlight"));
        assert_eq!(stack.depth(), 1);
        let flat2 = stack.flatten();
        assert_eq!(flat2, Style::default().fg(Color::White).bg(Color::Black));
    }

    #[test]
    fn color_palette_lighter_darker_contrast() {
        let p = ColorPalette::dark_default();
        let lighter_bg = p.lighter("background", 0.3);
        assert!(lighter_bg.is_some());
        let (r, _, _) = color_to_rgb(lighter_bg.unwrap()).unwrap();
        assert!(r > 30, "lighter background should be brighter");

        // foreground vs background should have good contrast
        let meets = p.check_contrast_aa("foreground", "background");
        assert_eq!(meets, Some(true));
    }

    #[test]
    fn color_palette_merge_and_remove() {
        let mut base = ColorPalette::new();
        base.set("a", Color::Rgb(10, 10, 10));
        base.set("b", Color::Rgb(20, 20, 20));

        let mut overlay = ColorPalette::new();
        overlay.set("b", Color::Rgb(99, 99, 99));
        overlay.set("c", Color::Rgb(30, 30, 30));

        base.merge(&overlay);
        assert_eq!(base.len(), 3);
        assert_eq!(base.get("b"), Some(Color::Rgb(99, 99, 99)));
        assert!(base.get("c").is_some());

        assert!(base.remove("a"));
        assert!(!base.remove("a"));
        assert_eq!(base.len(), 2);
    }


    #[test]
    fn cascade_resolve() {
        let mut cascade = StyleCascade::new();
        cascade.push("base", Style::default().fg(Color::White));
        cascade.push("theme", Style::default().bg(Color::Black));
        let resolved = cascade.resolve();
        assert!(resolved.fg.is_some() || resolved.bg.is_some());
    }

    #[test]
    fn cascade_remove() {
        let mut cascade = StyleCascade::new();
        cascade.push("a", Style::default());
        assert!(cascade.remove_layer("a"));
        assert!(cascade.is_empty());
    }

    #[test]
    fn media_query_basic() {
        let mq = StyleMediaQuery::basic();
        assert_eq!(mq.best_color_depth(), 4);
        assert!(!mq.is_wide());
    }

    #[test]
    fn media_query_full() {
        let mq = StyleMediaQuery::full();
        assert_eq!(mq.best_color_depth(), 24);
        assert!(mq.is_wide());
    }

    #[test]
    fn media_query_display() {
        let mq = StyleMediaQuery::basic();
        assert!(format!("{mq}").contains("4bit"));
    }

    #[test]
    fn variable_resolver_basic() {
        let mut vr = StyleVariableResolver::new();
        vr.set("color", "red");
        assert_eq!(vr.get("color"), Some("red"));
        assert_eq!(vr.resolve_string("fg: ${color}"), "fg: red");
    }

    #[test]
    fn variable_resolver_missing() {
        let vr = StyleVariableResolver::new();
        assert_eq!(vr.resolve_string("${missing}"), "${missing}");
    }

    #[test]
    fn style_diff_same() {
        let s = Style::default();
        assert!(StyleDiffComparison::are_equal(&s, &s));
    }

    #[test]
    fn style_diff_fg_changed() {
        let a = Style::default().fg(Color::Red);
        let b = Style::default().fg(Color::Blue);
        let changes = StyleDiffComparison::compare(&a, &b);
        assert!(changes.contains(&StyleDiffChange::FgChanged));
    }

    #[test]
    fn style_diff_bg_changed() {
        let a = Style::default();
        let b = Style::default().bg(Color::Green);
        assert!(!StyleDiffComparison::are_equal(&a, &b));
    }

    #[test]
    fn cascade_len() {
        let mut c = StyleCascade::new();
        c.push("a", Style::default());
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn variable_resolver_clear() {
        let mut vr = StyleVariableResolver::new();
        vr.set("a", "b");
        vr.clear();
        assert!(vr.is_empty());
    }


    #[test]
    fn style_var_resolver_basic() {
        let mut r = StyleVarResolver::new();
        r.set_variable("color", "red");
        assert_eq!(r.resolve("bg: ${color}"), "bg: red");
    }

    #[test]
    fn style_var_resolver_nested() {
        let mut r = StyleVarResolver::new();
        r.set_variable("primary", "${base}");
        r.set_variable("base", "blue");
        assert_eq!(r.resolve("${primary}"), "blue");
    }

    #[test]
    fn style_var_resolver_fallback() {
        let mut r = StyleVarResolver::new();
        r.set_fallback("missing", "default_val");
        assert_eq!(r.resolve("${missing}"), "default_val");
    }

    #[test]
    fn style_var_resolver_no_var() {
        let mut r = StyleVarResolver::new();
        assert_eq!(r.resolve("plain text"), "plain text");
    }

    #[test]
    fn style_var_resolver_count() {
        let mut r = StyleVarResolver::new();
        r.set_variable("a", "1");
        r.set_variable("b", "2");
        assert_eq!(r.variable_count(), 2);
    }

    #[test]
    fn style_var_resolver_remove() {
        let mut r = StyleVarResolver::new();
        r.set_variable("x", "y");
        assert!(r.has_variable("x"));
        r.remove_variable("x");
        assert!(!r.has_variable("x"));
    }

    #[test]
    fn style_theme_switcher_add_switch() {
        let mut sw = StyleThemeSwitcher::new();
        sw.add_theme(StyleThemeEntry::new("dark", true));
        sw.add_theme(StyleThemeEntry::new("light", false));
        assert_eq!(sw.active_theme_name(), Some("dark"));
        sw.switch_to(1);
        assert_eq!(sw.active_theme_name(), Some("light"));
    }

    #[test]
    fn style_theme_switcher_by_name() {
        let mut sw = StyleThemeSwitcher::new();
        sw.add_theme(StyleThemeEntry::new("monokai", true));
        sw.add_theme(StyleThemeEntry::new("solarized", false));
        assert!(sw.switch_by_name("solarized"));
        assert_eq!(sw.active_theme_name(), Some("solarized"));
    }

    #[test]
    fn style_theme_switcher_previous() {
        let mut sw = StyleThemeSwitcher::new();
        sw.add_theme(StyleThemeEntry::new("a", true));
        sw.add_theme(StyleThemeEntry::new("b", false));
        sw.switch_to(1);
        sw.switch_to_previous();
        assert_eq!(sw.active_theme_name(), Some("a"));
    }

    #[test]
    fn style_theme_dark_light_filter() {
        let mut sw = StyleThemeSwitcher::new();
        sw.add_theme(StyleThemeEntry::new("d1", true));
        sw.add_theme(StyleThemeEntry::new("l1", false));
        sw.add_theme(StyleThemeEntry::new("d2", true));
        assert_eq!(sw.dark_themes().len(), 2);
        assert_eq!(sw.light_themes().len(), 1);
    }

    #[test]
    fn style_theme_merge_styles() {
        let mut t1 = StyleThemeEntry::new("base", true);
        t1.set_style("bg", Style::default());
        let mut t2 = StyleThemeEntry::new("ext", true);
        t2.set_style("fg", Style::default());
        t2.set_style("bg", Style::default().fg(Color::Red));
        t1.merge_from(&t2);
        assert_eq!(t1.style_count(), 2);
    }

    #[test]
    fn style_var_resolver_max_depth() {
        let r = StyleVarResolver::new().with_max_depth(2);
        assert_eq!(r.max_depth, 2);
    }

    #[test]
    fn style_theme_priority_sort() {
        let mut sw = StyleThemeSwitcher::new();
        sw.add_theme(StyleThemeEntry::new("low", true).with_priority(1));
        sw.add_theme(StyleThemeEntry::new("high", true).with_priority(10));
        sw.add_theme(StyleThemeEntry::new("mid", true).with_priority(5));
        let sorted = sw.sorted_by_priority();
        assert_eq!(sorted[0].name, "high");
        assert_eq!(sorted[2].name, "low");
    }


    #[test]
    fn styleb_builder_valid() {
        let cfg = StyleBBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn styleb_builder_empty_name() {
        let r = StyleBBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn styleb_builder_bad_priority() {
        assert!(StyleBBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn styleb_builder_zero_max() {
        assert!(StyleBBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn styleb_cfg_merge() {
        let mut a = StyleBBuilder::new("a").property("x", "1").build().unwrap();
        let b = StyleBBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn styleb_cfg_display() {
        let cfg = StyleBBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }

    #[test]
    fn stylef_fmt_list() {
        let f = StyleFFmt::new(StyleFFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn stylef_fmt_kv() {
        let f = StyleFFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn stylef_fmt_section() {
        let f = StyleFFmt::new(StyleFFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn stylef_fmt_truncate() {
        let f = StyleFFmt::new(StyleFFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn stylef_fmt_opts_defaults() {
        let o = StyleFFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn styles_config_new() {
        let cfg = StylesConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn styles_config_set_get() {
        let mut cfg = StylesConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn styles_config_remove() {
        let mut cfg = StylesConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn styles_config_keys_sorted() {
        let mut cfg = StylesConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn styles_config_bump_version() {
        let mut cfg = StylesConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn styles_config_clear() {
        let mut cfg = StylesConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn styles_config_merge() {
        let mut cfg1 = StylesConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = StylesConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn styles_config_disable() {
        let mut cfg = StylesConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn styles_rate_tracker_empty() {
        let rt = StylesRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn styles_rate_tracker_record() {
        let mut rt = StylesRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn styles_rate_tracker_prune() {
        let mut rt = StylesRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn styles_validator_valid() {
        let v = StylesValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn styles_validator_errors() {
        let mut v = StylesValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn styles_validator_clear() {
        let mut v = StylesValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn styles_validator_merge() {
        let mut v1 = StylesValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = StylesValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn styles_rate_tracker_clear() {
        let mut rt = StylesRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yt_metrics_empty() {
        let m = YtMetrics::new("styles");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yt_metrics_record_and_mean() {
        let mut m = YtMetrics::new("styles");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yt_metrics_min_max() {
        let mut m = YtMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yt_metrics_variance_and_std() {
        let mut m = YtMetrics::new("v");
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
    fn yt_metrics_percentile() {
        let mut m = YtMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yt_metrics_merge() {
        let mut a = YtMetrics::new("a");
        a.record(1.0);
        let mut b = YtMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yt_metrics_reset() {
        let mut m = YtMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yt_rate_window_empty() {
        let rw = YtRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yt_rate_window_tick_and_rate() {
        let mut rw = YtRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yt_lru_cache_basic() {
        let mut c = YtLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yt_lru_cache_contains_and_keys() {
        let mut c = YtLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yt_lru_cache_remove() {
        let mut c = YtLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yt_metrics_sum() {
        let mut m = YtMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yt_metrics_label() {
        let m = YtMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yt_lru_cache_clear() {
        let mut c = YtLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for styles
    #[test]
    fn xa_styles_ring_new() {
        let rb = super::XaStylesRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_styles_ring_push_len() {
        let mut rb = super::XaStylesRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_styles_ring_wrap() {
        let mut rb = super::XaStylesRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_styles_ring_mean_empty() {
        let rb = super::XaStylesRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_styles_ring_mean_values() {
        let mut rb = super::XaStylesRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_styles_ring_min_max() {
        let mut rb = super::XaStylesRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_styles_ring_iter() {
        let mut rb = super::XaStylesRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_styles_counter_new() {
        let c = super::XaStylesCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_styles_counter_inc() {
        let mut c = super::XaStylesCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_styles_counter_inc_by() {
        let mut c = super::XaStylesCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_styles_counter_reset() {
        let mut c = super::XaStylesCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_styles_counter_clear() {
        let mut c = super::XaStylesCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_styles_counter_default() {
        let c = super::XaStylesCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 169 ----

    #[test]
    fn xc_169_pool_new_empty() {
        let pool: super::Xc169Pool<i32> = super::Xc169Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_169_pool_release_acquire() {
        let mut pool = super::Xc169Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_169_pool_acquire_empty() {
        let mut pool: super::Xc169Pool<i32> = super::Xc169Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_169_pool_full() {
        let mut pool = super::Xc169Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_169_pool_drain() {
        let mut pool = super::Xc169Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_169_pool_stats() {
        let mut pool = super::Xc169Pool::new(8);
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
    fn xc_169_pool_clear() {
        let mut pool = super::Xc169Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_169_pool_shrink() {
        let mut pool = super::Xc169Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_169_pool_default() {
        let pool: super::Xc169Pool<String> = super::Xc169Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_169_pool_extend() {
        let mut pool = super::Xc169Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_169_pool_retain() {
        let mut pool = super::Xc169Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_169_scheduler_round_robin() {
        let mut sched = super::Xc169Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_169_scheduler_empty() {
        let mut sched = super::Xc169Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_169_scheduler_reset() {
        let mut sched = super::Xc169Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_169_scheduler_add_remove() {
        let mut sched = super::Xc169Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_169_scheduler_targets() {
        let sched = super::Xc169Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_169_hash_empty() {
        assert_eq!(super::xc_169_hash(b""), 5381);
    }

    #[test]
    fn xc_169_hash_data() {
        let h = super::xc_169_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_169_hash(b"hello"), h);
    }

    #[test]
    fn xc_169_reverse_str() {
        assert_eq!(super::xc_169_reverse("abc"), "cba");
        assert_eq!(super::xc_169_reverse(""), "");
    }


    // --- xd_105 deepening tests ---

    #[test]
    fn xd_105_sm_initial_state() {
        let sm = Xd105StateMachine::new();
        assert_eq!(sm.current_state(), Xd105State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_105_sm_valid_idle_to_running() {
        let mut sm = Xd105StateMachine::new();
        assert!(sm.transition(Xd105State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd105State::Running);
    }

    #[test]
    fn xd_105_sm_valid_running_to_paused() {
        let mut sm = Xd105StateMachine::new();
        sm.transition(Xd105State::Running).unwrap();
        assert!(sm.transition(Xd105State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd105State::Paused);
    }

    #[test]
    fn xd_105_sm_valid_running_to_done() {
        let mut sm = Xd105StateMachine::new();
        sm.transition(Xd105State::Running).unwrap();
        assert!(sm.transition(Xd105State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd105State::Done);
    }

    #[test]
    fn xd_105_sm_valid_paused_to_running() {
        let mut sm = Xd105StateMachine::new();
        sm.transition(Xd105State::Running).unwrap();
        sm.transition(Xd105State::Paused).unwrap();
        assert!(sm.transition(Xd105State::Running).is_ok());
    }

    #[test]
    fn xd_105_sm_valid_done_to_idle() {
        let mut sm = Xd105StateMachine::new();
        sm.transition(Xd105State::Running).unwrap();
        sm.transition(Xd105State::Done).unwrap();
        assert!(sm.transition(Xd105State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd105State::Idle);
    }

    #[test]
    fn xd_105_sm_invalid_idle_to_done() {
        let mut sm = Xd105StateMachine::new();
        assert!(sm.transition(Xd105State::Done).is_err());
    }

    #[test]
    fn xd_105_sm_invalid_idle_to_paused() {
        let mut sm = Xd105StateMachine::new();
        assert!(sm.transition(Xd105State::Paused).is_err());
    }

    #[test]
    fn xd_105_sm_history_tracking() {
        let mut sm = Xd105StateMachine::new();
        sm.transition(Xd105State::Running).unwrap();
        sm.transition(Xd105State::Paused).unwrap();
        sm.transition(Xd105State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd105State::Idle);
        assert_eq!(sm.history()[0].to, Xd105State::Running);
        assert_eq!(sm.history()[1].from, Xd105State::Running);
        assert_eq!(sm.history()[2].to, Xd105State::Done);
    }

    #[test]
    fn xd_105_sm_serialize_deserialize() {
        let mut sm = Xd105StateMachine::new();
        sm.transition(Xd105State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd105StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd105State::Running));
    }

    #[test]
    fn xd_105_sm_deserialize_invalid() {
        assert_eq!(Xd105StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_105_sm_reset() {
        let mut sm = Xd105StateMachine::new();
        sm.transition(Xd105State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd105State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_105_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd105EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd105Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_105_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd105EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd105Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd105Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_105_bus_unsubscribe() {
        let mut bus = Xd105EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_105_event_kind_and_payload() {
        let e = Xd105Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd105Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_105_bus_clear_history() {
        let mut bus = Xd105EventBus::new();
        bus.publish(Xd105Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_105_sm_step_counter_increments() {
        let mut sm = Xd105StateMachine::new();
        sm.transition(Xd105State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd105State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_29 graph tests ------------------------------------------------

    #[test]
    fn xg_29_graph_empty() {
        let g = super::Xg29Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_29_graph_add_node() {
        let mut g = super::Xg29Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_29_graph_add_edge() {
        let mut g = super::Xg29Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_29_graph_neighbors() {
        let mut g = super::Xg29Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_29_graph_has_path() {
        let mut g = super::Xg29Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_29_graph_self_path() {
        let g = super::Xg29Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_29_graph_topo_sort() {
        let mut g = super::Xg29Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_29_graph_cycle_detect_false() {
        let mut g = super::Xg29Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_29_graph_cycle_detect_true() {
        let mut g = super::Xg29Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_29 heap tests -------------------------------------------------

    #[test]
    fn xg_29_heap_empty() {
        let h: super::Xg29Heap<i32> = super::Xg29Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_29_heap_push_pop() {
        let mut h = super::Xg29Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_29_heap_peek() {
        let mut h = super::Xg29Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_29_heap_drain_sorted() {
        let mut h = super::Xg29Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_29_heap_merge() {
        let mut a = super::Xg29Heap::new();
        let mut b = super::Xg29Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_29_heap_default() {
        let h: super::Xg29Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_29_graph_default() {
        let g: super::Xg29Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh168_skip_insert_contains() {
        let mut sl = super::Xh168SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh168_skip_remove() {
        let mut sl = super::Xh168SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh168_skip_len() {
        let mut sl = super::Xh168SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh168_skip_range_query() {
        let mut sl = super::Xh168SkipList::xh_new(4);
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
    fn xh168_skip_floor_ceiling() {
        let mut sl = super::Xh168SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh168_skip_rank() {
        let mut sl = super::Xh168SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh168_skip_empty() {
        let sl = super::Xh168SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh168_skip_duplicates() {
        let mut sl = super::Xh168SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh168_bitset_set_test() {
        let mut bs = super::Xh168BitSet::xh_new(256);
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
    fn xh168_bitset_clear_count() {
        let mut bs = super::Xh168BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh168_bitset_and_or_xor() {
        let mut a = super::Xh168BitSet::xh_new(128);
        let mut b = super::Xh168BitSet::xh_new(128);
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
    fn xh168_bitset_iter_ones() {
        let mut bs = super::Xh168BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh168_bitset_first_last() {
        let mut bs = super::Xh168BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh168_bitset_empty() {
        let bs = super::Xh168BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi168_deque_push_pop_back() {
        let mut dq = super::Xi168Deque::xi_new(4);
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
    fn xi168_deque_push_pop_front() {
        let mut dq = super::Xi168Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi168_deque_mixed_ops() {
        let mut dq = super::Xi168Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi168_deque_get_and_split() {
        let mut dq = super::Xi168Deque::xi_new(8);
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
    fn xi168_deque_rotate_left() {
        let mut dq = super::Xi168Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi168_deque_rotate_right() {
        let mut dq = super::Xi168Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi168_deque_grow() {
        let mut dq = super::Xi168Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi168_deque_empty() {
        let dq = super::Xi168Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi168_interval_tree_insert_query() {
        let mut tree = super::Xi168IntervalTree::xi_new();
        tree.xi_insert(super::Xi168Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi168Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi168Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi168_interval_tree_overlap() {
        let mut tree = super::Xi168IntervalTree::xi_new();
        tree.xi_insert(super::Xi168Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi168Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi168Interval::xi_new(12, 20));
        let q = super::Xi168Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi168_interval_tree_remove() {
        let mut tree = super::Xi168IntervalTree::xi_new();
        tree.xi_insert(super::Xi168Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi168Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi168_interval_tree_gaps() {
        let mut tree = super::Xi168IntervalTree::xi_new();
        tree.xi_insert(super::Xi168Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi168Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi168Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi168Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi168Interval::xi_new(8, 10));
    }

    #[test]
    fn xi168_interval_tree_merge() {
        let mut tree = super::Xi168IntervalTree::xi_new();
        tree.xi_insert(super::Xi168Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi168Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi168Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi168Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi168Interval::xi_new(10, 15));
    }

    #[test]
    fn xi168_interval_tree_all() {
        let mut tree = super::Xi168IntervalTree::xi_new();
        tree.xi_insert(super::Xi168Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi168Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi168_interval_tree_empty() {
        let tree = super::Xi168IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi168_interval_tree_contains_point() {
        let iv = super::Xi168Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 168) ---

    #[test]
    fn xj_168_uf_make_and_find() {
        let mut uf = super::Xj168UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_168_uf_union_connected() {
        let mut uf = super::Xj168UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_168_uf_component_count() {
        let mut uf = super::Xj168UnionFind::xj_new();
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
    fn xj_168_uf_component_size() {
        let mut uf = super::Xj168UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_168_uf_largest_component() {
        let mut uf = super::Xj168UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_168_uf_many_elements() {
        let mut uf = super::Xj168UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_168_uf_separate_components() {
        let mut uf = super::Xj168UnionFind::xj_new();
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
    fn xj_168_uf_path_compression() {
        let mut uf = super::Xj168UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_168_bt_insert_get() {
        let mut bt = super::Xj168BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_168_bt_contains_len() {
        let mut bt = super::Xj168BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_168_bt_replace() {
        let mut bt = super::Xj168BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_168_bt_remove() {
        let mut bt = super::Xj168BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_168_bt_keys_values() {
        let mut bt = super::Xj168BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_168_bt_range() {
        let mut bt = super::Xj168BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_168_bt_min_max() {
        let mut bt = super::Xj168BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_168_bt_many_inserts() {
        let mut bt = super::Xj168BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_168 segment tree tests ---

    #[test]
    fn xk_168_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk168SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_168_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk168SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_168_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk168SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_168_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk168SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_168_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk168SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_168_st_single_element() {
        let data = vec![42];
        let st = super::Xk168SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_168_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk168SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_168_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk168SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_168 disjoint intervals tests ---

    #[test]
    fn xk_168_di_add_and_count() {
        let mut di = super::Xk168DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_168_di_merge_overlap() {
        let mut di = super::Xk168DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_168_di_contains() {
        let mut di = super::Xk168DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_168_di_remove() {
        let mut di = super::Xk168DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_168_di_covered_length() {
        let mut di = super::Xk168DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_168_di_gaps() {
        let mut di = super::Xk168DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_168_di_merge_adjacent() {
        let mut di = super::Xk168DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_168_di_empty() {
        let di = super::Xk168DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_168_rope_new_empty() {
        let rope = super::Xl168Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_168_rope_from_str() {
        let rope = super::Xl168Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_168_rope_insert_at() {
        let mut rope = super::Xl168Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_168_rope_delete_range() {
        let mut rope = super::Xl168Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_168_rope_char_at() {
        let rope = super::Xl168Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_168_rope_split_concat() {
        let rope = super::Xl168Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_168_rope_line_count() {
        let rope = super::Xl168Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_168_rope_line_at() {
        let rope = super::Xl168Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_168_sa_build_and_search() {
        let sa = super::Xl168SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_168_sa_count() {
        let sa = super::Xl168SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_168_sa_longest_repeated() {
        let sa = super::Xl168SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_168_sa_all_positions() {
        let sa = super::Xl168SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_168_sa_len() {
        let sa = super::Xl168SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_168_sa_empty() {
        let sa = super::Xl168SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_168_rope_slice() {
        let rope = super::Xl168Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_168_sa_search_start() {
        let sa = super::Xl168SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_168_sparse_set_get() {
        let mut m = super::Xm168MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_168_sparse_row_col() {
        let mut m = super::Xm168MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_168_sparse_transpose() {
        let mut m = super::Xm168MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_168_sparse_multiply_vec() {
        let mut m = super::Xm168MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_168_sparse_nnz_density() {
        let mut m = super::Xm168MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_168_sparse_clear() {
        let mut m = super::Xm168MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_168_sparse_overwrite_zero() {
        let mut m = super::Xm168MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_168_tokenizer_basic() {
        let t = super::Xm168Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_168_tokenizer_count() {
        let t = super::Xm168Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_168_tokenizer_unique() {
        let t = super::Xm168Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_168_tokenizer_frequency() {
        let t = super::Xm168Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_168_tokenizer_delimiter() {
        let t = super::Xm168Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_168_tokenizer_whitespace() {
        let t = super::Xm168Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_168_tokenizer_empty() {
        let t = super::Xm168Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 168 ----

    #[test]
    fn xn_168_fenwick_prefix_sum() {
        let mut ft = super::Xn168Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_168_fenwick_range_sum() {
        let mut ft = super::Xn168Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_168_fenwick_point_query() {
        let mut ft = super::Xn168Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_168_fenwick_len() {
        let ft = super::Xn168Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_168_fenwick_multiple_updates() {
        let mut ft = super::Xn168Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_168_fenwick_single_element() {
        let mut ft = super::Xn168Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_168_fenwick_find_kth() {
        let mut ft = super::Xn168Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_168_fenwick_negative_delta() {
        let mut ft = super::Xn168Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 168 ----

    #[test]
    fn xn_168_avl_insert_get() {
        let mut m = super::Xn168AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_168_avl_remove() {
        let mut m = super::Xn168AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_168_avl_in_order() {
        let mut m = super::Xn168AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_168_avl_min_max() {
        let mut m = super::Xn168AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_168_avl_floor_ceiling() {
        let mut m = super::Xn168AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_168_avl_height_balanced() {
        let mut m = super::Xn168AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_168_avl_overwrite() {
        let mut m = super::Xn168AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_168_avl_empty() {
        let m: super::Xn168AVL<i32, i32> = super::Xn168AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo168RedBlack tests ---

    #[test]
    fn xo_168_rb_insert_and_get() {
        let mut tree = super::Xo168RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_168_rb_len_and_empty() {
        let mut tree = super::Xo168RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_168_rb_min_max() {
        let mut tree = super::Xo168RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_168_rb_contains() {
        let mut tree = super::Xo168RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_168_rb_remove() {
        let mut tree = super::Xo168RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_168_rb_in_order() {
        let mut tree = super::Xo168RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_168_rb_black_height() {
        let mut tree = super::Xo168RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_168_rb_overwrite() {
        let mut tree = super::Xo168RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo168ConsistentHash tests ---

    #[test]
    fn xo_168_ch_add_and_count() {
        let mut ring = super::Xo168ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_168_ch_remove_node() {
        let mut ring = super::Xo168ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_168_ch_get_node() {
        let mut ring = super::Xo168ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_168_ch_empty_ring() {
        let ring = super::Xo168ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_168_ch_distribution() {
        let mut ring = super::Xo168ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_168_ch_rebalance() {
        let mut ring = super::Xo168ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_168_ch_virtual_nodes() {
        let mut ring = super::Xo168ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_168_ch_consistent_lookup() {
        let mut ring = super::Xo168ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_168_splay_insert_get() {
        let mut t = super::Xp168SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_168_splay_remove() {
        let mut t = super::Xp168SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_168_splay_count_increases() {
        let mut t = super::Xp168SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_168_splay_depth() {
        let mut t = super::Xp168SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_168_splay_len_empty() {
        let t = super::Xp168SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_168_splay_min_max() {
        let mut t = super::Xp168SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_168_splay_overwrite() {
        let mut t = super::Xp168SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_168_splay_remove_missing() {
        let mut t = super::Xp168SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_168 treap tests ----
    #[test]
    fn xq_168_treap_empty() {
        let t = super::Xq168Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_168_treap_insert_get() {
        let mut t = super::Xq168Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_168_treap_overwrite() {
        let mut t = super::Xq168Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_168_treap_remove() {
        let mut t = super::Xq168Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_168_treap_min_max() {
        let mut t = super::Xq168Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_168_treap_rank() {
        let mut t = super::Xq168Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_168_treap_kth() {
        let mut t = super::Xq168Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_168_treap_in_order() {
        let mut t = super::Xq168Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_168 VEB tree tests ----
    #[test]
    fn xq_168_veb_empty() {
        let v = super::Xq168VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_168_veb_insert_contains() {
        let mut v = super::Xq168VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_168_veb_min_max() {
        let mut v = super::Xq168VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_168_veb_delete() {
        let mut v = super::Xq168VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_168_veb_successor() {
        let mut v = super::Xq168VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_168_veb_predecessor() {
        let mut v = super::Xq168VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_168_veb_count() {
        let mut v = super::Xq168VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_168_veb_duplicate_insert() {
        let mut v = super::Xq168VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_168_kdtree_empty() {
        let tree = super::Xr168KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_168_kdtree_insert_one() {
        let mut tree = super::Xr168KDTree::xr_new();
        tree.xr_insert(super::Xr168KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_168_kdtree_insert_multiple() {
        let mut tree = super::Xr168KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr168KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_168_kdtree_nearest_neighbor() {
        let mut tree = super::Xr168KDTree::xr_new();
        tree.xr_insert(super::Xr168KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr168KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr168KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_168_kdtree_nn_empty() {
        let tree = super::Xr168KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr168KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_168_kdtree_range_search() {
        let mut tree = super::Xr168KDTree::xr_new();
        tree.xr_insert(super::Xr168KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr168KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr168KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_168_kdtree_range_empty() {
        let mut tree = super::Xr168KDTree::xr_new();
        tree.xr_insert(super::Xr168KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_168_kdtree_all_points() {
        let mut tree = super::Xr168KDTree::xr_new();
        tree.xr_insert(super::Xr168KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr168KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_168_kdtree_depth() {
        let mut tree = super::Xr168KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr168KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_168_kdtree_bounding_box() {
        let mut tree = super::Xr168KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr168KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr168KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_168_persistent_array_new() {
        let arr = super::Xs168PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_168_persistent_array_push() {
        let mut arr = super::Xs168PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_168_persistent_array_set() {
        let mut arr = super::Xs168PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_168_persistent_array_diff() {
        let mut arr = super::Xs168PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_168_persistent_array_rollback() {
        let mut arr = super::Xs168PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_168_persistent_array_history() {
        let mut arr = super::Xs168PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_168_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs168PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_168_persistent_array_from_vec() {
        let arr = super::Xs168PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_168_concurrent_queue_new() {
        let q = super::Xs168ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_168_concurrent_queue_push_pop() {
        let mut q = super::Xs168ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_168_concurrent_queue_full() {
        let mut q = super::Xs168ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_168_concurrent_queue_drain() {
        let mut q = super::Xs168ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_168_concurrent_queue_try_pop() {
        let mut q = super::Xs168ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_168_concurrent_queue_clear() {
        let mut q = super::Xs168ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_168_range_map_new() {
        let rm = super::Xs168RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_168_range_map_insert_get() {
        let mut rm = super::Xs168RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_168_range_map_overlap() {
        let mut rm = super::Xs168RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_168_range_map_remove() {
        let mut rm = super::Xs168RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_168_range_map_gaps() {
        let mut rm = super::Xs168RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_168_range_map_coverage() {
        let mut rm = super::Xs168RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_168_range_map_contains() {
        let mut rm = super::Xs168RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_168_range_map_clear() {
        let mut rm = super::Xs168RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_168_circular_buffer_new() {
        let buf = super::Xs168CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_168_circular_buffer_push_pop() {
        let mut buf = super::Xs168CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_168_circular_buffer_overwrite() {
        let mut buf = super::Xs168CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_168_circular_buffer_peek() {
        let mut buf = super::Xs168CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_168_circular_buffer_is_full() {
        let mut buf = super::Xs168CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_168_circular_buffer_iter() {
        let mut buf = super::Xs168CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_168_circular_buffer_clear() {
        let mut buf = super::Xs168CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_168_circular_buffer_to_vec() {
        let mut buf = super::Xs168CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

    #[test]
    fn xs_168_stats_tracker_new() {
        let tracker = super::Xs168StatsTracker::xs_new();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_168_stats_tracker_mean() {
        let mut tracker = super::Xs168StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        tracker.xs_add(30.0);
        assert!((tracker.xs_mean() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn xs_168_stats_tracker_min_max() {
        let mut tracker = super::Xs168StatsTracker::xs_new();
        tracker.xs_add(5.0);
        tracker.xs_add(15.0);
        tracker.xs_add(10.0);
        assert_eq!(tracker.xs_min(), Some(5.0));
        assert_eq!(tracker.xs_max(), Some(15.0));
    }

    #[test]
    fn xs_168_stats_tracker_median() {
        let mut tracker = super::Xs168StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(3.0);
        tracker.xs_add(2.0);
        assert_eq!(tracker.xs_median(), Some(2.0));
    }

    #[test]
    fn xs_168_stats_tracker_variance() {
        let mut tracker = super::Xs168StatsTracker::xs_new();
        tracker.xs_add(2.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(5.0);
        tracker.xs_add(5.0);
        tracker.xs_add(7.0);
        tracker.xs_add(9.0);
        let var = tracker.xs_variance();
        assert!(var > 0.0);
    }

    #[test]
    fn xs_168_stats_tracker_range() {
        let mut tracker = super::Xs168StatsTracker::xs_new();
        tracker.xs_add(3.0);
        tracker.xs_add(7.0);
        tracker.xs_add(1.0);
        assert!((tracker.xs_range() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn xs_168_stats_tracker_clear() {
        let mut tracker = super::Xs168StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(2.0);
        tracker.xs_clear();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_168_stats_tracker_sum() {
        let mut tracker = super::Xs168StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        assert!((tracker.xs_sum() - 30.0).abs() < 1e-9);
    }

}