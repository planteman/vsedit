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

// ---------------------------------------------------------------------------
// TerminalHistory
// ---------------------------------------------------------------------------

/// Stores command history for a terminal instance with search, dedup, and max entries.
#[derive(Debug, Clone)]
pub struct TerminalHistory {
    entries: Vec<String>,
    max_entries: usize,
    cursor: usize,
}

impl TerminalHistory {
    /// Create a new history with the given maximum number of entries.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries: max_entries.max(1),
            cursor: 0,
        }
    }

    /// Push a command into history. Duplicates of the most recent entry are ignored.
    /// When the history exceeds `max_entries`, the oldest entry is removed.
    pub fn push(&mut self, command: impl Into<String>) {
        let cmd = command.into();
        if cmd.is_empty() {
            return;
        }
        // Deduplicate against the last entry.
        if self.entries.last().map(|s| s.as_str()) == Some(cmd.as_str()) {
            return;
        }
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(cmd);
        self.cursor = self.entries.len();
    }

    /// Return the number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return true if the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return all entries as a slice.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Clear all history entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = 0;
    }

    /// Navigate backwards (older) and return the entry, if any.
    pub fn prev(&mut self) -> Option<&str> {
        if self.cursor > 0 {
            self.cursor -= 1;
            Some(&self.entries[self.cursor])
        } else {
            None
        }
    }

    /// Navigate forwards (newer) and return the entry, if any.
    pub fn next(&mut self) -> Option<&str> {
        if self.cursor < self.entries.len().saturating_sub(1) {
            self.cursor += 1;
            Some(&self.entries[self.cursor])
        } else {
            self.cursor = self.entries.len();
            None
        }
    }

    /// Search history for entries containing `query` (case-insensitive), most recent first.
    pub fn search(&self, query: &str) -> Vec<&str> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .rev()
            .filter(|e| e.to_lowercase().contains(&q))
            .map(|e| e.as_str())
            .collect()
    }

    /// Remove all entries matching `command` exactly.
    pub fn remove_all(&mut self, command: &str) {
        self.entries.retain(|e| e != command);
        self.cursor = self.entries.len();
    }
}

impl Default for TerminalHistory {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl fmt::Display for TerminalHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TerminalHistory({}/{})", self.entries.len(), self.max_entries)
    }
}

// ---------------------------------------------------------------------------
// TerminalProfileConfig
// ---------------------------------------------------------------------------

/// A saved terminal configuration describing shell, working directory, environment, and dimensions.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalProfileConfig {
    pub name: String,
    pub shell_type: TerminalShellType,
    pub cwd: Option<String>,
    pub env: HashMap<String, String>,
    pub dimensions: TerminalDimensions,
}

impl TerminalProfileConfig {
    pub fn new(name: impl Into<String>, shell_type: TerminalShellType) -> Self {
        Self {
            name: name.into(),
            shell_type,
            cwd: None,
            env: HashMap::new(),
            dimensions: TerminalDimensions::standard(),
        }
    }

    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn with_dimensions(mut self, dims: TerminalDimensions) -> Self {
        self.dimensions = dims;
        self
    }
}

impl fmt::Display for TerminalProfileConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Profile({}, shell={}, dims={})",
            self.name, self.shell_type, self.dimensions
        )
    }
}

// ---------------------------------------------------------------------------
// TerminalProfileManager
// ---------------------------------------------------------------------------

/// Manages named terminal profiles with create/get/list/delete/set_default.
#[derive(Debug, Clone)]
pub struct TerminalProfileManager {
    profiles: HashMap<String, TerminalProfileConfig>,
    default_name: Option<String>,
}

impl TerminalProfileManager {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            default_name: None,
        }
    }

    /// Add or replace a profile. If this is the first profile, it becomes the default.
    pub fn create(&mut self, profile: TerminalProfileConfig) {
        let is_first = self.profiles.is_empty();
        let name = profile.name.clone();
        self.profiles.insert(name.clone(), profile);
        if is_first {
            self.default_name = Some(name);
        }
    }

    /// Get a profile by name.
    pub fn get(&self, name: &str) -> Option<&TerminalProfileConfig> {
        self.profiles.get(name)
    }

    /// List all profile names, sorted alphabetically.
    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.profiles.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Delete a profile by name. Returns true if it existed.
    pub fn delete(&mut self, name: &str) -> bool {
        let removed = self.profiles.remove(name).is_some();
        if removed && self.default_name.as_deref() == Some(name) {
            self.default_name = self.profiles.keys().next().cloned();
        }
        removed
    }

    /// Set the default profile by name. Returns false if the name doesn't exist.
    pub fn set_default(&mut self, name: &str) -> bool {
        if self.profiles.contains_key(name) {
            self.default_name = Some(name.to_string());
            true
        } else {
            false
        }
    }

    /// Return the default profile, if any.
    pub fn default_profile(&self) -> Option<&TerminalProfileConfig> {
        self.default_name.as_deref().and_then(|n| self.profiles.get(n))
    }

    /// Return the number of profiles.
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Return true if there are no profiles.
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

impl Default for TerminalProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AnsiColor / AnsiColorParser
// ---------------------------------------------------------------------------

/// A parsed ANSI SGR color.
#[derive(Debug, Clone, PartialEq)]
pub enum AnsiColor {
    /// Standard color index 0–7 (e.g. 30–37 for foreground).
    Standard(u8),
    /// Bright color index 0–7 (e.g. 90–97 for foreground).
    Bright(u8),
    /// 256-color palette index.
    Palette(u8),
    /// 24-bit true-color RGB.
    Rgb(u8, u8, u8),
    /// Reset / default color.
    Reset,
}

impl fmt::Display for AnsiColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standard(c) => write!(f, "standard({c})"),
            Self::Bright(c) => write!(f, "bright({c})"),
            Self::Palette(c) => write!(f, "palette({c})"),
            Self::Rgb(r, g, b) => write!(f, "rgb({r},{g},{b})"),
            Self::Reset => write!(f, "reset"),
        }
    }
}

/// Result of parsing an ANSI color escape sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct AnsiColorInfo {
    pub foreground: Option<AnsiColor>,
    pub background: Option<AnsiColor>,
    pub bold: bool,
    pub underline: bool,
}

/// Parses basic ANSI SGR (Select Graphic Rendition) escape sequences.
pub struct AnsiColorParser;

impl AnsiColorParser {
    /// Parse a single SGR parameter sequence (the numbers between `\x1b[` and `m`).
    /// Returns extracted color information.
    pub fn parse_sgr(params: &str) -> AnsiColorInfo {
        let mut info = AnsiColorInfo {
            foreground: None,
            background: None,
            bold: false,
            underline: false,
        };
        let codes: Vec<u8> = params
            .split(';')
            .filter_map(|s| s.parse::<u8>().ok())
            .collect();

        let mut i = 0;
        while i < codes.len() {
            match codes[i] {
                0 => {
                    info.foreground = Some(AnsiColor::Reset);
                    info.background = Some(AnsiColor::Reset);
                    info.bold = false;
                    info.underline = false;
                }
                1 => info.bold = true,
                4 => info.underline = true,
                // Standard foreground 30–37
                c @ 30..=37 => info.foreground = Some(AnsiColor::Standard(c - 30)),
                // Standard background 40–47
                c @ 40..=47 => info.background = Some(AnsiColor::Standard(c - 40)),
                // Bright foreground 90–97
                c @ 90..=97 => info.foreground = Some(AnsiColor::Bright(c - 90)),
                // Bright background 100–107
                c @ 100..=107 => info.background = Some(AnsiColor::Bright(c - 100)),
                // Extended foreground: 38;5;n or 38;2;r;g;b
                38 if i + 1 < codes.len() => {
                    if codes[i + 1] == 5 && i + 2 < codes.len() {
                        info.foreground = Some(AnsiColor::Palette(codes[i + 2]));
                        i += 2;
                    } else if codes[i + 1] == 2 && i + 4 < codes.len() {
                        info.foreground =
                            Some(AnsiColor::Rgb(codes[i + 2], codes[i + 3], codes[i + 4]));
                        i += 4;
                    }
                }
                // Extended background: 48;5;n or 48;2;r;g;b
                48 if i + 1 < codes.len() => {
                    if codes[i + 1] == 5 && i + 2 < codes.len() {
                        info.background = Some(AnsiColor::Palette(codes[i + 2]));
                        i += 2;
                    } else if codes[i + 1] == 2 && i + 4 < codes.len() {
                        info.background =
                            Some(AnsiColor::Rgb(codes[i + 2], codes[i + 3], codes[i + 4]));
                        i += 4;
                    }
                }
                39 => info.foreground = Some(AnsiColor::Reset),
                49 => info.background = Some(AnsiColor::Reset),
                _ => {}
            }
            i += 1;
        }
        info
    }

    /// Extract all SGR sequences from an input string and return the color info for each.
    pub fn extract_colors(input: &str) -> Vec<AnsiColorInfo> {
        let mut results = Vec::new();
        let bytes = input.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\x1b' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                // Find the closing 'm'
                let start = i + 2;
                let mut end = start;
                while end < bytes.len() && bytes[end] != b'm' {
                    end += 1;
                }
                if end < bytes.len() {
                    let params = &input[start..end];
                    results.push(Self::parse_sgr(params));
                    i = end + 1;
                    continue;
                }
            }
            i += 1;
        }
        results
    }

    /// Strip all ANSI escape sequences from a string, returning only visible text.
    pub fn strip_ansi(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let bytes = input.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\x1b' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                let start = i + 2;
                let mut end = start;
                while end < bytes.len() && !(bytes[end] as char).is_ascii_alphabetic() {
                    end += 1;
                }
                // skip past the final letter
                i = if end < bytes.len() { end + 1 } else { end };
            } else {
                out.push(bytes[i] as char);
                i += 1;
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

impl From<&str> for TerminalShellType {
    fn from(s: &str) -> Self {
        Self::from_path(s)
    }
}

impl From<TerminalProfileConfig> for TerminalInstance {
    fn from(profile: TerminalProfileConfig) -> Self {
        Self {
            id: 0,
            title: profile.name,
            shell_type: profile.shell_type,
            dimensions: profile.dimensions,
            active: false,
            group: "default".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Reconnection
// ---------------------------------------------------------------------------

/// State of a terminal reconnection attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectionState {
    Idle,
    Attempting,
    Connected,
    Failed,
}

impl fmt::Display for ReconnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Attempting => write!(f, "attempting"),
            Self::Connected => write!(f, "connected"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Tracks reconnection attempts with exponential back-off.
#[derive(Debug, Clone)]
pub struct TerminalReconnection {
    pub attempts: u32,
    pub max_attempts: u32,
    pub delay_ms: u64,
    pub state: ReconnectionState,
}

impl TerminalReconnection {
    pub fn new(max_attempts: u32, delay_ms: u64) -> Self {
        Self {
            attempts: 0,
            max_attempts,
            delay_ms,
            state: ReconnectionState::Idle,
        }
    }

    /// Try another reconnection attempt. Returns `false` when attempts are
    /// exhausted.
    pub fn attempt(&mut self) -> bool {
        if self.attempts >= self.max_attempts {
            self.state = ReconnectionState::Failed;
            return false;
        }
        self.attempts += 1;
        self.state = ReconnectionState::Attempting;
        true
    }

    pub fn reset(&mut self) {
        self.attempts = 0;
        self.state = ReconnectionState::Idle;
    }

    pub fn succeeded(&mut self) {
        self.state = ReconnectionState::Connected;
    }

    pub fn is_connected(&self) -> bool {
        self.state == ReconnectionState::Connected
    }

    pub fn attempts_remaining(&self) -> u32 {
        self.max_attempts.saturating_sub(self.attempts)
    }

    /// Exponential back-off: `delay_ms * 2^(attempts - 1)`, minimum `delay_ms`.
    pub fn next_delay(&self) -> u64 {
        if self.attempts == 0 {
            return self.delay_ms;
        }
        self.delay_ms.saturating_mul(1u64 << (self.attempts - 1).min(31))
    }
}

// ---------------------------------------------------------------------------
// Audio / Visual Bell
// ---------------------------------------------------------------------------

/// Action to take when the terminal bell fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BellAction {
    Audio,
    Visual,
    Both,
    Muted,
    Disabled,
}

impl fmt::Display for BellAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Audio => write!(f, "audio"),
            Self::Visual => write!(f, "visual"),
            Self::Both => write!(f, "both"),
            Self::Muted => write!(f, "muted"),
            Self::Disabled => write!(f, "disabled"),
        }
    }
}

/// Manages bell behaviour including mute windows.
#[derive(Debug, Clone)]
pub struct TerminalAudioBell {
    pub enabled: bool,
    pub visual_bell: bool,
    pub bell_count: u32,
    pub muted_until: Option<u64>,
}

impl TerminalAudioBell {
    pub fn new() -> Self {
        Self {
            enabled: true,
            visual_bell: false,
            bell_count: 0,
            muted_until: None,
        }
    }

    /// Process a bell event at the given timestamp and return the action taken.
    pub fn on_bell(&mut self, now: u64) -> BellAction {
        self.bell_count += 1;
        if !self.enabled {
            return BellAction::Disabled;
        }
        if self.is_muted(now) {
            return BellAction::Muted;
        }
        match self.visual_bell {
            true => BellAction::Both,
            false => BellAction::Audio,
        }
    }

    pub fn enable_visual(&mut self) {
        self.visual_bell = true;
    }

    pub fn mute_for(&mut self, duration: u64, now: u64) {
        self.muted_until = Some(now.saturating_add(duration));
    }

    pub fn is_muted(&self, now: u64) -> bool {
        self.muted_until.map_or(false, |until| now < until)
    }

    pub fn total_bells(&self) -> u32 {
        self.bell_count
    }
}

// ---------------------------------------------------------------------------
// Mouse / Selection Tracking
// ---------------------------------------------------------------------------

/// A rectangular selection range in the terminal grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
    pub start_col: u16,
    pub start_row: u16,
    pub end_col: u16,
    pub end_row: u16,
}

impl SelectionRange {
    /// Number of lines the selection spans (inclusive).
    pub fn span_lines(&self) -> u16 {
        self.end_row.saturating_sub(self.start_row) + 1
    }

    pub fn is_single_line(&self) -> bool {
        self.start_row == self.end_row
    }
}

impl fmt::Display for SelectionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({},{})..({},{})",
            self.start_col, self.start_row, self.end_col, self.end_row
        )
    }
}

/// Tracks mouse-driven text selection in the terminal viewport.
#[derive(Debug, Clone)]
pub struct TerminalMouseTracker {
    pub selecting: bool,
    pub start_pos: Option<(u16, u16)>,
    pub end_pos: Option<(u16, u16)>,
    pub selection_text: Option<String>,
}

impl TerminalMouseTracker {
    pub fn new() -> Self {
        Self {
            selecting: false,
            start_pos: None,
            end_pos: None,
            selection_text: None,
        }
    }

    pub fn start_selection(&mut self, col: u16, row: u16) {
        self.selecting = true;
        self.start_pos = Some((col, row));
        self.end_pos = None;
        self.selection_text = None;
    }

    pub fn update_selection(&mut self, col: u16, row: u16) {
        if self.selecting {
            self.end_pos = Some((col, row));
        }
    }

    pub fn end_selection(&mut self) -> Option<SelectionRange> {
        self.selecting = false;
        match (self.start_pos, self.end_pos) {
            (Some((sc, sr)), Some((ec, er))) => {
                // Normalise so start <= end.
                let (start_col, start_row, end_col, end_row) = if (sr, sc) <= (er, ec) {
                    (sc, sr, ec, er)
                } else {
                    (ec, er, sc, sr)
                };
                Some(SelectionRange {
                    start_col,
                    start_row,
                    end_col,
                    end_row,
                })
            }
            _ => None,
        }
    }

    pub fn clear(&mut self) {
        self.selecting = false;
        self.start_pos = None;
        self.end_pos = None;
        self.selection_text = None;
    }

    pub fn is_selecting(&self) -> bool {
        self.selecting
    }

    pub fn has_selection(&self) -> bool {
        self.start_pos.is_some() && self.end_pos.is_some()
    }
}

// ---------------------------------------------------------------------------
// Resize Debouncer
// ---------------------------------------------------------------------------

/// Debounces rapid resize events so the terminal is only resized once the user
/// has finished dragging.
#[derive(Debug, Clone)]
pub struct TerminalResizeDebouncer {
    pub pending: Option<TerminalDimensions>,
    pub last_applied: Option<TerminalDimensions>,
    pub debounce_ms: u64,
    pub last_event_time: u64,
}

impl TerminalResizeDebouncer {
    pub fn new(debounce_ms: u64) -> Self {
        Self {
            pending: None,
            last_applied: None,
            debounce_ms,
            last_event_time: 0,
        }
    }

    pub fn on_resize(&mut self, dims: TerminalDimensions, now: u64) {
        self.pending = Some(dims);
        self.last_event_time = now;
    }

    /// Returns `true` when enough time has elapsed since the last event.
    pub fn should_apply(&self, now: u64) -> bool {
        self.pending.is_some() && now.saturating_sub(self.last_event_time) >= self.debounce_ms
    }

    /// Consume the pending dimensions, moving them to `last_applied`.
    pub fn apply(&mut self) -> Option<TerminalDimensions> {
        if let Some(dims) = self.pending.take() {
            self.last_applied = Some(dims.clone());
            Some(dims)
        } else {
            None
        }
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn reset(&mut self) {
        self.pending = None;
        self.last_applied = None;
        self.last_event_time = 0;
    }
}


// ─── WbTerm Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for terminal lines.
#[derive(Debug, Clone)]
pub struct WbTermRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> WbTermRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for WbTermRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WbTermRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── WbTerm Builder & Validator ─────────────────────────────

/// Builder for constructing terminal configurations.
#[derive(Debug, Clone)]
pub struct WbTermBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl WbTermBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<WbTermCfg, WbTermBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(WbTermBuildErr { errors }); }
        Ok(WbTermCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated terminal configuration.
#[derive(Debug, Clone)]
pub struct WbTermCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl WbTermCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &WbTermCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for WbTermCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WbTermCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct WbTermBuildErr { pub errors: Vec<String> }

impl fmt::Display for WbTermBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WbTermBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for WbTermBuildErr {}


// ---------------------------------------------------------------------------
// wb_terminal – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XWbTerminalLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XWbTerminalPanelState {
    pub region: XWbTerminalLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XWbTerminalPanelState {
    pub fn new(region: XWbTerminalLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_wb_terminal_total_visible_area(panels: &[XWbTerminalPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_wb_terminal_count_in_region(
    panels: &[XWbTerminalPanelState],
    region: XWbTerminalLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_wb_terminal_widest_panel(panels: &[XWbTerminalPanelState]) -> Option<&XWbTerminalPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_wb_terminal_collapse_region(
    panels: &mut [XWbTerminalPanelState],
    region: XWbTerminalLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XWbTerminalLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XWbTerminalLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}


/// Configuration manager for wb_terminal functionality.
pub struct WbTerminalConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl WbTerminalConfig {
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

    pub fn merge(&mut self, other: &WbTerminalConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for wb_terminal operations.
pub struct WbTerminalRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl WbTerminalRateTracker {
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

/// Validation result collector for wb_terminal.
pub struct WbTerminalValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl WbTerminalValidationCollector {
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

    pub fn merge(&mut self, other: &WbTerminalValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
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
// xb_ utilities – batch 32
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer32 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer32 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_32(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_32<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_32<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_32(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_32(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 227
// ---------------------------------------------------------------------------

/// Generic object pool `Xc227Pool<T>`.
pub struct Xc227Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc227Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc227PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc227Pool<T> {
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
    pub fn stats(&self) -> Xc227PoolStats {
        Xc227PoolStats {
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

impl<T> Default for Xc227Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc227Scheduler`.
pub struct Xc227Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc227Scheduler {
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

impl Default for Xc227Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_227 hash for the given byte slice.
pub fn xc_227_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_227 convention.
pub fn xc_227_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe44 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe44Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe44PipelineError {
    pub stage: Xe44Stage,
    pub message: String,
}

impl std::fmt::Display for Xe44PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe44Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe44Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe44PipelineError>>>,
    stage_names: Vec<Xe44Stage>,
}

impl Xe44Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe44PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe44Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe44PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe44Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe44PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe44Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe44PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe44Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe44PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe44Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe44CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe44CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe44Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe44CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe44CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe44Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe44CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_44_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe44CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_44_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe44CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_44_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe44PipelineError> {
    Ok(data)
}

pub fn xe_44_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe44PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_44_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe44PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_44_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe44PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_44_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe44PipelineError> {
    Err(Xe44PipelineError {
        stage: Xe44Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_12: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg12Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg12Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg12Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_12: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg12Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg12Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg12Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg12Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 226).
pub struct Xh226SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh226SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 268 as u64,
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

/// A compact bit set supporting boolean operations (variant 226).
pub struct Xh226BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh226BitSet {
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
    fn update_config_works() {
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
    fn default_dimensions_works() {
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
    fn get_instance_works() {
        let mut svc = TerminalWorkbenchService::new();
        let id = svc.create_instance();
        let inst = svc.get_instance(id).unwrap();
        assert_eq!(inst.id, id);
        assert_eq!(inst.title, "Terminal 1");
        assert_eq!(inst.shell_type, TerminalShellType::Bash);
        assert_eq!(svc.get_instance(999), Err(TerminalError::InstanceNotFound(999)));
    }

    #[test]
    fn rename_instance_works() {
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

    // -- TerminalHistory tests --

    #[test]
    fn test_terminal_history_push_and_dedup() {
        let mut h = TerminalHistory::new(5);
        h.push("ls");
        h.push("pwd");
        h.push("pwd"); // duplicate – should be ignored
        assert_eq!(h.len(), 2);
        assert_eq!(h.entries(), &["ls", "pwd"]);
        // empty commands ignored
        h.push("");
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn test_terminal_history_max_entries() {
        let mut h = TerminalHistory::new(3);
        h.push("a");
        h.push("b");
        h.push("c");
        h.push("d"); // should evict "a"
        assert_eq!(h.len(), 3);
        assert_eq!(h.entries(), &["b", "c", "d"]);
    }

    #[test]
    fn test_terminal_history_navigation() {
        let mut h = TerminalHistory::new(10);
        h.push("first");
        h.push("second");
        h.push("third");
        // Navigate backwards
        assert_eq!(h.prev(), Some("third"));
        assert_eq!(h.prev(), Some("second"));
        assert_eq!(h.prev(), Some("first"));
        assert_eq!(h.prev(), None); // at beginning
        // Navigate forwards
        assert_eq!(h.next(), Some("second"));
        assert_eq!(h.next(), Some("third"));
        assert_eq!(h.next(), None); // past end
    }

    #[test]
    fn test_terminal_history_search() {
        let mut h = TerminalHistory::new(10);
        h.push("git status");
        h.push("cargo build");
        h.push("git log --oneline");
        h.push("cargo test");
        let results = h.search("git");
        assert_eq!(results, vec!["git log --oneline", "git status"]);
        // Case-insensitive
        let results = h.search("CARGO");
        assert_eq!(results, vec!["cargo test", "cargo build"]);
        assert!(h.search("nonexistent").is_empty());
    }

    #[test]
    fn test_terminal_history_remove_and_clear() {
        let mut h = TerminalHistory::new(10);
        h.push("keep");
        h.push("remove");
        h.push("keep2");
        h.remove_all("remove");
        assert_eq!(h.len(), 2);
        assert_eq!(h.entries(), &["keep", "keep2"]);
        h.clear();
        assert!(h.is_empty());
        assert_eq!(h.to_string(), "TerminalHistory(0/10)");
    }

    // -- TerminalProfileConfig + Manager tests --

    #[test]
    fn test_profile_config_builder_and_display() {
        let p = TerminalProfileConfig::new("dev", TerminalShellType::Zsh)
            .with_cwd("/home/user/project")
            .with_env("EDITOR", "vim")
            .with_dimensions(TerminalDimensions::wide());
        assert_eq!(p.name, "dev");
        assert_eq!(p.shell_type, TerminalShellType::Zsh);
        assert_eq!(p.cwd.as_deref(), Some("/home/user/project"));
        assert_eq!(p.env.get("EDITOR").map(|s| s.as_str()), Some("vim"));
        assert_eq!(p.dimensions, TerminalDimensions::wide());
        let display = format!("{p}");
        assert!(display.contains("dev"));
        assert!(display.contains("zsh"));
    }

    #[test]
    fn test_profile_manager_create_get_list_delete() {
        let mut mgr = TerminalProfileManager::new();
        assert!(mgr.is_empty());

        let p1 = TerminalProfileConfig::new("bash-dev", TerminalShellType::Bash);
        let p2 = TerminalProfileConfig::new("zsh-prod", TerminalShellType::Zsh);
        mgr.create(p1);
        mgr.create(p2);
        assert_eq!(mgr.len(), 2);

        assert!(mgr.get("bash-dev").is_some());
        assert_eq!(mgr.get("bash-dev").unwrap().shell_type, TerminalShellType::Bash);
        assert!(mgr.get("nonexistent").is_none());

        let names = mgr.list();
        assert_eq!(names, vec!["bash-dev", "zsh-prod"]);

        // First profile is the default
        assert_eq!(mgr.default_profile().unwrap().name, "bash-dev");

        // Delete the default
        assert!(mgr.delete("bash-dev"));
        assert!(!mgr.delete("bash-dev")); // already gone
        assert_eq!(mgr.len(), 1);
        // Default should have shifted
        assert!(mgr.default_profile().is_some());
    }

    #[test]
    fn test_profile_manager_set_default() {
        let mut mgr = TerminalProfileManager::new();
        mgr.create(TerminalProfileConfig::new("a", TerminalShellType::Bash));
        mgr.create(TerminalProfileConfig::new("b", TerminalShellType::Zsh));
        assert!(mgr.set_default("b"));
        assert_eq!(mgr.default_profile().unwrap().name, "b");
        assert!(!mgr.set_default("nonexistent"));
    }

    // -- AnsiColorParser tests --

    #[test]
    fn test_ansi_color_parser_standard_colors() {
        let info = AnsiColorParser::parse_sgr("1;31;42");
        assert!(info.bold);
        assert_eq!(info.foreground, Some(AnsiColor::Standard(1))); // red
        assert_eq!(info.background, Some(AnsiColor::Standard(2))); // green
    }

    #[test]
    fn test_ansi_color_parser_256_and_rgb() {
        // 256-color foreground
        let info = AnsiColorParser::parse_sgr("38;5;208");
        assert_eq!(info.foreground, Some(AnsiColor::Palette(208)));
        // RGB background
        let info = AnsiColorParser::parse_sgr("48;2;100;150;200");
        assert_eq!(info.background, Some(AnsiColor::Rgb(100, 150, 200)));
    }

    #[test]
    fn test_ansi_color_parser_extract_and_strip() {
        let input = "\x1b[1;32mHello\x1b[0m World";
        let colors = AnsiColorParser::extract_colors(input);
        assert_eq!(colors.len(), 2);
        assert!(colors[0].bold);
        assert_eq!(colors[0].foreground, Some(AnsiColor::Standard(2)));
        // reset
        assert_eq!(colors[1].foreground, Some(AnsiColor::Reset));

        let stripped = AnsiColorParser::strip_ansi(input);
        assert_eq!(stripped, "Hello World");
    }

    // -- From impls tests --

    #[test]
    fn test_shell_type_from_str() {
        let s: TerminalShellType = "bash".into();
        assert_eq!(s, TerminalShellType::Bash);
        let s: TerminalShellType = "zsh".into();
        assert_eq!(s, TerminalShellType::Zsh);
    }

    #[test]
    fn test_terminal_instance_from_profile_config() {
        let profile = TerminalProfileConfig::new("my-profile", TerminalShellType::Fish)
            .with_dimensions(TerminalDimensions::wide());
        let inst: TerminalInstance = profile.into();
        assert_eq!(inst.title, "my-profile");
        assert_eq!(inst.shell_type, TerminalShellType::Fish);
        assert_eq!(inst.dimensions, TerminalDimensions::wide());
        assert_eq!(inst.group, "default");
    }

    #[test]
    fn test_ansi_color_display() {
        assert_eq!(AnsiColor::Standard(1).to_string(), "standard(1)");
        assert_eq!(AnsiColor::Bright(3).to_string(), "bright(3)");
        assert_eq!(AnsiColor::Palette(42).to_string(), "palette(42)");
        assert_eq!(AnsiColor::Rgb(10, 20, 30).to_string(), "rgb(10,20,30)");
        assert_eq!(AnsiColor::Reset.to_string(), "reset");
    }

    // -- Reconnection tests --------------------------------------------------

    #[test]
    fn reconnection_basic_flow() {
        let mut r = TerminalReconnection::new(3, 100);
        assert_eq!(r.attempts_remaining(), 3);
        assert!(r.attempt());
        assert_eq!(r.state, ReconnectionState::Attempting);
        assert_eq!(r.attempts_remaining(), 2);
        r.succeeded();
        assert!(r.is_connected());
    }

    #[test]
    fn reconnection_exhaustion() {
        let mut r = TerminalReconnection::new(2, 50);
        assert!(r.attempt());
        assert!(r.attempt());
        assert!(!r.attempt());
        assert_eq!(r.state, ReconnectionState::Failed);
        assert_eq!(r.attempts_remaining(), 0);
    }

    #[test]
    fn reconnection_reset() {
        let mut r = TerminalReconnection::new(1, 10);
        assert!(r.attempt());
        r.reset();
        assert_eq!(r.attempts, 0);
        assert_eq!(r.state, ReconnectionState::Idle);
        assert!(r.attempt()); // can attempt again after reset
    }

    #[test]
    fn reconnection_exponential_backoff() {
        let mut r = TerminalReconnection::new(5, 100);
        assert_eq!(r.next_delay(), 100); // before any attempt
        r.attempt();
        assert_eq!(r.next_delay(), 100); // 100 * 2^0
        r.attempt();
        assert_eq!(r.next_delay(), 200); // 100 * 2^1
        r.attempt();
        assert_eq!(r.next_delay(), 400); // 100 * 2^2
    }

    #[test]
    fn reconnection_state_display() {
        assert_eq!(ReconnectionState::Idle.to_string(), "idle");
        assert_eq!(ReconnectionState::Attempting.to_string(), "attempting");
        assert_eq!(ReconnectionState::Connected.to_string(), "connected");
        assert_eq!(ReconnectionState::Failed.to_string(), "failed");
    }

    // -- Audio bell tests ----------------------------------------------------

    #[test]
    fn bell_audio_only() {
        let mut bell = TerminalAudioBell::new();
        assert_eq!(bell.on_bell(1000), BellAction::Audio);
        assert_eq!(bell.total_bells(), 1);
    }

    #[test]
    fn bell_visual_both() {
        let mut bell = TerminalAudioBell::new();
        bell.enable_visual();
        assert_eq!(bell.on_bell(1000), BellAction::Both);
    }

    #[test]
    fn bell_muted() {
        let mut bell = TerminalAudioBell::new();
        bell.mute_for(500, 1000);
        assert!(bell.is_muted(1200));
        assert_eq!(bell.on_bell(1200), BellAction::Muted);
        assert!(!bell.is_muted(1500));
        assert_eq!(bell.on_bell(1500), BellAction::Audio);
    }

    #[test]
    fn bell_disabled() {
        let mut bell = TerminalAudioBell::new();
        bell.enabled = false;
        assert_eq!(bell.on_bell(0), BellAction::Disabled);
        assert_eq!(bell.total_bells(), 1);
    }

    #[test]
    fn bell_action_display() {
        assert_eq!(BellAction::Audio.to_string(), "audio");
        assert_eq!(BellAction::Visual.to_string(), "visual");
        assert_eq!(BellAction::Both.to_string(), "both");
        assert_eq!(BellAction::Muted.to_string(), "muted");
        assert_eq!(BellAction::Disabled.to_string(), "disabled");
    }

    // -- Mouse tracker tests -------------------------------------------------

    #[test]
    fn mouse_selection_lifecycle() {
        let mut mt = TerminalMouseTracker::new();
        assert!(!mt.is_selecting());
        mt.start_selection(5, 10);
        assert!(mt.is_selecting());
        mt.update_selection(20, 12);
        assert!(mt.has_selection());
        let range = mt.end_selection().unwrap();
        assert!(!mt.is_selecting());
        assert_eq!(range.start_col, 5);
        assert_eq!(range.start_row, 10);
        assert_eq!(range.end_col, 20);
        assert_eq!(range.end_row, 12);
        assert_eq!(range.span_lines(), 3);
        assert!(!range.is_single_line());
    }

    #[test]
    fn mouse_selection_single_line() {
        let mut mt = TerminalMouseTracker::new();
        mt.start_selection(0, 5);
        mt.update_selection(40, 5);
        let range = mt.end_selection().unwrap();
        assert!(range.is_single_line());
        assert_eq!(range.span_lines(), 1);
        assert_eq!(range.to_string(), "(0,5)..(40,5)");
    }

    #[test]
    fn mouse_clear() {
        let mut mt = TerminalMouseTracker::new();
        mt.start_selection(1, 1);
        mt.update_selection(10, 10);
        mt.clear();
        assert!(!mt.is_selecting());
        assert!(!mt.has_selection());
        assert!(mt.end_selection().is_none());
    }

    // -- Resize debouncer tests ----------------------------------------------

    #[test]
    fn resize_debounce_timing() {
        let mut d = TerminalResizeDebouncer::new(100);
        let dims = TerminalDimensions { columns: 120, rows: 40 };
        d.on_resize(dims.clone(), 1000);
        assert!(d.has_pending());
        assert!(!d.should_apply(1050));
        assert!(d.should_apply(1100));
        let applied = d.apply().unwrap();
        assert_eq!(applied, dims);
        assert!(!d.has_pending());
        assert_eq!(d.last_applied, Some(dims));
    }

    #[test]
    fn resize_debounce_reset() {
        let mut d = TerminalResizeDebouncer::new(50);
        d.on_resize(TerminalDimensions { columns: 80, rows: 24 }, 500);
        d.reset();
        assert!(!d.has_pending());
        assert!(d.last_applied.is_none());
        assert!(d.apply().is_none());
    }

    #[test]
    fn wbterm_ringbuf_push_get() {
        let mut rb = WbTermRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn wbterm_ringbuf_overflow() {
        let mut rb = WbTermRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn wbterm_ringbuf_clear() {
        let mut rb = WbTermRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn wbterm_ringbuf_newest_oldest() {
        let mut rb = WbTermRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn wbterm_ringbuf_to_vec() {
        let mut rb = WbTermRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn wbterm_ringbuf_is_full() {
        let mut rb = WbTermRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn wbterm_builder_valid() {
        let cfg = WbTermBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn wbterm_builder_empty_name() {
        let r = WbTermBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn wbterm_builder_bad_priority() {
        assert!(WbTermBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn wbterm_builder_zero_max() {
        assert!(WbTermBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn wbterm_cfg_merge() {
        let mut a = WbTermBuilder::new("a").property("x", "1").build().unwrap();
        let b = WbTermBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn wbterm_cfg_display() {
        let cfg = WbTermBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    // -- wb_terminal additional tests -------------------------------------------

    #[test]
    fn x_wb_terminal_panel_state_new() {
        let p = XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XWbTerminalLayoutRegion::Sidebar);
    }

    #[test]
    fn x_wb_terminal_panel_area() {
        let p = XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_wb_terminal_panel_toggle() {
        let mut p = XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_wb_terminal_panel_resize() {
        let mut p = XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_wb_terminal_panel_is_narrow() {
        let mut p = XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_wb_terminal_total_visible_area_basic() {
        let panels = vec![
            XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Sidebar, "a"),
            XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_wb_terminal_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_wb_terminal_total_visible_area_hidden() {
        let mut panels = vec![
            XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Sidebar, "a"),
            XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_wb_terminal_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_wb_terminal_count_in_region_basic() {
        let panels = vec![
            XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Sidebar, "a"),
            XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Sidebar, "b"),
            XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_wb_terminal_count_in_region(&panels, XWbTerminalLayoutRegion::Sidebar), 2);
        assert_eq!(x_wb_terminal_count_in_region(&panels, XWbTerminalLayoutRegion::Editor), 1);
        assert_eq!(x_wb_terminal_count_in_region(&panels, XWbTerminalLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_wb_terminal_widest_panel_basic() {
        let mut panels = vec![
            XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Sidebar, "narrow"),
            XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_wb_terminal_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_wb_terminal_collapse_region_basic() {
        let mut panels = vec![
            XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Sidebar, "a"),
            XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Sidebar, "b"),
            XWbTerminalPanelState::new(XWbTerminalLayoutRegion::Editor, "c"),
        ];
        x_wb_terminal_collapse_region(&mut panels, XWbTerminalLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_wb_terminal_layout_constraint_clamp() {
        let lc = XWbTerminalLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_wb_terminal_layout_constraint_satisfied() {
        let lc = XWbTerminalLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_wb_terminal_widest_panel_empty() {
        let panels: Vec<XWbTerminalPanelState> = vec![];
        assert!(x_wb_terminal_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_wb_terminal_layout_region_eq() {
        assert_eq!(XWbTerminalLayoutRegion::Sidebar, XWbTerminalLayoutRegion::Sidebar);
        assert_ne!(XWbTerminalLayoutRegion::Sidebar, XWbTerminalLayoutRegion::Panel);
    }


    #[test]
    fn wb_terminal_config_new() {
        let cfg = WbTerminalConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn wb_terminal_config_set_get() {
        let mut cfg = WbTerminalConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn wb_terminal_config_remove() {
        let mut cfg = WbTerminalConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn wb_terminal_config_keys_sorted() {
        let mut cfg = WbTerminalConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn wb_terminal_config_bump_version() {
        let mut cfg = WbTerminalConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn wb_terminal_config_clear() {
        let mut cfg = WbTerminalConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn wb_terminal_config_merge() {
        let mut cfg1 = WbTerminalConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = WbTerminalConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn wb_terminal_config_disable() {
        let mut cfg = WbTerminalConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn wb_terminal_rate_tracker_empty() {
        let rt = WbTerminalRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn wb_terminal_rate_tracker_record() {
        let mut rt = WbTerminalRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn wb_terminal_rate_tracker_prune() {
        let mut rt = WbTerminalRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn wb_terminal_validator_valid() {
        let v = WbTerminalValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn wb_terminal_validator_errors() {
        let mut v = WbTerminalValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn wb_terminal_validator_clear() {
        let mut v = WbTerminalValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn wb_terminal_validator_merge() {
        let mut v1 = WbTerminalValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = WbTerminalValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn wb_terminal_rate_tracker_clear() {
        let mut rt = WbTerminalRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
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


    #[test]
    fn xb_ring_buffer_32_push_and_len() {
        let mut rb = super::XbRingBuffer32::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_32_overwrite() {
        let mut rb = super::XbRingBuffer32::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_32_get_out_of_bounds() {
        let rb = super::XbRingBuffer32::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_32_drain_all() {
        let mut rb = super::XbRingBuffer32::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_32_peek_front_back() {
        let mut rb = super::XbRingBuffer32::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_32_clear() {
        let mut rb = super::XbRingBuffer32::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_32_capacity() {
        let rb = super::XbRingBuffer32::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_32_basic() {
        let h = super::xb_fnv1a_32(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_32(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_32_different_inputs() {
        let h1 = super::xb_fnv1a_32(b"abc");
        let h2 = super::xb_fnv1a_32(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_32_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_32(&data);
        let dec = super::xb_rle_decode_32(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_32_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_32(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_32(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_32_values() {
        assert!((super::xb_clamp_32(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_32(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_32(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_32_values() {
        assert!((super::xb_lerp_32(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_32(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_32(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_32_wrap_around_twice() {
        let mut rb = super::XbRingBuffer32::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 227 ----

    #[test]
    fn xc_227_pool_new_empty() {
        let pool: super::Xc227Pool<i32> = super::Xc227Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_227_pool_release_acquire() {
        let mut pool = super::Xc227Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_227_pool_acquire_empty() {
        let mut pool: super::Xc227Pool<i32> = super::Xc227Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_227_pool_full() {
        let mut pool = super::Xc227Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_227_pool_drain() {
        let mut pool = super::Xc227Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_227_pool_stats() {
        let mut pool = super::Xc227Pool::new(8);
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
    fn xc_227_pool_clear() {
        let mut pool = super::Xc227Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_227_pool_shrink() {
        let mut pool = super::Xc227Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_227_pool_default() {
        let pool: super::Xc227Pool<String> = super::Xc227Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_227_pool_extend() {
        let mut pool = super::Xc227Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_227_pool_retain() {
        let mut pool = super::Xc227Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_227_scheduler_round_robin() {
        let mut sched = super::Xc227Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_227_scheduler_empty() {
        let mut sched = super::Xc227Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_227_scheduler_reset() {
        let mut sched = super::Xc227Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_227_scheduler_add_remove() {
        let mut sched = super::Xc227Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_227_scheduler_targets() {
        let sched = super::Xc227Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_227_hash_empty() {
        assert_eq!(super::xc_227_hash(b""), 5381);
    }

    #[test]
    fn xc_227_hash_data() {
        let h = super::xc_227_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_227_hash(b"hello"), h);
    }

    #[test]
    fn xc_227_reverse_str() {
        assert_eq!(super::xc_227_reverse("abc"), "cba");
        assert_eq!(super::xc_227_reverse(""), "");
    }


    #[test]
    fn xe_44_pipeline_empty() {
        let p = super::Xe44Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_44_pipeline_parse_stage() {
        let p = super::Xe44Pipeline::new()
            .add_parse(super::xe_44_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_44_pipeline_transform_double() {
        let p = super::Xe44Pipeline::new()
            .add_transform(super::xe_44_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_44_pipeline_validate_reverse() {
        let p = super::Xe44Pipeline::new()
            .add_validate(super::xe_44_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_44_pipeline_emit_filter() {
        let p = super::Xe44Pipeline::new()
            .add_emit(super::xe_44_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_44_pipeline_multi_stage() {
        let p = super::Xe44Pipeline::new()
            .add_parse(super::xe_44_pipeline_identity)
            .add_transform(super::xe_44_pipeline_double)
            .add_validate(super::xe_44_pipeline_reverse)
            .add_emit(super::xe_44_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_44_pipeline_error_propagation() {
        let p = super::Xe44Pipeline::new()
            .add_parse(super::xe_44_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe44Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_44_pipeline_compose() {
        let p1 = super::Xe44Pipeline::new()
            .add_parse(super::xe_44_pipeline_identity);
        let p2 = super::Xe44Pipeline::new()
            .add_transform(super::xe_44_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_44_pipeline_error_display() {
        let e = super::Xe44PipelineError {
            stage: super::Xe44Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_44_cache_put_get() {
        let mut c = super::Xe44Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_44_cache_miss() {
        let mut c: super::Xe44Cache<&str, i32> = super::Xe44Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_44_cache_ttl_expiry() {
        let mut c = super::Xe44Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_44_cache_evict() {
        let mut c = super::Xe44Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_44_cache_capacity() {
        let mut c = super::Xe44Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_44_cache_stats() {
        let mut c = super::Xe44Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_44_cache_clear() {
        let mut c = super::Xe44Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_12 graph tests ------------------------------------------------

    #[test]
    fn xg_12_graph_empty() {
        let g = super::Xg12Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_12_graph_add_node() {
        let mut g = super::Xg12Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_12_graph_add_edge() {
        let mut g = super::Xg12Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_12_graph_neighbors() {
        let mut g = super::Xg12Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_12_graph_has_path() {
        let mut g = super::Xg12Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_12_graph_self_path() {
        let g = super::Xg12Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_12_graph_topo_sort() {
        let mut g = super::Xg12Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_12_graph_cycle_detect_false() {
        let mut g = super::Xg12Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_12_graph_cycle_detect_true() {
        let mut g = super::Xg12Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_12 heap tests -------------------------------------------------

    #[test]
    fn xg_12_heap_empty() {
        let h: super::Xg12Heap<i32> = super::Xg12Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_12_heap_push_pop() {
        let mut h = super::Xg12Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_12_heap_peek() {
        let mut h = super::Xg12Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_12_heap_drain_sorted() {
        let mut h = super::Xg12Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_12_heap_merge() {
        let mut a = super::Xg12Heap::new();
        let mut b = super::Xg12Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_12_heap_default() {
        let h: super::Xg12Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_12_graph_default() {
        let g: super::Xg12Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh226_skip_insert_contains() {
        let mut sl = super::Xh226SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh226_skip_remove() {
        let mut sl = super::Xh226SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh226_skip_len() {
        let mut sl = super::Xh226SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh226_skip_range_query() {
        let mut sl = super::Xh226SkipList::xh_new(4);
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
    fn xh226_skip_floor_ceiling() {
        let mut sl = super::Xh226SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh226_skip_rank() {
        let mut sl = super::Xh226SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh226_skip_empty() {
        let sl = super::Xh226SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh226_skip_duplicates() {
        let mut sl = super::Xh226SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh226_bitset_set_test() {
        let mut bs = super::Xh226BitSet::xh_new(256);
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
    fn xh226_bitset_clear_count() {
        let mut bs = super::Xh226BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh226_bitset_and_or_xor() {
        let mut a = super::Xh226BitSet::xh_new(128);
        let mut b = super::Xh226BitSet::xh_new(128);
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
    fn xh226_bitset_iter_ones() {
        let mut bs = super::Xh226BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh226_bitset_first_last() {
        let mut bs = super::Xh226BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh226_bitset_empty() {
        let bs = super::Xh226BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}