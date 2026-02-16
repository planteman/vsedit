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
}
