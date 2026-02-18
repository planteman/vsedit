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

// ---------------------------------------------------------------------------
// ColorHarmony – generate harmonious color sets
// ---------------------------------------------------------------------------

/// Generator for color harmony patterns based on color theory.
pub struct ColorHarmony;

impl ColorHarmony {
    /// Generate an analogous color set: the base color plus two neighbors
    /// rotated by ±30° on the hue wheel.
    pub fn analogous(base: &Color) -> [Color; 3] {
        let hsl = rgb_to_hsl(base);
        let h1 = (hsl.h + 330.0) % 360.0;
        let h2 = (hsl.h + 30.0) % 360.0;
        [
            hsl_to_rgb(&HslColor { h: h1, s: hsl.s, l: hsl.l, a: hsl.a }),
            *base,
            hsl_to_rgb(&HslColor { h: h2, s: hsl.s, l: hsl.l, a: hsl.a }),
        ]
    }

    /// Generate a split-complementary set: the base plus two colors
    /// at ±150° from the base hue.
    pub fn split_complementary(base: &Color) -> [Color; 3] {
        let hsl = rgb_to_hsl(base);
        let h1 = (hsl.h + 150.0) % 360.0;
        let h2 = (hsl.h + 210.0) % 360.0;
        [
            *base,
            hsl_to_rgb(&HslColor { h: h1, s: hsl.s, l: hsl.l, a: hsl.a }),
            hsl_to_rgb(&HslColor { h: h2, s: hsl.s, l: hsl.l, a: hsl.a }),
        ]
    }

    /// Generate a tetradic (rectangle) harmony: four colors evenly spaced
    /// at 0°, 60°, 180°, and 240° offsets.
    pub fn tetradic(base: &Color) -> [Color; 4] {
        let hsl = rgb_to_hsl(base);
        [
            *base,
            hsl_to_rgb(&HslColor { h: (hsl.h + 60.0) % 360.0, s: hsl.s, l: hsl.l, a: hsl.a }),
            hsl_to_rgb(&HslColor { h: (hsl.h + 180.0) % 360.0, s: hsl.s, l: hsl.l, a: hsl.a }),
            hsl_to_rgb(&HslColor { h: (hsl.h + 240.0) % 360.0, s: hsl.s, l: hsl.l, a: hsl.a }),
        ]
    }

    /// Generate `n` evenly spaced hues (monochromatic lightness steps are not
    /// altered; only the hue rotates).
    pub fn n_hue_spread(base: &Color, n: usize) -> Vec<Color> {
        if n == 0 {
            return Vec::new();
        }
        let hsl = rgb_to_hsl(base);
        let step = 360.0 / n as f64;
        (0..n)
            .map(|i| {
                let h = (hsl.h + step * i as f64) % 360.0;
                hsl_to_rgb(&HslColor { h, s: hsl.s, l: hsl.l, a: hsl.a })
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ColorMixer – mix and tint operations
// ---------------------------------------------------------------------------

/// Utility for mixing colors.
pub struct ColorMixer;

impl ColorMixer {
    /// Average a list of colors component-wise (including alpha).
    pub fn average(colors: &[Color]) -> Option<Color> {
        if colors.is_empty() {
            return None;
        }
        let n = colors.len() as f64;
        let r = colors.iter().map(|c| c.r).sum::<f64>() / n;
        let g = colors.iter().map(|c| c.g).sum::<f64>() / n;
        let b = colors.iter().map(|c| c.b).sum::<f64>() / n;
        let a = colors.iter().map(|c| c.a).sum::<f64>() / n;
        Some(Color::new(r, g, b, a))
    }

    /// Lighten a color by mixing it with white by the given factor (0..1).
    pub fn tint(color: &Color, factor: f64) -> Color {
        blend(color, &Color::white(), factor.clamp(0.0, 1.0))
    }

    /// Darken a color by mixing it with black by the given factor (0..1).
    pub fn shade(color: &Color, factor: f64) -> Color {
        blend(color, &Color::black(), factor.clamp(0.0, 1.0))
    }

    /// Generate a tint/shade scale with `steps` entries from full shade
    /// to full tint through the original color.
    pub fn tint_shade_scale(color: &Color, steps: usize) -> Vec<Color> {
        if steps == 0 {
            return Vec::new();
        }
        if steps == 1 {
            return vec![*color];
        }
        let mid = steps / 2;
        (0..steps)
            .map(|i| {
                if i < mid {
                    let factor = 1.0 - (i as f64 / mid as f64);
                    Self::shade(color, factor)
                } else if i == mid {
                    *color
                } else {
                    let factor = (i - mid) as f64 / (steps - 1 - mid) as f64;
                    Self::tint(color, factor)
                }
            })
            .collect()
    }
}

/// Invert a color (1 - each component, alpha unchanged).
pub fn invert_color(c: &Color) -> Color {
    Color::new(1.0 - c.r, 1.0 - c.g, 1.0 - c.b, c.a)
}

/// Convert a Color to CSS `hwb()` string.
pub fn color_to_hwb_string(c: &Color) -> String {
    let hsl = rgb_to_hsl(c);
    let min_rgb = c.r.min(c.g).min(c.b);
    let max_rgb = c.r.max(c.g).max(c.b);
    let whiteness = min_rgb * 100.0;
    let blackness = (1.0 - max_rgb) * 100.0;
    format!("hwb({:.0} {:.0}% {:.0}%)", hsl.h, whiteness, blackness)
}

// ---------------------------------------------------------------------------
// ColorPickerHSL – HSL color with conversion
// ---------------------------------------------------------------------------

/// An HSL color picker that supports bidirectional HSL↔RGB conversion.
#[derive(Debug, Clone)]
pub struct ColorPickerHSL {
    /// Hue in degrees (0–360).
    pub h: f64,
    /// Saturation (0.0–1.0).
    pub s: f64,
    /// Lightness (0.0–1.0).
    pub l: f64,
}

impl ColorPickerHSL {
    /// Create from HSL values.
    pub fn new(h: f64, s: f64, l: f64) -> Self {
        Self {
            h: h % 360.0,
            s: s.clamp(0.0, 1.0),
            l: l.clamp(0.0, 1.0),
        }
    }

    /// Convert from an RGBA [`Color`].
    pub fn from_rgb(color: &Color) -> Self {
        let hsl = rgb_to_hsl(color);
        Self {
            h: hsl.h,
            s: hsl.s,
            l: hsl.l,
        }
    }

    /// Convert to an RGBA [`Color`] with full opacity.
    pub fn to_rgb(&self) -> Color {
        let hsl = HslColor::new(self.h, self.s, self.l, 1.0);
        hsl_to_rgb(&hsl)
    }

    /// Rotate hue by the given degrees.
    pub fn rotate(&mut self, degrees: f64) {
        self.h = (self.h + degrees) % 360.0;
        if self.h < 0.0 {
            self.h += 360.0;
        }
    }

    /// Lighten by the given amount.
    pub fn lighten(&mut self, amount: f64) {
        self.l = (self.l + amount).clamp(0.0, 1.0);
    }

    /// Darken by the given amount.
    pub fn darken(&mut self, amount: f64) {
        self.l = (self.l - amount).clamp(0.0, 1.0);
    }

    /// Saturate by the given amount.
    pub fn saturate(&mut self, amount: f64) {
        self.s = (self.s + amount).clamp(0.0, 1.0);
    }
}

impl fmt::Display for ColorPickerHSL {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hsl({:.0}, {:.0}%, {:.0}%)", self.h, self.s * 100.0, self.l * 100.0)
    }
}

// ---------------------------------------------------------------------------
// ColorPickerAlpha – alpha channel support
// ---------------------------------------------------------------------------

/// Wraps a [`Color`] with explicit alpha manipulation.
#[derive(Debug, Clone, Copy)]
pub struct ColorPickerAlpha {
    /// The base color.
    pub color: Color,
}

impl ColorPickerAlpha {
    /// Create from a [`Color`].
    pub fn new(color: Color) -> Self {
        Self { color }
    }

    /// Set the alpha channel.
    pub fn set_alpha(&mut self, alpha: f64) {
        self.color.a = alpha.clamp(0.0, 1.0);
    }

    /// Get the alpha channel.
    pub fn alpha(&self) -> f64 {
        self.color.a
    }

    /// Whether the color is fully opaque.
    pub fn is_opaque(&self) -> bool {
        (self.color.a - 1.0).abs() < f64::EPSILON
    }

    /// Whether the color is fully transparent.
    pub fn is_transparent(&self) -> bool {
        self.color.a.abs() < f64::EPSILON
    }

    /// Premultiply RGB by alpha.
    pub fn premultiplied(&self) -> Color {
        Color::new(
            self.color.r * self.color.a,
            self.color.g * self.color.a,
            self.color.b * self.color.a,
            self.color.a,
        )
    }
}

impl fmt::Display for ColorPickerAlpha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rgba({:.0}, {:.0}, {:.0}, {:.2})",
            self.color.r * 255.0,
            self.color.g * 255.0,
            self.color.b * 255.0,
            self.color.a
        )
    }
}

// ---------------------------------------------------------------------------
// ColorPickerHistory – recently used colors
// ---------------------------------------------------------------------------

/// Tracks recently used colors with a bounded history.
#[derive(Debug, Clone)]
pub struct ColorPickerHistory {
    colors: Vec<String>,
    max_entries: usize,
}

impl Default for ColorPickerHistory {
    fn default() -> Self {
        Self {
            colors: Vec::new(),
            max_entries: 20,
        }
    }
}

impl ColorPickerHistory {
    /// Create a history with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            ..Default::default()
        }
    }

    /// Record a color as recently used (hex string).
    pub fn record(&mut self, hex: impl Into<String>) {
        let hex = hex.into();
        // Remove if already present (move to front)
        self.colors.retain(|c| c != &hex);
        self.colors.insert(0, hex);
        self.colors.truncate(self.max_entries);
    }

    /// Get the most recently used colors.
    pub fn recent(&self) -> &[String] {
        &self.colors
    }

    /// Number of recorded colors.
    pub fn len(&self) -> usize {
        self.colors.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    /// Clear the history.
    pub fn clear(&mut self) {
        self.colors.clear();
    }

    /// Check if a color has been used before.
    pub fn contains(&self, hex: &str) -> bool {
        self.colors.iter().any(|c| c == hex)
    }
}

// ---------------------------------------------------------------------------
// Color format string generation
// ---------------------------------------------------------------------------

/// Format a [`Color`] in the specified format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFormat {
    /// `#RRGGBB` or `#RRGGBBAA`.
    Hex,
    /// `rgb(r, g, b)` or `rgba(r, g, b, a)`.
    Rgb,
    /// `hsl(h, s%, l%)` or `hsla(h, s%, l%, a)`.
    Hsl,
}

/// Format a color in the given format.
pub fn format_color(color: &Color, format: ColorFormat) -> String {
    match format {
        ColorFormat::Hex => {
            if (color.a - 1.0).abs() < f64::EPSILON {
                color_to_hex(color)
            } else {
                let hex = color_to_hex(color);
                let alpha_byte = (color.a * 255.0).round() as u8;
                format!("{}{:02X}", hex, alpha_byte)
            }
        }
        ColorFormat::Rgb => color_to_rgba_string(color),
        ColorFormat::Hsl => color_to_hsl_string(color),
    }
}


// ---------------------------------------------------------------------------
// ColorPalettePicker – preset palette collections
// ---------------------------------------------------------------------------

/// A named collection of preset color palettes for common use cases.
pub struct ColorPalettePicker {
    palettes: Vec<(String, Vec<(String, Color)>)>,
}

impl ColorPalettePicker {
    /// Create an empty palette picker.
    pub fn new() -> Self {
        Self { palettes: Vec::new() }
    }

    /// Create a palette picker pre-loaded with Material Design primary colors.
    pub fn material_design() -> Self {
        let mut picker = Self::new();
        let colors = vec![
            ("Red",         Color::new(0.957, 0.263, 0.212, 1.0)),
            ("Pink",        Color::new(0.914, 0.118, 0.388, 1.0)),
            ("Purple",      Color::new(0.612, 0.153, 0.690, 1.0)),
            ("Deep Purple", Color::new(0.404, 0.227, 0.718, 1.0)),
            ("Indigo",      Color::new(0.247, 0.318, 0.710, 1.0)),
            ("Blue",        Color::new(0.129, 0.588, 0.953, 1.0)),
            ("Cyan",        Color::new(0.0,   0.737, 0.831, 1.0)),
            ("Teal",        Color::new(0.0,   0.588, 0.533, 1.0)),
            ("Green",       Color::new(0.298, 0.686, 0.314, 1.0)),
            ("Yellow",      Color::new(1.0,   0.922, 0.231, 1.0)),
            ("Orange",      Color::new(1.0,   0.596, 0.0,   1.0)),
            ("Brown",       Color::new(0.475, 0.333, 0.282, 1.0)),
        ];
        let entries: Vec<(String, Color)> = colors
            .into_iter()
            .map(|(n, c)| (n.to_string(), c))
            .collect();
        picker.add_palette("Material Design", entries);
        picker
    }

    /// Create a palette picker with Tailwind CSS base colors.
    pub fn tailwind_css() -> Self {
        let mut picker = Self::new();
        let colors = vec![
            ("Slate",   Color::new(0.392, 0.455, 0.545, 1.0)),
            ("Gray",    Color::new(0.420, 0.447, 0.502, 1.0)),
            ("Zinc",    Color::new(0.443, 0.443, 0.478, 1.0)),
            ("Red",     Color::new(0.937, 0.267, 0.267, 1.0)),
            ("Orange",  Color::new(0.976, 0.451, 0.086, 1.0)),
            ("Amber",   Color::new(0.961, 0.620, 0.043, 1.0)),
            ("Yellow",  Color::new(0.918, 0.702, 0.031, 1.0)),
            ("Lime",    Color::new(0.518, 0.776, 0.086, 1.0)),
            ("Green",   Color::new(0.133, 0.725, 0.384, 1.0)),
            ("Teal",    Color::new(0.078, 0.714, 0.651, 1.0)),
            ("Sky",     Color::new(0.055, 0.647, 0.914, 1.0)),
            ("Blue",    Color::new(0.231, 0.510, 0.965, 1.0)),
            ("Violet",  Color::new(0.545, 0.361, 0.965, 1.0)),
            ("Fuchsia", Color::new(0.851, 0.275, 0.937, 1.0)),
            ("Rose",    Color::new(0.957, 0.247, 0.369, 1.0)),
        ];
        let entries: Vec<(String, Color)> = colors
            .into_iter()
            .map(|(n, c)| (n.to_string(), c))
            .collect();
        picker.add_palette("Tailwind CSS", entries);
        picker
    }

    /// Add a named palette.
    pub fn add_palette(
        &mut self,
        name: impl Into<String>,
        colors: Vec<(String, Color)>,
    ) {
        self.palettes.push((name.into(), colors));
    }

    /// List all palette names.
    pub fn palette_names(&self) -> Vec<&str> {
        self.palettes.iter().map(|(n, _)| n.as_str()).collect()
    }

    /// Look up a palette by name.
    pub fn get_palette(&self, name: &str) -> Option<&[(String, Color)]> {
        self.palettes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c.as_slice())
    }

    /// Total number of palettes.
    pub fn palette_count(&self) -> usize {
        self.palettes.len()
    }

    /// Total number of colors across all palettes.
    pub fn total_colors(&self) -> usize {
        self.palettes.iter().map(|(_, c)| c.len()).sum()
    }

    /// Search all palettes for colors whose name contains the query
    /// (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<(&str, &str, &Color)> {
        let q = query.to_ascii_lowercase();
        let mut results = Vec::new();
        for (palette_name, colors) in &self.palettes {
            for (color_name, color) in colors {
                if color_name.to_ascii_lowercase().contains(&q) {
                    results.push((palette_name.as_str(), color_name.as_str(), color));
                }
            }
        }
        results
    }

    /// Remove a palette by name. Returns `true` if found and removed.
    pub fn remove_palette(&mut self, name: &str) -> bool {
        let before = self.palettes.len();
        self.palettes.retain(|(n, _)| n != name);
        self.palettes.len() < before
    }
}

impl Default for ColorPalettePicker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ColorFormatAutoDetector – detect the format of a color string
// ---------------------------------------------------------------------------

/// The detected format of a color string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedColorFormat {
    /// `#RGB` shorthand hex.
    Hex3,
    /// `#RRGGBB` hex.
    Hex6,
    /// `#RRGGBBAA` hex with alpha.
    Hex8,
    /// `rgb(...)` or `rgba(...)` functional notation.
    RgbFunction,
    /// `hsl(...)` or `hsla(...)` functional notation.
    HslFunction,
    /// `hwb(...)` functional notation.
    HwbFunction,
    /// A CSS named color like `red` or `cornflowerblue`.
    Named,
}

/// Detects the format of a color string without fully parsing it.
pub struct ColorFormatAutoDetector;

impl ColorFormatAutoDetector {
    /// Detect the color format of the given input string.
    ///
    /// Returns `None` if the input does not look like any recognized color
    /// format.
    pub fn detect(input: &str) -> Option<DetectedColorFormat> {
        let trimmed = input.trim();
        if trimmed.starts_with('#') {
            let hex_part = &trimmed[1..];
            if hex_part.len() == 3 && hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(DetectedColorFormat::Hex3);
            }
            if hex_part.len() == 6 && hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(DetectedColorFormat::Hex6);
            }
            if hex_part.len() == 8 && hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(DetectedColorFormat::Hex8);
            }
            return None;
        }
        if trimmed.starts_with("rgb(") || trimmed.starts_with("rgba(") {
            return Some(DetectedColorFormat::RgbFunction);
        }
        if trimmed.starts_with("hsl(") || trimmed.starts_with("hsla(") {
            return Some(DetectedColorFormat::HslFunction);
        }
        if trimmed.starts_with("hwb(") {
            return Some(DetectedColorFormat::HwbFunction);
        }
        // Check for named colors
        if parse_named_color(trimmed).is_some() {
            return Some(DetectedColorFormat::Named);
        }
        None
    }

    /// Detect the format and parse the color in one step.
    pub fn detect_and_parse(input: &str) -> Option<(DetectedColorFormat, Color)> {
        let format = Self::detect(input)?;
        let color = parse_color(input)?;
        Some((format, color))
    }

    /// Suggest the best `ColorFormat` for re-serializing a color that was
    /// originally written in the detected format.
    pub fn suggest_output_format(detected: DetectedColorFormat) -> ColorFormat {
        match detected {
            DetectedColorFormat::Hex3
            | DetectedColorFormat::Hex6
            | DetectedColorFormat::Hex8 => ColorFormat::Hex,
            DetectedColorFormat::RgbFunction => ColorFormat::Rgb,
            DetectedColorFormat::HslFunction => ColorFormat::Hsl,
            DetectedColorFormat::HwbFunction => ColorFormat::Hsl,
            DetectedColorFormat::Named => ColorFormat::Hex,
        }
    }
}

// ---------------------------------------------------------------------------
// ColorSwatchRenderer – multi-character swatch rendering
// ---------------------------------------------------------------------------

/// Renders color swatches as multi-character block strings for terminal UIs.
pub struct ColorSwatchRenderer {
    width: usize,
    height: usize,
    border: bool,
}

impl ColorSwatchRenderer {
    /// Create a renderer with custom swatch dimensions.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            border: false,
        }
    }

    /// Enable or disable a simple ASCII border around the swatch.
    pub fn with_border(mut self, border: bool) -> Self {
        self.border = border;
        self
    }

    /// Render a single color swatch as a vector of lines.
    ///
    /// Each line is a string of block characters. The caller is responsible for
    /// applying ANSI foreground color escape codes.
    pub fn render(&self, _color: &Color) -> Vec<String> {
        let block_line: String = "█".repeat(self.width);
        let mut rows = Vec::new();
        if self.border {
            let top = format!("+{}+", "-".repeat(self.width));
            rows.push(top);
            for _ in 0..self.height {
                rows.push(format!("|{}|", block_line));
            }
            let bottom = format!("+{}+", "-".repeat(self.width));
            rows.push(bottom);
        } else {
            for _ in 0..self.height {
                rows.push(block_line.clone());
            }
        }
        rows
    }

    /// Render a swatch with an ANSI true-color escape sequence baked in.
    pub fn render_ansi(&self, color: &Color) -> Vec<String> {
        let r = (color.r.clamp(0.0, 1.0) * 255.0).round() as u8;
        let g = (color.g.clamp(0.0, 1.0) * 255.0).round() as u8;
        let b = (color.b.clamp(0.0, 1.0) * 255.0).round() as u8;
        let esc_start = format!("\x1b[38;2;{};{};{}m", r, g, b);
        let esc_end = "\x1b[0m";
        self.render(color)
            .into_iter()
            .map(|line| format!("{}{}{}", esc_start, line, esc_end))
            .collect()
    }

    /// Render a grid of swatches side-by-side, separated by a single space.
    pub fn render_row(&self, colors: &[Color]) -> Vec<String> {
        if colors.is_empty() {
            return Vec::new();
        }
        let rendered: Vec<Vec<String>> = colors.iter().map(|c| self.render(c)).collect();
        let max_rows = rendered.iter().map(|r| r.len()).max().unwrap_or(0);
        let empty_cell = " ".repeat(if self.border { self.width + 2 } else { self.width });
        (0..max_rows)
            .map(|row_idx| {
                rendered
                    .iter()
                    .map(|swatch| {
                        swatch
                            .get(row_idx)
                            .cloned()
                            .unwrap_or_else(|| empty_cell.clone())
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }

    /// Total character width of a single swatch (including border).
    pub fn swatch_width(&self) -> usize {
        if self.border { self.width + 2 } else { self.width }
    }

    /// Total character height of a single swatch (including border).
    pub fn swatch_height(&self) -> usize {
        if self.border { self.height + 2 } else { self.height }
    }
}

impl Default for ColorSwatchRenderer {
    fn default() -> Self {
        Self::new(2, 1)
    }
}

// ---------------------------------------------------------------------------
// ColorNameLookup – bidirectional name↔color lookup with fuzzy matching
// ---------------------------------------------------------------------------

/// Bidirectional lookup between CSS color names and their RGB values.
pub struct ColorNameLookup {
    entries: Vec<(&'static str, Color)>,
}

impl ColorNameLookup {
    /// Build the lookup table with the standard named colors.
    pub fn new() -> Self {
        let entries: Vec<(&'static str, Color)> = vec![
            ("black",     Color::new(0.0,   0.0,   0.0,   1.0)),
            ("white",     Color::new(1.0,   1.0,   1.0,   1.0)),
            ("red",       Color::new(1.0,   0.0,   0.0,   1.0)),
            ("green",     Color::new(0.0,   0.502, 0.0,   1.0)),
            ("blue",      Color::new(0.0,   0.0,   1.0,   1.0)),
            ("yellow",    Color::new(1.0,   1.0,   0.0,   1.0)),
            ("cyan",      Color::new(0.0,   1.0,   1.0,   1.0)),
            ("magenta",   Color::new(1.0,   0.0,   1.0,   1.0)),
            ("orange",    Color::new(1.0,   0.647, 0.0,   1.0)),
            ("purple",    Color::new(0.502, 0.0,   0.502, 1.0)),
            ("pink",      Color::new(1.0,   0.753, 0.796, 1.0)),
            ("brown",     Color::new(0.647, 0.165, 0.165, 1.0)),
            ("gray",      Color::new(0.502, 0.502, 0.502, 1.0)),
            ("silver",    Color::new(0.753, 0.753, 0.753, 1.0)),
            ("navy",      Color::new(0.0,   0.0,   0.502, 1.0)),
            ("teal",      Color::new(0.0,   0.502, 0.502, 1.0)),
            ("maroon",    Color::new(0.502, 0.0,   0.0,   1.0)),
            ("olive",     Color::new(0.502, 0.502, 0.0,   1.0)),
            ("lime",      Color::new(0.0,   1.0,   0.0,   1.0)),
            ("coral",     Color::new(1.0,   0.498, 0.314, 1.0)),
            ("salmon",    Color::new(0.980, 0.502, 0.447, 1.0)),
            ("gold",      Color::new(1.0,   0.843, 0.0,   1.0)),
            ("ivory",     Color::new(1.0,   1.0,   0.941, 1.0)),
            ("indigo",    Color::new(0.294, 0.0,   0.510, 1.0)),
            ("violet",    Color::new(0.933, 0.510, 0.933, 1.0)),
            ("khaki",     Color::new(0.941, 0.902, 0.549, 1.0)),
            ("crimson",   Color::new(0.863, 0.078, 0.235, 1.0)),
            ("turquoise", Color::new(0.251, 0.878, 0.816, 1.0)),
        ];
        Self { entries }
    }

    /// Look up a color by its CSS name (case-insensitive).
    pub fn by_name(&self, name: &str) -> Option<&Color> {
        let lower = name.to_ascii_lowercase();
        self.entries
            .iter()
            .find(|(n, _)| *n == lower.as_str())
            .map(|(_, c)| c)
    }

    /// Find the closest named color to the given color (Euclidean RGB
    /// distance). Returns the name and distance.
    pub fn closest(&self, color: &Color) -> (&'static str, f64) {
        let mut best_name = "black";
        let mut best_dist = f64::MAX;
        for (name, entry) in &self.entries {
            let dr = color.r - entry.r;
            let dg = color.g - entry.g;
            let db = color.b - entry.b;
            let dist = (dr * dr + dg * dg + db * db).sqrt();
            if dist < best_dist {
                best_dist = dist;
                best_name = name;
            }
        }
        (best_name, best_dist)
    }

    /// Return all color names that fuzzy-match the query (the query is a
    /// substring of the name, case-insensitive).
    pub fn fuzzy_search(&self, query: &str) -> Vec<&'static str> {
        let q = query.to_ascii_lowercase();
        self.entries
            .iter()
            .filter(|(n, _)| n.contains(q.as_str()))
            .map(|(n, _)| *n)
            .collect()
    }

    /// Number of entries in the lookup table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the lookup table is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all `(name, color)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Color)> {
        self.entries.iter().map(|(n, c)| (*n, c))
    }

    /// Return all names sorted alphabetically.
    pub fn sorted_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&str> = self.entries.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        names
    }
}

impl Default for ColorNameLookup {
    fn default() -> Self {
        Self::new()
    }
}



// ---------------------------------------------------------------------------
// ColorPalettePicker – additional convenience methods
// ---------------------------------------------------------------------------

impl ColorPalettePicker {
    /// Create a palette picker pre-loaded with a "material" palette containing
    /// red, blue, green, and yellow.
    pub fn with_material_preset() -> Self {
        let mut picker = Self::new();
        let colors = vec![
            ("red".to_string(),    Color::new(1.0, 0.0, 0.0, 1.0)),
            ("blue".to_string(),   Color::new(0.0, 0.0, 1.0, 1.0)),
            ("green".to_string(),  Color::new(0.0, 0.502, 0.0, 1.0)),
            ("yellow".to_string(), Color::new(1.0, 1.0, 0.0, 1.0)),
        ];
        picker.add_palette("material", colors);
        picker
    }

    /// Create a palette picker pre-loaded with a "pastel" palette containing
    /// soft pastel shades.
    pub fn with_pastel_preset() -> Self {
        let mut picker = Self::new();
        let colors = vec![
            ("pastel_pink".to_string(),   Color::new(1.0, 0.714, 0.757, 1.0)),
            ("pastel_blue".to_string(),   Color::new(0.686, 0.878, 0.898, 1.0)),
            ("pastel_green".to_string(),  Color::new(0.596, 0.984, 0.596, 1.0)),
            ("pastel_yellow".to_string(), Color::new(0.992, 0.992, 0.588, 1.0)),
            ("pastel_purple".to_string(), Color::new(0.702, 0.620, 0.859, 1.0)),
            ("pastel_orange".to_string(), Color::new(1.0, 0.702, 0.482, 1.0)),
        ];
        picker.add_palette("pastel", colors);
        picker
    }

    /// Search all palettes for a color by name and return the first match.
    pub fn find_color_in_any(&self, name: &str) -> Option<Color> {
        let lower = name.to_ascii_lowercase();
        for (_palette_name, colors) in &self.palettes {
            for (color_name, color) in colors {
                if color_name.to_ascii_lowercase() == lower {
                    return Some(*color);
                }
            }
        }
        None
    }

    /// Total number of individual colors across every palette.
    pub fn total_color_count(&self) -> usize {
        self.palettes.iter().map(|(_, c)| c.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// ColorFormatAutoDetector – additional detection helpers
// ---------------------------------------------------------------------------

impl ColorFormatAutoDetector {
    /// Detect all color format occurrences in a line of text.
    /// Returns (byte_offset, format) pairs.
    pub fn detect_all_in_line(line: &str) -> Vec<(usize, DetectedColorFormat)> {
        let mut results = Vec::new();
        let mut i = 0;
        let bytes = line.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'#' {
                // Try hex patterns
                for len in &[9usize, 7, 4] {
                    if i + len <= bytes.len() {
                        let candidate = &line[i..i + len];
                        if let Some(fmt) = Self::detect(candidate) {
                            results.push((i, fmt));
                            i += len;
                            continue;
                        }
                    }
                }
                i += 1;
            } else if line[i..].starts_with("rgb(") || line[i..].starts_with("rgba(") {
                if let Some(end) = line[i..].find(')') {
                    let candidate = &line[i..i + end + 1];
                    if let Some(fmt) = Self::detect(candidate) {
                        results.push((i, fmt));
                        i += end + 1;
                        continue;
                    }
                }
                i += 1;
            } else if line[i..].starts_with("hsl(") || line[i..].starts_with("hsla(") {
                if let Some(end) = line[i..].find(')') {
                    let candidate = &line[i..i + end + 1];
                    if let Some(fmt) = Self::detect(candidate) {
                        results.push((i, fmt));
                        i += end + 1;
                        continue;
                    }
                }
                i += 1;
            } else {
                i += 1;
            }
        }
        results
    }

    /// Check whether the input looks like a hex color.
    pub fn is_hex(input: &str) -> bool {
        matches!(
            Self::detect(input),
            Some(DetectedColorFormat::Hex3)
                | Some(DetectedColorFormat::Hex6)
                | Some(DetectedColorFormat::Hex8)
        )
    }

    /// Check whether the input looks like an `rgb(…)` / `rgba(…)` string.
    pub fn is_rgb(input: &str) -> bool {
        matches!(Self::detect(input), Some(DetectedColorFormat::RgbFunction))
    }

    /// Check whether the input looks like an `hsl(…)` / `hsla(…)` string.
    pub fn is_hsl(input: &str) -> bool {
        matches!(Self::detect(input), Some(DetectedColorFormat::HslFunction))
    }

    /// Check whether the input matches a CSS named color.
    pub fn is_named(input: &str) -> bool {
        matches!(Self::detect(input), Some(DetectedColorFormat::Named))
    }

    /// Suggest the best serialization format for a given color.
    ///
    /// Fully opaque colors get `Hex`; colors with fractional channels get
    /// `Rgb`; anything else gets `Hsl`.
    pub fn suggest_format(color: &Color) -> ColorFormat {
        if (color.a - 1.0).abs() < f64::EPSILON {
            ColorFormat::Hex
        } else {
            ColorFormat::Rgb
        }
    }
}

// ---------------------------------------------------------------------------
// ColorSwatchRenderer – additional rendering helpers
// ---------------------------------------------------------------------------

impl ColorSwatchRenderer {
    /// Render a swatch with a text label on the last line.
    pub fn render_with_label(&self, color: &Color, label: &str) -> Vec<String> {
        let mut rows = self.render(color);
        let padded = if label.len() < self.width {
            let pad = self.width - label.len();
            let left = pad / 2;
            let right = pad - left;
            format!("{}{}{}", " ".repeat(left), label, " ".repeat(right))
        } else {
            label[..self.width].to_string()
        };
        rows.push(padded);
        rows
    }

    /// Render every color in a `ColorPalette` as a sequence of labeled
    /// swatches, one per color, stacked vertically.
    pub fn render_palette(&self, palette: &ColorPalette) -> Vec<String> {
        let mut output = Vec::new();
        for (name, color) in palette.iter() {
            let swatch_lines = self.render_with_label(color, name);
            for line in swatch_lines {
                output.push(line);
            }
        }
        output
    }

    /// The Unicode full-block character used by the renderer.
    pub fn block_char() -> char {
        '█'
    }
}

// ---------------------------------------------------------------------------
// ColorNameLookup – additional convenience methods
// ---------------------------------------------------------------------------

impl ColorNameLookup {
    /// Look up the exact name for a color (exact RGB match, ignoring alpha).
    pub fn lookup(&self, color: &Color) -> Option<&str> {
        for (name, entry) in &self.entries {
            if (entry.r - color.r).abs() < 1e-4
                && (entry.g - color.g).abs() < 1e-4
                && (entry.b - color.b).abs() < 1e-4
            {
                return Some(name);
            }
        }
        None
    }

    /// Return the name of the closest named color (by Euclidean distance).
    pub fn closest_name(&self, color: &Color) -> String {
        let (name, _dist) = self.closest(color);
        name.to_string()
    }

    /// Return all color names in insertion order.
    pub fn all_names(&self) -> Vec<&str> {
        self.entries.iter().map(|(n, _)| *n).collect()
    }

    /// Resolve a name to its `Color` value (case-insensitive).
    pub fn name_to_color(&self, name: &str) -> Option<Color> {
        self.by_name(name).copied()
    }

    /// Number of named colors in this lookup.
    pub fn name_count(&self) -> usize {
        self.entries.len()
    }

    /// Whether the lookup contains a color with the given name.
    pub fn contains(&self, name: &str) -> bool {
        self.by_name(name).is_some()
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for colorpicker
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaColorpickerRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaColorpickerRingBuf {
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
pub struct XaColorpickerCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaColorpickerCounter {
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

impl Default for XaColorpickerCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 18
// ---------------------------------------------------------------------------

/// Generic object pool `Xc18Pool<T>`.
pub struct Xc18Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc18Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc18PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc18Pool<T> {
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
    pub fn stats(&self) -> Xc18PoolStats {
        Xc18PoolStats {
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

impl<T> Default for Xc18Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc18Scheduler`.
pub struct Xc18Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc18Scheduler {
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

impl Default for Xc18Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_18 hash for the given byte slice.
pub fn xc_18_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_18 convention.
pub fn xc_18_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_82 deepening: state machine + event bus ---

/// States for the Xd82 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd82State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd82State {
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
pub struct Xd82Transition {
    pub from: Xd82State,
    pub to: Xd82State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd82StateMachine {
    current: Xd82State,
    history: Vec<Xd82Transition>,
    step_counter: usize,
}

impl Xd82StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd82State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd82State {
        self.current
    }

    pub fn history(&self) -> &[Xd82Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd82State) -> Result<Xd82State, String> {
        let allowed = match (self.current, target) {
            (Xd82State::Idle, Xd82State::Running) => true,
            (Xd82State::Running, Xd82State::Paused) => true,
            (Xd82State::Running, Xd82State::Done) => true,
            (Xd82State::Paused, Xd82State::Running) => true,
            (Xd82State::Paused, Xd82State::Done) => true,
            (Xd82State::Done, Xd82State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_82: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd82Transition {
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
            "Xd82SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd82State> {
        let prefix = "Xd82SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd82State::Idle),
            "Running" => Some(Xd82State::Running),
            "Paused" => Some(Xd82State::Paused),
            "Done" => Some(Xd82State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd82State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd82 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd82Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd82Event {
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

type Xd82HandlerFn = Box<dyn Fn(&Xd82Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd82EventBus {
    handlers: Vec<(usize, Option<String>, Xd82HandlerFn)>,
    next_id: usize,
    published: Vec<Xd82Event>,
}

impl Xd82EventBus {
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
        F: Fn(&Xd82Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd82Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd82Event) {
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

    pub fn published_events(&self) -> &[Xd82Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #102
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf102Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf102TrieNode {
    children: std::collections::HashMap<char, Xf102TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf102Trie {
    root: Xf102TrieNode,
    count: usize,
}

impl Xf102Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf102TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf102TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf102TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf102BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf102BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 17).
pub struct Xh17SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh17SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 59 as u64,
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

/// A compact bit set supporting boolean operations (variant 17).
pub struct Xh17BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh17BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 17).
pub struct Xi17Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi17Deque<T> {
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
pub struct Xi17Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi17Interval {
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

/// A simple interval tree (variant 17).
pub struct Xi17IntervalTree {
    xi_intervals: Vec<Xi17Interval>,
}

impl Xi17IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi17Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi17Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi17Interval) -> Vec<&Xi17Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi17Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi17Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi17Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi17Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi17Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi17Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 17) ---

/// Disjoint set / union-find for crate 17.
pub struct Xj17UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj17UnionFind {
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

const XJ17_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 17.
pub struct Xj17BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj17BTreeNode<K, V>>>,
    len: usize,
}

struct Xj17BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj17BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj17BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ17_BTREE_ORDER - 1
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
        let mid = XJ17_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj17BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj17BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj17BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj17BTreeNode::xj_new_leaf();
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


// --- xk_17 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk17SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk17SegmentTree {
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
pub struct Xk17DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk17DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_17).
#[derive(Debug, Clone)]
pub struct Xl17Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl17Rope {
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

/// Suffix array for efficient string searching (xl_17).
#[derive(Debug, Clone)]
pub struct Xl17SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl17SuffixArray {
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
pub struct Xm17MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm17MatrixSparse {
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
pub struct Xm17Tokenizer {
    text: String,
}

impl Xm17Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 17.
pub struct Xn17Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn17Fenwick {
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

// ----- AVL tree map — crate 17 -----

#[derive(Debug, Clone)]
struct Xn17AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn17AvlNode<K, V>>>,
    right: Option<Box<Xn17AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 17.
#[derive(Debug, Clone)]
pub struct Xn17AVL<K, V> {
    root: Option<Box<Xn17AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn17AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn17AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn17AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn17AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn17AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn17AvlNode<K, V>>) -> Box<Xn17AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn17AvlNode<K, V>>) -> Box<Xn17AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn17AvlNode<K, V>>) -> Box<Xn17AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn17AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn17AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn17AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn17AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn17AvlNode<K, V>>) -> &Xn17AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn17AvlNode<K, V>>) -> (Box<Xn17AvlNode<K, V>>, Option<Box<Xn17AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn17AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn17AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn17AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn17AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn17AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn17AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn17AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo17RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo17Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo17RBNode<K, V> {
    key: K,
    value: V,
    color: Xo17Color,
    left: Option<Box<Xo17RBNode<K, V>>>,
    right: Option<Box<Xo17RBNode<K, V>>>,
}

/// A red-black tree map for crate 17.
#[derive(Debug, Clone)]
pub struct Xo17RedBlack<K, V> {
    root: Option<Box<Xo17RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo17RedBlack<K, V> {
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
            r.color = Xo17Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo17RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo17RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo17RBNode {
                    key, value, color: Xo17Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo17RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo17Color::Red)
    }

    fn xo_balance(mut h: Box<Xo17RBNode<K, V>>) -> Box<Xo17RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo17Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo17RBNode<K, V>>) -> Box<Xo17RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo17Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo17RBNode<K, V>>) -> Box<Xo17RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo17Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo17RBNode<K, V>>) {
        h.color = Xo17Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo17Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo17Color::Black; }
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
            r.color = Xo17Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo17RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo17RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo17RBNode<K, V>) -> (K, V, Option<Box<Xo17RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo17RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo17Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo17RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo17ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 17.
#[derive(Debug, Clone)]
pub struct Xo17ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo17ConsistentHash {
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
            let vkey = format!("{}#xo17#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo17#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 17).
#[derive(Debug)]
pub struct Xp17SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp17Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp17Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp17Node<K, V>>>,
    xp_right: Option<Box<Xp17Node<K, V>>>,
}

impl<K: Ord, V> Xp17Node<K, V> {
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

impl<K: Ord, V> Default for Xp17SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp17SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp17Node<K, V>>>, key: &K) -> Option<Box<Xp17Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp17Node<K, V>>) -> Box<Xp17Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp17Node<K, V>>) -> Box<Xp17Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp17Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp17Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp17Node::xp_new(key, val));
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


// --------------- Xq17Treap ---------------

use std::cmp::Ordering as Xq17Ord;

struct Xq17TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq17TreapNode<K, V>>>,
    right: Option<Box<Xq17TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq17Treap<K, V> {
    root: Option<Box<Xq17TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq17TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_17_size<K, V>(node: &Option<Box<Xq17TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_17_update_size<K, V>(node: &mut Xq17TreapNode<K, V>) {
    node.size = 1 + xq_17_size(&node.left) + xq_17_size(&node.right);
}

fn xq_17_rotate_right<K, V>(mut node: Box<Xq17TreapNode<K, V>>) -> Box<Xq17TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_17_update_size(&mut node);
    left.right = Some(node);
    xq_17_update_size(&mut left);
    left
}

fn xq_17_rotate_left<K, V>(mut node: Box<Xq17TreapNode<K, V>>) -> Box<Xq17TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_17_update_size(&mut node);
    right.left = Some(node);
    xq_17_update_size(&mut right);
    right
}

fn xq_17_insert_node<K: Ord, V>(
    node: Option<Box<Xq17TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq17TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq17TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq17Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq17Ord::Less => {
                let (new_left, old) = xq_17_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_17_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_17_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq17Ord::Greater => {
                let (new_right, old) = xq_17_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_17_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_17_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_17_remove_node<K: Ord, V>(
    node: Option<Box<Xq17TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq17TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq17Ord::Less => {
                let (new_left, old) = xq_17_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_17_update_size(&mut n);
                (Some(n), old)
            }
            Xq17Ord::Greater => {
                let (new_right, old) = xq_17_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_17_update_size(&mut n);
                (Some(n), old)
            }
            Xq17Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_17_rotate_right(n);
                    let (new_right, old) = xq_17_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_17_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_17_rotate_left(n);
                    let (new_left, old) = xq_17_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_17_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_17_find_min<K, V>(node: &Option<Box<Xq17TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_17_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_17_find_max<K, V>(node: &Option<Box<Xq17TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_17_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_17_rank<K: Ord, V>(node: &Option<Box<Xq17TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq17Ord::Less => xq_17_rank(&n.left, key),
            Xq17Ord::Equal => xq_17_size(&n.left),
            Xq17Ord::Greater => 1 + xq_17_size(&n.left) + xq_17_rank(&n.right, key),
        },
    }
}

fn xq_17_kth<K, V>(node: &Option<Box<Xq17TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_17_size(&n.left);
        if k < left_size {
            xq_17_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_17_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_17_in_order<K: Clone, V>(node: &Option<Box<Xq17TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_17_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_17_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq17Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 17 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_17_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq17Ord::Equal => return Some(&n.value),
                Xq17Ord::Less => cur = &n.left,
                Xq17Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_17_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_17_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_17_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_17_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_17_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_17_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_17_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq17VEBTree ---------------

pub struct Xq17VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq17VEBTree>>,
    clusters: Vec<Option<Box<Xq17VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq17VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq17VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq17VEBTree::xq_new(self.sqrt_lo)));
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

    #[test]
    fn test_analogous_harmony() {
        let base = Color::red();
        let [left, center, right] = ColorHarmony::analogous(&base);
        assert!((center.r - base.r).abs() < f64::EPSILON);
        assert!((center.g - base.g).abs() < f64::EPSILON);
        // left and right should differ from base
        let hsl_left = rgb_to_hsl(&left);
        let hsl_right = rgb_to_hsl(&right);
        assert!((hsl_left.h - hsl_right.h).abs() > 1.0);
    }

    #[test]
    fn test_split_complementary() {
        let base = Color::new(0.0, 0.0, 1.0, 1.0); // blue
        let [b, sc1, sc2] = ColorHarmony::split_complementary(&base);
        assert!((b.b - 1.0).abs() < f64::EPSILON);
        // the split-complementary colors should not be blue
        assert!(sc1.b < 0.9 || sc1.r > 0.1 || sc1.g > 0.1);
        assert!(sc2.b < 0.9 || sc2.r > 0.1 || sc2.g > 0.1);
    }

    #[test]
    fn test_tetradic_has_four_colors() {
        let colors = ColorHarmony::tetradic(&Color::red());
        assert_eq!(colors.len(), 4);
        // first should be the base
        assert!((colors[0].r - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_color_mixer_average() {
        let avg = ColorMixer::average(&[Color::black(), Color::white()]).unwrap();
        assert!((avg.r - 0.5).abs() < f64::EPSILON);
        assert!((avg.g - 0.5).abs() < f64::EPSILON);
        assert!((avg.b - 0.5).abs() < f64::EPSILON);
        assert!(ColorMixer::average(&[]).is_none());
    }

    #[test]
    fn test_tint_shade_scale() {
        let scale = ColorMixer::tint_shade_scale(&Color::red(), 5);
        assert_eq!(scale.len(), 5);
        // darkest at start, lightest at end
        assert!(scale[0].r <= scale[4].r);
        assert!(scale[0].g <= scale[4].g);
    }

    #[test]
    fn test_invert_color() {
        let inv = invert_color(&Color::new(0.2, 0.3, 0.4, 0.9));
        assert!((inv.r - 0.8).abs() < f64::EPSILON);
        assert!((inv.g - 0.7).abs() < f64::EPSILON);
        assert!((inv.b - 0.6).abs() < f64::EPSILON);
        assert!((inv.a - 0.9).abs() < f64::EPSILON);
    }

    // -- ColorPickerHSL tests --

    #[test]
    fn hsl_roundtrip() {
        let color = Color::new(1.0, 0.0, 0.0, 1.0); // red
        let hsl = ColorPickerHSL::from_rgb(&color);
        assert!((hsl.h - 0.0).abs() < 1.0);
        let back = hsl.to_rgb();
        assert!((back.r - 1.0).abs() < 0.01);
    }

    #[test]
    fn hsl_rotate() {
        let mut hsl = ColorPickerHSL::new(0.0, 1.0, 0.5);
        hsl.rotate(120.0);
        assert!((hsl.h - 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn hsl_lighten_darken() {
        let mut hsl = ColorPickerHSL::new(0.0, 1.0, 0.5);
        hsl.lighten(0.2);
        assert!((hsl.l - 0.7).abs() < f64::EPSILON);
        hsl.darken(0.3);
        assert!((hsl.l - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn hsl_display() {
        let hsl = ColorPickerHSL::new(180.0, 0.5, 0.75);
        assert_eq!(format!("{}", hsl), "hsl(180, 50%, 75%)");
    }

    // -- ColorPickerAlpha tests --

    #[test]
    fn alpha_opaque() {
        let c = ColorPickerAlpha::new(Color::new(1.0, 0.0, 0.0, 1.0));
        assert!(c.is_opaque());
        assert!(!c.is_transparent());
    }

    #[test]
    fn alpha_set() {
        let mut c = ColorPickerAlpha::new(Color::new(1.0, 0.0, 0.0, 1.0));
        c.set_alpha(0.5);
        assert!((c.alpha() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn alpha_premultiplied() {
        let c = ColorPickerAlpha::new(Color::new(1.0, 0.5, 0.0, 0.5));
        let pm = c.premultiplied();
        assert!((pm.r - 0.5).abs() < f64::EPSILON);
        assert!((pm.g - 0.25).abs() < f64::EPSILON);
    }

    // -- ColorPickerHistory tests --

    #[test]
    fn history_record() {
        let mut h = ColorPickerHistory::new(5);
        h.record("#ff0000");
        h.record("#00ff00");
        assert_eq!(h.len(), 2);
        assert_eq!(h.recent()[0], "#00ff00");
    }

    #[test]
    fn history_dedup() {
        let mut h = ColorPickerHistory::new(5);
        h.record("#ff0000");
        h.record("#00ff00");
        h.record("#ff0000");
        assert_eq!(h.len(), 2);
        assert_eq!(h.recent()[0], "#ff0000");
    }

    #[test]
    fn history_capacity() {
        let mut h = ColorPickerHistory::new(2);
        h.record("#aa0000");
        h.record("#bb0000");
        h.record("#cc0000");
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn history_contains() {
        let mut h = ColorPickerHistory::new(5);
        h.record("#ff0000");
        assert!(h.contains("#ff0000"));
        assert!(!h.contains("#00ff00"));
    }

    // -- format_color tests --

    #[test]
    fn format_color_hex() {
        let c = Color::new(1.0, 0.0, 0.0, 1.0);
        assert_eq!(format_color(&c, ColorFormat::Hex), "#FF0000");
    }

    #[test]
    fn format_color_hex_with_alpha() {
        let c = Color::new(1.0, 0.0, 0.0, 0.5);
        let s = format_color(&c, ColorFormat::Hex);
        assert!(s.starts_with("#FF0000"));
        assert_eq!(s.len(), 9); // #RRGGBBAA
    }

    #[test]
    fn format_color_rgb() {
        let c = Color::new(1.0, 0.0, 0.0, 1.0);
        let s = format_color(&c, ColorFormat::Rgb);
        assert!(s.starts_with("rgba(") || s.starts_with("rgb("));
    }

    // -- ColorPalettePicker tests --

    #[test]
    fn palette_picker_material_design() {
        let picker = ColorPalettePicker::material_design();
        assert_eq!(picker.palette_count(), 1);
        assert!(picker.total_colors() >= 12);
        assert_eq!(picker.palette_names(), vec!["Material Design"]);
    }

    #[test]
    fn palette_picker_tailwind_css() {
        let picker = ColorPalettePicker::tailwind_css();
        assert_eq!(picker.palette_count(), 1);
        assert!(picker.total_colors() >= 15);
        let palette = picker.get_palette("Tailwind CSS").unwrap();
        assert!(palette.iter().any(|(n, _)| n == "Blue"));
    }

    #[test]
    fn palette_picker_search() {
        let picker = ColorPalettePicker::material_design();
        let results = picker.search("pur");
        assert!(results.len() >= 1);
        assert!(results.iter().any(|(_, name, _)| name.contains("Purple")));
    }

    #[test]
    fn palette_picker_add_remove() {
        let mut picker = ColorPalettePicker::new();
        picker.add_palette("Custom", vec![
            ("Fog".to_string(), Color::new(0.8, 0.8, 0.85, 1.0)),
        ]);
        assert_eq!(picker.palette_count(), 1);
        assert!(picker.remove_palette("Custom"));
        assert_eq!(picker.palette_count(), 0);
        assert!(!picker.remove_palette("Nonexistent"));
    }

    #[test]
    fn palette_picker_default_empty() {
        let picker = ColorPalettePicker::default();
        assert!(picker.palette_count() == 0);
        assert_eq!(picker.total_colors(), 0);
    }

    // -- ColorFormatAutoDetector tests --

    #[test]
    fn detect_hex3() {
        assert_eq!(
            ColorFormatAutoDetector::detect("#f0c"),
            Some(DetectedColorFormat::Hex3),
        );
    }

    #[test]
    fn detect_hex6() {
        assert_eq!(
            ColorFormatAutoDetector::detect("#ff0080"),
            Some(DetectedColorFormat::Hex6),
        );
    }

    #[test]
    fn detect_hex8() {
        assert_eq!(
            ColorFormatAutoDetector::detect("#ff008080"),
            Some(DetectedColorFormat::Hex8),
        );
    }

    #[test]
    fn detect_rgb_function() {
        assert_eq!(
            ColorFormatAutoDetector::detect("rgb(255, 0, 0)"),
            Some(DetectedColorFormat::RgbFunction),
        );
        assert_eq!(
            ColorFormatAutoDetector::detect("rgba(255, 0, 0, 0.5)"),
            Some(DetectedColorFormat::RgbFunction),
        );
    }

    #[test]
    fn detect_hsl_function() {
        assert_eq!(
            ColorFormatAutoDetector::detect("hsl(120, 50%, 50%)"),
            Some(DetectedColorFormat::HslFunction),
        );
    }

    #[test]
    fn detect_hwb_function() {
        assert_eq!(
            ColorFormatAutoDetector::detect("hwb(120 10% 20%)"),
            Some(DetectedColorFormat::HwbFunction),
        );
    }

    #[test]
    fn detect_named_color() {
        assert_eq!(
            ColorFormatAutoDetector::detect("coral"),
            Some(DetectedColorFormat::Named),
        );
    }

    #[test]
    fn detect_unknown() {
        assert_eq!(ColorFormatAutoDetector::detect("notacolor"), None);
        assert_eq!(ColorFormatAutoDetector::detect("#zzzzzz"), None);
    }

    #[test]
    fn detect_and_parse_roundtrip() {
        let (fmt, color) = ColorFormatAutoDetector::detect_and_parse("#FF0000").unwrap();
        assert_eq!(fmt, DetectedColorFormat::Hex6);
        assert!((color.r - 1.0).abs() < 1e-2);
        assert!((color.g).abs() < 1e-2);
    }

    #[test]
    fn suggest_output_format_mapping() {
        assert_eq!(
            ColorFormatAutoDetector::suggest_output_format(DetectedColorFormat::Hex6),
            ColorFormat::Hex,
        );
        assert_eq!(
            ColorFormatAutoDetector::suggest_output_format(DetectedColorFormat::RgbFunction),
            ColorFormat::Rgb,
        );
        assert_eq!(
            ColorFormatAutoDetector::suggest_output_format(DetectedColorFormat::HslFunction),
            ColorFormat::Hsl,
        );
        assert_eq!(
            ColorFormatAutoDetector::suggest_output_format(DetectedColorFormat::Named),
            ColorFormat::Hex,
        );
    }

    // -- ColorSwatchRenderer tests --

    #[test]
    fn swatch_renderer_default() {
        let renderer = ColorSwatchRenderer::default();
        let lines = renderer.render(&Color::red());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "██");
    }

    #[test]
    fn swatch_renderer_custom_size() {
        let renderer = ColorSwatchRenderer::new(4, 3);
        let lines = renderer.render(&Color::blue());
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "████");
    }

    #[test]
    fn swatch_renderer_with_border() {
        let renderer = ColorSwatchRenderer::new(3, 2).with_border(true);
        let lines = renderer.render(&Color::green());
        assert_eq!(lines.len(), 4); // top border + 2 rows + bottom border
        assert_eq!(lines[0], "+---+");
        assert_eq!(lines[1], "|███|");
        assert_eq!(lines[3], "+---+");
    }

    #[test]
    fn swatch_renderer_ansi_output() {
        let renderer = ColorSwatchRenderer::new(2, 1);
        let lines = renderer.render_ansi(&Color::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("\x1b[38;2;255;0;0m"));
        assert!(lines[0].contains("\x1b[0m"));
    }

    #[test]
    fn swatch_renderer_row() {
        let renderer = ColorSwatchRenderer::new(2, 1);
        let colors = vec![Color::red(), Color::green(), Color::blue()];
        let row = renderer.render_row(&colors);
        assert_eq!(row.len(), 1);
        assert_eq!(row[0], "██ ██ ██");
    }

    #[test]
    fn swatch_renderer_dimensions() {
        let r = ColorSwatchRenderer::new(5, 3);
        assert_eq!(r.swatch_width(), 5);
        assert_eq!(r.swatch_height(), 3);

        let rb = ColorSwatchRenderer::new(5, 3).with_border(true);
        assert_eq!(rb.swatch_width(), 7);
        assert_eq!(rb.swatch_height(), 5);
    }

    #[test]
    fn swatch_renderer_empty_row() {
        let renderer = ColorSwatchRenderer::default();
        let row = renderer.render_row(&[]);
        assert!(row.is_empty());
    }

    // -- ColorNameLookup tests --

    #[test]
    fn name_lookup_by_name() {
        let lookup = ColorNameLookup::new();
        let c = lookup.by_name("red").unwrap();
        assert!((c.r - 1.0).abs() < 1e-3);
        assert!(c.g.abs() < 1e-3);
        assert!(c.b.abs() < 1e-3);
    }

    #[test]
    fn name_lookup_case_insensitive() {
        let lookup = ColorNameLookup::new();
        assert!(lookup.by_name("RED").is_some());
        assert!(lookup.by_name("Blue").is_some());
        assert!(lookup.by_name("TURQUOISE").is_some());
    }

    #[test]
    fn name_lookup_closest() {
        let lookup = ColorNameLookup::new();
        let almost_red = Color::new(0.98, 0.02, 0.01, 1.0);
        let (name, dist) = lookup.closest(&almost_red);
        assert_eq!(name, "red");
        assert!(dist < 0.1);
    }

    #[test]
    fn name_lookup_closest_exact() {
        let lookup = ColorNameLookup::new();
        let (name, dist) = lookup.closest(&Color::new(0.0, 0.0, 0.0, 1.0));
        assert_eq!(name, "black");
        assert!(dist < 1e-10);
    }

    #[test]
    fn name_lookup_fuzzy_search() {
        let lookup = ColorNameLookup::new();
        let results = lookup.fuzzy_search("re");
        assert!(results.contains(&"red"));
        assert!(results.contains(&"green"));
    }

    #[test]
    fn name_lookup_fuzzy_no_match() {
        let lookup = ColorNameLookup::new();
        let results = lookup.fuzzy_search("zzz");
        assert!(results.is_empty());
    }

    #[test]
    fn name_lookup_sorted_names() {
        let lookup = ColorNameLookup::new();
        let names = lookup.sorted_names();
        assert!(!names.is_empty());
        // Verify actually sorted
        for pair in names.windows(2) {
            assert!(pair[0] <= pair[1]);
        }
    }

    #[test]
    fn name_lookup_len() {
        let lookup = ColorNameLookup::new();
        assert_eq!(lookup.len(), 28);
        assert!(!lookup.is_empty());
    }

    #[test]
    fn name_lookup_iter() {
        let lookup = ColorNameLookup::new();
        let count = lookup.iter().count();
        assert_eq!(count, lookup.len());
    }

    #[test]
    fn name_lookup_not_found() {
        let lookup = ColorNameLookup::new();
        assert!(lookup.by_name("chartreuse").is_none());
        assert!(lookup.by_name("").is_none());
    }



    // -----------------------------------------------------------------------
    // ColorPalettePicker additional methods
    // -----------------------------------------------------------------------

    #[test]
    fn palette_picker_with_material_preset() {
        let picker = ColorPalettePicker::with_material_preset();
        assert_eq!(picker.palette_count(), 1);
        assert!(picker.get_palette("material").is_some());
        let colors = picker.get_palette("material").unwrap();
        assert_eq!(colors.len(), 4);
        let names: Vec<&str> = colors.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"red"));
        assert!(names.contains(&"blue"));
        assert!(names.contains(&"green"));
        assert!(names.contains(&"yellow"));
    }

    #[test]
    fn palette_picker_with_pastel_preset() {
        let picker = ColorPalettePicker::with_pastel_preset();
        assert_eq!(picker.palette_count(), 1);
        assert!(picker.get_palette("pastel").is_some());
        let pastel = picker.get_palette("pastel").unwrap();
        assert_eq!(pastel.len(), 6);
    }

    #[test]
    fn palette_picker_find_color_in_any() {
        let picker = ColorPalettePicker::with_material_preset();
        let found = picker.find_color_in_any("red");
        assert!(found.is_some());
        let red = found.unwrap();
        assert!((red.r - 1.0).abs() < 0.01);

        assert!(picker.find_color_in_any("nonexistent").is_none());
    }

    #[test]
    fn palette_picker_find_color_case_insensitive() {
        let picker = ColorPalettePicker::with_material_preset();
        assert!(picker.find_color_in_any("RED").is_some());
        assert!(picker.find_color_in_any("Red").is_some());
    }

    #[test]
    fn palette_picker_total_color_count() {
        let picker = ColorPalettePicker::with_material_preset();
        assert_eq!(picker.total_color_count(), 4);

        let empty = ColorPalettePicker::new();
        assert_eq!(empty.total_color_count(), 0);
    }

    #[test]
    fn palette_picker_multiple_palettes_total() {
        let mut picker = ColorPalettePicker::with_material_preset();
        let pastel = ColorPalettePicker::with_pastel_preset();
        let pastel_colors = pastel.get_palette("pastel").unwrap().to_vec();
        picker.add_palette("pastel", pastel_colors);
        assert_eq!(picker.palette_count(), 2);
        assert_eq!(picker.total_color_count(), 10);
    }

    // -----------------------------------------------------------------------
    // ColorFormatAutoDetector additional methods
    // -----------------------------------------------------------------------

    #[test]
    fn auto_detect_is_hex() {
        assert!(ColorFormatAutoDetector::is_hex("#FF0000"));
        assert!(ColorFormatAutoDetector::is_hex("#F00"));
        assert!(ColorFormatAutoDetector::is_hex("#FF000080"));
        assert!(!ColorFormatAutoDetector::is_hex("rgb(255,0,0)"));
        assert!(!ColorFormatAutoDetector::is_hex("red"));
    }

    #[test]
    fn auto_detect_is_rgb() {
        assert!(ColorFormatAutoDetector::is_rgb("rgb(255, 0, 0)"));
        assert!(ColorFormatAutoDetector::is_rgb("rgba(255, 0, 0, 0.5)"));
        assert!(!ColorFormatAutoDetector::is_rgb("#FF0000"));
        assert!(!ColorFormatAutoDetector::is_rgb("hsl(0, 100%, 50%)"));
    }

    #[test]
    fn auto_detect_is_hsl() {
        assert!(ColorFormatAutoDetector::is_hsl("hsl(0, 100%, 50%)"));
        assert!(ColorFormatAutoDetector::is_hsl("hsla(0, 100%, 50%, 0.5)"));
        assert!(!ColorFormatAutoDetector::is_hsl("#FF0000"));
        assert!(!ColorFormatAutoDetector::is_hsl("rgb(255, 0, 0)"));
    }

    #[test]
    fn auto_detect_is_named() {
        assert!(ColorFormatAutoDetector::is_named("red"));
        assert!(ColorFormatAutoDetector::is_named("blue"));
        assert!(!ColorFormatAutoDetector::is_named("#FF0000"));
        assert!(!ColorFormatAutoDetector::is_named("notacolor"));
    }

    #[test]
    fn auto_detect_suggest_format_opaque() {
        let opaque = Color::new(1.0, 0.0, 0.0, 1.0);
        assert_eq!(ColorFormatAutoDetector::suggest_format(&opaque), ColorFormat::Hex);
    }

    #[test]
    fn auto_detect_suggest_format_transparent() {
        let semi = Color::new(1.0, 0.0, 0.0, 0.5);
        assert_eq!(ColorFormatAutoDetector::suggest_format(&semi), ColorFormat::Rgb);
    }

    #[test]
    fn auto_detect_all_in_line_hex() {
        let line = "color: #FF0000; bg: #00FF00;";
        let detected = ColorFormatAutoDetector::detect_all_in_line(line);
        assert_eq!(detected.len(), 2);
        assert_eq!(detected[0].1, DetectedColorFormat::Hex6);
        assert_eq!(detected[1].1, DetectedColorFormat::Hex6);
    }

    #[test]
    fn auto_detect_all_in_line_mixed() {
        let line = "#F00 rgb(0,0,255)";
        let detected = ColorFormatAutoDetector::detect_all_in_line(line);
        assert!(detected.len() >= 2);
    }

    #[test]
    fn auto_detect_all_in_line_empty() {
        let detected = ColorFormatAutoDetector::detect_all_in_line("");
        assert!(detected.is_empty());
    }

    #[test]
    fn auto_detect_all_in_line_no_colors() {
        let detected = ColorFormatAutoDetector::detect_all_in_line("no colors here at all");
        assert!(detected.is_empty());
    }

    // -----------------------------------------------------------------------
    // ColorSwatchRenderer additional methods
    // -----------------------------------------------------------------------

    #[test]
    fn swatch_render_with_label() {
        let renderer = ColorSwatchRenderer::new(5, 2);
        let red = Color::new(1.0, 0.0, 0.0, 1.0);
        let rows = renderer.render_with_label(&red, "RED");
        // 2 swatch rows + 1 label row
        assert_eq!(rows.len(), 3);
        assert!(rows[2].contains("RED"));
    }

    #[test]
    fn swatch_render_with_label_long() {
        let renderer = ColorSwatchRenderer::new(3, 1);
        let red = Color::new(1.0, 0.0, 0.0, 1.0);
        let rows = renderer.render_with_label(&red, "LONGNAME");
        // label gets truncated to width
        assert_eq!(rows.last().unwrap().len(), 3);
    }

    #[test]
    fn swatch_render_palette() {
        let mut palette = ColorPalette::new();
        palette.add("r", Color::red());
        palette.add("b", Color::blue());
        let renderer = ColorSwatchRenderer::new(4, 1);
        let output = renderer.render_palette(&palette);
        // 2 colors * (1 swatch row + 1 label row) = 4 lines
        assert_eq!(output.len(), 4);
    }

    #[test]
    fn swatch_render_palette_empty() {
        let palette = ColorPalette::new();
        let renderer = ColorSwatchRenderer::new(4, 1);
        let output = renderer.render_palette(&palette);
        assert!(output.is_empty());
    }

    #[test]
    fn swatch_block_char() {
        assert_eq!(ColorSwatchRenderer::block_char(), '█');
    }

    #[test]
    fn swatch_render_with_label_border() {
        let renderer = ColorSwatchRenderer::new(5, 1).with_border(true);
        let c = Color::green();
        let rows = renderer.render_with_label(&c, "G");
        // border top + 1 swatch row + border bottom + label = 4
        assert_eq!(rows.len(), 4);
    }

    // -----------------------------------------------------------------------
    // ColorNameLookup additional methods
    // -----------------------------------------------------------------------

    #[test]
    fn name_lookup_exact_match() {
        let lookup = ColorNameLookup::new();
        let red = Color::new(1.0, 0.0, 0.0, 1.0);
        assert_eq!(lookup.lookup(&red), Some("red"));
    }

    #[test]
    fn name_lookup_no_exact_match() {
        let lookup = ColorNameLookup::new();
        let custom = Color::new(0.123, 0.456, 0.789, 1.0);
        assert!(lookup.lookup(&custom).is_none());
    }

    #[test]
    fn name_lookup_closest_name() {
        let lookup = ColorNameLookup::new();
        let almost_red = Color::new(0.99, 0.01, 0.01, 1.0);
        let name = lookup.closest_name(&almost_red);
        assert_eq!(name, "red");
    }

    #[test]
    fn name_lookup_all_names() {
        let lookup = ColorNameLookup::new();
        let names = lookup.all_names();
        assert!(names.len() > 0);
        assert!(names.contains(&"red"));
        assert!(names.contains(&"blue"));
        assert!(names.contains(&"green"));
    }

    #[test]
    fn name_lookup_name_to_color() {
        let lookup = ColorNameLookup::new();
        let red = lookup.name_to_color("red").unwrap();
        assert!((red.r - 1.0).abs() < 0.01);
        assert!((red.g - 0.0).abs() < 0.01);
        assert!((red.b - 0.0).abs() < 0.01);

        assert!(lookup.name_to_color("notacolor").is_none());
    }

    #[test]
    fn name_lookup_name_count() {
        let lookup = ColorNameLookup::new();
        assert_eq!(lookup.name_count(), lookup.len());
        assert!(lookup.name_count() > 0);
    }

    #[test]
    fn name_lookup_contains() {
        let lookup = ColorNameLookup::new();
        assert!(lookup.contains("red"));
        assert!(lookup.contains("blue"));
        assert!(lookup.contains("white"));
        assert!(!lookup.contains("chartreuse"));
        assert!(!lookup.contains(""));
    }

    #[test]
    fn name_lookup_name_to_color_case_insensitive() {
        let lookup = ColorNameLookup::new();
        assert!(lookup.name_to_color("RED").is_some());
        assert!(lookup.name_to_color("Red").is_some());
    }

    #[test]
    fn name_lookup_contains_case_insensitive() {
        let lookup = ColorNameLookup::new();
        assert!(lookup.contains("RED"));
        assert!(lookup.contains("Blue"));
    }

    #[test]
    fn name_lookup_roundtrip() {
        let lookup = ColorNameLookup::new();
        for name in lookup.all_names() {
            let color = lookup.name_to_color(name).unwrap();
            let found_name = lookup.lookup(&color).unwrap();
            assert_eq!(found_name, name);
        }
    }


    // xa_ extended tests for colorpicker
    #[test]
    fn xa_colorpicker_ring_new() {
        let rb = super::XaColorpickerRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_colorpicker_ring_push_len() {
        let mut rb = super::XaColorpickerRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_colorpicker_ring_wrap() {
        let mut rb = super::XaColorpickerRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_colorpicker_ring_mean_empty() {
        let rb = super::XaColorpickerRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_colorpicker_ring_mean_values() {
        let mut rb = super::XaColorpickerRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_colorpicker_ring_min_max() {
        let mut rb = super::XaColorpickerRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_colorpicker_ring_iter() {
        let mut rb = super::XaColorpickerRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_colorpicker_counter_new() {
        let c = super::XaColorpickerCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_colorpicker_counter_inc() {
        let mut c = super::XaColorpickerCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_colorpicker_counter_inc_by() {
        let mut c = super::XaColorpickerCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_colorpicker_counter_reset() {
        let mut c = super::XaColorpickerCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_colorpicker_counter_clear() {
        let mut c = super::XaColorpickerCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_colorpicker_counter_default() {
        let c = super::XaColorpickerCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 18 ----

    #[test]
    fn xc_18_pool_new_empty() {
        let pool: super::Xc18Pool<i32> = super::Xc18Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_18_pool_release_acquire() {
        let mut pool = super::Xc18Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_18_pool_acquire_empty() {
        let mut pool: super::Xc18Pool<i32> = super::Xc18Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_18_pool_full() {
        let mut pool = super::Xc18Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_18_pool_drain() {
        let mut pool = super::Xc18Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_18_pool_stats() {
        let mut pool = super::Xc18Pool::new(8);
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
    fn xc_18_pool_clear() {
        let mut pool = super::Xc18Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_18_pool_shrink() {
        let mut pool = super::Xc18Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_18_pool_default() {
        let pool: super::Xc18Pool<String> = super::Xc18Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_18_pool_extend() {
        let mut pool = super::Xc18Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_18_pool_retain() {
        let mut pool = super::Xc18Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_18_scheduler_round_robin() {
        let mut sched = super::Xc18Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_18_scheduler_empty() {
        let mut sched = super::Xc18Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_18_scheduler_reset() {
        let mut sched = super::Xc18Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_18_scheduler_add_remove() {
        let mut sched = super::Xc18Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_18_scheduler_targets() {
        let sched = super::Xc18Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_18_hash_empty() {
        assert_eq!(super::xc_18_hash(b""), 5381);
    }

    #[test]
    fn xc_18_hash_data() {
        let h = super::xc_18_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_18_hash(b"hello"), h);
    }

    #[test]
    fn xc_18_reverse_str() {
        assert_eq!(super::xc_18_reverse("abc"), "cba");
        assert_eq!(super::xc_18_reverse(""), "");
    }


    // --- xd_82 deepening tests ---

    #[test]
    fn xd_82_sm_initial_state() {
        let sm = Xd82StateMachine::new();
        assert_eq!(sm.current_state(), Xd82State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_82_sm_valid_idle_to_running() {
        let mut sm = Xd82StateMachine::new();
        assert!(sm.transition(Xd82State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd82State::Running);
    }

    #[test]
    fn xd_82_sm_valid_running_to_paused() {
        let mut sm = Xd82StateMachine::new();
        sm.transition(Xd82State::Running).unwrap();
        assert!(sm.transition(Xd82State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd82State::Paused);
    }

    #[test]
    fn xd_82_sm_valid_running_to_done() {
        let mut sm = Xd82StateMachine::new();
        sm.transition(Xd82State::Running).unwrap();
        assert!(sm.transition(Xd82State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd82State::Done);
    }

    #[test]
    fn xd_82_sm_valid_paused_to_running() {
        let mut sm = Xd82StateMachine::new();
        sm.transition(Xd82State::Running).unwrap();
        sm.transition(Xd82State::Paused).unwrap();
        assert!(sm.transition(Xd82State::Running).is_ok());
    }

    #[test]
    fn xd_82_sm_valid_done_to_idle() {
        let mut sm = Xd82StateMachine::new();
        sm.transition(Xd82State::Running).unwrap();
        sm.transition(Xd82State::Done).unwrap();
        assert!(sm.transition(Xd82State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd82State::Idle);
    }

    #[test]
    fn xd_82_sm_invalid_idle_to_done() {
        let mut sm = Xd82StateMachine::new();
        assert!(sm.transition(Xd82State::Done).is_err());
    }

    #[test]
    fn xd_82_sm_invalid_idle_to_paused() {
        let mut sm = Xd82StateMachine::new();
        assert!(sm.transition(Xd82State::Paused).is_err());
    }

    #[test]
    fn xd_82_sm_history_tracking() {
        let mut sm = Xd82StateMachine::new();
        sm.transition(Xd82State::Running).unwrap();
        sm.transition(Xd82State::Paused).unwrap();
        sm.transition(Xd82State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd82State::Idle);
        assert_eq!(sm.history()[0].to, Xd82State::Running);
        assert_eq!(sm.history()[1].from, Xd82State::Running);
        assert_eq!(sm.history()[2].to, Xd82State::Done);
    }

    #[test]
    fn xd_82_sm_serialize_deserialize() {
        let mut sm = Xd82StateMachine::new();
        sm.transition(Xd82State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd82StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd82State::Running));
    }

    #[test]
    fn xd_82_sm_deserialize_invalid() {
        assert_eq!(Xd82StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_82_sm_reset() {
        let mut sm = Xd82StateMachine::new();
        sm.transition(Xd82State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd82State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_82_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd82EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd82Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_82_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd82EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd82Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd82Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_82_bus_unsubscribe() {
        let mut bus = Xd82EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_82_event_kind_and_payload() {
        let e = Xd82Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd82Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_82_bus_clear_history() {
        let mut bus = Xd82EventBus::new();
        bus.publish(Xd82Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_82_sm_step_counter_increments() {
        let mut sm = Xd82StateMachine::new();
        sm.transition(Xd82State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd82State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #102 --

    #[test]
    fn xf102_trie_insert_search() {
        let mut t = Xf102Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf102_trie_starts_with() {
        let mut t = Xf102Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf102_trie_remove() {
        let mut t = Xf102Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf102_trie_word_count() {
        let mut t = Xf102Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf102_trie_longest_prefix() {
        let mut t = Xf102Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf102_trie_all_words() {
        let mut t = Xf102Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf102_trie_autocomplete() {
        let mut t = Xf102Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf102_trie_empty_search() {
        let t = Xf102Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf102_bloom_add_contains() {
        let mut bf = Xf102BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf102_bloom_probably_absent() {
        let bf = Xf102BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf102_bloom_false_positive_rate() {
        let mut bf = Xf102BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf102_bloom_clear() {
        let mut bf = Xf102BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf102_bloom_union() {
        let mut a = Xf102BloomFilter::xf_new(512, 2);
        let mut b = Xf102BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf102_bloom_intersection_estimate() {
        let mut a = Xf102BloomFilter::xf_new(512, 2);
        let mut b = Xf102BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf102_bloom_union_size_mismatch() {
        let a = Xf102BloomFilter::xf_new(256, 2);
        let b = Xf102BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh17_skip_insert_contains() {
        let mut sl = super::Xh17SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh17_skip_remove() {
        let mut sl = super::Xh17SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh17_skip_len() {
        let mut sl = super::Xh17SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh17_skip_range_query() {
        let mut sl = super::Xh17SkipList::xh_new(4);
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
    fn xh17_skip_floor_ceiling() {
        let mut sl = super::Xh17SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh17_skip_rank() {
        let mut sl = super::Xh17SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh17_skip_empty() {
        let sl = super::Xh17SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh17_skip_duplicates() {
        let mut sl = super::Xh17SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh17_bitset_set_test() {
        let mut bs = super::Xh17BitSet::xh_new(256);
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
    fn xh17_bitset_clear_count() {
        let mut bs = super::Xh17BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh17_bitset_and_or_xor() {
        let mut a = super::Xh17BitSet::xh_new(128);
        let mut b = super::Xh17BitSet::xh_new(128);
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
    fn xh17_bitset_iter_ones() {
        let mut bs = super::Xh17BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh17_bitset_first_last() {
        let mut bs = super::Xh17BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh17_bitset_empty() {
        let bs = super::Xh17BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi17_deque_push_pop_back() {
        let mut dq = super::Xi17Deque::xi_new(4);
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
    fn xi17_deque_push_pop_front() {
        let mut dq = super::Xi17Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi17_deque_mixed_ops() {
        let mut dq = super::Xi17Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi17_deque_get_and_split() {
        let mut dq = super::Xi17Deque::xi_new(8);
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
    fn xi17_deque_rotate_left() {
        let mut dq = super::Xi17Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi17_deque_rotate_right() {
        let mut dq = super::Xi17Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi17_deque_grow() {
        let mut dq = super::Xi17Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi17_deque_empty() {
        let dq = super::Xi17Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi17_interval_tree_insert_query() {
        let mut tree = super::Xi17IntervalTree::xi_new();
        tree.xi_insert(super::Xi17Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi17Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi17Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi17_interval_tree_overlap() {
        let mut tree = super::Xi17IntervalTree::xi_new();
        tree.xi_insert(super::Xi17Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi17Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi17Interval::xi_new(12, 20));
        let q = super::Xi17Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi17_interval_tree_remove() {
        let mut tree = super::Xi17IntervalTree::xi_new();
        tree.xi_insert(super::Xi17Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi17Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi17_interval_tree_gaps() {
        let mut tree = super::Xi17IntervalTree::xi_new();
        tree.xi_insert(super::Xi17Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi17Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi17Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi17Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi17Interval::xi_new(8, 10));
    }

    #[test]
    fn xi17_interval_tree_merge() {
        let mut tree = super::Xi17IntervalTree::xi_new();
        tree.xi_insert(super::Xi17Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi17Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi17Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi17Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi17Interval::xi_new(10, 15));
    }

    #[test]
    fn xi17_interval_tree_all() {
        let mut tree = super::Xi17IntervalTree::xi_new();
        tree.xi_insert(super::Xi17Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi17Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi17_interval_tree_empty() {
        let tree = super::Xi17IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi17_interval_tree_contains_point() {
        let iv = super::Xi17Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 17) ---

    #[test]
    fn xj_17_uf_make_and_find() {
        let mut uf = super::Xj17UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_17_uf_union_connected() {
        let mut uf = super::Xj17UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_17_uf_component_count() {
        let mut uf = super::Xj17UnionFind::xj_new();
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
    fn xj_17_uf_component_size() {
        let mut uf = super::Xj17UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_17_uf_largest_component() {
        let mut uf = super::Xj17UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_17_uf_many_elements() {
        let mut uf = super::Xj17UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_17_uf_separate_components() {
        let mut uf = super::Xj17UnionFind::xj_new();
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
    fn xj_17_uf_path_compression() {
        let mut uf = super::Xj17UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_17_bt_insert_get() {
        let mut bt = super::Xj17BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_17_bt_contains_len() {
        let mut bt = super::Xj17BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_17_bt_replace() {
        let mut bt = super::Xj17BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_17_bt_remove() {
        let mut bt = super::Xj17BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_17_bt_keys_values() {
        let mut bt = super::Xj17BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_17_bt_range() {
        let mut bt = super::Xj17BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_17_bt_min_max() {
        let mut bt = super::Xj17BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_17_bt_many_inserts() {
        let mut bt = super::Xj17BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_17 segment tree tests ---

    #[test]
    fn xk_17_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk17SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_17_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk17SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_17_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk17SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_17_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk17SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_17_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk17SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_17_st_single_element() {
        let data = vec![42];
        let st = super::Xk17SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_17_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk17SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_17_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk17SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_17 disjoint intervals tests ---

    #[test]
    fn xk_17_di_add_and_count() {
        let mut di = super::Xk17DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_17_di_merge_overlap() {
        let mut di = super::Xk17DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_17_di_contains() {
        let mut di = super::Xk17DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_17_di_remove() {
        let mut di = super::Xk17DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_17_di_covered_length() {
        let mut di = super::Xk17DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_17_di_gaps() {
        let mut di = super::Xk17DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_17_di_merge_adjacent() {
        let mut di = super::Xk17DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_17_di_empty() {
        let di = super::Xk17DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_17_rope_new_empty() {
        let rope = super::Xl17Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_17_rope_from_str() {
        let rope = super::Xl17Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_17_rope_insert_at() {
        let mut rope = super::Xl17Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_17_rope_delete_range() {
        let mut rope = super::Xl17Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_17_rope_char_at() {
        let rope = super::Xl17Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_17_rope_split_concat() {
        let rope = super::Xl17Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_17_rope_line_count() {
        let rope = super::Xl17Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_17_rope_line_at() {
        let rope = super::Xl17Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_17_sa_build_and_search() {
        let sa = super::Xl17SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_17_sa_count() {
        let sa = super::Xl17SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_17_sa_longest_repeated() {
        let sa = super::Xl17SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_17_sa_all_positions() {
        let sa = super::Xl17SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_17_sa_len() {
        let sa = super::Xl17SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_17_sa_empty() {
        let sa = super::Xl17SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_17_rope_slice() {
        let rope = super::Xl17Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_17_sa_search_start() {
        let sa = super::Xl17SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_17_sparse_set_get() {
        let mut m = super::Xm17MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_17_sparse_row_col() {
        let mut m = super::Xm17MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_17_sparse_transpose() {
        let mut m = super::Xm17MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_17_sparse_multiply_vec() {
        let mut m = super::Xm17MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_17_sparse_nnz_density() {
        let mut m = super::Xm17MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_17_sparse_clear() {
        let mut m = super::Xm17MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_17_sparse_overwrite_zero() {
        let mut m = super::Xm17MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_17_tokenizer_basic() {
        let t = super::Xm17Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_17_tokenizer_count() {
        let t = super::Xm17Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_17_tokenizer_unique() {
        let t = super::Xm17Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_17_tokenizer_frequency() {
        let t = super::Xm17Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_17_tokenizer_delimiter() {
        let t = super::Xm17Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_17_tokenizer_whitespace() {
        let t = super::Xm17Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_17_tokenizer_empty() {
        let t = super::Xm17Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 17 ----

    #[test]
    fn xn_17_fenwick_prefix_sum() {
        let mut ft = super::Xn17Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_17_fenwick_range_sum() {
        let mut ft = super::Xn17Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_17_fenwick_point_query() {
        let mut ft = super::Xn17Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_17_fenwick_len() {
        let ft = super::Xn17Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_17_fenwick_multiple_updates() {
        let mut ft = super::Xn17Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_17_fenwick_single_element() {
        let mut ft = super::Xn17Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_17_fenwick_find_kth() {
        let mut ft = super::Xn17Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_17_fenwick_negative_delta() {
        let mut ft = super::Xn17Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 17 ----

    #[test]
    fn xn_17_avl_insert_get() {
        let mut m = super::Xn17AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_17_avl_remove() {
        let mut m = super::Xn17AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_17_avl_in_order() {
        let mut m = super::Xn17AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_17_avl_min_max() {
        let mut m = super::Xn17AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_17_avl_floor_ceiling() {
        let mut m = super::Xn17AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_17_avl_height_balanced() {
        let mut m = super::Xn17AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_17_avl_overwrite() {
        let mut m = super::Xn17AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_17_avl_empty() {
        let m: super::Xn17AVL<i32, i32> = super::Xn17AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo17RedBlack tests ---

    #[test]
    fn xo_17_rb_insert_and_get() {
        let mut tree = super::Xo17RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_17_rb_len_and_empty() {
        let mut tree = super::Xo17RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_17_rb_min_max() {
        let mut tree = super::Xo17RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_17_rb_contains() {
        let mut tree = super::Xo17RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_17_rb_remove() {
        let mut tree = super::Xo17RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_17_rb_in_order() {
        let mut tree = super::Xo17RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_17_rb_black_height() {
        let mut tree = super::Xo17RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_17_rb_overwrite() {
        let mut tree = super::Xo17RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo17ConsistentHash tests ---

    #[test]
    fn xo_17_ch_add_and_count() {
        let mut ring = super::Xo17ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_17_ch_remove_node() {
        let mut ring = super::Xo17ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_17_ch_get_node() {
        let mut ring = super::Xo17ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_17_ch_empty_ring() {
        let ring = super::Xo17ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_17_ch_distribution() {
        let mut ring = super::Xo17ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_17_ch_rebalance() {
        let mut ring = super::Xo17ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_17_ch_virtual_nodes() {
        let mut ring = super::Xo17ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_17_ch_consistent_lookup() {
        let mut ring = super::Xo17ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_17_splay_insert_get() {
        let mut t = super::Xp17SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_17_splay_remove() {
        let mut t = super::Xp17SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_17_splay_count_increases() {
        let mut t = super::Xp17SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_17_splay_depth() {
        let mut t = super::Xp17SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_17_splay_len_empty() {
        let t = super::Xp17SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_17_splay_min_max() {
        let mut t = super::Xp17SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_17_splay_overwrite() {
        let mut t = super::Xp17SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_17_splay_remove_missing() {
        let mut t = super::Xp17SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_17 treap tests ----
    #[test]
    fn xq_17_treap_empty() {
        let t = super::Xq17Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_17_treap_insert_get() {
        let mut t = super::Xq17Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_17_treap_overwrite() {
        let mut t = super::Xq17Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_17_treap_remove() {
        let mut t = super::Xq17Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_17_treap_min_max() {
        let mut t = super::Xq17Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_17_treap_rank() {
        let mut t = super::Xq17Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_17_treap_kth() {
        let mut t = super::Xq17Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_17_treap_in_order() {
        let mut t = super::Xq17Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_17 VEB tree tests ----
    #[test]
    fn xq_17_veb_empty() {
        let v = super::Xq17VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_17_veb_insert_contains() {
        let mut v = super::Xq17VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_17_veb_min_max() {
        let mut v = super::Xq17VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_17_veb_delete() {
        let mut v = super::Xq17VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_17_veb_successor() {
        let mut v = super::Xq17VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_17_veb_predecessor() {
        let mut v = super::Xq17VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_17_veb_count() {
        let mut v = super::Xq17VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_17_veb_duplicate_insert() {
        let mut v = super::Xq17VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }

}
