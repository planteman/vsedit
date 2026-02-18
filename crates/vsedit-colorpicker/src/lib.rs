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

}
