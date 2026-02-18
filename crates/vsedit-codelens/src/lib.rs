//! Inline code lens decorations.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during code lens operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeLensError {
    /// The range is invalid (start is after end).
    InvalidRange {
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    },
    /// A required field was missing when building a command.
    MissingField(&'static str),
    /// The lens could not be resolved by any provider.
    UnresolvedLens { data: String },
}

impl fmt::Display for CodeLensError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange {
                start_line,
                start_col,
                end_line,
                end_col,
            } => write!(
                f,
                "invalid range: ({start_line}:{start_col}) > ({end_line}:{end_col})"
            ),
            Self::MissingField(name) => write!(f, "missing required field: {name}"),
            Self::UnresolvedLens { data } => {
                write!(f, "lens with data '{data}' could not be resolved")
            }
        }
    }
}

impl std::error::Error for CodeLensError {}

// ---------------------------------------------------------------------------
// Command & CommandBuilder
// ---------------------------------------------------------------------------

/// A command associated with a code lens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub title: String,
    pub command_id: String,
    pub tooltip: String,
    pub arguments: Vec<String>,
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.command_id, self.title)
    }
}

/// Builder for constructing a [`Command`] with validation.
#[derive(Debug, Clone, Default)]
pub struct CommandBuilder {
    title: Option<String>,
    command_id: Option<String>,
    tooltip: Option<String>,
    arguments: Vec<String>,
}

impl CommandBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn command_id(mut self, id: impl Into<String>) -> Self {
        self.command_id = Some(id.into());
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn argument(mut self, arg: impl Into<String>) -> Self {
        self.arguments.push(arg.into());
        self
    }

    pub fn arguments(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.arguments.extend(args.into_iter().map(Into::into));
        self
    }

    /// Build the command, returning an error if required fields are missing.
    pub fn build(self) -> Result<Command, CodeLensError> {
        let title = self.title.ok_or(CodeLensError::MissingField("title"))?;
        let command_id = self
            .command_id
            .ok_or(CodeLensError::MissingField("command_id"))?;
        Ok(Command {
            title,
            command_id,
            tooltip: self.tooltip.unwrap_or_default(),
            arguments: self.arguments,
        })
    }
}

/// A code lens representing a command anchored to a source range.
///
/// A code lens may be unresolved (no command yet) when first returned by a
/// provider. The service calls [`CodeLensProvider::resolve_code_lens`] to
/// fill in the command lazily.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLens {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub command: Option<Command>,
    pub data: String,
}

impl fmt::Display for CodeLens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.is_resolved() {
            "resolved"
        } else {
            "unresolved"
        };
        write!(
            f,
            "CodeLens({}:{}-{}:{}, {status})",
            self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
}

impl CodeLens {
    /// Create a new unresolved code lens for the given range.
    pub fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
            command: None,
            data: String::new(),
        }
    }

    /// Create a code lens with range validation.
    pub fn try_new(
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> Result<Self, CodeLensError> {
        if start_line > end_line || (start_line == end_line && start_col > end_col) {
            return Err(CodeLensError::InvalidRange {
                start_line,
                start_col,
                end_line,
                end_col,
            });
        }
        Ok(Self::new(start_line, start_col, end_line, end_col))
    }

    /// Returns `true` if a command has been attached.
    pub fn is_resolved(&self) -> bool {
        self.command.is_some()
    }

    /// Attach a command to this lens, making it resolved.
    pub fn with_command(mut self, command: Command) -> Self {
        self.command = Some(command);
        self
    }

    /// Attach provider-specific data to this lens.
    pub fn with_data(mut self, data: impl Into<String>) -> Self {
        self.data = data.into();
        self
    }

    /// Returns `true` if this lens spans a single line.
    pub fn is_single_line(&self) -> bool {
        self.start_line == self.end_line
    }

    /// Number of lines this lens spans (inclusive).
    pub fn line_span(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Returns `true` if the given line falls within this lens's range.
    pub fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }
}

/// Provider that supplies code lenses for a document.
pub trait CodeLensProvider: Send + Sync {
    /// Return all code lenses for the given document URI.
    ///
    /// Lenses may be returned unresolved (without a command); the service will
    /// call [`resolve_code_lens`](CodeLensProvider::resolve_code_lens) later.
    fn provide_code_lenses(&self, uri: &str) -> Vec<CodeLens>;

    /// Fill in the command for an unresolved code lens.
    ///
    /// The default implementation returns the lens unchanged.
    fn resolve_code_lens(&self, lens: CodeLens) -> CodeLens {
        lens
    }
}

/// Service that manages registered [`CodeLensProvider`]s and collects lenses.
pub struct CodeLensService {
    providers: Vec<Box<dyn CodeLensProvider>>,
}

impl CodeLensService {
    /// Create an empty service with no providers.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a code lens provider.
    pub fn register(&mut self, provider: impl CodeLensProvider + 'static) {
        self.providers.push(Box::new(provider));
    }

    /// Returns the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Collect code lenses from all providers for the given URI.
    pub fn provide_code_lenses(&self, uri: &str) -> Vec<CodeLens> {
        self.providers
            .iter()
            .flat_map(|p| p.provide_code_lenses(uri))
            .collect()
    }

    /// Resolve all unresolved lenses using the provider that produced them.
    ///
    /// This convenience method resolves every lens by iterating providers in
    /// registration order and calling `resolve_code_lens` on the first
    /// provider that returns a resolved result.
    pub fn resolve_all(&self, lenses: Vec<CodeLens>) -> Vec<CodeLens> {
        lenses
            .into_iter()
            .map(|lens| {
                if lens.is_resolved() {
                    return lens;
                }
                for provider in &self.providers {
                    let resolved = provider.resolve_code_lens(lens.clone());
                    if resolved.is_resolved() {
                        return resolved;
                    }
                }
                lens
            })
            .collect()
    }
}

impl Default for CodeLensService {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CodeLensService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodeLensService")
            .field("provider_count", &self.providers.len())
            .finish()
    }
}

impl CodeLensService {
    /// Collect and immediately resolve all lenses for the given URI.
    pub fn provide_and_resolve(&self, uri: &str) -> Vec<CodeLens> {
        let lenses = self.provide_code_lenses(uri);
        self.resolve_all(lenses)
    }

    /// Resolve lenses, returning an error for any that remain unresolved.
    pub fn resolve_all_strict(&self, lenses: Vec<CodeLens>) -> Result<Vec<CodeLens>, CodeLensError> {
        let resolved = self.resolve_all(lenses);
        for lens in &resolved {
            if !lens.is_resolved() {
                return Err(CodeLensError::UnresolvedLens {
                    data: lens.data.clone(),
                });
            }
        }
        Ok(resolved)
    }

    /// Return only the lenses that overlap the given line.
    pub fn lenses_at_line(&self, uri: &str, line: u32) -> Vec<CodeLens> {
        self.provide_code_lenses(uri)
            .into_iter()
            .filter(|l| l.contains_line(line))
            .collect()
    }
}

/// Accumulated statistics for codelens operations.
#[derive(Debug, Clone, PartialEq)]
pub struct CodelensStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl CodelensStats {
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
    pub fn merge(&mut self, other: &CodelensStats) {
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

impl Default for CodelensStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CodelensStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CodelensStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for codelens.
#[derive(Debug, Clone)]
pub struct CodelensValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl CodelensValidator {
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

impl Default for CodelensValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Code lens rendering
// ---------------------------------------------------------------------------

/// Style descriptor for rendering code lenses above a line.
#[derive(Debug, Clone)]
pub struct CodeLensStyle {
    /// Prefix before the entire lens line (e.g. ANSI dim-on).
    pub prefix: String,
    /// Suffix after the entire lens line (e.g. ANSI reset).
    pub suffix: String,
    /// Separator between lenses on the same line (e.g. " | ").
    pub separator: String,
}

impl Default for CodeLensStyle {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            suffix: String::new(),
            separator: " | ".into(),
        }
    }
}

/// Render a code-lens annotation line that appears above a source line.
///
/// Returns `None` if there are no resolved lenses with a command title.
/// Unresolved lenses (no command) are silently skipped.
pub fn render_code_lens_line(lenses: &[CodeLens], style: &CodeLensStyle) -> Option<String> {
    let titles: Vec<&str> = lenses
        .iter()
        .filter_map(|l| l.command.as_ref().map(|c| c.title.as_str()))
        .collect();
    if titles.is_empty() {
        return None;
    }
    let mut out = String::new();
    out.push_str(&style.prefix);
    out.push_str(&titles.join(&style.separator));
    out.push_str(&style.suffix);
    Some(out)
}

/// Group lenses by their start line, returning `(line, lenses)` pairs
/// sorted by line number.
pub fn group_lenses_by_line(lenses: &[CodeLens]) -> Vec<(u32, Vec<&CodeLens>)> {
    let mut map: std::collections::BTreeMap<u32, Vec<&CodeLens>> =
        std::collections::BTreeMap::new();
    for lens in lenses {
        map.entry(lens.start_line).or_default().push(lens);
    }
    map.into_iter().collect()
}

// ---------------------------------------------------------------------------
// CodeLensCommand – predefined command types
// ---------------------------------------------------------------------------

/// Predefined command types for common code lens actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeLensCommand {
    ShowReferences { count: u32 },
    RunTest { test_name: String },
    ShowImplementations { count: u32 },
    Debug { target: String },
    Custom { command_id: String, title: String },
}

impl CodeLensCommand {
    /// Convert this predefined command into a [`Command`].
    pub fn to_command(&self) -> Command {
        match self {
            Self::ShowReferences { count } => Command {
                title: format!("{count} reference{}", if *count == 1 { "" } else { "s" }),
                command_id: "editor.showReferences".into(),
                tooltip: "Show all references".into(),
                arguments: vec![],
            },
            Self::RunTest { test_name } => Command {
                title: format!("▶ Run Test: {test_name}"),
                command_id: "test.run".into(),
                tooltip: format!("Run test '{test_name}'"),
                arguments: vec![test_name.clone()],
            },
            Self::ShowImplementations { count } => Command {
                title: format!("{count} implementation{}", if *count == 1 { "" } else { "s" }),
                command_id: "editor.showImplementations".into(),
                tooltip: "Show all implementations".into(),
                arguments: vec![],
            },
            Self::Debug { target } => Command {
                title: format!("⏵ Debug: {target}"),
                command_id: "debug.start".into(),
                tooltip: format!("Start debugging '{target}'"),
                arguments: vec![target.clone()],
            },
            Self::Custom { command_id, title } => Command {
                title: title.clone(),
                command_id: command_id.clone(),
                tooltip: title.clone(),
                arguments: vec![],
            },
        }
    }
}

// ---------------------------------------------------------------------------
// codelens_group_adjacent
// ---------------------------------------------------------------------------

/// Group lenses that are within `max_gap` lines of each other.
///
/// Lenses are sorted by `start_line` first. Adjacent lenses whose start lines
/// differ by at most `max_gap` are placed in the same group.
pub fn codelens_group_adjacent(lenses: &[CodeLens], max_gap: u32) -> Vec<Vec<&CodeLens>> {
    let mut sorted: Vec<&CodeLens> = lenses.iter().collect();
    sorted.sort_by_key(|l| l.start_line);

    let mut groups: Vec<Vec<&CodeLens>> = Vec::new();
    for lens in sorted {
        let start_new = match groups.last() {
            Some(group) => {
                let last_line = group.last().unwrap().start_line;
                lens.start_line.saturating_sub(last_line) > max_gap
            }
            None => true,
        };
        if start_new {
            groups.push(vec![lens]);
        } else {
            groups.last_mut().unwrap().push(lens);
        }
    }
    groups
}

// ---------------------------------------------------------------------------
// CodeLensFilter
// ---------------------------------------------------------------------------

/// Filter criteria for selecting a subset of code lenses.
#[derive(Debug, Clone, Default)]
pub struct CodeLensFilter {
    pub resolved_only: bool,
    pub command_ids: Option<Vec<String>>,
    pub line_range: Option<(u32, u32)>,
}

impl CodeLensFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn only_resolved(mut self) -> Self {
        self.resolved_only = true;
        self
    }

    pub fn with_command_ids(mut self, ids: Vec<String>) -> Self {
        self.command_ids = Some(ids);
        self
    }

    pub fn with_line_range(mut self, start: u32, end: u32) -> Self {
        self.line_range = Some((start, end));
        self
    }

    /// Apply this filter to a slice of lenses, returning references to those
    /// that match all criteria.
    pub fn apply<'a>(&self, lenses: &'a [CodeLens]) -> Vec<&'a CodeLens> {
        lenses
            .iter()
            .filter(|lens| {
                if self.resolved_only && !lens.is_resolved() {
                    return false;
                }
                if let Some(ids) = &self.command_ids {
                    match &lens.command {
                        Some(cmd) => {
                            if !ids.contains(&cmd.command_id) {
                                return false;
                            }
                        }
                        None => return false,
                    }
                }
                if let Some((start, end)) = self.line_range {
                    if lens.start_line < start || lens.start_line > end {
                        return false;
                    }
                }
                true
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Lens resolution caching
// ---------------------------------------------------------------------------

/// A simple cache for resolved code lens commands, keyed by `(uri, data)`.
pub struct LensCache {
    entries: std::collections::HashMap<(String, String), Command>,
}

impl LensCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    /// Store a resolved command for the given URI and lens data.
    pub fn insert(&mut self, uri: &str, data: &str, command: Command) {
        self.entries.insert((uri.to_string(), data.to_string()), command);
    }

    /// Look up a cached resolution.
    pub fn get(&self, uri: &str, data: &str) -> Option<&Command> {
        self.entries.get(&(uri.to_string(), data.to_string()))
    }

    /// Remove all entries for a given URI (e.g., when the document changes).
    pub fn invalidate_uri(&mut self, uri: &str) {
        self.entries.retain(|(u, _), _| u != uri);
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for LensCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Lens visibility management
// ---------------------------------------------------------------------------

/// Manages which lines have their code lenses visible or collapsed.
pub struct LensVisibility {
    /// Lines where lenses are hidden (collapsed).
    hidden_lines: std::collections::HashSet<u32>,
    /// Global visibility toggle.
    pub enabled: bool,
}

impl LensVisibility {
    /// Create with all lenses visible.
    pub fn new() -> Self {
        Self {
            hidden_lines: std::collections::HashSet::new(),
            enabled: true,
        }
    }

    /// Hide lenses on a specific line.
    pub fn hide_line(&mut self, line: u32) {
        self.hidden_lines.insert(line);
    }

    /// Show lenses on a specific line.
    pub fn show_line(&mut self, line: u32) {
        self.hidden_lines.remove(&line);
    }

    /// Toggle visibility of lenses on a specific line.
    pub fn toggle_line(&mut self, line: u32) {
        if self.hidden_lines.contains(&line) {
            self.hidden_lines.remove(&line);
        } else {
            self.hidden_lines.insert(line);
        }
    }

    /// Check if lenses on a given line should be displayed.
    pub fn is_visible(&self, line: u32) -> bool {
        self.enabled && !self.hidden_lines.contains(&line)
    }

    /// Filter lenses, keeping only those on visible lines.
    pub fn filter_visible<'a>(&self, lenses: &'a [CodeLens]) -> Vec<&'a CodeLens> {
        lenses.iter().filter(|l| self.is_visible(l.start_line)).collect()
    }
}

impl Default for LensVisibility {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Lens click handling
// ---------------------------------------------------------------------------

/// Represents a click event on a code lens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LensClickEvent {
    pub line: u32,
    pub command_id: String,
    pub arguments: Vec<String>,
}

/// Find the code lens at a given line and resolve its click event.
/// Returns `None` if no resolved lens exists at that line.
pub fn resolve_click(lenses: &[CodeLens], line: u32) -> Option<LensClickEvent> {
    lenses
        .iter()
        .find(|l| l.start_line == line && l.is_resolved())
        .and_then(|l| {
            l.command.as_ref().map(|cmd| LensClickEvent {
                line,
                command_id: cmd.command_id.clone(),
                arguments: cmd.arguments.clone(),
            })
        })
}

// ---------------------------------------------------------------------------
// CodeLens analysis and filtering utilities
// ---------------------------------------------------------------------------

/// Count how many lenses in a slice are resolved (have a command).
pub fn count_resolved(lenses: &[CodeLens]) -> usize {
    lenses.iter().filter(|l| l.is_resolved()).count()
}

/// Count how many lenses are unresolved (no command).
pub fn count_unresolved(lenses: &[CodeLens]) -> usize {
    lenses.iter().filter(|l| !l.is_resolved()).count()
}

/// Return the distinct lines that have at least one code lens.
pub fn distinct_lens_lines(lenses: &[CodeLens]) -> Vec<u32> {
    let mut lines: Vec<u32> = lenses.iter().map(|l| l.start_line).collect();
    lines.sort();
    lines.dedup();
    lines
}

/// Find all lenses whose command title contains a given substring.
pub fn find_lenses_by_title<'a>(lenses: &'a [CodeLens], substring: &'a str) -> Vec<&'a CodeLens> {
    lenses
        .iter()
        .filter(|l| {
            l.command
                .as_ref()
                .map_or(false, |c| c.title.contains(substring))
        })
        .collect()
}

/// Find all lenses whose command_id matches exactly.
pub fn find_lenses_by_command_id<'a>(lenses: &'a [CodeLens], command_id: &str) -> Vec<&'a CodeLens> {
    lenses
        .iter()
        .filter(|l| {
            l.command
                .as_ref()
                .map_or(false, |c| c.command_id == command_id)
        })
        .collect()
}

/// Compute the total line span covered by all lenses (non-overlapping union).
pub fn total_line_coverage(lenses: &[CodeLens]) -> u32 {
    if lenses.is_empty() {
        return 0;
    }
    let mut intervals: Vec<(u32, u32)> = lenses
        .iter()
        .map(|l| (l.start_line, l.start_line + l.line_span()))
        .collect();
    intervals.sort();
    let mut total = 0u32;
    let mut current_end = 0u32;
    for (start, end) in intervals {
        if start >= current_end {
            total += end - start;
            current_end = end;
        } else if end > current_end {
            total += end - current_end;
            current_end = end;
        }
    }
    total
}

/// Check if any lens in the set has non-empty data attached.
pub fn any_has_data(lenses: &[CodeLens]) -> bool {
    lenses.iter().any(|l| !l.data.is_empty())
}

// ---------------------------------------------------------------------------
// CodeLens analysis utilities
// ---------------------------------------------------------------------------

/// Count the number of resolved lenses (those with a command attached).
pub fn resolved_count(lenses: &[CodeLens]) -> usize {
    lenses.iter().filter(|l| l.is_resolved()).count()
}

/// Count the number of unresolved lenses.
pub fn unresolved_count(lenses: &[CodeLens]) -> usize {
    lenses.iter().filter(|l| !l.is_resolved()).count()
}

/// Group lenses by their start line, returning a mapping from line number
/// to the lenses on that line.
pub fn group_by_line(lenses: &[CodeLens]) -> std::collections::HashMap<u32, Vec<&CodeLens>> {
    let mut map: std::collections::HashMap<u32, Vec<&CodeLens>> = std::collections::HashMap::new();
    for lens in lenses {
        map.entry(lens.start_line).or_default().push(lens);
    }
    map
}

/// Return all unique start lines that have at least one code lens.
pub fn lens_lines(lenses: &[CodeLens]) -> Vec<u32> {
    let mut lines: Vec<u32> = lenses.iter().map(|l| l.start_line).collect();
    lines.sort_unstable();
    lines.dedup();
    lines
}

/// Merge two lens sets, deduplicating by (start_line, start_col, end_line, end_col).
pub fn merge_lenses(a: &[CodeLens], b: &[CodeLens]) -> Vec<CodeLens> {
    let mut result: Vec<CodeLens> = a.to_vec();
    for lens in b {
        let dup = result.iter().any(|existing| {
            existing.start_line == lens.start_line
                && existing.start_col == lens.start_col
                && existing.end_line == lens.end_line
                && existing.end_col == lens.end_col
        });
        if !dup {
            result.push(lens.clone());
        }
    }
    result
}

/// Return the total line span covered by all lenses (max end_line - min start_line).
pub fn total_span(lenses: &[CodeLens]) -> u32 {
    if lenses.is_empty() {
        return 0;
    }
    let min_start = lenses.iter().map(|l| l.start_line).min().unwrap();
    let max_end = lenses.iter().map(|l| l.end_line).max().unwrap();
    max_end.saturating_sub(min_start)
}

/// Filter lenses to only those whose command_id matches a given pattern.
pub fn filter_by_command<'a>(lenses: &'a [CodeLens], command_id: &str) -> Vec<&'a CodeLens> {
    lenses
        .iter()
        .filter(|l| {
            l.command
                .as_ref()
                .map(|c| c.command_id == command_id)
                .unwrap_or(false)
        })
        .collect()
}

/// Format a code lens as a display string using a given style.
pub fn format_lens(lens: &CodeLens, style: &CodeLensStyle) -> String {
    if let Some(cmd) = &lens.command {
        format!("{}{}{}", style.prefix, cmd.title, style.suffix)
    } else {
        format!("{}(unresolved){}", style.prefix, style.suffix)
    }
}

/// Format all lenses on a single line into a combined display string.
pub fn format_line_lenses(lenses: &[&CodeLens], style: &CodeLensStyle) -> String {
    lenses
        .iter()
        .map(|l| format_lens(l, style))
        .collect::<Vec<_>>()
        .join(&style.separator)
}

// ---------------------------------------------------------------------------
// CodeLensResolveQueue – priority-based resolve ordering
// ---------------------------------------------------------------------------

/// A priority queue entry for lens resolution scheduling.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolveEntry {
    priority: u32,
    lens_index: usize,
}

/// Priority queue that schedules code lens resolution by priority.
///
/// Higher-priority lenses (e.g. those currently in the viewport) are resolved
/// first. Internally maintains a sorted list of `(priority, lens_index)` pairs.
#[derive(Debug, Clone)]
pub struct CodeLensResolveQueue {
    entries: Vec<ResolveEntry>,
}

impl CodeLensResolveQueue {
    /// Create an empty resolve queue.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Enqueue a lens index with the given priority. Higher values are
    /// dequeued first.
    pub fn push(&mut self, priority: u32, lens_index: usize) {
        self.entries.push(ResolveEntry {
            priority,
            lens_index,
        });
        // Keep sorted in descending priority order.
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Remove and return the lens index with the highest priority.
    pub fn pop(&mut self) -> Option<usize> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.entries.remove(0).lens_index)
        }
    }

    /// Peek at the highest-priority lens index without removing it.
    pub fn peek(&self) -> Option<usize> {
        self.entries.first().map(|e| e.lens_index)
    }

    /// Return the number of pending entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove all entries for a specific lens index.
    pub fn remove_index(&mut self, lens_index: usize) {
        self.entries.retain(|e| e.lens_index != lens_index);
    }

    /// Clear all pending entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Drain all entries in priority order, returning lens indices.
    pub fn drain_all(&mut self) -> Vec<usize> {
        let indices: Vec<usize> = self.entries.iter().map(|e| e.lens_index).collect();
        self.entries.clear();
        indices
    }
}

impl Default for CodeLensResolveQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CodeLensCommandExecutor – execute commands with undo support
// ---------------------------------------------------------------------------

/// Record of a single command execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRecord {
    pub command: Command,
    pub line: u32,
}

/// Executes code lens commands and keeps a history for undo support.
#[derive(Debug, Clone)]
pub struct CodeLensCommandExecutor {
    history: Vec<ExecutionRecord>,
}

impl CodeLensCommandExecutor {
    /// Create a new executor with empty history.
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    /// Execute a command from a code lens on a given line, recording it in
    /// history.  Returns the command that was executed.
    pub fn execute(&mut self, lens: &CodeLens) -> Result<&Command, CodeLensError> {
        let cmd = lens
            .command
            .as_ref()
            .ok_or(CodeLensError::UnresolvedLens {
                data: lens.data.clone(),
            })?;
        self.history.push(ExecutionRecord {
            command: cmd.clone(),
            line: lens.start_line,
        });
        Ok(&self.history.last().unwrap().command)
    }

    /// Undo the most recent command execution, returning the record that was
    /// undone, or `None` if history is empty.
    pub fn undo(&mut self) -> Option<ExecutionRecord> {
        self.history.pop()
    }

    /// Return a reference to the most recently executed record.
    pub fn last_execution(&self) -> Option<&ExecutionRecord> {
        self.history.last()
    }

    /// Return the number of commands in the history.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Return `true` if no commands have been executed.
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// Clear the entire execution history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Return all execution records in chronological order.
    pub fn history(&self) -> &[ExecutionRecord] {
        &self.history
    }
}

impl Default for CodeLensCommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CodeLensDisplayFilter – filter lenses for display
// ---------------------------------------------------------------------------

/// Filter that determines which code lenses to display based on multiple
/// criteria: type name, provider name, and resolved status.
#[derive(Debug, Clone)]
pub struct CodeLensDisplayFilter {
    /// If set, only show lenses whose `data` field starts with one of these.
    pub type_prefixes: Vec<String>,
    /// If `true`, only resolved lenses are shown.
    pub resolved_only: bool,
    /// If set, hide lenses on these specific lines.
    pub excluded_lines: Vec<u32>,
}

impl CodeLensDisplayFilter {
    /// Create a permissive filter that shows everything.
    pub fn new() -> Self {
        Self {
            type_prefixes: Vec::new(),
            resolved_only: false,
            excluded_lines: Vec::new(),
        }
    }

    /// Restrict display to lenses whose `data` starts with one of the given
    /// prefixes.
    pub fn with_type_prefixes(mut self, prefixes: Vec<String>) -> Self {
        self.type_prefixes = prefixes;
        self
    }

    /// Only display resolved lenses.
    pub fn only_resolved(mut self) -> Self {
        self.resolved_only = true;
        self
    }

    /// Exclude lenses on specific lines from display.
    pub fn exclude_lines(mut self, lines: Vec<u32>) -> Self {
        self.excluded_lines = lines;
        self
    }

    /// Test whether a single lens passes this filter.
    pub fn matches(&self, lens: &CodeLens) -> bool {
        if self.resolved_only && !lens.is_resolved() {
            return false;
        }
        if self.excluded_lines.contains(&lens.start_line) {
            return false;
        }
        if !self.type_prefixes.is_empty() {
            return self
                .type_prefixes
                .iter()
                .any(|p| lens.data.starts_with(p));
        }
        true
    }

    /// Apply the filter to a slice, returning only matching lenses.
    pub fn apply<'a>(&self, lenses: &'a [CodeLens]) -> Vec<&'a CodeLens> {
        lenses.iter().filter(|l| self.matches(l)).collect()
    }
}

impl Default for CodeLensDisplayFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// merge_adjacent_lenses – combine same-line lenses into display units
// ---------------------------------------------------------------------------

/// A merged display unit combining all lenses that share the same start line.
#[derive(Debug, Clone)]
pub struct MergedLensGroup {
    /// The common start line for every lens in this group.
    pub line: u32,
    /// The lenses that were merged into this group.
    pub lenses: Vec<CodeLens>,
}

impl MergedLensGroup {
    /// Return the number of lenses in this group.
    pub fn count(&self) -> usize {
        self.lenses.len()
    }

    /// Return `true` if every lens in the group is resolved.
    pub fn all_resolved(&self) -> bool {
        self.lenses.iter().all(|l| l.is_resolved())
    }

    /// Collect all command titles from resolved lenses.
    pub fn titles(&self) -> Vec<&str> {
        self.lenses
            .iter()
            .filter_map(|l| l.command.as_ref().map(|c| c.title.as_str()))
            .collect()
    }
}

/// Merge code lenses that share the same start line into
/// [`MergedLensGroup`]s, sorted by line number.
pub fn merge_adjacent_lenses(lenses: &[CodeLens]) -> Vec<MergedLensGroup> {
    let mut map: std::collections::BTreeMap<u32, Vec<CodeLens>> =
        std::collections::BTreeMap::new();
    for lens in lenses {
        map.entry(lens.start_line).or_default().push(lens.clone());
    }
    map.into_iter()
        .map(|(line, lenses)| MergedLensGroup { line, lenses })
        .collect()
}

// ---------------------------------------------------------------------------
// CodeLensClickHandler - code lens click handler
// ---------------------------------------------------------------------------

/// Severity level for code lens click handler issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CodeLensClickHandlerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for CodeLensClickHandlerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [CodeLensClickHandler].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLensClickHandlerEntry {
    pub id: String,
    pub label: String,
    pub severity: CodeLensClickHandlerSeverity,
    pub detail: Option<String>,
    pub lens_count: usize,
    enabled: bool,
}

impl CodeLensClickHandlerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: CodeLensClickHandlerSeverity::Low,
            detail: None,
            lens_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: CodeLensClickHandlerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_lens_count(mut self, val: usize) -> Self {
        self.lens_count = val;
        self
    }

    pub fn is_clickable(&self) -> bool {
        self.enabled && self.severity >= CodeLensClickHandlerSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.lens_count, det)
    }
}

impl fmt::Display for CodeLensClickHandlerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [CodeLensClickHandlerEntry] items.
#[derive(Debug, Clone)]
pub struct CodeLensClickHandler {
    entries: Vec<CodeLensClickHandlerEntry>,
    name: String,
    capacity: usize,
}

impl CodeLensClickHandler {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: CodeLensClickHandlerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<CodeLensClickHandlerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&CodeLensClickHandlerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn lens_count(&self) -> usize { self.entries.len() }

    pub fn is_clickable(&self) -> bool {
        self.entries.iter().any(|e| e.is_clickable())
    }

    pub fn entries_by_severity(&self, severity: CodeLensClickHandlerSeverity) -> Vec<&CodeLensClickHandlerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= CodeLensClickHandlerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&CodeLensClickHandlerEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&CodeLensClickHandlerEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// CodeLensCacheInvalidator - code lens cache invalidator
// ---------------------------------------------------------------------------

/// Configuration for [CodeLensCacheInvalidator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLensCacheInvalidatorConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub cache_size: usize,
}

impl CodeLensCacheInvalidatorConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, cache_size: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_cache_size(mut self, val: usize) -> Self { self.cache_size = val; self }
}

impl Default for CodeLensCacheInvalidatorConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [CodeLensCacheInvalidator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLensCacheInvalidatorItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl CodeLensCacheInvalidatorItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn is_cache_valid(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for CodeLensCacheInvalidatorItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [CodeLensCacheInvalidatorItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct CodeLensCacheInvalidator {
    config: CodeLensCacheInvalidatorConfig,
    items: Vec<CodeLensCacheInvalidatorItem>,
}

impl CodeLensCacheInvalidator {
    pub fn new(config: CodeLensCacheInvalidatorConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: CodeLensCacheInvalidatorItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<CodeLensCacheInvalidatorItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&CodeLensCacheInvalidatorItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn cache_size(&self) -> usize { self.items.len() }

    pub fn is_cache_valid(&self) -> bool {
        self.items.iter().any(|i| i.is_cache_valid())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&CodeLensCacheInvalidatorItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&CodeLensCacheInvalidatorItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &CodeLensCacheInvalidatorConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



/// Configuration manager for codelens functionality.
pub struct CodelensConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl CodelensConfig {
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

    pub fn merge(&mut self, other: &CodelensConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for codelens operations.
pub struct CodelensRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl CodelensRateTracker {
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

/// Validation result collector for codelens.
pub struct CodelensValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl CodelensValidationCollector {
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

    pub fn merge(&mut self, other: &CodelensValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Code lens provider and resolution — extended utilities (ya)
// ---------------------------------------------------------------------------

/// Metric accumulator for codelens operations.
#[derive(Debug, Clone)]
pub struct YaMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YaMetrics {
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

/// Sliding-window rate counter for codelens.
#[derive(Debug, Clone)]
pub struct YaRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YaRateWindow {
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

/// A small LRU-style cache for codelens lookups.
#[derive(Debug, Clone)]
pub struct YaLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YaLruCache {
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
// xa_ extended helpers for codelens
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaCodelensRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaCodelensRingBuf {
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
pub struct XaCodelensCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaCodelensCounter {
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

impl Default for XaCodelensCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 16
// ---------------------------------------------------------------------------

/// Generic object pool `Xc16Pool<T>`.
pub struct Xc16Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc16Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc16PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc16Pool<T> {
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
    pub fn stats(&self) -> Xc16PoolStats {
        Xc16PoolStats {
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

impl<T> Default for Xc16Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc16Scheduler`.
pub struct Xc16Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc16Scheduler {
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

impl Default for Xc16Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_16 hash for the given byte slice.
pub fn xc_16_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_16 convention.
pub fn xc_16_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_52 deepening: state machine + event bus ---

/// States for the Xd52 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd52State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd52State {
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
pub struct Xd52Transition {
    pub from: Xd52State,
    pub to: Xd52State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd52StateMachine {
    current: Xd52State,
    history: Vec<Xd52Transition>,
    step_counter: usize,
}

impl Xd52StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd52State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd52State {
        self.current
    }

    pub fn history(&self) -> &[Xd52Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd52State) -> Result<Xd52State, String> {
        let allowed = match (self.current, target) {
            (Xd52State::Idle, Xd52State::Running) => true,
            (Xd52State::Running, Xd52State::Paused) => true,
            (Xd52State::Running, Xd52State::Done) => true,
            (Xd52State::Paused, Xd52State::Running) => true,
            (Xd52State::Paused, Xd52State::Done) => true,
            (Xd52State::Done, Xd52State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_52: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd52Transition {
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
            "Xd52SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd52State> {
        let prefix = "Xd52SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd52State::Idle),
            "Running" => Some(Xd52State::Running),
            "Paused" => Some(Xd52State::Paused),
            "Done" => Some(Xd52State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd52State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd52 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd52Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd52Event {
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

type Xd52HandlerFn = Box<dyn Fn(&Xd52Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd52EventBus {
    handlers: Vec<(usize, Option<String>, Xd52HandlerFn)>,
    next_id: usize,
    published: Vec<Xd52Event>,
}

impl Xd52EventBus {
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
        F: Fn(&Xd52Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd52Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd52Event) {
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

    pub fn published_events(&self) -> &[Xd52Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #50
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf50Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf50TrieNode {
    children: std::collections::HashMap<char, Xf50TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf50Trie {
    root: Xf50TrieNode,
    count: usize,
}

impl Xf50Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf50TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf50TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf50TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf50BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf50BloomFilter {
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

    // -- Test providers -----------------------------------------------------

    struct ReferenceCountProvider;

    impl CodeLensProvider for ReferenceCountProvider {
        fn provide_code_lenses(&self, _uri: &str) -> Vec<CodeLens> {
            vec![
                CodeLens {
                    data: "ref_count".into(),
                    ..CodeLens::new(0, 0, 0, 10)
                },
                CodeLens {
                    data: "ref_count".into(),
                    ..CodeLens::new(5, 0, 5, 15)
                },
            ]
        }

        fn resolve_code_lens(&self, mut lens: CodeLens) -> CodeLens {
            if lens.data == "ref_count" {
                lens.command = Some(Command {
                    title: "3 references".into(),
                    command_id: "editor.showReferences".into(),
                    tooltip: "Show all references".into(),
                    arguments: vec![],
                });
            }
            lens
        }
    }

    struct RunTestProvider;

    impl CodeLensProvider for RunTestProvider {
        fn provide_code_lenses(&self, uri: &str) -> Vec<CodeLens> {
            if uri.ends_with("_test.rs") {
                vec![CodeLens {
                    command: Some(Command {
                        title: "▶ Run Test".into(),
                        command_id: "test.run".into(),
                        tooltip: "Run this test".into(),
                        arguments: vec![uri.to_string()],
                    }),
                    ..CodeLens::new(1, 0, 1, 20)
                }]
            } else {
                vec![]
            }
        }
    }

    // -- Tests --------------------------------------------------------------

    #[test]
    fn code_lens_new_is_unresolved() {
        let lens = CodeLens::new(10, 0, 10, 25);
        assert!(!lens.is_resolved());
        assert_eq!(lens.start_line, 10);
        assert_eq!(lens.end_col, 25);
        assert!(lens.data.is_empty());
    }

    #[test]
    fn service_collects_from_multiple_providers() {
        let mut service = CodeLensService::new();
        service.register(ReferenceCountProvider);
        service.register(RunTestProvider);

        assert_eq!(service.provider_count(), 2);

        let lenses = service.provide_code_lenses("main_test.rs");
        assert_eq!(lenses.len(), 3);
    }

    #[test]
    fn resolve_fills_in_command() {
        let mut service = CodeLensService::new();
        service.register(ReferenceCountProvider);

        let lenses = service.provide_code_lenses("main.rs");
        assert!(lenses.iter().all(|l| !l.is_resolved()));

        let resolved = service.resolve_all(lenses);
        assert!(resolved.iter().all(|l| l.is_resolved()));
        assert_eq!(resolved[0].command.as_ref().unwrap().title, "3 references");
    }

    #[test]
    fn provider_filters_by_uri() {
        let mut service = CodeLensService::new();
        service.register(RunTestProvider);

        let test_lenses = service.provide_code_lenses("foo_test.rs");
        assert_eq!(test_lenses.len(), 1);
        assert!(test_lenses[0].is_resolved());

        let src_lenses = service.provide_code_lenses("foo.rs");
        assert!(src_lenses.is_empty());
    }

    #[test]
    fn empty_service_returns_no_lenses() {
        let service = CodeLensService::default();
        assert_eq!(service.provider_count(), 0);
        assert!(service.provide_code_lenses("any.rs").is_empty());
    }

    #[test]
    fn resolve_skips_already_resolved() {
        let mut service = CodeLensService::new();
        service.register(RunTestProvider);

        let lenses = service.provide_code_lenses("x_test.rs");
        assert!(lenses[0].is_resolved());
        let original_title = lenses[0].command.as_ref().unwrap().title.clone();

        let resolved = service.resolve_all(lenses);
        assert_eq!(resolved[0].command.as_ref().unwrap().title, original_title);
    }

    // -- Additional tests ---------------------------------------------------

    #[test]
    fn try_new_validates_range() {
        assert!(CodeLens::try_new(0, 0, 5, 10).is_ok());
        assert!(CodeLens::try_new(5, 0, 5, 10).is_ok());
        assert!(CodeLens::try_new(5, 10, 5, 10).is_ok());

        let err = CodeLens::try_new(5, 11, 5, 10).unwrap_err();
        assert_eq!(
            err,
            CodeLensError::InvalidRange {
                start_line: 5,
                start_col: 11,
                end_line: 5,
                end_col: 10,
            }
        );

        let err2 = CodeLens::try_new(10, 0, 5, 0).unwrap_err();
        assert!(matches!(err2, CodeLensError::InvalidRange { .. }));
    }

    #[test]
    fn code_lens_display() {
        let lens = CodeLens::new(1, 0, 3, 20);
        assert_eq!(format!("{lens}"), "CodeLens(1:0-3:20, unresolved)");

        let resolved = lens.with_command(Command {
            title: "t".into(),
            command_id: "c".into(),
            tooltip: String::new(),
            arguments: vec![],
        });
        assert!(format!("{resolved}").contains("resolved"));
    }

    #[test]
    fn command_display() {
        let cmd = Command {
            title: "Run".into(),
            command_id: "test.run".into(),
            tooltip: String::new(),
            arguments: vec![],
        };
        assert_eq!(format!("{cmd}"), "[test.run] Run");
    }

    #[test]
    fn command_builder_success() {
        let cmd = CommandBuilder::new()
            .title("5 references")
            .command_id("editor.showReferences")
            .tooltip("Show all references")
            .argument("file.rs")
            .argument("line:10")
            .build()
            .unwrap();

        assert_eq!(cmd.title, "5 references");
        assert_eq!(cmd.command_id, "editor.showReferences");
        assert_eq!(cmd.tooltip, "Show all references");
        assert_eq!(cmd.arguments, vec!["file.rs", "line:10"]);
    }

    #[test]
    fn command_builder_missing_title() {
        let result = CommandBuilder::new()
            .command_id("test.run")
            .build();
        assert_eq!(result.unwrap_err(), CodeLensError::MissingField("title"));
    }

    #[test]
    fn command_builder_missing_command_id() {
        let result = CommandBuilder::new()
            .title("Run")
            .build();
        assert_eq!(
            result.unwrap_err(),
            CodeLensError::MissingField("command_id")
        );
    }

    #[test]
    fn code_lens_with_data_and_command() {
        let lens = CodeLens::new(0, 0, 0, 5)
            .with_data("my_provider")
            .with_command(Command {
                title: "Go".into(),
                command_id: "go".into(),
                tooltip: String::new(),
                arguments: vec![],
            });
        assert!(lens.is_resolved());
        assert_eq!(lens.data, "my_provider");
    }

    #[test]
    fn single_line_and_line_span() {
        let single = CodeLens::new(3, 0, 3, 10);
        assert!(single.is_single_line());
        assert_eq!(single.line_span(), 1);

        let multi = CodeLens::new(3, 0, 7, 10);
        assert!(!multi.is_single_line());
        assert_eq!(multi.line_span(), 5);
    }

    #[test]
    fn contains_line() {
        let lens = CodeLens::new(5, 0, 10, 0);
        assert!(!lens.contains_line(4));
        assert!(lens.contains_line(5));
        assert!(lens.contains_line(7));
        assert!(lens.contains_line(10));
        assert!(!lens.contains_line(11));
    }

    #[test]
    fn lenses_at_line_filters_correctly() {
        let mut service = CodeLensService::new();
        service.register(ReferenceCountProvider);

        let at_0 = service.lenses_at_line("main.rs", 0);
        assert_eq!(at_0.len(), 1);
        assert_eq!(at_0[0].start_line, 0);

        let at_5 = service.lenses_at_line("main.rs", 5);
        assert_eq!(at_5.len(), 1);
        assert_eq!(at_5[0].start_line, 5);

        let at_99 = service.lenses_at_line("main.rs", 99);
        assert!(at_99.is_empty());
    }

    #[test]
    fn provide_and_resolve_convenience() {
        let mut service = CodeLensService::new();
        service.register(ReferenceCountProvider);

        let lenses = service.provide_and_resolve("main.rs");
        assert_eq!(lenses.len(), 2);
        assert!(lenses.iter().all(|l| l.is_resolved()));
    }

    #[test]
    fn resolve_all_strict_returns_error_for_unresolved() {
        struct NoopProvider;
        impl CodeLensProvider for NoopProvider {
            fn provide_code_lenses(&self, _uri: &str) -> Vec<CodeLens> {
                vec![CodeLens::new(0, 0, 0, 1).with_data("noop")]
            }
        }

        let mut service = CodeLensService::new();
        service.register(NoopProvider);

        let lenses = service.provide_code_lenses("file.rs");
        let err = service.resolve_all_strict(lenses).unwrap_err();
        assert_eq!(
            err,
            CodeLensError::UnresolvedLens {
                data: "noop".into()
            }
        );
    }

    #[test]
    fn service_debug_impl() {
        let service = CodeLensService::new();
        let dbg = format!("{service:?}");
        assert!(dbg.contains("CodeLensService"));
        assert!(dbg.contains("provider_count"));
    }

    #[test]
    fn error_display_messages() {
        let e1 = CodeLensError::InvalidRange {
            start_line: 5,
            start_col: 3,
            end_line: 2,
            end_col: 1,
        };
        assert!(format!("{e1}").contains("invalid range"));

        let e2 = CodeLensError::MissingField("title");
        assert!(format!("{e2}").contains("title"));

        let e3 = CodeLensError::UnresolvedLens {
            data: "xyz".into(),
        };
        assert!(format!("{e3}").contains("xyz"));
    }

    #[test]
    fn codelens_stats_new_defaults() {
        let stats = CodelensStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn codelens_stats_record_success() {
        let mut stats = CodelensStats::new();
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
    fn codelens_stats_record_failure() {
        let mut stats = CodelensStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn codelens_stats_reset() {
        let mut stats = CodelensStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn codelens_stats_merge() {
        let mut a = CodelensStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = CodelensStats::new();
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
    fn codelens_stats_display() {
        let mut stats = CodelensStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn codelens_stats_default() {
        let stats = CodelensStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn codelens_validator_accepts_and_rejects() {
        let mut v = CodelensValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn codelens_validator_warnings() {
        let mut v = CodelensValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn codelens_validator_clear_and_merge() {
        let mut v = CodelensValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = CodelensValidationCollector::new();
        a.add_error("a_err");
        let mut b = CodelensValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    // -- render tests -------------------------------------------------------

    #[test]
    fn render_code_lens_line_resolved() {
        let lenses = vec![
            CodeLens {
                command: Some(Command {
                    title: "3 references".into(),
                    command_id: "showRefs".into(),
                    tooltip: String::new(),
                    arguments: vec![],
                }),
                ..CodeLens::new(0, 0, 0, 10)
            },
            CodeLens {
                command: Some(Command {
                    title: "Run Test".into(),
                    command_id: "test.run".into(),
                    tooltip: String::new(),
                    arguments: vec![],
                }),
                ..CodeLens::new(0, 0, 0, 10)
            },
        ];
        let result = render_code_lens_line(&lenses, &CodeLensStyle::default()).unwrap();
        assert_eq!(result, "3 references | Run Test");
    }

    #[test]
    fn render_code_lens_line_no_resolved() {
        let lenses = vec![CodeLens::new(0, 0, 0, 10)];
        assert!(render_code_lens_line(&lenses, &CodeLensStyle::default()).is_none());
    }

    #[test]
    fn render_code_lens_line_with_style() {
        let lens = CodeLens {
            command: Some(Command {
                title: "1 impl".into(),
                command_id: "showImpl".into(),
                tooltip: String::new(),
                arguments: vec![],
            }),
            ..CodeLens::new(0, 0, 0, 5)
        };
        let style = CodeLensStyle {
            prefix: "<".into(),
            suffix: ">".into(),
            separator: " | ".into(),
        };
        assert_eq!(render_code_lens_line(&[lens], &style).unwrap(), "<1 impl>");
    }

    #[test]
    fn group_lenses_by_line_groups_correctly() {
        let lenses = vec![
            CodeLens::new(5, 0, 5, 10),
            CodeLens::new(1, 0, 1, 10),
            CodeLens::new(5, 0, 5, 20),
        ];
        let groups = group_lenses_by_line(&lenses);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, 1);
        assert_eq!(groups[0].1.len(), 1);
        assert_eq!(groups[1].0, 5);
        assert_eq!(groups[1].1.len(), 2);
    }

    #[test]
    fn test_codelens_command_show_references() {
        let cmd = CodeLensCommand::ShowReferences { count: 5 };
        let command = cmd.to_command();
        assert_eq!(command.title, "5 references");
        assert_eq!(command.command_id, "editor.showReferences");
        assert_eq!(command.tooltip, "Show all references");

        let single = CodeLensCommand::ShowReferences { count: 1 };
        assert_eq!(single.to_command().title, "1 reference");
    }

    #[test]
    fn test_codelens_command_run_test() {
        let cmd = CodeLensCommand::RunTest {
            test_name: "my_test".into(),
        };
        let command = cmd.to_command();
        assert_eq!(command.title, "▶ Run Test: my_test");
        assert_eq!(command.command_id, "test.run");
        assert_eq!(command.tooltip, "Run test 'my_test'");
        assert_eq!(command.arguments, vec!["my_test".to_string()]);
    }

    #[test]
    fn test_codelens_command_custom() {
        let cmd = CodeLensCommand::Custom {
            command_id: "my.cmd".into(),
            title: "Do Thing".into(),
        };
        let command = cmd.to_command();
        assert_eq!(command.title, "Do Thing");
        assert_eq!(command.command_id, "my.cmd");
        assert_eq!(command.tooltip, "Do Thing");
    }

    #[test]
    fn test_group_adjacent_within_gap() {
        let lenses = vec![
            CodeLens::new(1, 0, 1, 10),
            CodeLens::new(2, 0, 2, 10),
            CodeLens::new(3, 0, 3, 10),
        ];
        let groups = codelens_group_adjacent(&lenses, 2);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 3);
    }

    #[test]
    fn test_group_adjacent_separate_groups() {
        let lenses = vec![
            CodeLens::new(1, 0, 1, 10),
            CodeLens::new(2, 0, 2, 10),
            CodeLens::new(10, 0, 10, 10),
            CodeLens::new(11, 0, 11, 10),
        ];
        let groups = codelens_group_adjacent(&lenses, 2);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[1].len(), 2);
        assert_eq!(groups[0][0].start_line, 1);
        assert_eq!(groups[1][0].start_line, 10);
    }

    #[test]
    fn test_filter_resolved_only() {
        let resolved = CodeLens::new(1, 0, 1, 10).with_command(Command {
            title: "test".into(),
            command_id: "cmd".into(),
            tooltip: "tip".into(),
            arguments: vec![],
        });
        let unresolved = CodeLens::new(2, 0, 2, 10);
        let lenses = vec![resolved, unresolved];

        let filter = CodeLensFilter::new().only_resolved();
        let result = filter.apply(&lenses);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start_line, 1);
    }

    #[test]
    fn test_filter_by_line_range() {
        let lenses = vec![
            CodeLens::new(1, 0, 1, 10),
            CodeLens::new(5, 0, 5, 10),
            CodeLens::new(10, 0, 10, 10),
            CodeLens::new(15, 0, 15, 10),
        ];
        let filter = CodeLensFilter::new().with_line_range(4, 11);
        let result = filter.apply(&lenses);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].start_line, 5);
        assert_eq!(result[1].start_line, 10);
    }

    #[test]
    fn lens_cache_insert_and_get() {
        let mut cache = LensCache::new();
        assert!(cache.is_empty());
        let cmd = Command {
            title: "Run".into(),
            command_id: "run".into(),
            tooltip: String::new(),
            arguments: vec![],
        };
        cache.insert("file:///a.rs", "test_data", cmd.clone());
        assert_eq!(cache.len(), 1);
        let found = cache.get("file:///a.rs", "test_data").unwrap();
        assert_eq!(found.title, "Run");
        assert!(cache.get("file:///a.rs", "other").is_none());
    }

    #[test]
    fn lens_cache_invalidate_uri() {
        let mut cache = LensCache::new();
        let cmd = Command {
            title: "T".into(),
            command_id: "c".into(),
            tooltip: String::new(),
            arguments: vec![],
        };
        cache.insert("file:///a.rs", "d1", cmd.clone());
        cache.insert("file:///a.rs", "d2", cmd.clone());
        cache.insert("file:///b.rs", "d1", cmd);
        assert_eq!(cache.len(), 3);
        cache.invalidate_uri("file:///a.rs");
        assert_eq!(cache.len(), 1);
        assert!(cache.get("file:///b.rs", "d1").is_some());
    }

    #[test]
    fn lens_visibility_toggle() {
        let mut vis = LensVisibility::new();
        assert!(vis.is_visible(10));
        vis.hide_line(10);
        assert!(!vis.is_visible(10));
        vis.toggle_line(10);
        assert!(vis.is_visible(10));
    }

    #[test]
    fn lens_visibility_global_disable() {
        let mut vis = LensVisibility::new();
        vis.enabled = false;
        assert!(!vis.is_visible(1));
        assert!(!vis.is_visible(100));
    }

    #[test]
    fn lens_visibility_filter() {
        let mut vis = LensVisibility::new();
        vis.hide_line(5);
        let lenses = vec![
            CodeLens::new(1, 0, 1, 10),
            CodeLens::new(5, 0, 5, 10),
            CodeLens::new(10, 0, 10, 10),
        ];
        let visible = vis.filter_visible(&lenses);
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].start_line, 1);
        assert_eq!(visible[1].start_line, 10);
    }

    #[test]
    fn resolve_click_found() {
        let cmd = Command {
            title: "Run Tests".into(),
            command_id: "test.run".into(),
            tooltip: String::new(),
            arguments: vec!["arg1".into()],
        };
        let lenses = vec![
            CodeLens::new(5, 0, 5, 10).with_command(cmd),
            CodeLens::new(10, 0, 10, 10),
        ];
        let event = resolve_click(&lenses, 5).unwrap();
        assert_eq!(event.command_id, "test.run");
        assert_eq!(event.arguments, vec!["arg1"]);
        assert_eq!(event.line, 5);
    }

    #[test]
    fn resolve_click_unresolved_returns_none() {
        let lenses = vec![CodeLens::new(5, 0, 5, 10)];
        assert!(resolve_click(&lenses, 5).is_none());
    }

    #[test]
    fn resolve_click_wrong_line_returns_none() {
        let cmd = Command {
            title: "T".into(),
            command_id: "c".into(),
            tooltip: String::new(),
            arguments: vec![],
        };
        let lenses = vec![CodeLens::new(5, 0, 5, 10).with_command(cmd)];
        assert!(resolve_click(&lenses, 10).is_none());
    }

    #[test]
    fn count_resolved_mixed() {
        let cmd = Command {
            title: "T".into(),
            command_id: "c".into(),
            tooltip: String::new(),
            arguments: vec![],
        };
        let lenses = vec![
            CodeLens::new(1, 0, 1, 5).with_command(cmd.clone()),
            CodeLens::new(2, 0, 2, 5),
            CodeLens::new(3, 0, 3, 5).with_command(cmd),
        ];
        assert_eq!(count_resolved(&lenses), 2);
        assert_eq!(count_unresolved(&lenses), 1);
    }

    #[test]
    fn distinct_lens_lines_deduplicates() {
        let lenses = vec![
            CodeLens::new(1, 0, 1, 5),
            CodeLens::new(1, 6, 1, 10),
            CodeLens::new(3, 0, 3, 5),
        ];
        assert_eq!(distinct_lens_lines(&lenses), vec![1, 3]);
    }

    #[test]
    fn find_lenses_by_title_matches() {
        let cmd = Command {
            title: "Run Test".into(),
            command_id: "test.run".into(),
            tooltip: String::new(),
            arguments: vec![],
        };
        let lenses = vec![
            CodeLens::new(1, 0, 1, 5).with_command(cmd),
            CodeLens::new(2, 0, 2, 5),
        ];
        let found = find_lenses_by_title(&lenses, "Run");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn find_lenses_by_command_id_matches() {
        let cmd = Command {
            title: "T".into(),
            command_id: "editor.action.run".into(),
            tooltip: String::new(),
            arguments: vec![],
        };
        let lenses = vec![
            CodeLens::new(1, 0, 1, 5).with_command(cmd),
            CodeLens::new(2, 0, 2, 5),
        ];
        let found = find_lenses_by_command_id(&lenses, "editor.action.run");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn total_line_coverage_non_overlapping() {
        let lenses = vec![
            CodeLens::new(1, 0, 3, 5),  // lines 1-3, span=3
            CodeLens::new(5, 0, 7, 5),  // lines 5-7, span=3
        ];
        assert_eq!(total_line_coverage(&lenses), 6);
    }

    #[test]
    fn total_line_coverage_overlapping() {
        let lenses = vec![
            CodeLens::new(1, 0, 3, 5),
            CodeLens::new(2, 0, 4, 5),
        ];
        // union: 1-4, = 4 lines
        assert_eq!(total_line_coverage(&lenses), 4);
    }

    #[test]
    fn total_line_coverage_empty() {
        assert_eq!(total_line_coverage(&[]), 0);
    }

    #[test]
    fn any_has_data_true() {
        let lenses = vec![
            CodeLens::new(1, 0, 1, 5).with_data("info"),
        ];
        assert!(any_has_data(&lenses));
    }

    #[test]
    fn any_has_data_false() {
        let lenses = vec![CodeLens::new(1, 0, 1, 5)];
        assert!(!any_has_data(&lenses));
    }

    // -- resolved_count / unresolved_count -------------------------------------

    #[test]
    fn resolved_count_basic() {
        let mut lens = CodeLens::new(1, 0, 1, 10);
        lens.command = Some(Command { title: "Run".into(), command_id: "test.run".into(), tooltip: String::new(), arguments: vec![] });
        let lenses = vec![lens, CodeLens::new(2, 0, 2, 10)];
        assert_eq!(resolved_count(&lenses), 1);
        assert_eq!(unresolved_count(&lenses), 1);
    }

    // -- group_by_line ---------------------------------------------------------

    #[test]
    fn group_by_line_groups() {
        let lenses = vec![
            CodeLens::new(1, 0, 1, 5),
            CodeLens::new(1, 6, 1, 10),
            CodeLens::new(3, 0, 3, 5),
        ];
        let groups = group_by_line(&lenses);
        assert_eq!(groups[&1].len(), 2);
        assert_eq!(groups[&3].len(), 1);
    }

    // -- lens_lines ------------------------------------------------------------

    #[test]
    fn lens_lines_unique() {
        let lenses = vec![
            CodeLens::new(5, 0, 5, 10),
            CodeLens::new(1, 0, 1, 5),
            CodeLens::new(5, 2, 5, 8),
        ];
        let lines = lens_lines(&lenses);
        assert_eq!(lines, vec![1, 5]);
    }

    // -- merge_lenses ----------------------------------------------------------

    #[test]
    fn merge_lenses_deduplicates() {
        let a = vec![CodeLens::new(1, 0, 1, 5)];
        let b = vec![CodeLens::new(1, 0, 1, 5), CodeLens::new(2, 0, 2, 5)];
        let merged = merge_lenses(&a, &b);
        assert_eq!(merged.len(), 2);
    }

    // -- total_span ------------------------------------------------------------

    #[test]
    fn total_span_computed() {
        let lenses = vec![CodeLens::new(3, 0, 5, 0), CodeLens::new(10, 0, 15, 0)];
        assert_eq!(total_span(&lenses), 12); // 15 - 3
    }

    #[test]
    fn total_span_empty() {
        let lenses: Vec<CodeLens> = vec![];
        assert_eq!(total_span(&lenses), 0);
    }

    // -- format_lens -----------------------------------------------------------

    #[test]
    fn format_lens_resolved() {
        let mut lens = CodeLens::new(1, 0, 1, 10);
        lens.command = Some(Command { title: "5 references".into(), command_id: "show".into(), tooltip: String::new(), arguments: vec![] });
        let style = CodeLensStyle { prefix: "[ ".into(), suffix: " ]".into(), separator: " | ".into() };
        assert_eq!(format_lens(&lens, &style), "[ 5 references ]");
    }

    #[test]
    fn format_lens_unresolved() {
        let lens = CodeLens::new(1, 0, 1, 10);
        let style = CodeLensStyle { prefix: "".into(), suffix: "".into(), separator: " | ".into() };
        assert_eq!(format_lens(&lens, &style), "(unresolved)");
    }

    // -- CodeLensResolveQueue tests -----------------------------------------

    #[test]
    fn resolve_queue_pop_highest_priority_first() {
        let mut q = CodeLensResolveQueue::new();
        q.push(1, 10);
        q.push(5, 20);
        q.push(3, 30);
        assert_eq!(q.pop(), Some(20));
        assert_eq!(q.pop(), Some(30));
        assert_eq!(q.pop(), Some(10));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn resolve_queue_peek_does_not_remove() {
        let mut q = CodeLensResolveQueue::new();
        q.push(10, 42);
        assert_eq!(q.peek(), Some(42));
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn resolve_queue_remove_index() {
        let mut q = CodeLensResolveQueue::new();
        q.push(1, 0);
        q.push(2, 1);
        q.push(3, 2);
        q.remove_index(1);
        assert_eq!(q.len(), 2);
        assert_eq!(q.pop(), Some(2));
        assert_eq!(q.pop(), Some(0));
    }

    #[test]
    fn resolve_queue_drain_all() {
        let mut q = CodeLensResolveQueue::new();
        q.push(1, 100);
        q.push(3, 200);
        q.push(2, 300);
        let all = q.drain_all();
        assert_eq!(all, vec![200, 300, 100]);
        assert!(q.is_empty());
    }

    // -- CodeLensCommandExecutor tests --------------------------------------

    #[test]
    fn executor_execute_resolved_lens() {
        let mut exec = CodeLensCommandExecutor::new();
        let lens = CodeLens::new(5, 0, 5, 10).with_command(Command {
            title: "Run".into(),
            command_id: "run".into(),
            tooltip: String::new(),
            arguments: vec![],
        });
        let cmd = exec.execute(&lens).unwrap();
        assert_eq!(cmd.command_id, "run");
        assert_eq!(exec.history_len(), 1);
    }

    #[test]
    fn executor_execute_unresolved_fails() {
        let mut exec = CodeLensCommandExecutor::new();
        let lens = CodeLens::new(1, 0, 1, 5).with_data("some_data");
        let result = exec.execute(&lens);
        assert!(result.is_err());
        assert_eq!(exec.history_len(), 0);
    }

    #[test]
    fn executor_undo_returns_last() {
        let mut exec = CodeLensCommandExecutor::new();
        let lens_a = CodeLens::new(1, 0, 1, 5).with_command(Command {
            title: "A".into(),
            command_id: "a".into(),
            tooltip: String::new(),
            arguments: vec![],
        });
        let lens_b = CodeLens::new(2, 0, 2, 5).with_command(Command {
            title: "B".into(),
            command_id: "b".into(),
            tooltip: String::new(),
            arguments: vec![],
        });
        exec.execute(&lens_a).unwrap();
        exec.execute(&lens_b).unwrap();
        let undone = exec.undo().unwrap();
        assert_eq!(undone.command.command_id, "b");
        assert_eq!(undone.line, 2);
        assert_eq!(exec.history_len(), 1);
    }

    #[test]
    fn executor_undo_empty_returns_none() {
        let mut exec = CodeLensCommandExecutor::new();
        assert!(exec.undo().is_none());
    }

    // -- CodeLensDisplayFilter tests ----------------------------------------

    #[test]
    fn display_filter_shows_all_by_default() {
        let filter = CodeLensDisplayFilter::new();
        let lenses = vec![
            CodeLens::new(1, 0, 1, 10).with_data("ref"),
            CodeLens::new(2, 0, 2, 10),
        ];
        assert_eq!(filter.apply(&lenses).len(), 2);
    }

    #[test]
    fn display_filter_resolved_only() {
        let filter = CodeLensDisplayFilter::new().only_resolved();
        let lenses = vec![
            CodeLens::new(1, 0, 1, 10).with_command(Command {
                title: "T".into(),
                command_id: "c".into(),
                tooltip: String::new(),
                arguments: vec![],
            }),
            CodeLens::new(2, 0, 2, 10),
        ];
        let result = filter.apply(&lenses);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start_line, 1);
    }

    #[test]
    fn display_filter_type_prefix() {
        let filter = CodeLensDisplayFilter::new()
            .with_type_prefixes(vec!["ref".into()]);
        let lenses = vec![
            CodeLens::new(1, 0, 1, 10).with_data("ref_count"),
            CodeLens::new(2, 0, 2, 10).with_data("test_run"),
        ];
        let result = filter.apply(&lenses);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].data, "ref_count");
    }

    #[test]
    fn display_filter_exclude_lines() {
        let filter = CodeLensDisplayFilter::new().exclude_lines(vec![3, 5]);
        let lenses = vec![
            CodeLens::new(3, 0, 3, 10),
            CodeLens::new(4, 0, 4, 10),
            CodeLens::new(5, 0, 5, 10),
        ];
        let result = filter.apply(&lenses);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start_line, 4);
    }

    // -- merge_adjacent_lenses tests ----------------------------------------

    #[test]
    fn merge_adjacent_groups_by_line() {
        let lenses = vec![
            CodeLens::new(10, 0, 10, 5).with_data("a"),
            CodeLens::new(10, 6, 10, 12).with_data("b"),
            CodeLens::new(20, 0, 20, 8).with_data("c"),
        ];
        let groups = merge_adjacent_lenses(&lenses);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].line, 10);
        assert_eq!(groups[0].count(), 2);
        assert_eq!(groups[1].line, 20);
        assert_eq!(groups[1].count(), 1);
    }

    #[test]
    fn merge_adjacent_empty_input() {
        let groups = merge_adjacent_lenses(&[]);
        assert!(groups.is_empty());
    }

    #[test]
    fn merged_group_titles() {
        let lenses = vec![
            CodeLens::new(1, 0, 1, 5).with_command(Command {
                title: "3 references".into(),
                command_id: "ref".into(),
                tooltip: String::new(),
                arguments: vec![],
            }),
            CodeLens::new(1, 6, 1, 12), // unresolved
        ];
        let groups = merge_adjacent_lenses(&lenses);
        assert_eq!(groups[0].titles(), vec!["3 references"]);
        assert!(!groups[0].all_resolved());
    }

#[test]
    fn codelensclickhandler_severity_ordering() {
        assert!(CodeLensClickHandlerSeverity::Critical > CodeLensClickHandlerSeverity::High);
        assert!(CodeLensClickHandlerSeverity::High > CodeLensClickHandlerSeverity::Medium);
        assert!(CodeLensClickHandlerSeverity::Medium > CodeLensClickHandlerSeverity::Low);
    }

    #[test]
    fn codelensclickhandler_severity_display() {
        assert_eq!(CodeLensClickHandlerSeverity::Low.to_string(), "low");
        assert_eq!(CodeLensClickHandlerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn codelensclickhandler_entry_creation() {
        let e = CodeLensClickHandlerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, CodeLensClickHandlerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn codelensclickhandler_entry_builder() {
        let e = CodeLensClickHandlerEntry::new("e2", "Entry 2")
            .with_severity(CodeLensClickHandlerSeverity::High)
            .with_detail("some detail")
            .with_lens_count(42);
        assert_eq!(e.severity, CodeLensClickHandlerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.lens_count, 42);
    }

    #[test]
    fn codelensclickhandler_entry_enable_disable() {
        let mut e = CodeLensClickHandlerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn codelensclickhandler_add_and_count() {
        let mut mgr = CodeLensClickHandler::new("test");
        mgr.add(CodeLensClickHandlerEntry::new("a", "A"));
        mgr.add(CodeLensClickHandlerEntry::new("b", "B").with_severity(CodeLensClickHandlerSeverity::High));
        assert_eq!(mgr.lens_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn codelensclickhandler_remove() {
        let mut mgr = CodeLensClickHandler::new("test");
        mgr.add(CodeLensClickHandlerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn codelensclickhandler_capacity() {
        let mut mgr = CodeLensClickHandler::new("test").with_capacity(1);
        assert!(mgr.add(CodeLensClickHandlerEntry::new("a", "A")));
        assert!(!mgr.add(CodeLensClickHandlerEntry::new("b", "B")));
    }

    #[test]
    fn codelensclickhandler_sorted_by_severity() {
        let mut mgr = CodeLensClickHandler::new("test");
        mgr.add(CodeLensClickHandlerEntry::new("lo", "Low"));
        mgr.add(CodeLensClickHandlerEntry::new("hi", "High").with_severity(CodeLensClickHandlerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, CodeLensClickHandlerSeverity::Critical);
    }

    #[test]
    fn codelensclickhandler_summary() {
        let mgr = CodeLensClickHandler::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn codelenscacheinvalidator_config_defaults() {
        let cfg = CodeLensCacheInvalidatorConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn codelenscacheinvalidator_item_creation() {
        let item = CodeLensCacheInvalidatorItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn codelenscacheinvalidator_add_and_get() {
        let mut mgr = CodeLensCacheInvalidator::new(CodeLensCacheInvalidatorConfig::new("test"));
        mgr.add(CodeLensCacheInvalidatorItem::new("k1", "v1"));
        assert_eq!(mgr.cache_size(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn codelenscacheinvalidator_remove_item() {
        let mut mgr = CodeLensCacheInvalidator::new(CodeLensCacheInvalidatorConfig::new("test"));
        mgr.add(CodeLensCacheInvalidatorItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn codelenscacheinvalidator_sorted_by_priority() {
        let mut mgr = CodeLensCacheInvalidator::new(CodeLensCacheInvalidatorConfig::new("test"));
        mgr.add(CodeLensCacheInvalidatorItem::new("lo", "low").with_priority(1));
        mgr.add(CodeLensCacheInvalidatorItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn codelenscacheinvalidator_items_with_tag() {
        let mut mgr = CodeLensCacheInvalidator::new(CodeLensCacheInvalidatorConfig::new("test"));
        mgr.add(CodeLensCacheInvalidatorItem::new("a", "1").with_tag("x"));
        mgr.add(CodeLensCacheInvalidatorItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn codelenscacheinvalidator_report() {
        let mgr = CodeLensCacheInvalidator::new(CodeLensCacheInvalidatorConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn codelens_config_new() {
        let cfg = CodelensConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn codelens_config_set_get() {
        let mut cfg = CodelensConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn codelens_config_remove() {
        let mut cfg = CodelensConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn codelens_config_keys_sorted() {
        let mut cfg = CodelensConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn codelens_config_bump_version() {
        let mut cfg = CodelensConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn codelens_config_clear() {
        let mut cfg = CodelensConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn codelens_config_merge() {
        let mut cfg1 = CodelensConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = CodelensConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn codelens_config_disable() {
        let mut cfg = CodelensConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn codelens_rate_tracker_empty() {
        let rt = CodelensRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn codelens_rate_tracker_record() {
        let mut rt = CodelensRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn codelens_rate_tracker_prune() {
        let mut rt = CodelensRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn codelens_validator_valid() {
        let v = CodelensValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn codelens_validator_errors() {
        let mut v = CodelensValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn codelens_validator_clear() {
        let mut v = CodelensValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn codelens_validator_merge() {
        let mut v1 = CodelensValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = CodelensValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn codelens_rate_tracker_clear() {
        let mut rt = CodelensRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn ya_metrics_empty() {
        let m = YaMetrics::new("codelens");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ya_metrics_record_and_mean() {
        let mut m = YaMetrics::new("codelens");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ya_metrics_min_max() {
        let mut m = YaMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ya_metrics_variance_and_std() {
        let mut m = YaMetrics::new("v");
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
    fn ya_metrics_percentile() {
        let mut m = YaMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn ya_metrics_merge() {
        let mut a = YaMetrics::new("a");
        a.record(1.0);
        let mut b = YaMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn ya_metrics_reset() {
        let mut m = YaMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn ya_rate_window_empty() {
        let rw = YaRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn ya_rate_window_tick_and_rate() {
        let mut rw = YaRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn ya_lru_cache_basic() {
        let mut c = YaLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn ya_lru_cache_contains_and_keys() {
        let mut c = YaLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn ya_lru_cache_remove() {
        let mut c = YaLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn ya_metrics_sum() {
        let mut m = YaMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ya_metrics_label() {
        let m = YaMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn ya_lru_cache_clear() {
        let mut c = YaLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for codelens
    #[test]
    fn xa_codelens_ring_new() {
        let rb = super::XaCodelensRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_codelens_ring_push_len() {
        let mut rb = super::XaCodelensRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_codelens_ring_wrap() {
        let mut rb = super::XaCodelensRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_codelens_ring_mean_empty() {
        let rb = super::XaCodelensRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_codelens_ring_mean_values() {
        let mut rb = super::XaCodelensRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_codelens_ring_min_max() {
        let mut rb = super::XaCodelensRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_codelens_ring_iter() {
        let mut rb = super::XaCodelensRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_codelens_counter_new() {
        let c = super::XaCodelensCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_codelens_counter_inc() {
        let mut c = super::XaCodelensCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_codelens_counter_inc_by() {
        let mut c = super::XaCodelensCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_codelens_counter_reset() {
        let mut c = super::XaCodelensCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_codelens_counter_clear() {
        let mut c = super::XaCodelensCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_codelens_counter_default() {
        let c = super::XaCodelensCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 16 ----

    #[test]
    fn xc_16_pool_new_empty() {
        let pool: super::Xc16Pool<i32> = super::Xc16Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_16_pool_release_acquire() {
        let mut pool = super::Xc16Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_16_pool_acquire_empty() {
        let mut pool: super::Xc16Pool<i32> = super::Xc16Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_16_pool_full() {
        let mut pool = super::Xc16Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_16_pool_drain() {
        let mut pool = super::Xc16Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_16_pool_stats() {
        let mut pool = super::Xc16Pool::new(8);
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
    fn xc_16_pool_clear() {
        let mut pool = super::Xc16Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_16_pool_shrink() {
        let mut pool = super::Xc16Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_16_pool_default() {
        let pool: super::Xc16Pool<String> = super::Xc16Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_16_pool_extend() {
        let mut pool = super::Xc16Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_16_pool_retain() {
        let mut pool = super::Xc16Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_16_scheduler_round_robin() {
        let mut sched = super::Xc16Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_16_scheduler_empty() {
        let mut sched = super::Xc16Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_16_scheduler_reset() {
        let mut sched = super::Xc16Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_16_scheduler_add_remove() {
        let mut sched = super::Xc16Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_16_scheduler_targets() {
        let sched = super::Xc16Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_16_hash_empty() {
        assert_eq!(super::xc_16_hash(b""), 5381);
    }

    #[test]
    fn xc_16_hash_data() {
        let h = super::xc_16_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_16_hash(b"hello"), h);
    }

    #[test]
    fn xc_16_reverse_str() {
        assert_eq!(super::xc_16_reverse("abc"), "cba");
        assert_eq!(super::xc_16_reverse(""), "");
    }


    // --- xd_52 deepening tests ---

    #[test]
    fn xd_52_sm_initial_state() {
        let sm = Xd52StateMachine::new();
        assert_eq!(sm.current_state(), Xd52State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_52_sm_valid_idle_to_running() {
        let mut sm = Xd52StateMachine::new();
        assert!(sm.transition(Xd52State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd52State::Running);
    }

    #[test]
    fn xd_52_sm_valid_running_to_paused() {
        let mut sm = Xd52StateMachine::new();
        sm.transition(Xd52State::Running).unwrap();
        assert!(sm.transition(Xd52State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd52State::Paused);
    }

    #[test]
    fn xd_52_sm_valid_running_to_done() {
        let mut sm = Xd52StateMachine::new();
        sm.transition(Xd52State::Running).unwrap();
        assert!(sm.transition(Xd52State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd52State::Done);
    }

    #[test]
    fn xd_52_sm_valid_paused_to_running() {
        let mut sm = Xd52StateMachine::new();
        sm.transition(Xd52State::Running).unwrap();
        sm.transition(Xd52State::Paused).unwrap();
        assert!(sm.transition(Xd52State::Running).is_ok());
    }

    #[test]
    fn xd_52_sm_valid_done_to_idle() {
        let mut sm = Xd52StateMachine::new();
        sm.transition(Xd52State::Running).unwrap();
        sm.transition(Xd52State::Done).unwrap();
        assert!(sm.transition(Xd52State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd52State::Idle);
    }

    #[test]
    fn xd_52_sm_invalid_idle_to_done() {
        let mut sm = Xd52StateMachine::new();
        assert!(sm.transition(Xd52State::Done).is_err());
    }

    #[test]
    fn xd_52_sm_invalid_idle_to_paused() {
        let mut sm = Xd52StateMachine::new();
        assert!(sm.transition(Xd52State::Paused).is_err());
    }

    #[test]
    fn xd_52_sm_history_tracking() {
        let mut sm = Xd52StateMachine::new();
        sm.transition(Xd52State::Running).unwrap();
        sm.transition(Xd52State::Paused).unwrap();
        sm.transition(Xd52State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd52State::Idle);
        assert_eq!(sm.history()[0].to, Xd52State::Running);
        assert_eq!(sm.history()[1].from, Xd52State::Running);
        assert_eq!(sm.history()[2].to, Xd52State::Done);
    }

    #[test]
    fn xd_52_sm_serialize_deserialize() {
        let mut sm = Xd52StateMachine::new();
        sm.transition(Xd52State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd52StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd52State::Running));
    }

    #[test]
    fn xd_52_sm_deserialize_invalid() {
        assert_eq!(Xd52StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_52_sm_reset() {
        let mut sm = Xd52StateMachine::new();
        sm.transition(Xd52State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd52State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_52_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd52EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd52Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_52_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd52EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd52Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd52Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_52_bus_unsubscribe() {
        let mut bus = Xd52EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_52_event_kind_and_payload() {
        let e = Xd52Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd52Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_52_bus_clear_history() {
        let mut bus = Xd52EventBus::new();
        bus.publish(Xd52Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_52_sm_step_counter_increments() {
        let mut sm = Xd52StateMachine::new();
        sm.transition(Xd52State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd52State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #50 --

    #[test]
    fn xf50_trie_insert_search() {
        let mut t = Xf50Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf50_trie_starts_with() {
        let mut t = Xf50Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf50_trie_remove() {
        let mut t = Xf50Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf50_trie_word_count() {
        let mut t = Xf50Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf50_trie_longest_prefix() {
        let mut t = Xf50Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf50_trie_all_words() {
        let mut t = Xf50Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf50_trie_autocomplete() {
        let mut t = Xf50Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf50_trie_empty_search() {
        let t = Xf50Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf50_bloom_add_contains() {
        let mut bf = Xf50BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf50_bloom_probably_absent() {
        let bf = Xf50BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf50_bloom_false_positive_rate() {
        let mut bf = Xf50BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf50_bloom_clear() {
        let mut bf = Xf50BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf50_bloom_union() {
        let mut a = Xf50BloomFilter::xf_new(512, 2);
        let mut b = Xf50BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf50_bloom_intersection_estimate() {
        let mut a = Xf50BloomFilter::xf_new(512, 2);
        let mut b = Xf50BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf50_bloom_union_size_mismatch() {
        let a = Xf50BloomFilter::xf_new(256, 2);
        let b = Xf50BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }

}
