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

}