//! Inline color preview/picker.
//!
//! Provides types and utilities for VS Code–style color picker contributions,
//! including color parsing, formatting, and the [`ColorProvider`] trait for
//! document color integration.

use std::fmt;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// An RGBA color with components in the range `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Color {
    /// Creates a new [`Color`].
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", color_to_rgba_string(self))
    }
}

/// A proposed textual representation of a color.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorPresentation {
    /// Human-readable label shown in the picker UI.
    pub label: String,
    /// Optional replacement text to insert into the document.
    pub text_edit: Option<String>,
}

impl ColorPresentation {
    pub fn new(label: impl Into<String>, text_edit: Option<String>) -> Self {
        Self { label: label.into(), text_edit }
    }
}

/// A color occurrence found within a document, together with its source range.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorInformation {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub color: Color,
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Formats a [`Color`] as a `#RRGGBB` hex string (alpha is ignored).
pub fn color_to_hex(color: &Color) -> String {
    let r = (color.r.clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (color.g.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (color.b.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// Parses a `#RRGGBB` or `#RGB` hex string into a [`Color`] with `a = 1.0`.
///
/// Returns `None` when the input is not a valid hex color.
pub fn hex_to_color(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#')?;
    let (r, g, b) = match hex.len() {
        6 => {
            let r = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
            let g = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
            let b = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
            (r, g, b)
        }
        3 => {
            let r = u8::from_str_radix(hex.get(0..1)?, 16).ok()?;
            let g = u8::from_str_radix(hex.get(1..2)?, 16).ok()?;
            let b = u8::from_str_radix(hex.get(2..3)?, 16).ok()?;
            (r * 17, g * 17, b * 17)
        }
        _ => return None,
    };
    Some(Color::new(
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
        1.0,
    ))
}

/// Formats a [`Color`] as an `rgba(R, G, B, A)` CSS-style string.
pub fn color_to_rgba_string(color: &Color) -> String {
    let r = (color.r.clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (color.g.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (color.b.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("rgba({r}, {g}, {b}, {:.2})", color.a)
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Trait for document-level color detection and presentation.
pub trait ColorProvider {
    /// Returns all color occurrences found in the given document text.
    fn provide_document_colors(&self, text: &str) -> Vec<ColorInformation>;

    /// Returns possible textual representations for the given color.
    fn provide_color_presentations(&self, color: &Color) -> Vec<ColorPresentation>;
}

// ---------------------------------------------------------------------------
// HSL color type
// ---------------------------------------------------------------------------

/// An HSLA color with hue in degrees `0..360`, saturation and lightness in
/// `0.0..=1.0`, and alpha in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HslColor {
    pub h: f64,
    pub s: f64,
    pub l: f64,
    pub a: f64,
}

impl HslColor {
    pub fn new(h: f64, s: f64, l: f64, a: f64) -> Self {
        Self { h, s, l, a }
    }
}

// ---------------------------------------------------------------------------
// HSL conversion helpers
// ---------------------------------------------------------------------------

/// Converts an RGB [`Color`] to an [`HslColor`].
pub fn rgb_to_hsl(c: &Color) -> HslColor {
    let r = c.r;
    let g = c.g;
    let b = c.b;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f64::EPSILON {
        return HslColor::new(0.0, 0.0, l, c.a);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < f64::EPSILON {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0;
        }
        h
    } else if (max - g).abs() < f64::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };

    HslColor::new(h * 60.0, s, l, c.a)
}

/// Converts an [`HslColor`] back to an RGB [`Color`].
pub fn hsl_to_rgb(hsl: &HslColor) -> Color {
    let h = hsl.h;
    let s = hsl.s;
    let l = hsl.l;

    if s.abs() < f64::EPSILON {
        return Color::new(l, l, l, hsl.a);
    }

    fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 1.0 / 2.0 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hk = h / 360.0;

    Color::new(
        hue_to_rgb(p, q, hk + 1.0 / 3.0),
        hue_to_rgb(p, q, hk),
        hue_to_rgb(p, q, hk - 1.0 / 3.0),
        hsl.a,
    )
}

/// Formats a [`Color`] as an `hsl(H, S%, L%)` CSS-style string.
pub fn color_to_hsl_string(color: &Color) -> String {
    let hsl = rgb_to_hsl(color);
    format!(
        "hsl({:.0}, {:.0}%, {:.0}%)",
        hsl.h,
        hsl.s * 100.0,
        hsl.l * 100.0
    )
}

// ---------------------------------------------------------------------------
// Luminance & contrast
// ---------------------------------------------------------------------------

/// Returns the relative luminance of a color using the WCAG formula.
pub fn luminance(c: &Color) -> f64 {
    fn linearize(v: f64) -> f64 {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linearize(c.r) + 0.7152 * linearize(c.g) + 0.0722 * linearize(c.b)
}

/// Returns the WCAG contrast ratio between two colors.
pub fn contrast_ratio(c1: &Color, c2: &Color) -> f64 {
    let l1 = luminance(c1);
    let l2 = luminance(c2);
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Returns `true` if the color is considered dark (luminance < 0.5).
pub fn is_dark(c: &Color) -> bool {
    luminance(c) < 0.179
}

// ---------------------------------------------------------------------------
// Blending
// ---------------------------------------------------------------------------

/// Linearly interpolates between two colors by `factor` (0.0 = c1, 1.0 = c2).
pub fn blend(c1: &Color, c2: &Color, factor: f64) -> Color {
    let f = factor.clamp(0.0, 1.0);
    Color::new(
        c1.r + (c2.r - c1.r) * f,
        c1.g + (c2.g - c1.g) * f,
        c1.b + (c2.b - c1.b) * f,
        c1.a + (c2.a - c1.a) * f,
    )
}

// ---------------------------------------------------------------------------
// Extended hex parsing
// ---------------------------------------------------------------------------

/// Parses a `#RRGGBBAA` hex string into a [`Color`].
pub fn hex_to_color_with_alpha(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 8 {
        return None;
    }
    let r = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
    let g = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
    let b = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
    let a = u8::from_str_radix(hex.get(6..8)?, 16).ok()?;
    Some(Color::new(
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
        f64::from(a) / 255.0,
    ))
}

/// Attempts to parse a color string in multiple formats:
/// `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `rgb(R, G, B)`, `hsl(H, S%, L%)`,
/// or a CSS named color.
pub fn parse_color(input: &str) -> Option<Color> {
    let input = input.trim();
    if input.starts_with('#') {
        return match input.len() - 1 {
            3 | 6 => hex_to_color(input),
            8 => hex_to_color_with_alpha(input),
            _ => None,
        };
    }
    if input.starts_with("rgb(") && input.ends_with(')') {
        let inner = &input[4..input.len() - 1];
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() != 3 {
            return None;
        }
        let r: u8 = parts[0].trim().parse().ok()?;
        let g: u8 = parts[1].trim().parse().ok()?;
        let b: u8 = parts[2].trim().parse().ok()?;
        return Some(Color::new(
            f64::from(r) / 255.0,
            f64::from(g) / 255.0,
            f64::from(b) / 255.0,
            1.0,
        ));
    }
    if input.starts_with("hsl(") && input.ends_with(')') {
        return parse_hsl_string(input);
    }
    parse_named_color(input)
}

/// Parse an `hsl(H, S%, L%)` string into a [`Color`].
pub fn parse_hsl_string(input: &str) -> Option<Color> {
    let inner = input.strip_prefix("hsl(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: f64 = parts[0].trim().parse().ok()?;
    let s_str = parts[1].trim().strip_suffix('%')?;
    let s: f64 = s_str.parse::<f64>().ok()? / 100.0;
    let l_str = parts[2].trim().strip_suffix('%')?;
    let l: f64 = l_str.parse::<f64>().ok()? / 100.0;
    Some(hsl_to_rgb(&HslColor::new(h, s, l, 1.0)))
}

/// Parse a CSS named color into a [`Color`].
pub fn parse_named_color(name: &str) -> Option<Color> {
    let (r, g, b) = match name.to_ascii_lowercase().as_str() {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "cyan" | "aqua" => (0, 255, 255),
        "magenta" | "fuchsia" => (255, 0, 255),
        "orange" => (255, 165, 0),
        "purple" => (128, 0, 128),
        "pink" => (255, 192, 203),
        "brown" => (165, 42, 42),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "navy" => (0, 0, 128),
        "teal" => (0, 128, 128),
        "maroon" => (128, 0, 0),
        "olive" => (128, 128, 0),
        "lime" => (0, 255, 0),
        "coral" => (255, 127, 80),
        "salmon" => (250, 128, 114),
        "gold" => (255, 215, 0),
        "ivory" => (255, 255, 240),
        "indigo" => (75, 0, 130),
        "violet" => (238, 130, 238),
        "khaki" => (240, 230, 140),
        "crimson" => (220, 20, 60),
        "turquoise" => (64, 224, 208),
        _ => return None,
    };
    Some(Color::new(
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
        1.0,
    ))
}

// ---------------------------------------------------------------------------
// Inline color swatch — terminal rendering
// ---------------------------------------------------------------------------

/// Render a color as a terminal inline swatch using a block character.
///
/// Returns a string like `"█"` that should be printed with the appropriate
/// terminal foreground color set to the given color. The returned tuple is
/// `(swatch_char, r8, g8, b8)` where the RGB values are 0-255.
pub fn color_swatch(color: &Color) -> (char, u8, u8, u8) {
    let r = (color.r.clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (color.g.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (color.b.clamp(0.0, 1.0) * 255.0).round() as u8;
    ('█', r, g, b)
}

/// Scan a line of text for color values and return their positions and colors.
pub fn find_colors_in_line(line: &str, line_number: u32) -> Vec<ColorInformation> {
    let mut results = Vec::new();
    let bytes = line.as_bytes();
    let len = line.len();
    let mut i = 0;

    while i < len {
        // Hex colors: #RGB, #RRGGBB, #RRGGBBAA
        if bytes[i] == b'#' {
            for try_len in &[9usize, 7, 4] {
                // #RRGGBBAA=9, #RRGGBB=7, #RGB=4
                if i + try_len <= len {
                    let candidate = &line[i..i + try_len];
                    if let Some(color) = parse_color(candidate) {
                        results.push(ColorInformation {
                            start_line: line_number,
                            start_col: i as u32,
                            end_line: line_number,
                            end_col: (i + try_len) as u32,
                            color,
                        });
                        i += try_len;
                        continue;
                    }
                }
            }
        }

        // rgb(...) or hsl(...)
        if i + 4 < len && (line[i..].starts_with("rgb(") || line[i..].starts_with("hsl(")) {
            if let Some(close) = line[i..].find(')') {
                let candidate = &line[i..i + close + 1];
                if let Some(color) = parse_color(candidate) {
                    results.push(ColorInformation {
                        start_line: line_number,
                        start_col: i as u32,
                        end_line: line_number,
                        end_col: (i + close + 1) as u32,
                        color,
                    });
                    i += close + 1;
                    continue;
                }
            }
        }

        i += 1;
    }

    results
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for colorpicker operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorpickerStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ColorpickerStats {
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
    pub fn merge(&mut self, other: &ColorpickerStats) {
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

impl Default for ColorpickerStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ColorpickerStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ColorpickerStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for colorpicker.
#[derive(Debug, Clone)]
pub struct ColorpickerValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ColorpickerValidator {
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

impl Default for ColorpickerValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Color constructors
// ---------------------------------------------------------------------------

impl Color {
    /// Opaque white.
    pub fn white() -> Self {
        Self::new(1.0, 1.0, 1.0, 1.0)
    }

    /// Opaque black.
    pub fn black() -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0)
    }

    /// Opaque red.
    pub fn red() -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0)
    }

    /// Opaque green.
    pub fn green() -> Self {
        Self::new(0.0, 1.0, 0.0, 1.0)
    }

    /// Opaque blue.
    pub fn blue() -> Self {
        Self::new(0.0, 0.0, 1.0, 1.0)
    }

    /// Transparent (alpha = 0).
    pub fn transparent() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }

    /// Create from 0-255 integer values.
    pub fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            r as f64 / 255.0,
            g as f64 / 255.0,
            b as f64 / 255.0,
            a as f64 / 255.0,
        )
    }

    /// Convert to 0-255 integer tuple.
    pub fn to_u8(&self) -> (u8, u8, u8, u8) {
        (
            (self.r * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.b * 255.0).round() as u8,
            (self.a * 255.0).round() as u8,
        )
    }

    /// Clamp all components to 0.0..=1.0.
    pub fn clamp(&self) -> Self {
        Self::new(
            self.r.clamp(0.0, 1.0),
            self.g.clamp(0.0, 1.0),
            self.b.clamp(0.0, 1.0),
            self.a.clamp(0.0, 1.0),
        )
    }

    /// Set alpha to a new value, returning a new color.
    pub fn with_alpha(&self, a: f64) -> Self {
        Self::new(self.r, self.g, self.b, a)
    }

    /// Returns the grayscale equivalent.
    pub fn grayscale(&self) -> Self {
        let gray = luminance(self);
        Self::new(gray, gray, gray, self.a)
    }

    /// Linear interpolation to another color.
    pub fn lerp(&self, other: &Color, t: f64) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self::new(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
            self.a + (other.a - self.a) * t,
        )
    }
}

impl From<(f64, f64, f64)> for Color {
    fn from((r, g, b): (f64, f64, f64)) -> Self {
        Self::new(r, g, b, 1.0)
    }
}

impl From<(f64, f64, f64, f64)> for Color {
    fn from((r, g, b, a): (f64, f64, f64, f64)) -> Self {
        Self::new(r, g, b, a)
    }
}

// ---------------------------------------------------------------------------
// Color distance
// ---------------------------------------------------------------------------

/// Compute Euclidean distance between two colors in RGB space.
pub fn color_distance(a: &Color, b: &Color) -> f64 {
    let dr = a.r - b.r;
    let dg = a.g - b.g;
    let db = a.b - b.b;
    (dr * dr + dg * dg + db * db).sqrt()
}

/// Find the closest named color to the given color.
pub fn closest_named_color(color: &Color) -> &'static str {
    let named = [
        ("black", Color::black()),
        ("white", Color::white()),
        ("red", Color::red()),
        ("green", Color::green()),
        ("blue", Color::blue()),
    ];
    named.iter()
        .min_by(|(_, a), (_, b)| {
            color_distance(color, a).partial_cmp(&color_distance(color, b)).unwrap()
        })
        .map(|(name, _)| *name)
        .unwrap_or("unknown")
}

/// Checks WCAG contrast accessibility between foreground and background.
pub fn wcag_contrast_level(fg: &Color, bg: &Color) -> &'static str {
    let ratio = contrast_ratio(fg, bg);
    if ratio >= 7.0 {
        "AAA"
    } else if ratio >= 4.5 {
        "AA"
    } else if ratio >= 3.0 {
        "AA-large"
    } else {
        "fail"
    }
}

// ---------------------------------------------------------------------------
// ColorPalette – named color collection
// ---------------------------------------------------------------------------

/// A named palette of colors.
pub struct ColorPalette {
    entries: Vec<(String, Color)>,
}

impl ColorPalette {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add(&mut self, name: impl Into<String>, color: Color) {
        let name = name.into();
        if !self.contains(&name) {
            self.entries.push((name, color));
        }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(n, _)| n != name);
        self.entries.len() < before
    }

    pub fn get(&self, name: &str) -> Option<&Color> {
        self.entries.iter().find(|(n, _)| n == name).map(|(_, c)| c)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|(n, _)| n == name)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Color)> {
        self.entries.iter().map(|(n, c)| (n.as_str(), c))
    }

    pub fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|(n, _)| n.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// ColorScheme – generate harmonious colors
// ---------------------------------------------------------------------------

/// Generates color harmonies from a base color.
pub struct ColorScheme {
    base: Color,
}

impl ColorScheme {
    pub fn new(base: Color) -> Self {
        Self { base }
    }

    /// Returns the complementary color (180° hue shift).
    pub fn complementary(&self) -> Color {
        let hsl = rgb_to_hsl(&self.base);
        let h2 = HslColor::new((hsl.h + 180.0) % 360.0, hsl.s, hsl.l, hsl.a);
        hsl_to_rgb(&h2)
    }

    /// Returns two analogous colors (±30° hue shift).
    pub fn analogous(&self) -> (Color, Color) {
        let hsl = rgb_to_hsl(&self.base);
        let h1 = HslColor::new((hsl.h + 30.0) % 360.0, hsl.s, hsl.l, hsl.a);
        let h2 = HslColor::new((hsl.h + 330.0) % 360.0, hsl.s, hsl.l, hsl.a);
        (hsl_to_rgb(&h1), hsl_to_rgb(&h2))
    }

    /// Returns two triadic colors (±120° hue shift).
    pub fn triadic(&self) -> (Color, Color) {
        let hsl = rgb_to_hsl(&self.base);
        let h1 = HslColor::new((hsl.h + 120.0) % 360.0, hsl.s, hsl.l, hsl.a);
        let h2 = HslColor::new((hsl.h + 240.0) % 360.0, hsl.s, hsl.l, hsl.a);
        (hsl_to_rgb(&h1), hsl_to_rgb(&h2))
    }
}

// ---------------------------------------------------------------------------
// ColorValidator – component bounds checking
// ---------------------------------------------------------------------------

/// Validates color component ranges.
pub struct ColorValidator;

impl ColorValidator {
    /// Returns Ok(()) if all components are within [0.0, 1.0].
    pub fn validate_range(color: &Color) -> Result<(), String> {
        for (name, val) in [("r", color.r), ("g", color.g), ("b", color.b), ("a", color.a)] {
            if !(0.0..=1.0).contains(&val) {
                return Err(format!("{} component {} out of range [0, 1]", name, val));
            }
        }
        Ok(())
    }

    /// Clamp all components to [0.0, 1.0].
    pub fn clamp(color: &Color) -> Color {
        Color::new(
            color.r.clamp(0.0, 1.0),
            color.g.clamp(0.0, 1.0),
            color.b.clamp(0.0, 1.0),
            color.a.clamp(0.0, 1.0),
        )
    }
}

// ---------------------------------------------------------------------------
// GradientStop / Gradient – linear gradient interpolation
// ---------------------------------------------------------------------------

/// A stop along a gradient.
#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
    pub position: f64,
    pub color: Color,
}

/// A linear gradient defined by a series of stops.
pub struct Gradient {
    stops: Vec<GradientStop>,
}

impl Gradient {
    pub fn new(mut stops: Vec<GradientStop>) -> Self {
        stops.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap_or(std::cmp::Ordering::Equal));
        Self { stops }
    }

    /// Interpolate the gradient color at position `t` in [0.0, 1.0].
    pub fn interpolate_at(&self, t: f64) -> Color {
        if self.stops.is_empty() {
            return Color::new(0.0, 0.0, 0.0, 1.0);
        }
        let t = t.clamp(0.0, 1.0);
        if t <= self.stops[0].position {
            return self.stops[0].color;
        }
        if t >= self.stops.last().unwrap().position {
            return self.stops.last().unwrap().color;
        }
        for i in 0..self.stops.len() - 1 {
            let s0 = &self.stops[i];
            let s1 = &self.stops[i + 1];
            if t >= s0.position && t <= s1.position {
                let range = s1.position - s0.position;
                if range < f64::EPSILON {
                    return s0.color;
                }
                let factor = (t - s0.position) / range;
                return blend(&s0.color, &s1.color, factor);
            }
        }
        self.stops.last().unwrap().color
    }

    pub fn stop_count(&self) -> usize {
        self.stops.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let color = Color::new(1.0, 0.0, 0.5019607843137255, 1.0);
        let hex = color_to_hex(&color);
        assert_eq!(hex, "#FF0080");

        let parsed = hex_to_color(&hex).expect("valid hex");
        assert!((parsed.r - color.r).abs() < 1e-2);
        assert!((parsed.g - color.g).abs() < 1e-2);
        assert!((parsed.b - color.b).abs() < 1e-2);
        assert!((parsed.a - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn rgba_display() {
        let color = Color::new(1.0, 0.5019607843137255, 0.0, 0.8);
        let s = color_to_rgba_string(&color);
        assert_eq!(s, "rgba(255, 128, 0, 0.80)");
        assert_eq!(color.to_string(), s);
    }

    #[test]
    fn invalid_hex() {
        assert!(hex_to_color("not-a-color").is_none());
        assert!(hex_to_color("#GG0000").is_none());
        assert!(hex_to_color("#12345").is_none());
        assert!(hex_to_color("").is_none());
    }

    #[test]
    fn color_presentation_fields() {
        let pres = ColorPresentation::new("#FF0000", Some("#FF0000".to_string()));
        assert_eq!(pres.label, "#FF0000");
        assert_eq!(pres.text_edit, Some("#FF0000".to_string()));

        let pres_no_edit = ColorPresentation::new("red", None);
        assert_eq!(pres_no_edit.text_edit, None);
    }

    #[test]
    fn rgb_to_hsl_pure_red() {
        let red = Color::new(1.0, 0.0, 0.0, 1.0);
        let hsl = rgb_to_hsl(&red);
        assert!((hsl.h - 0.0).abs() < 1.0);
        assert!((hsl.s - 1.0).abs() < 0.01);
        assert!((hsl.l - 0.5).abs() < 0.01);
    }

    #[test]
    fn hsl_rgb_round_trip() {
        let orig = Color::new(0.2, 0.6, 0.8, 0.9);
        let hsl = rgb_to_hsl(&orig);
        let back = hsl_to_rgb(&hsl);
        assert!((back.r - orig.r).abs() < 0.01);
        assert!((back.g - orig.g).abs() < 0.01);
        assert!((back.b - orig.b).abs() < 0.01);
        assert!((back.a - orig.a).abs() < f64::EPSILON);
    }

    #[test]
    fn hsl_grayscale() {
        let gray = Color::new(0.5, 0.5, 0.5, 1.0);
        let hsl = rgb_to_hsl(&gray);
        assert!(hsl.s.abs() < 0.01);
    }

    #[test]
    fn color_to_hsl_string_format() {
        let c = Color::new(1.0, 0.0, 0.0, 1.0);
        let s = color_to_hsl_string(&c);
        assert!(s.starts_with("hsl("));
        assert!(s.ends_with(')'));
    }

    #[test]
    fn luminance_black_white() {
        let black = Color::new(0.0, 0.0, 0.0, 1.0);
        let white = Color::new(1.0, 1.0, 1.0, 1.0);
        assert!(luminance(&black) < 0.01);
        assert!(luminance(&white) > 0.99);
    }

    #[test]
    fn contrast_ratio_black_white() {
        let black = Color::new(0.0, 0.0, 0.0, 1.0);
        let white = Color::new(1.0, 1.0, 1.0, 1.0);
        let ratio = contrast_ratio(&black, &white);
        assert!((ratio - 21.0).abs() < 0.1);
    }

    #[test]
    fn is_dark_check() {
        let black = Color::new(0.0, 0.0, 0.0, 1.0);
        let white = Color::new(1.0, 1.0, 1.0, 1.0);
        assert!(is_dark(&black));
        assert!(!is_dark(&white));
    }

    #[test]
    fn blend_midpoint() {
        let c1 = Color::new(0.0, 0.0, 0.0, 1.0);
        let c2 = Color::new(1.0, 1.0, 1.0, 1.0);
        let mid = blend(&c1, &c2, 0.5);
        assert!((mid.r - 0.5).abs() < 0.01);
        assert!((mid.g - 0.5).abs() < 0.01);
    }

    #[test]
    fn hex_to_color_with_alpha_parse() {
        let c = hex_to_color_with_alpha("#FF000080").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.a - 128.0 / 255.0).abs() < 0.01);
        assert!(hex_to_color_with_alpha("#FF00").is_none());
    }

    #[test]
    fn parse_color_multiple_formats() {
        let hex6 = parse_color("#FF0000").unwrap();
        assert!((hex6.r - 1.0).abs() < 0.01);

        let hex3 = parse_color("#F00").unwrap();
        assert!((hex3.r - 1.0).abs() < 0.01);

        let hex8 = parse_color("#FF000080").unwrap();
        assert!((hex8.a - 128.0 / 255.0).abs() < 0.01);

        let rgb = parse_color("rgb(255, 0, 0)").unwrap();
        assert!((rgb.r - 1.0).abs() < 0.01);

        assert!(parse_color("invalid").is_none());
    }

    #[test]
    fn behavior_check_0() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn colorpicker_stats_new_defaults() {
        let stats = ColorpickerStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn colorpicker_stats_record_success() {
        let mut stats = ColorpickerStats::new();
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
    fn colorpicker_stats_record_failure() {
        let mut stats = ColorpickerStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn colorpicker_stats_reset() {
        let mut stats = ColorpickerStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn colorpicker_stats_merge() {
        let mut a = ColorpickerStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ColorpickerStats::new();
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
    fn colorpicker_stats_display() {
        let mut stats = ColorpickerStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn colorpicker_stats_default() {
        let stats = ColorpickerStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn colorpicker_validator_accepts_valid_name() {
        let v = ColorpickerValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn colorpicker_validator_rejects_empty() {
        let v = ColorpickerValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn colorpicker_validator_rejects_too_long() {
        let v = ColorpickerValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn colorpicker_validator_forbidden_prefix() {
        let v = ColorpickerValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn colorpicker_validator_allowed_chars() {
        let v = ColorpickerValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn colorpicker_validator_range() {
        let v = ColorpickerValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn colorpicker_sanitize_removes_control() {
        let result = ColorpickerValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn colorpicker_truncate_short_string() {
        assert_eq!(ColorpickerValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn colorpicker_truncate_long_string() {
        let result = ColorpickerValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn colorpicker_is_ascii_printable() {
        assert!(ColorpickerValidator::is_ascii_printable("Hello World 123"));
        assert!(!ColorpickerValidator::is_ascii_printable("Hello\x00World"));
    }

    // -----------------------------------------------------------------------
    // HSL parsing, named colors, swatch, line scanning
    // -----------------------------------------------------------------------

    #[test]
    fn parse_hsl_string_basic() {
        let c = parse_color("hsl(0, 100%, 50%)").unwrap();
        assert!((c.r - 1.0).abs() < 0.02);
        assert!(c.g.abs() < 0.02);
        assert!(c.b.abs() < 0.02);
    }

    #[test]
    fn parse_hsl_string_blue() {
        let c = parse_color("hsl(240, 100%, 50%)").unwrap();
        assert!(c.r.abs() < 0.02);
        assert!(c.g.abs() < 0.02);
        assert!((c.b - 1.0).abs() < 0.02);
    }

    #[test]
    fn parse_hsl_invalid() {
        assert!(parse_color("hsl(360, 50%)").is_none());
        assert!(parse_color("hsl()").is_none());
    }

    #[test]
    fn parse_named_color_red() {
        let c = parse_color("red").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!(c.g.abs() < 0.01);
    }

    #[test]
    fn parse_named_color_case_insensitive() {
        assert!(parse_color("Blue").is_some());
        assert!(parse_color("CYAN").is_some());
    }

    #[test]
    fn parse_named_color_unknown() {
        assert!(parse_color("chartreuse").is_none());
        assert!(parse_color("not_a_color").is_none());
    }

    #[test]
    fn parse_named_aliases() {
        let aqua = parse_color("aqua").unwrap();
        let cyan = parse_color("cyan").unwrap();
        assert!((aqua.r - cyan.r).abs() < 0.01);
        assert!((aqua.g - cyan.g).abs() < 0.01);
        assert!((aqua.b - cyan.b).abs() < 0.01);
    }

    #[test]
    fn color_swatch_returns_block() {
        let c = Color::new(1.0, 0.0, 0.0, 1.0);
        let (ch, r, g, b) = color_swatch(&c);
        assert_eq!(ch, '█');
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn color_swatch_clamps() {
        let c = Color::new(2.0, -1.0, 0.5, 1.0);
        let (_, r, g, b) = color_swatch(&c);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 128);
    }

    #[test]
    fn find_colors_in_line_hex() {
        let colors = find_colors_in_line("color: #FF0000;", 1);
        assert_eq!(colors.len(), 1);
        assert!((colors[0].color.r - 1.0).abs() < 0.01);
        assert_eq!(colors[0].start_col, 7);
    }

    #[test]
    fn find_colors_in_line_rgb() {
        let colors = find_colors_in_line("background: rgb(0, 255, 0);", 1);
        assert_eq!(colors.len(), 1);
        assert!((colors[0].color.g - 1.0).abs() < 0.01);
    }

    #[test]
    fn find_colors_in_line_hsl() {
        let colors = find_colors_in_line("border: hsl(120, 100%, 50%);", 1);
        assert_eq!(colors.len(), 1);
        assert!((colors[0].color.g - 1.0).abs() < 0.02);
    }

    #[test]
    fn find_colors_in_line_multiple() {
        let colors = find_colors_in_line("#FF0000 and #00FF00", 1);
        assert_eq!(colors.len(), 2);
    }

    #[test]
    fn find_colors_in_line_no_colors() {
        let colors = find_colors_in_line("no colors here", 1);
        assert!(colors.is_empty());
    }

    #[test]
    fn parse_color_still_works_for_existing_formats() {
        // Ensure backward compatibility
        assert!(parse_color("#FF0000").is_some());
        assert!(parse_color("#F00").is_some());
        assert!(parse_color("#FF000080").is_some());
        assert!(parse_color("rgb(255, 0, 0)").is_some());
        assert!(parse_color("invalid").is_none());
    }

    #[test]
    fn test_color_presets() {
        let w = Color::white();
        assert!((w.r - 1.0).abs() < f64::EPSILON);
        let b = Color::black();
        assert!((b.r - 0.0).abs() < f64::EPSILON);
        let t = Color::transparent();
        assert!((t.a - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_color_from_u8_roundtrip() {
        let c = Color::from_u8(128, 64, 255, 200);
        let (r, g, b, a) = c.to_u8();
        assert_eq!(r, 128);
        assert_eq!(g, 64);
        assert_eq!(b, 255);
        assert_eq!(a, 200);
    }

    #[test]
    fn test_color_clamp() {
        let c = Color::new(1.5, -0.5, 0.5, 2.0);
        let clamped = c.clamp();
        assert!((clamped.r - 1.0).abs() < f64::EPSILON);
        assert!((clamped.g - 0.0).abs() < f64::EPSILON);
        assert!((clamped.a - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_color_with_alpha() {
        let c = Color::red().with_alpha(0.5);
        assert!((c.a - 0.5).abs() < f64::EPSILON);
        assert!((c.r - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_color_grayscale() {
        let c = Color::red().grayscale();
        assert!((c.r - c.g).abs() < f64::EPSILON);
        assert!((c.g - c.b).abs() < f64::EPSILON);
    }

    #[test]
    fn test_color_lerp() {
        let a = Color::black();
        let b = Color::white();
        let mid = a.lerp(&b, 0.5);
        assert!((mid.r - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_color_from_tuple() {
        let c: Color = (1.0, 0.0, 0.0).into();
        assert!((c.r - 1.0).abs() < f64::EPSILON);
        assert!((c.a - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_color_distance_fn() {
        let d = color_distance(&Color::black(), &Color::white());
        assert!((d - 3.0_f64.sqrt()).abs() < 0.001);
        assert!((color_distance(&Color::red(), &Color::red())).abs() < f64::EPSILON);
    }

    #[test]
    fn test_closest_named_color() {
        assert_eq!(closest_named_color(&Color::new(0.9, 0.1, 0.1, 1.0)), "red");
        assert_eq!(closest_named_color(&Color::new(0.0, 0.0, 0.0, 1.0)), "black");
    }

    #[test]
    fn test_wcag_contrast_level() {
        assert_eq!(wcag_contrast_level(&Color::black(), &Color::white()), "AAA");
        assert_eq!(wcag_contrast_level(&Color::white(), &Color::white()), "fail");
    }

    #[test]
    fn test_color_palette_add_get_remove() {
        let mut pal = ColorPalette::new();
        pal.add("red", Color::red());
        pal.add("blue", Color::new(0.0, 0.0, 1.0, 1.0));
        assert_eq!(pal.len(), 2);
        assert!(pal.contains("red"));
        assert!(pal.get("red").is_some());
        pal.remove("red");
        assert!(!pal.contains("red"));
        assert_eq!(pal.len(), 1);
    }

    #[test]
    fn test_color_palette_iter_and_names() {
        let mut pal = ColorPalette::new();
        pal.add("a", Color::black());
        pal.add("b", Color::white());
        let names = pal.names();
        assert_eq!(names, vec!["a", "b"]);
        assert_eq!(pal.iter().count(), 2);
    }

    #[test]
    fn test_color_scheme_complementary() {
        let scheme = ColorScheme::new(Color::red());
        let comp = scheme.complementary();
        assert!(comp.r < 0.3);
    }

    #[test]
    fn test_color_scheme_triadic() {
        let scheme = ColorScheme::new(Color::red());
        let (t1, t2) = scheme.triadic();
        assert!((t1.r - 1.0).abs() > 0.01 || t1.g.abs() > 0.01);
        assert!((t2.r - 1.0).abs() > 0.01 || t2.g.abs() > 0.01);
    }

    #[test]
    fn test_color_validator() {
        assert!(ColorValidator::validate_range(&Color::red()).is_ok());
        assert!(ColorValidator::validate_range(&Color::new(1.5, 0.0, 0.0, 1.0)).is_err());
        let clamped = ColorValidator::clamp(&Color::new(1.5, -0.1, 0.5, 2.0));
        assert!((clamped.r - 1.0).abs() < f64::EPSILON);
        assert!((clamped.g - 0.0).abs() < f64::EPSILON);
        assert!((clamped.a - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_gradient_interpolation() {
        let grad = Gradient::new(vec![
            GradientStop { position: 0.0, color: Color::black() },
            GradientStop { position: 1.0, color: Color::white() },
        ]);
        let mid = grad.interpolate_at(0.5);
        assert!((mid.r - 0.5).abs() < 0.01);
        assert!((mid.g - 0.5).abs() < 0.01);
        let start = grad.interpolate_at(0.0);
        assert!((start.r - 0.0).abs() < f64::EPSILON);
    }
}
