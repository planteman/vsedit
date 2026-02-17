//! Theme management.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur during theme operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ThemeError {
    ThemeNotFound(String),
    DuplicateTheme(String),
    InvalidColor(String),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::ThemeNotFound(id) => write!(f, "theme not found: {}", id),
            ThemeError::DuplicateTheme(id) => write!(f, "duplicate theme: {}", id),
            ThemeError::InvalidColor(c) => write!(f, "invalid color: {}", c),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ThemeType {
    Light,
    Dark,
    HighContrast,
    HighContrastLight,
}

impl fmt::Display for ThemeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeType::Light => write!(f, "Light"),
            ThemeType::Dark => write!(f, "Dark"),
            ThemeType::HighContrast => write!(f, "High Contrast"),
            ThemeType::HighContrastLight => write!(f, "High Contrast Light"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenColor {
    pub scope: Vec<String>,
    pub foreground: Option<String>,
    pub font_style: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ColorTheme {
    pub id: String,
    pub label: String,
    pub theme_type: ThemeType,
    pub colors: HashMap<String, String>,
    pub token_colors: Vec<TokenColor>,
}

impl fmt::Display for ColorTheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.label, self.theme_type)
    }
}

impl ColorTheme {
    /// Find the first token color whose scopes contain the given value.
    pub fn get_token_color(&self, scope: &str) -> Option<&TokenColor> {
        self.token_colors.iter().find(|tc| tc.scope.iter().any(|s| s == scope))
    }

    /// Set or override a color value in the theme.
    pub fn set_color(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.colors.insert(key.into(), value.into());
    }

    /// Returns true if the theme type is Dark or HighContrast.
    pub fn is_dark(&self) -> bool {
        matches!(self.theme_type, ThemeType::Dark | ThemeType::HighContrast)
    }
}

/// Summary information about a theme.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSummary {
    pub id: String,
    pub label: String,
    pub theme_type: ThemeType,
}

/// Callback type for theme change events.
type ThemeChangeCallback = Box<dyn Fn(&ColorTheme)>;

/// Service for theme management.
pub struct ThemeService {
    themes: Vec<ColorTheme>,
    active_theme: Option<usize>,
    change_listeners: Vec<ThemeChangeCallback>,
}

impl ThemeService {
    pub fn new() -> Self {
        Self {
            themes: Vec::new(),
            active_theme: None,
            change_listeners: Vec::new(),
        }
    }

    /// Create a `ThemeService` pre-loaded with all built-in themes from
    /// `vsedit-theme`.
    pub fn with_builtins() -> Self {
        let mut svc = Self::new();
        for theme in vsedit_theme::builtin_themes() {
            svc.register_core_theme(theme);
        }
        svc
    }

    pub fn register_theme(&mut self, theme: ColorTheme) {
        self.themes.push(theme);
    }

    /// Register a theme from `vsedit-theme::ColorTheme`, converting to the
    /// local `ColorTheme` type.
    pub fn register_core_theme(&mut self, core: vsedit_theme::ColorTheme) {
        let theme = convert_core_theme(core);
        self.themes.push(theme);
    }

    pub fn set_active(&mut self, id: &str) -> bool {
        if let Some(idx) = self.themes.iter().position(|t| t.id == id) {
            self.active_theme = Some(idx);
            self.fire_change();
            true
        } else {
            false
        }
    }

    /// Set the active theme, returning an error if not found.
    pub fn set_theme(&mut self, id: &str) -> Result<(), ThemeError> {
        if self.set_active(id) {
            Ok(())
        } else {
            Err(ThemeError::ThemeNotFound(id.to_string()))
        }
    }

    pub fn get_active(&self) -> Option<&ColorTheme> {
        self.active_theme.and_then(|i| self.themes.get(i))
    }

    pub fn get_color(&self, key: &str) -> Option<&str> {
        self.get_active()
            .and_then(|t| t.colors.get(key))
            .map(|s| s.as_str())
    }

    pub fn get_themes_by_type(&self, theme_type: &ThemeType) -> Vec<&ColorTheme> {
        self.themes.iter().filter(|t| &t.theme_type == theme_type).collect()
    }

    pub fn theme_count(&self) -> usize {
        self.themes.len()
    }

    /// Register a theme, returning an error if a theme with the same id exists.
    pub fn try_register(&mut self, theme: ColorTheme) -> Result<(), ThemeError> {
        if self.themes.iter().any(|t| t.id == theme.id) {
            return Err(ThemeError::DuplicateTheme(theme.id));
        }
        self.themes.push(theme);
        Ok(())
    }

    /// Remove a theme by id. Returns the removed theme or an error.
    pub fn unregister(&mut self, id: &str) -> Result<ColorTheme, ThemeError> {
        let idx = self.themes.iter().position(|t| t.id == id)
            .ok_or_else(|| ThemeError::ThemeNotFound(id.to_string()))?;
        // If the active theme is being removed, clear it.
        if self.active_theme == Some(idx) {
            self.active_theme = None;
        } else if let Some(active) = self.active_theme {
            if active > idx {
                self.active_theme = Some(active - 1);
            }
        }
        Ok(self.themes.remove(idx))
    }

    /// Search for themes whose label contains the given substring (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&ColorTheme> {
        let q = query.to_lowercase();
        self.themes.iter().filter(|t| t.label.to_lowercase().contains(&q)).collect()
    }

    /// Get a theme by id.
    pub fn get_theme(&self, id: &str) -> Option<&ColorTheme> {
        self.themes.iter().find(|t| t.id == id)
    }

    /// Get the theme type of the currently active theme.
    pub fn active_theme_type(&self) -> Option<&ThemeType> {
        self.get_active().map(|t| &t.theme_type)
    }

    /// Return a summary list of all registered themes.
    pub fn list_themes(&self) -> Vec<ThemeSummary> {
        self.themes.iter().map(|t| ThemeSummary {
            id: t.id.clone(),
            label: t.label.clone(),
            theme_type: t.theme_type.clone(),
        }).collect()
    }

    /// Register a callback invoked whenever the active theme changes.
    pub fn on_did_change_theme<F: Fn(&ColorTheme) + 'static>(&mut self, f: F) {
        self.change_listeners.push(Box::new(f));
    }

    /// Returns `true` if the active theme is a high-contrast variant.
    pub fn is_high_contrast(&self) -> bool {
        self.get_active().map_or(false, |t| {
            matches!(t.theme_type, ThemeType::HighContrast | ThemeType::HighContrastLight)
        })
    }

    fn fire_change(&self) {
        if let Some(theme) = self.get_active() {
            for cb in &self.change_listeners {
                cb(theme);
            }
        }
    }
}

impl Default for ThemeService {
    fn default() -> Self {
        Self::new()
    }
}

/// A validated color value in hex format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorValue {
    pub hex: String,
}

impl ColorValue {
    /// Create a new `ColorValue` from a hex string.
    ///
    /// The string must start with '#' and be either 4 characters (`#RGB`)
    /// or 7 characters (`#RRGGBB`). Each character after the '#' must be
    /// a valid hexadecimal digit.
    pub fn new(hex: impl Into<String>) -> Result<Self, ThemeError> {
        let hex = hex.into();
        if !hex.starts_with('#') {
            return Err(ThemeError::InvalidColor(
                format!("color must start with '#': {}", hex),
            ));
        }
        let digits = &hex[1..];
        if digits.len() != 3 && digits.len() != 6 {
            return Err(ThemeError::InvalidColor(
                format!("color must have 3 or 6 hex digits after '#': {}", hex),
            ));
        }
        if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ThemeError::InvalidColor(
                format!("color contains non-hex characters: {}", hex),
            ));
        }
        Ok(Self { hex })
    }

    /// Parse the red component from a 7-character hex color.
    ///
    /// For short-form colors (`#RGB`) the digit is expanded (e.g. `A` → `AA`).
    pub fn red(&self) -> u8 {
        if self.hex.len() == 7 {
            u8::from_str_radix(&self.hex[1..3], 16).unwrap_or(0)
        } else {
            let ch = &self.hex[1..2];
            let expanded = format!("{}{}", ch, ch);
            u8::from_str_radix(&expanded, 16).unwrap_or(0)
        }
    }

    /// Parse the green component from a 7-character hex color.
    ///
    /// For short-form colors (`#RGB`) the digit is expanded (e.g. `B` → `BB`).
    pub fn green(&self) -> u8 {
        if self.hex.len() == 7 {
            u8::from_str_radix(&self.hex[3..5], 16).unwrap_or(0)
        } else {
            let ch = &self.hex[2..3];
            let expanded = format!("{}{}", ch, ch);
            u8::from_str_radix(&expanded, 16).unwrap_or(0)
        }
    }

    /// Parse the blue component from a 7-character hex color.
    ///
    /// For short-form colors (`#RGB`) the digit is expanded (e.g. `C` → `CC`).
    pub fn blue(&self) -> u8 {
        if self.hex.len() == 7 {
            u8::from_str_radix(&self.hex[5..7], 16).unwrap_or(0)
        } else {
            let ch = &self.hex[3..4];
            let expanded = format!("{}{}", ch, ch);
            u8::from_str_radix(&expanded, 16).unwrap_or(0)
        }
    }

    /// Returns `true` when the perceived brightness of the color exceeds 128.
    ///
    /// Uses the formula `(r*299 + g*587 + b*114) / 1000`.
    pub fn is_light(&self) -> bool {
        let r = self.red() as u32;
        let g = self.green() as u32;
        let b = self.blue() as u32;
        let brightness = (r * 299 + g * 587 + b * 114) / 1000;
        brightness > 128
    }

    /// Return the color as an `(R, G, B)` tuple.
    pub fn to_rgb_tuple(&self) -> (u8, u8, u8) {
        (self.red(), self.green(), self.blue())
    }
}

impl fmt::Display for ColorValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.hex)
    }
}

/// Utilities for merging theme data.
pub struct ThemeMerger;

impl ThemeMerger {
    /// Merge two color maps. Values in `overlay` take precedence over `base`.
    pub fn merge_colors(
        base: &HashMap<String, String>,
        overlay: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut merged = base.clone();
        for (key, value) in overlay {
            merged.insert(key.clone(), value.clone());
        }
        merged
    }

    /// Merge two token-color slices by concatenating them, with `overlay`
    /// entries appended after `base`.
    pub fn merge_token_colors(
        base: &[TokenColor],
        overlay: &[TokenColor],
    ) -> Vec<TokenColor> {
        let mut merged = base.to_vec();
        merged.extend_from_slice(overlay);
        merged
    }
}

impl ColorTheme {
    /// Returns the number of entries in the color map.
    pub fn color_count(&self) -> usize {
        self.colors.len()
    }

    /// Returns the number of token-color entries.
    pub fn token_color_count(&self) -> usize {
        self.token_colors.len()
    }

    /// Returns `true` if the color map contains the given key.
    pub fn has_color(&self, key: &str) -> bool {
        self.colors.contains_key(key)
    }

    /// Remove a color entry by key, returning the value if it existed.
    pub fn remove_color(&mut self, key: &str) -> Option<String> {
        self.colors.remove(key)
    }

    /// Return a sorted list of all color keys.
    pub fn color_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.colors.keys().map(|k| k.as_str()).collect();
        keys.sort();
        keys
    }
}

// ---------------------------------------------------------------------------
// ThemeColorMap — resolving semantic token colors
// ---------------------------------------------------------------------------

/// Maps semantic token types to resolved colors from a theme.
///
/// Provides fast lookup of foreground colors for tokens like "keyword",
/// "string", "comment", etc.
#[derive(Debug, Clone)]
pub struct ThemeColorMap {
    token_map: HashMap<String, String>,
}

impl ThemeColorMap {
    /// Build a color map from a theme's token colors.
    ///
    /// For each token color entry, every scope gets mapped to the foreground color.
    pub fn from_theme(theme: &ColorTheme) -> Self {
        let mut map = HashMap::new();
        for tc in &theme.token_colors {
            if let Some(ref fg) = tc.foreground {
                for scope in &tc.scope {
                    map.insert(scope.clone(), fg.clone());
                }
            }
        }
        Self { token_map: map }
    }

    /// Look up the foreground color for a token scope.
    pub fn get_color(&self, scope: &str) -> Option<&str> {
        self.token_map.get(scope).map(|s| s.as_str())
    }

    /// Look up a color, trying each scope in order until one matches.
    pub fn resolve_scopes(&self, scopes: &[&str]) -> Option<&str> {
        for scope in scopes {
            if let Some(color) = self.get_color(scope) {
                return Some(color);
            }
        }
        None
    }

    /// Returns the number of scope-to-color mappings.
    pub fn len(&self) -> usize {
        self.token_map.len()
    }

    /// Returns `true` if there are no mappings.
    pub fn is_empty(&self) -> bool {
        self.token_map.is_empty()
    }

    /// Returns all mapped scope names.
    pub fn scopes(&self) -> Vec<&str> {
        let mut s: Vec<&str> = self.token_map.keys().map(|k| k.as_str()).collect();
        s.sort();
        s
    }
}

// ---------------------------------------------------------------------------
// theme_contrast_ratio — WCAG contrast ratio calculation
// ---------------------------------------------------------------------------

/// Compute the relative luminance of a color (sRGB) per WCAG 2.0.
///
/// The input is a `ColorValue`. Returns a value between 0.0 (black) and 1.0 (white).
pub fn relative_luminance(color: &ColorValue) -> f64 {
    fn linearize(channel: u8) -> f64 {
        let c = channel as f64 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let r = linearize(color.red());
    let g = linearize(color.green());
    let b = linearize(color.blue());
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Compute the WCAG 2.0 contrast ratio between two colors.
///
/// Returns a value between 1.0 (no contrast) and 21.0 (maximum contrast).
pub fn theme_contrast_ratio(fg: &ColorValue, bg: &ColorValue) -> f64 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let lighter = l1.max(l2);
    let darker = l1.min(l2);
    (lighter + 0.05) / (darker + 0.05)
}

/// Check if the contrast ratio meets WCAG AA requirements for normal text (≥ 4.5).
pub fn meets_wcag_aa(fg: &ColorValue, bg: &ColorValue) -> bool {
    theme_contrast_ratio(fg, bg) >= 4.5
}

/// Check if the contrast ratio meets WCAG AAA requirements for normal text (≥ 7.0).
pub fn meets_wcag_aaa(fg: &ColorValue, bg: &ColorValue) -> bool {
    theme_contrast_ratio(fg, bg) >= 7.0
}

// ---------------------------------------------------------------------------
// color_blend — alpha compositing
// ---------------------------------------------------------------------------

/// Blend a foreground color over a background color using alpha compositing.
///
/// `alpha` is a value from 0.0 (fully transparent) to 1.0 (fully opaque).
/// Returns the blended color as a `ColorValue`.
pub fn color_blend(fg: &ColorValue, bg: &ColorValue, alpha: f64) -> ColorValue {
    let alpha = alpha.clamp(0.0, 1.0);
    let blend = |f: u8, b: u8| -> u8 {
        let result = (f as f64) * alpha + (b as f64) * (1.0 - alpha);
        result.round().clamp(0.0, 255.0) as u8
    };
    let r = blend(fg.red(), bg.red());
    let g = blend(fg.green(), bg.green());
    let b = blend(fg.blue(), bg.blue());
    let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
    ColorValue::new(hex).unwrap()
}

/// Blend two colors at 50% (simple average).
pub fn color_mix(a: &ColorValue, b: &ColorValue) -> ColorValue {
    color_blend(a, b, 0.5)
}

/// Lighten a color by blending with white.
pub fn color_lighten(color: &ColorValue, amount: f64) -> ColorValue {
    let white = ColorValue::new("#ffffff").unwrap();
    color_blend(&white, color, amount)
}

/// Darken a color by blending with black.
pub fn color_darken(color: &ColorValue, amount: f64) -> ColorValue {
    let black = ColorValue::new("#000000").unwrap();
    color_blend(&black, color, amount)
}

/// Convert a `vsedit_theme::ColorTheme` into the local `ColorTheme`.
fn convert_core_theme(core: vsedit_theme::ColorTheme) -> ColorTheme {
    let colors: HashMap<String, String> = core.colors.iter()
        .map(|(k, v)| (k.clone(), v.to_hex()))
        .collect();

    let token_colors: Vec<TokenColor> = core.token_colors.iter()
        .map(|tc| TokenColor {
            scope: tc.scope.clone(),
            foreground: tc.settings.foreground.map(|c| c.to_hex()),
            font_style: tc.settings.font_style.clone(),
        })
        .collect();

    let theme_type = match core.theme_type {
        vsedit_theme::ThemeType::Light => ThemeType::Light,
        vsedit_theme::ThemeType::Dark => ThemeType::Dark,
        vsedit_theme::ThemeType::HighContrast => ThemeType::HighContrast,
        vsedit_theme::ThemeType::HighContrastLight => ThemeType::HighContrastLight,
    };

    ColorTheme {
        id: core.id,
        label: core.label,
        theme_type,
        colors,
        token_colors,
    }
}

/// Result of comparing two themes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeComparison {
    /// Color keys present in `b` but not in `a`.
    pub added_colors: Vec<String>,
    /// Color keys present in `a` but not in `b`.
    pub removed_colors: Vec<String>,
    /// Color keys present in both but with different values.
    pub changed_colors: Vec<String>,
}

impl ThemeComparison {
    /// Compare theme `a` against theme `b` and return the differences.
    pub fn compare(a: &ColorTheme, b: &ColorTheme) -> Self {
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();
        for key in a.colors.keys() {
            match b.colors.get(key) {
                None => removed.push(key.clone()),
                Some(bv) if bv != a.colors.get(key).unwrap() => changed.push(key.clone()),
                _ => {}
            }
        }
        for key in b.colors.keys() {
            if !a.colors.contains_key(key) {
                added.push(key.clone());
            }
        }
        added.sort();
        removed.sort();
        changed.sort();
        ThemeComparison { added_colors: added, removed_colors: removed, changed_colors: changed }
    }
}

// ---------------------------------------------------------------------------
// ColorValue helpers
// ---------------------------------------------------------------------------

impl ColorValue {
    /// Compute the relative luminance per WCAG 2.0.
    pub fn luminance(&self) -> f64 {
        relative_luminance(self)
    }

    /// Compute the WCAG 2.0 contrast ratio against another color.
    pub fn contrast_ratio(&self, other: &ColorValue) -> f64 {
        theme_contrast_ratio(self, other)
    }

    /// Format the color as a normalized 7-character hex string (e.g. "#ff0000").
    pub fn to_hex_string(&self) -> String {
        if self.hex.len() == 4 {
            // Expand #RGB to #RRGGBB
            let chars: Vec<char> = self.hex.chars().collect();
            format!(
                "#{0}{0}{1}{1}{2}{2}",
                chars[1], chars[2], chars[3]
            )
        } else {
            self.hex.to_lowercase()
        }
    }

    /// Create a ColorValue from individual R, G, B components.
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
        Self { hex }
    }
}

// ---------------------------------------------------------------------------
// ThemeType helpers
// ---------------------------------------------------------------------------

impl ThemeType {
    /// Returns true for Dark and HighContrast types.
    pub fn is_dark(&self) -> bool {
        matches!(self, Self::Dark | Self::HighContrast)
    }

    /// Returns true for Light and HighContrastLight types.
    pub fn is_light(&self) -> bool {
        matches!(self, Self::Light | Self::HighContrastLight)
    }
}

// ---------------------------------------------------------------------------
// ColorTheme helpers
// ---------------------------------------------------------------------------

impl ColorTheme {
    /// Returns true if the theme type is Light or HighContrastLight.
    pub fn is_light(&self) -> bool {
        self.theme_type.is_light()
    }
}

// ---------------------------------------------------------------------------
// ThemeService helpers
// ---------------------------------------------------------------------------

impl ThemeService {
    /// Return all dark themes (Dark or HighContrast).
    pub fn dark_themes(&self) -> Vec<&ColorTheme> {
        self.themes.iter().filter(|t| t.theme_type.is_dark()).collect()
    }

    /// Return all light themes (Light or HighContrastLight).
    pub fn light_themes(&self) -> Vec<&ColorTheme> {
        self.themes.iter().filter(|t| t.theme_type.is_light()).collect()
    }

    /// Return a sorted list of all theme IDs.
    pub fn all_theme_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.themes.iter().map(|t| t.id.as_str()).collect();
        ids.sort();
        ids
    }
}

impl fmt::Display for ThemeService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let active = self
            .get_active()
            .map(|t| t.label.as_str())
            .unwrap_or("none");
        write!(
            f,
            "ThemeService({} themes, active={})",
            self.themes.len(),
            active
        )
    }
}

// ---------------------------------------------------------------------------
// ColorValidator — validate and convert hex color strings
// ---------------------------------------------------------------------------

/// Validates hex color strings and converts between formats.
///
/// Supports `#RGB`, `#RRGGBB`, and `#RRGGBBAA` formats.
#[derive(Debug, Clone)]
pub struct ColorValidator;

impl ColorValidator {
    /// Validate a hex color string.
    ///
    /// Accepted formats: `#RGB`, `#RRGGBB`, `#RRGGBBAA`.
    pub fn validate(hex: &str) -> Result<(), ThemeError> {
        if !hex.starts_with('#') {
            return Err(ThemeError::InvalidColor(
                format!("color must start with '#': {}", hex),
            ));
        }
        let digits = &hex[1..];
        match digits.len() {
            3 | 6 | 8 => {}
            _ => {
                return Err(ThemeError::InvalidColor(
                    format!("color must have 3, 6, or 8 hex digits after '#': {}", hex),
                ));
            }
        }
        if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ThemeError::InvalidColor(
                format!("color contains non-hex characters: {}", hex),
            ));
        }
        Ok(())
    }

    /// Return `true` if the string is a valid hex color.
    pub fn is_valid(hex: &str) -> bool {
        Self::validate(hex).is_ok()
    }

    /// Normalize a color to lowercase `#rrggbb` (discarding alpha if present).
    ///
    /// Short-form `#RGB` is expanded to `#RRGGBB`.
    pub fn normalize(hex: &str) -> Result<String, ThemeError> {
        Self::validate(hex)?;
        let digits = &hex[1..];
        match digits.len() {
            3 => {
                let chars: Vec<char> = digits.chars().collect();
                Ok(format!(
                    "#{0}{0}{1}{1}{2}{2}",
                    chars[0].to_ascii_lowercase(),
                    chars[1].to_ascii_lowercase(),
                    chars[2].to_ascii_lowercase(),
                ))
            }
            6 => Ok(format!("#{}", digits.to_ascii_lowercase())),
            8 => Ok(format!("#{}", digits[..6].to_ascii_lowercase())),
            _ => unreachable!(),
        }
    }

    /// Extract the alpha component from a `#RRGGBBAA` color (0–255).
    ///
    /// Returns 255 (fully opaque) for colors without an alpha channel.
    pub fn alpha(hex: &str) -> Result<u8, ThemeError> {
        Self::validate(hex)?;
        let digits = &hex[1..];
        if digits.len() == 8 {
            u8::from_str_radix(&digits[6..8], 16)
                .map_err(|_| ThemeError::InvalidColor(hex.to_string()))
        } else {
            Ok(255)
        }
    }

    /// Convert a `#RRGGBB` color to `#RRGGBBAA` by appending an alpha value.
    pub fn with_alpha(hex: &str, alpha: u8) -> Result<String, ThemeError> {
        let normalized = Self::normalize(hex)?;
        Ok(format!("{}{:02x}", normalized, alpha))
    }
}

// ---------------------------------------------------------------------------
// ThemeDiff — detailed comparison of two themes
// ---------------------------------------------------------------------------

/// A single difference between two theme values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffEntry {
    Added { key: String, value: String },
    Removed { key: String, value: String },
    Changed { key: String, old: String, new: String },
}

impl fmt::Display for DiffEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffEntry::Added { key, value } => write!(f, "+ {}: {}", key, value),
            DiffEntry::Removed { key, value } => write!(f, "- {}: {}", key, value),
            DiffEntry::Changed { key, old, new } => {
                write!(f, "~ {}: {} -> {}", key, old, new)
            }
        }
    }
}

/// Detailed diff between two themes, covering both colors and token colors.
#[derive(Debug, Clone)]
pub struct ThemeDiff {
    pub color_diffs: Vec<DiffEntry>,
    pub token_scope_diffs: Vec<DiffEntry>,
}

impl ThemeDiff {
    /// Compare two themes and produce a detailed diff.
    pub fn diff(a: &ColorTheme, b: &ColorTheme) -> Self {
        let mut color_diffs = Vec::new();

        // Color map diffs
        for (key, val_a) in &a.colors {
            match b.colors.get(key) {
                None => color_diffs.push(DiffEntry::Removed {
                    key: key.clone(),
                    value: val_a.clone(),
                }),
                Some(val_b) if val_b != val_a => color_diffs.push(DiffEntry::Changed {
                    key: key.clone(),
                    old: val_a.clone(),
                    new: val_b.clone(),
                }),
                _ => {}
            }
        }
        for (key, val_b) in &b.colors {
            if !a.colors.contains_key(key) {
                color_diffs.push(DiffEntry::Added {
                    key: key.clone(),
                    value: val_b.clone(),
                });
            }
        }
        color_diffs.sort_by(|x, y| {
            let k = |e: &DiffEntry| match e {
                DiffEntry::Added { key, .. }
                | DiffEntry::Removed { key, .. }
                | DiffEntry::Changed { key, .. } => key.clone(),
            };
            k(x).cmp(&k(y))
        });

        // Token-color scope diffs: build scope→foreground maps
        let scope_map = |tc_list: &[TokenColor]| -> HashMap<String, String> {
            let mut m = HashMap::new();
            for tc in tc_list {
                if let Some(ref fg) = tc.foreground {
                    for s in &tc.scope {
                        m.insert(s.clone(), fg.clone());
                    }
                }
            }
            m
        };
        let sa = scope_map(&a.token_colors);
        let sb = scope_map(&b.token_colors);
        let mut token_scope_diffs = Vec::new();
        for (scope, fg_a) in &sa {
            match sb.get(scope) {
                None => token_scope_diffs.push(DiffEntry::Removed {
                    key: scope.clone(),
                    value: fg_a.clone(),
                }),
                Some(fg_b) if fg_b != fg_a => token_scope_diffs.push(DiffEntry::Changed {
                    key: scope.clone(),
                    old: fg_a.clone(),
                    new: fg_b.clone(),
                }),
                _ => {}
            }
        }
        for (scope, fg_b) in &sb {
            if !sa.contains_key(scope) {
                token_scope_diffs.push(DiffEntry::Added {
                    key: scope.clone(),
                    value: fg_b.clone(),
                });
            }
        }
        token_scope_diffs.sort_by(|x, y| {
            let k = |e: &DiffEntry| match e {
                DiffEntry::Added { key, .. }
                | DiffEntry::Removed { key, .. }
                | DiffEntry::Changed { key, .. } => key.clone(),
            };
            k(x).cmp(&k(y))
        });

        Self { color_diffs, token_scope_diffs }
    }

    /// Returns true when the two themes are identical.
    pub fn is_empty(&self) -> bool {
        self.color_diffs.is_empty() && self.token_scope_diffs.is_empty()
    }

    /// Total number of differences.
    pub fn len(&self) -> usize {
        self.color_diffs.len() + self.token_scope_diffs.len()
    }
}

impl fmt::Display for ThemeDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "no differences");
        }
        if !self.color_diffs.is_empty() {
            writeln!(f, "Colors:")?;
            for d in &self.color_diffs {
                writeln!(f, "  {}", d)?;
            }
        }
        if !self.token_scope_diffs.is_empty() {
            writeln!(f, "Token scopes:")?;
            for d in &self.token_scope_diffs {
                writeln!(f, "  {}", d)?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ThemeInheritance — child theme inheriting from a parent
// ---------------------------------------------------------------------------

/// Represents a child theme that inherits from a parent, overriding specific
/// colors and token colors.
#[derive(Debug, Clone)]
pub struct ThemeInheritance {
    pub parent: ColorTheme,
    pub overrides: ColorTheme,
}

impl ThemeInheritance {
    pub fn new(parent: ColorTheme, overrides: ColorTheme) -> Self {
        Self { parent, overrides }
    }

    /// Resolve the inheritance into a single `ColorTheme`.
    ///
    /// The child's id, label, and theme_type are used. Colors and token colors
    /// from the parent are merged with the child's overrides taking precedence.
    pub fn resolve(&self) -> ColorTheme {
        let colors = ThemeMerger::merge_colors(&self.parent.colors, &self.overrides.colors);
        let token_colors =
            ThemeMerger::merge_token_colors(&self.parent.token_colors, &self.overrides.token_colors);
        ColorTheme {
            id: self.overrides.id.clone(),
            label: self.overrides.label.clone(),
            theme_type: self.overrides.theme_type.clone(),
            colors,
            token_colors,
        }
    }

    /// Return the set of color keys that the child overrides.
    pub fn overridden_color_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self
            .overrides
            .colors
            .keys()
            .filter(|k| self.parent.colors.contains_key(*k))
            .cloned()
            .collect();
        keys.sort();
        keys
    }

    /// Return the set of color keys that the child adds (not present in parent).
    pub fn added_color_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self
            .overrides
            .colors
            .keys()
            .filter(|k| !self.parent.colors.contains_key(*k))
            .cloned()
            .collect();
        keys.sort();
        keys
    }
}

impl fmt::Display for ThemeInheritance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (inherits from {})",
            self.overrides.label, self.parent.label
        )
    }
}

// ---------------------------------------------------------------------------
// TokenColorMatcher — scope matching with prefix / glob patterns
// ---------------------------------------------------------------------------

/// Matches token scopes using prefix or simple glob patterns.
///
/// Patterns:
/// - `"keyword"` matches exactly `"keyword"`.
/// - `"keyword.*"` matches any scope starting with `"keyword."`.
/// - `"*"` matches everything.
#[derive(Debug, Clone)]
pub struct TokenColorMatcher {
    rules: Vec<(String, String)>, // (pattern, foreground)
}

impl TokenColorMatcher {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a rule mapping a pattern to a foreground color.
    pub fn add_rule(&mut self, pattern: impl Into<String>, foreground: impl Into<String>) {
        self.rules.push((pattern.into(), foreground.into()));
    }

    /// Build a matcher from a theme's token colors.
    ///
    /// Each scope in each token color entry becomes a rule.
    pub fn from_theme(theme: &ColorTheme) -> Self {
        let mut matcher = Self::new();
        for tc in &theme.token_colors {
            if let Some(ref fg) = tc.foreground {
                for scope in &tc.scope {
                    matcher.add_rule(scope.clone(), fg.clone());
                }
            }
        }
        matcher
    }

    /// Match a scope string against the rules.
    ///
    /// Returns the foreground color of the best matching rule. Exact matches
    /// are preferred over prefix/glob matches; among the same kind, longer
    /// patterns win.
    pub fn match_scope(&self, scope: &str) -> Option<&str> {
        // (is_exact, pattern_len)
        let mut best: Option<(bool, usize, &str)> = None;
        for (pattern, fg) in &self.rules {
            let (matched, is_exact) = if pattern == "*" {
                (true, false)
            } else if let Some(prefix) = pattern.strip_suffix(".*") {
                let m = scope == prefix || scope.starts_with(&format!("{}.", prefix));
                (m, false)
            } else {
                (scope == pattern, true)
            };
            if matched {
                let dominated = match best {
                    None => true,
                    Some((prev_exact, prev_len, _)) => {
                        (is_exact && !prev_exact)
                            || (is_exact == prev_exact && pattern.len() > prev_len)
                    }
                };
                if dominated {
                    best = Some((is_exact, pattern.len(), fg.as_str()));
                }
            }
        }
        best.map(|(_, _, fg)| fg)
    }

    /// Returns the number of rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for TokenColorMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TokenColorMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TokenColorMatcher({} rules)", self.rules.len())
    }
}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

impl From<(u8, u8, u8)> for ColorValue {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self::from_rgb(r, g, b)
    }
}

impl From<&ColorTheme> for ThemeSummary {
    fn from(theme: &ColorTheme) -> Self {
        Self {
            id: theme.id.clone(),
            label: theme.label.clone(),
            theme_type: theme.theme_type.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// ThemePreview – generate color swatches for a theme
// ---------------------------------------------------------------------------

/// A single color swatch entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorSwatch {
    pub key: String,
    pub hex_value: String,
    pub label: String,
}

impl ColorSwatch {
    /// Render a swatch as `"key: #hex"`.
    pub fn render(&self) -> String {
        format!("{}: {}", self.key, self.hex_value)
    }
}

impl fmt::Display for ColorSwatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.label, self.hex_value)
    }
}

/// Generates preview swatches for theme colors.
#[derive(Debug)]
pub struct ThemePreview;

impl ThemePreview {
    pub fn new() -> Self {
        Self
    }

    /// Build swatches for the given color keys that exist in the theme.
    pub fn generate_swatches(theme: &ColorTheme, keys: &[&str]) -> Vec<ColorSwatch> {
        keys.iter()
            .filter_map(|&k| {
                theme.colors.get(k).map(|hex| ColorSwatch {
                    key: k.to_string(),
                    hex_value: hex.clone(),
                    label: k.to_string(),
                })
            })
            .collect()
    }

    /// Render a multi-line preview string from a slice of swatches.
    pub fn render_preview(swatches: &[ColorSwatch]) -> String {
        if swatches.is_empty() {
            return String::from("(no swatches)");
        }
        swatches
            .iter()
            .map(|s| s.render())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Return the total number of color entries in a theme.
    pub fn swatch_count(theme: &ColorTheme) -> usize {
        theme.colors.len()
    }
}

// ---------------------------------------------------------------------------
// ThemeMigrator – migrate deprecated color keys
// ---------------------------------------------------------------------------

/// Describes a single key migration.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorMigration {
    pub old_key: String,
    pub new_key: String,
    pub transform: Option<String>,
}

impl fmt::Display for ColorMigration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.transform {
            Some(t) => write!(f, "{} -> {} ({})", self.old_key, self.new_key, t),
            None => write!(f, "{} -> {}", self.old_key, self.new_key),
        }
    }
}

/// Migrates deprecated colour keys to their replacements.
#[derive(Debug, Clone)]
pub struct ThemeMigrator {
    pub migrations: Vec<ColorMigration>,
}

impl ThemeMigrator {
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }

    /// Register a migration from `old` to `new`.
    pub fn add_migration(&mut self, old: &str, new: &str) {
        self.migrations.push(ColorMigration {
            old_key: old.to_string(),
            new_key: new.to_string(),
            transform: None,
        });
    }

    /// Apply all registered migrations to `theme`, moving values from old keys
    /// to new keys (only when the old key is present and the new key is absent).
    /// Returns the number of migrations actually applied.
    pub fn migrate_theme(&self, theme: &mut ColorTheme) -> usize {
        let mut count = 0;
        for m in &self.migrations {
            if theme.colors.contains_key(&m.old_key) && !theme.colors.contains_key(&m.new_key) {
                if let Some(val) = theme.colors.remove(&m.old_key) {
                    theme.colors.insert(m.new_key.clone(), val);
                    count += 1;
                }
            }
        }
        count
    }

    /// Return the list of deprecated keys that are still present in `theme`.
    pub fn has_deprecated_keys(&self, theme: &ColorTheme) -> Vec<String> {
        self.migrations
            .iter()
            .filter(|m| theme.colors.contains_key(&m.old_key))
            .map(|m| m.old_key.clone())
            .collect()
    }

    /// Number of registered migrations.
    pub fn migration_count(&self) -> usize {
        self.migrations.len()
    }
}

impl fmt::Display for ThemeMigrator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ThemeMigrator({} migrations)", self.migrations.len())
    }
}

// ---------------------------------------------------------------------------
// ThemeContributionMerger – merge prioritised colour contributions
// ---------------------------------------------------------------------------

/// A single theme contribution from an extension or module.
#[derive(Debug, Clone)]
pub struct ThemeContribution {
    pub source: String,
    pub colors: HashMap<String, String>,
    pub priority: u32,
}

impl fmt::Display for ThemeContribution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({} colors, priority {})",
            self.source,
            self.colors.len(),
            self.priority,
        )
    }
}

/// Merges a base colour map with prioritised contributions.
#[derive(Debug, Clone)]
pub struct ThemeContributionMerger {
    pub base_colors: HashMap<String, String>,
    pub contributions: Vec<ThemeContribution>,
}

impl ThemeContributionMerger {
    pub fn new() -> Self {
        Self {
            base_colors: HashMap::new(),
            contributions: Vec::new(),
        }
    }

    /// Set the base colours (lowest priority).
    pub fn set_base(&mut self, colors: HashMap<String, String>) {
        self.base_colors = colors;
    }

    /// Add a contribution with a given priority.
    pub fn add_contribution(
        &mut self,
        source: &str,
        colors: HashMap<String, String>,
        priority: u32,
    ) {
        self.contributions.push(ThemeContribution {
            source: source.to_string(),
            colors,
            priority,
        });
    }

    /// Merge all contributions on top of the base. Higher priority wins.
    pub fn merge(&self) -> HashMap<String, String> {
        let mut result = self.base_colors.clone();
        let mut sorted: Vec<&ThemeContribution> = self.contributions.iter().collect();
        sorted.sort_by_key(|c| c.priority);
        for contrib in sorted {
            for (k, v) in &contrib.colors {
                result.insert(k.clone(), v.clone());
            }
        }
        result
    }

    /// Return keys that appear in more than one contribution.
    pub fn conflict_keys(&self) -> Vec<String> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for contrib in &self.contributions {
            for k in contrib.colors.keys() {
                *counts.entry(k.as_str()).or_insert(0) += 1;
            }
        }
        let mut keys: Vec<String> = counts
            .into_iter()
            .filter(|&(_, c)| c > 1)
            .map(|(k, _)| k.to_string())
            .collect();
        keys.sort();
        keys
    }

    /// Number of registered contributions.
    pub fn contribution_count(&self) -> usize {
        self.contributions.len()
    }
}

impl fmt::Display for ThemeContributionMerger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ThemeContributionMerger({} base, {} contributions)",
            self.base_colors.len(),
            self.contributions.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// ThemeIconMapper – map file types to icons
// ---------------------------------------------------------------------------

/// Maps file-type identifiers to icon representations.
#[derive(Debug, Clone)]
pub struct ThemeIconMapper {
    pub icon_map: HashMap<String, String>,
}

impl ThemeIconMapper {
    pub fn new() -> Self {
        Self {
            icon_map: HashMap::new(),
        }
    }

    /// Create a mapper pre-populated with common defaults.
    pub fn with_defaults() -> Self {
        let mut m = Self::new();
        m.icon_map.insert("file".into(), "📄".into());
        m.icon_map.insert("folder".into(), "📁".into());
        m.icon_map.insert("rust".into(), "🦀".into());
        m.icon_map.insert("python".into(), "🐍".into());
        m.icon_map.insert("javascript".into(), "📜".into());
        m
    }

    /// Resolve an icon for the given file type; falls back to "📄".
    pub fn resolve_icon(&self, file_type: &str) -> &str {
        self.icon_map
            .get(file_type)
            .map(|s| s.as_str())
            .unwrap_or("📄")
    }

    /// Register a custom file-type → icon mapping.
    pub fn register(&mut self, file_type: &str, icon: &str) {
        self.icon_map
            .insert(file_type.to_string(), icon.to_string());
    }

    /// Number of registered icons.
    pub fn icon_count(&self) -> usize {
        self.icon_map.len()
    }

    /// Check whether an icon is registered for the given file type.
    pub fn has_icon(&self, file_type: &str) -> bool {
        self.icon_map.contains_key(file_type)
    }
}

impl fmt::Display for ThemeIconMapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ThemeIconMapper({} icons)", self.icon_map.len())
    }
}


// === Theme Preview Widget ===

/// Theme Preview Widget implementation.
#[derive(Debug, Clone)]
pub struct ThemePreviewWidget {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: ThemePreviewWidgetStats,
}

/// Statistics for ThemePreviewWidget.
#[derive(Debug, Clone, Default)]
pub struct ThemePreviewWidgetStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl ThemePreviewWidgetStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl ThemePreviewWidget {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: ThemePreviewWidgetStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &ThemePreviewWidgetStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for ThemePreviewWidget {
    fn default() -> Self {
        Self::new()
    }
}

// === Theme Import Handler ===

/// Priority level for ThemeImportHandler items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeImportHandlerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl ThemeImportHandlerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for ThemeImportHandlerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Theme Import Handler implementation.
#[derive(Debug, Clone)]
pub struct ThemeImportHandler {
    items: Vec<ThemeImportHandlerItem>,
    max_items: usize,
    default_priority: ThemeImportHandlerPriority,
}

/// A single item in ThemeImportHandler.
#[derive(Debug, Clone)]
pub struct ThemeImportHandlerItem {
    pub id: String,
    pub label: String,
    pub priority: ThemeImportHandlerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl ThemeImportHandlerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: ThemeImportHandlerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: ThemeImportHandlerPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl ThemeImportHandler {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: ThemeImportHandlerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: ThemeImportHandlerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<ThemeImportHandlerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&ThemeImportHandlerItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: ThemeImportHandlerPriority) -> Vec<&ThemeImportHandlerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ThemeImportHandlerItem> {
        let mut sorted: Vec<&ThemeImportHandlerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&ThemeImportHandlerItem> {
        let mut sorted: Vec<&ThemeImportHandlerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&ThemeImportHandlerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: ThemeImportHandlerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> ThemeImportHandlerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &ThemeImportHandlerItem> {
        self.items.iter()
    }
}

impl Default for ThemeImportHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dark_theme() -> ColorTheme {
        let mut colors = HashMap::new();
        colors.insert("editor.background".into(), "#1e1e1e".into());
        colors.insert("editor.foreground".into(), "#d4d4d4".into());
        ColorTheme {
            id: "dark-plus".into(),
            label: "Dark+".into(),
            theme_type: ThemeType::Dark,
            colors,
            token_colors: vec![TokenColor {
                scope: vec!["keyword".into()],
                foreground: Some("#569cd6".into()),
                font_style: None,
            }],
        }
    }

    #[test]
    fn register_and_activate() {
        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());
        assert_eq!(svc.theme_count(), 1);
        assert!(svc.get_active().is_none());
        assert!(svc.set_active("dark-plus"));
        assert_eq!(svc.get_active().unwrap().label, "Dark+");
    }

    #[test]
    fn get_color_from_active() {
        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());
        svc.set_active("dark-plus");
        assert_eq!(svc.get_color("editor.background"), Some("#1e1e1e"));
        assert!(svc.get_color("nonexistent").is_none());
    }

    #[test]
    fn filter_by_type() {
        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());
        svc.register_theme(ColorTheme {
            id: "light-plus".into(),
            label: "Light+".into(),
            theme_type: ThemeType::Light,
            colors: HashMap::new(),
            token_colors: Vec::new(),
        });
        assert_eq!(svc.get_themes_by_type(&ThemeType::Dark).len(), 1);
        assert_eq!(svc.get_themes_by_type(&ThemeType::Light).len(), 1);
        assert_eq!(svc.get_themes_by_type(&ThemeType::HighContrast).len(), 0);
    }

    #[test]
    fn set_active_nonexistent() {
        let mut svc = ThemeService::new();
        assert!(!svc.set_active("missing"));
    }

    #[test]
    fn try_register_duplicate() {
        let mut svc = ThemeService::new();
        svc.try_register(dark_theme()).unwrap();
        let err = svc.try_register(dark_theme()).unwrap_err();
        assert_eq!(err, ThemeError::DuplicateTheme("dark-plus".into()));
    }

    #[test]
    fn unregister_theme() {
        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());
        assert_eq!(svc.theme_count(), 1);
        let removed = svc.unregister("dark-plus").unwrap();
        assert_eq!(removed.id, "dark-plus");
        assert_eq!(svc.theme_count(), 0);
    }

    #[test]
    fn unregister_clears_active() {
        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());
        svc.set_active("dark-plus");
        svc.unregister("dark-plus").unwrap();
        assert!(svc.get_active().is_none());
    }

    #[test]
    fn unregister_nonexistent() {
        let mut svc = ThemeService::new();
        let err = svc.unregister("missing").unwrap_err();
        assert_eq!(err, ThemeError::ThemeNotFound("missing".into()));
    }

    #[test]
    fn search_themes() {
        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());
        svc.register_theme(ColorTheme {
            id: "light-plus".into(),
            label: "Light+".into(),
            theme_type: ThemeType::Light,
            colors: HashMap::new(),
            token_colors: Vec::new(),
        });
        assert_eq!(svc.search("dark").len(), 1);
        assert_eq!(svc.search("LIGHT").len(), 1);
        assert_eq!(svc.search("+").len(), 2);
        assert_eq!(svc.search("missing").len(), 0);
    }

    #[test]
    fn get_theme_by_id() {
        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());
        assert!(svc.get_theme("dark-plus").is_some());
        assert!(svc.get_theme("nope").is_none());
    }

    #[test]
    fn get_token_color_by_scope() {
        let theme = dark_theme();
        let tc = theme.get_token_color("keyword");
        assert!(tc.is_some());
        assert_eq!(tc.unwrap().foreground.as_deref(), Some("#569cd6"));
        assert!(theme.get_token_color("variable").is_none());
    }

    #[test]
    fn set_color_override() {
        let mut theme = dark_theme();
        theme.set_color("editor.background", "#000000");
        assert_eq!(theme.colors.get("editor.background").unwrap(), "#000000");
    }

    #[test]
    fn is_dark_check() {
        assert!(dark_theme().is_dark());
        let light = ColorTheme {
            id: "l".into(),
            label: "L".into(),
            theme_type: ThemeType::Light,
            colors: HashMap::new(),
            token_colors: Vec::new(),
        };
        assert!(!light.is_dark());
        let hc = ColorTheme {
            id: "hc".into(),
            label: "HC".into(),
            theme_type: ThemeType::HighContrast,
            colors: HashMap::new(),
            token_colors: Vec::new(),
        };
        assert!(hc.is_dark());
    }

    #[test]
    fn active_theme_type() {
        let mut svc = ThemeService::new();
        assert!(svc.active_theme_type().is_none());
        svc.register_theme(dark_theme());
        svc.set_active("dark-plus");
        assert_eq!(svc.active_theme_type(), Some(&ThemeType::Dark));
    }

    #[test]
    fn display_impls() {
        assert_eq!(format!("{}", ThemeType::Dark), "Dark");
        assert_eq!(format!("{}", ThemeType::HighContrastLight), "High Contrast Light");
        let theme = dark_theme();
        assert_eq!(format!("{}", theme), "Dark+ (Dark)");
        let err = ThemeError::ThemeNotFound("x".into());
        assert_eq!(format!("{}", err), "theme not found: x");
    }

    #[test]
    fn test_color_value_valid() {
        let color = ColorValue::new("#ff00aa").unwrap();
        assert_eq!(color.hex, "#ff00aa");

        let short = ColorValue::new("#abc").unwrap();
        assert_eq!(short.hex, "#abc");
    }

    #[test]
    fn test_color_value_invalid_no_hash() {
        let result = ColorValue::new("ff00aa");
        assert!(result.is_err());
        match result.unwrap_err() {
            ThemeError::InvalidColor(msg) => {
                assert!(msg.contains("must start with '#'"));
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_color_value_invalid_length() {
        let result = ColorValue::new("#ff00a");
        assert!(result.is_err());
        match result.unwrap_err() {
            ThemeError::InvalidColor(msg) => {
                assert!(msg.contains("3 or 6 hex digits"));
            }
            other => panic!("unexpected error: {:?}", other),
        }

        let result2 = ColorValue::new("#ff");
        assert!(result2.is_err());
    }

    #[test]
    fn test_color_value_rgb_components() {
        let color = ColorValue::new("#1e90ff").unwrap();
        assert_eq!(color.red(), 0x1e);
        assert_eq!(color.green(), 0x90);
        assert_eq!(color.blue(), 0xff);
    }

    #[test]
    fn test_color_value_is_light() {
        // White is light
        let white = ColorValue::new("#ffffff").unwrap();
        assert!(white.is_light());

        // A very light yellow is light
        let light_yellow = ColorValue::new("#ffffcc").unwrap();
        assert!(light_yellow.is_light());
    }

    #[test]
    fn test_color_value_is_dark_color() {
        // Black is not light
        let black = ColorValue::new("#000000").unwrap();
        assert!(!black.is_light());

        // A dark blue is not light
        let dark_blue = ColorValue::new("#00008b").unwrap();
        assert!(!dark_blue.is_light());
    }

    #[test]
    fn test_color_value_display() {
        let color = ColorValue::new("#abcdef").unwrap();
        assert_eq!(format!("{}", color), "#abcdef");

        let short = ColorValue::new("#abc").unwrap();
        assert_eq!(format!("{}", short), "#abc");
    }

    #[test]
    fn test_color_value_to_rgb_tuple() {
        let color = ColorValue::new("#ff8040").unwrap();
        assert_eq!(color.to_rgb_tuple(), (0xff, 0x80, 0x40));

        let black = ColorValue::new("#000000").unwrap();
        assert_eq!(black.to_rgb_tuple(), (0, 0, 0));

        let white = ColorValue::new("#ffffff").unwrap();
        assert_eq!(white.to_rgb_tuple(), (255, 255, 255));
    }

    #[test]
    fn test_merge_colors() {
        let mut base = HashMap::new();
        base.insert("editor.background".into(), "#1e1e1e".into());
        base.insert("editor.foreground".into(), "#d4d4d4".into());

        let mut overlay = HashMap::new();
        overlay.insert("editor.background".into(), "#000000".into());
        overlay.insert("statusBar.background".into(), "#007acc".into());

        let merged = ThemeMerger::merge_colors(&base, &overlay);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged.get("editor.background").unwrap(), "#000000");
        assert_eq!(merged.get("editor.foreground").unwrap(), "#d4d4d4");
        assert_eq!(merged.get("statusBar.background").unwrap(), "#007acc");
    }

    #[test]
    fn test_merge_token_colors() {
        let base = vec![TokenColor {
            scope: vec!["keyword".into()],
            foreground: Some("#569cd6".into()),
            font_style: None,
        }];
        let overlay = vec![TokenColor {
            scope: vec!["string".into()],
            foreground: Some("#ce9178".into()),
            font_style: Some("italic".into()),
        }];

        let merged = ThemeMerger::merge_token_colors(&base, &overlay);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].scope[0], "keyword");
        assert_eq!(merged[1].scope[0], "string");
        assert_eq!(merged[1].font_style.as_deref(), Some("italic"));
    }

    #[test]
    fn test_color_count() {
        let theme = dark_theme();
        assert_eq!(theme.color_count(), 2);
        assert_eq!(theme.token_color_count(), 1);
    }

    #[test]
    fn test_has_and_remove_color() {
        let mut theme = dark_theme();
        assert!(theme.has_color("editor.background"));
        assert!(!theme.has_color("statusBar.background"));

        let removed = theme.remove_color("editor.background");
        assert_eq!(removed, Some("#1e1e1e".into()));
        assert!(!theme.has_color("editor.background"));
        assert_eq!(theme.color_count(), 1);

        let removed_again = theme.remove_color("editor.background");
        assert!(removed_again.is_none());
    }

    #[test]
    fn test_color_keys_sorted() {
        let mut theme = dark_theme();
        theme.set_color("activityBar.background", "#333333");
        theme.set_color("statusBar.background", "#007acc");

        let keys = theme.color_keys();
        assert_eq!(keys.len(), 4);
        // Verify the keys are sorted alphabetically
        assert_eq!(keys[0], "activityBar.background");
        assert_eq!(keys[1], "editor.background");
        assert_eq!(keys[2], "editor.foreground");
        assert_eq!(keys[3], "statusBar.background");
    }

    // -- ThemeSummary / list_themes --

    #[test]
    fn list_themes_empty() {
        let svc = ThemeService::new();
        assert!(svc.list_themes().is_empty());
    }

    #[test]
    fn list_themes_populated() {
        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());
        svc.register_theme(ColorTheme {
            id: "light-plus".into(),
            label: "Light+".into(),
            theme_type: ThemeType::Light,
            colors: HashMap::new(),
            token_colors: Vec::new(),
        });
        let summaries = svc.list_themes();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].id, "dark-plus");
        assert_eq!(summaries[0].label, "Dark+");
        assert_eq!(summaries[0].theme_type, ThemeType::Dark);
        assert_eq!(summaries[1].id, "light-plus");
    }

    // -- set_theme --

    #[test]
    fn set_theme_ok() {
        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());
        assert!(svc.set_theme("dark-plus").is_ok());
        assert_eq!(svc.get_active().unwrap().id, "dark-plus");
    }

    #[test]
    fn set_theme_not_found() {
        let mut svc = ThemeService::new();
        let err = svc.set_theme("missing").unwrap_err();
        assert_eq!(err, ThemeError::ThemeNotFound("missing".into()));
    }

    // -- on_did_change_theme --

    #[test]
    fn on_did_change_theme_fires() {
        use std::cell::Cell;
        use std::rc::Rc;

        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());

        let called = Rc::new(Cell::new(false));
        let called_clone = called.clone();
        svc.on_did_change_theme(move |_theme| {
            called_clone.set(true);
        });

        svc.set_active("dark-plus");
        assert!(called.get());
    }

    // -- is_high_contrast --

    #[test]
    fn is_high_contrast_false() {
        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());
        svc.set_active("dark-plus");
        assert!(!svc.is_high_contrast());
    }

    #[test]
    fn is_high_contrast_true() {
        let mut svc = ThemeService::new();
        svc.register_theme(ColorTheme {
            id: "hc".into(),
            label: "HC".into(),
            theme_type: ThemeType::HighContrast,
            colors: HashMap::new(),
            token_colors: Vec::new(),
        });
        svc.set_active("hc");
        assert!(svc.is_high_contrast());
    }

    // -- with_builtins --

    #[test]
    fn with_builtins_loads_themes() {
        let svc = ThemeService::with_builtins();
        assert!(svc.theme_count() >= 6, "expected 6+ builtins, got {}", svc.theme_count());
        let summaries = svc.list_themes();
        let ids: Vec<&str> = summaries.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"vs-dark-plus"));
        assert!(ids.contains(&"vs-light-plus"));
        assert!(ids.contains(&"monokai"));
        assert!(ids.contains(&"solarized-dark"));
        assert!(ids.contains(&"hc-black"));
        assert!(ids.contains(&"hc-light"));
    }

    #[test]
    fn with_builtins_activate_and_get_color() {
        let mut svc = ThemeService::with_builtins();
        svc.set_active("vs-dark-plus");
        let bg = svc.get_color("editor.background");
        assert!(bg.is_some());
        assert_eq!(bg.unwrap(), "#1E1E1E");
    }

    #[test]
    fn with_builtins_switch_theme() {
        let mut svc = ThemeService::with_builtins();
        svc.set_theme("monokai").unwrap();
        assert_eq!(svc.get_active().unwrap().id, "monokai");
        svc.set_theme("solarized-dark").unwrap();
        assert_eq!(svc.get_active().unwrap().id, "solarized-dark");
    }

    #[test]
    fn compare_identical_themes() {
        let t = dark_theme();
        let cmp = ThemeComparison::compare(&t, &t);
        assert!(cmp.added_colors.is_empty());
        assert!(cmp.removed_colors.is_empty());
        assert!(cmp.changed_colors.is_empty());
    }

    #[test]
    fn compare_added_removed() {
        let a = dark_theme();
        let mut colors = HashMap::new();
        colors.insert("new.color".into(), "#ff0000".into());
        let b = ColorTheme { id: "b".into(), label: "B".into(), theme_type: ThemeType::Dark, colors, token_colors: vec![] };
        let cmp = ThemeComparison::compare(&a, &b);
        assert!(cmp.added_colors.contains(&"new.color".to_string()));
        assert!(!cmp.removed_colors.is_empty());
    }

    #[test]
    fn compare_changed() {
        let a = dark_theme();
        let mut b = a.clone();
        b.colors.insert("editor.background".into(), "#000000".into());
        let cmp = ThemeComparison::compare(&a, &b);
        assert!(cmp.changed_colors.contains(&"editor.background".to_string()));
    }

    // ---- ThemeColorMap tests ----

    #[test]
    fn theme_color_map_from_theme() {
        let theme = dark_theme();
        let map = ThemeColorMap::from_theme(&theme);
        assert!(!map.is_empty());
        assert_eq!(map.get_color("keyword"), Some("#569cd6"));
        assert!(map.get_color("nonexistent").is_none());
    }

    #[test]
    fn theme_color_map_resolve_scopes() {
        let theme = dark_theme();
        let map = ThemeColorMap::from_theme(&theme);
        // Try scopes in order, "variable" doesn't exist but "keyword" does
        assert_eq!(
            map.resolve_scopes(&["variable", "keyword"]),
            Some("#569cd6")
        );
        assert!(map.resolve_scopes(&["nonexistent"]).is_none());
    }

    #[test]
    fn theme_color_map_scopes_list() {
        let theme = dark_theme();
        let map = ThemeColorMap::from_theme(&theme);
        let scopes = map.scopes();
        assert!(scopes.contains(&"keyword"));
        assert_eq!(map.len(), 1);
    }

    // ---- theme_contrast_ratio tests ----

    #[test]
    fn contrast_ratio_black_white() {
        let black = ColorValue::new("#000000").unwrap();
        let white = ColorValue::new("#ffffff").unwrap();
        let ratio = theme_contrast_ratio(&black, &white);
        assert!((ratio - 21.0).abs() < 0.1); // should be ~21:1
    }

    #[test]
    fn contrast_ratio_same_color() {
        let color = ColorValue::new("#808080").unwrap();
        let ratio = theme_contrast_ratio(&color, &color);
        assert!((ratio - 1.0).abs() < 0.01); // should be ~1:1
    }

    #[test]
    fn meets_wcag_aa_check() {
        let black = ColorValue::new("#000000").unwrap();
        let white = ColorValue::new("#ffffff").unwrap();
        assert!(meets_wcag_aa(&black, &white));
        assert!(meets_wcag_aaa(&black, &white));

        // Similar grays won't pass
        let gray1 = ColorValue::new("#808080").unwrap();
        let gray2 = ColorValue::new("#909090").unwrap();
        assert!(!meets_wcag_aa(&gray1, &gray2));
    }

    #[test]
    fn relative_luminance_extremes() {
        let black = ColorValue::new("#000000").unwrap();
        let white = ColorValue::new("#ffffff").unwrap();
        assert!(relative_luminance(&black) < 0.01);
        assert!((relative_luminance(&white) - 1.0).abs() < 0.01);
    }

    // ---- color_blend tests ----

    #[test]
    fn blend_fully_opaque() {
        let fg = ColorValue::new("#ff0000").unwrap();
        let bg = ColorValue::new("#0000ff").unwrap();
        let result = color_blend(&fg, &bg, 1.0);
        assert_eq!(result.red(), 255);
        assert_eq!(result.green(), 0);
        assert_eq!(result.blue(), 0);
    }

    #[test]
    fn blend_fully_transparent() {
        let fg = ColorValue::new("#ff0000").unwrap();
        let bg = ColorValue::new("#0000ff").unwrap();
        let result = color_blend(&fg, &bg, 0.0);
        assert_eq!(result.red(), 0);
        assert_eq!(result.green(), 0);
        assert_eq!(result.blue(), 255);
    }

    #[test]
    fn blend_half_alpha() {
        let fg = ColorValue::new("#ff0000").unwrap();
        let bg = ColorValue::new("#0000ff").unwrap();
        let result = color_blend(&fg, &bg, 0.5);
        // Should be roughly (128, 0, 128)
        assert!((result.red() as i32 - 128).abs() <= 1);
        assert_eq!(result.green(), 0);
        assert!((result.blue() as i32 - 128).abs() <= 1);
    }

    #[test]
    fn color_mix_symmetric() {
        let a = ColorValue::new("#ff0000").unwrap();
        let b = ColorValue::new("#0000ff").unwrap();
        let mixed = color_mix(&a, &b);
        assert!((mixed.red() as i32 - 128).abs() <= 1);
        assert!((mixed.blue() as i32 - 128).abs() <= 1);
    }

    #[test]
    fn color_lighten_moves_toward_white() {
        let dark = ColorValue::new("#333333").unwrap();
        let lighter = color_lighten(&dark, 0.5);
        assert!(lighter.red() > dark.red());
        assert!(lighter.green() > dark.green());
        assert!(lighter.blue() > dark.blue());
    }

    #[test]
    fn color_darken_moves_toward_black() {
        let light = ColorValue::new("#cccccc").unwrap();
        let darker = color_darken(&light, 0.5);
        assert!(darker.red() < light.red());
        assert!(darker.green() < light.green());
        assert!(darker.blue() < light.blue());
    }

    // -- ColorValue helpers ------------------------------------------------

    #[test]
    fn color_luminance() {
        let white = ColorValue::new("#ffffff").unwrap();
        let black = ColorValue::new("#000000").unwrap();
        assert!(white.luminance() > 0.9);
        assert!(black.luminance() < 0.01);
    }

    #[test]
    fn color_contrast_ratio_bw() {
        let white = ColorValue::new("#ffffff").unwrap();
        let black = ColorValue::new("#000000").unwrap();
        let ratio = white.contrast_ratio(&black);
        assert!(ratio > 20.0);
    }

    #[test]
    fn color_to_hex_string_long() {
        let c = ColorValue::new("#FF0000").unwrap();
        assert_eq!(c.to_hex_string(), "#ff0000");
    }

    #[test]
    fn color_to_hex_string_short() {
        let c = ColorValue::new("#f00").unwrap();
        assert_eq!(c.to_hex_string(), "#ff0000");
    }

    #[test]
    fn color_from_rgb() {
        let c = ColorValue::from_rgb(255, 128, 0);
        assert_eq!(c.red(), 255);
        assert_eq!(c.green(), 128);
        assert_eq!(c.blue(), 0);
    }

    // -- ThemeType helpers -------------------------------------------------

    #[test]
    fn theme_type_is_dark_light() {
        assert!(ThemeType::Dark.is_dark());
        assert!(ThemeType::HighContrast.is_dark());
        assert!(!ThemeType::Light.is_dark());
        assert!(ThemeType::Light.is_light());
        assert!(ThemeType::HighContrastLight.is_light());
        assert!(!ThemeType::Dark.is_light());
    }

    // -- ColorTheme::is_light ----------------------------------------------

    #[test]
    fn color_theme_is_light() {
        let light = ColorTheme {
            id: "light".into(),
            label: "Light".into(),
            theme_type: ThemeType::Light,
            colors: HashMap::new(),
            token_colors: Vec::new(),
        };
        assert!(light.is_light());
        assert!(!dark_theme().is_light());
    }

    // -- ThemeService helpers -----------------------------------------------

    #[test]
    fn service_dark_and_light_themes() {
        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());
        svc.register_theme(ColorTheme {
            id: "light-plus".into(),
            label: "Light+".into(),
            theme_type: ThemeType::Light,
            colors: HashMap::new(),
            token_colors: Vec::new(),
        });
        assert_eq!(svc.dark_themes().len(), 1);
        assert_eq!(svc.light_themes().len(), 1);
    }

    #[test]
    fn service_all_theme_ids() {
        let mut svc = ThemeService::new();
        svc.register_theme(dark_theme());
        let ids = svc.all_theme_ids();
        assert_eq!(ids, vec!["dark-plus"]);
    }

    #[test]
    fn service_display() {
        let mut svc = ThemeService::new();
        let s = format!("{}", svc);
        assert!(s.contains("0 themes"));
        assert!(s.contains("active=none"));
        svc.register_theme(dark_theme());
        svc.set_active("dark-plus");
        let s2 = format!("{}", svc);
        assert!(s2.contains("active=Dark+"));
    }

    // ---- ColorValidator tests ----

    #[test]
    fn color_validator_valid_formats() {
        assert!(ColorValidator::is_valid("#abc"));
        assert!(ColorValidator::is_valid("#aabbcc"));
        assert!(ColorValidator::is_valid("#aabbccdd"));
        assert!(!ColorValidator::is_valid("abc"));
        assert!(!ColorValidator::is_valid("#ab"));
        assert!(!ColorValidator::is_valid("#abcde"));
        assert!(!ColorValidator::is_valid("#gggggg"));
    }

    #[test]
    fn color_validator_normalize() {
        assert_eq!(ColorValidator::normalize("#ABC").unwrap(), "#aabbcc");
        assert_eq!(ColorValidator::normalize("#FF0000").unwrap(), "#ff0000");
        assert_eq!(
            ColorValidator::normalize("#ff000080").unwrap(),
            "#ff0000"
        );
        assert!(ColorValidator::normalize("bad").is_err());
    }

    #[test]
    fn color_validator_alpha() {
        assert_eq!(ColorValidator::alpha("#ff0000").unwrap(), 255);
        assert_eq!(ColorValidator::alpha("#ff000080").unwrap(), 0x80);
        assert_eq!(ColorValidator::alpha("#abc").unwrap(), 255);
    }

    #[test]
    fn color_validator_with_alpha() {
        let result = ColorValidator::with_alpha("#FF0000", 128).unwrap();
        assert_eq!(result, "#ff000080");
        let result2 = ColorValidator::with_alpha("#abc", 0).unwrap();
        assert_eq!(result2, "#aabbcc00");
    }

    // ---- ThemeDiff tests ----

    #[test]
    fn theme_diff_identical() {
        let t = dark_theme();
        let diff = ThemeDiff::diff(&t, &t);
        assert!(diff.is_empty());
        assert_eq!(diff.len(), 0);
        assert_eq!(format!("{}", diff), "no differences");
    }

    #[test]
    fn theme_diff_color_changes() {
        let a = dark_theme();
        let mut b = a.clone();
        b.colors.insert("editor.background".into(), "#000000".into());
        b.colors.insert("statusBar.background".into(), "#007acc".into());
        b.colors.remove("editor.foreground");
        let diff = ThemeDiff::diff(&a, &b);
        assert!(!diff.is_empty());
        // Should have: 1 changed, 1 added, 1 removed = 3 color diffs
        assert_eq!(diff.color_diffs.len(), 3);
        let display = format!("{}", diff);
        assert!(display.contains("Colors:"));
    }

    #[test]
    fn theme_diff_token_scope_changes() {
        let a = dark_theme();
        let mut b = a.clone();
        b.token_colors = vec![TokenColor {
            scope: vec!["string".into()],
            foreground: Some("#ce9178".into()),
            font_style: None,
        }];
        let diff = ThemeDiff::diff(&a, &b);
        // "keyword" removed, "string" added
        assert_eq!(diff.token_scope_diffs.len(), 2);
        let display = format!("{}", diff);
        assert!(display.contains("Token scopes:"));
    }

    // ---- ThemeInheritance tests ----

    #[test]
    fn theme_inheritance_resolve() {
        let parent = dark_theme();
        let mut child_colors = HashMap::new();
        child_colors.insert("editor.background".into(), "#000000".into());
        child_colors.insert("statusBar.background".into(), "#007acc".into());
        let child = ColorTheme {
            id: "child-dark".into(),
            label: "Child Dark".into(),
            theme_type: ThemeType::Dark,
            colors: child_colors,
            token_colors: vec![],
        };
        let inheritance = ThemeInheritance::new(parent, child);
        let resolved = inheritance.resolve();
        assert_eq!(resolved.id, "child-dark");
        assert_eq!(resolved.colors.get("editor.background").unwrap(), "#000000");
        assert_eq!(
            resolved.colors.get("editor.foreground").unwrap(),
            "#d4d4d4"
        );
        assert_eq!(
            resolved.colors.get("statusBar.background").unwrap(),
            "#007acc"
        );
        // Parent token colors are inherited
        assert_eq!(resolved.token_colors.len(), 1);
        assert_eq!(resolved.token_colors[0].scope[0], "keyword");

        assert_eq!(inheritance.overridden_color_keys(), vec!["editor.background"]);
        assert_eq!(inheritance.added_color_keys(), vec!["statusBar.background"]);
        assert_eq!(
            format!("{}", inheritance),
            "Child Dark (inherits from Dark+)"
        );
    }

    // ---- TokenColorMatcher tests ----

    #[test]
    fn token_color_matcher_exact_and_prefix() {
        let mut matcher = TokenColorMatcher::new();
        matcher.add_rule("keyword", "#569cd6");
        matcher.add_rule("keyword.*", "#c586c0");
        matcher.add_rule("*", "#d4d4d4");

        // Exact match
        assert_eq!(matcher.match_scope("keyword"), Some("#569cd6"));
        // Prefix match — "keyword.control" starts with "keyword."
        assert_eq!(matcher.match_scope("keyword.control"), Some("#c586c0"));
        // Wildcard fallback
        assert_eq!(matcher.match_scope("variable"), Some("#d4d4d4"));
        // No rules → None
        let empty = TokenColorMatcher::new();
        assert!(empty.match_scope("anything").is_none());

        assert_eq!(matcher.rule_count(), 3);
        assert_eq!(format!("{}", matcher), "TokenColorMatcher(3 rules)");
    }

    #[test]
    fn token_color_matcher_from_theme() {
        let theme = dark_theme();
        let matcher = TokenColorMatcher::from_theme(&theme);
        assert_eq!(matcher.match_scope("keyword"), Some("#569cd6"));
        assert!(matcher.match_scope("string").is_none());
    }

    // ---- From impls tests ----

    #[test]
    fn color_value_from_tuple() {
        let c: ColorValue = (255u8, 0u8, 128u8).into();
        assert_eq!(c.red(), 255);
        assert_eq!(c.green(), 0);
        assert_eq!(c.blue(), 128);
    }

    #[test]
    fn theme_summary_from_color_theme() {
        let theme = dark_theme();
        let summary = ThemeSummary::from(&theme);
        assert_eq!(summary.id, "dark-plus");
        assert_eq!(summary.label, "Dark+");
        assert_eq!(summary.theme_type, ThemeType::Dark);
    }

    // ---- DiffEntry Display ----

    #[test]
    fn diff_entry_display() {
        let added = DiffEntry::Added {
            key: "k".into(),
            value: "#fff".into(),
        };
        assert_eq!(format!("{}", added), "+ k: #fff");
        let removed = DiffEntry::Removed {
            key: "k".into(),
            value: "#000".into(),
        };
        assert_eq!(format!("{}", removed), "- k: #000");
        let changed = DiffEntry::Changed {
            key: "k".into(),
            old: "#000".into(),
            new: "#fff".into(),
        };
        assert_eq!(format!("{}", changed), "~ k: #000 -> #fff");
    }

    // ---- ThemePreview tests ----

    #[test]
    fn preview_generate_swatches() {
        let theme = dark_theme();
        let swatches =
            ThemePreview::generate_swatches(&theme, &["editor.background", "editor.foreground"]);
        assert_eq!(swatches.len(), 2);
        assert_eq!(swatches[0].key, "editor.background");
        assert_eq!(swatches[0].hex_value, "#1e1e1e");
        assert_eq!(swatches[0].render(), "editor.background: #1e1e1e");
    }

    #[test]
    fn preview_missing_keys_skipped() {
        let theme = dark_theme();
        let swatches =
            ThemePreview::generate_swatches(&theme, &["editor.background", "nonexistent"]);
        assert_eq!(swatches.len(), 1);
    }

    #[test]
    fn preview_render_preview() {
        let swatches = vec![
            ColorSwatch {
                key: "a".into(),
                hex_value: "#111".into(),
                label: "a".into(),
            },
            ColorSwatch {
                key: "b".into(),
                hex_value: "#222".into(),
                label: "b".into(),
            },
        ];
        let out = ThemePreview::render_preview(&swatches);
        assert!(out.contains("a: #111"));
        assert!(out.contains('\n'));
        assert_eq!(ThemePreview::render_preview(&[]), "(no swatches)");
    }

    #[test]
    fn preview_swatch_count() {
        let theme = dark_theme();
        assert_eq!(ThemePreview::swatch_count(&theme), 2);
    }

    // ---- ThemeMigrator tests ----

    #[test]
    fn migrator_apply_migrations() {
        let mut migrator = ThemeMigrator::new();
        migrator.add_migration("old.bg", "editor.background2");
        migrator.add_migration("old.fg", "editor.foreground2");
        assert_eq!(migrator.migration_count(), 2);

        let mut theme = dark_theme();
        theme.colors.insert("old.bg".into(), "#aaa".into());
        theme.colors.insert("old.fg".into(), "#bbb".into());
        let applied = migrator.migrate_theme(&mut theme);
        assert_eq!(applied, 2);
        assert!(!theme.colors.contains_key("old.bg"));
        assert_eq!(theme.colors.get("editor.background2").unwrap(), "#aaa");
    }

    #[test]
    fn migrator_skips_when_new_key_exists() {
        let mut migrator = ThemeMigrator::new();
        migrator.add_migration("editor.background", "editor.foreground");
        let mut theme = dark_theme();
        let applied = migrator.migrate_theme(&mut theme);
        assert_eq!(applied, 0);
    }

    #[test]
    fn migrator_has_deprecated_keys() {
        let mut migrator = ThemeMigrator::new();
        migrator.add_migration("editor.background", "new.bg");
        let theme = dark_theme();
        let deprecated = migrator.has_deprecated_keys(&theme);
        assert_eq!(deprecated, vec!["editor.background"]);
    }

    // ---- ThemeContributionMerger tests ----

    #[test]
    fn merger_higher_priority_wins() {
        let mut merger = ThemeContributionMerger::new();
        let mut base = HashMap::new();
        base.insert("bg".into(), "#000".into());
        merger.set_base(base);

        let mut low = HashMap::new();
        low.insert("bg".into(), "#111".into());
        merger.add_contribution("ext-a", low, 1);

        let mut high = HashMap::new();
        high.insert("bg".into(), "#222".into());
        merger.add_contribution("ext-b", high, 10);

        let merged = merger.merge();
        assert_eq!(merged.get("bg").unwrap(), "#222");
    }

    #[test]
    fn merger_conflict_keys() {
        let mut merger = ThemeContributionMerger::new();
        let mut a = HashMap::new();
        a.insert("bg".into(), "#000".into());
        merger.add_contribution("a", a, 1);
        let mut b = HashMap::new();
        b.insert("bg".into(), "#111".into());
        merger.add_contribution("b", b, 2);

        let conflicts = merger.conflict_keys();
        assert_eq!(conflicts, vec!["bg"]);
        assert_eq!(merger.contribution_count(), 2);
    }

    #[test]
    fn merger_base_preserved_without_contributions() {
        let mut merger = ThemeContributionMerger::new();
        let mut base = HashMap::new();
        base.insert("fg".into(), "#fff".into());
        merger.set_base(base);
        let merged = merger.merge();
        assert_eq!(merged.get("fg").unwrap(), "#fff");
    }

    // ---- ThemeIconMapper tests ----

    #[test]
    fn icon_mapper_defaults() {
        let mapper = ThemeIconMapper::with_defaults();
        assert_eq!(mapper.icon_count(), 5);
        assert_eq!(mapper.resolve_icon("rust"), "🦀");
        assert_eq!(mapper.resolve_icon("python"), "🐍");
        assert_eq!(mapper.resolve_icon("unknown"), "📄");
        assert!(mapper.has_icon("folder"));
        assert!(!mapper.has_icon("go"));
    }

    #[test]
    fn icon_mapper_register_custom() {
        let mut mapper = ThemeIconMapper::new();
        assert_eq!(mapper.icon_count(), 0);
        mapper.register("go", "🐹");
        assert!(mapper.has_icon("go"));
        assert_eq!(mapper.resolve_icon("go"), "🐹");
        assert_eq!(mapper.icon_count(), 1);
    }

    #[test]
    fn themePreviewWidget_new() {
        let s = ThemePreviewWidget::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn themePreviewWidget_add_contains() {
        let mut s = ThemePreviewWidget::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn themePreviewWidget_add_duplicate() {
        let mut s = ThemePreviewWidget::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn themePreviewWidget_remove() {
        let mut s = ThemePreviewWidget::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn themePreviewWidget_capacity() {
        let s = ThemePreviewWidget::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn themePreviewWidget_search() {
        let mut s = ThemePreviewWidget::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn themePreviewWidget_stats() {
        let mut s = ThemePreviewWidget::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn themeImportHandler_new() {
        let m = ThemeImportHandler::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn themeImportHandler_add_find() {
        let mut m = ThemeImportHandler::new();
        m.add(ThemeImportHandlerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn themeImportHandler_priority_filter() {
        let mut m = ThemeImportHandler::new();
        m.add(ThemeImportHandlerItem::new("a", "A").with_priority(ThemeImportHandlerPriority::High));
        m.add(ThemeImportHandlerItem::new("b", "B").with_priority(ThemeImportHandlerPriority::Low));
        m.add(ThemeImportHandlerItem::new("c", "C").with_priority(ThemeImportHandlerPriority::High));
        assert_eq!(m.by_priority(ThemeImportHandlerPriority::High).len(), 2);
    }

    #[test]
    fn themeImportHandler_remove() {
        let mut m = ThemeImportHandler::new();
        m.add(ThemeImportHandlerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn themeImportHandler_search() {
        let mut m = ThemeImportHandler::new();
        m.add(ThemeImportHandlerItem::new("id1", "Hello World"));
        m.add(ThemeImportHandlerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn themeImportHandler_total_weight() {
        let mut m = ThemeImportHandler::new();
        m.add(ThemeImportHandlerItem::new("a", "A").with_priority(ThemeImportHandlerPriority::Critical));
        m.add(ThemeImportHandlerItem::new("b", "B").with_priority(ThemeImportHandlerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn themeImportHandler_capacity_limit() {
        let mut m = ThemeImportHandler::new().with_max_items(2);
        m.add(ThemeImportHandlerItem::new("1", "one"));
        m.add(ThemeImportHandlerItem::new("2", "two"));
        assert!(!m.add(ThemeImportHandlerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn themeImportHandler_sorted_by_priority() {
        let mut m = ThemeImportHandler::new();
        m.add(ThemeImportHandlerItem::new("lo", "Low").with_priority(ThemeImportHandlerPriority::Low));
        m.add(ThemeImportHandlerItem::new("hi", "High").with_priority(ThemeImportHandlerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn themeImportHandler_item_metadata() {
        let mut item = ThemeImportHandlerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn themePreviewWidget_enabled_toggle() {
        let mut s = ThemePreviewWidget::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn themeImportHandler_priority_display() {
        assert_eq!(format!("{}", ThemeImportHandlerPriority::High), "high");
        assert_eq!(format!("{}", ThemeImportHandlerPriority::Low), "low");
    }

}
