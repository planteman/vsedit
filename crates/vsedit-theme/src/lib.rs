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
//! - [`TerminalColor`] — maps theme colors to terminal-displayable colors.
//! - [`TokenStyle`] — resolved style for a token (foreground, background, bold, italic, underline).

use std::fmt;
use std::collections::HashMap;
use std::path::Path;

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
// TerminalColor
// ---------------------------------------------------------------------------

/// A color mapped for terminal display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalColor {
    /// True-color RGB.
    Rgb(u8, u8, u8),
    /// 256-color index.
    Indexed(u8),
}

impl TerminalColor {
    /// Convert an RGB color to the closest xterm 256-color index.
    pub fn from_rgb_256(r: u8, g: u8, b: u8) -> Self {
        Self::Indexed(rgb_to_256(r, g, b))
    }

    /// Convert a [`Color`] to a `TerminalColor`, using true color when
    /// `true_color` is set, otherwise the closest 256-color index.
    pub fn from_color(color: &Color, true_color: bool) -> Self {
        if true_color {
            Self::Rgb(color.r, color.g, color.b)
        } else {
            Self::from_rgb_256(color.r, color.g, color.b)
        }
    }
}

/// Maps an RGB colour to the nearest xterm-256 index.
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    // Check greyscale ramp (232–255) first.
    if r == g && g == b {
        if r < 8 {
            return 16; // black
        }
        if r > 248 {
            return 231; // white
        }
        return (((r as u16 - 8) * 24 / 247) as u8) + 232;
    }
    // 6×6×6 colour cube (indices 16–231).
    let ri = ((r as u16) * 5 / 255) as u8;
    let gi = ((g as u16) * 5 / 255) as u8;
    let bi = ((b as u16) * 5 / 255) as u8;
    16 + 36 * ri + 6 * gi + bi
}

/// Resolve a VS Code color ID from a theme to a [`TerminalColor`].
pub fn resolve_color(theme: &ColorTheme, color_id: &str, true_color: bool) -> Option<TerminalColor> {
    theme.get_color(color_id).map(|c| TerminalColor::from_color(c, true_color))
}

// ---------------------------------------------------------------------------
// TokenStyle
// ---------------------------------------------------------------------------

/// Resolved visual style for a syntax token.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TokenStyle {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl TokenStyle {
    /// Build a `TokenStyle` from [`TokenSettings`].
    pub fn from_settings(settings: &TokenSettings) -> Self {
        let (bold, italic, underline) = parse_font_style(settings.font_style.as_deref());
        Self {
            foreground: settings.foreground,
            background: settings.background,
            bold,
            italic,
            underline,
        }
    }
}

fn parse_font_style(style: Option<&str>) -> (bool, bool, bool) {
    match style {
        Some(s) => {
            let bold = s.contains("bold");
            let italic = s.contains("italic");
            let underline = s.contains("underline");
            (bold, italic, underline)
        }
        None => (false, false, false),
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

    /// Resolves a [`TokenStyle`] for the given TextMate scopes.
    pub fn get_token_style(&self, scopes: &[&str]) -> Option<TokenStyle> {
        self.get_token_color(scopes).map(TokenStyle::from_settings)
    }

    /// Returns `true` when this theme is a high-contrast variant.
    pub fn is_high_contrast(&self) -> bool {
        matches!(self.theme_type, ThemeType::HighContrast | ThemeType::HighContrastLight)
    }
}

/// A rule scope matches an input scope when the input equals the rule or the
/// input starts with the rule followed by a `.` (prefix matching).
fn scope_matches(input: &str, rule: &str) -> bool {
    input == rule || input.starts_with(rule) && input.as_bytes().get(rule.len()) == Some(&b'.')
}

// ---------------------------------------------------------------------------
// File-based theme loading
// ---------------------------------------------------------------------------

/// Errors from theme file operations.
#[derive(Debug)]
pub enum ThemeFileError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for ThemeFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemeFileError::Io(e) => write!(f, "IO error: {e}"),
            ThemeFileError::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl From<std::io::Error> for ThemeFileError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for ThemeFileError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// Parse a VS Code `.json` theme file from disk.
///
/// If the theme JSON contains an `"include"` field, the referenced file is
/// loaded relative to the theme file's directory and used as a base; the
/// top-level theme's colors and token colors are then merged on top.
pub fn parse_theme_file(path: &Path) -> Result<ColorTheme, ThemeFileError> {
    let json = std::fs::read_to_string(path)?;
    let raw: RawThemeWithInclude = serde_json::from_str(&json)?;

    let base = if let Some(ref include) = raw.include {
        let base_path = path.parent().unwrap_or(Path::new(".")).join(include);
        if base_path.exists() {
            Some(parse_theme_file(&base_path)?)
        } else {
            None
        }
    } else {
        None
    };

    let mut theme = ColorTheme::from_json(&json)?;

    if let Some(base) = base {
        // Merge: base provides defaults, top-level overrides.
        let mut merged_colors = base.colors;
        merged_colors.extend(theme.colors);
        theme.colors = merged_colors;

        let mut merged_tokens = base.token_colors;
        merged_tokens.extend(theme.token_colors);
        theme.token_colors = merged_tokens;
    }

    Ok(theme)
}

// ---------------------------------------------------------------------------
// JSON deserialization helpers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawThemeWithInclude {
    include: Option<String>,
}

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

/// Returns the built-in Monokai theme.
pub fn monokai() -> ColorTheme {
    let mut colors = HashMap::new();
    let c = |hex: &str| Color::from_hex(hex).unwrap();

    colors.insert("editor.background".into(), c("#272822"));
    colors.insert("editor.foreground".into(), c("#F8F8F2"));
    colors.insert("editor.lineHighlightBackground".into(), c("#3E3D32"));
    colors.insert("editor.selectionBackground".into(), c("#49483E"));
    colors.insert("editorCursor.foreground".into(), c("#F8F8F0"));
    colors.insert("editorWhitespace.foreground".into(), c("#3B3A32"));
    colors.insert("editorLineNumber.foreground".into(), c("#90908A"));
    colors.insert("editorLineNumber.activeForeground".into(), c("#C2C2BF"));
    colors.insert("editorIndentGuide.background".into(), c("#464741"));
    colors.insert("editorIndentGuide.activeBackground".into(), c("#767771"));
    colors.insert("activityBar.background".into(), c("#272822"));
    colors.insert("activityBar.foreground".into(), c("#F8F8F2"));
    colors.insert("sideBar.background".into(), c("#1E1F1C"));
    colors.insert("sideBar.foreground".into(), c("#C3C5B1"));
    colors.insert("sideBarTitle.foreground".into(), c("#C3C5B1"));
    colors.insert("statusBar.background".into(), c("#414339"));
    colors.insert("statusBar.foreground".into(), c("#F8F8F2"));
    colors.insert("titleBar.activeBackground".into(), c("#272822"));
    colors.insert("titleBar.activeForeground".into(), c("#F8F8F2"));
    colors.insert("tab.activeBackground".into(), c("#272822"));
    colors.insert("tab.activeForeground".into(), c("#F8F8F2"));
    colors.insert("tab.inactiveBackground".into(), c("#1E1F1C"));
    colors.insert("tab.inactiveForeground".into(), c("#C3C5B1"));
    colors.insert("panel.background".into(), c("#272822"));
    colors.insert("panel.border".into(), c("#414339"));
    colors.insert("input.background".into(), c("#414339"));
    colors.insert("input.foreground".into(), c("#F8F8F2"));
    colors.insert("input.border".into(), c("#555651"));
    colors.insert("focusBorder".into(), c("#75715E"));
    colors.insert("list.activeSelectionBackground".into(), c("#49483E"));

    let token_colors = vec![
        token("Comment", &["comment", "punctuation.definition.comment"], "#75715E", Some("italic")),
        token("String", &["string", "string.quoted"], "#E6DB74", None),
        token("Number", &["constant.numeric"], "#AE81FF", None),
        token("Keyword", &["keyword", "storage.type", "storage.modifier"], "#F92672", None),
        token("Type", &["entity.name.type", "support.type"], "#A6E22E", Some("italic")),
        token("Function", &["entity.name.function", "support.function"], "#A6E22E", None),
        token("Variable", &["variable", "variable.other"], "#F8F8F2", None),
        token("Constant", &["constant", "constant.language"], "#AE81FF", None),
        token("Operator", &["keyword.operator"], "#F92672", None),
        token("Parameter", &["variable.parameter"], "#FD971F", Some("italic")),
        token("Property", &["variable.other.property"], "#F8F8F2", None),
        token("Tag", &["entity.name.tag"], "#F92672", None),
        token("Attribute", &["entity.other.attribute-name"], "#A6E22E", None),
        token("Punctuation", &["punctuation"], "#F8F8F2", None),
        token("Escape", &["constant.character.escape"], "#AE81FF", None),
    ];

    ColorTheme {
        id: "monokai".into(),
        label: "Monokai".into(),
        theme_type: ThemeType::Dark,
        colors,
        token_colors,
    }
}

/// Returns the built-in Solarized Dark theme.
pub fn solarized_dark() -> ColorTheme {
    let mut colors = HashMap::new();
    let c = |hex: &str| Color::from_hex(hex).unwrap();

    colors.insert("editor.background".into(), c("#002B36"));
    colors.insert("editor.foreground".into(), c("#839496"));
    colors.insert("editor.lineHighlightBackground".into(), c("#073642"));
    colors.insert("editor.selectionBackground".into(), c("#274642"));
    colors.insert("editorCursor.foreground".into(), c("#D30102"));
    colors.insert("editorWhitespace.foreground".into(), c("#073642"));
    colors.insert("editorLineNumber.foreground".into(), c("#586E75"));
    colors.insert("editorLineNumber.activeForeground".into(), c("#93A1A1"));
    colors.insert("editorIndentGuide.background".into(), c("#073642"));
    colors.insert("editorIndentGuide.activeBackground".into(), c("#586E75"));
    colors.insert("activityBar.background".into(), c("#003847"));
    colors.insert("activityBar.foreground".into(), c("#93A1A1"));
    colors.insert("sideBar.background".into(), c("#00212B"));
    colors.insert("sideBar.foreground".into(), c("#839496"));
    colors.insert("sideBarTitle.foreground".into(), c("#93A1A1"));
    colors.insert("statusBar.background".into(), c("#003847"));
    colors.insert("statusBar.foreground".into(), c("#93A1A1"));
    colors.insert("titleBar.activeBackground".into(), c("#002B36"));
    colors.insert("titleBar.activeForeground".into(), c("#93A1A1"));
    colors.insert("tab.activeBackground".into(), c("#002B36"));
    colors.insert("tab.activeForeground".into(), c("#FDF6E3"));
    colors.insert("tab.inactiveBackground".into(), c("#00212B"));
    colors.insert("tab.inactiveForeground".into(), c("#586E75"));
    colors.insert("panel.background".into(), c("#002B36"));
    colors.insert("panel.border".into(), c("#073642"));
    colors.insert("input.background".into(), c("#073642"));
    colors.insert("input.foreground".into(), c("#93A1A1"));
    colors.insert("input.border".into(), c("#586E75"));
    colors.insert("focusBorder".into(), c("#268BD2"));
    colors.insert("list.activeSelectionBackground".into(), c("#073642"));

    let token_colors = vec![
        token("Comment", &["comment", "punctuation.definition.comment"], "#586E75", Some("italic")),
        token("String", &["string", "string.quoted"], "#2AA198", None),
        token("Number", &["constant.numeric"], "#D33682", None),
        token("Keyword", &["keyword", "storage.type", "storage.modifier"], "#859900", None),
        token("Type", &["entity.name.type", "support.type"], "#B58900", None),
        token("Function", &["entity.name.function", "support.function"], "#268BD2", None),
        token("Variable", &["variable", "variable.other"], "#839496", None),
        token("Constant", &["constant", "constant.language"], "#CB4B16", None),
        token("Operator", &["keyword.operator"], "#859900", None),
        token("Parameter", &["variable.parameter"], "#839496", None),
        token("Property", &["variable.other.property"], "#268BD2", None),
        token("Tag", &["entity.name.tag"], "#268BD2", None),
        token("Attribute", &["entity.other.attribute-name"], "#93A1A1", None),
        token("Punctuation", &["punctuation"], "#839496", None),
        token("Escape", &["constant.character.escape"], "#CB4B16", None),
    ];

    ColorTheme {
        id: "solarized-dark".into(),
        label: "Solarized Dark".into(),
        theme_type: ThemeType::Dark,
        colors,
        token_colors,
    }
}

/// Returns the built-in High Contrast theme (dark).
pub fn high_contrast() -> ColorTheme {
    let mut colors = HashMap::new();
    let c = |hex: &str| Color::from_hex(hex).unwrap();

    colors.insert("editor.background".into(), c("#000000"));
    colors.insert("editor.foreground".into(), c("#FFFFFF"));
    colors.insert("editor.lineHighlightBackground".into(), c("#000000"));
    colors.insert("editor.selectionBackground".into(), c("#264F78"));
    colors.insert("editorCursor.foreground".into(), c("#FFFFFF"));
    colors.insert("editorWhitespace.foreground".into(), c("#6B6B6B"));
    colors.insert("editorLineNumber.foreground".into(), c("#FFFFFF"));
    colors.insert("editorLineNumber.activeForeground".into(), c("#FFFFFF"));
    colors.insert("editorIndentGuide.background".into(), c("#6B6B6B"));
    colors.insert("editorIndentGuide.activeBackground".into(), c("#FFFFFF"));
    colors.insert("activityBar.background".into(), c("#000000"));
    colors.insert("activityBar.foreground".into(), c("#FFFFFF"));
    colors.insert("sideBar.background".into(), c("#000000"));
    colors.insert("sideBar.foreground".into(), c("#FFFFFF"));
    colors.insert("sideBarTitle.foreground".into(), c("#FFFFFF"));
    colors.insert("statusBar.background".into(), c("#000000"));
    colors.insert("statusBar.foreground".into(), c("#FFFFFF"));
    colors.insert("titleBar.activeBackground".into(), c("#000000"));
    colors.insert("titleBar.activeForeground".into(), c("#FFFFFF"));
    colors.insert("tab.activeBackground".into(), c("#000000"));
    colors.insert("tab.activeForeground".into(), c("#FFFFFF"));
    colors.insert("tab.inactiveBackground".into(), c("#000000"));
    colors.insert("tab.inactiveForeground".into(), c("#FFFFFF"));
    colors.insert("contrastBorder".into(), c("#6FC3DF"));
    colors.insert("contrastActiveBorder".into(), c("#F38518"));
    colors.insert("panel.background".into(), c("#000000"));
    colors.insert("panel.border".into(), c("#6FC3DF"));
    colors.insert("input.background".into(), c("#000000"));
    colors.insert("input.foreground".into(), c("#FFFFFF"));
    colors.insert("input.border".into(), c("#6FC3DF"));
    colors.insert("focusBorder".into(), c("#F38518"));
    colors.insert("list.activeSelectionBackground".into(), c("#000000"));

    let token_colors = vec![
        token("Comment", &["comment", "punctuation.definition.comment"], "#7CA668", None),
        token("String", &["string", "string.quoted"], "#CE9178", None),
        token("Number", &["constant.numeric"], "#B5CEA8", None),
        token("Keyword", &["keyword", "storage.type", "storage.modifier"], "#569CD6", Some("bold")),
        token("Type", &["entity.name.type", "support.type"], "#4EC9B0", Some("bold")),
        token("Function", &["entity.name.function", "support.function"], "#DCDCAA", None),
        token("Variable", &["variable", "variable.other"], "#9CDCFE", None),
        token("Constant", &["constant", "constant.language"], "#569CD6", Some("bold")),
        token("Operator", &["keyword.operator"], "#FFFFFF", None),
        token("Parameter", &["variable.parameter"], "#9CDCFE", None),
        token("Property", &["variable.other.property"], "#9CDCFE", None),
        token("Tag", &["entity.name.tag"], "#569CD6", Some("bold")),
        token("Attribute", &["entity.other.attribute-name"], "#9CDCFE", None),
        token("Punctuation", &["punctuation"], "#FFFFFF", None),
        token("Escape", &["constant.character.escape"], "#D7BA7D", Some("bold")),
    ];

    ColorTheme {
        id: "hc-black".into(),
        label: "High Contrast".into(),
        theme_type: ThemeType::HighContrast,
        colors,
        token_colors,
    }
}

/// Returns the built-in High Contrast Light theme.
pub fn high_contrast_light() -> ColorTheme {
    let mut colors = HashMap::new();
    let c = |hex: &str| Color::from_hex(hex).unwrap();

    colors.insert("editor.background".into(), c("#FFFFFF"));
    colors.insert("editor.foreground".into(), c("#000000"));
    colors.insert("editor.lineHighlightBackground".into(), c("#FFFFFF"));
    colors.insert("editor.selectionBackground".into(), c("#0F4A85"));
    colors.insert("editorCursor.foreground".into(), c("#000000"));
    colors.insert("editorWhitespace.foreground".into(), c("#6B6B6B"));
    colors.insert("editorLineNumber.foreground".into(), c("#000000"));
    colors.insert("editorLineNumber.activeForeground".into(), c("#000000"));
    colors.insert("editorIndentGuide.background".into(), c("#6B6B6B"));
    colors.insert("editorIndentGuide.activeBackground".into(), c("#000000"));
    colors.insert("activityBar.background".into(), c("#FFFFFF"));
    colors.insert("activityBar.foreground".into(), c("#000000"));
    colors.insert("sideBar.background".into(), c("#FFFFFF"));
    colors.insert("sideBar.foreground".into(), c("#000000"));
    colors.insert("sideBarTitle.foreground".into(), c("#000000"));
    colors.insert("statusBar.background".into(), c("#FFFFFF"));
    colors.insert("statusBar.foreground".into(), c("#000000"));
    colors.insert("titleBar.activeBackground".into(), c("#FFFFFF"));
    colors.insert("titleBar.activeForeground".into(), c("#000000"));
    colors.insert("tab.activeBackground".into(), c("#FFFFFF"));
    colors.insert("tab.activeForeground".into(), c("#000000"));
    colors.insert("tab.inactiveBackground".into(), c("#FFFFFF"));
    colors.insert("tab.inactiveForeground".into(), c("#000000"));
    colors.insert("contrastBorder".into(), c("#0F4A85"));
    colors.insert("contrastActiveBorder".into(), c("#B5200D"));
    colors.insert("panel.background".into(), c("#FFFFFF"));
    colors.insert("panel.border".into(), c("#0F4A85"));
    colors.insert("input.background".into(), c("#FFFFFF"));
    colors.insert("input.foreground".into(), c("#000000"));
    colors.insert("input.border".into(), c("#0F4A85"));
    colors.insert("focusBorder".into(), c("#B5200D"));
    colors.insert("list.activeSelectionBackground".into(), c("#FFFFFF"));

    let token_colors = vec![
        token("Comment", &["comment", "punctuation.definition.comment"], "#008000", None),
        token("String", &["string", "string.quoted"], "#A31515", None),
        token("Number", &["constant.numeric"], "#098658", None),
        token("Keyword", &["keyword", "storage.type", "storage.modifier"], "#0000FF", Some("bold")),
        token("Type", &["entity.name.type", "support.type"], "#267F99", Some("bold")),
        token("Function", &["entity.name.function", "support.function"], "#795E26", None),
        token("Variable", &["variable", "variable.other"], "#001080", None),
        token("Constant", &["constant", "constant.language"], "#0000FF", Some("bold")),
        token("Operator", &["keyword.operator"], "#000000", None),
        token("Parameter", &["variable.parameter"], "#001080", None),
        token("Property", &["variable.other.property"], "#001080", None),
        token("Tag", &["entity.name.tag"], "#800000", Some("bold")),
        token("Attribute", &["entity.other.attribute-name"], "#FF0000", None),
        token("Punctuation", &["punctuation"], "#000000", None),
        token("Escape", &["constant.character.escape"], "#EE0000", Some("bold")),
    ];

    ColorTheme {
        id: "hc-light".into(),
        label: "High Contrast Light".into(),
        theme_type: ThemeType::HighContrastLight,
        colors,
        token_colors,
    }
}

/// Returns all built-in themes.
pub fn builtin_themes() -> Vec<ColorTheme> {
    vec![
        dark_plus(),
        light_plus(),
        monokai(),
        solarized_dark(),
        high_contrast(),
        high_contrast_light(),
    ]
}

// ---------------------------------------------------------------------------
// ThemeMixer — blend two themes
// ---------------------------------------------------------------------------

/// Blends two colors using linear interpolation.
/// `t` is the blend factor: 0.0 = entirely `a`, 1.0 = entirely `b`.
pub fn blend_colors(a: &Color, b: &Color, t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    Color::rgba(
        (a.r as f64 * inv + b.r as f64 * t).round() as u8,
        (a.g as f64 * inv + b.g as f64 * t).round() as u8,
        (a.b as f64 * inv + b.b as f64 * t).round() as u8,
        (a.a as f64 * inv + b.a as f64 * t).round() as u8,
    )
}

/// Mixes two `ColorTheme`s by blending their workbench colors.
///
/// The result uses `base`'s metadata (name, theme_type, token_colors)
/// and blends workbench colors with `overlay` at the given factor.
pub struct ThemeMixer;

impl ThemeMixer {
    /// Blend workbench colors from two themes.
    /// Returns a new map with all keys from both themes blended at factor `t`.
    pub fn blend_workbench_colors(
        base: &HashMap<String, Color>,
        overlay: &HashMap<String, Color>,
        t: f64,
    ) -> HashMap<String, Color> {
        let mut result = HashMap::new();
        for (key, base_color) in base {
            let blended = match overlay.get(key) {
                Some(overlay_color) => blend_colors(base_color, overlay_color, t),
                None => *base_color,
            };
            result.insert(key.clone(), blended);
        }
        // Add keys only in overlay
        for (key, overlay_color) in overlay {
            if !base.contains_key(key) {
                result.insert(key.clone(), *overlay_color);
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// WCAG contrast ratio validation
// ---------------------------------------------------------------------------

/// Compute the relative luminance of a color (sRGB).
/// See <https://www.w3.org/TR/WCAG20/#relativeluminancedef>.
pub fn relative_luminance(c: &Color) -> f64 {
    fn linearize(channel: u8) -> f64 {
        let s = channel as f64 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linearize(c.r) + 0.7152 * linearize(c.g) + 0.0722 * linearize(c.b)
}

/// Compute the WCAG contrast ratio between two colors.
/// Result is in [1.0, 21.0].
pub fn contrast_ratio(a: &Color, b: &Color) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (lighter, darker) = if la > lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}

/// WCAG conformance level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WcagLevel {
    /// Contrast ratio < 3.0 — fails all criteria.
    Fail,
    /// Contrast ratio >= 3.0 — passes for large text (AA large).
    AALarge,
    /// Contrast ratio >= 4.5 — passes AA for normal text.
    AA,
    /// Contrast ratio >= 7.0 — passes AAA for normal text.
    AAA,
}

impl WcagLevel {
    /// Determine the WCAG level from a contrast ratio.
    pub fn from_ratio(ratio: f64) -> Self {
        if ratio >= 7.0 {
            Self::AAA
        } else if ratio >= 4.5 {
            Self::AA
        } else if ratio >= 3.0 {
            Self::AALarge
        } else {
            Self::Fail
        }
    }
}

/// Validate that a foreground/background pair meets a minimum WCAG level.
pub fn validate_contrast(fg: &Color, bg: &Color, minimum: WcagLevel) -> bool {
    let ratio = contrast_ratio(fg, bg);
    let actual = WcagLevel::from_ratio(ratio);
    match minimum {
        WcagLevel::Fail => true,
        WcagLevel::AALarge => matches!(actual, WcagLevel::AALarge | WcagLevel::AA | WcagLevel::AAA),
        WcagLevel::AA => matches!(actual, WcagLevel::AA | WcagLevel::AAA),
        WcagLevel::AAA => matches!(actual, WcagLevel::AAA),
    }
}

// ---------------------------------------------------------------------------
// Theme color palette extraction
// ---------------------------------------------------------------------------

/// Extracts the unique color palette from a theme's workbench colors.
#[derive(Debug, Clone)]
pub struct ColorPalette {
    colors: Vec<Color>,
}

impl ColorPalette {
    /// Extract unique colors from a workbench color map.
    pub fn from_workbench_colors(colors: &HashMap<String, Color>) -> Self {
        let mut seen = std::collections::HashSet::new();
        let mut palette = Vec::new();
        for color in colors.values() {
            let key = (color.r, color.g, color.b, color.a);
            if seen.insert(key) {
                palette.push(*color);
            }
        }
        palette.sort_by_key(|c| (c.r, c.g, c.b, c.a));
        Self { colors: palette }
    }

    /// Number of unique colors.
    pub fn len(&self) -> usize {
        self.colors.len()
    }

    /// Whether the palette is empty.
    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    /// Get the palette colors.
    pub fn colors(&self) -> &[Color] {
        &self.colors
    }

    /// Find the darkest color (lowest luminance).
    pub fn darkest(&self) -> Option<&Color> {
        self.colors
            .iter()
            .min_by(|a, b| relative_luminance(a).partial_cmp(&relative_luminance(b)).unwrap())
    }

    /// Find the lightest color (highest luminance).
    pub fn lightest(&self) -> Option<&Color> {
        self.colors
            .iter()
            .max_by(|a, b| relative_luminance(a).partial_cmp(&relative_luminance(b)).unwrap())
    }

    /// Average color of the palette.
    pub fn average(&self) -> Option<Color> {
        if self.colors.is_empty() {
            return None;
        }
        let n = self.colors.len() as f64;
        let r = self.colors.iter().map(|c| c.r as f64).sum::<f64>() / n;
        let g = self.colors.iter().map(|c| c.g as f64).sum::<f64>() / n;
        let b = self.colors.iter().map(|c| c.b as f64).sum::<f64>() / n;
        let a = self.colors.iter().map(|c| c.a as f64).sum::<f64>() / n;
        Some(Color::rgba(r.round() as u8, g.round() as u8, b.round() as u8, a.round() as u8))
    }
}

// ---------------------------------------------------------------------------
// Color operations
// ---------------------------------------------------------------------------

impl Color {
    /// Returns the complementary color (hue rotated 180°).
    pub fn complementary(&self) -> Self {
        Self::rgb(255 - self.r, 255 - self.g, 255 - self.b)
    }

    /// Lighten the color by a percentage (0.0–1.0).
    pub fn lighten(&self, amount: f64) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        Self::rgba(
            (self.r as f64 + (255.0 - self.r as f64) * amount).round() as u8,
            (self.g as f64 + (255.0 - self.g as f64) * amount).round() as u8,
            (self.b as f64 + (255.0 - self.b as f64) * amount).round() as u8,
            self.a,
        )
    }

    /// Darken the color by a percentage (0.0–1.0).
    pub fn darken(&self, amount: f64) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        Self::rgba(
            (self.r as f64 * (1.0 - amount)).round() as u8,
            (self.g as f64 * (1.0 - amount)).round() as u8,
            (self.b as f64 * (1.0 - amount)).round() as u8,
            self.a,
        )
    }

    /// Convert to greyscale using luminance weighting.
    pub fn to_greyscale(&self) -> Self {
        let lum = (0.2126 * self.r as f64 + 0.7152 * self.g as f64 + 0.0722 * self.b as f64).round() as u8;
        Self::rgba(lum, lum, lum, self.a)
    }

    /// Returns true if this color is considered "dark" (luminance < 0.5).
    pub fn is_dark(&self) -> bool {
        relative_luminance(self) < 0.5
    }

    /// Convert to HSL representation: (hue 0–360, saturation 0–1, lightness 0–1).
    pub fn to_hsl(&self) -> (f64, f64, f64) {
        let r = self.r as f64 / 255.0;
        let g = self.g as f64 / 255.0;
        let b = self.b as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if (max - min).abs() < f64::EPSILON {
            return (0.0, 0.0, l);
        }
        let d = max - min;
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let h = if (max - r).abs() < f64::EPSILON {
            let mut h = (g - b) / d;
            if g < b { h += 6.0; }
            h
        } else if (max - g).abs() < f64::EPSILON {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h * 60.0, s, l)
    }

    /// Create a color from HSL values (hue 0–360, saturation 0–1, lightness 0–1).
    pub fn from_hsl(h: f64, s: f64, l: f64) -> Self {
        let s = s.clamp(0.0, 1.0);
        let l = l.clamp(0.0, 1.0);
        if s.abs() < f64::EPSILON {
            let v = (l * 255.0).round() as u8;
            return Self::rgb(v, v, v);
        }
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        let h = h / 360.0;
        let hue_to_rgb = |t: f64| -> f64 {
            let t = ((t % 1.0) + 1.0) % 1.0;
            if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            }
        };
        Self::rgb(
            (hue_to_rgb(h + 1.0 / 3.0) * 255.0).round() as u8,
            (hue_to_rgb(h) * 255.0).round() as u8,
            (hue_to_rgb(h - 1.0 / 3.0) * 255.0).round() as u8,
        )
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

// ---------------------------------------------------------------------------
// Theme diff
// ---------------------------------------------------------------------------

/// A single color change between two themes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorChange {
    pub key: String,
    pub old: Option<Color>,
    pub new: Option<Color>,
}

/// Computes the difference between two themes' workbench colors.
///
/// Returns a list of keys that were added, removed, or changed.
pub fn theme_diff(old: &ColorTheme, new: &ColorTheme) -> Vec<ColorChange> {
    let mut changes = Vec::new();
    // Changed or removed in new
    for (key, old_color) in &old.colors {
        match new.colors.get(key) {
            Some(new_color) if new_color != old_color => {
                changes.push(ColorChange { key: key.clone(), old: Some(*old_color), new: Some(*new_color) });
            }
            None => {
                changes.push(ColorChange { key: key.clone(), old: Some(*old_color), new: None });
            }
            _ => {}
        }
    }
    // Added in new
    for (key, new_color) in &new.colors {
        if !old.colors.contains_key(key) {
            changes.push(ColorChange { key: key.clone(), old: None, new: Some(*new_color) });
        }
    }
    changes.sort_by(|a, b| a.key.cmp(&b.key));
    changes
}

// ---------------------------------------------------------------------------
// Theme validation
// ---------------------------------------------------------------------------

/// Core workbench color keys that a well-formed theme should define.
pub const REQUIRED_COLOR_KEYS: &[&str] = &[
    "editor.background",
    "editor.foreground",
    "editorCursor.foreground",
    "editorLineNumber.foreground",
    "editor.selectionBackground",
    "editor.lineHighlightBackground",
    "statusBar.background",
    "statusBar.foreground",
    "sideBar.background",
    "sideBar.foreground",
    "activityBar.background",
    "activityBar.foreground",
    "tab.activeBackground",
    "tab.activeForeground",
    "tab.inactiveBackground",
    "tab.inactiveForeground",
    "titleBar.activeBackground",
    "titleBar.activeForeground",
];

/// A validation issue found in a theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeValidationIssue {
    pub key: String,
    pub kind: ValidationIssueKind,
}

/// Kind of theme validation issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationIssueKind {
    /// A required color key is missing.
    MissingRequired,
    /// Foreground/background contrast is too low for accessibility.
    LowContrast { ratio: u32 },
}

/// Validate a theme, checking for missing required colors and low-contrast pairs.
pub fn validate_theme(theme: &ColorTheme) -> Vec<ThemeValidationIssue> {
    let mut issues = Vec::new();

    for &key in REQUIRED_COLOR_KEYS {
        if !theme.colors.contains_key(key) {
            issues.push(ThemeValidationIssue {
                key: key.to_string(),
                kind: ValidationIssueKind::MissingRequired,
            });
        }
    }

    // Check editor foreground/background contrast
    if let (Some(fg), Some(bg)) = (
        theme.colors.get("editor.foreground"),
        theme.colors.get("editor.background"),
    ) {
        let ratio = contrast_ratio(fg, bg);
        if ratio < 3.0 {
            issues.push(ThemeValidationIssue {
                key: "editor.foreground/editor.background".to_string(),
                kind: ValidationIssueKind::LowContrast { ratio: (ratio * 100.0) as u32 },
            });
        }
    }

    issues
}

// ---------------------------------------------------------------------------
// Theme inheritance
// ---------------------------------------------------------------------------

impl ColorTheme {
    /// Create a child theme that inherits from this (parent) theme.
    ///
    /// The child's colors override the parent's; token colors from the child
    /// take priority (appended after parent rules, so they match first by
    /// the longest-prefix algorithm only when scopes overlap).
    pub fn with_overrides(&self, overrides: &ColorTheme) -> Self {
        let mut colors = self.colors.clone();
        colors.extend(overrides.colors.iter().map(|(k, v)| (k.clone(), *v)));

        let mut token_colors = self.token_colors.clone();
        token_colors.extend(overrides.token_colors.iter().cloned());

        Self {
            id: overrides.id.clone(),
            label: overrides.label.clone(),
            theme_type: overrides.theme_type,
            colors,
            token_colors,
        }
    }

    /// Returns all workbench color keys defined in this theme, sorted.
    pub fn color_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.colors.keys().map(|s| s.as_str()).collect();
        keys.sort();
        keys
    }

    /// Count of token color rules.
    pub fn token_color_count(&self) -> usize {
        self.token_colors.len()
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

    // -- Monokai built-in --

    #[test]
    fn monokai_has_enough_colors() {
        let theme = monokai();
        assert!(theme.colors.len() >= 30, "monokai: {} colors", theme.colors.len());
        assert!(theme.token_colors.len() >= 10);
        assert_eq!(theme.theme_type, ThemeType::Dark);
        assert_eq!(theme.id, "monokai");
    }

    #[test]
    fn monokai_keyword_color() {
        let theme = monokai();
        let s = theme.get_token_color(&["keyword"]).unwrap();
        assert_eq!(s.foreground, Color::from_hex("#F92672"));
    }

    // -- Solarized Dark built-in --

    #[test]
    fn solarized_dark_has_enough_colors() {
        let theme = solarized_dark();
        assert!(theme.colors.len() >= 30, "solarized: {} colors", theme.colors.len());
        assert!(theme.token_colors.len() >= 10);
        assert_eq!(theme.theme_type, ThemeType::Dark);
    }

    #[test]
    fn solarized_dark_background() {
        let theme = solarized_dark();
        assert_eq!(theme.get_color("editor.background"), Some(&Color::from_hex("#002B36").unwrap()));
    }

    // -- High contrast built-in --

    #[test]
    fn high_contrast_theme() {
        let theme = high_contrast();
        assert_eq!(theme.theme_type, ThemeType::HighContrast);
        assert!(theme.is_high_contrast());
        assert!(theme.colors.len() >= 30);
        assert_eq!(theme.get_color("editor.background"), Some(&Color::from_hex("#000000").unwrap()));
        assert!(theme.colors.contains_key("contrastBorder"));
    }

    #[test]
    fn high_contrast_light_theme() {
        let theme = high_contrast_light();
        assert_eq!(theme.theme_type, ThemeType::HighContrastLight);
        assert!(theme.is_high_contrast());
        assert!(theme.colors.len() >= 30);
        assert_eq!(theme.get_color("editor.background"), Some(&Color::from_hex("#FFFFFF").unwrap()));
    }

    #[test]
    fn is_high_contrast_false_for_normal() {
        assert!(!dark_plus().is_high_contrast());
        assert!(!light_plus().is_high_contrast());
        assert!(!monokai().is_high_contrast());
    }

    // -- TerminalColor --

    #[test]
    fn terminal_color_true_color() {
        let c = Color::rgb(0x1E, 0x1E, 0x1E);
        let tc = TerminalColor::from_color(&c, true);
        assert_eq!(tc, TerminalColor::Rgb(0x1E, 0x1E, 0x1E));
    }

    #[test]
    fn terminal_color_256() {
        let c = Color::rgb(0xFF, 0x00, 0x00);
        let tc = TerminalColor::from_color(&c, false);
        match tc {
            TerminalColor::Indexed(idx) => assert!(idx >= 16, "expected 256-color idx, got {idx}"),
            _ => panic!("expected Indexed"),
        }
    }

    #[test]
    fn terminal_color_greyscale() {
        let c = Color::rgb(128, 128, 128);
        let tc = TerminalColor::from_color(&c, false);
        match tc {
            TerminalColor::Indexed(idx) => assert!(idx >= 232, "grey should map to greyscale ramp, got {idx}"),
            _ => panic!("expected Indexed"),
        }
    }

    #[test]
    fn terminal_color_black() {
        let tc = TerminalColor::from_rgb_256(0, 0, 0);
        assert_eq!(tc, TerminalColor::Indexed(16));
    }

    #[test]
    fn terminal_color_white() {
        let tc = TerminalColor::from_rgb_256(255, 255, 255);
        assert_eq!(tc, TerminalColor::Indexed(231));
    }

    // -- resolve_color --

    #[test]
    fn resolve_color_found() {
        let theme = dark_plus();
        let tc = resolve_color(&theme, "editor.background", true).unwrap();
        assert_eq!(tc, TerminalColor::Rgb(0x1E, 0x1E, 0x1E));
    }

    #[test]
    fn resolve_color_missing() {
        let theme = dark_plus();
        assert!(resolve_color(&theme, "nonexistent", true).is_none());
    }

    #[test]
    fn resolve_color_256_mode() {
        let theme = dark_plus();
        let tc = resolve_color(&theme, "statusBar.background", false).unwrap();
        match tc {
            TerminalColor::Indexed(_) => {}
            _ => panic!("expected Indexed in 256-color mode"),
        }
    }

    // -- TokenStyle --

    #[test]
    fn token_style_from_settings() {
        let settings = TokenSettings {
            foreground: Color::from_hex("#FF0000"),
            background: Color::from_hex("#00FF00"),
            font_style: Some("bold italic underline".into()),
        };
        let style = TokenStyle::from_settings(&settings);
        assert_eq!(style.foreground, Color::from_hex("#FF0000"));
        assert_eq!(style.background, Color::from_hex("#00FF00"));
        assert!(style.bold);
        assert!(style.italic);
        assert!(style.underline);
    }

    #[test]
    fn token_style_no_font_style() {
        let settings = TokenSettings {
            foreground: Color::from_hex("#FF0000"),
            background: None,
            font_style: None,
        };
        let style = TokenStyle::from_settings(&settings);
        assert!(!style.bold);
        assert!(!style.italic);
        assert!(!style.underline);
    }

    #[test]
    fn get_token_style_from_theme() {
        let theme = monokai();
        let style = theme.get_token_style(&["comment"]).unwrap();
        assert_eq!(style.foreground, Color::from_hex("#75715E"));
        assert!(style.italic);
        assert!(!style.bold);
    }

    #[test]
    fn get_token_style_none() {
        let theme = dark_plus();
        assert!(theme.get_token_style(&["meta.unknown.scope"]).is_none());
    }

    // -- parse_theme_file --

    #[test]
    fn parse_theme_file_basic() {
        let dir = std::env::temp_dir().join("vsedit_test_theme_basic");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_theme.json");
        std::fs::write(&path, r##"{
            "name": "File Theme",
            "type": "dark",
            "colors": { "editor.background": "#111111" },
            "tokenColors": [
                { "scope": "comment", "settings": { "foreground": "#AAAAAA" } }
            ]
        }"##).unwrap();

        let theme = parse_theme_file(&path).unwrap();
        assert_eq!(theme.label, "File Theme");
        assert_eq!(theme.get_color("editor.background"), Some(&Color::from_hex("#111111").unwrap()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_theme_file_with_include() {
        let dir = std::env::temp_dir().join("vsedit_test_theme_include");
        let _ = std::fs::create_dir_all(&dir);

        // Base theme
        std::fs::write(dir.join("base.json"), r##"{
            "name": "Base",
            "type": "dark",
            "colors": { "editor.background": "#111111", "editor.foreground": "#CCCCCC" },
            "tokenColors": [
                { "scope": "comment", "settings": { "foreground": "#AAAAAA" } }
            ]
        }"##).unwrap();

        // Child theme includes base and overrides one color
        std::fs::write(dir.join("child.json"), r##"{
            "name": "Child",
            "type": "dark",
            "include": "base.json",
            "colors": { "editor.background": "#222222" },
            "tokenColors": [
                { "scope": "string", "settings": { "foreground": "#BBBBBB" } }
            ]
        }"##).unwrap();

        let theme = parse_theme_file(&dir.join("child.json")).unwrap();
        // Background overridden by child
        assert_eq!(theme.get_color("editor.background"), Some(&Color::from_hex("#222222").unwrap()));
        // Foreground inherited from base
        assert_eq!(theme.get_color("editor.foreground"), Some(&Color::from_hex("#CCCCCC").unwrap()));
        // Token colors merged
        assert_eq!(theme.token_colors.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_theme_file_not_found() {
        let result = parse_theme_file(Path::new("/nonexistent/theme.json"));
        assert!(result.is_err());
    }

    // -- builtin_themes --

    #[test]
    fn builtin_themes_list() {
        let themes = builtin_themes();
        assert_eq!(themes.len(), 6);
        let ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"vs-dark-plus"));
        assert!(ids.contains(&"vs-light-plus"));
        assert!(ids.contains(&"monokai"));
        assert!(ids.contains(&"solarized-dark"));
        assert!(ids.contains(&"hc-black"));
        assert!(ids.contains(&"hc-light"));
    }

    // -- scope_matches edge cases --

    #[test]
    fn scope_matches_prefix_boundary() {
        assert!(scope_matches("string.quoted.double", "string"));
        assert!(scope_matches("string.quoted.double", "string.quoted"));
        assert!(!scope_matches("stringx", "string"));
        assert!(scope_matches("string", "string"));
    }

    // -- ThemeMixer tests ----------------------------------------------------

    #[test]
    fn blend_colors_midpoint() {
        let a = Color::rgb(0, 0, 0);
        let b = Color::rgb(200, 100, 50);
        let mid = blend_colors(&a, &b, 0.5);
        assert_eq!(mid.r, 100);
        assert_eq!(mid.g, 50);
        assert_eq!(mid.b, 25);
    }

    #[test]
    fn blend_workbench_colors_union() {
        let mut base = HashMap::new();
        base.insert("bg".into(), Color::rgb(0, 0, 0));
        let mut overlay = HashMap::new();
        overlay.insert("bg".into(), Color::rgb(100, 100, 100));
        overlay.insert("fg".into(), Color::rgb(200, 200, 200));
        let result = ThemeMixer::blend_workbench_colors(&base, &overlay, 0.5);
        assert_eq!(result.len(), 2);
        assert_eq!(result["bg"].r, 50);
        assert_eq!(result["fg"], Color::rgb(200, 200, 200));
    }

    // -- WCAG contrast tests -------------------------------------------------

    #[test]
    fn contrast_ratio_black_white() {
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        let ratio = contrast_ratio(&black, &white);
        assert!(ratio > 20.0);
        assert_eq!(WcagLevel::from_ratio(ratio), WcagLevel::AAA);
    }

    #[test]
    fn contrast_ratio_same_color_is_one() {
        let c = Color::rgb(128, 128, 128);
        let ratio = contrast_ratio(&c, &c);
        assert!((ratio - 1.0).abs() < 0.01);
        assert_eq!(WcagLevel::from_ratio(ratio), WcagLevel::Fail);
    }

    #[test]
    fn validate_contrast_checks_level() {
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        assert!(validate_contrast(&black, &white, WcagLevel::AAA));
        assert!(validate_contrast(&black, &white, WcagLevel::AA));
    }

    // -- ColorPalette tests --------------------------------------------------

    #[test]
    fn palette_extracts_unique_colors() {
        let mut colors = HashMap::new();
        colors.insert("a".into(), Color::rgb(255, 0, 0));
        colors.insert("b".into(), Color::rgb(0, 255, 0));
        colors.insert("c".into(), Color::rgb(255, 0, 0)); // duplicate
        let palette = ColorPalette::from_workbench_colors(&colors);
        assert_eq!(palette.len(), 2);
    }

    #[test]
    fn palette_darkest_lightest() {
        let mut colors = HashMap::new();
        colors.insert("dark".into(), Color::rgb(10, 10, 10));
        colors.insert("light".into(), Color::rgb(240, 240, 240));
        let palette = ColorPalette::from_workbench_colors(&colors);
        assert_eq!(palette.darkest().unwrap().r, 10);
        assert_eq!(palette.lightest().unwrap().r, 240);
        let avg = palette.average().unwrap();
        assert_eq!(avg.r, 125);
    }

    // -- Color operations ----------------------------------------------------

    #[test]
    fn color_complementary() {
        let c = Color::rgb(255, 0, 128);
        let comp = c.complementary();
        assert_eq!(comp, Color::rgb(0, 255, 127));
    }

    #[test]
    fn color_lighten_darken() {
        let c = Color::rgb(100, 100, 100);
        let lighter = c.lighten(0.5);
        assert!(lighter.r > c.r);
        assert!(lighter.g > c.g);
        let darker = c.darken(0.5);
        assert!(darker.r < c.r);
        assert!(darker.g < c.g);
        // Lighten/darken by 0 should be identity
        assert_eq!(c.lighten(0.0), c);
        assert_eq!(c.darken(0.0), c);
    }

    #[test]
    fn color_greyscale_and_is_dark() {
        let red = Color::rgb(255, 0, 0);
        let grey = red.to_greyscale();
        assert_eq!(grey.r, grey.g);
        assert_eq!(grey.g, grey.b);
        // Pure black is dark, pure white is not
        assert!(Color::rgb(0, 0, 0).is_dark());
        assert!(!Color::rgb(255, 255, 255).is_dark());
    }

    #[test]
    fn color_hsl_roundtrip() {
        let original = Color::rgb(200, 100, 50);
        let (h, s, l) = original.to_hsl();
        let restored = Color::from_hsl(h, s, l);
        // Allow ±1 rounding tolerance
        assert!((original.r as i16 - restored.r as i16).abs() <= 1);
        assert!((original.g as i16 - restored.g as i16).abs() <= 1);
        assert!((original.b as i16 - restored.b as i16).abs() <= 1);
    }

    #[test]
    fn color_display_trait() {
        let c = Color::rgb(0xAB, 0xCD, 0xEF);
        assert_eq!(format!("{c}"), "#ABCDEF");
    }

    // -- Theme diff ----------------------------------------------------------

    #[test]
    fn theme_diff_detects_changes() {
        let dark = dark_plus();
        let light = light_plus();
        let diff = theme_diff(&dark, &light);
        // Both have editor.background but with different values
        assert!(diff.iter().any(|c| c.key == "editor.background"
            && c.old == Some(Color::from_hex("#1E1E1E").unwrap())
            && c.new == Some(Color::from_hex("#FFFFFF").unwrap())));
    }

    #[test]
    fn theme_diff_empty_for_identical() {
        let t = dark_plus();
        let diff = theme_diff(&t, &t);
        assert!(diff.is_empty());
    }

    // -- Theme validation ----------------------------------------------------

    #[test]
    fn validate_builtin_themes_pass() {
        // All built-in themes should have all required keys
        for theme in builtin_themes() {
            let issues: Vec<_> = validate_theme(&theme).into_iter()
                .filter(|i| matches!(i.kind, ValidationIssueKind::MissingRequired))
                .collect();
            assert!(issues.is_empty(), "theme '{}' missing keys: {:?}", theme.id, issues);
        }
    }

    #[test]
    fn validate_incomplete_theme_reports_missing() {
        let theme = ColorTheme {
            id: "empty".into(),
            label: "Empty".into(),
            theme_type: ThemeType::Dark,
            colors: HashMap::new(),
            token_colors: Vec::new(),
        };
        let issues = validate_theme(&theme);
        let missing: Vec<_> = issues.iter()
            .filter(|i| matches!(i.kind, ValidationIssueKind::MissingRequired))
            .collect();
        assert_eq!(missing.len(), REQUIRED_COLOR_KEYS.len());
    }

    // -- Theme inheritance ---------------------------------------------------

    #[test]
    fn theme_with_overrides() {
        let parent = dark_plus();
        let mut child_colors = HashMap::new();
        child_colors.insert("editor.background".into(), Color::rgb(0, 0, 0));
        child_colors.insert("custom.color".into(), Color::rgb(1, 2, 3));
        let child = ColorTheme {
            id: "child".into(),
            label: "Child".into(),
            theme_type: ThemeType::Dark,
            colors: child_colors,
            token_colors: Vec::new(),
        };
        let merged = parent.with_overrides(&child);
        // Overridden
        assert_eq!(merged.get_color("editor.background"), Some(&Color::rgb(0, 0, 0)));
        // Inherited from parent
        assert_eq!(merged.get_color("editor.foreground"), parent.get_color("editor.foreground"));
        // Added by child
        assert_eq!(merged.get_color("custom.color"), Some(&Color::rgb(1, 2, 3)));
        assert_eq!(merged.id, "child");
    }

    #[test]
    fn color_keys_sorted() {
        let theme = dark_plus();
        let keys = theme.color_keys();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }
}
