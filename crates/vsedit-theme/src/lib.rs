//! Color theme service.
//!
//! Equivalent to VS Code's `vs/workbench/services/themes/common/workbenchThemeService.ts`.
//!
//! # Key types
//!
//! - [`Color`] — RGBA color value with hex parsing.
//! - [`ThemeType`] — light, dark, or high-contrast variant.
//! - [`ColorTheme`] — a complete color theme with workbench colors and token color rules.
//! - [`TokenColor`] — a TextMate scope-based token color rule.

use std::collections::HashMap;

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

/// An RGBA color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Creates an opaque RGB color.
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Creates an RGBA color.
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parses a hex color string in the form `#RRGGBB` or `#RRGGBBAA`.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#')?;
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::rgba(r, g, b, a))
            }
            _ => None,
        }
    }

    /// Formats the color as `#RRGGBB` (opaque) or `#RRGGBBAA`.
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }
}

// ---------------------------------------------------------------------------
// ThemeType
// ---------------------------------------------------------------------------

/// Discriminates light, dark, and high-contrast themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeType {
    Light,
    Dark,
    HighContrast,
    HighContrastLight,
}

impl ThemeType {
    fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "light" => Self::Light,
            "hc" | "highcontrast" | "hc-black" => Self::HighContrast,
            "hclight" | "hc-light" | "highcontrastlight" => Self::HighContrastLight,
            _ => Self::Dark,
        }
    }
}

// ---------------------------------------------------------------------------
// TokenColor / TokenSettings
// ---------------------------------------------------------------------------

/// Visual settings for a token color rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSettings {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    /// One of `"bold"`, `"italic"`, `"underline"`, or a space-separated combination.
    pub font_style: Option<String>,
}

/// A TextMate-style token color rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenColor {
    pub name: Option<String>,
    /// TextMate scopes this rule matches, e.g. `["comment", "string.quoted"]`.
    pub scope: Vec<String>,
    pub settings: TokenSettings,
}

// ---------------------------------------------------------------------------
// ColorTheme
// ---------------------------------------------------------------------------

/// A loaded color theme.
#[derive(Debug, Clone)]
pub struct ColorTheme {
    pub id: String,
    pub label: String,
    pub theme_type: ThemeType,
    /// Workbench colors keyed by identifier (e.g. `"editor.background"`).
    pub colors: HashMap<String, Color>,
    /// TextMate token color rules.
    pub token_colors: Vec<TokenColor>,
}

impl ColorTheme {
    /// Parses a VS Code–compatible theme JSON file.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let raw: RawTheme = serde_json::from_str(json)?;

        let theme_type = ThemeType::from_str_loose(&raw.theme_type.unwrap_or_default());

        let mut colors = HashMap::new();
        for (k, v) in &raw.colors {
            if let Some(c) = Color::from_hex(v) {
                colors.insert(k.clone(), c);
            }
        }

        let token_colors = raw
            .token_colors
            .into_iter()
            .map(|tc| {
                let scope = match tc.scope {
                    Some(RawScope::One(s)) => s
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                    Some(RawScope::Many(v)) => v,
                    None => Vec::new(),
                };
                TokenColor {
                    name: tc.name,
                    scope,
                    settings: TokenSettings {
                        foreground: tc.settings.foreground.as_deref().and_then(Color::from_hex),
                        background: tc.settings.background.as_deref().and_then(Color::from_hex),
                        font_style: tc.settings.font_style,
                    },
                }
            })
            .collect();

        let id = raw.name.clone().unwrap_or_default();
        let label = raw.name.unwrap_or_default();

        Ok(Self {
            id,
            label,
            theme_type,
            colors,
            token_colors,
        })
    }

    /// Returns the workbench color for the given key.
    pub fn get_color(&self, key: &str) -> Option<&Color> {
        self.colors.get(key)
    }

    /// Finds the best matching token settings for a list of TextMate scopes.
    ///
    /// Matches the rule whose scope is the longest prefix of any provided scope.
    pub fn get_token_color(&self, scopes: &[&str]) -> Option<&TokenSettings> {
        let mut best: Option<(usize, &TokenSettings)> = None;

        for rule in &self.token_colors {
            for rule_scope in &rule.scope {
                for input in scopes {
                    if scope_matches(input, rule_scope) {
                        let score = rule_scope.len();
                        if best.map_or(true, |(prev, _)| score > prev) {
                            best = Some((score, &rule.settings));
                        }
                    }
                }
            }
        }

        best.map(|(_, s)| s)
    }
}

/// A rule scope matches an input scope when the input equals the rule or the
/// input starts with the rule followed by a `.` (prefix matching).
fn scope_matches(input: &str, rule: &str) -> bool {
    input == rule || input.starts_with(rule) && input.as_bytes().get(rule.len()) == Some(&b'.')
}

// ---------------------------------------------------------------------------
// JSON deserialization helpers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawTheme {
    name: Option<String>,
    #[serde(rename = "type")]
    theme_type: Option<String>,
    #[serde(default)]
    colors: HashMap<String, String>,
    #[serde(rename = "tokenColors", default)]
    token_colors: Vec<RawTokenColor>,
}

#[derive(Deserialize)]
struct RawTokenColor {
    name: Option<String>,
    scope: Option<RawScope>,
    settings: RawTokenSettings,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawScope {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct RawTokenSettings {
    foreground: Option<String>,
    background: Option<String>,
    #[serde(rename = "fontStyle")]
    font_style: Option<String>,
}

// ---------------------------------------------------------------------------
// Built-in themes
// ---------------------------------------------------------------------------

/// Returns the built-in Dark+ theme.
pub fn dark_plus() -> ColorTheme {
    let mut colors = HashMap::new();
    let c = |hex: &str| Color::from_hex(hex).unwrap();

    colors.insert("editor.background".into(), c("#1E1E1E"));
    colors.insert("editor.foreground".into(), c("#D4D4D4"));
    colors.insert("editor.lineHighlightBackground".into(), c("#2A2D2E"));
    colors.insert("editor.selectionBackground".into(), c("#264F78"));
    colors.insert("editorCursor.foreground".into(), c("#AEAFAD"));
    colors.insert("editorWhitespace.foreground".into(), c("#3B3A32"));
    colors.insert("editorLineNumber.foreground".into(), c("#858585"));
    colors.insert("editorLineNumber.activeForeground".into(), c("#C6C6C6"));
    colors.insert("editorIndentGuide.background".into(), c("#404040"));
    colors.insert("editorIndentGuide.activeBackground".into(), c("#707070"));
    colors.insert("activityBar.background".into(), c("#333333"));
    colors.insert("activityBar.foreground".into(), c("#FFFFFF"));
    colors.insert("sideBar.background".into(), c("#252526"));
    colors.insert("sideBar.foreground".into(), c("#CCCCCC"));
    colors.insert("sideBarTitle.foreground".into(), c("#BBBBBB"));
    colors.insert("statusBar.background".into(), c("#007ACC"));
    colors.insert("statusBar.foreground".into(), c("#FFFFFF"));
    colors.insert("titleBar.activeBackground".into(), c("#3C3C3C"));
    colors.insert("titleBar.activeForeground".into(), c("#CCCCCC"));
    colors.insert("tab.activeBackground".into(), c("#1E1E1E"));
    colors.insert("tab.activeForeground".into(), c("#FFFFFF"));
    colors.insert("tab.inactiveBackground".into(), c("#2D2D2D"));
    colors.insert("tab.inactiveForeground".into(), c("#FFFFFF80"));

    let token_colors = vec![
        token("Comment", &["comment", "punctuation.definition.comment"], "#6A9955", None),
        token("String", &["string", "string.quoted"], "#CE9178", None),
        token("Number", &["constant.numeric"], "#B5CEA8", None),
        token("Keyword", &["keyword", "storage.type", "storage.modifier"], "#569CD6", None),
        token("Type", &["entity.name.type", "support.type"], "#4EC9B0", None),
        token("Function", &["entity.name.function", "support.function"], "#DCDCAA", None),
        token("Variable", &["variable", "variable.other"], "#9CDCFE", None),
        token("Constant", &["constant", "constant.language"], "#569CD6", None),
        token("Operator", &["keyword.operator"], "#D4D4D4", None),
        token("Parameter", &["variable.parameter"], "#9CDCFE", None),
        token("Property", &["variable.other.property"], "#9CDCFE", None),
        token("Tag", &["entity.name.tag"], "#569CD6", None),
        token("Attribute", &["entity.other.attribute-name"], "#9CDCFE", None),
        token("Punctuation", &["punctuation"], "#D4D4D4", None),
        token("Escape", &["constant.character.escape"], "#D7BA7D", None),
    ];

    ColorTheme {
        id: "vs-dark-plus".into(),
        label: "Dark+ (default dark)".into(),
        theme_type: ThemeType::Dark,
        colors,
        token_colors,
    }
}

/// Returns the built-in Light+ theme.
pub fn light_plus() -> ColorTheme {
    let mut colors = HashMap::new();
    let c = |hex: &str| Color::from_hex(hex).unwrap();

    colors.insert("editor.background".into(), c("#FFFFFF"));
    colors.insert("editor.foreground".into(), c("#000000"));
    colors.insert("editor.lineHighlightBackground".into(), c("#F5F5F5"));
    colors.insert("editor.selectionBackground".into(), c("#ADD6FF"));
    colors.insert("editorCursor.foreground".into(), c("#000000"));
    colors.insert("editorWhitespace.foreground".into(), c("#D3D3D3"));
    colors.insert("editorLineNumber.foreground".into(), c("#237893"));
    colors.insert("editorLineNumber.activeForeground".into(), c("#0B216F"));
    colors.insert("editorIndentGuide.background".into(), c("#D3D3D3"));
    colors.insert("editorIndentGuide.activeBackground".into(), c("#939393"));
    colors.insert("activityBar.background".into(), c("#2C2C2C"));
    colors.insert("activityBar.foreground".into(), c("#FFFFFF"));
    colors.insert("sideBar.background".into(), c("#F3F3F3"));
    colors.insert("sideBar.foreground".into(), c("#616161"));
    colors.insert("sideBarTitle.foreground".into(), c("#6F6F6F"));
    colors.insert("statusBar.background".into(), c("#007ACC"));
    colors.insert("statusBar.foreground".into(), c("#FFFFFF"));
    colors.insert("titleBar.activeBackground".into(), c("#DDDDDD"));
    colors.insert("titleBar.activeForeground".into(), c("#333333"));
    colors.insert("tab.activeBackground".into(), c("#FFFFFF"));
    colors.insert("tab.activeForeground".into(), c("#333333"));
    colors.insert("tab.inactiveBackground".into(), c("#ECECEC"));
    colors.insert("tab.inactiveForeground".into(), c("#33333380"));

    let token_colors = vec![
        token("Comment", &["comment", "punctuation.definition.comment"], "#008000", None),
        token("String", &["string", "string.quoted"], "#A31515", None),
        token("Number", &["constant.numeric"], "#098658", None),
        token("Keyword", &["keyword", "storage.type", "storage.modifier"], "#0000FF", None),
        token("Type", &["entity.name.type", "support.type"], "#267F99", None),
        token("Function", &["entity.name.function", "support.function"], "#795E26", None),
        token("Variable", &["variable", "variable.other"], "#001080", None),
        token("Constant", &["constant", "constant.language"], "#0000FF", None),
        token("Operator", &["keyword.operator"], "#000000", None),
        token("Parameter", &["variable.parameter"], "#001080", None),
        token("Property", &["variable.other.property"], "#001080", None),
        token("Tag", &["entity.name.tag"], "#800000", None),
        token("Attribute", &["entity.other.attribute-name"], "#FF0000", None),
        token("Punctuation", &["punctuation"], "#000000", None),
        token("Escape", &["constant.character.escape"], "#EE0000", None),
    ];

    ColorTheme {
        id: "vs-light-plus".into(),
        label: "Light+ (default light)".into(),
        theme_type: ThemeType::Light,
        colors,
        token_colors,
    }
}

fn token(name: &str, scopes: &[&str], fg: &str, style: Option<&str>) -> TokenColor {
    TokenColor {
        name: Some(name.into()),
        scope: scopes.iter().map(|s| (*s).to_string()).collect(),
        settings: TokenSettings {
            foreground: Color::from_hex(fg),
            background: None,
            font_style: style.map(String::from),
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Color parsing --

    #[test]
    fn color_from_hex_rgb() {
        let c = Color::from_hex("#1E1E1E").unwrap();
        assert_eq!(c, Color::rgb(0x1E, 0x1E, 0x1E));
    }

    #[test]
    fn color_from_hex_rgba() {
        let c = Color::from_hex("#FF000080").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 128));
    }

    #[test]
    fn color_from_hex_invalid() {
        assert!(Color::from_hex("not-a-color").is_none());
        assert!(Color::from_hex("#GG0000").is_none());
        assert!(Color::from_hex("#12345").is_none());
        assert!(Color::from_hex("").is_none());
    }

    #[test]
    fn color_to_hex_opaque() {
        assert_eq!(Color::rgb(0x1E, 0x1E, 0x1E).to_hex(), "#1E1E1E");
    }

    #[test]
    fn color_to_hex_alpha() {
        assert_eq!(Color::rgba(255, 0, 0, 128).to_hex(), "#FF000080");
    }

    #[test]
    fn color_roundtrip() {
        for hex in &["#ABCDEF", "#00FF0080", "#000000", "#FFFFFF"] {
            let c = Color::from_hex(hex).unwrap();
            assert_eq!(&c.to_hex(), hex);
        }
    }

    // -- Theme loading from JSON --

    #[test]
    fn theme_from_json_basic() {
        let json = r##"{
            "name": "Test Theme",
            "type": "dark",
            "colors": {
                "editor.background": "#1E1E1E",
                "editor.foreground": "#D4D4D4"
            },
            "tokenColors": [
                {
                    "scope": "comment",
                    "settings": { "foreground": "#6A9955" }
                }
            ]
        }"##;

        let theme = ColorTheme::from_json(json).unwrap();
        assert_eq!(theme.label, "Test Theme");
        assert_eq!(theme.theme_type, ThemeType::Dark);
        assert_eq!(theme.colors.len(), 2);
        assert_eq!(theme.token_colors.len(), 1);
    }

    #[test]
    fn theme_from_json_scope_array() {
        let json = r##"{
            "name": "Multi",
            "type": "light",
            "colors": {},
            "tokenColors": [
                {
                    "name": "Strings",
                    "scope": ["string", "string.quoted"],
                    "settings": { "foreground": "#A31515", "fontStyle": "italic" }
                }
            ]
        }"##;

        let theme = ColorTheme::from_json(json).unwrap();
        assert_eq!(theme.theme_type, ThemeType::Light);
        let tc = &theme.token_colors[0];
        assert_eq!(tc.scope, vec!["string", "string.quoted"]);
        assert_eq!(tc.settings.font_style.as_deref(), Some("italic"));
    }

    #[test]
    fn theme_from_json_comma_separated_scope() {
        let json = r##"{
            "name": "Comma",
            "colors": {},
            "tokenColors": [
                {
                    "scope": "keyword, storage.type",
                    "settings": { "foreground": "#0000FF" }
                }
            ]
        }"##;

        let theme = ColorTheme::from_json(json).unwrap();
        assert_eq!(
            theme.token_colors[0].scope,
            vec!["keyword", "storage.type"]
        );
    }

    // -- Color lookup --

    #[test]
    fn get_color_found() {
        let theme = dark_plus();
        let bg = theme.get_color("editor.background").unwrap();
        assert_eq!(bg, &Color::from_hex("#1E1E1E").unwrap());
    }

    #[test]
    fn get_color_missing() {
        let theme = dark_plus();
        assert!(theme.get_color("nonexistent.color").is_none());
    }

    // -- Token color matching --

    #[test]
    fn token_color_exact_match() {
        let theme = dark_plus();
        let settings = theme.get_token_color(&["comment"]).unwrap();
        assert_eq!(settings.foreground, Color::from_hex("#6A9955"));
    }

    #[test]
    fn token_color_prefix_match() {
        let theme = dark_plus();
        let settings = theme.get_token_color(&["constant.numeric.float"]).unwrap();
        // Should match "constant.numeric" rule
        assert_eq!(settings.foreground, Color::from_hex("#B5CEA8"));
    }

    #[test]
    fn token_color_best_match() {
        let theme = dark_plus();
        // "variable.other.property" should prefer the more specific "variable.other.property"
        // rule over the shorter "variable" rule.
        let settings = theme.get_token_color(&["variable.other.property"]).unwrap();
        assert_eq!(settings.foreground, Color::from_hex("#9CDCFE"));
    }

    #[test]
    fn token_color_no_match() {
        let theme = dark_plus();
        assert!(theme.get_token_color(&["meta.unknown.scope"]).is_none());
    }

    #[test]
    fn token_color_multiple_input_scopes() {
        let theme = dark_plus();
        let settings = theme
            .get_token_color(&["source.rust", "string.quoted.double"])
            .unwrap();
        // Should match "string.quoted"
        assert_eq!(settings.foreground, Color::from_hex("#CE9178"));
    }

    // -- Default themes --

    #[test]
    fn dark_plus_has_enough_colors() {
        let theme = dark_plus();
        assert!(
            theme.colors.len() >= 15,
            "dark+ should have at least 15 workbench colors, got {}",
            theme.colors.len()
        );
        assert!(
            theme.token_colors.len() >= 10,
            "dark+ should have at least 10 token colors, got {}",
            theme.token_colors.len()
        );
        assert_eq!(theme.theme_type, ThemeType::Dark);
    }

    #[test]
    fn light_plus_has_enough_colors() {
        let theme = light_plus();
        assert!(
            theme.colors.len() >= 15,
            "light+ should have at least 15 workbench colors, got {}",
            theme.colors.len()
        );
        assert!(
            theme.token_colors.len() >= 10,
            "light+ should have at least 10 token colors, got {}",
            theme.token_colors.len()
        );
        assert_eq!(theme.theme_type, ThemeType::Light);
    }

    // -- ThemeType parsing --

    #[test]
    fn theme_type_parsing() {
        assert_eq!(ThemeType::from_str_loose("light"), ThemeType::Light);
        assert_eq!(ThemeType::from_str_loose("dark"), ThemeType::Dark);
        assert_eq!(ThemeType::from_str_loose("hc"), ThemeType::HighContrast);
        assert_eq!(ThemeType::from_str_loose("hcLight"), ThemeType::HighContrastLight);
        // Unknown falls back to dark
        assert_eq!(ThemeType::from_str_loose("unknown"), ThemeType::Dark);
    }
}
