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
}
