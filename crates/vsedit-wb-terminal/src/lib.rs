//! Terminal management.

use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TerminalShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
    Custom(String),
}

impl fmt::Display for TerminalShellType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bash => write!(f, "bash"),
            Self::Zsh => write!(f, "zsh"),
            Self::Fish => write!(f, "fish"),
            Self::PowerShell => write!(f, "powershell"),
            Self::Cmd => write!(f, "cmd"),
            Self::Custom(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerminalDimensions {
    pub columns: u32,
    pub rows: u32,
}

impl TerminalDimensions {
    /// Returns the total cell area (columns × rows).
    pub fn area(&self) -> u32 {
        self.columns * self.rows
    }

    /// Resize with minimum constraints (columns ≥ 1, rows ≥ 1).
    pub fn resize(&mut self, columns: u32, rows: u32) {
        self.columns = columns.max(1);
        self.rows = rows.max(1);
    }
}

impl fmt::Display for TerminalDimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.columns, self.rows)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CursorStyle {
    Block,
    Underline,
    Line,
}

impl fmt::Display for CursorStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Block => write!(f, "block"),
            Self::Underline => write!(f, "underline"),
            Self::Line => write!(f, "line"),
        }
    }
}

/// Errors returned by terminal operations.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalError {
    InstanceNotFound(u32),
    NoActiveInstance,
    InvalidDimensions { columns: u32, rows: u32 },
}

impl fmt::Display for TerminalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstanceNotFound(id) => write!(f, "terminal instance {id} not found"),
            Self::NoActiveInstance => write!(f, "no active terminal instance"),
            Self::InvalidDimensions { columns, rows } => {
                write!(f, "invalid dimensions: {columns}x{rows}")
            }
        }
    }
}

/// A single terminal instance.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalInstance {
    pub id: u32,
    pub title: String,
    pub shell_type: TerminalShellType,
    pub dimensions: TerminalDimensions,
    pub active: bool,
    pub group: String,
}

#[derive(Debug, Clone)]
pub struct TerminalWorkbenchConfig {
    pub default_shell: TerminalShellType,
    pub font_size: u32,
    pub font_family: String,
    pub cursor_style: CursorStyle,
    pub scrollback: u32,
}

impl Default for TerminalWorkbenchConfig {
    fn default() -> Self {
        Self {
            default_shell: TerminalShellType::Bash,
            font_size: 14,
            font_family: "monospace".into(),
            cursor_style: CursorStyle::Block,
            scrollback: 1000,
        }
    }
}

/// Builder for `TerminalWorkbenchConfig`.
#[derive(Debug)]
pub struct TerminalWorkbenchConfigBuilder {
    config: TerminalWorkbenchConfig,
}

impl TerminalWorkbenchConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: TerminalWorkbenchConfig::default(),
        }
    }

    pub fn default_shell(mut self, shell: TerminalShellType) -> Self {
        self.config.default_shell = shell;
        self
    }

    pub fn font_size(mut self, size: u32) -> Self {
        self.config.font_size = size;
        self
    }

    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.config.font_family = family.into();
        self
    }

    pub fn cursor_style(mut self, style: CursorStyle) -> Self {
        self.config.cursor_style = style;
        self
    }

    pub fn scrollback(mut self, lines: u32) -> Self {
        self.config.scrollback = lines;
        self
    }

    pub fn build(self) -> TerminalWorkbenchConfig {
        self.config
    }
}

impl Default for TerminalWorkbenchConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Service for terminal workbench functionality.
pub struct TerminalWorkbenchService {
    config: TerminalWorkbenchConfig,
    instance_count: u32,
    instances: HashMap<u32, TerminalInstance>,
    active_instance_id: Option<u32>,
}

impl TerminalWorkbenchService {
    pub fn new() -> Self {
        Self {
            config: TerminalWorkbenchConfig::default(),
            instance_count: 0,
            instances: HashMap::new(),
            active_instance_id: None,
        }
    }

    pub fn get_config(&self) -> &TerminalWorkbenchConfig {
        &self.config
    }

    pub fn update_config(&mut self, config: TerminalWorkbenchConfig) {
        self.config = config;
    }

    pub fn default_dimensions() -> TerminalDimensions {
        TerminalDimensions {
            columns: 80,
            rows: 24,
        }
    }

    pub fn create_instance(&mut self) -> u32 {
        self.instance_count += 1;
        let id = self.instance_count;
        let instance = TerminalInstance {
            id,
            title: format!("Terminal {id}"),
            shell_type: self.config.default_shell.clone(),
            dimensions: Self::default_dimensions(),
            active: false,
            group: "default".into(),
        };
        self.instances.insert(id, instance);
        id
    }

    pub fn close_instance(&mut self, id: u32) {
        if self.instances.remove(&id).is_some() {
            if self.active_instance_id == Some(id) {
                self.active_instance_id = None;
            }
        }
    }

    pub fn active_instance_count(&self) -> u32 {
        self.instances.len() as u32
    }

    /// Returns a reference to the instance with the given id.
    pub fn get_instance(&self, id: u32) -> Result<&TerminalInstance, TerminalError> {
        self.instances
            .get(&id)
            .ok_or(TerminalError::InstanceNotFound(id))
    }

    /// Renames an existing instance.
    pub fn rename_instance(&mut self, id: u32, title: impl Into<String>) -> Result<(), TerminalError> {
        self.instances
            .get_mut(&id)
            .ok_or(TerminalError::InstanceNotFound(id))?
            .title = title.into();
        Ok(())
    }

    /// Sets the active instance by id.
    pub fn set_active_instance(&mut self, id: u32) -> Result<(), TerminalError> {
        if !self.instances.contains_key(&id) {
            return Err(TerminalError::InstanceNotFound(id));
        }
        // Deactivate previous
        if let Some(prev) = self.active_instance_id {
            if let Some(inst) = self.instances.get_mut(&prev) {
                inst.active = false;
            }
        }
        self.active_instance_id = Some(id);
        if let Some(inst) = self.instances.get_mut(&id) {
            inst.active = true;
        }
        Ok(())
    }

    /// Returns the active instance id, if any.
    pub fn get_active_instance_id(&self) -> Result<u32, TerminalError> {
        self.active_instance_id.ok_or(TerminalError::NoActiveInstance)
    }

    /// Creates a new terminal instance by splitting an existing one.
    /// Copies the shell type, dimensions, and group from the source terminal.
    pub fn split_terminal(&mut self, source_id: u32) -> Result<u32, TerminalError> {
        let source = self
            .instances
            .get(&source_id)
            .ok_or(TerminalError::InstanceNotFound(source_id))?
            .clone();
        self.instance_count += 1;
        let new_id = self.instance_count;
        let instance = TerminalInstance {
            id: new_id,
            title: format!("Split of {}", source.title),
            shell_type: source.shell_type,
            dimensions: source.dimensions,
            active: false,
            group: source.group,
        };
        self.instances.insert(new_id, instance);
        Ok(new_id)
    }

    /// Moves a terminal instance to the specified group.
    pub fn move_to_group(&mut self, terminal_id: u32, group: impl Into<String>) -> Result<(), TerminalError> {
        self.instances
            .get_mut(&terminal_id)
            .ok_or(TerminalError::InstanceNotFound(terminal_id))?
            .group = group.into();
        Ok(())
    }
}

impl Default for TerminalWorkbenchService {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for wb-terminal operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbTerminalStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbTerminalStats {
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
    pub fn merge(&mut self, other: &WbTerminalStats) {
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

impl Default for WbTerminalStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbTerminalStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbTerminalStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-terminal.
#[derive(Debug, Clone)]
pub struct WbTerminalValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbTerminalValidator {
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

impl Default for WbTerminalValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TerminalProfile
// ---------------------------------------------------------------------------

/// A registered terminal shell profile.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalProfile {
    pub name: String,
    pub shell: TerminalShellType,
    pub path: String,
}

// ---------------------------------------------------------------------------
// TerminalProfileResolver
// ---------------------------------------------------------------------------

/// Resolves terminal shell profiles by name.
pub struct TerminalProfileResolver {
    profiles: Vec<TerminalProfile>,
}

impl TerminalProfileResolver {
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    pub fn add_profile(&mut self, name: &str, shell: TerminalShellType, path: &str) {
        self.profiles.push(TerminalProfile {
            name: name.to_string(),
            shell,
            path: path.to_string(),
        });
    }

    pub fn resolve(&self, name: &str) -> Option<&TerminalProfile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    pub fn default_profile(&self) -> Option<&TerminalProfile> {
        self.profiles.first()
    }

    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    pub fn profile_names(&self) -> Vec<&str> {
        self.profiles.iter().map(|p| p.name.as_str()).collect()
    }
}

impl Default for TerminalProfileResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TerminalEnvironmentMerge
// ---------------------------------------------------------------------------

/// Merges environment variables for terminal instances.
pub struct TerminalEnvironmentMerge {
    vars: HashMap<String, String>,
}

impl TerminalEnvironmentMerge {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.vars.insert(key.to_string(), value.to_string());
    }

    /// Append a value to an existing variable with `:` separator.
    pub fn append_path(&mut self, key: &str, value: &str) {
        self.vars
            .entry(key.to_string())
            .and_modify(|existing| {
                existing.push(':');
                existing.push_str(value);
            })
            .or_insert_with(|| value.to_string());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(|s| s.as_str())
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.vars.remove(key).is_some()
    }

    pub fn to_vec(&self) -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = self
            .vars
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs
    }

    pub fn len(&self) -> usize {
        self.vars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }
}

impl Default for TerminalEnvironmentMerge {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// terminal_title_from_process
// ---------------------------------------------------------------------------

/// Generate a terminal title from a process name and arguments.
/// Truncates to 40 characters with `…` if too long.
pub fn terminal_title_from_process(process_name: &str, args: &[&str]) -> String {
    let full = if args.is_empty() {
        process_name.to_string()
    } else {
        format!("{} {}", process_name, args.join(" "))
    };
    if full.chars().count() <= 40 {
        full
    } else {
        let truncated: String = full.chars().take(39).collect();
        format!("{truncated}\u{2026}")
    }
}

/// Result of searching terminal titles.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalSearchResult {
    pub terminal_id: String,
    pub line_number: usize,
    pub content: String,
    pub match_start: usize,
    pub match_end: usize,
}

/// Searches terminal titles for the given query string.
pub fn search_terminals(service: &TerminalWorkbenchService, query: &str) -> Vec<TerminalSearchResult> {
    let mut results = Vec::new();
    for inst in service.instances.values() {
        if let Some(start) = inst.title.find(query) {
            results.push(TerminalSearchResult {
                terminal_id: inst.id.to_string(),
                line_number: 1,
                content: inst.title.clone(),
                match_start: start,
                match_end: start + query.len(),
            });
        }
    }
    results
}

// ---------------------------------------------------------------------------
// TerminalShellType helpers
// ---------------------------------------------------------------------------

impl TerminalShellType {
    /// Returns all built-in (non-custom) shell types.
    pub fn builtins() -> &'static [TerminalShellType] {
        &[
            TerminalShellType::Bash,
            TerminalShellType::Zsh,
            TerminalShellType::Fish,
            TerminalShellType::PowerShell,
            TerminalShellType::Cmd,
        ]
    }

    /// Parse a shell type from a path or name string.
    pub fn from_path(path: &str) -> Self {
        let name = path.rsplit('/').next().unwrap_or(path);
        let name = name.rsplit('\\').next().unwrap_or(name);
        match name.to_lowercase().as_str() {
            "bash" | "bash.exe" => Self::Bash,
            "zsh" => Self::Zsh,
            "fish" => Self::Fish,
            "powershell" | "pwsh" | "powershell.exe" | "pwsh.exe" => Self::PowerShell,
            "cmd" | "cmd.exe" => Self::Cmd,
            _ => Self::Custom(name.to_string()),
        }
    }

    /// Returns the default prompt character for this shell.
    pub fn prompt_char(&self) -> &str {
        match self {
            Self::Bash | Self::Zsh => "$",
            Self::Fish => ">",
            Self::PowerShell => "PS>",
            Self::Cmd => ">",
            Self::Custom(_) => "$",
        }
    }

    /// Returns true if this is a Unix shell.
    pub fn is_unix(&self) -> bool {
        matches!(self, Self::Bash | Self::Zsh | Self::Fish)
    }

    /// Returns true if this is a Windows shell.
    pub fn is_windows(&self) -> bool {
        matches!(self, Self::PowerShell | Self::Cmd)
    }
}

// ---------------------------------------------------------------------------
// TerminalDimensions helpers
// ---------------------------------------------------------------------------

impl TerminalDimensions {
    /// Standard 80x24 terminal size.
    pub fn standard() -> Self {
        Self { columns: 80, rows: 24 }
    }

    /// Wide terminal (120x40).
    pub fn wide() -> Self {
        Self { columns: 120, rows: 40 }
    }

    /// Returns true if the terminal is wider than it is tall (in cells).
    pub fn is_landscape(&self) -> bool {
        self.columns > self.rows
    }

    /// Returns (columns, rows) as a tuple.
    pub fn as_tuple(&self) -> (u32, u32) {
        (self.columns, self.rows)
    }

    /// Scale dimensions by a factor (rounded).
    pub fn scale(&self, factor: f64) -> Self {
        Self {
            columns: (self.columns as f64 * factor).round() as u32,
            rows: (self.rows as f64 * factor).round() as u32,
        }
    }
}

impl Default for TerminalDimensions {
    fn default() -> Self {
        Self::standard()
    }
}

impl From<(u32, u32)> for TerminalDimensions {
    fn from((columns, rows): (u32, u32)) -> Self {
        Self { columns, rows }
    }
}

// ---------------------------------------------------------------------------
// Terminal escape sequence helpers
// ---------------------------------------------------------------------------

/// Generate an ANSI escape sequence to set the terminal title.
pub fn ansi_set_title(title: &str) -> String {
    format!("\x1b]0;{title}\x07")
}

/// Generate an ANSI escape to clear the screen.
pub fn ansi_clear_screen() -> &'static str {
    "\x1b[2J\x1b[H"
}

/// Generate an ANSI escape to move the cursor.
pub fn ansi_cursor_move(row: u32, col: u32) -> String {
    format!("\x1b[{};{}H", row, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_management() {
        let mut svc = TerminalWorkbenchService::new();
        assert_eq!(svc.active_instance_count(), 0);
        let id1 = svc.create_instance();
        let _id2 = svc.create_instance();
        assert_eq!(svc.active_instance_count(), 2);
        svc.close_instance(id1);
        assert_eq!(svc.active_instance_count(), 1);
    }

    #[test]
    fn default_config() {
        let svc = TerminalWorkbenchService::new();
        let cfg = svc.get_config();
        assert_eq!(cfg.default_shell, TerminalShellType::Bash);
        assert_eq!(cfg.font_size, 14);
        assert_eq!(cfg.cursor_style, CursorStyle::Block);
    }

    #[test]
    fn update_config() {
        let mut svc = TerminalWorkbenchService::new();
        let cfg = TerminalWorkbenchConfig {
            default_shell: TerminalShellType::Zsh,
            font_size: 16,
            font_family: "Fira Code".into(),
            cursor_style: CursorStyle::Line,
            scrollback: 5000,
        };
        svc.update_config(cfg);
        assert_eq!(svc.get_config().default_shell, TerminalShellType::Zsh);
        assert_eq!(svc.get_config().scrollback, 5000);
    }

    #[test]
    fn default_dimensions() {
        let dims = TerminalWorkbenchService::default_dimensions();
        assert_eq!(dims.columns, 80);
        assert_eq!(dims.rows, 24);
    }

    #[test]
    fn display_shell_type() {
        assert_eq!(TerminalShellType::Bash.to_string(), "bash");
        assert_eq!(TerminalShellType::Zsh.to_string(), "zsh");
        assert_eq!(TerminalShellType::Fish.to_string(), "fish");
        assert_eq!(TerminalShellType::PowerShell.to_string(), "powershell");
        assert_eq!(TerminalShellType::Cmd.to_string(), "cmd");
        assert_eq!(TerminalShellType::Custom("/bin/sh".into()).to_string(), "/bin/sh");
    }

    #[test]
    fn display_cursor_style() {
        assert_eq!(CursorStyle::Block.to_string(), "block");
        assert_eq!(CursorStyle::Underline.to_string(), "underline");
        assert_eq!(CursorStyle::Line.to_string(), "line");
    }

    #[test]
    fn display_dimensions() {
        let dims = TerminalDimensions { columns: 120, rows: 40 };
        assert_eq!(dims.to_string(), "120x40");
    }

    #[test]
    fn dimensions_area() {
        let dims = TerminalDimensions { columns: 80, rows: 24 };
        assert_eq!(dims.area(), 1920);
    }

    #[test]
    fn dimensions_resize() {
        let mut dims = TerminalDimensions { columns: 80, rows: 24 };
        dims.resize(120, 40);
        assert_eq!(dims.columns, 120);
        assert_eq!(dims.rows, 40);
        dims.resize(0, 0);
        assert_eq!(dims.columns, 1);
        assert_eq!(dims.rows, 1);
    }

    #[test]
    fn terminal_error_display() {
        assert_eq!(
            TerminalError::InstanceNotFound(42).to_string(),
            "terminal instance 42 not found"
        );
        assert_eq!(
            TerminalError::NoActiveInstance.to_string(),
            "no active terminal instance"
        );
        assert_eq!(
            TerminalError::InvalidDimensions { columns: 0, rows: 0 }.to_string(),
            "invalid dimensions: 0x0"
        );
    }

    #[test]
    fn get_instance() {
        let mut svc = TerminalWorkbenchService::new();
        let id = svc.create_instance();
        let inst = svc.get_instance(id).unwrap();
        assert_eq!(inst.id, id);
        assert_eq!(inst.title, "Terminal 1");
        assert_eq!(inst.shell_type, TerminalShellType::Bash);
        assert_eq!(svc.get_instance(999), Err(TerminalError::InstanceNotFound(999)));
    }

    #[test]
    fn rename_instance() {
        let mut svc = TerminalWorkbenchService::new();
        let id = svc.create_instance();
        svc.rename_instance(id, "Dev Server").unwrap();
        assert_eq!(svc.get_instance(id).unwrap().title, "Dev Server");
        assert_eq!(
            svc.rename_instance(999, "nope"),
            Err(TerminalError::InstanceNotFound(999))
        );
    }

    #[test]
    fn active_instance_tracking() {
        let mut svc = TerminalWorkbenchService::new();
        assert_eq!(svc.get_active_instance_id(), Err(TerminalError::NoActiveInstance));
        let id1 = svc.create_instance();
        let id2 = svc.create_instance();
        svc.set_active_instance(id1).unwrap();
        assert_eq!(svc.get_active_instance_id(), Ok(id1));
        assert!(svc.get_instance(id1).unwrap().active);
        svc.set_active_instance(id2).unwrap();
        assert_eq!(svc.get_active_instance_id(), Ok(id2));
        assert!(!svc.get_instance(id1).unwrap().active);
        assert!(svc.get_instance(id2).unwrap().active);
        assert_eq!(
            svc.set_active_instance(999),
            Err(TerminalError::InstanceNotFound(999))
        );
    }

    #[test]
    fn close_active_instance_clears_active() {
        let mut svc = TerminalWorkbenchService::new();
        let id = svc.create_instance();
        svc.set_active_instance(id).unwrap();
        svc.close_instance(id);
        assert_eq!(svc.get_active_instance_id(), Err(TerminalError::NoActiveInstance));
        assert_eq!(svc.active_instance_count(), 0);
    }

    #[test]
    fn config_builder() {
        let cfg = TerminalWorkbenchConfigBuilder::new()
            .default_shell(TerminalShellType::Fish)
            .font_size(18)
            .font_family("JetBrains Mono")
            .cursor_style(CursorStyle::Underline)
            .scrollback(2000)
            .build();
        assert_eq!(cfg.default_shell, TerminalShellType::Fish);
        assert_eq!(cfg.font_size, 18);
        assert_eq!(cfg.font_family, "JetBrains Mono");
        assert_eq!(cfg.cursor_style, CursorStyle::Underline);
        assert_eq!(cfg.scrollback, 2000);
    }

    #[test]
    fn eq_terminalshelltype_same() {
        assert_eq!(TerminalShellType::Bash, TerminalShellType::Bash);
    }

    #[test]
    fn ne_terminalshelltype_diff() {
        assert_ne!(TerminalShellType::Bash, TerminalShellType::Zsh);
    }

    #[test]
    fn eq_cursorstyle_same() {
        assert_eq!(CursorStyle::Block, CursorStyle::Block);
    }

    #[test]
    fn ne_cursorstyle_diff() {
        assert_ne!(CursorStyle::Block, CursorStyle::Underline);
    }

    #[test]
    fn eq_terminalerror_same() {
        assert_eq!(TerminalError::NoActiveInstance, TerminalError::NoActiveInstance);
    }

    #[test]
    fn ne_terminalerror_diff() {
        assert_ne!(TerminalError::NoActiveInstance, TerminalError::InstanceNotFound(1));
    }

    #[test]
    fn display_terminalshelltype_variants() {
        assert!(!TerminalShellType::Bash.to_string().is_empty());
        assert!(!TerminalShellType::Zsh.to_string().is_empty());
        assert!(!TerminalShellType::Fish.to_string().is_empty());
        assert!(!TerminalShellType::PowerShell.to_string().is_empty());
        assert!(!TerminalShellType::Cmd.to_string().is_empty());
    }

    #[test]
    fn display_cursorstyle_variants() {
        assert!(!CursorStyle::Block.to_string().is_empty());
        assert!(!CursorStyle::Underline.to_string().is_empty());
        assert!(!CursorStyle::Line.to_string().is_empty());
    }

    #[test]
    fn display_terminalerror_variants() {
        assert!(!TerminalError::NoActiveInstance.to_string().is_empty());
        assert!(!TerminalError::NoActiveInstance.to_string().is_empty());
    }

    // -- TerminalProfileResolver tests --

    #[test]
    fn profile_resolver_empty() {
        let resolver = TerminalProfileResolver::new();
        assert_eq!(resolver.profile_count(), 0);
        assert!(resolver.default_profile().is_none());
        assert!(resolver.resolve("bash").is_none());
    }

    #[test]
    fn profile_resolver_add_and_resolve() {
        let mut resolver = TerminalProfileResolver::new();
        resolver.add_profile("bash", TerminalShellType::Bash, "/bin/bash");
        resolver.add_profile("zsh", TerminalShellType::Zsh, "/bin/zsh");
        assert_eq!(resolver.profile_count(), 2);
        let bash = resolver.resolve("bash").unwrap();
        assert_eq!(bash.shell, TerminalShellType::Bash);
        assert_eq!(bash.path, "/bin/bash");
        assert!(resolver.resolve("fish").is_none());
    }

    #[test]
    fn profile_resolver_default_profile() {
        let mut resolver = TerminalProfileResolver::new();
        resolver.add_profile("zsh", TerminalShellType::Zsh, "/bin/zsh");
        resolver.add_profile("bash", TerminalShellType::Bash, "/bin/bash");
        assert_eq!(resolver.default_profile().unwrap().name, "zsh");
    }

    #[test]
    fn profile_resolver_names() {
        let mut resolver = TerminalProfileResolver::new();
        resolver.add_profile("bash", TerminalShellType::Bash, "/bin/bash");
        resolver.add_profile("zsh", TerminalShellType::Zsh, "/bin/zsh");
        assert_eq!(resolver.profile_names(), vec!["bash", "zsh"]);
    }

    // -- TerminalEnvironmentMerge tests --

    #[test]
    fn env_merge_set_and_get() {
        let mut env = TerminalEnvironmentMerge::new();
        env.set("HOME", "/home/user");
        assert_eq!(env.get("HOME"), Some("/home/user"));
        assert_eq!(env.get("MISSING"), None);
        assert_eq!(env.len(), 1);
    }

    #[test]
    fn env_merge_append_path() {
        let mut env = TerminalEnvironmentMerge::new();
        env.set("PATH", "/usr/bin");
        env.append_path("PATH", "/usr/local/bin");
        assert_eq!(env.get("PATH"), Some("/usr/bin:/usr/local/bin"));
    }

    #[test]
    fn env_merge_append_path_new_key() {
        let mut env = TerminalEnvironmentMerge::new();
        env.append_path("PATH", "/usr/bin");
        assert_eq!(env.get("PATH"), Some("/usr/bin"));
    }

    #[test]
    fn env_merge_remove() {
        let mut env = TerminalEnvironmentMerge::new();
        env.set("KEY", "val");
        assert!(env.remove("KEY"));
        assert!(!env.remove("KEY"));
        assert!(env.is_empty());
    }

    #[test]
    fn env_merge_to_vec() {
        let mut env = TerminalEnvironmentMerge::new();
        env.set("B", "2");
        env.set("A", "1");
        let pairs = env.to_vec();
        assert_eq!(pairs, vec![("A".into(), "1".into()), ("B".into(), "2".into())]);
    }

    // -- terminal_title_from_process tests --

    #[test]
    fn title_from_process_no_args() {
        assert_eq!(terminal_title_from_process("bash", &[]), "bash");
    }

    #[test]
    fn title_from_process_with_args() {
        let title = terminal_title_from_process("node", &["server.js", "--port", "3000"]);
        assert_eq!(title, "node server.js --port 3000");
    }

    #[test]
    fn title_from_process_truncated() {
        let title = terminal_title_from_process(
            "python3",
            &["very_long_script_name_that_exceeds_the_limit.py", "--verbose"],
        );
        assert_eq!(title.chars().count(), 40);
        assert!(title.ends_with('\u{2026}'));
    }

    #[test]
    fn wb_terminal_stats_new_defaults() {
        let stats = WbTerminalStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_terminal_stats_record_success() {
        let mut stats = WbTerminalStats::new();
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
    fn wb_terminal_stats_record_failure() {
        let mut stats = WbTerminalStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_terminal_stats_reset() {
        let mut stats = WbTerminalStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_terminal_stats_merge() {
        let mut a = WbTerminalStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbTerminalStats::new();
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
    fn wb_terminal_stats_display() {
        let mut stats = WbTerminalStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_terminal_stats_default() {
        let stats = WbTerminalStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_terminal_validator_accepts_valid_name() {
        let v = WbTerminalValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_terminal_validator_rejects_empty() {
        let v = WbTerminalValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_terminal_validator_rejects_too_long() {
        let v = WbTerminalValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_terminal_validator_forbidden_prefix() {
        let v = WbTerminalValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_terminal_validator_allowed_chars() {
        let v = WbTerminalValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_terminal_validator_range() {
        let v = WbTerminalValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_terminal_sanitize_removes_control() {
        let result = WbTerminalValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_terminal_truncate_short_string() {
        assert_eq!(WbTerminalValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_terminal_truncate_long_string() {
        let result = WbTerminalValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_terminal_is_ascii_printable() {
        assert!(WbTerminalValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbTerminalValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn test_split_terminal_basic() {
        let mut svc = TerminalWorkbenchService::new();
        let id = svc.create_instance();
        svc.rename_instance(id, "Dev").unwrap();
        let split_id = svc.split_terminal(id).unwrap();
        let split = svc.get_instance(split_id).unwrap();
        assert_eq!(split.title, "Split of Dev");
        assert_eq!(split.shell_type, svc.get_instance(id).unwrap().shell_type);
        assert_ne!(split_id, id);
    }

    #[test]
    fn test_split_terminal_not_found() {
        let mut svc = TerminalWorkbenchService::new();
        assert_eq!(svc.split_terminal(999), Err(TerminalError::InstanceNotFound(999)));
    }

    #[test]
    fn test_move_to_group() {
        let mut svc = TerminalWorkbenchService::new();
        let id = svc.create_instance();
        assert_eq!(svc.get_instance(id).unwrap().group, "default");
        svc.move_to_group(id, "editors").unwrap();
        assert_eq!(svc.get_instance(id).unwrap().group, "editors");
    }

    #[test]
    fn test_move_to_group_not_found() {
        let mut svc = TerminalWorkbenchService::new();
        assert_eq!(svc.move_to_group(999, "editors"), Err(TerminalError::InstanceNotFound(999)));
    }

    #[test]
    fn test_search_terminals_found() {
        let mut svc = TerminalWorkbenchService::new();
        let id = svc.create_instance();
        svc.rename_instance(id, "Dev Server").unwrap();
        let _id2 = svc.create_instance();
        let results = search_terminals(&svc, "Dev");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].terminal_id, id.to_string());
        assert_eq!(results[0].content, "Dev Server");
        assert_eq!(results[0].match_start, 0);
        assert_eq!(results[0].match_end, 3);
    }

    #[test]
    fn test_search_terminals_not_found() {
        let mut svc = TerminalWorkbenchService::new();
        svc.create_instance();
        let results = search_terminals(&svc, "nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_shell_type_builtins() {
        assert_eq!(TerminalShellType::builtins().len(), 5);
    }

    #[test]
    fn test_shell_type_from_path() {
        assert_eq!(TerminalShellType::from_path("/usr/bin/bash"), TerminalShellType::Bash);
        assert_eq!(TerminalShellType::from_path("/usr/bin/zsh"), TerminalShellType::Zsh);
        assert_eq!(TerminalShellType::from_path("C:\\Windows\\cmd.exe"), TerminalShellType::Cmd);
        assert_eq!(TerminalShellType::from_path("pwsh"), TerminalShellType::PowerShell);
        assert!(matches!(TerminalShellType::from_path("myshell"), TerminalShellType::Custom(_)));
    }

    #[test]
    fn test_shell_type_prompt_and_unix_windows() {
        assert_eq!(TerminalShellType::Bash.prompt_char(), "$");
        assert_eq!(TerminalShellType::Cmd.prompt_char(), ">");
        assert!(TerminalShellType::Zsh.is_unix());
        assert!(!TerminalShellType::Zsh.is_windows());
        assert!(TerminalShellType::Cmd.is_windows());
    }

    #[test]
    fn test_terminal_dimensions_standard() {
        let std = TerminalDimensions::standard();
        assert_eq!(std.columns, 80);
        assert_eq!(std.rows, 24);
        assert_eq!(std.area(), 1920);
        assert!(std.is_landscape());
    }

    #[test]
    fn test_terminal_dimensions_display_and_default() {
        let d = TerminalDimensions::default();
        assert_eq!(format!("{d}"), "80x24");
    }

    #[test]
    fn test_terminal_dimensions_from_tuple() {
        let d: TerminalDimensions = (100u32, 50u32).into();
        assert_eq!(d.columns, 100);
        assert_eq!(d.rows, 50);
        assert_eq!(d.as_tuple(), (100, 50));
    }

    #[test]
    fn test_terminal_dimensions_scale() {
        let d = TerminalDimensions::standard().scale(2.0);
        assert_eq!(d.columns, 160);
        assert_eq!(d.rows, 48);
    }

    #[test]
    fn test_ansi_set_title() {
        let seq = ansi_set_title("Test");
        assert!(seq.contains("Test"));
        assert!(seq.starts_with("\x1b]0;"));
    }

    #[test]
    fn test_ansi_cursor_move() {
        let seq = ansi_cursor_move(5, 10);
        assert_eq!(seq, "\x1b[5;10H");
    }
}
