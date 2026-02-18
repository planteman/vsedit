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

// ---------------------------------------------------------------------------
// TitleHistory – tracks previous window titles
// ---------------------------------------------------------------------------

/// Tracks a bounded history of rendered window titles.
#[derive(Debug, Clone)]
pub struct TitleHistory {
    entries: Vec<TitleHistoryEntry>,
    capacity: usize,
}

/// A single entry in the title history.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleHistoryEntry {
    pub title: String,
    pub timestamp_ms: u64,
}

impl TitleHistory {
    /// Create a new history with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a new title onto the history. If the capacity is exceeded the
    /// oldest entry is removed.
    pub fn push(&mut self, title: String, timestamp_ms: u64) {
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(TitleHistoryEntry {
            title,
            timestamp_ms,
        });
    }

    /// Returns the most recent title, if any.
    pub fn latest(&self) -> Option<&TitleHistoryEntry> {
        self.entries.last()
    }

    /// Returns all entries from oldest to newest.
    pub fn entries(&self) -> &[TitleHistoryEntry] {
        &self.entries
    }

    /// Returns the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all history entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns `true` if the given title differs from the most recent entry.
    pub fn is_changed(&self, title: &str) -> bool {
        match self.latest() {
            Some(entry) => entry.title != title,
            None => true,
        }
    }
}

// ---------------------------------------------------------------------------
// Platform – platform-specific title formatting
// ---------------------------------------------------------------------------

/// Target platform for title bar formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    Linux,
}

impl Platform {
    /// Detects the current compilation target platform.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Windows => write!(f, "windows"),
            Platform::MacOs => write!(f, "macos"),
            Platform::Linux => write!(f, "linux"),
        }
    }
}

/// Formats a window title according to platform conventions.
///
/// - **macOS**: shows the filename only (no path prefix) when a file is active,
///   since macOS uses a proxy icon for the full path.
/// - **Windows**: uses backslash separators in paths.
/// - **Linux**: uses the full forward-slash path.
pub fn format_title_for_platform(
    platform: Platform,
    context: &TitleBarContext,
) -> String {
    let file_part = match (&context.active_file, platform) {
        (Some(path), Platform::MacOs) => {
            // macOS convention: display only the filename.
            path.rsplit('/').next().unwrap_or(path).to_string()
        }
        (Some(path), Platform::Windows) => {
            path.replace('/', "\\")
        }
        (Some(path), Platform::Linux) => path.clone(),
        (None, _) => String::new(),
    };

    let dirty = if context.is_dirty { "● " } else { "" };

    if file_part.is_empty() {
        format!("{dirty}{}", context.app_name)
    } else {
        format!("{dirty}{file_part} — {}", context.app_name)
    }
}

// ---------------------------------------------------------------------------
// TitleBarTheme – title bar color / theme customization tracking
// ---------------------------------------------------------------------------

/// RGBA colour represented as four `u8` components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parse a `#RRGGBB` or `#RRGGBBAA` hex colour string.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self { r, g, b, a: 255 })
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self { r, g, b, a })
            }
            _ => None,
        }
    }

    /// Render as a `#RRGGBB` hex string (alpha is omitted when fully opaque).
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

impl fmt::Display for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Title bar visual theme configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleBarTheme {
    pub background: Rgba,
    pub foreground: Rgba,
    pub active_background: Rgba,
    pub active_foreground: Rgba,
    pub border: Option<Rgba>,
}

impl TitleBarTheme {
    /// A sensible dark theme default.
    pub fn dark() -> Self {
        Self {
            background: Rgba::new(30, 30, 30, 255),
            foreground: Rgba::new(204, 204, 204, 255),
            active_background: Rgba::new(30, 30, 30, 255),
            active_foreground: Rgba::new(255, 255, 255, 255),
            border: None,
        }
    }

    /// A sensible light theme default.
    pub fn light() -> Self {
        Self {
            background: Rgba::new(221, 221, 221, 255),
            foreground: Rgba::new(51, 51, 51, 255),
            active_background: Rgba::new(221, 221, 221, 255),
            active_foreground: Rgba::new(0, 0, 0, 255),
            border: None,
        }
    }

    /// Returns `true` if the background colour is considered "dark"
    /// (perceived luminance < 128).
    pub fn is_dark(&self) -> bool {
        perceived_luminance(self.background) < 128
    }

    /// Set a border colour.
    pub fn with_border(mut self, color: Rgba) -> Self {
        self.border = Some(color);
        self
    }
}

/// Compute perceived luminance using the standard rec. 601 formula,
/// returning a value in 0..=255.
fn perceived_luminance(c: Rgba) -> u8 {
    let lum = 0.299 * c.r as f64 + 0.587 * c.g as f64 + 0.114 * c.b as f64;
    lum.round() as u8
}

// ---------------------------------------------------------------------------
// TitleBarVariableResolver
// ---------------------------------------------------------------------------

/// Resolves `${varName}` placeholders inside a template string.
pub struct TitleBarVariableResolver {
    variables: HashMap<String, String>,
}

impl TitleBarVariableResolver {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    pub fn set(&mut self, var_name: &str, value: &str) {
        self.variables.insert(var_name.to_string(), value.to_string());
    }

    /// Replace every `${key}` in `template` with the corresponding value.
    pub fn resolve(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (key, value) in &self.variables {
            let pattern = format!("${{{}}}", key);
            result = result.replace(&pattern, value);
        }
        result
    }

    /// Populate resolver variables from a `TitleBarContext`.
    pub fn set_from_context(&mut self, ctx: &TitleBarContext) {
        if let Some(ref v) = ctx.workspace_name {
            self.set("workspaceName", v);
        }
        if let Some(ref v) = ctx.active_file {
            self.set("activeFile", v);
        }
        if let Some(ref v) = ctx.root_folder {
            self.set("rootFolder", v);
        }
        if let Some(ref v) = ctx.remote_host {
            self.set("remoteHost", v);
        }
        if let Some(ref v) = ctx.git_branch {
            self.set("gitBranch", v);
        }
        self.set("appName", &ctx.app_name);
    }

    /// Return a sorted list of `(name, value)` pairs.
    pub fn list_variables(&self) -> Vec<(String, String)> {
        let mut vars: Vec<(String, String)> = self
            .variables
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        vars.sort_by(|a, b| a.0.cmp(&b.0));
        vars
    }
}

// ---------------------------------------------------------------------------
// TitleBarPathTruncator
// ---------------------------------------------------------------------------

/// Shortens long file-system paths for display in the title bar.
pub struct TitleBarPathTruncator {
    max_length: usize,
}

impl TitleBarPathTruncator {
    pub fn new(max_length: usize) -> Self {
        Self { max_length }
    }

    pub fn set_max_length(&mut self, max_length: usize) {
        self.max_length = max_length;
    }

    /// If `path` exceeds `max_length`, keep only the last segment prefixed
    /// with `…/`.
    pub fn truncate(&self, path: &str) -> String {
        if path.len() <= self.max_length {
            return path.to_string();
        }
        match path.rsplit_once('/') {
            Some((_, last)) => format!("…/{}", last),
            None => path.to_string(),
        }
    }

    /// Keep the first and last path segments with `…` in the middle.
    pub fn truncate_middle(&self, path: &str) -> String {
        if path.len() <= self.max_length {
            return path.to_string();
        }
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.len() <= 2 {
            return self.truncate(path);
        }
        let prefix = if path.starts_with('/') { "/" } else { "" };
        format!("{}{}/…/{}", prefix, segments[0], segments[segments.len() - 1])
    }
}

// ---------------------------------------------------------------------------
// TitleBarDirtyNotifier
// ---------------------------------------------------------------------------

/// Tracks which files have unsaved changes and provides a dirty indicator.
pub struct TitleBarDirtyNotifier {
    dirty_files: Vec<String>,
    indicator: String,
}

impl TitleBarDirtyNotifier {
    pub fn new() -> Self {
        Self {
            dirty_files: Vec::new(),
            indicator: "●".to_string(),
        }
    }

    pub fn add_dirty(&mut self, file: &str) {
        if !self.dirty_files.contains(&file.to_string()) {
            self.dirty_files.push(file.to_string());
        }
    }

    pub fn remove_dirty(&mut self, file: &str) {
        self.dirty_files.retain(|f| f != file);
    }

    pub fn is_any_dirty(&self) -> bool {
        !self.dirty_files.is_empty()
    }

    pub fn dirty_count(&self) -> usize {
        self.dirty_files.len()
    }

    /// Returns the indicator string if any file is dirty, otherwise empty.
    pub fn indicator_text(&self) -> String {
        if self.is_any_dirty() {
            self.indicator.clone()
        } else {
            String::new()
        }
    }

    pub fn set_indicator(&mut self, s: &str) {
        self.indicator = s.to_string();
    }

    pub fn clear(&mut self) {
        self.dirty_files.clear();
    }
}

impl fmt::Display for TitleBarDirtyNotifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_any_dirty() {
            write!(f, "{} ({} dirty)", self.indicator, self.dirty_count())
        } else {
            write!(f, "clean")
        }
    }
}

// ---------------------------------------------------------------------------
// TitleBarUpdateDebouncer
// ---------------------------------------------------------------------------

/// Prevents excessively frequent title-bar repaints by debouncing updates.
pub struct TitleBarUpdateDebouncer {
    last_update_ms: u64,
    debounce_ms: u64,
    pending_title: Option<String>,
}

impl TitleBarUpdateDebouncer {
    pub fn new(debounce_ms: u64) -> Self {
        Self {
            last_update_ms: 0,
            debounce_ms,
            pending_title: None,
        }
    }

    /// Queue a title update to be emitted after the debounce window elapses.
    pub fn request_update(&mut self, title: &str, _now_ms: u64) {
        self.pending_title = Some(title.to_string());
    }

    /// Returns `true` when the debounce interval has passed and there is a
    /// pending title.
    pub fn should_update(&self, now_ms: u64) -> bool {
        self.pending_title.is_some() && now_ms >= self.last_update_ms + self.debounce_ms
    }

    /// Consume the pending title if the debounce window has elapsed.
    pub fn flush(&mut self, now_ms: u64) -> Option<String> {
        if self.should_update(now_ms) {
            self.last_update_ms = now_ms;
            self.pending_title.take()
        } else {
            None
        }
    }

    /// Bypass debouncing: immediately return the given title and clear any
    /// pending update.
    pub fn force_update(&mut self, title: &str) -> String {
        self.pending_title = None;
        title.to_string()
    }
}


// ---------------------------------------------------------------------------
// TitleBreadcrumbMode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TitleBreadcrumbMode {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl TitleBreadcrumbMode {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for TitleBreadcrumbMode {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for TitleBreadcrumbMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "TitleBreadcrumbMode({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// TitleMenuIntegration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TitleMenuIntegration {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl TitleMenuIntegration {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for TitleMenuIntegration {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for TitleMenuIntegration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "TitleMenuIntegration({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// TitleBreadcrumbModeSnapshot — point-in-time snapshot of TitleBreadcrumbMode state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TitleBreadcrumbModeSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl TitleBreadcrumbModeSnapshot {
    pub fn capture(source: &TitleBreadcrumbMode, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for TitleBreadcrumbModeSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// TitleMenuIntegrationStats — aggregate statistics for TitleMenuIntegration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct TitleMenuIntegrationStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl TitleMenuIntegrationStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for TitleMenuIntegrationStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// TitleBreadcrumbModeConfig — configuration for TitleBreadcrumbMode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TitleBreadcrumbModeConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl TitleBreadcrumbModeConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for TitleBreadcrumbModeConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for TitleBreadcrumbModeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// TitleTemplateEngine
// ---------------------------------------------------------------------------

/// Format window title from a template with `${var}` substitution.
#[derive(Debug, Clone)]
pub struct TitleTemplateEngine {
    variables: HashMap<String, String>,
}

impl TitleTemplateEngine {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    pub fn register_variable(&mut self, name: &str, value: &str) {
        self.variables.insert(name.to_string(), value.to_string());
    }

    pub fn expand_template(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (key, val) in &self.variables {
            result = result.replace(&format!("${{{key}}}"), val);
        }
        result
    }

    pub fn unresolved_variables(&self, template: &str) -> Vec<String> {
        let mut vars = Vec::new();
        let mut rest = template;
        while let Some(start) = rest.find("${") {
            if let Some(end) = rest[start + 2..].find('}') {
                let name = &rest[start + 2..start + 2 + end];
                if !self.variables.contains_key(name) && !vars.contains(&name.to_string()) {
                    vars.push(name.to_string());
                }
                rest = &rest[start + 2 + end + 1..];
            } else {
                break;
            }
        }
        vars
    }

    pub fn validate_template(template: &str) -> bool {
        let mut depth = 0i32;
        let bytes = template.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'$' && bytes[i + 1] == b'{' {
                depth += 1;
                i += 2;
            } else if bytes[i] == b'}' && depth > 0 {
                depth -= 1;
                i += 1;
            } else {
                i += 1;
            }
        }
        depth == 0
    }
}

// ---------------------------------------------------------------------------
// TitleSegment
// ---------------------------------------------------------------------------

/// A segment of a window title with optional separator.
#[derive(Debug, Clone)]
pub struct TitleSegment {
    pub text: String,
    pub separator: String,
    pub visible: bool,
}

impl TitleSegment {
    pub fn new(text: &str, separator: &str) -> Self {
        Self {
            text: text.to_string(),
            separator: separator.to_string(),
            visible: true,
        }
    }

    pub fn join_visible(segments: &[TitleSegment]) -> String {
        let visible: Vec<&str> = segments
            .iter()
            .filter(|s| s.visible && !s.text.is_empty())
            .map(|s| s.text.as_str())
            .collect();
        if visible.is_empty() {
            return String::new();
        }
        let sep = segments
            .first()
            .map(|s| s.separator.as_str())
            .unwrap_or(" - ");
        visible.join(sep)
    }

    pub fn toggle_segment(&mut self) {
        self.visible = !self.visible;
    }

    pub fn visible_count(segments: &[TitleSegment]) -> usize {
        segments.iter().filter(|s| s.visible).count()
    }
}

// ---------------------------------------------------------------------------
// TitleDirtyIndicator
// ---------------------------------------------------------------------------

/// Track modified (dirty) file state for window title.
#[derive(Debug, Clone)]
pub struct TitleDirtyIndicator {
    dirty_files: Vec<String>,
    prefix: String,
    suffix: String,
}

impl TitleDirtyIndicator {
    pub fn new() -> Self {
        Self {
            dirty_files: Vec::new(),
            prefix: "● ".to_string(),
            suffix: String::new(),
        }
    }

    pub fn with_prefix(mut self, prefix: &str) -> Self {
        self.prefix = prefix.to_string();
        self
    }

    pub fn with_suffix(mut self, suffix: &str) -> Self {
        self.suffix = suffix.to_string();
        self
    }

    pub fn set_dirty(&mut self, file: &str) {
        if !self.dirty_files.contains(&file.to_string()) {
            self.dirty_files.push(file.to_string());
        }
    }

    pub fn clear_dirty(&mut self, file: &str) {
        self.dirty_files.retain(|f| f != file);
    }

    pub fn is_dirty(&self, file: &str) -> bool {
        self.dirty_files.iter().any(|f| f == file)
    }

    pub fn dirty_count(&self) -> usize {
        self.dirty_files.len()
    }

    pub fn format_indicator(&self, title: &str) -> String {
        if self.dirty_files.is_empty() {
            title.to_string()
        } else {
            format!("{}{}{}", self.prefix, title, self.suffix)
        }
    }
}


/// Configuration manager for wb_title functionality.
pub struct WbTitleConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl WbTitleConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &WbTitleConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for wb_title operations.
pub struct WbTitleRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl WbTitleRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for wb_title.
pub struct WbTitleValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl WbTitleValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &WbTitleValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Title bar rendering and variable resolution — extended utilities (zt)
// ---------------------------------------------------------------------------

/// Metric accumulator for title_bar operations.
#[derive(Debug, Clone)]
pub struct ZtMetrics {
    samples: Vec<f64>,
    label: String,
}

impl ZtMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for title_bar.
#[derive(Debug, Clone)]
pub struct ZtRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl ZtRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for title_bar lookups.
#[derive(Debug, Clone)]
pub struct ZtLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZtLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for wb_title
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbTitleRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbTitleRingBuf {
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
pub struct XaWbTitleCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbTitleCounter {
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

impl Default for XaWbTitleCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 230
// ---------------------------------------------------------------------------

/// Generic object pool `Xc230Pool<T>`.
pub struct Xc230Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc230Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc230PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc230Pool<T> {
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
    pub fn stats(&self) -> Xc230PoolStats {
        Xc230PoolStats {
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

impl<T> Default for Xc230Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc230Scheduler`.
pub struct Xc230Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc230Scheduler {
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

impl Default for Xc230Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_230 hash for the given byte slice.
pub fn xc_230_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_230 convention.
pub fn xc_230_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_32 deepening: state machine + event bus ---

/// States for the Xd32 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd32State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd32State {
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
pub struct Xd32Transition {
    pub from: Xd32State,
    pub to: Xd32State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd32StateMachine {
    current: Xd32State,
    history: Vec<Xd32Transition>,
    step_counter: usize,
}

impl Xd32StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd32State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd32State {
        self.current
    }

    pub fn history(&self) -> &[Xd32Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd32State) -> Result<Xd32State, String> {
        let allowed = match (self.current, target) {
            (Xd32State::Idle, Xd32State::Running) => true,
            (Xd32State::Running, Xd32State::Paused) => true,
            (Xd32State::Running, Xd32State::Done) => true,
            (Xd32State::Paused, Xd32State::Running) => true,
            (Xd32State::Paused, Xd32State::Done) => true,
            (Xd32State::Done, Xd32State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_32: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd32Transition {
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
            "Xd32SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd32State> {
        let prefix = "Xd32SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd32State::Idle),
            "Running" => Some(Xd32State::Running),
            "Paused" => Some(Xd32State::Paused),
            "Done" => Some(Xd32State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd32State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd32 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd32Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd32Event {
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

type Xd32HandlerFn = Box<dyn Fn(&Xd32Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd32EventBus {
    handlers: Vec<(usize, Option<String>, Xd32HandlerFn)>,
    next_id: usize,
    published: Vec<Xd32Event>,
}

impl Xd32EventBus {
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
        F: Fn(&Xd32Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd32Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd32Event) {
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

    pub fn published_events(&self) -> &[Xd32Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #30
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf30Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf30TrieNode {
    children: std::collections::HashMap<char, Xf30TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf30Trie {
    root: Xf30TrieNode,
    count: usize,
}

impl Xf30Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf30TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf30TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf30TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf30BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf30BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 229).
pub struct Xh229SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh229SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 271 as u64,
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

/// A compact bit set supporting boolean operations (variant 229).
pub struct Xh229BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh229BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 229).
pub struct Xi229Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi229Deque<T> {
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
pub struct Xi229Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi229Interval {
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

/// A simple interval tree (variant 229).
pub struct Xi229IntervalTree {
    xi_intervals: Vec<Xi229Interval>,
}

impl Xi229IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi229Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi229Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi229Interval) -> Vec<&Xi229Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi229Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi229Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi229Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi229Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi229Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi229Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 229) ---

/// Disjoint set / union-find for crate 229.
pub struct Xj229UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj229UnionFind {
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

const XJ229_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 229.
pub struct Xj229BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj229BTreeNode<K, V>>>,
    len: usize,
}

struct Xj229BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj229BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj229BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ229_BTREE_ORDER - 1
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
        let mid = XJ229_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj229BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj229BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj229BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj229BTreeNode::xj_new_leaf();
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


// --- xk_229 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk229SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk229SegmentTree {
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
pub struct Xk229DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk229DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_229).
#[derive(Debug, Clone)]
pub struct Xl229Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl229Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_229).
#[derive(Debug, Clone)]
pub struct Xl229SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl229SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm229MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm229MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm229Tokenizer {
    text: String,
}

impl Xm229Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 229.
pub struct Xn229Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn229Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 229 -----

#[derive(Debug, Clone)]
struct Xn229AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn229AvlNode<K, V>>>,
    right: Option<Box<Xn229AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 229.
#[derive(Debug, Clone)]
pub struct Xn229AVL<K, V> {
    root: Option<Box<Xn229AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn229AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn229AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn229AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn229AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn229AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn229AvlNode<K, V>>) -> Box<Xn229AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn229AvlNode<K, V>>) -> Box<Xn229AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn229AvlNode<K, V>>) -> Box<Xn229AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn229AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn229AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn229AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn229AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn229AvlNode<K, V>>) -> &Xn229AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn229AvlNode<K, V>>) -> (Box<Xn229AvlNode<K, V>>, Option<Box<Xn229AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn229AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn229AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn229AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn229AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn229AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn229AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn229AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo229RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo229Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo229RBNode<K, V> {
    key: K,
    value: V,
    color: Xo229Color,
    left: Option<Box<Xo229RBNode<K, V>>>,
    right: Option<Box<Xo229RBNode<K, V>>>,
}

/// A red-black tree map for crate 229.
#[derive(Debug, Clone)]
pub struct Xo229RedBlack<K, V> {
    root: Option<Box<Xo229RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo229RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo229Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo229RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo229RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo229RBNode {
                    key, value, color: Xo229Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo229RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo229Color::Red)
    }

    fn xo_balance(mut h: Box<Xo229RBNode<K, V>>) -> Box<Xo229RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo229Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo229RBNode<K, V>>) -> Box<Xo229RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo229Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo229RBNode<K, V>>) -> Box<Xo229RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo229Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo229RBNode<K, V>>) {
        h.color = Xo229Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo229Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo229Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo229Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo229RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo229RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo229RBNode<K, V>) -> (K, V, Option<Box<Xo229RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo229RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo229Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo229RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo229ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 229.
#[derive(Debug, Clone)]
pub struct Xo229ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo229ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo229#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo229#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 229).
#[derive(Debug)]
pub struct Xp229SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp229Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp229Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp229Node<K, V>>>,
    xp_right: Option<Box<Xp229Node<K, V>>>,
}

impl<K: Ord, V> Xp229Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp229SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp229SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp229Node<K, V>>>, key: &K) -> Option<Box<Xp229Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp229Node<K, V>>) -> Box<Xp229Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp229Node<K, V>>) -> Box<Xp229Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp229Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp229Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp229Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq229Treap ---------------

use std::cmp::Ordering as Xq229Ord;

struct Xq229TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq229TreapNode<K, V>>>,
    right: Option<Box<Xq229TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq229Treap<K, V> {
    root: Option<Box<Xq229TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq229TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_229_size<K, V>(node: &Option<Box<Xq229TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_229_update_size<K, V>(node: &mut Xq229TreapNode<K, V>) {
    node.size = 1 + xq_229_size(&node.left) + xq_229_size(&node.right);
}

fn xq_229_rotate_right<K, V>(mut node: Box<Xq229TreapNode<K, V>>) -> Box<Xq229TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_229_update_size(&mut node);
    left.right = Some(node);
    xq_229_update_size(&mut left);
    left
}

fn xq_229_rotate_left<K, V>(mut node: Box<Xq229TreapNode<K, V>>) -> Box<Xq229TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_229_update_size(&mut node);
    right.left = Some(node);
    xq_229_update_size(&mut right);
    right
}

fn xq_229_insert_node<K: Ord, V>(
    node: Option<Box<Xq229TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq229TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq229TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq229Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq229Ord::Less => {
                let (new_left, old) = xq_229_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_229_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_229_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq229Ord::Greater => {
                let (new_right, old) = xq_229_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_229_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_229_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_229_remove_node<K: Ord, V>(
    node: Option<Box<Xq229TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq229TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq229Ord::Less => {
                let (new_left, old) = xq_229_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_229_update_size(&mut n);
                (Some(n), old)
            }
            Xq229Ord::Greater => {
                let (new_right, old) = xq_229_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_229_update_size(&mut n);
                (Some(n), old)
            }
            Xq229Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_229_rotate_right(n);
                    let (new_right, old) = xq_229_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_229_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_229_rotate_left(n);
                    let (new_left, old) = xq_229_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_229_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_229_find_min<K, V>(node: &Option<Box<Xq229TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_229_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_229_find_max<K, V>(node: &Option<Box<Xq229TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_229_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_229_rank<K: Ord, V>(node: &Option<Box<Xq229TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq229Ord::Less => xq_229_rank(&n.left, key),
            Xq229Ord::Equal => xq_229_size(&n.left),
            Xq229Ord::Greater => 1 + xq_229_size(&n.left) + xq_229_rank(&n.right, key),
        },
    }
}

fn xq_229_kth<K, V>(node: &Option<Box<Xq229TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_229_size(&n.left);
        if k < left_size {
            xq_229_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_229_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_229_in_order<K: Clone, V>(node: &Option<Box<Xq229TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_229_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_229_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq229Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 229 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_229_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq229Ord::Equal => return Some(&n.value),
                Xq229Ord::Less => cur = &n.left,
                Xq229Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_229_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_229_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_229_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_229_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_229_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_229_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_229_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq229VEBTree ---------------

pub struct Xq229VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq229VEBTree>>,
    clusters: Vec<Option<Box<Xq229VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq229VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq229VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq229VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
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
    fn wbtitle_validator_accepts_and_rejects() {
        let mut v = WbTitleValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn wbtitle_validator_warnings() {
        let mut v = WbTitleValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn wbtitle_validator_clear_and_merge() {
        let mut v = WbTitleValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = WbTitleValidationCollector::new();
        a.add_error("a_err");
        let mut b = WbTitleValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
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

    // -----------------------------------------------------------------------
    // TitleHistory tests
    // -----------------------------------------------------------------------

    #[test]
    fn history_push_and_latest() {
        let mut h = TitleHistory::new(5);
        assert!(h.is_empty());
        h.push("title1".into(), 100);
        h.push("title2".into(), 200);
        assert_eq!(h.len(), 2);
        assert_eq!(h.latest().unwrap().title, "title2");
        assert_eq!(h.latest().unwrap().timestamp_ms, 200);
    }

    #[test]
    fn history_evicts_oldest_on_overflow() {
        let mut h = TitleHistory::new(2);
        h.push("a".into(), 1);
        h.push("b".into(), 2);
        h.push("c".into(), 3);
        assert_eq!(h.len(), 2);
        assert_eq!(h.entries()[0].title, "b");
        assert_eq!(h.entries()[1].title, "c");
    }

    #[test]
    fn history_is_changed() {
        let mut h = TitleHistory::new(5);
        assert!(h.is_changed("anything"));
        h.push("first".into(), 1);
        assert!(!h.is_changed("first"));
        assert!(h.is_changed("second"));
    }

    #[test]
    fn history_clear() {
        let mut h = TitleHistory::new(5);
        h.push("a".into(), 1);
        h.push("b".into(), 2);
        h.clear();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    // -----------------------------------------------------------------------
    // Platform formatting tests
    // -----------------------------------------------------------------------

    #[test]
    fn platform_display() {
        assert_eq!(Platform::Windows.to_string(), "windows");
        assert_eq!(Platform::MacOs.to_string(), "macos");
        assert_eq!(Platform::Linux.to_string(), "linux");
    }

    #[test]
    fn format_title_macos_shows_filename_only() {
        let ctx = TitleBarContext::new("VSEdit")
            .with_active_file("src/main.rs");
        let title = format_title_for_platform(Platform::MacOs, &ctx);
        assert_eq!(title, "main.rs — VSEdit");
    }

    #[test]
    fn format_title_windows_uses_backslash() {
        let ctx = TitleBarContext::new("VSEdit")
            .with_active_file("src/lib/mod.rs");
        let title = format_title_for_platform(Platform::Windows, &ctx);
        assert_eq!(title, "src\\lib\\mod.rs — VSEdit");
    }

    #[test]
    fn format_title_linux_full_path() {
        let ctx = TitleBarContext::new("VSEdit")
            .with_active_file("/home/user/project/main.rs");
        let title = format_title_for_platform(Platform::Linux, &ctx);
        assert_eq!(title, "/home/user/project/main.rs — VSEdit");
    }

    #[test]
    fn format_title_no_file() {
        let ctx = TitleBarContext::new("VSEdit");
        let title = format_title_for_platform(Platform::Linux, &ctx);
        assert_eq!(title, "VSEdit");
    }

    #[test]
    fn format_title_dirty_indicator() {
        let ctx = TitleBarContext::new("VSEdit")
            .with_active_file("f.rs")
            .with_dirty(true);
        let title = format_title_for_platform(Platform::Linux, &ctx);
        assert!(title.starts_with("● "));
    }

    // -----------------------------------------------------------------------
    // Rgba / TitleBarTheme tests
    // -----------------------------------------------------------------------

    #[test]
    fn rgba_from_hex_6() {
        let c = Rgba::from_hex("#1e1e1e").unwrap();
        assert_eq!(c, Rgba::new(30, 30, 30, 255));
    }

    #[test]
    fn rgba_from_hex_8() {
        let c = Rgba::from_hex("#1e1e1e80").unwrap();
        assert_eq!(c, Rgba::new(30, 30, 30, 128));
    }

    #[test]
    fn rgba_from_hex_invalid() {
        assert!(Rgba::from_hex("nope").is_none());
        assert!(Rgba::from_hex("#zzzzzz").is_none());
    }

    #[test]
    fn rgba_to_hex_opaque() {
        let c = Rgba::new(255, 0, 128, 255);
        assert_eq!(c.to_hex(), "#ff0080");
    }

    #[test]
    fn rgba_to_hex_transparent() {
        let c = Rgba::new(255, 0, 128, 100);
        assert_eq!(c.to_hex(), "#ff008064");
    }

    #[test]
    fn rgba_display() {
        let c = Rgba::new(0, 0, 0, 255);
        assert_eq!(format!("{c}"), "#000000");
    }

    #[test]
    fn theme_dark_is_dark() {
        assert!(TitleBarTheme::dark().is_dark());
    }

    #[test]
    fn theme_light_is_not_dark() {
        assert!(!TitleBarTheme::light().is_dark());
    }

    #[test]
    fn theme_with_border() {
        let t = TitleBarTheme::dark().with_border(Rgba::new(100, 100, 100, 255));
        assert_eq!(t.border, Some(Rgba::new(100, 100, 100, 255)));
    }

    // -- TitleBarVariableResolver tests --

    #[test]
    fn resolver_basic_substitution() {
        let mut r = TitleBarVariableResolver::new();
        r.set("name", "hello");
        assert_eq!(r.resolve("${name} world"), "hello world");
    }

    #[test]
    fn resolver_no_match_unchanged() {
        let r = TitleBarVariableResolver::new();
        assert_eq!(r.resolve("${missing}"), "${missing}");
    }

    #[test]
    fn resolver_set_from_context() {
        let mut ctx = TitleBarContext::new("vsedit");
        ctx.active_file = Some("main.rs".into());
        ctx.workspace_name = Some("ws".into());
        let mut r = TitleBarVariableResolver::new();
        r.set_from_context(&ctx);
        assert_eq!(
            r.resolve("${appName}: ${activeFile}"),
            "vsedit: main.rs"
        );
    }

    #[test]
    fn resolver_list_variables_sorted() {
        let mut r = TitleBarVariableResolver::new();
        r.set("beta", "2");
        r.set("alpha", "1");
        let vars = r.list_variables();
        assert_eq!(vars[0].0, "alpha");
        assert_eq!(vars[1].0, "beta");
    }

    // -- TitleBarPathTruncator tests --

    #[test]
    fn truncator_short_path_unchanged() {
        let t = TitleBarPathTruncator::new(50);
        assert_eq!(t.truncate("/home/user"), "/home/user");
    }

    #[test]
    fn truncator_long_path_truncated() {
        let t = TitleBarPathTruncator::new(10);
        assert_eq!(
            t.truncate("/very/long/path/to/file.rs"),
            "…/file.rs"
        );
    }

    #[test]
    fn truncator_middle_keeps_first_and_last() {
        let t = TitleBarPathTruncator::new(8);
        assert_eq!(
            t.truncate_middle("/a/b/c/d/e"),
            "/a/…/e"
        );
    }

    #[test]
    fn truncator_set_max_length() {
        let mut t = TitleBarPathTruncator::new(100);
        assert_eq!(t.truncate("/a/b/c/d/e"), "/a/b/c/d/e");
        t.set_max_length(5);
        assert_eq!(t.truncate("/a/b/c/d/e"), "…/e");
    }

    // -- TitleBarDirtyNotifier tests --

    #[test]
    fn dirty_notifier_initially_clean() {
        let n = TitleBarDirtyNotifier::new();
        assert!(!n.is_any_dirty());
        assert_eq!(n.dirty_count(), 0);
        assert_eq!(n.indicator_text(), "");
    }

    #[test]
    fn dirty_notifier_add_remove() {
        let mut n = TitleBarDirtyNotifier::new();
        n.add_dirty("a.rs");
        n.add_dirty("b.rs");
        assert_eq!(n.dirty_count(), 2);
        n.remove_dirty("a.rs");
        assert_eq!(n.dirty_count(), 1);
        assert!(n.is_any_dirty());
        assert_eq!(n.indicator_text(), "●");
    }

    #[test]
    fn dirty_notifier_no_duplicates() {
        let mut n = TitleBarDirtyNotifier::new();
        n.add_dirty("a.rs");
        n.add_dirty("a.rs");
        assert_eq!(n.dirty_count(), 1);
    }

    #[test]
    fn dirty_notifier_display() {
        let mut n = TitleBarDirtyNotifier::new();
        assert_eq!(format!("{}", n), "clean");
        n.add_dirty("x.rs");
        n.set_indicator("*");
        assert_eq!(format!("{}", n), "* (1 dirty)");
    }

    // -- TitleBarUpdateDebouncer tests --

    #[test]
    fn debouncer_does_not_fire_early() {
        let mut d = TitleBarUpdateDebouncer::new(100);
        d.request_update("title", 0);
        assert!(!d.should_update(50));
        assert!(d.flush(50).is_none());
    }

    #[test]
    fn debouncer_fires_after_interval() {
        let mut d = TitleBarUpdateDebouncer::new(100);
        d.request_update("new title", 0);
        assert!(d.should_update(100));
        assert_eq!(d.flush(100), Some("new title".into()));
        // consumed
        assert!(d.flush(200).is_none());
    }

    #[test]
    fn debouncer_force_update_bypasses() {
        let mut d = TitleBarUpdateDebouncer::new(1000);
        d.request_update("queued", 0);
        let result = d.force_update("forced");
        assert_eq!(result, "forced");
        // pending cleared
        assert!(!d.should_update(2000));
    }

    #[test] fn titleBreadcrumbMode_new() { let s = TitleBreadcrumbMode::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn titleBreadcrumbMode_add() { let mut s = TitleBreadcrumbMode::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn titleBreadcrumbMode_remove() { let mut s = TitleBreadcrumbMode::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn titleBreadcrumbMode_config() { let mut s = TitleBreadcrumbMode::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn titleBreadcrumbMode_nav() { let mut s = TitleBreadcrumbMode::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn titleBreadcrumbMode_filter() { let mut s = TitleBreadcrumbMode::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn titleBreadcrumbMode_display() { assert!(format!("{}", TitleBreadcrumbMode::new()).contains("TitleBreadcrumbMode")); }
    #[test] fn titleMenuIntegration_new() { let s = TitleMenuIntegration::new(); assert!(s.is_empty()); }
    #[test] fn titleMenuIntegration_add() { let mut s = TitleMenuIntegration::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn titleMenuIntegration_active() { let mut s = TitleMenuIntegration::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn titleMenuIntegration_error() { let mut s = TitleMenuIntegration::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn titleMenuIntegration_rm_group() { let mut s = TitleMenuIntegration::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn titleMenuIntegration_display() { assert!(format!("{}", TitleMenuIntegration::new()).contains("TitleMenuIntegration")); }


    #[test] fn titleBreadcrumbMode_snap_capture() {
        let s = TitleBreadcrumbMode::new();
        let snap = TitleBreadcrumbModeSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn titleBreadcrumbMode_snap_stale() {
        let s = TitleBreadcrumbMode::new();
        let snap = TitleBreadcrumbModeSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn titleBreadcrumbMode_snap_diff() {
        let s = TitleBreadcrumbMode::new();
        let s1v = TitleBreadcrumbModeSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn titleBreadcrumbMode_snap_display() {
        let s = TitleBreadcrumbMode::new();
        let snap = TitleBreadcrumbModeSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn titleMenuIntegration_stats_record() {
        let mut st = TitleMenuIntegrationStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn titleMenuIntegration_stats_hit_ratio() {
        let mut st = TitleMenuIntegrationStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn titleMenuIntegration_stats_merge() {
        let mut a = TitleMenuIntegrationStats::new();
        a.total_adds = 5;
        let mut b = TitleMenuIntegrationStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn titleMenuIntegration_stats_display() {
        let st = TitleMenuIntegrationStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn titleBreadcrumbMode_config_default() {
        let c = TitleBreadcrumbModeConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn titleBreadcrumbMode_config_builder() {
        let c = TitleBreadcrumbModeConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn titleBreadcrumbMode_config_labels() {
        let mut c = TitleBreadcrumbModeConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn titleBreadcrumbMode_config_cleanup_threshold() {
        let c = TitleBreadcrumbModeConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn titleBreadcrumbMode_config_display() {
        assert!(format!("{}", TitleBreadcrumbModeConfig::new()).contains("Config"));
    }
    #[test] fn titleMenuIntegration_stats_peaks() {
        let mut st = TitleMenuIntegrationStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- TitleTemplateEngine -----------------------------------------------

    #[test]
    fn template_engine_expand() {
        let mut eng = TitleTemplateEngine::new();
        eng.register_variable("activeFile", "main.rs");
        eng.register_variable("rootFolder", "myproject");
        assert_eq!(
            eng.expand_template("${activeFile} - ${rootFolder}"),
            "main.rs - myproject"
        );
    }

    #[test]
    fn template_engine_no_vars() {
        let eng = TitleTemplateEngine::new();
        assert_eq!(eng.expand_template("Static Title"), "Static Title");
    }

    #[test]
    fn template_engine_unresolved() {
        let eng = TitleTemplateEngine::new();
        let unresolved = eng.unresolved_variables("${activeFile} - ${dirty}");
        assert_eq!(unresolved, vec!["activeFile", "dirty"]);
    }

    #[test]
    fn template_engine_validate_ok() {
        assert!(TitleTemplateEngine::validate_template("${file} - ${folder}"));
    }

    #[test]
    fn template_engine_validate_mismatch() {
        assert!(!TitleTemplateEngine::validate_template("${file - ${folder}"));
    }

    // -- TitleSegment ------------------------------------------------------

    #[test]
    fn segment_join_visible() {
        let segments = vec![
            TitleSegment::new("main.rs", " - "),
            TitleSegment::new("myproject", " - "),
            TitleSegment::new("VSEdit", " - "),
        ];
        assert_eq!(TitleSegment::join_visible(&segments), "main.rs - myproject - VSEdit");
    }

    #[test]
    fn segment_toggle() {
        let mut seg = TitleSegment::new("text", " | ");
        assert!(seg.visible);
        seg.toggle_segment();
        assert!(!seg.visible);
    }

    #[test]
    fn segment_visible_count() {
        let mut segments = vec![
            TitleSegment::new("a", " - "),
            TitleSegment::new("b", " - "),
        ];
        assert_eq!(TitleSegment::visible_count(&segments), 2);
        segments[0].visible = false;
        assert_eq!(TitleSegment::visible_count(&segments), 1);
    }

    #[test]
    fn segment_join_hidden() {
        let segments = vec![
            TitleSegment { text: "a".into(), separator: " - ".into(), visible: false },
            TitleSegment::new("b", " - "),
        ];
        assert_eq!(TitleSegment::join_visible(&segments), "b");
    }

    // -- TitleDirtyIndicator -----------------------------------------------

    #[test]
    fn dirty_indicator_basic() {
        let mut ind = TitleDirtyIndicator::new();
        ind.set_dirty("main.rs");
        assert!(ind.is_dirty("main.rs"));
        assert_eq!(ind.dirty_count(), 1);
        assert_eq!(ind.format_indicator("main.rs"), "● main.rs");
    }

    #[test]
    fn dirty_indicator_clear() {
        let mut ind = TitleDirtyIndicator::new();
        ind.set_dirty("f.rs");
        ind.clear_dirty("f.rs");
        assert!(!ind.is_dirty("f.rs"));
        assert_eq!(ind.format_indicator("f.rs"), "f.rs");
    }

    #[test]
    fn dirty_indicator_custom_prefix() {
        let mut ind = TitleDirtyIndicator::new().with_prefix("[M] ").with_suffix(" *");
        ind.set_dirty("x.rs");
        assert_eq!(ind.format_indicator("x.rs"), "[M] x.rs *");
    }


    #[test]
    fn wb_title_config_new() {
        let cfg = WbTitleConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn wb_title_config_set_get() {
        let mut cfg = WbTitleConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn wb_title_config_remove() {
        let mut cfg = WbTitleConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn wb_title_config_keys_sorted() {
        let mut cfg = WbTitleConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn wb_title_config_bump_version() {
        let mut cfg = WbTitleConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn wb_title_config_clear() {
        let mut cfg = WbTitleConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn wb_title_config_merge() {
        let mut cfg1 = WbTitleConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = WbTitleConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn wb_title_config_disable() {
        let mut cfg = WbTitleConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn wb_title_rate_tracker_empty() {
        let rt = WbTitleRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn wb_title_rate_tracker_record() {
        let mut rt = WbTitleRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn wb_title_rate_tracker_prune() {
        let mut rt = WbTitleRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn wb_title_validator_valid() {
        let v = WbTitleValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn wb_title_validator_errors() {
        let mut v = WbTitleValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn wb_title_validator_clear() {
        let mut v = WbTitleValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn wb_title_validator_merge() {
        let mut v1 = WbTitleValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = WbTitleValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn wb_title_rate_tracker_clear() {
        let mut rt = WbTitleRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zt_metrics_empty() {
        let m = ZtMetrics::new("title_bar");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zt_metrics_record_and_mean() {
        let mut m = ZtMetrics::new("title_bar");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zt_metrics_min_max() {
        let mut m = ZtMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zt_metrics_variance_and_std() {
        let mut m = ZtMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn zt_metrics_percentile() {
        let mut m = ZtMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn zt_metrics_merge() {
        let mut a = ZtMetrics::new("a");
        a.record(1.0);
        let mut b = ZtMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn zt_metrics_reset() {
        let mut m = ZtMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn zt_rate_window_empty() {
        let rw = ZtRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn zt_rate_window_tick_and_rate() {
        let mut rw = ZtRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn zt_lru_cache_basic() {
        let mut c = ZtLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn zt_lru_cache_contains_and_keys() {
        let mut c = ZtLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn zt_lru_cache_remove() {
        let mut c = ZtLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn zt_metrics_sum() {
        let mut m = ZtMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zt_metrics_label() {
        let m = ZtMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn zt_lru_cache_clear() {
        let mut c = ZtLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for wb_title
    #[test]
    fn xa_wb_title_ring_new() {
        let rb = super::XaWbTitleRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_title_ring_push_len() {
        let mut rb = super::XaWbTitleRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_title_ring_wrap() {
        let mut rb = super::XaWbTitleRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_title_ring_mean_empty() {
        let rb = super::XaWbTitleRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_title_ring_mean_values() {
        let mut rb = super::XaWbTitleRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_title_ring_min_max() {
        let mut rb = super::XaWbTitleRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_title_ring_iter() {
        let mut rb = super::XaWbTitleRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_title_counter_new() {
        let c = super::XaWbTitleCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_title_counter_inc() {
        let mut c = super::XaWbTitleCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_title_counter_inc_by() {
        let mut c = super::XaWbTitleCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_title_counter_reset() {
        let mut c = super::XaWbTitleCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_title_counter_clear() {
        let mut c = super::XaWbTitleCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_title_counter_default() {
        let c = super::XaWbTitleCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 230 ----

    #[test]
    fn xc_230_pool_new_empty() {
        let pool: super::Xc230Pool<i32> = super::Xc230Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_230_pool_release_acquire() {
        let mut pool = super::Xc230Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_230_pool_acquire_empty() {
        let mut pool: super::Xc230Pool<i32> = super::Xc230Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_230_pool_full() {
        let mut pool = super::Xc230Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_230_pool_drain() {
        let mut pool = super::Xc230Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_230_pool_stats() {
        let mut pool = super::Xc230Pool::new(8);
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
    fn xc_230_pool_clear() {
        let mut pool = super::Xc230Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_230_pool_shrink() {
        let mut pool = super::Xc230Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_230_pool_default() {
        let pool: super::Xc230Pool<String> = super::Xc230Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_230_pool_extend() {
        let mut pool = super::Xc230Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_230_pool_retain() {
        let mut pool = super::Xc230Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_230_scheduler_round_robin() {
        let mut sched = super::Xc230Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_230_scheduler_empty() {
        let mut sched = super::Xc230Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_230_scheduler_reset() {
        let mut sched = super::Xc230Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_230_scheduler_add_remove() {
        let mut sched = super::Xc230Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_230_scheduler_targets() {
        let sched = super::Xc230Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_230_hash_empty() {
        assert_eq!(super::xc_230_hash(b""), 5381);
    }

    #[test]
    fn xc_230_hash_data() {
        let h = super::xc_230_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_230_hash(b"hello"), h);
    }

    #[test]
    fn xc_230_reverse_str() {
        assert_eq!(super::xc_230_reverse("abc"), "cba");
        assert_eq!(super::xc_230_reverse(""), "");
    }


    // --- xd_32 deepening tests ---

    #[test]
    fn xd_32_sm_initial_state() {
        let sm = Xd32StateMachine::new();
        assert_eq!(sm.current_state(), Xd32State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_32_sm_valid_idle_to_running() {
        let mut sm = Xd32StateMachine::new();
        assert!(sm.transition(Xd32State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd32State::Running);
    }

    #[test]
    fn xd_32_sm_valid_running_to_paused() {
        let mut sm = Xd32StateMachine::new();
        sm.transition(Xd32State::Running).unwrap();
        assert!(sm.transition(Xd32State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd32State::Paused);
    }

    #[test]
    fn xd_32_sm_valid_running_to_done() {
        let mut sm = Xd32StateMachine::new();
        sm.transition(Xd32State::Running).unwrap();
        assert!(sm.transition(Xd32State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd32State::Done);
    }

    #[test]
    fn xd_32_sm_valid_paused_to_running() {
        let mut sm = Xd32StateMachine::new();
        sm.transition(Xd32State::Running).unwrap();
        sm.transition(Xd32State::Paused).unwrap();
        assert!(sm.transition(Xd32State::Running).is_ok());
    }

    #[test]
    fn xd_32_sm_valid_done_to_idle() {
        let mut sm = Xd32StateMachine::new();
        sm.transition(Xd32State::Running).unwrap();
        sm.transition(Xd32State::Done).unwrap();
        assert!(sm.transition(Xd32State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd32State::Idle);
    }

    #[test]
    fn xd_32_sm_invalid_idle_to_done() {
        let mut sm = Xd32StateMachine::new();
        assert!(sm.transition(Xd32State::Done).is_err());
    }

    #[test]
    fn xd_32_sm_invalid_idle_to_paused() {
        let mut sm = Xd32StateMachine::new();
        assert!(sm.transition(Xd32State::Paused).is_err());
    }

    #[test]
    fn xd_32_sm_history_tracking() {
        let mut sm = Xd32StateMachine::new();
        sm.transition(Xd32State::Running).unwrap();
        sm.transition(Xd32State::Paused).unwrap();
        sm.transition(Xd32State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd32State::Idle);
        assert_eq!(sm.history()[0].to, Xd32State::Running);
        assert_eq!(sm.history()[1].from, Xd32State::Running);
        assert_eq!(sm.history()[2].to, Xd32State::Done);
    }

    #[test]
    fn xd_32_sm_serialize_deserialize() {
        let mut sm = Xd32StateMachine::new();
        sm.transition(Xd32State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd32StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd32State::Running));
    }

    #[test]
    fn xd_32_sm_deserialize_invalid() {
        assert_eq!(Xd32StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_32_sm_reset() {
        let mut sm = Xd32StateMachine::new();
        sm.transition(Xd32State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd32State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_32_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd32EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd32Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_32_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd32EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd32Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd32Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_32_bus_unsubscribe() {
        let mut bus = Xd32EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_32_event_kind_and_payload() {
        let e = Xd32Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd32Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_32_bus_clear_history() {
        let mut bus = Xd32EventBus::new();
        bus.publish(Xd32Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_32_sm_step_counter_increments() {
        let mut sm = Xd32StateMachine::new();
        sm.transition(Xd32State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd32State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #30 --

    #[test]
    fn xf30_trie_insert_search() {
        let mut t = Xf30Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf30_trie_starts_with() {
        let mut t = Xf30Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf30_trie_remove() {
        let mut t = Xf30Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf30_trie_word_count() {
        let mut t = Xf30Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf30_trie_longest_prefix() {
        let mut t = Xf30Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf30_trie_all_words() {
        let mut t = Xf30Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf30_trie_autocomplete() {
        let mut t = Xf30Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf30_trie_empty_search() {
        let t = Xf30Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf30_bloom_add_contains() {
        let mut bf = Xf30BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf30_bloom_probably_absent() {
        let bf = Xf30BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf30_bloom_false_positive_rate() {
        let mut bf = Xf30BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf30_bloom_clear() {
        let mut bf = Xf30BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf30_bloom_union() {
        let mut a = Xf30BloomFilter::xf_new(512, 2);
        let mut b = Xf30BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf30_bloom_intersection_estimate() {
        let mut a = Xf30BloomFilter::xf_new(512, 2);
        let mut b = Xf30BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf30_bloom_union_size_mismatch() {
        let a = Xf30BloomFilter::xf_new(256, 2);
        let b = Xf30BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh229_skip_insert_contains() {
        let mut sl = super::Xh229SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh229_skip_remove() {
        let mut sl = super::Xh229SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh229_skip_len() {
        let mut sl = super::Xh229SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh229_skip_range_query() {
        let mut sl = super::Xh229SkipList::xh_new(4);
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
    fn xh229_skip_floor_ceiling() {
        let mut sl = super::Xh229SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh229_skip_rank() {
        let mut sl = super::Xh229SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh229_skip_empty() {
        let sl = super::Xh229SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh229_skip_duplicates() {
        let mut sl = super::Xh229SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh229_bitset_set_test() {
        let mut bs = super::Xh229BitSet::xh_new(256);
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
    fn xh229_bitset_clear_count() {
        let mut bs = super::Xh229BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh229_bitset_and_or_xor() {
        let mut a = super::Xh229BitSet::xh_new(128);
        let mut b = super::Xh229BitSet::xh_new(128);
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
    fn xh229_bitset_iter_ones() {
        let mut bs = super::Xh229BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh229_bitset_first_last() {
        let mut bs = super::Xh229BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh229_bitset_empty() {
        let bs = super::Xh229BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi229_deque_push_pop_back() {
        let mut dq = super::Xi229Deque::xi_new(4);
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
    fn xi229_deque_push_pop_front() {
        let mut dq = super::Xi229Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi229_deque_mixed_ops() {
        let mut dq = super::Xi229Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi229_deque_get_and_split() {
        let mut dq = super::Xi229Deque::xi_new(8);
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
    fn xi229_deque_rotate_left() {
        let mut dq = super::Xi229Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi229_deque_rotate_right() {
        let mut dq = super::Xi229Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi229_deque_grow() {
        let mut dq = super::Xi229Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi229_deque_empty() {
        let dq = super::Xi229Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi229_interval_tree_insert_query() {
        let mut tree = super::Xi229IntervalTree::xi_new();
        tree.xi_insert(super::Xi229Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi229Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi229Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi229_interval_tree_overlap() {
        let mut tree = super::Xi229IntervalTree::xi_new();
        tree.xi_insert(super::Xi229Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi229Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi229Interval::xi_new(12, 20));
        let q = super::Xi229Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi229_interval_tree_remove() {
        let mut tree = super::Xi229IntervalTree::xi_new();
        tree.xi_insert(super::Xi229Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi229Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi229_interval_tree_gaps() {
        let mut tree = super::Xi229IntervalTree::xi_new();
        tree.xi_insert(super::Xi229Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi229Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi229Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi229Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi229Interval::xi_new(8, 10));
    }

    #[test]
    fn xi229_interval_tree_merge() {
        let mut tree = super::Xi229IntervalTree::xi_new();
        tree.xi_insert(super::Xi229Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi229Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi229Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi229Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi229Interval::xi_new(10, 15));
    }

    #[test]
    fn xi229_interval_tree_all() {
        let mut tree = super::Xi229IntervalTree::xi_new();
        tree.xi_insert(super::Xi229Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi229Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi229_interval_tree_empty() {
        let tree = super::Xi229IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi229_interval_tree_contains_point() {
        let iv = super::Xi229Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 229) ---

    #[test]
    fn xj_229_uf_make_and_find() {
        let mut uf = super::Xj229UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_229_uf_union_connected() {
        let mut uf = super::Xj229UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_229_uf_component_count() {
        let mut uf = super::Xj229UnionFind::xj_new();
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
    fn xj_229_uf_component_size() {
        let mut uf = super::Xj229UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_229_uf_largest_component() {
        let mut uf = super::Xj229UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_229_uf_many_elements() {
        let mut uf = super::Xj229UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_229_uf_separate_components() {
        let mut uf = super::Xj229UnionFind::xj_new();
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
    fn xj_229_uf_path_compression() {
        let mut uf = super::Xj229UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_229_bt_insert_get() {
        let mut bt = super::Xj229BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_229_bt_contains_len() {
        let mut bt = super::Xj229BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_229_bt_replace() {
        let mut bt = super::Xj229BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_229_bt_remove() {
        let mut bt = super::Xj229BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_229_bt_keys_values() {
        let mut bt = super::Xj229BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_229_bt_range() {
        let mut bt = super::Xj229BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_229_bt_min_max() {
        let mut bt = super::Xj229BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_229_bt_many_inserts() {
        let mut bt = super::Xj229BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_229 segment tree tests ---

    #[test]
    fn xk_229_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk229SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_229_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk229SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_229_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk229SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_229_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk229SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_229_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk229SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_229_st_single_element() {
        let data = vec![42];
        let st = super::Xk229SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_229_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk229SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_229_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk229SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_229 disjoint intervals tests ---

    #[test]
    fn xk_229_di_add_and_count() {
        let mut di = super::Xk229DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_229_di_merge_overlap() {
        let mut di = super::Xk229DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_229_di_contains() {
        let mut di = super::Xk229DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_229_di_remove() {
        let mut di = super::Xk229DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_229_di_covered_length() {
        let mut di = super::Xk229DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_229_di_gaps() {
        let mut di = super::Xk229DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_229_di_merge_adjacent() {
        let mut di = super::Xk229DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_229_di_empty() {
        let di = super::Xk229DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_229_rope_new_empty() {
        let rope = super::Xl229Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_229_rope_from_str() {
        let rope = super::Xl229Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_229_rope_insert_at() {
        let mut rope = super::Xl229Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_229_rope_delete_range() {
        let mut rope = super::Xl229Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_229_rope_char_at() {
        let rope = super::Xl229Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_229_rope_split_concat() {
        let rope = super::Xl229Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_229_rope_line_count() {
        let rope = super::Xl229Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_229_rope_line_at() {
        let rope = super::Xl229Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_229_sa_build_and_search() {
        let sa = super::Xl229SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_229_sa_count() {
        let sa = super::Xl229SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_229_sa_longest_repeated() {
        let sa = super::Xl229SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_229_sa_all_positions() {
        let sa = super::Xl229SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_229_sa_len() {
        let sa = super::Xl229SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_229_sa_empty() {
        let sa = super::Xl229SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_229_rope_slice() {
        let rope = super::Xl229Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_229_sa_search_start() {
        let sa = super::Xl229SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_229_sparse_set_get() {
        let mut m = super::Xm229MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_229_sparse_row_col() {
        let mut m = super::Xm229MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_229_sparse_transpose() {
        let mut m = super::Xm229MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_229_sparse_multiply_vec() {
        let mut m = super::Xm229MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_229_sparse_nnz_density() {
        let mut m = super::Xm229MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_229_sparse_clear() {
        let mut m = super::Xm229MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_229_sparse_overwrite_zero() {
        let mut m = super::Xm229MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_229_tokenizer_basic() {
        let t = super::Xm229Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_229_tokenizer_count() {
        let t = super::Xm229Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_229_tokenizer_unique() {
        let t = super::Xm229Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_229_tokenizer_frequency() {
        let t = super::Xm229Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_229_tokenizer_delimiter() {
        let t = super::Xm229Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_229_tokenizer_whitespace() {
        let t = super::Xm229Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_229_tokenizer_empty() {
        let t = super::Xm229Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 229 ----

    #[test]
    fn xn_229_fenwick_prefix_sum() {
        let mut ft = super::Xn229Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_229_fenwick_range_sum() {
        let mut ft = super::Xn229Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_229_fenwick_point_query() {
        let mut ft = super::Xn229Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_229_fenwick_len() {
        let ft = super::Xn229Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_229_fenwick_multiple_updates() {
        let mut ft = super::Xn229Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_229_fenwick_single_element() {
        let mut ft = super::Xn229Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_229_fenwick_find_kth() {
        let mut ft = super::Xn229Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_229_fenwick_negative_delta() {
        let mut ft = super::Xn229Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 229 ----

    #[test]
    fn xn_229_avl_insert_get() {
        let mut m = super::Xn229AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_229_avl_remove() {
        let mut m = super::Xn229AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_229_avl_in_order() {
        let mut m = super::Xn229AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_229_avl_min_max() {
        let mut m = super::Xn229AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_229_avl_floor_ceiling() {
        let mut m = super::Xn229AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_229_avl_height_balanced() {
        let mut m = super::Xn229AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_229_avl_overwrite() {
        let mut m = super::Xn229AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_229_avl_empty() {
        let m: super::Xn229AVL<i32, i32> = super::Xn229AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo229RedBlack tests ---

    #[test]
    fn xo_229_rb_insert_and_get() {
        let mut tree = super::Xo229RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_229_rb_len_and_empty() {
        let mut tree = super::Xo229RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_229_rb_min_max() {
        let mut tree = super::Xo229RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_229_rb_contains() {
        let mut tree = super::Xo229RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_229_rb_remove() {
        let mut tree = super::Xo229RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_229_rb_in_order() {
        let mut tree = super::Xo229RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_229_rb_black_height() {
        let mut tree = super::Xo229RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_229_rb_overwrite() {
        let mut tree = super::Xo229RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo229ConsistentHash tests ---

    #[test]
    fn xo_229_ch_add_and_count() {
        let mut ring = super::Xo229ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_229_ch_remove_node() {
        let mut ring = super::Xo229ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_229_ch_get_node() {
        let mut ring = super::Xo229ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_229_ch_empty_ring() {
        let ring = super::Xo229ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_229_ch_distribution() {
        let mut ring = super::Xo229ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_229_ch_rebalance() {
        let mut ring = super::Xo229ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_229_ch_virtual_nodes() {
        let mut ring = super::Xo229ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_229_ch_consistent_lookup() {
        let mut ring = super::Xo229ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_229_splay_insert_get() {
        let mut t = super::Xp229SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_229_splay_remove() {
        let mut t = super::Xp229SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_229_splay_count_increases() {
        let mut t = super::Xp229SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_229_splay_depth() {
        let mut t = super::Xp229SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_229_splay_len_empty() {
        let t = super::Xp229SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_229_splay_min_max() {
        let mut t = super::Xp229SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_229_splay_overwrite() {
        let mut t = super::Xp229SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_229_splay_remove_missing() {
        let mut t = super::Xp229SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_229 treap tests ----
    #[test]
    fn xq_229_treap_empty() {
        let t = super::Xq229Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_229_treap_insert_get() {
        let mut t = super::Xq229Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_229_treap_overwrite() {
        let mut t = super::Xq229Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_229_treap_remove() {
        let mut t = super::Xq229Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_229_treap_min_max() {
        let mut t = super::Xq229Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_229_treap_rank() {
        let mut t = super::Xq229Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_229_treap_kth() {
        let mut t = super::Xq229Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_229_treap_in_order() {
        let mut t = super::Xq229Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_229 VEB tree tests ----
    #[test]
    fn xq_229_veb_empty() {
        let v = super::Xq229VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_229_veb_insert_contains() {
        let mut v = super::Xq229VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_229_veb_min_max() {
        let mut v = super::Xq229VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_229_veb_delete() {
        let mut v = super::Xq229VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_229_veb_successor() {
        let mut v = super::Xq229VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_229_veb_predecessor() {
        let mut v = super::Xq229VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_229_veb_count() {
        let mut v = super::Xq229VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_229_veb_duplicate_insert() {
        let mut v = super::Xq229VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }

}
