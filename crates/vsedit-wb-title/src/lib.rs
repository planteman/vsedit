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

}
