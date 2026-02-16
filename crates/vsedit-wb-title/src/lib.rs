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

/// Accumulated statistics for wb-title operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbTitleStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbTitleStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &WbTitleStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for WbTitleStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbTitleStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbTitleStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-title.
#[derive(Debug, Clone)]
pub struct WbTitleValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbTitleValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for WbTitleValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TitleBarContext – resolves variables from workspace state
// ---------------------------------------------------------------------------

/// Context for resolving title bar variables from workspace state.
#[derive(Debug, Clone)]
pub struct TitleBarContext {
    pub active_file: Option<String>,
    pub root_folder: Option<String>,
    pub app_name: String,
    pub is_dirty: bool,
    pub remote_host: Option<String>,
    pub workspace_name: Option<String>,
    pub git_branch: Option<String>,
}

impl TitleBarContext {
    pub fn new(app_name: &str) -> Self {
        Self {
            active_file: None,
            root_folder: None,
            app_name: app_name.to_string(),
            is_dirty: false,
            remote_host: None,
            workspace_name: None,
            git_branch: None,
        }
    }

    pub fn with_active_file(mut self, file: &str) -> Self {
        self.active_file = Some(file.to_string());
        self
    }

    pub fn with_root_folder(mut self, folder: &str) -> Self {
        self.root_folder = Some(folder.to_string());
        self
    }

    pub fn with_dirty(mut self, dirty: bool) -> Self {
        self.is_dirty = dirty;
        self
    }

    pub fn with_remote_host(mut self, host: &str) -> Self {
        self.remote_host = Some(host.to_string());
        self
    }

    pub fn with_workspace_name(mut self, name: &str) -> Self {
        self.workspace_name = Some(name.to_string());
        self
    }

    pub fn with_git_branch(mut self, branch: &str) -> Self {
        self.git_branch = Some(branch.to_string());
        self
    }

    /// Resolve a variable to its string value from this context.
    pub fn resolve_variable(&self, var: &TitleBarVariable) -> String {
        match var {
            TitleBarVariable::ActiveFile => {
                self.active_file.clone().unwrap_or_default()
            }
            TitleBarVariable::RootFolder => {
                self.root_folder.clone().unwrap_or_default()
            }
            TitleBarVariable::AppName => self.app_name.clone(),
            TitleBarVariable::Separator => " - ".to_string(),
            TitleBarVariable::Dirty => {
                if self.is_dirty {
                    "●".to_string()
                } else {
                    String::new()
                }
            }
            TitleBarVariable::RemoteHost => {
                self.remote_host.clone().unwrap_or_default()
            }
        }
    }

    /// Populate a HashMap of all variable values for template rendering.
    pub fn to_variables(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for var in TitleBarVariable::all_variants() {
            map.insert(var.key().to_string(), self.resolve_variable(var));
        }
        // Extra context-only keys not in the enum.
        if let Some(ref ws) = self.workspace_name {
            map.insert("workspaceName".to_string(), ws.clone());
        }
        if let Some(ref branch) = self.git_branch {
            map.insert("gitBranch".to_string(), branch.clone());
        }
        map
    }
}

// ---------------------------------------------------------------------------
// title_template_render / title_default_render
// ---------------------------------------------------------------------------

/// Render a title template string with variable substitution from a context.
///
/// Template uses `${varName}` syntax. Unknown variables are left as empty
/// strings. Also supports `${varName:defaultValue}` for default values.
pub fn title_template_render(template: &str, context: &TitleBarContext) -> String {
    let vars = context.to_variables();
    let mut result = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        if let Some(end) = rest.find('}') {
            let expr = &rest[..end];
            let (name, default) = match expr.find(':') {
                Some(pos) => (&expr[..pos], &expr[pos + 1..]),
                None => (expr, ""),
            };
            let value = vars.get(name).filter(|v| !v.is_empty());
            match value {
                Some(v) => result.push_str(v),
                None => result.push_str(default),
            }
            rest = &rest[end + 1..];
        } else {
            // No closing brace – emit the literal `${` and continue.
            result.push_str("${");
        }
    }
    result.push_str(rest);
    result
}

/// Render a title with the standard format: `[dirty] activeFile - rootFolder - appName`.
pub fn title_default_render(context: &TitleBarContext) -> String {
    let mut parts: Vec<String> = Vec::new();

    if context.is_dirty {
        parts.push("●".to_string());
    }

    if let Some(ref file) = context.active_file {
        parts.push(file.clone());
    }

    if let Some(ref folder) = context.root_folder {
        parts.push(folder.clone());
    }

    parts.push(context.app_name.clone());

    parts.join(" - ")
}

// ---------------------------------------------------------------------------
// TitleBarDirtyIndicator
// ---------------------------------------------------------------------------

/// Configurable dirty/modified indicator for the title bar.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleBarDirtyIndicator {
    pub prefix: String,
    pub suffix: String,
    pub indicator: String,
    pub show_when_clean: bool,
    pub clean_indicator: String,
}

impl TitleBarDirtyIndicator {
    pub fn new() -> Self {
        Self {
            prefix: String::new(),
            suffix: " ".to_string(),
            indicator: "●".to_string(),
            show_when_clean: false,
            clean_indicator: String::new(),
        }
    }

    pub fn with_indicator(mut self, s: &str) -> Self {
        self.indicator = s.to_string();
        self
    }

    pub fn with_prefix(mut self, s: &str) -> Self {
        self.prefix = s.to_string();
        self
    }

    pub fn with_suffix(mut self, s: &str) -> Self {
        self.suffix = s.to_string();
        self
    }

    pub fn with_clean_indicator(mut self, s: &str) -> Self {
        self.clean_indicator = s.to_string();
        self.show_when_clean = true;
        self
    }

    /// Render the indicator string. Returns the formatted indicator when dirty,
    /// or the clean indicator (possibly empty) when not dirty.
    pub fn render(&self, is_dirty: bool) -> String {
        if is_dirty {
            format!("{}{}{}", self.prefix, self.indicator, self.suffix)
        } else if self.show_when_clean {
            format!("{}{}{}", self.prefix, self.clean_indicator, self.suffix)
        } else {
            String::new()
        }
    }

    /// VS Code–style indicator: `● ` prefix.
    pub fn vscode_style() -> Self {
        Self {
            prefix: String::new(),
            suffix: " ".to_string(),
            indicator: "●".to_string(),
            show_when_clean: false,
            clean_indicator: String::new(),
        }
    }

    /// Emacs-style indicator: `**` prefix.
    pub fn emacs_style() -> Self {
        Self {
            prefix: String::new(),
            suffix: " ".to_string(),
            indicator: "**".to_string(),
            show_when_clean: false,
            clean_indicator: String::new(),
        }
    }
}

impl Default for TitleBarDirtyIndicator {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn wb_title_stats_new_defaults() {
        let stats = WbTitleStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_title_stats_record_success() {
        let mut stats = WbTitleStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_title_stats_record_failure() {
        let mut stats = WbTitleStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_title_stats_reset() {
        let mut stats = WbTitleStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_title_stats_merge() {
        let mut a = WbTitleStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbTitleStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn wb_title_stats_display() {
        let mut stats = WbTitleStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_title_stats_default() {
        let stats = WbTitleStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_title_validator_accepts_valid_name() {
        let v = WbTitleValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_title_validator_rejects_empty() {
        let v = WbTitleValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_title_validator_rejects_too_long() {
        let v = WbTitleValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_title_validator_forbidden_prefix() {
        let v = WbTitleValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_title_validator_allowed_chars() {
        let v = WbTitleValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_title_validator_range() {
        let v = WbTitleValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_title_sanitize_removes_control() {
        let result = WbTitleValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_title_truncate_short_string() {
        assert_eq!(WbTitleValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_title_truncate_long_string() {
        let result = WbTitleValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_title_is_ascii_printable() {
        assert!(WbTitleValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbTitleValidator::is_ascii_printable("Hello\x00World"));
    }

    // -----------------------------------------------------------------------
    // TitleBarContext tests
    // -----------------------------------------------------------------------

    #[test]
    fn context_new_defaults() {
        let ctx = TitleBarContext::new("VSEdit");
        assert_eq!(ctx.app_name, "VSEdit");
        assert!(ctx.active_file.is_none());
        assert!(ctx.root_folder.is_none());
        assert!(!ctx.is_dirty);
        assert!(ctx.remote_host.is_none());
        assert!(ctx.workspace_name.is_none());
        assert!(ctx.git_branch.is_none());
    }

    #[test]
    fn context_builder_chain() {
        let ctx = TitleBarContext::new("App")
            .with_active_file("main.rs")
            .with_root_folder("/project")
            .with_dirty(true)
            .with_remote_host("ssh://host")
            .with_workspace_name("my-ws")
            .with_git_branch("main");
        assert_eq!(ctx.active_file.as_deref(), Some("main.rs"));
        assert_eq!(ctx.root_folder.as_deref(), Some("/project"));
        assert!(ctx.is_dirty);
        assert_eq!(ctx.remote_host.as_deref(), Some("ssh://host"));
        assert_eq!(ctx.workspace_name.as_deref(), Some("my-ws"));
        assert_eq!(ctx.git_branch.as_deref(), Some("main"));
    }

    #[test]
    fn context_resolve_active_file() {
        let ctx = TitleBarContext::new("App").with_active_file("lib.rs");
        assert_eq!(
            ctx.resolve_variable(&TitleBarVariable::ActiveFile),
            "lib.rs"
        );
    }

    #[test]
    fn context_resolve_missing_active_file() {
        let ctx = TitleBarContext::new("App");
        assert_eq!(
            ctx.resolve_variable(&TitleBarVariable::ActiveFile),
            ""
        );
    }

    #[test]
    fn context_resolve_dirty() {
        let dirty = TitleBarContext::new("A").with_dirty(true);
        let clean = TitleBarContext::new("A").with_dirty(false);
        assert_eq!(dirty.resolve_variable(&TitleBarVariable::Dirty), "●");
        assert_eq!(clean.resolve_variable(&TitleBarVariable::Dirty), "");
    }

    #[test]
    fn context_resolve_separator() {
        let ctx = TitleBarContext::new("App");
        assert_eq!(
            ctx.resolve_variable(&TitleBarVariable::Separator),
            " - "
        );
    }

    #[test]
    fn context_resolve_remote_host() {
        let ctx = TitleBarContext::new("App").with_remote_host("myhost");
        assert_eq!(
            ctx.resolve_variable(&TitleBarVariable::RemoteHost),
            "myhost"
        );
    }

    #[test]
    fn context_to_variables_includes_all() {
        let ctx = TitleBarContext::new("App")
            .with_active_file("f.rs")
            .with_root_folder("/r")
            .with_dirty(true)
            .with_remote_host("h")
            .with_workspace_name("ws")
            .with_git_branch("dev");
        let vars = ctx.to_variables();
        assert_eq!(vars.get("activeFile").unwrap(), "f.rs");
        assert_eq!(vars.get("rootFolder").unwrap(), "/r");
        assert_eq!(vars.get("appName").unwrap(), "App");
        assert_eq!(vars.get("dirty").unwrap(), "●");
        assert_eq!(vars.get("remoteHost").unwrap(), "h");
        assert_eq!(vars.get("workspaceName").unwrap(), "ws");
        assert_eq!(vars.get("gitBranch").unwrap(), "dev");
    }

    #[test]
    fn context_to_variables_omits_unset_extras() {
        let ctx = TitleBarContext::new("App");
        let vars = ctx.to_variables();
        assert!(!vars.contains_key("workspaceName"));
        assert!(!vars.contains_key("gitBranch"));
    }

    // -----------------------------------------------------------------------
    // title_template_render / title_default_render tests
    // -----------------------------------------------------------------------

    #[test]
    fn template_render_basic() {
        let ctx = TitleBarContext::new("VSEdit")
            .with_active_file("main.rs")
            .with_root_folder("/proj");
        let result = title_template_render("${activeFile} - ${rootFolder} - ${appName}", &ctx);
        assert_eq!(result, "main.rs - /proj - VSEdit");
    }

    #[test]
    fn template_render_default_value() {
        let ctx = TitleBarContext::new("App");
        let result = title_template_render("${activeFile:Untitled}", &ctx);
        assert_eq!(result, "Untitled");
    }

    #[test]
    fn template_render_default_not_used_when_set() {
        let ctx = TitleBarContext::new("App").with_active_file("f.rs");
        let result = title_template_render("${activeFile:Untitled}", &ctx);
        assert_eq!(result, "f.rs");
    }

    #[test]
    fn template_render_unknown_variable() {
        let ctx = TitleBarContext::new("App");
        let result = title_template_render("${unknownVar}", &ctx);
        assert_eq!(result, "");
    }

    #[test]
    fn template_render_unknown_with_default() {
        let ctx = TitleBarContext::new("App");
        let result = title_template_render("${unknownVar:fallback}", &ctx);
        assert_eq!(result, "fallback");
    }

    #[test]
    fn template_render_literal_text_preserved() {
        let ctx = TitleBarContext::new("App");
        let result = title_template_render("hello world", &ctx);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn template_render_mixed_text_and_vars() {
        let ctx = TitleBarContext::new("App").with_active_file("x.rs");
        let result = title_template_render("[${activeFile}] ${appName}", &ctx);
        assert_eq!(result, "[x.rs] App");
    }

    #[test]
    fn template_render_unclosed_brace() {
        let ctx = TitleBarContext::new("App");
        let result = title_template_render("${appName", &ctx);
        assert_eq!(result, "${appName");
    }

    #[test]
    fn template_render_workspace_and_branch() {
        let ctx = TitleBarContext::new("App")
            .with_workspace_name("ws")
            .with_git_branch("feat");
        let result =
            title_template_render("${workspaceName} (${gitBranch})", &ctx);
        assert_eq!(result, "ws (feat)");
    }

    #[test]
    fn default_render_all_parts() {
        let ctx = TitleBarContext::new("VSEdit")
            .with_active_file("main.rs")
            .with_root_folder("/proj")
            .with_dirty(true);
        assert_eq!(title_default_render(&ctx), "● - main.rs - /proj - VSEdit");
    }

    #[test]
    fn default_render_clean() {
        let ctx = TitleBarContext::new("VSEdit")
            .with_active_file("main.rs")
            .with_root_folder("/proj");
        assert_eq!(title_default_render(&ctx), "main.rs - /proj - VSEdit");
    }

    #[test]
    fn default_render_minimal() {
        let ctx = TitleBarContext::new("App");
        assert_eq!(title_default_render(&ctx), "App");
    }

    // -----------------------------------------------------------------------
    // TitleBarDirtyIndicator tests
    // -----------------------------------------------------------------------

    #[test]
    fn dirty_indicator_defaults() {
        let d = TitleBarDirtyIndicator::new();
        assert_eq!(d.indicator, "●");
        assert_eq!(d.prefix, "");
        assert_eq!(d.suffix, " ");
        assert!(!d.show_when_clean);
        assert_eq!(d.clean_indicator, "");
    }

    #[test]
    fn dirty_indicator_render_dirty() {
        let d = TitleBarDirtyIndicator::new();
        assert_eq!(d.render(true), "● ");
    }

    #[test]
    fn dirty_indicator_render_clean_hidden() {
        let d = TitleBarDirtyIndicator::new();
        assert_eq!(d.render(false), "");
    }

    #[test]
    fn dirty_indicator_render_clean_shown() {
        let d = TitleBarDirtyIndicator::new()
            .with_clean_indicator("○");
        assert_eq!(d.render(false), "○ ");
    }

    #[test]
    fn dirty_indicator_custom() {
        let d = TitleBarDirtyIndicator::new()
            .with_indicator("*")
            .with_prefix("[")
            .with_suffix("]");
        assert_eq!(d.render(true), "[*]");
    }

    #[test]
    fn dirty_indicator_vscode_style() {
        let d = TitleBarDirtyIndicator::vscode_style();
        assert_eq!(d.render(true), "● ");
        assert_eq!(d.render(false), "");
    }

    #[test]
    fn dirty_indicator_emacs_style() {
        let d = TitleBarDirtyIndicator::emacs_style();
        assert_eq!(d.render(true), "** ");
        assert_eq!(d.render(false), "");
    }

    #[test]
    fn dirty_indicator_default_trait() {
        let d = TitleBarDirtyIndicator::default();
        assert_eq!(d, TitleBarDirtyIndicator::new());
    }
}
