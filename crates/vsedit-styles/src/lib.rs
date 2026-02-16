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
}
