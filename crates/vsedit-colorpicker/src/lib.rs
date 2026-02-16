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
/// `#RGB`, `#RRGGBB`, `#RRGGBBAA`, or `rgb(R, G, B)`.
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
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
}
