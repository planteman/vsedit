//! Window title formatting.

use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TitleBarStyle {
    Native,
    Custom,
}

impl fmt::Display for TitleBarStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TitleBarStyle::Native => write!(f, "native"),
            TitleBarStyle::Custom => write!(f, "custom"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TitleBarVariable {
    ActiveFile,
    RootFolder,
    AppName,
    Separator,
    Dirty,
    RemoteHost,
}

impl TitleBarVariable {
    /// Returns the variable key used in templates and the variables map.
    pub fn key(&self) -> &'static str {
        match self {
            TitleBarVariable::ActiveFile => "activeFile",
            TitleBarVariable::RootFolder => "rootFolder",
            TitleBarVariable::AppName => "appName",
            TitleBarVariable::Separator => "separator",
            TitleBarVariable::Dirty => "dirty",
            TitleBarVariable::RemoteHost => "remoteHost",
        }
    }

    /// Returns all known title bar variable variants.
    pub fn all_variants() -> &'static [TitleBarVariable] {
        &[
            TitleBarVariable::ActiveFile,
            TitleBarVariable::RootFolder,
            TitleBarVariable::AppName,
            TitleBarVariable::Separator,
            TitleBarVariable::Dirty,
            TitleBarVariable::RemoteHost,
        ]
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "activeFile" => Some(TitleBarVariable::ActiveFile),
            "rootFolder" => Some(TitleBarVariable::RootFolder),
            "appName" => Some(TitleBarVariable::AppName),
            "separator" => Some(TitleBarVariable::Separator),
            "dirty" => Some(TitleBarVariable::Dirty),
            "remoteHost" => Some(TitleBarVariable::RemoteHost),
            _ => None,
        }
    }
}

impl fmt::Display for TitleBarVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${{{}}}", self.key())
    }
}

#[derive(Debug, Clone)]
pub struct TitleBarTemplate {
    pub parts: Vec<TitleBarVariable>,
}

impl TitleBarTemplate {
    /// Parses a template string containing `${var}` patterns into parts.
    ///
    /// Recognized variables: `activeFile`, `rootFolder`, `appName`,
    /// `separator`, `dirty`, `remoteHost`. Unknown variables are ignored.
    pub fn parse(template_str: &str) -> Self {
        let mut parts = Vec::new();
        let mut rest = template_str;
        while let Some(start) = rest.find("${") {
            rest = &rest[start + 2..];
            if let Some(end) = rest.find('}') {
                let var_name = &rest[..end];
                if let Some(var) = TitleBarVariable::from_key(var_name) {
                    parts.push(var);
                }
                rest = &rest[end + 1..];
            }
        }
        TitleBarTemplate { parts }
    }
}

/// Errors that can occur during title bar operations.
#[derive(Debug, Clone, PartialEq)]
pub enum TitleError {
    /// A variable name is empty or contains invalid characters.
    InvalidVariableName(String),
    /// A variable value exceeds the maximum allowed length.
    ValueTooLong { name: String, len: usize, max: usize },
    /// The template string is empty.
    EmptyTemplate,
}

impl fmt::Display for TitleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TitleError::InvalidVariableName(name) => {
                write!(f, "invalid variable name: '{name}'")
            }
            TitleError::ValueTooLong { name, len, max } => {
                write!(f, "value for '{name}' is {len} chars, max {max}")
            }
            TitleError::EmptyTemplate => write!(f, "template string is empty"),
        }
    }
}

impl std::error::Error for TitleError {}

/// Maximum length allowed for a single variable value.
const MAX_VARIABLE_VALUE_LEN: usize = 512;

impl TitleBarTemplate {
    /// Returns the number of variable slots in this template.
    pub fn variable_count(&self) -> usize {
        self.parts.len()
    }

    /// Returns true if the template contains the given variable.
    pub fn contains(&self, var: &TitleBarVariable) -> bool {
        self.parts.contains(var)
    }

    /// Parse a template string, returning an error if the input is empty.
    pub fn try_parse(template_str: &str) -> Result<Self, TitleError> {
        if template_str.is_empty() {
            return Err(TitleError::EmptyTemplate);
        }
        Ok(Self::parse(template_str))
    }
}

impl PartialEq for TitleBarTemplate {
    fn eq(&self, other: &Self) -> bool {
        self.parts == other.parts
    }
}

impl fmt::Display for TitleBarTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{part}")?;
        }
        Ok(())
    }
}

/// Builder for constructing a [`TitleBarService`] with validation.
#[derive(Debug)]
pub struct TitleBarServiceBuilder {
    style: TitleBarStyle,
    template: Option<TitleBarTemplate>,
    variables: HashMap<String, String>,
}

impl TitleBarServiceBuilder {
    pub fn new(style: TitleBarStyle) -> Self {
        Self {
            style,
            template: None,
            variables: HashMap::new(),
        }
    }

    pub fn template(mut self, template: TitleBarTemplate) -> Self {
        self.template = Some(template);
        self
    }

    pub fn variable(mut self, name: &str, value: &str) -> Result<Self, TitleError> {
        validate_variable_name(name)?;
        validate_variable_value(name, value)?;
        self.variables.insert(name.to_string(), value.to_string());
        Ok(self)
    }

    pub fn build(self) -> TitleBarService {
        TitleBarService {
            template: self.template.unwrap_or(TitleBarTemplate { parts: Vec::new() }),
            style: self.style,
            variables: self.variables,
        }
    }
}

fn validate_variable_name(name: &str) -> Result<(), TitleError> {
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(TitleError::InvalidVariableName(name.to_string()));
    }
    Ok(())
}

fn validate_variable_value(name: &str, value: &str) -> Result<(), TitleError> {
    if value.len() > MAX_VARIABLE_VALUE_LEN {
        return Err(TitleError::ValueTooLong {
            name: name.to_string(),
            len: value.len(),
            max: MAX_VARIABLE_VALUE_LEN,
        });
    }
    Ok(())
}

/// Service for title bar management.
pub struct TitleBarService {
    template: TitleBarTemplate,
    style: TitleBarStyle,
    variables: HashMap<String, String>,
}

impl TitleBarService {
    pub fn new(style: TitleBarStyle) -> Self {
        Self {
            template: TitleBarTemplate { parts: Vec::new() },
            style,
            variables: HashMap::new(),
        }
    }

    pub fn set_template(&mut self, template: TitleBarTemplate) {
        self.template = template;
    }

    pub fn set_style(&mut self, style: TitleBarStyle) {
        self.style = style;
    }

    pub fn set_variable(&mut self, name: &str, value: &str) {
        self.variables.insert(name.to_string(), value.to_string());
    }

    pub fn get_variable(&self, name: &str) -> Option<&str> {
        self.variables.get(name).map(|s| s.as_str())
    }

    pub fn clear_variable(&mut self, name: &str) {
        self.variables.remove(name);
    }

    pub fn clear_all_variables(&mut self) {
        self.variables.clear();
    }

    /// Convenience method to set the active file variable.
    pub fn set_active_file(&mut self, filename: &str) {
        self.set_variable("activeFile", filename);
    }

    /// Convenience method to set the dirty indicator.
    pub fn set_dirty(&mut self, dirty: bool) {
        if dirty {
            self.set_variable("dirty", "●");
        } else {
            self.clear_variable("dirty");
        }
    }

    /// Renders a default title string from the given components.
    pub fn render_default_title(app_name: &str, file: Option<&str>, dirty: bool) -> String {
        let dirty_indicator = if dirty { "● " } else { "" };
        match file {
            Some(f) => format!("{dirty_indicator}{f} - {app_name}"),
            None => format!("{dirty_indicator}{app_name}"),
        }
    }

    pub fn render(&self) -> String {
        self.template
            .parts
            .iter()
            .map(|part| {
                self.variables
                    .get(part.key())
                    .cloned()
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn get_style(&self) -> &TitleBarStyle {
        &self.style
    }

    /// Sets a variable with validation, returning an error on invalid input.
    pub fn set_variable_checked(&mut self, name: &str, value: &str) -> Result<(), TitleError> {
        validate_variable_name(name)?;
        validate_variable_value(name, value)?;
        self.variables.insert(name.to_string(), value.to_string());
        Ok(())
    }

    /// Returns the number of variables currently set.
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// Returns the current template.
    pub fn template(&self) -> &TitleBarTemplate {
        &self.template
    }

    /// Convenience method to set the root folder variable.
    pub fn set_root_folder(&mut self, folder: &str) {
        self.set_variable("rootFolder", folder);
    }

    /// Convenience method to set the remote host variable.
    pub fn set_remote_host(&mut self, host: &str) {
        self.set_variable("remoteHost", host);
    }

    /// Renders the title and truncates it to `max_len` characters, appending
    /// an ellipsis if truncation occurs.
    pub fn render_truncated(&self, max_len: usize) -> String {
        let rendered = self.render();
        if rendered.len() <= max_len {
            rendered
        } else if max_len <= 3 {
            rendered[..max_len].to_string()
        } else {
            format!("{}...", &rendered[..max_len - 3])
        }
    }

    /// Returns a list of variable keys that the current template references
    /// but have no value set.
    pub fn missing_variables(&self) -> Vec<&'static str> {
        self.template
            .parts
            .iter()
            .filter(|var| !self.variables.contains_key(var.key()))
            .map(|var| var.key())
            .collect()
    }
}

impl fmt::Debug for TitleBarService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TitleBarService")
            .field("style", &self.style)
            .field("template_parts", &self.template.parts.len())
            .field("variables", &self.variables.len())
            .finish()
    }
}

impl fmt::Display for TitleBarService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.style, self.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_template() {
        let mut svc = TitleBarService::new(TitleBarStyle::Custom);
        svc.set_template(TitleBarTemplate {
            parts: vec![
                TitleBarVariable::ActiveFile,
                TitleBarVariable::Separator,
                TitleBarVariable::AppName,
            ],
        });
        svc.set_variable("activeFile", "main.rs");
        svc.set_variable("separator", " - ");
        svc.set_variable("appName", "VSEdit");
        assert_eq!(svc.render(), "main.rs - VSEdit");
    }

    #[test]
    fn missing_variables_render_empty() {
        let mut svc = TitleBarService::new(TitleBarStyle::Native);
        svc.set_template(TitleBarTemplate {
            parts: vec![TitleBarVariable::Dirty, TitleBarVariable::ActiveFile],
        });
        svc.set_variable("activeFile", "lib.rs");
        assert_eq!(svc.render(), "lib.rs");
    }

    #[test]
    fn style_access() {
        let svc = TitleBarService::new(TitleBarStyle::Native);
        assert_eq!(*svc.get_style(), TitleBarStyle::Native);
    }

    #[test]
    fn parse_template_string() {
        let tpl = TitleBarTemplate::parse("${activeFile} - ${appName}");
        assert_eq!(tpl.parts.len(), 2);
        assert_eq!(tpl.parts[0], TitleBarVariable::ActiveFile);
        assert_eq!(tpl.parts[1], TitleBarVariable::AppName);
    }

    #[test]
    fn parse_template_ignores_unknown_vars() {
        let tpl = TitleBarTemplate::parse("${unknown}${appName}");
        assert_eq!(tpl.parts.len(), 1);
        assert_eq!(tpl.parts[0], TitleBarVariable::AppName);
    }

    #[test]
    fn set_and_get_style() {
        let mut svc = TitleBarService::new(TitleBarStyle::Native);
        assert_eq!(*svc.get_style(), TitleBarStyle::Native);
        svc.set_style(TitleBarStyle::Custom);
        assert_eq!(*svc.get_style(), TitleBarStyle::Custom);
    }

    #[test]
    fn clear_variable() {
        let mut svc = TitleBarService::new(TitleBarStyle::Native);
        svc.set_variable("appName", "VSEdit");
        assert_eq!(svc.get_variable("appName"), Some("VSEdit"));
        svc.clear_variable("appName");
        assert_eq!(svc.get_variable("appName"), None);
    }

    #[test]
    fn clear_all_variables() {
        let mut svc = TitleBarService::new(TitleBarStyle::Native);
        svc.set_variable("appName", "VSEdit");
        svc.set_variable("activeFile", "main.rs");
        svc.clear_all_variables();
        assert_eq!(svc.get_variable("appName"), None);
        assert_eq!(svc.get_variable("activeFile"), None);
    }

    #[test]
    fn convenience_set_active_file() {
        let mut svc = TitleBarService::new(TitleBarStyle::Custom);
        svc.set_active_file("test.rs");
        assert_eq!(svc.get_variable("activeFile"), Some("test.rs"));
    }

    #[test]
    fn convenience_set_dirty() {
        let mut svc = TitleBarService::new(TitleBarStyle::Custom);
        svc.set_dirty(true);
        assert_eq!(svc.get_variable("dirty"), Some("●"));
        svc.set_dirty(false);
        assert_eq!(svc.get_variable("dirty"), None);
    }

    #[test]
    fn render_default_title_with_file() {
        let title = TitleBarService::render_default_title("VSEdit", Some("main.rs"), false);
        assert_eq!(title, "main.rs - VSEdit");
    }

    #[test]
    fn render_default_title_dirty_no_file() {
        let title = TitleBarService::render_default_title("VSEdit", None, true);
        assert_eq!(title, "● VSEdit");
    }

    #[test]
    fn display_title_bar_variable() {
        assert_eq!(TitleBarVariable::ActiveFile.to_string(), "${activeFile}");
        assert_eq!(TitleBarVariable::Dirty.to_string(), "${dirty}");
    }

    #[test]
    fn display_title_bar_style() {
        assert_eq!(TitleBarStyle::Native.to_string(), "native");
        assert_eq!(TitleBarStyle::Custom.to_string(), "custom");
    }

    #[test]
    fn builder_creates_service() {
        let svc = TitleBarServiceBuilder::new(TitleBarStyle::Custom)
            .template(TitleBarTemplate::parse("${activeFile}${separator}${appName}"))
            .variable("activeFile", "main.rs")
            .unwrap()
            .variable("separator", " - ")
            .unwrap()
            .variable("appName", "VSEdit")
            .unwrap()
            .build();
        assert_eq!(svc.render(), "main.rs - VSEdit");
        assert_eq!(*svc.get_style(), TitleBarStyle::Custom);
    }

    #[test]
    fn builder_rejects_invalid_variable_name() {
        let result = TitleBarServiceBuilder::new(TitleBarStyle::Native)
            .variable("", "value");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TitleError::InvalidVariableName(String::new())
        );
    }

    #[test]
    fn builder_rejects_too_long_value() {
        let long_value = "x".repeat(513);
        let result = TitleBarServiceBuilder::new(TitleBarStyle::Native)
            .variable("appName", &long_value);
        assert!(result.is_err());
        match result.unwrap_err() {
            TitleError::ValueTooLong { len, max, .. } => {
                assert_eq!(len, 513);
                assert_eq!(max, 512);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn set_variable_checked_validates() {
        let mut svc = TitleBarService::new(TitleBarStyle::Native);
        assert!(svc.set_variable_checked("app-name", "v").is_err());
        assert!(svc.set_variable_checked("appName", "VSEdit").is_ok());
        assert_eq!(svc.get_variable("appName"), Some("VSEdit"));
    }

    #[test]
    fn try_parse_empty_template() {
        let result = TitleBarTemplate::try_parse("");
        assert_eq!(result, Err(TitleError::EmptyTemplate));
    }

    #[test]
    fn try_parse_valid_template() {
        let tpl = TitleBarTemplate::try_parse("${dirty}${activeFile}").unwrap();
        assert_eq!(tpl.variable_count(), 2);
        assert!(tpl.contains(&TitleBarVariable::Dirty));
        assert!(tpl.contains(&TitleBarVariable::ActiveFile));
    }

    #[test]
    fn render_truncated_short() {
        let mut svc = TitleBarService::new(TitleBarStyle::Custom);
        svc.set_template(TitleBarTemplate::parse("${appName}"));
        svc.set_variable("appName", "VSEdit");
        assert_eq!(svc.render_truncated(100), "VSEdit");
    }

    #[test]
    fn render_truncated_long() {
        let mut svc = TitleBarService::new(TitleBarStyle::Custom);
        svc.set_template(TitleBarTemplate::parse("${appName}"));
        svc.set_variable("appName", "A Very Long Application Name");
        assert_eq!(svc.render_truncated(10), "A Very ...");
    }

    #[test]
    fn missing_variables_reports_unset() {
        let mut svc = TitleBarService::new(TitleBarStyle::Custom);
        svc.set_template(TitleBarTemplate::parse("${activeFile} - ${appName}"));
        svc.set_variable("appName", "VSEdit");
        let missing = svc.missing_variables();
        assert_eq!(missing, vec!["activeFile"]);
    }

    #[test]
    fn variable_count_tracks_set_vars() {
        let mut svc = TitleBarService::new(TitleBarStyle::Custom);
        assert_eq!(svc.variable_count(), 0);
        svc.set_variable("appName", "VSEdit");
        assert_eq!(svc.variable_count(), 1);
        svc.set_variable("activeFile", "lib.rs");
        assert_eq!(svc.variable_count(), 2);
        svc.clear_variable("appName");
        assert_eq!(svc.variable_count(), 1);
    }

    #[test]
    fn all_variants_returns_six() {
        assert_eq!(TitleBarVariable::all_variants().len(), 6);
    }

    #[test]
    fn title_error_display() {
        let e = TitleError::EmptyTemplate;
        assert_eq!(e.to_string(), "template string is empty");
        let e2 = TitleError::InvalidVariableName("bad!".into());
        assert!(e2.to_string().contains("bad!"));
    }

    #[test]
    fn service_debug_and_display() {
        let mut svc = TitleBarService::new(TitleBarStyle::Custom);
        svc.set_template(TitleBarTemplate::parse("${appName}"));
        svc.set_variable("appName", "VSEdit");
        let dbg = format!("{svc:?}");
        assert!(dbg.contains("TitleBarService"));
        let disp = format!("{svc}");
        assert!(disp.contains("custom"));
        assert!(disp.contains("VSEdit"));
    }

    #[test]
    fn template_display() {
        let tpl = TitleBarTemplate::parse("${activeFile}${separator}${appName}");
        let s = tpl.to_string();
        assert!(s.contains("${activeFile}"));
        assert!(s.contains("${appName}"));
    }

    #[test]
    fn template_equality() {
        let a = TitleBarTemplate::parse("${activeFile}${appName}");
        let b = TitleBarTemplate::parse("${activeFile}${appName}");
        assert_eq!(a, b);
    }

    #[test]
    fn convenience_set_root_folder_and_remote_host() {
        let mut svc = TitleBarService::new(TitleBarStyle::Native);
        svc.set_root_folder("/home/user/project");
        svc.set_remote_host("devbox.internal");
        assert_eq!(svc.get_variable("rootFolder"), Some("/home/user/project"));
        assert_eq!(svc.get_variable("remoteHost"), Some("devbox.internal"));
    }
}
