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

/// Service for theme management.
pub struct ThemeService {
    themes: Vec<ColorTheme>,
    active_theme: Option<usize>,
}

impl ThemeService {
    pub fn new() -> Self {
        Self {
            themes: Vec::new(),
            active_theme: None,
        }
    }

    pub fn register_theme(&mut self, theme: ColorTheme) {
        self.themes.push(theme);
    }

    pub fn set_active(&mut self, id: &str) -> bool {
        if let Some(idx) = self.themes.iter().position(|t| t.id == id) {
            self.active_theme = Some(idx);
            true
        } else {
            false
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
}
