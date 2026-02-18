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
// ThemeInheritance — structured theme extension
// ---------------------------------------------------------------------------

/// Describes how a child theme extends a base (parent) theme.
///
/// Colors and token rules from the child override matching entries in the
/// parent.  Unmatched parent entries are inherited as-is.
#[derive(Debug, Clone)]
pub struct ThemeInheritance {
    /// Identifier of the parent theme.
    pub parent_id: String,
    /// Color overrides supplied by the child.
    pub color_overrides: HashMap<String, Color>,
    /// Token-color overrides supplied by the child.
    pub token_overrides: Vec<TokenColor>,
}

impl ThemeInheritance {
    /// Build a new inheritance descriptor.
    pub fn new(parent_id: &str) -> Self {
        Self {
            parent_id: parent_id.to_string(),
            color_overrides: HashMap::new(),
            token_overrides: Vec::new(),
        }
    }

    /// Add a workbench color override.
    pub fn set_color(&mut self, key: &str, color: Color) -> &mut Self {
        self.color_overrides.insert(key.to_string(), color);
        self
    }

    /// Add a token-color override.
    pub fn add_token_override(&mut self, rule: TokenColor) -> &mut Self {
        self.token_overrides.push(rule);
        self
    }

    /// Apply this inheritance to a parent theme, producing the merged child.
    pub fn apply(&self, parent: &ColorTheme, child_id: &str, child_label: &str) -> ColorTheme {
        let mut colors = parent.colors.clone();
        colors.extend(self.color_overrides.iter().map(|(k, v)| (k.clone(), *v)));

        let mut token_colors = parent.token_colors.clone();
        token_colors.extend(self.token_overrides.iter().cloned());

        ColorTheme {
            id: child_id.to_string(),
            label: child_label.to_string(),
            theme_type: parent.theme_type,
            colors,
            token_colors,
        }
    }

    /// Returns the number of color overrides.
    pub fn color_override_count(&self) -> usize {
        self.color_overrides.len()
    }

    /// Returns the number of token-color overrides.
    pub fn token_override_count(&self) -> usize {
        self.token_overrides.len()
    }
}

// ---------------------------------------------------------------------------
// ThemeTokenColorCustomization — user-level scope overrides
// ---------------------------------------------------------------------------

/// User-provided overrides for specific TextMate scopes.
///
/// This mirrors VS Code's `editor.tokenColorCustomizations.textMateRules`
/// setting, allowing end-users to tweak individual token colors without
/// creating an entirely new theme file.
#[derive(Debug, Clone)]
pub struct ThemeTokenColorCustomization {
    rules: Vec<TokenColor>,
}

impl ThemeTokenColorCustomization {
    /// Create an empty customization set.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a customization rule for the given scopes.
    pub fn add_rule(
        &mut self,
        scopes: &[&str],
        foreground: Option<Color>,
        font_style: Option<&str>,
    ) -> &mut Self {
        self.rules.push(TokenColor {
            name: None,
            scope: scopes.iter().map(|s| (*s).to_string()).collect(),
            settings: TokenSettings {
                foreground,
                background: None,
                font_style: font_style.map(String::from),
            },
        });
        self
    }

    /// Apply these customizations on top of an existing theme, returning a
    /// new theme with the user rules appended (highest priority).
    pub fn apply(&self, theme: &ColorTheme) -> ColorTheme {
        let mut token_colors = theme.token_colors.clone();
        token_colors.extend(self.rules.iter().cloned());
        ColorTheme {
            id: theme.id.clone(),
            label: theme.label.clone(),
            theme_type: theme.theme_type,
            colors: theme.colors.clone(),
            token_colors,
        }
    }

    /// Number of customization rules.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Whether there are no customization rules.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ThemeContrastCalculator — batch WCAG checking
// ---------------------------------------------------------------------------

/// Result of a single contrast check between two theme color keys.
#[derive(Debug, Clone, PartialEq)]
pub struct ContrastViolation {
    /// The foreground color key.
    pub fg_key: String,
    /// The background color key.
    pub bg_key: String,
    /// The computed contrast ratio.
    pub ratio: f64,
    /// The WCAG level achieved.
    pub achieved: WcagLevel,
    /// The minimum level that was required.
    pub required: WcagLevel,
}

/// Well-known foreground/background pairs that should be checked together.
const CONTRAST_PAIRS: &[(&str, &str)] = &[
    ("editor.foreground", "editor.background"),
    ("editorLineNumber.foreground", "editor.background"),
    ("statusBar.foreground", "statusBar.background"),
    ("sideBar.foreground", "sideBar.background"),
    ("activityBar.foreground", "activityBar.background"),
    ("tab.activeForeground", "tab.activeBackground"),
    ("tab.inactiveForeground", "tab.inactiveBackground"),
    ("titleBar.activeForeground", "titleBar.activeBackground"),
];

/// Batch accessibility checker for a theme's color pairs.
///
/// Iterates over well-known foreground/background pairs and reports any that
/// fail to meet the requested [`WcagLevel`].
pub struct ThemeContrastCalculator;

impl ThemeContrastCalculator {
    /// Check all well-known pairs against the given minimum WCAG level.
    pub fn check(theme: &ColorTheme, minimum: WcagLevel) -> Vec<ContrastViolation> {
        let mut violations = Vec::new();
        for &(fg_key, bg_key) in CONTRAST_PAIRS {
            if let (Some(fg), Some(bg)) = (theme.colors.get(fg_key), theme.colors.get(bg_key)) {
                let ratio = contrast_ratio(fg, bg);
                let achieved = WcagLevel::from_ratio(ratio);
                if !validate_contrast(fg, bg, minimum) {
                    violations.push(ContrastViolation {
                        fg_key: fg_key.to_string(),
                        bg_key: bg_key.to_string(),
                        ratio,
                        achieved,
                        required: minimum,
                    });
                }
            }
        }
        violations
    }

    /// Check a custom list of (foreground_key, background_key) pairs.
    pub fn check_pairs(
        theme: &ColorTheme,
        pairs: &[(&str, &str)],
        minimum: WcagLevel,
    ) -> Vec<ContrastViolation> {
        let mut violations = Vec::new();
        for &(fg_key, bg_key) in pairs {
            if let (Some(fg), Some(bg)) = (theme.colors.get(fg_key), theme.colors.get(bg_key)) {
                let ratio = contrast_ratio(fg, bg);
                let achieved = WcagLevel::from_ratio(ratio);
                if !validate_contrast(fg, bg, minimum) {
                    violations.push(ContrastViolation {
                        fg_key: fg_key.to_string(),
                        bg_key: bg_key.to_string(),
                        ratio,
                        achieved,
                        required: minimum,
                    });
                }
            }
        }
        violations
    }

    /// Returns the number of well-known pairs that will be checked.
    pub fn pair_count() -> usize {
        CONTRAST_PAIRS.len()
    }
}

// ---------------------------------------------------------------------------
// parse_color_string — multi-format color parsing
// ---------------------------------------------------------------------------

/// Parse a color from various string formats:
///
/// - `#RGB`         (shorthand hex, e.g. `#F00` → red)
/// - `#RRGGBB`      (standard hex)
/// - `#RRGGBBAA`    (hex with alpha)
/// - `rgb(r, g, b)` (CSS-style, values 0–255)
/// - `rgba(r, g, b, a)` (CSS-style, `a` is 0.0–1.0)
pub fn parse_color_string(input: &str) -> Option<Color> {
    let s = input.trim();

    // Hex formats
    if let Some(hex) = s.strip_prefix('#') {
        return match hex.len() {
            3 => {
                // Expand #RGB → #RRGGBB
                let mut expanded = String::with_capacity(7);
                expanded.push('#');
                for ch in hex.chars() {
                    expanded.push(ch);
                    expanded.push(ch);
                }
                Color::from_hex(&expanded)
            }
            6 | 8 => Color::from_hex(s),
            _ => None,
        };
    }

    // rgb(r, g, b)
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 3 {
            let r = parts[0].trim().parse::<u8>().ok()?;
            let g = parts[1].trim().parse::<u8>().ok()?;
            let b = parts[2].trim().parse::<u8>().ok()?;
            return Some(Color::rgb(r, g, b));
        }
        return None;
    }

    // rgba(r, g, b, a)
    if let Some(inner) = s.strip_prefix("rgba(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 4 {
            let r = parts[0].trim().parse::<u8>().ok()?;
            let g = parts[1].trim().parse::<u8>().ok()?;
            let b = parts[2].trim().parse::<u8>().ok()?;
            let a_f: f64 = parts[3].trim().parse().ok()?;
            let a = (a_f.clamp(0.0, 1.0) * 255.0).round() as u8;
            return Some(Color::rgba(r, g, b, a));
        }
        return None;
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// === Theme Color Fallback Chain ===

/// Theme Color Fallback Chain implementation.
#[derive(Debug, Clone)]
pub struct ThemeColorFallbackChain {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: ThemeColorFallbackChainStats,
}

/// Statistics for ThemeColorFallbackChain.
#[derive(Debug, Clone, Default)]
pub struct ThemeColorFallbackChainStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl ThemeColorFallbackChainStats {
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

impl ThemeColorFallbackChain {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: ThemeColorFallbackChainStats::default(),
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

    pub fn stats(&self) -> &ThemeColorFallbackChainStats {
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

impl Default for ThemeColorFallbackChain {
    fn default() -> Self {
        Self::new()
    }
}

// === Theme Editor Token Colorizer ===

/// Priority level for ThemeEditorTokenColorizer items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeEditorTokenColorizerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl ThemeEditorTokenColorizerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for ThemeEditorTokenColorizerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Theme Editor Token Colorizer implementation.
#[derive(Debug, Clone)]
pub struct ThemeEditorTokenColorizer {
    items: Vec<ThemeEditorTokenColorizerItem>,
    max_items: usize,
    default_priority: ThemeEditorTokenColorizerPriority,
}

/// A single item in ThemeEditorTokenColorizer.
#[derive(Debug, Clone)]
pub struct ThemeEditorTokenColorizerItem {
    pub id: String,
    pub label: String,
    pub priority: ThemeEditorTokenColorizerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl ThemeEditorTokenColorizerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: ThemeEditorTokenColorizerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: ThemeEditorTokenColorizerPriority) -> Self {
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

impl ThemeEditorTokenColorizer {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: ThemeEditorTokenColorizerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: ThemeEditorTokenColorizerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<ThemeEditorTokenColorizerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&ThemeEditorTokenColorizerItem> {
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

    pub fn by_priority(&self, priority: ThemeEditorTokenColorizerPriority) -> Vec<&ThemeEditorTokenColorizerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ThemeEditorTokenColorizerItem> {
        let mut sorted: Vec<&ThemeEditorTokenColorizerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&ThemeEditorTokenColorizerItem> {
        let mut sorted: Vec<&ThemeEditorTokenColorizerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&ThemeEditorTokenColorizerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: ThemeEditorTokenColorizerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> ThemeEditorTokenColorizerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &ThemeEditorTokenColorizerItem> {
        self.items.iter()
    }
}

impl Default for ThemeEditorTokenColorizer {
    fn default() -> Self {
        Self::new()
    }
}


/// Theme configuration manager.
#[derive(Debug, Clone)]
pub struct ThemeConfig {
    entries: Vec<ThemeEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single theme entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl ThemeEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl ThemeConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: ThemeEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&ThemeEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ThemeEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&ThemeEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&ThemeEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&ThemeEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<ThemeEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
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
// xa_ extended helpers for theme
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaThemeRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaThemeRingBuf {
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
pub struct XaThemeCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaThemeCounter {
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

impl Default for XaThemeCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 182
// ---------------------------------------------------------------------------

/// Generic object pool `Xc182Pool<T>`.
pub struct Xc182Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc182Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc182PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc182Pool<T> {
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
    pub fn stats(&self) -> Xc182PoolStats {
        Xc182PoolStats {
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

impl<T> Default for Xc182Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc182Scheduler`.
pub struct Xc182Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc182Scheduler {
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

impl Default for Xc182Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_182 hash for the given byte slice.
pub fn xc_182_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_182 convention.
pub fn xc_182_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_57 deepening: state machine + event bus ---

/// States for the Xd57 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd57State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd57State {
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
pub struct Xd57Transition {
    pub from: Xd57State,
    pub to: Xd57State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd57StateMachine {
    current: Xd57State,
    history: Vec<Xd57Transition>,
    step_counter: usize,
}

impl Xd57StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd57State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd57State {
        self.current
    }

    pub fn history(&self) -> &[Xd57Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd57State) -> Result<Xd57State, String> {
        let allowed = match (self.current, target) {
            (Xd57State::Idle, Xd57State::Running) => true,
            (Xd57State::Running, Xd57State::Paused) => true,
            (Xd57State::Running, Xd57State::Done) => true,
            (Xd57State::Paused, Xd57State::Running) => true,
            (Xd57State::Paused, Xd57State::Done) => true,
            (Xd57State::Done, Xd57State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_57: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd57Transition {
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
            "Xd57SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd57State> {
        let prefix = "Xd57SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd57State::Idle),
            "Running" => Some(Xd57State::Running),
            "Paused" => Some(Xd57State::Paused),
            "Done" => Some(Xd57State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd57State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd57 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd57Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd57Event {
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

type Xd57HandlerFn = Box<dyn Fn(&Xd57Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd57EventBus {
    handlers: Vec<(usize, Option<String>, Xd57HandlerFn)>,
    next_id: usize,
    published: Vec<Xd57Event>,
}

impl Xd57EventBus {
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
        F: Fn(&Xd57Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd57Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd57Event) {
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

    pub fn published_events(&self) -> &[Xd57Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #55
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf55Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf55TrieNode {
    children: std::collections::HashMap<char, Xf55TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf55Trie {
    root: Xf55TrieNode,
    count: usize,
}

impl Xf55Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf55TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf55TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf55TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf55BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf55BloomFilter {
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

    // -- parse_color_string --------------------------------------------------

    #[test]
    fn parse_color_string_hex_rgb_short() {
        let c = parse_color_string("#F00").unwrap();
        assert_eq!(c, Color::rgb(255, 0, 0));
    }

    #[test]
    fn parse_color_string_hex_rrggbb() {
        let c = parse_color_string("#1E1E1E").unwrap();
        assert_eq!(c, Color::rgb(0x1E, 0x1E, 0x1E));
    }

    #[test]
    fn parse_color_string_hex_rrggbbaa() {
        let c = parse_color_string("#FF000080").unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 128));
    }

    #[test]
    fn parse_color_string_rgb_func() {
        let c = parse_color_string("rgb(10, 20, 30)").unwrap();
        assert_eq!(c, Color::rgb(10, 20, 30));
    }

    #[test]
    fn parse_color_string_rgba_func() {
        let c = parse_color_string("rgba(10, 20, 30, 0.5)").unwrap();
        assert_eq!(c, Color::rgba(10, 20, 30, 128));
    }

    #[test]
    fn parse_color_string_invalid_returns_none() {
        assert!(parse_color_string("not-a-color").is_none());
        assert!(parse_color_string("#GG0000").is_none());
        assert!(parse_color_string("rgb(256, 0, 0)").is_none());
        assert!(parse_color_string("#12345").is_none());
    }

    #[test]
    fn parse_color_string_whitespace_trimmed() {
        let c = parse_color_string("  #FF0000  ").unwrap();
        assert_eq!(c, Color::rgb(255, 0, 0));
    }

    // -- ThemeInheritance ----------------------------------------------------

    #[test]
    fn theme_inheritance_apply() {
        let parent = dark_plus();
        let mut inh = ThemeInheritance::new("vs-dark-plus");
        inh.set_color("editor.background", Color::rgb(0, 0, 0));
        inh.set_color("custom.new", Color::rgb(1, 2, 3));
        let child = inh.apply(&parent, "my-child", "My Child");
        assert_eq!(child.get_color("editor.background"), Some(&Color::rgb(0, 0, 0)));
        assert_eq!(child.get_color("editor.foreground"), parent.get_color("editor.foreground"));
        assert_eq!(child.get_color("custom.new"), Some(&Color::rgb(1, 2, 3)));
        assert_eq!(child.id, "my-child");
    }

    #[test]
    fn theme_inheritance_counts() {
        let mut inh = ThemeInheritance::new("parent");
        assert_eq!(inh.color_override_count(), 0);
        assert_eq!(inh.token_override_count(), 0);
        inh.set_color("a", Color::rgb(0, 0, 0));
        inh.set_color("b", Color::rgb(1, 1, 1));
        assert_eq!(inh.color_override_count(), 2);
    }

    // -- ThemeTokenColorCustomization ----------------------------------------

    #[test]
    fn token_color_customization_apply() {
        let theme = dark_plus();
        let original_count = theme.token_color_count();
        let mut cust = ThemeTokenColorCustomization::new();
        // Use a more specific scope so the customization wins by longest-prefix.
        cust.add_rule(&["comment.line"], Some(Color::rgb(255, 0, 0)), Some("bold"));
        assert_eq!(cust.len(), 1);
        assert!(!cust.is_empty());
        let custom = cust.apply(&theme);
        assert_eq!(custom.token_color_count(), original_count + 1);
        let style = custom.get_token_style(&["comment.line.double-slash"]).unwrap();
        assert_eq!(style.foreground, Some(Color::rgb(255, 0, 0)));
        assert!(style.bold);
    }

    // -- ThemeContrastCalculator ---------------------------------------------

    #[test]
    fn contrast_calculator_high_contrast_passes_aaa() {
        let hc = high_contrast();
        let violations = ThemeContrastCalculator::check(&hc, WcagLevel::AAA);
        // High contrast theme should pass AAA for editor fg/bg
        assert!(
            !violations.iter().any(|v| v.fg_key == "editor.foreground"),
            "high contrast editor fg/bg should pass AAA"
        );
    }

    #[test]
    fn contrast_calculator_pair_count() {
        assert!(ThemeContrastCalculator::pair_count() > 0);
    }

    #[test]
    fn contrast_calculator_custom_pairs() {
        let theme = dark_plus();
        let pairs = &[("editor.foreground", "editor.background")];
        let violations = ThemeContrastCalculator::check_pairs(&theme, pairs, WcagLevel::AA);
        // Dark+ editor fg/bg should pass AA
        assert!(violations.is_empty(), "Dark+ editor should pass AA: {:?}", violations);
    }

    #[test]
    fn contrast_calculator_low_contrast_detected() {
        let mut colors = HashMap::new();
        colors.insert("editor.foreground".into(), Color::rgb(128, 128, 128));
        colors.insert("editor.background".into(), Color::rgb(130, 130, 130));
        let theme = ColorTheme {
            id: "low".into(),
            label: "Low".into(),
            theme_type: ThemeType::Dark,
            colors,
            token_colors: Vec::new(),
        };
        let violations = ThemeContrastCalculator::check(&theme, WcagLevel::AA);
        assert!(violations.iter().any(|v| v.fg_key == "editor.foreground"));
    }

    #[test]
    fn themeColorFallbackChain_new() {
        let s = ThemeColorFallbackChain::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn themeColorFallbackChain_add_contains() {
        let mut s = ThemeColorFallbackChain::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn themeColorFallbackChain_add_duplicate() {
        let mut s = ThemeColorFallbackChain::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn themeColorFallbackChain_remove() {
        let mut s = ThemeColorFallbackChain::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn themeColorFallbackChain_capacity() {
        let s = ThemeColorFallbackChain::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn themeColorFallbackChain_search() {
        let mut s = ThemeColorFallbackChain::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn themeColorFallbackChain_stats() {
        let mut s = ThemeColorFallbackChain::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn themeEditorTokenColorizer_new() {
        let m = ThemeEditorTokenColorizer::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn themeEditorTokenColorizer_add_find() {
        let mut m = ThemeEditorTokenColorizer::new();
        m.add(ThemeEditorTokenColorizerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn themeEditorTokenColorizer_priority_filter() {
        let mut m = ThemeEditorTokenColorizer::new();
        m.add(ThemeEditorTokenColorizerItem::new("a", "A").with_priority(ThemeEditorTokenColorizerPriority::High));
        m.add(ThemeEditorTokenColorizerItem::new("b", "B").with_priority(ThemeEditorTokenColorizerPriority::Low));
        m.add(ThemeEditorTokenColorizerItem::new("c", "C").with_priority(ThemeEditorTokenColorizerPriority::High));
        assert_eq!(m.by_priority(ThemeEditorTokenColorizerPriority::High).len(), 2);
    }

    #[test]
    fn themeEditorTokenColorizer_remove() {
        let mut m = ThemeEditorTokenColorizer::new();
        m.add(ThemeEditorTokenColorizerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn themeEditorTokenColorizer_search() {
        let mut m = ThemeEditorTokenColorizer::new();
        m.add(ThemeEditorTokenColorizerItem::new("id1", "Hello World"));
        m.add(ThemeEditorTokenColorizerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn themeEditorTokenColorizer_total_weight() {
        let mut m = ThemeEditorTokenColorizer::new();
        m.add(ThemeEditorTokenColorizerItem::new("a", "A").with_priority(ThemeEditorTokenColorizerPriority::Critical));
        m.add(ThemeEditorTokenColorizerItem::new("b", "B").with_priority(ThemeEditorTokenColorizerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn themeEditorTokenColorizer_capacity_limit() {
        let mut m = ThemeEditorTokenColorizer::new().with_max_items(2);
        m.add(ThemeEditorTokenColorizerItem::new("1", "one"));
        m.add(ThemeEditorTokenColorizerItem::new("2", "two"));
        assert!(!m.add(ThemeEditorTokenColorizerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn themeEditorTokenColorizer_sorted_by_priority() {
        let mut m = ThemeEditorTokenColorizer::new();
        m.add(ThemeEditorTokenColorizerItem::new("lo", "Low").with_priority(ThemeEditorTokenColorizerPriority::Low));
        m.add(ThemeEditorTokenColorizerItem::new("hi", "High").with_priority(ThemeEditorTokenColorizerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn themeEditorTokenColorizer_item_metadata() {
        let mut item = ThemeEditorTokenColorizerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn themeColorFallbackChain_enabled_toggle() {
        let mut s = ThemeColorFallbackChain::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn themeEditorTokenColorizer_priority_display() {
        assert_eq!(format!("{}", ThemeEditorTokenColorizerPriority::High), "high");
        assert_eq!(format!("{}", ThemeEditorTokenColorizerPriority::Low), "low");
    }


    #[test]
    fn theme_entry_creation() {
        let e = ThemeEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn theme_entry_with_priority() {
        let e = ThemeEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn theme_entry_metadata() {
        let e = ThemeEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn theme_entry_remove_meta() {
        let mut e = ThemeEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn theme_entry_activate_deactivate() {
        let mut e = ThemeEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn theme_config_add_sorted() {
        let mut c = ThemeConfig::new(10);
        c.add(ThemeEntry::new("lo", "Lo").with_priority(1));
        c.add(ThemeEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn theme_config_capacity() {
        let mut c = ThemeConfig::new(1);
        assert!(c.add(ThemeEntry::new("a", "A")));
        assert!(!c.add(ThemeEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn theme_config_remove() {
        let mut c = ThemeConfig::new(10);
        c.add(ThemeEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn theme_config_get() {
        let mut c = ThemeConfig::new(10);
        c.add(ThemeEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn theme_config_active_entries() {
        let mut c = ThemeConfig::new(10);
        c.add(ThemeEntry::new("a", "A"));
        c.add(ThemeEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn theme_config_enable_disable() {
        let mut c = ThemeConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn theme_config_clear() {
        let mut c = ThemeConfig::new(10);
        c.add(ThemeEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn theme_config_find_by_label() {
        let mut c = ThemeConfig::new(10);
        c.add(ThemeEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn theme_config_top_n() {
        let mut c = ThemeConfig::new(10);
        c.add(ThemeEntry::new("a", "A").with_priority(1));
        c.add(ThemeEntry::new("b", "B").with_priority(2));
        c.add(ThemeEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn theme_config_deactivate_activate_all() {
        let mut c = ThemeConfig::new(10);
        c.add(ThemeEntry::new("a", "A"));
        c.add(ThemeEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn theme_config_highest_priority() {
        let mut c = ThemeConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(ThemeEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn theme_config_contains() {
        let mut c = ThemeConfig::new(10);
        c.add(ThemeEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn theme_config_labels() {
        let mut c = ThemeConfig::new(10);
        c.add(ThemeEntry::new("a", "Alpha"));
        c.add(ThemeEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn theme_config_drain_inactive() {
        let mut c = ThemeConfig::new(10);
        c.add(ThemeEntry::new("a", "A"));
        c.add(ThemeEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
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


    // xa_ extended tests for theme
    #[test]
    fn xa_theme_ring_new() {
        let rb = super::XaThemeRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_theme_ring_push_len() {
        let mut rb = super::XaThemeRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_theme_ring_wrap() {
        let mut rb = super::XaThemeRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_theme_ring_mean_empty() {
        let rb = super::XaThemeRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_theme_ring_mean_values() {
        let mut rb = super::XaThemeRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_theme_ring_min_max() {
        let mut rb = super::XaThemeRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_theme_ring_iter() {
        let mut rb = super::XaThemeRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_theme_counter_new() {
        let c = super::XaThemeCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_theme_counter_inc() {
        let mut c = super::XaThemeCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_theme_counter_inc_by() {
        let mut c = super::XaThemeCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_theme_counter_reset() {
        let mut c = super::XaThemeCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_theme_counter_clear() {
        let mut c = super::XaThemeCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_theme_counter_default() {
        let c = super::XaThemeCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 182 ----

    #[test]
    fn xc_182_pool_new_empty() {
        let pool: super::Xc182Pool<i32> = super::Xc182Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_182_pool_release_acquire() {
        let mut pool = super::Xc182Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_182_pool_acquire_empty() {
        let mut pool: super::Xc182Pool<i32> = super::Xc182Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_182_pool_full() {
        let mut pool = super::Xc182Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_182_pool_drain() {
        let mut pool = super::Xc182Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_182_pool_stats() {
        let mut pool = super::Xc182Pool::new(8);
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
    fn xc_182_pool_clear() {
        let mut pool = super::Xc182Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_182_pool_shrink() {
        let mut pool = super::Xc182Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_182_pool_default() {
        let pool: super::Xc182Pool<String> = super::Xc182Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_182_pool_extend() {
        let mut pool = super::Xc182Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_182_pool_retain() {
        let mut pool = super::Xc182Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_182_scheduler_round_robin() {
        let mut sched = super::Xc182Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_182_scheduler_empty() {
        let mut sched = super::Xc182Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_182_scheduler_reset() {
        let mut sched = super::Xc182Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_182_scheduler_add_remove() {
        let mut sched = super::Xc182Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_182_scheduler_targets() {
        let sched = super::Xc182Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_182_hash_empty() {
        assert_eq!(super::xc_182_hash(b""), 5381);
    }

    #[test]
    fn xc_182_hash_data() {
        let h = super::xc_182_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_182_hash(b"hello"), h);
    }

    #[test]
    fn xc_182_reverse_str() {
        assert_eq!(super::xc_182_reverse("abc"), "cba");
        assert_eq!(super::xc_182_reverse(""), "");
    }


    // --- xd_57 deepening tests ---

    #[test]
    fn xd_57_sm_initial_state() {
        let sm = Xd57StateMachine::new();
        assert_eq!(sm.current_state(), Xd57State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_57_sm_valid_idle_to_running() {
        let mut sm = Xd57StateMachine::new();
        assert!(sm.transition(Xd57State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd57State::Running);
    }

    #[test]
    fn xd_57_sm_valid_running_to_paused() {
        let mut sm = Xd57StateMachine::new();
        sm.transition(Xd57State::Running).unwrap();
        assert!(sm.transition(Xd57State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd57State::Paused);
    }

    #[test]
    fn xd_57_sm_valid_running_to_done() {
        let mut sm = Xd57StateMachine::new();
        sm.transition(Xd57State::Running).unwrap();
        assert!(sm.transition(Xd57State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd57State::Done);
    }

    #[test]
    fn xd_57_sm_valid_paused_to_running() {
        let mut sm = Xd57StateMachine::new();
        sm.transition(Xd57State::Running).unwrap();
        sm.transition(Xd57State::Paused).unwrap();
        assert!(sm.transition(Xd57State::Running).is_ok());
    }

    #[test]
    fn xd_57_sm_valid_done_to_idle() {
        let mut sm = Xd57StateMachine::new();
        sm.transition(Xd57State::Running).unwrap();
        sm.transition(Xd57State::Done).unwrap();
        assert!(sm.transition(Xd57State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd57State::Idle);
    }

    #[test]
    fn xd_57_sm_invalid_idle_to_done() {
        let mut sm = Xd57StateMachine::new();
        assert!(sm.transition(Xd57State::Done).is_err());
    }

    #[test]
    fn xd_57_sm_invalid_idle_to_paused() {
        let mut sm = Xd57StateMachine::new();
        assert!(sm.transition(Xd57State::Paused).is_err());
    }

    #[test]
    fn xd_57_sm_history_tracking() {
        let mut sm = Xd57StateMachine::new();
        sm.transition(Xd57State::Running).unwrap();
        sm.transition(Xd57State::Paused).unwrap();
        sm.transition(Xd57State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd57State::Idle);
        assert_eq!(sm.history()[0].to, Xd57State::Running);
        assert_eq!(sm.history()[1].from, Xd57State::Running);
        assert_eq!(sm.history()[2].to, Xd57State::Done);
    }

    #[test]
    fn xd_57_sm_serialize_deserialize() {
        let mut sm = Xd57StateMachine::new();
        sm.transition(Xd57State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd57StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd57State::Running));
    }

    #[test]
    fn xd_57_sm_deserialize_invalid() {
        assert_eq!(Xd57StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_57_sm_reset() {
        let mut sm = Xd57StateMachine::new();
        sm.transition(Xd57State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd57State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_57_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd57EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd57Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_57_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd57EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd57Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd57Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_57_bus_unsubscribe() {
        let mut bus = Xd57EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_57_event_kind_and_payload() {
        let e = Xd57Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd57Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_57_bus_clear_history() {
        let mut bus = Xd57EventBus::new();
        bus.publish(Xd57Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_57_sm_step_counter_increments() {
        let mut sm = Xd57StateMachine::new();
        sm.transition(Xd57State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd57State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #55 --

    #[test]
    fn xf55_trie_insert_search() {
        let mut t = Xf55Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf55_trie_starts_with() {
        let mut t = Xf55Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf55_trie_remove() {
        let mut t = Xf55Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf55_trie_word_count() {
        let mut t = Xf55Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf55_trie_longest_prefix() {
        let mut t = Xf55Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf55_trie_all_words() {
        let mut t = Xf55Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf55_trie_autocomplete() {
        let mut t = Xf55Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf55_trie_empty_search() {
        let t = Xf55Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf55_bloom_add_contains() {
        let mut bf = Xf55BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf55_bloom_probably_absent() {
        let bf = Xf55BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf55_bloom_false_positive_rate() {
        let mut bf = Xf55BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf55_bloom_clear() {
        let mut bf = Xf55BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf55_bloom_union() {
        let mut a = Xf55BloomFilter::xf_new(512, 2);
        let mut b = Xf55BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf55_bloom_intersection_estimate() {
        let mut a = Xf55BloomFilter::xf_new(512, 2);
        let mut b = Xf55BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf55_bloom_union_size_mismatch() {
        let a = Xf55BloomFilter::xf_new(256, 2);
        let b = Xf55BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }

}
