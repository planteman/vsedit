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
}
