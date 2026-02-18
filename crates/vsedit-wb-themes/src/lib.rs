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


// ─── ThemeWb Builder & Validator ─────────────────────────────

/// Builder for constructing themes configurations.
#[derive(Debug, Clone)]
pub struct ThemeWbBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl ThemeWbBuilder {
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

    pub fn build(self) -> Result<ThemeWbCfg, ThemeWbBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(ThemeWbBuildErr { errors }); }
        Ok(ThemeWbCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated themes configuration.
#[derive(Debug, Clone)]
pub struct ThemeWbCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl ThemeWbCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &ThemeWbCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for ThemeWbCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ThemeWbCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct ThemeWbBuildErr { pub errors: Vec<String> }

impl fmt::Display for ThemeWbBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ThemeWbBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for ThemeWbBuildErr {}

// ─── ThemeWb Formatter ───────────────────────────────────────

/// Formatting options for theme output.
#[derive(Debug, Clone)]
pub struct ThemeWbFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for ThemeWbFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl ThemeWbFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for theme data.
pub struct ThemeWbFmt {
    options: ThemeWbFmtOpts,
}

impl ThemeWbFmt {
    pub fn new(options: ThemeWbFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: ThemeWbFmtOpts::default() } }

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


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for wb_themes
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbThemesRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbThemesRingBuf {
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
pub struct XaWbThemesCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbThemesCounter {
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

impl Default for XaWbThemesCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 229
// ---------------------------------------------------------------------------

/// Generic object pool `Xc229Pool<T>`.
pub struct Xc229Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc229Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc229PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc229Pool<T> {
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
    pub fn stats(&self) -> Xc229PoolStats {
        Xc229PoolStats {
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

impl<T> Default for Xc229Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc229Scheduler`.
pub struct Xc229Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc229Scheduler {
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

impl Default for Xc229Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_229 hash for the given byte slice.
pub fn xc_229_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_229 convention.
pub fn xc_229_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_81 deepening: state machine + event bus ---

/// States for the Xd81 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd81State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd81State {
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
pub struct Xd81Transition {
    pub from: Xd81State,
    pub to: Xd81State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd81StateMachine {
    current: Xd81State,
    history: Vec<Xd81Transition>,
    step_counter: usize,
}

impl Xd81StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd81State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd81State {
        self.current
    }

    pub fn history(&self) -> &[Xd81Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd81State) -> Result<Xd81State, String> {
        let allowed = match (self.current, target) {
            (Xd81State::Idle, Xd81State::Running) => true,
            (Xd81State::Running, Xd81State::Paused) => true,
            (Xd81State::Running, Xd81State::Done) => true,
            (Xd81State::Paused, Xd81State::Running) => true,
            (Xd81State::Paused, Xd81State::Done) => true,
            (Xd81State::Done, Xd81State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_81: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd81Transition {
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
            "Xd81SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd81State> {
        let prefix = "Xd81SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd81State::Idle),
            "Running" => Some(Xd81State::Running),
            "Paused" => Some(Xd81State::Paused),
            "Done" => Some(Xd81State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd81State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd81 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd81Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd81Event {
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

type Xd81HandlerFn = Box<dyn Fn(&Xd81Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd81EventBus {
    handlers: Vec<(usize, Option<String>, Xd81HandlerFn)>,
    next_id: usize,
    published: Vec<Xd81Event>,
}

impl Xd81EventBus {
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
        F: Fn(&Xd81Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd81Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd81Event) {
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

    pub fn published_events(&self) -> &[Xd81Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #101
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf101Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf101TrieNode {
    children: std::collections::HashMap<char, Xf101TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf101Trie {
    root: Xf101TrieNode,
    count: usize,
}

impl Xf101Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf101TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf101TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf101TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf101BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf101BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 228).
pub struct Xh228SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh228SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 270 as u64,
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

/// A compact bit set supporting boolean operations (variant 228).
pub struct Xh228BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh228BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 228).
pub struct Xi228Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi228Deque<T> {
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
pub struct Xi228Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi228Interval {
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

/// A simple interval tree (variant 228).
pub struct Xi228IntervalTree {
    xi_intervals: Vec<Xi228Interval>,
}

impl Xi228IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi228Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi228Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi228Interval) -> Vec<&Xi228Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi228Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi228Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi228Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi228Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi228Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi228Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 228) ---

/// Disjoint set / union-find for crate 228.
pub struct Xj228UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj228UnionFind {
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

const XJ228_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 228.
pub struct Xj228BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj228BTreeNode<K, V>>>,
    len: usize,
}

struct Xj228BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj228BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj228BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ228_BTREE_ORDER - 1
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
        let mid = XJ228_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj228BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj228BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj228BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj228BTreeNode::xj_new_leaf();
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


// --- xk_228 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk228SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk228SegmentTree {
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
pub struct Xk228DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk228DisjointIntervals {
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


    #[test]
    fn themewb_builder_valid() {
        let cfg = ThemeWbBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn themewb_builder_empty_name() {
        let r = ThemeWbBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn themewb_builder_bad_priority() {
        assert!(ThemeWbBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn themewb_builder_zero_max() {
        assert!(ThemeWbBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn themewb_cfg_merge() {
        let mut a = ThemeWbBuilder::new("a").property("x", "1").build().unwrap();
        let b = ThemeWbBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn themewb_cfg_display() {
        let cfg = ThemeWbBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }

    #[test]
    fn themewb_fmt_list() {
        let f = ThemeWbFmt::new(ThemeWbFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn themewb_fmt_kv() {
        let f = ThemeWbFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn themewb_fmt_section() {
        let f = ThemeWbFmt::new(ThemeWbFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn themewb_fmt_truncate() {
        let f = ThemeWbFmt::new(ThemeWbFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn themewb_fmt_opts_defaults() {
        let o = ThemeWbFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for wb_themes
    #[test]
    fn xa_wb_themes_ring_new() {
        let rb = super::XaWbThemesRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_themes_ring_push_len() {
        let mut rb = super::XaWbThemesRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_themes_ring_wrap() {
        let mut rb = super::XaWbThemesRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_themes_ring_mean_empty() {
        let rb = super::XaWbThemesRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_themes_ring_mean_values() {
        let mut rb = super::XaWbThemesRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_themes_ring_min_max() {
        let mut rb = super::XaWbThemesRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_themes_ring_iter() {
        let mut rb = super::XaWbThemesRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_themes_counter_new() {
        let c = super::XaWbThemesCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_themes_counter_inc() {
        let mut c = super::XaWbThemesCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_themes_counter_inc_by() {
        let mut c = super::XaWbThemesCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_themes_counter_reset() {
        let mut c = super::XaWbThemesCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_themes_counter_clear() {
        let mut c = super::XaWbThemesCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_themes_counter_default() {
        let c = super::XaWbThemesCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 229 ----

    #[test]
    fn xc_229_pool_new_empty() {
        let pool: super::Xc229Pool<i32> = super::Xc229Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_229_pool_release_acquire() {
        let mut pool = super::Xc229Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_229_pool_acquire_empty() {
        let mut pool: super::Xc229Pool<i32> = super::Xc229Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_229_pool_full() {
        let mut pool = super::Xc229Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_229_pool_drain() {
        let mut pool = super::Xc229Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_229_pool_stats() {
        let mut pool = super::Xc229Pool::new(8);
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
    fn xc_229_pool_clear() {
        let mut pool = super::Xc229Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_229_pool_shrink() {
        let mut pool = super::Xc229Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_229_pool_default() {
        let pool: super::Xc229Pool<String> = super::Xc229Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_229_pool_extend() {
        let mut pool = super::Xc229Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_229_pool_retain() {
        let mut pool = super::Xc229Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_229_scheduler_round_robin() {
        let mut sched = super::Xc229Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_229_scheduler_empty() {
        let mut sched = super::Xc229Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_229_scheduler_reset() {
        let mut sched = super::Xc229Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_229_scheduler_add_remove() {
        let mut sched = super::Xc229Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_229_scheduler_targets() {
        let sched = super::Xc229Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_229_hash_empty() {
        assert_eq!(super::xc_229_hash(b""), 5381);
    }

    #[test]
    fn xc_229_hash_data() {
        let h = super::xc_229_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_229_hash(b"hello"), h);
    }

    #[test]
    fn xc_229_reverse_str() {
        assert_eq!(super::xc_229_reverse("abc"), "cba");
        assert_eq!(super::xc_229_reverse(""), "");
    }


    // --- xd_81 deepening tests ---

    #[test]
    fn xd_81_sm_initial_state() {
        let sm = Xd81StateMachine::new();
        assert_eq!(sm.current_state(), Xd81State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_81_sm_valid_idle_to_running() {
        let mut sm = Xd81StateMachine::new();
        assert!(sm.transition(Xd81State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd81State::Running);
    }

    #[test]
    fn xd_81_sm_valid_running_to_paused() {
        let mut sm = Xd81StateMachine::new();
        sm.transition(Xd81State::Running).unwrap();
        assert!(sm.transition(Xd81State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd81State::Paused);
    }

    #[test]
    fn xd_81_sm_valid_running_to_done() {
        let mut sm = Xd81StateMachine::new();
        sm.transition(Xd81State::Running).unwrap();
        assert!(sm.transition(Xd81State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd81State::Done);
    }

    #[test]
    fn xd_81_sm_valid_paused_to_running() {
        let mut sm = Xd81StateMachine::new();
        sm.transition(Xd81State::Running).unwrap();
        sm.transition(Xd81State::Paused).unwrap();
        assert!(sm.transition(Xd81State::Running).is_ok());
    }

    #[test]
    fn xd_81_sm_valid_done_to_idle() {
        let mut sm = Xd81StateMachine::new();
        sm.transition(Xd81State::Running).unwrap();
        sm.transition(Xd81State::Done).unwrap();
        assert!(sm.transition(Xd81State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd81State::Idle);
    }

    #[test]
    fn xd_81_sm_invalid_idle_to_done() {
        let mut sm = Xd81StateMachine::new();
        assert!(sm.transition(Xd81State::Done).is_err());
    }

    #[test]
    fn xd_81_sm_invalid_idle_to_paused() {
        let mut sm = Xd81StateMachine::new();
        assert!(sm.transition(Xd81State::Paused).is_err());
    }

    #[test]
    fn xd_81_sm_history_tracking() {
        let mut sm = Xd81StateMachine::new();
        sm.transition(Xd81State::Running).unwrap();
        sm.transition(Xd81State::Paused).unwrap();
        sm.transition(Xd81State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd81State::Idle);
        assert_eq!(sm.history()[0].to, Xd81State::Running);
        assert_eq!(sm.history()[1].from, Xd81State::Running);
        assert_eq!(sm.history()[2].to, Xd81State::Done);
    }

    #[test]
    fn xd_81_sm_serialize_deserialize() {
        let mut sm = Xd81StateMachine::new();
        sm.transition(Xd81State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd81StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd81State::Running));
    }

    #[test]
    fn xd_81_sm_deserialize_invalid() {
        assert_eq!(Xd81StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_81_sm_reset() {
        let mut sm = Xd81StateMachine::new();
        sm.transition(Xd81State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd81State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_81_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd81EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd81Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_81_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd81EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd81Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd81Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_81_bus_unsubscribe() {
        let mut bus = Xd81EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_81_event_kind_and_payload() {
        let e = Xd81Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd81Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_81_bus_clear_history() {
        let mut bus = Xd81EventBus::new();
        bus.publish(Xd81Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_81_sm_step_counter_increments() {
        let mut sm = Xd81StateMachine::new();
        sm.transition(Xd81State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd81State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #101 --

    #[test]
    fn xf101_trie_insert_search() {
        let mut t = Xf101Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf101_trie_starts_with() {
        let mut t = Xf101Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf101_trie_remove() {
        let mut t = Xf101Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf101_trie_word_count() {
        let mut t = Xf101Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf101_trie_longest_prefix() {
        let mut t = Xf101Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf101_trie_all_words() {
        let mut t = Xf101Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf101_trie_autocomplete() {
        let mut t = Xf101Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf101_trie_empty_search() {
        let t = Xf101Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf101_bloom_add_contains() {
        let mut bf = Xf101BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf101_bloom_probably_absent() {
        let bf = Xf101BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf101_bloom_false_positive_rate() {
        let mut bf = Xf101BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf101_bloom_clear() {
        let mut bf = Xf101BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf101_bloom_union() {
        let mut a = Xf101BloomFilter::xf_new(512, 2);
        let mut b = Xf101BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf101_bloom_intersection_estimate() {
        let mut a = Xf101BloomFilter::xf_new(512, 2);
        let mut b = Xf101BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf101_bloom_union_size_mismatch() {
        let a = Xf101BloomFilter::xf_new(256, 2);
        let b = Xf101BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh228_skip_insert_contains() {
        let mut sl = super::Xh228SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh228_skip_remove() {
        let mut sl = super::Xh228SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh228_skip_len() {
        let mut sl = super::Xh228SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh228_skip_range_query() {
        let mut sl = super::Xh228SkipList::xh_new(4);
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
    fn xh228_skip_floor_ceiling() {
        let mut sl = super::Xh228SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh228_skip_rank() {
        let mut sl = super::Xh228SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh228_skip_empty() {
        let sl = super::Xh228SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh228_skip_duplicates() {
        let mut sl = super::Xh228SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh228_bitset_set_test() {
        let mut bs = super::Xh228BitSet::xh_new(256);
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
    fn xh228_bitset_clear_count() {
        let mut bs = super::Xh228BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh228_bitset_and_or_xor() {
        let mut a = super::Xh228BitSet::xh_new(128);
        let mut b = super::Xh228BitSet::xh_new(128);
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
    fn xh228_bitset_iter_ones() {
        let mut bs = super::Xh228BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh228_bitset_first_last() {
        let mut bs = super::Xh228BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh228_bitset_empty() {
        let bs = super::Xh228BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi228_deque_push_pop_back() {
        let mut dq = super::Xi228Deque::xi_new(4);
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
    fn xi228_deque_push_pop_front() {
        let mut dq = super::Xi228Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi228_deque_mixed_ops() {
        let mut dq = super::Xi228Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi228_deque_get_and_split() {
        let mut dq = super::Xi228Deque::xi_new(8);
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
    fn xi228_deque_rotate_left() {
        let mut dq = super::Xi228Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi228_deque_rotate_right() {
        let mut dq = super::Xi228Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi228_deque_grow() {
        let mut dq = super::Xi228Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi228_deque_empty() {
        let dq = super::Xi228Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi228_interval_tree_insert_query() {
        let mut tree = super::Xi228IntervalTree::xi_new();
        tree.xi_insert(super::Xi228Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi228Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi228Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi228_interval_tree_overlap() {
        let mut tree = super::Xi228IntervalTree::xi_new();
        tree.xi_insert(super::Xi228Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi228Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi228Interval::xi_new(12, 20));
        let q = super::Xi228Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi228_interval_tree_remove() {
        let mut tree = super::Xi228IntervalTree::xi_new();
        tree.xi_insert(super::Xi228Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi228Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi228_interval_tree_gaps() {
        let mut tree = super::Xi228IntervalTree::xi_new();
        tree.xi_insert(super::Xi228Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi228Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi228Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi228Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi228Interval::xi_new(8, 10));
    }

    #[test]
    fn xi228_interval_tree_merge() {
        let mut tree = super::Xi228IntervalTree::xi_new();
        tree.xi_insert(super::Xi228Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi228Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi228Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi228Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi228Interval::xi_new(10, 15));
    }

    #[test]
    fn xi228_interval_tree_all() {
        let mut tree = super::Xi228IntervalTree::xi_new();
        tree.xi_insert(super::Xi228Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi228Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi228_interval_tree_empty() {
        let tree = super::Xi228IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi228_interval_tree_contains_point() {
        let iv = super::Xi228Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 228) ---

    #[test]
    fn xj_228_uf_make_and_find() {
        let mut uf = super::Xj228UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_228_uf_union_connected() {
        let mut uf = super::Xj228UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_228_uf_component_count() {
        let mut uf = super::Xj228UnionFind::xj_new();
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
    fn xj_228_uf_component_size() {
        let mut uf = super::Xj228UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_228_uf_largest_component() {
        let mut uf = super::Xj228UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_228_uf_many_elements() {
        let mut uf = super::Xj228UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_228_uf_separate_components() {
        let mut uf = super::Xj228UnionFind::xj_new();
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
    fn xj_228_uf_path_compression() {
        let mut uf = super::Xj228UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_228_bt_insert_get() {
        let mut bt = super::Xj228BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_228_bt_contains_len() {
        let mut bt = super::Xj228BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_228_bt_replace() {
        let mut bt = super::Xj228BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_228_bt_remove() {
        let mut bt = super::Xj228BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_228_bt_keys_values() {
        let mut bt = super::Xj228BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_228_bt_range() {
        let mut bt = super::Xj228BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_228_bt_min_max() {
        let mut bt = super::Xj228BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_228_bt_many_inserts() {
        let mut bt = super::Xj228BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_228 segment tree tests ---

    #[test]
    fn xk_228_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk228SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_228_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk228SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_228_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk228SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_228_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk228SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_228_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk228SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_228_st_single_element() {
        let data = vec![42];
        let st = super::Xk228SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_228_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk228SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_228_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk228SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_228 disjoint intervals tests ---

    #[test]
    fn xk_228_di_add_and_count() {
        let mut di = super::Xk228DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_228_di_merge_overlap() {
        let mut di = super::Xk228DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_228_di_contains() {
        let mut di = super::Xk228DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_228_di_remove() {
        let mut di = super::Xk228DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_228_di_covered_length() {
        let mut di = super::Xk228DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_228_di_gaps() {
        let mut di = super::Xk228DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_228_di_merge_adjacent() {
        let mut di = super::Xk228DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_228_di_empty() {
        let di = super::Xk228DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}