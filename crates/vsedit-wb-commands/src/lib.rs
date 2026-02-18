//! Workbench command execution.

use std::fmt;

/// Errors that can occur during command operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// The command id was empty or invalid.
    InvalidId(String),
    /// A command with the given id already exists.
    DuplicateId(String),
    /// The referenced command was not found.
    NotFound(String),
    /// The command is disabled and cannot be executed.
    Disabled(String),
    /// The title was empty.
    EmptyTitle,
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::InvalidId(id) => write!(f, "invalid command id: '{id}'"),
            CommandError::DuplicateId(id) => write!(f, "duplicate command id: '{id}'"),
            CommandError::NotFound(id) => write!(f, "command not found: '{id}'"),
            CommandError::Disabled(id) => write!(f, "command is disabled: '{id}'"),
            CommandError::EmptyTitle => write!(f, "command title must not be empty"),
        }
    }
}

impl std::error::Error for CommandError {}

/// Result type for command operations.
pub type CommandResult<T> = Result<T, CommandError>;

/// Origin of a command invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandSource {
    User,
    Extension,
    System,
    Keybinding,
}

impl fmt::Display for CommandSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandSource::User => write!(f, "User"),
            CommandSource::Extension => write!(f, "Extension"),
            CommandSource::System => write!(f, "System"),
            CommandSource::Keybinding => write!(f, "Keybinding"),
        }
    }
}

/// Descriptor for a registered command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDescriptor {
    pub id: String,
    pub title: String,
    pub category: Option<String>,
    pub source: CommandSource,
    pub enabled: bool,
    pub keybinding: Option<String>,
}

/// Builder for constructing a [`CommandDescriptor`] with defaults.
pub struct CommandDescriptorBuilder {
    id: String,
    title: String,
    category: Option<String>,
    source: CommandSource,
    enabled: bool,
    keybinding: Option<String>,
}

impl CommandDescriptorBuilder {
    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    pub fn source(mut self, source: CommandSource) -> Self {
        self.source = source;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn keybinding(mut self, keybinding: impl Into<String>) -> Self {
        self.keybinding = Some(keybinding.into());
        self
    }

    pub fn build(self) -> CommandDescriptor {
        CommandDescriptor {
            id: self.id,
            title: self.title,
            category: self.category,
            source: self.source,
            enabled: self.enabled,
            keybinding: self.keybinding,
        }
    }

    /// Builds the descriptor after validating the id and title.
    pub fn try_build(self) -> CommandResult<CommandDescriptor> {
        if !CommandDescriptor::is_valid_id(&self.id) {
            return Err(CommandError::InvalidId(self.id));
        }
        if self.title.is_empty() {
            return Err(CommandError::EmptyTitle);
        }
        Ok(self.build())
    }
}

impl fmt::Display for CommandDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref cat) = self.category {
            write!(f, "{cat}: {title} ({id})", title = self.title, id = self.id)
        } else {
            write!(f, "{title} ({id})", title = self.title, id = self.id)
        }
    }
}

impl CommandDescriptor {
    pub fn builder(id: impl Into<String>, title: impl Into<String>) -> CommandDescriptorBuilder {
        CommandDescriptorBuilder {
            id: id.into(),
            title: title.into(),
            category: None,
            source: CommandSource::System,
            enabled: true,
            keybinding: None,
        }
    }

    /// Returns the qualified id: `"category.id"` if a category is set, otherwise just the id.
    pub fn qualified_id(&self) -> String {
        match self.category {
            Some(ref cat) => format!("{cat}.{id}", id = self.id),
            None => self.id.clone(),
        }
    }

    /// Returns `true` if the command id looks valid (non-empty, ascii, no whitespace).
    pub fn is_valid_id(id: &str) -> bool {
        !id.is_empty() && id.is_ascii() && !id.contains(char::is_whitespace)
    }
}

/// Registry of all available commands.
pub struct CommandRegistry {
    commands: Vec<CommandDescriptor>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn register(&mut self, descriptor: CommandDescriptor) {
        self.commands.push(descriptor);
    }

    pub fn unregister(&mut self, id: &str) -> bool {
        let len = self.commands.len();
        self.commands.retain(|c| c.id != id);
        self.commands.len() != len
    }

    pub fn get_command(&self, id: &str) -> Option<&CommandDescriptor> {
        self.commands.iter().find(|c| c.id == id)
    }

    pub fn get_all(&self) -> &[CommandDescriptor] {
        &self.commands
    }

    /// Finds commands whose id or title contain the query substring (case-insensitive).
    pub fn find_commands(&self, query: &str) -> Vec<&CommandDescriptor> {
        let q = query.to_lowercase();
        self.commands
            .iter()
            .filter(|c| {
                c.id.to_lowercase().contains(&q) || c.title.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Returns commands matching the given category.
    pub fn get_by_category(&self, category: &str) -> Vec<&CommandDescriptor> {
        self.commands
            .iter()
            .filter(|c| c.category.as_deref() == Some(category))
            .collect()
    }

    /// Sets the enabled state of a command by id. Returns `true` if found.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        if let Some(cmd) = self.commands.iter_mut().find(|c| c.id == id) {
            cmd.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Returns all enabled commands.
    pub fn get_enabled_commands(&self) -> Vec<&CommandDescriptor> {
        self.commands.iter().filter(|c| c.enabled).collect()
    }

    /// Returns whether a command with the given id exists.
    pub fn has_command(&self, id: &str) -> bool {
        self.commands.iter().any(|c| c.id == id)
    }

    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    /// Registers a command only if no command with the same id exists.
    pub fn try_register(&mut self, descriptor: CommandDescriptor) -> CommandResult<()> {
        if !CommandDescriptor::is_valid_id(&descriptor.id) {
            return Err(CommandError::InvalidId(descriptor.id));
        }
        if self.has_command(&descriptor.id) {
            return Err(CommandError::DuplicateId(descriptor.id));
        }
        self.commands.push(descriptor);
        Ok(())
    }

    /// Returns all distinct categories present in the registry.
    pub fn categories(&self) -> Vec<&str> {
        let mut cats: Vec<&str> = self
            .commands
            .iter()
            .filter_map(|c| c.category.as_deref())
            .collect();
        cats.sort_unstable();
        cats.dedup();
        cats
    }

    /// Returns commands originating from the given source.
    pub fn get_by_source(&self, source: CommandSource) -> Vec<&CommandDescriptor> {
        self.commands
            .iter()
            .filter(|c| c.source == source)
            .collect()
    }

    /// Disables all commands, returning how many were already disabled.
    pub fn disable_all(&mut self) -> usize {
        let mut already_disabled = 0;
        for cmd in &mut self.commands {
            if !cmd.enabled {
                already_disabled += 1;
            }
            cmd.enabled = false;
        }
        already_disabled
    }

    /// Enables all commands, returning how many were already enabled.
    pub fn enable_all(&mut self) -> usize {
        let mut already_enabled = 0;
        for cmd in &mut self.commands {
            if cmd.enabled {
                already_enabled += 1;
            }
            cmd.enabled = true;
        }
        already_enabled
    }
}

impl fmt::Display for CommandRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CommandRegistry({total} commands, {enabled} enabled)",
            total = self.commands.len(),
            enabled = self.commands.iter().filter(|c| c.enabled).count(),
        )
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Record of a single command execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandExecutionEntry {
    pub command_id: String,
    pub source: CommandSource,
    /// Monotonic sequence number assigned at log time.
    pub seq: u64,
}

/// Append-only log of command executions.
pub struct CommandExecutionLog {
    entries: Vec<CommandExecutionEntry>,
    next_seq: u64,
}

impl CommandExecutionLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_seq: 1,
        }
    }

    /// Record a command execution and return its sequence number.
    pub fn log(&mut self, command_id: impl Into<String>, source: CommandSource) -> u64 {
        let seq = self.next_seq;
        self.entries.push(CommandExecutionEntry {
            command_id: command_id.into(),
            source,
            seq,
        });
        self.next_seq += 1;
        seq
    }

    /// Return the `n` most recent entries (oldest first).
    pub fn get_recent(&self, n: usize) -> &[CommandExecutionEntry] {
        let start = self.entries.len().saturating_sub(n);
        &self.entries[start..]
    }

    /// Return all entries matching the given command id.
    pub fn get_by_command(&self, id: &str) -> Vec<&CommandExecutionEntry> {
        self.entries.iter().filter(|e| e.command_id == id).collect()
    }

    /// Remove all entries from the log.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Total number of entries in the log.
    pub fn total_count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for CommandExecutionLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Outcome of executing a single command within a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchStepResult {
    /// The command executed successfully.
    Ok,
    /// The command failed with the given error; rollback will be attempted.
    Failed(CommandError),
}

/// Execute a sequence of commands through a registry, with rollback support.
///
/// Commands are executed in order. If any command is not found or is disabled
/// the batch stops and returns information about completed steps so that the
/// caller can undo them.
pub struct CommandBatch {
    command_ids: Vec<String>,
}

impl CommandBatch {
    pub fn new() -> Self {
        Self {
            command_ids: Vec::new(),
        }
    }

    /// Append a command id to the batch.
    pub fn push(&mut self, id: impl Into<String>) {
        self.command_ids.push(id.into());
    }

    /// Number of commands in the batch.
    pub fn len(&self) -> usize {
        self.command_ids.len()
    }

    /// Returns `true` when the batch contains no commands.
    pub fn is_empty(&self) -> bool {
        self.command_ids.len() == 0
    }

    /// Validate and "execute" each command in order against the registry.
    ///
    /// Returns a vec of results, one per step attempted. On the first failure
    /// execution stops; the returned vec will contain `Ok` for every command
    /// that succeeded followed by a single `Failed` entry.  The caller can
    /// use the successful prefix as a rollback list.
    pub fn execute(&self, registry: &CommandRegistry) -> Vec<(String, BatchStepResult)> {
        let mut results = Vec::with_capacity(self.command_ids.len());
        for id in &self.command_ids {
            match registry.get_command(id) {
                None => {
                    results.push((
                        id.clone(),
                        BatchStepResult::Failed(CommandError::NotFound(id.clone())),
                    ));
                    break;
                }
                Some(cmd) if !cmd.enabled => {
                    results.push((
                        id.clone(),
                        BatchStepResult::Failed(CommandError::Disabled(id.clone())),
                    ));
                    break;
                }
                Some(_) => {
                    results.push((id.clone(), BatchStepResult::Ok));
                }
            }
        }
        results
    }

    /// Return the list of command ids that would need to be rolled back given
    /// the results of [`execute`].  Only successfully-executed ids are returned,
    /// in reverse order.
    pub fn rollback_ids(results: &[(String, BatchStepResult)]) -> Vec<String> {
        results
            .iter()
            .filter(|(_, r)| *r == BatchStepResult::Ok)
            .map(|(id, _)| id.clone())
            .rev()
            .collect()
    }
}

impl Default for CommandBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Stateless utilities for validating command identifiers.
pub struct CommandValidator;

impl CommandValidator {
    /// A command id is valid when it is non-empty, ASCII-only, contains no
    /// whitespace, and has at least one dot separator (e.g. `"editor.save"`).
    pub fn validate_command_id(id: &str) -> bool {
        CommandDescriptor::is_valid_id(id) && id.contains('.')
    }

    /// Validates a slice of ids, returning the first invalid one (if any).
    pub fn first_invalid(ids: &[&str]) -> Option<String> {
        ids.iter()
            .find(|id| !Self::validate_command_id(id))
            .map(|id| (*id).to_string())
    }
}

// ── Command history ──

/// Records executed commands with monotonic sequence numbers.
pub struct CommandHistory {
    entries: Vec<(u64, String, CommandSource)>,
    next_seq: u64,
}

impl CommandHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_seq: 1,
        }
    }

    /// Push a command execution record.
    pub fn push(&mut self, command_id: impl Into<String>, source: CommandSource) {
        self.entries.push((self.next_seq, command_id.into(), source));
        self.next_seq += 1;
    }

    /// Return the `n` most recent entries (oldest first).
    pub fn recent(&self, n: usize) -> &[(u64, String, CommandSource)] {
        let start = self.entries.len().saturating_sub(n);
        &self.entries[start..]
    }

    /// Total number of recorded executions.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}

// ── Command palette ──

/// Searches and filters commands for quick-access display.
pub struct CommandPalette<'a> {
    registry: &'a CommandRegistry,
}

impl<'a> CommandPalette<'a> {
    pub fn new(registry: &'a CommandRegistry) -> Self {
        Self { registry }
    }

    /// Return all enabled commands whose id or title contain `query`
    /// (case-insensitive), sorted by title.
    pub fn search(&self, query: &str) -> Vec<&'a CommandDescriptor> {
        let q = query.to_lowercase();
        let mut results: Vec<&CommandDescriptor> = self
            .registry
            .get_all()
            .iter()
            .filter(|c| {
                c.enabled
                    && (c.id.to_lowercase().contains(&q)
                        || c.title.to_lowercase().contains(&q))
            })
            .collect();
        results.sort_by(|a, b| a.title.cmp(&b.title));
        results
    }

    /// Return all enabled commands, sorted by title.
    pub fn all_enabled(&self) -> Vec<&'a CommandDescriptor> {
        let mut cmds: Vec<&CommandDescriptor> = self
            .registry
            .get_all()
            .iter()
            .filter(|c| c.enabled)
            .collect();
        cmds.sort_by(|a, b| a.title.cmp(&b.title));
        cmds
    }
}

/// An item displayed in the command palette with a display label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPaletteItem {
    pub command_id: String,
    pub category: Option<String>,
    pub title: String,
    pub keybinding: Option<String>,
}

impl CommandPaletteItem {
    pub fn new(command_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            command_id: command_id.into(),
            category: None,
            title: title.into(),
            keybinding: None,
        }
    }

    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    pub fn with_keybinding(mut self, keybinding: impl Into<String>) -> Self {
        self.keybinding = Some(keybinding.into());
        self
    }

    /// Generate the display label: "Category: Title" or just "Title".
    pub fn display_label(&self) -> String {
        match &self.category {
            Some(cat) => format!("{}: {}", cat, self.title),
            None => self.title.clone(),
        }
    }

    /// Build from a CommandDescriptor.
    pub fn from_descriptor(desc: &CommandDescriptor) -> Self {
        Self {
            command_id: desc.id.clone(),
            category: desc.category.clone(),
            title: desc.title.clone(),
            keybinding: desc.keybinding.clone(),
        }
    }
}

impl fmt::Display for CommandPaletteItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = self.display_label();
        match &self.keybinding {
            Some(kb) => write!(f, "{label}  ({kb})"),
            None => write!(f, "{label}"),
        }
    }
}

/// A search result with a relevance score (higher = better match).
#[derive(Debug, Clone)]
pub struct PaletteSearchResult {
    pub item: CommandPaletteItem,
    pub score: i32,
}

/// Compute a fuzzy match score. Returns 0 for no match, higher for better matches.
fn fuzzy_score(query: &str, text: &str) -> i32 {
    if query.is_empty() {
        return 1;
    }
    let query_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();

    // Exact substring match gets highest score
    if text_lower.contains(&query_lower) {
        let bonus = if text_lower.starts_with(&query_lower) {
            50
        } else {
            0
        };
        return 100 + bonus - text.len() as i32;
    }

    // Character-by-character fuzzy match
    let mut qi = 0;
    let query_chars: Vec<char> = query_lower.chars().collect();
    let mut score = 0i32;
    let mut prev_matched = false;

    for c in text_lower.chars() {
        if qi < query_chars.len() && c == query_chars[qi] {
            score += if prev_matched { 3 } else { 1 };
            qi += 1;
            prev_matched = true;
        } else {
            prev_matched = false;
        }
    }

    if qi == query_chars.len() {
        score
    } else {
        0
    }
}

/// Search palette items with fuzzy matching, returning results sorted by score descending.
pub fn command_palette_search(
    items: &[CommandPaletteItem],
    query: &str,
) -> Vec<PaletteSearchResult> {
    let mut results: Vec<PaletteSearchResult> = items
        .iter()
        .filter_map(|item| {
            let label = item.display_label();
            let label_score = fuzzy_score(query, &label);
            let id_score = fuzzy_score(query, &item.command_id);
            let score = label_score.max(id_score);
            if score > 0 {
                Some(PaletteSearchResult {
                    item: item.clone(),
                    score,
                })
            } else {
                None
            }
        })
        .collect();
    results.sort_by(|a, b| b.score.cmp(&a.score));
    results
}

/// Tracks recently used commands in the command palette.
#[derive(Debug, Clone)]
pub struct CommandPaletteHistory {
    entries: Vec<String>,
    max_size: usize,
}

impl CommandPaletteHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size,
        }
    }

    /// Record a command as recently used. Moves it to the front if already present.
    pub fn record(&mut self, command_id: impl Into<String>) {
        let id = command_id.into();
        self.entries.retain(|e| e != &id);
        self.entries.insert(0, id);
        if self.entries.len() > self.max_size {
            self.entries.truncate(self.max_size);
        }
    }

    /// Get the most recently used command ids, most recent first.
    pub fn recent(&self) -> &[String] {
        &self.entries
    }

    /// Get the number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Check if a command is in the history.
    pub fn contains(&self, command_id: &str) -> bool {
        self.entries.iter().any(|e| e == command_id)
    }

    /// Boost search results by sorting recently used commands higher.
    pub fn boost_results(&self, results: &mut [PaletteSearchResult]) {
        for result in results.iter_mut() {
            if let Some(pos) = self.entries.iter().position(|e| e == &result.item.command_id) {
                result.score += (self.max_size - pos) as i32 * 10;
            }
        }
        results.sort_by(|a, b| b.score.cmp(&a.score));
    }
}

impl Default for CommandPaletteHistory {
    fn default() -> Self {
        Self::new(50)
    }
}

// ---------------------------------------------------------------------------
// CommandChain – chain multiple commands sequentially with error handling
// ---------------------------------------------------------------------------

/// Result of executing a single step in a command chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainStepResult {
    Success,
    Skipped,
    Failed(String),
}

/// A step in a command chain.
#[derive(Debug, Clone)]
pub struct ChainStep {
    /// Command id to execute.
    pub command_id: String,
    /// Whether to continue the chain if this step fails.
    pub continue_on_error: bool,
    /// Result after execution.
    pub result: Option<ChainStepResult>,
}

/// Chain multiple commands to execute sequentially.
#[derive(Debug, Clone)]
pub struct CommandChain {
    pub name: String,
    steps: Vec<ChainStep>,
}

impl CommandChain {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            steps: Vec::new(),
        }
    }

    /// Append a step that halts the chain on failure.
    pub fn then(&mut self, command_id: &str) -> &mut Self {
        self.steps.push(ChainStep {
            command_id: command_id.to_string(),
            continue_on_error: false,
            result: None,
        });
        self
    }

    /// Append a step that allows the chain to continue even on failure.
    pub fn then_optional(&mut self, command_id: &str) -> &mut Self {
        self.steps.push(ChainStep {
            command_id: command_id.to_string(),
            continue_on_error: true,
            result: None,
        });
        self
    }

    /// Simulate execution against a registry, marking steps as success/failed.
    pub fn execute(&mut self, registry: &CommandRegistry) -> bool {
        for step in &mut self.steps {
            match registry.get_command(&step.command_id) {
                Some(cmd) if cmd.enabled => {
                    step.result = Some(ChainStepResult::Success);
                }
                Some(_) => {
                    step.result = Some(ChainStepResult::Failed("disabled".to_string()));
                    if !step.continue_on_error {
                        return false;
                    }
                }
                None => {
                    step.result = Some(ChainStepResult::Failed("not found".to_string()));
                    if !step.continue_on_error {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Number of steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Count of successfully executed steps.
    pub fn success_count(&self) -> usize {
        self.steps.iter().filter(|s| s.result == Some(ChainStepResult::Success)).count()
    }
}

// ---------------------------------------------------------------------------
// CommandThrottle – throttle command execution
// ---------------------------------------------------------------------------

/// Tracks command execution timestamps for throttling.
#[derive(Debug, Clone)]
pub struct CommandThrottle {
    /// Minimum interval in milliseconds between executions of the same command.
    pub interval_ms: u64,
    /// Map of command_id → last execution timestamp (ms since epoch).
    last_exec: std::collections::HashMap<String, u64>,
}

impl CommandThrottle {
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            last_exec: std::collections::HashMap::new(),
        }
    }

    /// Check whether the command is allowed to execute at the given timestamp.
    pub fn is_allowed(&self, command_id: &str, now_ms: u64) -> bool {
        match self.last_exec.get(command_id) {
            Some(&last) => now_ms.saturating_sub(last) >= self.interval_ms,
            None => true,
        }
    }

    /// Record an execution of the command at the given timestamp.
    pub fn record(&mut self, command_id: &str, now_ms: u64) {
        self.last_exec.insert(command_id.to_string(), now_ms);
    }

    /// Try to execute: returns true and records if allowed, false otherwise.
    pub fn try_execute(&mut self, command_id: &str, now_ms: u64) -> bool {
        if self.is_allowed(command_id, now_ms) {
            self.record(command_id, now_ms);
            true
        } else {
            false
        }
    }

    /// Clear throttle state for all commands.
    pub fn clear(&mut self) {
        self.last_exec.clear();
    }
}

// ---------------------------------------------------------------------------
// CommandGroup – group related commands
// ---------------------------------------------------------------------------

/// A named group of related commands.
#[derive(Debug, Clone)]
pub struct CommandGroup {
    pub name: String,
    pub description: String,
    command_ids: Vec<String>,
}

impl CommandGroup {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            command_ids: Vec::new(),
        }
    }

    /// Add a command id to the group.
    pub fn add(&mut self, command_id: &str) {
        if !self.command_ids.iter().any(|id| id == command_id) {
            self.command_ids.push(command_id.to_string());
        }
    }

    /// Remove a command id from the group.
    pub fn remove(&mut self, command_id: &str) -> bool {
        let before = self.command_ids.len();
        self.command_ids.retain(|id| id != command_id);
        self.command_ids.len() != before
    }

    /// All command ids in this group.
    pub fn commands(&self) -> &[String] {
        &self.command_ids
    }

    /// Number of commands in the group.
    pub fn len(&self) -> usize {
        self.command_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.command_ids.is_empty()
    }

    /// Check if a command is in this group.
    pub fn contains(&self, command_id: &str) -> bool {
        self.command_ids.iter().any(|id| id == command_id)
    }

    /// Resolve group commands against a registry, returning descriptors found.
    pub fn resolve<'a>(&self, registry: &'a CommandRegistry) -> Vec<&'a CommandDescriptor> {
        self.command_ids
            .iter()
            .filter_map(|id| registry.get_command(id))
            .collect()
    }
}

/// Summary statistics for a `CommandRegistry`.
pub struct CommandRegistryStats {
    pub total: usize,
    pub enabled: usize,
    pub disabled: usize,
    pub category_count: usize,
}

impl CommandRegistry {
    /// Compute summary statistics for the registry.
    pub fn stats(&self) -> CommandRegistryStats {
        let enabled = self.get_enabled_commands().len();
        CommandRegistryStats {
            total: self.command_count(),
            enabled,
            disabled: self.command_count() - enabled,
            category_count: self.categories().len(),
        }
    }

    /// Return commands whose id starts with the given prefix.
    pub fn find_by_prefix(&self, prefix: &str) -> Vec<&CommandDescriptor> {
        self.commands
            .iter()
            .filter(|c| c.id.starts_with(prefix))
            .collect()
    }

    /// Return the ids of all disabled commands.
    pub fn disabled_command_ids(&self) -> Vec<&str> {
        self.commands
            .iter()
            .filter(|c| !c.enabled)
            .map(|c| c.id.as_str())
            .collect()
    }

    /// Rename a command's title, returning true if found.
    pub fn rename_title(&mut self, id: &str, new_title: &str) -> bool {
        if let Some(cmd) = self.commands.iter_mut().find(|c| c.id == id) {
            cmd.title = new_title.to_string();
            true
        } else {
            false
        }
    }

    /// Return commands sorted alphabetically by title.
    pub fn sorted_by_title(&self) -> Vec<&CommandDescriptor> {
        let mut cmds: Vec<&CommandDescriptor> = self.commands.iter().collect();
        cmds.sort_by(|a, b| a.title.cmp(&b.title));
        cmds
    }
}

impl CommandExecutionLog {
    /// Return the most frequently executed command id and its count, if any.
    pub fn most_frequent(&self) -> Option<(String, usize)> {
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for entry in &self.entries {
            *counts.entry(entry.command_id.as_str()).or_insert(0) += 1;
        }
        counts
            .into_iter()
            .max_by_key(|&(_, c)| c)
            .map(|(id, c)| (id.to_string(), c))
    }

    /// Return distinct command ids that have been executed.
    pub fn distinct_command_ids(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        self.entries
            .iter()
            .filter(|e| seen.insert(e.command_id.clone()))
            .map(|e| e.command_id.clone())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// CommandAlias – map short names to canonical command ids
// ---------------------------------------------------------------------------

/// Maps alias names to canonical command ids, with reverse lookup support.
#[derive(Debug, Clone)]
pub struct CommandAliasMap {
    /// alias → canonical command id
    aliases: std::collections::HashMap<String, String>,
}

impl CommandAliasMap {
    pub fn new() -> Self {
        Self {
            aliases: std::collections::HashMap::new(),
        }
    }

    /// Register an alias pointing to a canonical command id.
    /// Returns the previous target if the alias was already defined.
    pub fn set(&mut self, alias: &str, command_id: &str) -> Option<String> {
        self.aliases
            .insert(alias.to_string(), command_id.to_string())
    }

    /// Resolve an alias to its canonical command id.
    /// If the input is not an alias, returns `None`.
    pub fn resolve(&self, alias: &str) -> Option<&str> {
        self.aliases.get(alias).map(|s| s.as_str())
    }

    /// Resolve an alias, or return the original string if it is not aliased.
    pub fn resolve_or_identity<'a>(&'a self, id: &'a str) -> &'a str {
        self.aliases.get(id).map(|s| s.as_str()).unwrap_or(id)
    }

    /// Remove an alias. Returns `true` if it existed.
    pub fn remove(&mut self, alias: &str) -> bool {
        self.aliases.remove(alias).is_some()
    }

    /// Return all aliases that point to the given command id.
    pub fn aliases_for(&self, command_id: &str) -> Vec<&str> {
        self.aliases
            .iter()
            .filter(|(_, v)| v.as_str() == command_id)
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Number of defined aliases.
    pub fn len(&self) -> usize {
        self.aliases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.aliases.is_empty()
    }

    /// Check whether an alias exists.
    pub fn contains(&self, alias: &str) -> bool {
        self.aliases.contains_key(alias)
    }

    /// Clear all aliases.
    pub fn clear(&mut self) {
        self.aliases.clear();
    }
}

impl Default for CommandAliasMap {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// KeybindingResolver – resolve keybinding strings to command ids
// ---------------------------------------------------------------------------

/// A single keybinding entry mapping a key combination to a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingEntry {
    /// Normalised key string, e.g. `"ctrl+shift+p"`.
    pub keys: String,
    /// The command id this keybinding triggers.
    pub command_id: String,
    /// Optional "when" clause that must evaluate to true.
    pub when: Option<String>,
}

/// Resolves key combinations to commands, respecting optional "when" conditions.
#[derive(Debug, Clone)]
pub struct KeybindingResolver {
    bindings: Vec<KeybindingEntry>,
}

impl KeybindingResolver {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// Normalise a key string to lowercase with modifiers sorted.
    ///
    /// `"Shift+Ctrl+P"` → `"ctrl+shift+p"`
    pub fn normalise_keys(raw: &str) -> String {
        let mut parts: Vec<&str> = raw.split('+').collect();
        let key = parts.pop().unwrap_or("");
        parts.sort_unstable();
        parts.push(key);
        parts
            .iter()
            .map(|p| p.to_lowercase())
            .collect::<Vec<_>>()
            .join("+")
    }

    /// Add a keybinding. The key string is normalised before storage.
    pub fn add(&mut self, keys: &str, command_id: &str, when: Option<&str>) {
        self.bindings.push(KeybindingEntry {
            keys: Self::normalise_keys(keys),
            command_id: command_id.to_string(),
            when: when.map(|s| s.to_string()),
        });
    }

    /// Remove all bindings for the given command id.
    pub fn remove_command(&mut self, command_id: &str) -> usize {
        let before = self.bindings.len();
        self.bindings.retain(|b| b.command_id != command_id);
        before - self.bindings.len()
    }

    /// Find all bindings that match the given key combination (normalised).
    pub fn resolve(&self, keys: &str) -> Vec<&KeybindingEntry> {
        let norm = Self::normalise_keys(keys);
        self.bindings
            .iter()
            .filter(|b| b.keys == norm)
            .collect()
    }

    /// Find the first binding matching the keys whose `when` clause is
    /// satisfied by the provided context evaluator.
    pub fn resolve_with_context<F>(&self, keys: &str, eval_when: F) -> Option<&KeybindingEntry>
    where
        F: Fn(&str) -> bool,
    {
        let norm = Self::normalise_keys(keys);
        self.bindings.iter().find(|b| {
            b.keys == norm
                && match &b.when {
                    Some(clause) => eval_when(clause),
                    None => true,
                }
        })
    }

    /// Return all bindings for a given command id.
    pub fn bindings_for(&self, command_id: &str) -> Vec<&KeybindingEntry> {
        self.bindings
            .iter()
            .filter(|b| b.command_id == command_id)
            .collect()
    }

    /// Total number of registered keybindings.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl Default for KeybindingResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CommandArgs – typed argument parsing for command invocations
// ---------------------------------------------------------------------------

/// A single argument value that can be passed to a command.
#[derive(Debug, Clone, PartialEq)]
pub enum ArgValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

impl fmt::Display for ArgValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArgValue::String(s) => write!(f, "{s}"),
            ArgValue::Int(n) => write!(f, "{n}"),
            ArgValue::Float(n) => write!(f, "{n}"),
            ArgValue::Bool(b) => write!(f, "{b}"),
            ArgValue::Null => write!(f, "null"),
        }
    }
}

/// Holds named arguments for a command invocation.
#[derive(Debug, Clone)]
pub struct CommandArgs {
    args: std::collections::HashMap<String, ArgValue>,
}

impl CommandArgs {
    pub fn new() -> Self {
        Self {
            args: std::collections::HashMap::new(),
        }
    }

    /// Insert a named argument, returning any previous value.
    pub fn set(&mut self, key: &str, value: ArgValue) -> Option<ArgValue> {
        self.args.insert(key.to_string(), value)
    }

    /// Retrieve an argument by name.
    pub fn get(&self, key: &str) -> Option<&ArgValue> {
        self.args.get(key)
    }

    /// Retrieve a string argument, returning `None` if missing or wrong type.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.args.get(key) {
            Some(ArgValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Retrieve an integer argument.
    pub fn get_int(&self, key: &str) -> Option<i64> {
        match self.args.get(key) {
            Some(ArgValue::Int(n)) => Some(*n),
            _ => None,
        }
    }

    /// Retrieve a boolean argument.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.args.get(key) {
            Some(ArgValue::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    /// Check whether the argument map contains a key.
    pub fn contains_key(&self, key: &str) -> bool {
        self.args.contains_key(key)
    }

    /// Number of arguments.
    pub fn len(&self) -> usize {
        self.args.len()
    }

    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    /// Parse a simple `key=value` argument string.
    ///
    /// Recognises booleans (`true`/`false`), integers, and falls back to
    /// string. Returns `None` if the input contains no `=`.
    pub fn parse_kv(input: &str) -> Option<(String, ArgValue)> {
        let (key, raw_val) = input.split_once('=')?;
        let key = key.trim().to_string();
        let raw_val = raw_val.trim();
        let value = match raw_val {
            "true" => ArgValue::Bool(true),
            "false" => ArgValue::Bool(false),
            "null" => ArgValue::Null,
            _ => {
                if let Ok(n) = raw_val.parse::<i64>() {
                    ArgValue::Int(n)
                } else if let Ok(n) = raw_val.parse::<f64>() {
                    ArgValue::Float(n)
                } else {
                    ArgValue::String(raw_val.to_string())
                }
            }
        };
        Some((key, value))
    }

    /// Parse multiple `key=value` pairs separated by whitespace.
    pub fn parse_many(input: &str) -> Self {
        let mut args = Self::new();
        for token in input.split_whitespace() {
            if let Some((key, value)) = Self::parse_kv(token) {
                args.set(&key, value);
            }
        }
        args
    }
}

impl Default for CommandArgs {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// WhenClause – evaluate context conditions for command enablement
// ---------------------------------------------------------------------------

/// Evaluates "when" clause expressions against a set of boolean context keys.
///
/// Supported operators:
/// - bare key: `editorFocus` → true if key is present and true
/// - negation: `!editorFocus`
/// - conjunction: `editorFocus && editorHasSelection`
/// - disjunction: `editorFocus || terminalFocus`
///
/// `&&` binds tighter than `||` (standard precedence).
#[derive(Debug, Clone)]
pub struct WhenClauseContext {
    keys: std::collections::HashMap<String, bool>,
}

impl WhenClauseContext {
    pub fn new() -> Self {
        Self {
            keys: std::collections::HashMap::new(),
        }
    }

    /// Set a context key to a boolean value.
    pub fn set(&mut self, key: &str, value: bool) {
        self.keys.insert(key.to_string(), value);
    }

    /// Remove a context key.
    pub fn remove(&mut self, key: &str) -> bool {
        self.keys.remove(key).is_some()
    }

    /// Get the value of a context key (defaults to `false` if absent).
    pub fn get(&self, key: &str) -> bool {
        self.keys.get(key).copied().unwrap_or(false)
    }

    /// Evaluate a when-clause expression.
    ///
    /// Grammar (simplified):
    ///   expr     = or_expr
    ///   or_expr  = and_expr ( "||" and_expr )*
    ///   and_expr = atom ( "&&" atom )*
    ///   atom     = "!" atom | key
    pub fn evaluate(&self, expr: &str) -> bool {
        let expr = expr.trim();
        if expr.is_empty() {
            return true;
        }
        // Split on `||` first (lower precedence)
        let or_parts: Vec<&str> = Self::split_top_level(expr, "||");
        or_parts.iter().any(|part| self.eval_and(part.trim()))
    }

    fn eval_and(&self, expr: &str) -> bool {
        let and_parts: Vec<&str> = Self::split_top_level(expr, "&&");
        and_parts.iter().all(|part| self.eval_atom(part.trim()))
    }

    fn eval_atom(&self, expr: &str) -> bool {
        let expr = expr.trim();
        if let Some(rest) = expr.strip_prefix('!') {
            !self.eval_atom(rest.trim())
        } else {
            self.get(expr)
        }
    }

    /// Split a string on a delimiter, but only at the top level (no nesting
    /// support needed for our simple grammar).
    fn split_top_level<'a>(s: &'a str, delim: &str) -> Vec<&'a str> {
        s.split(delim).collect()
    }

    /// Number of context keys set.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Clear all context keys.
    pub fn clear(&mut self) {
        self.keys.clear();
    }
}

impl Default for WhenClauseContext {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CommandInvocation – represents a parsed command invocation with args
// ---------------------------------------------------------------------------

/// A fully parsed command invocation: command id + arguments.
#[derive(Debug, Clone)]
pub struct CommandInvocation {
    pub command_id: String,
    pub args: CommandArgs,
    pub source: CommandSource,
}

impl CommandInvocation {
    pub fn new(command_id: &str, source: CommandSource) -> Self {
        Self {
            command_id: command_id.to_string(),
            args: CommandArgs::new(),
            source,
        }
    }

    /// Parse a textual invocation of the form `"command.id arg1=val1 arg2=val2"`.
    pub fn parse(input: &str, source: CommandSource) -> Option<Self> {
        let mut parts = input.splitn(2, char::is_whitespace);
        let command_id = parts.next()?.trim();
        if command_id.is_empty() {
            return None;
        }
        let args = match parts.next() {
            Some(rest) => CommandArgs::parse_many(rest),
            None => CommandArgs::new(),
        };
        Some(Self {
            command_id: command_id.to_string(),
            args,
            source,
        })
    }

    /// Validate the invocation against a registry, returning the descriptor
    /// if the command exists and is enabled.
    pub fn validate<'a>(
        &self,
        registry: &'a CommandRegistry,
    ) -> CommandResult<&'a CommandDescriptor> {
        match registry.get_command(&self.command_id) {
            None => Err(CommandError::NotFound(self.command_id.clone())),
            Some(cmd) if !cmd.enabled => Err(CommandError::Disabled(self.command_id.clone())),
            Some(cmd) => Ok(cmd),
        }
    }
}

impl fmt::Display for CommandInvocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (via {})", self.command_id, self.source)
    }
}

// ---------------------------------------------------------------------------
// CommandPaletteItem
// ---------------------------------------------------------------------------

/// An item for display in the command palette.
#[derive(Debug, Clone)]
pub struct CommandPaletteItemV2 {
    pub label: String,
    pub detail: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub keybinding: Option<String>,
}

impl CommandPaletteItemV2 {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            detail: None,
            description: None,
            category: None,
            keybinding: None,
        }
    }

    pub fn with_category(mut self, cat: &str) -> Self {
        self.category = Some(cat.to_string());
        self
    }

    pub fn with_keybinding(mut self, kb: &str) -> Self {
        self.keybinding = Some(kb.to_string());
        self
    }

    pub fn matches_query(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.label.to_lowercase().contains(&q)
            || self.category.as_deref().unwrap_or("").to_lowercase().contains(&q)
            || self.description.as_deref().unwrap_or("").to_lowercase().contains(&q)
    }

    pub fn display_string(&self) -> String {
        match &self.category {
            Some(cat) => format!("{}: {}", cat, self.label),
            None => self.label.clone(),
        }
    }

    pub fn sort_key(&self) -> String {
        format!(
            "{}:{}",
            self.category.as_deref().unwrap_or(""),
            self.label
        )
        .to_lowercase()
    }
}

// ---------------------------------------------------------------------------
// CommandContextGuard
// ---------------------------------------------------------------------------

/// Check context before command execution.
#[derive(Debug, Clone)]
pub struct CommandContextGuard {
    required_keys: Vec<String>,
    forbidden_keys: Vec<String>,
}

impl CommandContextGuard {
    pub fn new() -> Self {
        Self {
            required_keys: Vec::new(),
            forbidden_keys: Vec::new(),
        }
    }

    pub fn require(mut self, key: &str) -> Self {
        self.required_keys.push(key.to_string());
        self
    }

    pub fn forbid(mut self, key: &str) -> Self {
        self.forbidden_keys.push(key.to_string());
        self
    }

    pub fn evaluate(&self, context: &std::collections::HashMap<String, String>) -> bool {
        for key in &self.required_keys {
            if !context.contains_key(key) {
                return false;
            }
        }
        for key in &self.forbidden_keys {
            if context.contains_key(key) {
                return false;
            }
        }
        true
    }

    pub fn missing_requirements(&self, context: &std::collections::HashMap<String, String>) -> Vec<String> {
        self.required_keys
            .iter()
            .filter(|k| !context.contains_key(k.as_str()))
            .cloned()
            .collect()
    }
}

// ---------------------------------------------------------------------------
// CommandUndoStack
// ---------------------------------------------------------------------------

/// Undo/redo stack with descriptions.
#[derive(Debug, Clone)]
pub struct CommandUndoStack {
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
    max_size: usize,
}

impl CommandUndoStack {
    pub fn new(max_size: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size,
        }
    }

    pub fn push_undoable(&mut self, description: String) {
        if self.undo_stack.len() >= self.max_size {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(description);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> Option<String> {
        if let Some(desc) = self.undo_stack.pop() {
            self.redo_stack.push(desc.clone());
            Some(desc)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<String> {
        if let Some(desc) = self.redo_stack.pop() {
            self.undo_stack.push(desc.clone());
            Some(desc)
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_description(&self) -> Option<&str> {
        self.undo_stack.last().map(|s| s.as_str())
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}


// ---------------------------------------------------------------------------
// wb_commands – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XWbCommandsLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XWbCommandsPanelState {
    pub region: XWbCommandsLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XWbCommandsPanelState {
    pub fn new(region: XWbCommandsLayoutRegion, label: impl Into<String>) -> Self {
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
pub fn x_wb_commands_total_visible_area(panels: &[XWbCommandsPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_wb_commands_count_in_region(
    panels: &[XWbCommandsPanelState],
    region: XWbCommandsLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_wb_commands_widest_panel(panels: &[XWbCommandsPanelState]) -> Option<&XWbCommandsPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_wb_commands_collapse_region(
    panels: &mut [XWbCommandsPanelState],
    region: XWbCommandsLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XWbCommandsLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XWbCommandsLayoutConstraint {
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



// ---------------------------------------------------------------------------
// wb_commands – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for workbench command palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWbCommandsCommandCategory {
    Editor,
    View,
    File,
    Debug,
}

impl YWbCommandsCommandCategory {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Editor => 0,
            Self::View => 1,
            Self::File => 2,
            Self::Debug => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Editor => "Editor",
            Self::View => "View",
            Self::File => "File",
            Self::Debug => "Debug",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWbCommandsCommandCategory] {
        &[
            YWbCommandsCommandCategory::Editor,
            YWbCommandsCommandCategory::View,
            YWbCommandsCommandCategory::File,
            YWbCommandsCommandCategory::Debug,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWbCommandsCommandCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks command history data.
#[derive(Debug, Clone)]
pub struct YWbCommandsCommandHistory {
    pub commands: Vec<String>,
    pub max_size: usize,
    pub exec_count: u64,
}

impl YWbCommandsCommandHistory {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            max_size: 0,
            exec_count: 0,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWbCommandsCommandHistory({}: {:?})", "commands", self.commands)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_wb_commands_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_wb_commands_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_wb_commands_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_wb_commands_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_wb_commands_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_wb_commands_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_wb_commands_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_wb_commands_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// wb_commands – Extended command alias helpers
// ---------------------------------------------------------------------------

/// Priority levels for command alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWbCommandsPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWbCommandsPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZWbCommandsPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWbCommandsPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks command alias data.
#[derive(Debug, Clone)]
pub struct ZWbCommandsCommandAlias {
    pub mappings: Vec<(String, String)>,
    pub enabled: bool,
    pub scope: String,
}

impl ZWbCommandsCommandAlias {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            mappings: Vec::new(),
            enabled: false,
            scope: String::new(),
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.mappings.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWbCommandsCommandAlias[enabled={:?}, scope={:?}]", self.enabled, self.scope)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for command alias.
pub fn z_wb_commands_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_wb_commands_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_wb_commands_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_wb_commands_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_wb_commands_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_wb_commands_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_wb_commands_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 103
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer103 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer103 {
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
pub fn xb_fnv1a_103(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_103<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_103<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_103(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_103(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 204
// ---------------------------------------------------------------------------

/// Generic object pool `Xc204Pool<T>`.
pub struct Xc204Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc204Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc204PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc204Pool<T> {
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
    pub fn stats(&self) -> Xc204PoolStats {
        Xc204PoolStats {
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

impl<T> Default for Xc204Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc204Scheduler`.
pub struct Xc204Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc204Scheduler {
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

impl Default for Xc204Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_204 hash for the given byte slice.
pub fn xc_204_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_204 convention.
pub fn xc_204_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe116 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe116Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe116PipelineError {
    pub stage: Xe116Stage,
    pub message: String,
}

impl std::fmt::Display for Xe116PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe116Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe116Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe116PipelineError>>>,
    stage_names: Vec<Xe116Stage>,
}

impl Xe116Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe116PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe116Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe116PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe116Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe116PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe116Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe116PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe116Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe116PipelineError> {
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

    pub fn compose(mut self, other: Xe116Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe116CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe116CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe116Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe116CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe116CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe116Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe116CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_116_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe116CacheEntry {
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

    fn xe_116_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe116CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_116_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe116PipelineError> {
    Ok(data)
}

pub fn xe_116_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe116PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_116_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe116PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_116_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe116PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_116_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe116PipelineError> {
    Err(Xe116PipelineError {
        stage: Xe116Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_114: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg114Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg114Graph {
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

impl Default for Xg114Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_114: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg114Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg114Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg114Heap<T>) {
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

impl<T: Ord> Default for Xg114Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 203).
pub struct Xh203SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh203SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 245 as u64,
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

/// A compact bit set supporting boolean operations (variant 203).
pub struct Xh203BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh203BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 203).
pub struct Xi203Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi203Deque<T> {
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
pub struct Xi203Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi203Interval {
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

/// A simple interval tree (variant 203).
pub struct Xi203IntervalTree {
    xi_intervals: Vec<Xi203Interval>,
}

impl Xi203IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi203Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi203Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi203Interval) -> Vec<&Xi203Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi203Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi203Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi203Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi203Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi203Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi203Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 203) ---

/// Disjoint set / union-find for crate 203.
pub struct Xj203UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj203UnionFind {
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

const XJ203_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 203.
pub struct Xj203BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj203BTreeNode<K, V>>>,
    len: usize,
}

struct Xj203BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj203BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj203BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ203_BTREE_ORDER - 1
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
        let mid = XJ203_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj203BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj203BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj203BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj203BTreeNode::xj_new_leaf();
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


// --- xk_203 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk203SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk203SegmentTree {
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
pub struct Xk203DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk203DisjointIntervals {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cmd(id: &str, title: &str) -> CommandDescriptor {
        CommandDescriptor {
            id: id.to_string(),
            title: title.to_string(),
            category: None,
            source: CommandSource::System,
            enabled: true,
            keybinding: None,
        }
    }

    fn make_cmd_with_category(id: &str, title: &str, category: &str) -> CommandDescriptor {
        CommandDescriptor {
            id: id.to_string(),
            title: title.to_string(),
            category: Some(category.to_string()),
            source: CommandSource::System,
            enabled: true,
            keybinding: None,
        }
    }

    #[test]
    fn register_and_get() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("editor.action.format", "Format Document"));
        assert_eq!(reg.command_count(), 1);
        assert!(reg.get_command("editor.action.format").is_some());
        assert!(reg.get_command("missing").is_none());
    }

    #[test]
    fn unregister_works() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("cmd1", "Command 1"));
        assert!(reg.unregister("cmd1"));
        assert!(!reg.unregister("cmd1"));
        assert_eq!(reg.command_count(), 0);
    }

    #[test]
    fn find_commands_works() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("editor.format", "Format Document"));
        reg.register(make_cmd("editor.save", "Save File"));
        reg.register(make_cmd("view.zoom", "Zoom In"));
        let results = reg.find_commands("format");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "editor.format");
        let results = reg.find_commands("EDITOR");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn builder_defaults() {
        let cmd = CommandDescriptor::builder("test.cmd", "Test Command").build();
        assert_eq!(cmd.id, "test.cmd");
        assert_eq!(cmd.title, "Test Command");
        assert_eq!(cmd.category, None);
        assert_eq!(cmd.source, CommandSource::System);
        assert!(cmd.enabled);
        assert_eq!(cmd.keybinding, None);
    }

    #[test]
    fn builder_with_all_fields() {
        let cmd = CommandDescriptor::builder("edit.copy", "Copy")
            .category("Edit")
            .source(CommandSource::Keybinding)
            .enabled(false)
            .keybinding("Ctrl+C")
            .build();
        assert_eq!(cmd.category.as_deref(), Some("Edit"));
        assert_eq!(cmd.source, CommandSource::Keybinding);
        assert!(!cmd.enabled);
        assert_eq!(cmd.keybinding.as_deref(), Some("Ctrl+C"));
    }

    #[test]
    fn get_by_category_works() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd_with_category("editor.format", "Format", "Editor"));
        reg.register(make_cmd_with_category("editor.save", "Save", "Editor"));
        reg.register(make_cmd_with_category("view.zoom", "Zoom", "View"));
        reg.register(make_cmd("misc.noop", "No-op"));
        let editor_cmds = reg.get_by_category("Editor");
        assert_eq!(editor_cmds.len(), 2);
        let view_cmds = reg.get_by_category("View");
        assert_eq!(view_cmds.len(), 1);
        let none_cmds = reg.get_by_category("Missing");
        assert!(none_cmds.is_empty());
    }

    #[test]
    fn set_enabled_and_get_enabled() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("cmd.a", "A"));
        reg.register(make_cmd("cmd.b", "B"));
        assert_eq!(reg.get_enabled_commands().len(), 2);
        assert!(reg.set_enabled("cmd.a", false));
        assert_eq!(reg.get_enabled_commands().len(), 1);
        assert_eq!(reg.get_enabled_commands()[0].id, "cmd.b");
        assert!(!reg.set_enabled("nonexistent", false));
    }

    #[test]
    fn has_command_works() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("cmd.exists", "Exists"));
        assert!(reg.has_command("cmd.exists"));
        assert!(!reg.has_command("cmd.missing"));
    }

    #[test]
    fn try_register_rejects_duplicate() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("dup.id", "First"));
        let result = reg.try_register(make_cmd("dup.id", "Second"));
        assert_eq!(result, Err(CommandError::DuplicateId("dup.id".into())));
        assert_eq!(reg.command_count(), 1);
    }

    #[test]
    fn try_register_rejects_invalid_id() {
        let mut reg = CommandRegistry::new();
        let result = reg.try_register(make_cmd("has space", "Bad"));
        assert_eq!(
            result,
            Err(CommandError::InvalidId("has space".into()))
        );
        let result = reg.try_register(make_cmd("", "Empty Id"));
        assert_eq!(result, Err(CommandError::InvalidId("".into())));
    }

    #[test]
    fn try_build_validates_id_and_title() {
        let ok = CommandDescriptor::builder("valid.id", "Title").try_build();
        assert!(ok.is_ok());

        let bad_id = CommandDescriptor::builder("bad id", "Title").try_build();
        assert_eq!(bad_id, Err(CommandError::InvalidId("bad id".into())));

        let empty_title = CommandDescriptor::builder("ok.id", "").try_build();
        assert_eq!(empty_title, Err(CommandError::EmptyTitle));
    }

    #[test]
    fn is_valid_id_rules() {
        assert!(CommandDescriptor::is_valid_id("editor.format"));
        assert!(CommandDescriptor::is_valid_id("a"));
        assert!(!CommandDescriptor::is_valid_id(""));
        assert!(!CommandDescriptor::is_valid_id("has space"));
        assert!(!CommandDescriptor::is_valid_id("tab\there"));
    }

    #[test]
    fn qualified_id_with_and_without_category() {
        let with_cat = CommandDescriptor::builder("save", "Save")
            .category("File")
            .build();
        assert_eq!(with_cat.qualified_id(), "File.save");

        let no_cat = CommandDescriptor::builder("save", "Save").build();
        assert_eq!(no_cat.qualified_id(), "save");
    }

    #[test]
    fn display_impls() {
        let cmd = CommandDescriptor::builder("edit.copy", "Copy")
            .category("Edit")
            .build();
        assert_eq!(cmd.to_string(), "Edit: Copy (edit.copy)");

        let cmd2 = CommandDescriptor::builder("misc.noop", "No-op").build();
        assert_eq!(cmd2.to_string(), "No-op (misc.noop)");

        assert_eq!(CommandSource::User.to_string(), "User");
        assert_eq!(CommandSource::Extension.to_string(), "Extension");

        let mut reg = CommandRegistry::new();
        assert_eq!(reg.to_string(), "CommandRegistry(0 commands, 0 enabled)");
        reg.register(make_cmd("a", "A"));
        assert_eq!(reg.to_string(), "CommandRegistry(1 commands, 1 enabled)");
    }

    #[test]
    fn categories_returns_sorted_unique() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd_with_category("a", "A", "View"));
        reg.register(make_cmd_with_category("b", "B", "Edit"));
        reg.register(make_cmd_with_category("c", "C", "View"));
        reg.register(make_cmd("d", "D"));
        let cats = reg.categories();
        assert_eq!(cats, vec!["Edit", "View"]);
    }

    #[test]
    fn get_by_source_works() {
        let mut reg = CommandRegistry::new();
        reg.register(
            CommandDescriptor::builder("a", "A")
                .source(CommandSource::User)
                .build(),
        );
        reg.register(
            CommandDescriptor::builder("b", "B")
                .source(CommandSource::Extension)
                .build(),
        );
        reg.register(
            CommandDescriptor::builder("c", "C")
                .source(CommandSource::User)
                .build(),
        );
        assert_eq!(reg.get_by_source(CommandSource::User).len(), 2);
        assert_eq!(reg.get_by_source(CommandSource::Extension).len(), 1);
        assert_eq!(reg.get_by_source(CommandSource::Keybinding).len(), 0);
    }

    #[test]
    fn disable_and_enable_all() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("a", "A"));
        reg.register(make_cmd("b", "B"));
        reg.set_enabled("a", false);

        let already_disabled = reg.disable_all();
        assert_eq!(already_disabled, 1);
        assert_eq!(reg.get_enabled_commands().len(), 0);

        let already_enabled = reg.enable_all();
        assert_eq!(already_enabled, 0);
        assert_eq!(reg.get_enabled_commands().len(), 2);
    }

    #[test]
    fn command_error_display() {
        let e = CommandError::NotFound("x".into());
        assert_eq!(e.to_string(), "command not found: 'x'");
        let e2 = CommandError::EmptyTitle;
        assert_eq!(e2.to_string(), "command title must not be empty");
    }

    #[test]
    fn command_descriptor_equality() {
        let a = CommandDescriptor::builder("id", "Title").build();
        let b = CommandDescriptor::builder("id", "Title").build();
        assert_eq!(a, b);

        let c = CommandDescriptor::builder("id", "Other").build();
        assert_ne!(a, c);
    }

    // ── CommandExecutionLog tests ──────────────────────────────────

    #[test]
    fn execution_log_records_and_counts() {
        let mut log = CommandExecutionLog::new();
        assert_eq!(log.total_count(), 0);
        let seq1 = log.log("editor.save", CommandSource::User);
        let seq2 = log.log("editor.format", CommandSource::Keybinding);
        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        assert_eq!(log.total_count(), 2);
    }

    #[test]
    fn execution_log_get_recent_and_clear() {
        let mut log = CommandExecutionLog::new();
        log.log("a", CommandSource::System);
        log.log("b", CommandSource::System);
        log.log("c", CommandSource::System);

        let recent = log.get_recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].command_id, "b");
        assert_eq!(recent[1].command_id, "c");

        // Requesting more than available returns all.
        assert_eq!(log.get_recent(100).len(), 3);

        log.clear();
        assert_eq!(log.total_count(), 0);
        assert!(log.get_recent(5).is_empty());
    }

    #[test]
    fn execution_log_get_by_command() {
        let mut log = CommandExecutionLog::new();
        log.log("editor.save", CommandSource::User);
        log.log("editor.format", CommandSource::User);
        log.log("editor.save", CommandSource::Keybinding);

        let saves = log.get_by_command("editor.save");
        assert_eq!(saves.len(), 2);
        assert_eq!(saves[0].source, CommandSource::User);
        assert_eq!(saves[1].source, CommandSource::Keybinding);
        assert!(log.get_by_command("nonexistent").is_empty());
    }

    // ── CommandBatch tests ────────────────────────────────────────

    #[test]
    fn batch_execute_all_succeed() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("cmd.a", "A"));
        reg.register(make_cmd("cmd.b", "B"));

        let mut batch = CommandBatch::new();
        batch.push("cmd.a");
        batch.push("cmd.b");
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());

        let results = batch.execute(&reg);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1, BatchStepResult::Ok);
        assert_eq!(results[1].1, BatchStepResult::Ok);
        assert!(CommandBatch::rollback_ids(&results).is_empty() == false);
    }

    #[test]
    fn batch_stops_on_failure_and_provides_rollback() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("cmd.a", "A"));
        // cmd.b intentionally missing
        reg.register(make_cmd("cmd.c", "C"));

        let mut batch = CommandBatch::new();
        batch.push("cmd.a");
        batch.push("cmd.b"); // will fail
        batch.push("cmd.c"); // should never be reached

        let results = batch.execute(&reg);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1, BatchStepResult::Ok);
        assert!(matches!(results[1].1, BatchStepResult::Failed(CommandError::NotFound(_))));

        let rollback = CommandBatch::rollback_ids(&results);
        assert_eq!(rollback, vec!["cmd.a"]);
    }

    // ── CommandValidator tests ────────────────────────────────────

    #[test]
    fn validator_requires_dot_separator() {
        assert!(CommandValidator::validate_command_id("editor.save"));
        assert!(CommandValidator::validate_command_id("a.b.c"));
        // Valid per is_valid_id but missing dot → rejected by validator.
        assert!(!CommandValidator::validate_command_id("nodot"));
        assert!(!CommandValidator::validate_command_id(""));
        assert!(!CommandValidator::validate_command_id("has space.cmd"));
    }

    #[test]
    fn validator_first_invalid() {
        let ids = vec!["editor.save", "view.zoom"];
        assert_eq!(CommandValidator::first_invalid(&ids), None);

        let ids_bad = vec!["editor.save", "bad", "view.zoom"];
        assert_eq!(CommandValidator::first_invalid(&ids_bad), Some("bad".into()));
    }

    // ── CommandHistory tests ──

    #[test]
    fn history_push_and_count() {
        let mut h = CommandHistory::new();
        assert_eq!(h.count(), 0);
        h.push("editor.save", CommandSource::User);
        h.push("editor.format", CommandSource::Keybinding);
        assert_eq!(h.count(), 2);
    }

    #[test]
    fn history_recent() {
        let mut h = CommandHistory::new();
        h.push("a", CommandSource::System);
        h.push("b", CommandSource::System);
        h.push("c", CommandSource::System);
        let recent = h.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].1, "b");
        assert_eq!(recent[1].1, "c");
    }

    #[test]
    fn history_clear() {
        let mut h = CommandHistory::new();
        h.push("x", CommandSource::User);
        h.clear();
        assert_eq!(h.count(), 0);
        assert!(h.recent(10).is_empty());
    }

    // ── CommandPalette tests ──

    #[test]
    fn palette_search_filters_and_sorts() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("editor.format", "Format Document"));
        reg.register(make_cmd("editor.save", "Save File"));
        reg.register(make_cmd("view.zoom", "Zoom In"));
        let palette = CommandPalette::new(&reg);
        let results = palette.search("format");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "editor.format");
    }

    #[test]
    fn palette_excludes_disabled() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("a", "Alpha"));
        reg.register(make_cmd("b", "Beta"));
        reg.set_enabled("a", false);
        let palette = CommandPalette::new(&reg);
        let all = palette.all_enabled();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "b");
    }

    #[test]
    fn palette_all_enabled_sorted() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("z", "Zebra"));
        reg.register(make_cmd("a", "Apple"));
        let palette = CommandPalette::new(&reg);
        let all = palette.all_enabled();
        assert_eq!(all[0].title, "Apple");
        assert_eq!(all[1].title, "Zebra");
    }

    // ── CommandPaletteItem / search / history tests ──

    #[test]
    fn palette_item_display_label_with_category() {
        let item = CommandPaletteItem::new("editor.action.format", "Format Document")
            .with_category("Editor");
        assert_eq!(item.display_label(), "Editor: Format Document");
    }

    #[test]
    fn palette_item_display_label_without_category() {
        let item = CommandPaletteItem::new("workbench.action.quit", "Quit");
        assert_eq!(item.display_label(), "Quit");
    }

    #[test]
    fn palette_item_display_with_keybinding() {
        let item = CommandPaletteItem::new("editor.action.format", "Format")
            .with_keybinding("Shift+Alt+F");
        let s = format!("{}", item);
        assert!(s.contains("Format"));
        assert!(s.contains("Shift+Alt+F"));
    }

    #[test]
    fn palette_item_from_descriptor() {
        let desc = CommandDescriptor::builder("cmd.test", "Test Command")
            .category("Testing")
            .keybinding("Ctrl+T")
            .build();
        let item = CommandPaletteItem::from_descriptor(&desc);
        assert_eq!(item.command_id, "cmd.test");
        assert_eq!(item.category.as_deref(), Some("Testing"));
        assert_eq!(item.keybinding.as_deref(), Some("Ctrl+T"));
    }

    #[test]
    fn palette_search_exact_substring() {
        let items = vec![
            CommandPaletteItem::new("a", "Format Document"),
            CommandPaletteItem::new("b", "Open File"),
            CommandPaletteItem::new("c", "Format Selection"),
        ];
        let results = command_palette_search(&items, "Format");
        assert_eq!(results.len(), 2);
        assert!(results[0].item.title.contains("Format"));
    }

    #[test]
    fn palette_search_fuzzy() {
        let items = vec![
            CommandPaletteItem::new("a", "Format Document"),
            CommandPaletteItem::new("b", "Open File"),
        ];
        let results = command_palette_search(&items, "fmtdoc");
        // Should fuzzy-match "Format Document"
        assert!(results.iter().any(|r| r.item.command_id == "a"));
    }

    #[test]
    fn palette_search_empty_query_returns_all() {
        let items = vec![
            CommandPaletteItem::new("a", "A"),
            CommandPaletteItem::new("b", "B"),
        ];
        let results = command_palette_search(&items, "");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn palette_history_record_and_recent() {
        let mut history = CommandPaletteHistory::new(3);
        history.record("cmd.a");
        history.record("cmd.b");
        history.record("cmd.c");
        assert_eq!(history.recent(), &["cmd.c", "cmd.b", "cmd.a"]);
    }

    #[test]
    fn palette_history_moves_to_front() {
        let mut history = CommandPaletteHistory::new(5);
        history.record("cmd.a");
        history.record("cmd.b");
        history.record("cmd.a"); // should move to front
        assert_eq!(history.recent()[0], "cmd.a");
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn palette_history_max_size() {
        let mut history = CommandPaletteHistory::new(2);
        history.record("cmd.a");
        history.record("cmd.b");
        history.record("cmd.c");
        assert_eq!(history.len(), 2);
        assert!(!history.contains("cmd.a"));
    }

    #[test]
    fn palette_history_boost_results() {
        let mut history = CommandPaletteHistory::new(10);
        history.record("b");
        let items = vec![
            CommandPaletteItem::new("a", "Alpha"),
            CommandPaletteItem::new("b", "Beta"),
        ];
        let mut results = command_palette_search(&items, "");
        history.boost_results(&mut results);
        assert_eq!(results[0].item.command_id, "b");
    }

    #[test]
    fn palette_history_clear() {
        let mut history = CommandPaletteHistory::new(10);
        history.record("x");
        history.clear();
        assert!(history.is_empty());
    }

    // ---- CommandChain tests ----

    #[test]
    fn chain_execute_all_success() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("save", "Save"));
        reg.register(make_cmd("format", "Format"));
        let mut chain = CommandChain::new("save-format");
        chain.then("format").then("save");
        assert!(chain.execute(&reg));
        assert_eq!(chain.success_count(), 2);
    }

    #[test]
    fn chain_stops_on_missing_command() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("save", "Save"));
        let mut chain = CommandChain::new("test");
        chain.then("missing").then("save");
        assert!(!chain.execute(&reg));
        assert_eq!(chain.success_count(), 0);
    }

    #[test]
    fn chain_continue_on_error() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("save", "Save"));
        let mut chain = CommandChain::new("test");
        chain.then_optional("missing").then("save");
        assert!(chain.execute(&reg));
        assert_eq!(chain.success_count(), 1);
        assert_eq!(chain.step_count(), 2);
    }

    // ---- CommandThrottle tests ----

    #[test]
    fn throttle_allows_first_call() {
        let mut throttle = CommandThrottle::new(100);
        assert!(throttle.try_execute("save", 0));
    }

    #[test]
    fn throttle_blocks_rapid_calls() {
        let mut throttle = CommandThrottle::new(100);
        assert!(throttle.try_execute("save", 0));
        assert!(!throttle.try_execute("save", 50));
        assert!(throttle.try_execute("save", 100));
    }

    #[test]
    fn throttle_independent_commands() {
        let mut throttle = CommandThrottle::new(100);
        assert!(throttle.try_execute("save", 0));
        assert!(throttle.try_execute("format", 0));
    }

    // ---- CommandGroup tests ----

    #[test]
    fn group_add_and_contains() {
        let mut group = CommandGroup::new("edit", "Editing commands");
        group.add("copy");
        group.add("paste");
        group.add("copy"); // duplicate ignored
        assert_eq!(group.len(), 2);
        assert!(group.contains("copy"));
        assert!(!group.contains("cut"));
    }

    #[test]
    fn group_remove() {
        let mut group = CommandGroup::new("edit", "Editing");
        group.add("copy");
        group.add("paste");
        assert!(group.remove("copy"));
        assert!(!group.contains("copy"));
        assert!(!group.remove("nonexistent"));
    }

    #[test]
    fn group_resolve_against_registry() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("copy", "Copy"));
        reg.register(make_cmd("paste", "Paste"));
        let mut group = CommandGroup::new("edit", "Edit");
        group.add("copy");
        group.add("paste");
        group.add("missing");
        let resolved = group.resolve(&reg);
        assert_eq!(resolved.len(), 2);
    }

    #[test]
    fn registry_stats_empty() {
        let reg = CommandRegistry::new();
        let s = reg.stats();
        assert_eq!(s.total, 0);
        assert_eq!(s.enabled, 0);
        assert_eq!(s.disabled, 0);
        assert_eq!(s.category_count, 0);
    }

    #[test]
    fn registry_stats_with_mixed_commands() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("a", "A"));
        let mut cmd_b = make_cmd("b", "B");
        cmd_b.enabled = false;
        reg.register(cmd_b);
        let s = reg.stats();
        assert_eq!(s.total, 2);
        assert_eq!(s.enabled, 1);
        assert_eq!(s.disabled, 1);
    }

    #[test]
    fn find_by_prefix_matches() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("editor.copy", "Copy"));
        reg.register(make_cmd("editor.paste", "Paste"));
        reg.register(make_cmd("file.save", "Save"));
        let results = reg.find_by_prefix("editor.");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn find_by_prefix_no_match() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("file.save", "Save"));
        let results = reg.find_by_prefix("editor.");
        assert!(results.is_empty());
    }

    #[test]
    fn disabled_command_ids_returns_only_disabled() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("a", "A"));
        let mut cmd_b = make_cmd("b", "B");
        cmd_b.enabled = false;
        reg.register(cmd_b);
        let ids = reg.disabled_command_ids();
        assert_eq!(ids, vec!["b"]);
    }

    #[test]
    fn rename_title_updates_existing() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("x", "Old"));
        assert!(reg.rename_title("x", "New"));
        assert_eq!(reg.get_command("x").unwrap().title, "New");
    }

    #[test]
    fn rename_title_returns_false_for_missing() {
        let mut reg = CommandRegistry::new();
        assert!(!reg.rename_title("nope", "New"));
    }

    #[test]
    fn sorted_by_title_alphabetical() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("c", "Zebra"));
        reg.register(make_cmd("a", "Apple"));
        reg.register(make_cmd("b", "Mango"));
        let sorted = reg.sorted_by_title();
        assert_eq!(sorted[0].title, "Apple");
        assert_eq!(sorted[1].title, "Mango");
        assert_eq!(sorted[2].title, "Zebra");
    }

    #[test]
    fn execution_log_most_frequent() {
        let mut log = CommandExecutionLog::new();
        log.log("a", CommandSource::User);
        log.log("b", CommandSource::User);
        log.log("a", CommandSource::Keybinding);
        let (id, count) = log.most_frequent().unwrap();
        assert_eq!(id, "a");
        assert_eq!(count, 2);
    }

    #[test]
    fn execution_log_most_frequent_empty() {
        let log = CommandExecutionLog::new();
        assert!(log.most_frequent().is_none());
    }

    #[test]
    fn execution_log_distinct_command_ids() {
        let mut log = CommandExecutionLog::new();
        log.log("a", CommandSource::User);
        log.log("b", CommandSource::User);
        log.log("a", CommandSource::Keybinding);
        let ids = log.distinct_command_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"a".to_string()));
        assert!(ids.contains(&"b".to_string()));
    }

    // ── CommandAliasMap tests ──

    #[test]
    fn alias_set_resolve_and_identity() {
        let mut aliases = CommandAliasMap::new();
        assert!(aliases.set("fmt", "editor.action.formatDocument").is_none());
        assert_eq!(
            aliases.resolve("fmt"),
            Some("editor.action.formatDocument")
        );
        assert_eq!(aliases.resolve("unknown"), None);
        assert_eq!(
            aliases.resolve_or_identity("fmt"),
            "editor.action.formatDocument"
        );
        assert_eq!(aliases.resolve_or_identity("literal.id"), "literal.id");
    }

    #[test]
    fn alias_overwrite_returns_previous() {
        let mut aliases = CommandAliasMap::new();
        aliases.set("fmt", "editor.format.v1");
        let prev = aliases.set("fmt", "editor.format.v2");
        assert_eq!(prev, Some("editor.format.v1".to_string()));
        assert_eq!(aliases.resolve("fmt"), Some("editor.format.v2"));
    }

    #[test]
    fn alias_remove_and_contains() {
        let mut aliases = CommandAliasMap::new();
        aliases.set("s", "editor.save");
        assert!(aliases.contains("s"));
        assert!(aliases.remove("s"));
        assert!(!aliases.contains("s"));
        assert!(!aliases.remove("s"));
    }

    #[test]
    fn alias_reverse_lookup() {
        let mut aliases = CommandAliasMap::new();
        aliases.set("fmt", "editor.format");
        aliases.set("format", "editor.format");
        aliases.set("save", "editor.save");
        let mut found = aliases.aliases_for("editor.format");
        found.sort();
        assert_eq!(found, vec!["fmt", "format"]);
        assert!(aliases.aliases_for("nonexistent").is_empty());
    }

    // ── KeybindingResolver tests ──

    #[test]
    fn keybinding_normalise_sorts_modifiers() {
        assert_eq!(
            KeybindingResolver::normalise_keys("Shift+Ctrl+P"),
            "ctrl+shift+p"
        );
        assert_eq!(
            KeybindingResolver::normalise_keys("Alt+Shift+F"),
            "alt+shift+f"
        );
        assert_eq!(KeybindingResolver::normalise_keys("A"), "a");
    }

    #[test]
    fn keybinding_resolve_basic() {
        let mut resolver = KeybindingResolver::new();
        resolver.add("Ctrl+S", "editor.save", None);
        resolver.add("Ctrl+Shift+P", "workbench.commandPalette", None);
        let matches = resolver.resolve("ctrl+s");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].command_id, "editor.save");
        assert!(resolver.resolve("ctrl+z").is_empty());
    }

    #[test]
    fn keybinding_resolve_with_context() {
        let mut resolver = KeybindingResolver::new();
        resolver.add("Ctrl+D", "editor.addSelection", Some("editorFocus"));
        resolver.add("Ctrl+D", "terminal.sendSequence", Some("terminalFocus"));

        // editorFocus is true
        let entry = resolver
            .resolve_with_context("Ctrl+D", |clause| clause == "editorFocus")
            .unwrap();
        assert_eq!(entry.command_id, "editor.addSelection");

        // terminalFocus is true
        let entry = resolver
            .resolve_with_context("Ctrl+D", |clause| clause == "terminalFocus")
            .unwrap();
        assert_eq!(entry.command_id, "terminal.sendSequence");

        // nothing matches
        let entry = resolver.resolve_with_context("Ctrl+D", |_| false);
        assert!(entry.is_none());
    }

    #[test]
    fn keybinding_remove_command() {
        let mut resolver = KeybindingResolver::new();
        resolver.add("Ctrl+S", "editor.save", None);
        resolver.add("Ctrl+Shift+S", "editor.save", None);
        resolver.add("Ctrl+Z", "editor.undo", None);
        let removed = resolver.remove_command("editor.save");
        assert_eq!(removed, 2);
        assert!(resolver.bindings_for("editor.save").is_empty());
        assert_eq!(resolver.len(), 1);
    }

    // ── CommandArgs tests ──

    #[test]
    fn args_parse_kv_types() {
        let (k, v) = CommandArgs::parse_kv("line=42").unwrap();
        assert_eq!(k, "line");
        assert_eq!(v, ArgValue::Int(42));

        let (_, v) = CommandArgs::parse_kv("verbose=true").unwrap();
        assert_eq!(v, ArgValue::Bool(true));

        let (_, v) = CommandArgs::parse_kv("name=hello").unwrap();
        assert_eq!(v, ArgValue::String("hello".to_string()));

        let (_, v) = CommandArgs::parse_kv("ratio=3.14").unwrap();
        assert_eq!(v, ArgValue::Float(3.14));

        let (_, v) = CommandArgs::parse_kv("val=null").unwrap();
        assert_eq!(v, ArgValue::Null);

        assert!(CommandArgs::parse_kv("no-equals").is_none());
    }

    #[test]
    fn args_parse_many_and_accessors() {
        let args = CommandArgs::parse_many("file=main.rs line=10 force=true");
        assert_eq!(args.len(), 3);
        assert_eq!(args.get_str("file"), Some("main.rs"));
        assert_eq!(args.get_int("line"), Some(10));
        assert_eq!(args.get_bool("force"), Some(true));
        assert_eq!(args.get_str("missing"), None);
        assert!(args.contains_key("file"));
        assert!(!args.contains_key("missing"));
    }

    // ── WhenClauseContext tests ──

    #[test]
    fn when_clause_simple_key() {
        let mut ctx = WhenClauseContext::new();
        ctx.set("editorFocus", true);
        assert!(ctx.evaluate("editorFocus"));
        assert!(!ctx.evaluate("terminalFocus"));
    }

    #[test]
    fn when_clause_negation() {
        let mut ctx = WhenClauseContext::new();
        ctx.set("editorFocus", true);
        assert!(!ctx.evaluate("!editorFocus"));
        assert!(ctx.evaluate("!terminalFocus"));
    }

    #[test]
    fn when_clause_and_or() {
        let mut ctx = WhenClauseContext::new();
        ctx.set("editorFocus", true);
        ctx.set("editorHasSelection", true);
        ctx.set("terminalFocus", false);

        // AND: both true
        assert!(ctx.evaluate("editorFocus && editorHasSelection"));
        // AND: one false
        assert!(!ctx.evaluate("editorFocus && terminalFocus"));
        // OR: one true
        assert!(ctx.evaluate("editorFocus || terminalFocus"));
        // OR: both false
        assert!(!ctx.evaluate("terminalFocus || nonexistent"));
    }

    #[test]
    fn when_clause_empty_is_true() {
        let ctx = WhenClauseContext::new();
        assert!(ctx.evaluate(""));
        assert!(ctx.evaluate("   "));
    }

    // ── CommandInvocation tests ──

    #[test]
    fn invocation_parse_with_args() {
        let inv = CommandInvocation::parse(
            "editor.goto line=42 col=10",
            CommandSource::User,
        )
        .unwrap();
        assert_eq!(inv.command_id, "editor.goto");
        assert_eq!(inv.args.get_int("line"), Some(42));
        assert_eq!(inv.args.get_int("col"), Some(10));
        assert_eq!(inv.source, CommandSource::User);
    }

    #[test]
    fn invocation_parse_no_args() {
        let inv =
            CommandInvocation::parse("editor.save", CommandSource::Keybinding).unwrap();
        assert_eq!(inv.command_id, "editor.save");
        assert!(inv.args.is_empty());
    }

    #[test]
    fn invocation_parse_empty_returns_none() {
        assert!(CommandInvocation::parse("", CommandSource::User).is_none());
        assert!(CommandInvocation::parse("   ", CommandSource::User).is_none());
    }

    #[test]
    fn invocation_validate_against_registry() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("editor.save", "Save"));
        let mut disabled = make_cmd("editor.close", "Close");
        disabled.enabled = false;
        reg.register(disabled);

        let ok = CommandInvocation::new("editor.save", CommandSource::User);
        assert!(ok.validate(&reg).is_ok());

        let missing = CommandInvocation::new("nope", CommandSource::User);
        assert_eq!(
            missing.validate(&reg),
            Err(CommandError::NotFound("nope".into()))
        );

        let dis = CommandInvocation::new("editor.close", CommandSource::User);
        assert_eq!(
            dis.validate(&reg),
            Err(CommandError::Disabled("editor.close".into()))
        );
    }

    #[test]
    fn invocation_display() {
        let inv = CommandInvocation::new("editor.save", CommandSource::Keybinding);
        assert_eq!(inv.to_string(), "editor.save (via Keybinding)");
    }

    // -- CommandPaletteItemV2 ------------------------------------------------

    #[test]
    fn palette_item_matches_query() {
        let item = CommandPaletteItemV2::new("Format Document").with_category("Edit");
        assert!(item.matches_query("format"));
        assert!(item.matches_query("Edit"));
        assert!(!item.matches_query("zzz"));
    }

    #[test]
    fn palette_item_display_string() {
        let item = CommandPaletteItemV2::new("Save").with_category("File");
        assert_eq!(item.display_string(), "File: Save");
    }

    #[test]
    fn palette_item_display_no_category() {
        let item = CommandPaletteItemV2::new("Save");
        assert_eq!(item.display_string(), "Save");
    }

    #[test]
    fn palette_item_sort_key() {
        let a = CommandPaletteItemV2::new("Zoom").with_category("View");
        let b = CommandPaletteItemV2::new("Close").with_category("File");
        assert!(b.sort_key() < a.sort_key());
    }

    // -- CommandContextGuard -----------------------------------------------

    #[test]
    fn context_guard_passes() {
        let guard = CommandContextGuard::new().require("editorFocus");
        let mut ctx = std::collections::HashMap::new();
        ctx.insert("editorFocus".into(), "true".into());
        assert!(guard.evaluate(&ctx));
    }

    #[test]
    fn context_guard_fails_missing() {
        let guard = CommandContextGuard::new().require("editorFocus");
        let ctx = std::collections::HashMap::new();
        assert!(!guard.evaluate(&ctx));
        assert_eq!(guard.missing_requirements(&ctx), vec!["editorFocus"]);
    }

    #[test]
    fn context_guard_forbidden() {
        let guard = CommandContextGuard::new().forbid("readOnly");
        let mut ctx = std::collections::HashMap::new();
        ctx.insert("readOnly".into(), "true".into());
        assert!(!guard.evaluate(&ctx));
    }

    // -- CommandUndoStack --------------------------------------------------

    #[test]
    fn undo_stack_basic() {
        let mut stack = CommandUndoStack::new(10);
        stack.push_undoable("type 'a'".into());
        assert!(stack.can_undo());
        assert_eq!(stack.undo_description(), Some("type 'a'"));
        let desc = stack.undo();
        assert_eq!(desc, Some("type 'a'".into()));
        assert!(stack.can_redo());
    }

    #[test]
    fn undo_redo_cycle() {
        let mut stack = CommandUndoStack::new(10);
        stack.push_undoable("action1".into());
        stack.push_undoable("action2".into());
        stack.undo();
        assert!(stack.can_redo());
        let redo = stack.redo();
        assert_eq!(redo, Some("action2".into()));
    }

    #[test]
    fn undo_clears_redo_on_new_action() {
        let mut stack = CommandUndoStack::new(10);
        stack.push_undoable("a".into());
        stack.undo();
        stack.push_undoable("b".into());
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_stack_clear() {
        let mut stack = CommandUndoStack::new(10);
        stack.push_undoable("x".into());
        stack.clear();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_stack_max_size() {
        let mut stack = CommandUndoStack::new(2);
        stack.push_undoable("a".into());
        stack.push_undoable("b".into());
        stack.push_undoable("c".into());
        assert_eq!(stack.undo_description(), Some("c"));
        stack.undo();
        assert_eq!(stack.undo_description(), Some("b"));
    }


    // -- wb_commands additional tests -------------------------------------------

    #[test]
    fn x_wb_commands_panel_state_new() {
        let p = XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XWbCommandsLayoutRegion::Sidebar);
    }

    #[test]
    fn x_wb_commands_panel_area() {
        let p = XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_wb_commands_panel_toggle() {
        let mut p = XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_wb_commands_panel_resize() {
        let mut p = XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_wb_commands_panel_is_narrow() {
        let mut p = XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_wb_commands_total_visible_area_basic() {
        let panels = vec![
            XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Sidebar, "a"),
            XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_wb_commands_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_wb_commands_total_visible_area_hidden() {
        let mut panels = vec![
            XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Sidebar, "a"),
            XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_wb_commands_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_wb_commands_count_in_region_basic() {
        let panels = vec![
            XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Sidebar, "a"),
            XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Sidebar, "b"),
            XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_wb_commands_count_in_region(&panels, XWbCommandsLayoutRegion::Sidebar), 2);
        assert_eq!(x_wb_commands_count_in_region(&panels, XWbCommandsLayoutRegion::Editor), 1);
        assert_eq!(x_wb_commands_count_in_region(&panels, XWbCommandsLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_wb_commands_widest_panel_basic() {
        let mut panels = vec![
            XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Sidebar, "narrow"),
            XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_wb_commands_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_wb_commands_collapse_region_basic() {
        let mut panels = vec![
            XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Sidebar, "a"),
            XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Sidebar, "b"),
            XWbCommandsPanelState::new(XWbCommandsLayoutRegion::Editor, "c"),
        ];
        x_wb_commands_collapse_region(&mut panels, XWbCommandsLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_wb_commands_layout_constraint_clamp() {
        let lc = XWbCommandsLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_wb_commands_layout_constraint_satisfied() {
        let lc = XWbCommandsLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_wb_commands_widest_panel_empty() {
        let panels: Vec<XWbCommandsPanelState> = vec![];
        assert!(x_wb_commands_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_wb_commands_layout_region_eq() {
        assert_eq!(XWbCommandsLayoutRegion::Sidebar, XWbCommandsLayoutRegion::Sidebar);
        assert_ne!(XWbCommandsLayoutRegion::Sidebar, XWbCommandsLayoutRegion::Panel);
    }


    // -- wb_commands extended domain tests ----------------------------------------

    #[test]
    fn y_wb_commands_enum_index() {
        assert_eq!(YWbCommandsCommandCategory::Editor.index(), 0);
        assert_eq!(YWbCommandsCommandCategory::View.index(), 1);
        assert_eq!(YWbCommandsCommandCategory::File.index(), 2);
        assert_eq!(YWbCommandsCommandCategory::Debug.index(), 3);
    }

    #[test]
    fn y_wb_commands_enum_label() {
        assert_eq!(YWbCommandsCommandCategory::Editor.label(), "Editor");
        assert_eq!(YWbCommandsCommandCategory::View.label(), "View");
        assert_eq!(YWbCommandsCommandCategory::File.label(), "File");
        assert_eq!(YWbCommandsCommandCategory::Debug.label(), "Debug");
    }

    #[test]
    fn y_wb_commands_enum_all() {
        let all = YWbCommandsCommandCategory::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_wb_commands_enum_is_default() {
        assert!(YWbCommandsCommandCategory::Editor.is_default());
        assert!(!YWbCommandsCommandCategory::Debug.is_default());
    }

    #[test]
    fn y_wb_commands_enum_display() {
        assert_eq!(format!("{}", YWbCommandsCommandCategory::Editor), "Editor");
    }

    #[test]
    fn y_wb_commands_struct_new() {
        let s = YWbCommandsCommandHistory::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_wb_commands_struct_clear() {
        let mut s = YWbCommandsCommandHistory::new();
        s.commands.push("test".into());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_wb_commands_fingerprint_deterministic() {
        let h1 = y_wb_commands_fingerprint("hello");
        let h2 = y_wb_commands_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_wb_commands_fingerprint("a"), y_wb_commands_fingerprint("b"));
    }

    #[test]
    fn y_wb_commands_truncate_short() {
        assert_eq!(y_wb_commands_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_wb_commands_truncate_long() {
        let r = y_wb_commands_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_wb_commands_normalize_key_basic() {
        assert_eq!(y_wb_commands_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_wb_commands_split_path_basic() {
        let parts = y_wb_commands_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_wb_commands_count_occurrences_basic() {
        assert_eq!(y_wb_commands_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_wb_commands_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_wb_commands_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_wb_commands_in_range_basic() {
        assert!(y_wb_commands_in_range(5, 1, 10));
        assert!(y_wb_commands_in_range(1, 1, 10));
        assert!(y_wb_commands_in_range(10, 1, 10));
        assert!(!y_wb_commands_in_range(0, 1, 10));
        assert!(!y_wb_commands_in_range(11, 1, 10));
    }

    #[test]
    fn y_wb_commands_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_wb_commands_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_wb_commands_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_wb_commands_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- wb_commands Z-extended tests -----------------------------------------------

    #[test]
    fn z_wb_commands_priority_weight() {
        assert_eq!(ZWbCommandsPriority::Idle.weight(), 0);
        assert_eq!(ZWbCommandsPriority::Normal.weight(), 2);
        assert_eq!(ZWbCommandsPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_wb_commands_priority_label() {
        assert_eq!(ZWbCommandsPriority::Low.label(), "low");
        assert_eq!(ZWbCommandsPriority::High.label(), "high");
    }

    #[test]
    fn z_wb_commands_priority_is_elevated() {
        assert!(!ZWbCommandsPriority::Normal.is_elevated());
        assert!(ZWbCommandsPriority::High.is_elevated());
        assert!(ZWbCommandsPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_wb_commands_priority_display() {
        assert_eq!(format!("{}", ZWbCommandsPriority::Idle), "idle");
    }

    #[test]
    fn z_wb_commands_priority_all_asc() {
        let all = ZWbCommandsPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWbCommandsPriority::Idle);
        assert_eq!(all[4], ZWbCommandsPriority::Realtime);
    }

    #[test]
    fn z_wb_commands_struct_new() {
        let s = ZWbCommandsCommandAlias::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_wb_commands_struct_toggled_clone() {
        let s = ZWbCommandsCommandAlias::new();
        let t = s.toggled_clone();
        let _ = t.scope;
    }

    #[test]
    fn z_wb_commands_rolling_hash_deterministic() {
        let h1 = z_wb_commands_rolling_hash(b"test");
        let h2 = z_wb_commands_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_wb_commands_rolling_hash(b"a"), z_wb_commands_rolling_hash(b"b"));
    }

    #[test]
    fn z_wb_commands_pad_to_basic() {
        assert_eq!(z_wb_commands_pad_to("hi", 5), "hi   ");
        assert_eq!(z_wb_commands_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_wb_commands_is_identifier_basic() {
        assert!(z_wb_commands_is_identifier("foo_bar"));
        assert!(z_wb_commands_is_identifier("abc123"));
        assert!(!z_wb_commands_is_identifier(""));
        assert!(!z_wb_commands_is_identifier("has space"));
    }

    #[test]
    fn z_wb_commands_levenshtein_basic() {
        assert_eq!(z_wb_commands_levenshtein("", ""), 0);
        assert_eq!(z_wb_commands_levenshtein("abc", "abc"), 0);
        assert_eq!(z_wb_commands_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_wb_commands_unique_words_basic() {
        let w = z_wb_commands_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_wb_commands_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_wb_commands_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_wb_commands_common_prefix_basic() {
        assert_eq!(z_wb_commands_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_wb_commands_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_wb_commands_struct_clear() {
        let mut s = ZWbCommandsCommandAlias::new();
        s.mappings.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_wb_commands_rolling_hash_empty() {
        let h = z_wb_commands_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_103_push_and_len() {
        let mut rb = super::XbRingBuffer103::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_103_overwrite() {
        let mut rb = super::XbRingBuffer103::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_103_get_out_of_bounds() {
        let rb = super::XbRingBuffer103::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_103_drain_all() {
        let mut rb = super::XbRingBuffer103::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_103_peek_front_back() {
        let mut rb = super::XbRingBuffer103::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_103_clear() {
        let mut rb = super::XbRingBuffer103::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_103_capacity() {
        let rb = super::XbRingBuffer103::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_103_basic() {
        let h = super::xb_fnv1a_103(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_103(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_103_different_inputs() {
        let h1 = super::xb_fnv1a_103(b"abc");
        let h2 = super::xb_fnv1a_103(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_103_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_103(&data);
        let dec = super::xb_rle_decode_103(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_103_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_103(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_103(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_103_values() {
        assert!((super::xb_clamp_103(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_103(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_103(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_103_values() {
        assert!((super::xb_lerp_103(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_103(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_103(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_103_wrap_around_twice() {
        let mut rb = super::XbRingBuffer103::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 204 ----

    #[test]
    fn xc_204_pool_new_empty() {
        let pool: super::Xc204Pool<i32> = super::Xc204Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_204_pool_release_acquire() {
        let mut pool = super::Xc204Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_204_pool_acquire_empty() {
        let mut pool: super::Xc204Pool<i32> = super::Xc204Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_204_pool_full() {
        let mut pool = super::Xc204Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_204_pool_drain() {
        let mut pool = super::Xc204Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_204_pool_stats() {
        let mut pool = super::Xc204Pool::new(8);
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
    fn xc_204_pool_clear() {
        let mut pool = super::Xc204Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_204_pool_shrink() {
        let mut pool = super::Xc204Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_204_pool_default() {
        let pool: super::Xc204Pool<String> = super::Xc204Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_204_pool_extend() {
        let mut pool = super::Xc204Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_204_pool_retain() {
        let mut pool = super::Xc204Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_204_scheduler_round_robin() {
        let mut sched = super::Xc204Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_204_scheduler_empty() {
        let mut sched = super::Xc204Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_204_scheduler_reset() {
        let mut sched = super::Xc204Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_204_scheduler_add_remove() {
        let mut sched = super::Xc204Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_204_scheduler_targets() {
        let sched = super::Xc204Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_204_hash_empty() {
        assert_eq!(super::xc_204_hash(b""), 5381);
    }

    #[test]
    fn xc_204_hash_data() {
        let h = super::xc_204_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_204_hash(b"hello"), h);
    }

    #[test]
    fn xc_204_reverse_str() {
        assert_eq!(super::xc_204_reverse("abc"), "cba");
        assert_eq!(super::xc_204_reverse(""), "");
    }


    #[test]
    fn xe_116_pipeline_empty() {
        let p = super::Xe116Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_116_pipeline_parse_stage() {
        let p = super::Xe116Pipeline::new()
            .add_parse(super::xe_116_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_116_pipeline_transform_double() {
        let p = super::Xe116Pipeline::new()
            .add_transform(super::xe_116_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_116_pipeline_validate_reverse() {
        let p = super::Xe116Pipeline::new()
            .add_validate(super::xe_116_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_116_pipeline_emit_filter() {
        let p = super::Xe116Pipeline::new()
            .add_emit(super::xe_116_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_116_pipeline_multi_stage() {
        let p = super::Xe116Pipeline::new()
            .add_parse(super::xe_116_pipeline_identity)
            .add_transform(super::xe_116_pipeline_double)
            .add_validate(super::xe_116_pipeline_reverse)
            .add_emit(super::xe_116_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_116_pipeline_error_propagation() {
        let p = super::Xe116Pipeline::new()
            .add_parse(super::xe_116_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe116Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_116_pipeline_compose() {
        let p1 = super::Xe116Pipeline::new()
            .add_parse(super::xe_116_pipeline_identity);
        let p2 = super::Xe116Pipeline::new()
            .add_transform(super::xe_116_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_116_pipeline_error_display() {
        let e = super::Xe116PipelineError {
            stage: super::Xe116Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_116_cache_put_get() {
        let mut c = super::Xe116Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_116_cache_miss() {
        let mut c: super::Xe116Cache<&str, i32> = super::Xe116Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_116_cache_ttl_expiry() {
        let mut c = super::Xe116Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_116_cache_evict() {
        let mut c = super::Xe116Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_116_cache_capacity() {
        let mut c = super::Xe116Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_116_cache_stats() {
        let mut c = super::Xe116Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_116_cache_clear() {
        let mut c = super::Xe116Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_114 graph tests ------------------------------------------------

    #[test]
    fn xg_114_graph_empty() {
        let g = super::Xg114Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_114_graph_add_node() {
        let mut g = super::Xg114Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_114_graph_add_edge() {
        let mut g = super::Xg114Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_114_graph_neighbors() {
        let mut g = super::Xg114Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_114_graph_has_path() {
        let mut g = super::Xg114Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_114_graph_self_path() {
        let g = super::Xg114Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_114_graph_topo_sort() {
        let mut g = super::Xg114Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_114_graph_cycle_detect_false() {
        let mut g = super::Xg114Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_114_graph_cycle_detect_true() {
        let mut g = super::Xg114Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_114 heap tests -------------------------------------------------

    #[test]
    fn xg_114_heap_empty() {
        let h: super::Xg114Heap<i32> = super::Xg114Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_114_heap_push_pop() {
        let mut h = super::Xg114Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_114_heap_peek() {
        let mut h = super::Xg114Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_114_heap_drain_sorted() {
        let mut h = super::Xg114Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_114_heap_merge() {
        let mut a = super::Xg114Heap::new();
        let mut b = super::Xg114Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_114_heap_default() {
        let h: super::Xg114Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_114_graph_default() {
        let g: super::Xg114Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh203_skip_insert_contains() {
        let mut sl = super::Xh203SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh203_skip_remove() {
        let mut sl = super::Xh203SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh203_skip_len() {
        let mut sl = super::Xh203SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh203_skip_range_query() {
        let mut sl = super::Xh203SkipList::xh_new(4);
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
    fn xh203_skip_floor_ceiling() {
        let mut sl = super::Xh203SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh203_skip_rank() {
        let mut sl = super::Xh203SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh203_skip_empty() {
        let sl = super::Xh203SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh203_skip_duplicates() {
        let mut sl = super::Xh203SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh203_bitset_set_test() {
        let mut bs = super::Xh203BitSet::xh_new(256);
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
    fn xh203_bitset_clear_count() {
        let mut bs = super::Xh203BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh203_bitset_and_or_xor() {
        let mut a = super::Xh203BitSet::xh_new(128);
        let mut b = super::Xh203BitSet::xh_new(128);
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
    fn xh203_bitset_iter_ones() {
        let mut bs = super::Xh203BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh203_bitset_first_last() {
        let mut bs = super::Xh203BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh203_bitset_empty() {
        let bs = super::Xh203BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi203_deque_push_pop_back() {
        let mut dq = super::Xi203Deque::xi_new(4);
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
    fn xi203_deque_push_pop_front() {
        let mut dq = super::Xi203Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi203_deque_mixed_ops() {
        let mut dq = super::Xi203Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi203_deque_get_and_split() {
        let mut dq = super::Xi203Deque::xi_new(8);
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
    fn xi203_deque_rotate_left() {
        let mut dq = super::Xi203Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi203_deque_rotate_right() {
        let mut dq = super::Xi203Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi203_deque_grow() {
        let mut dq = super::Xi203Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi203_deque_empty() {
        let dq = super::Xi203Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi203_interval_tree_insert_query() {
        let mut tree = super::Xi203IntervalTree::xi_new();
        tree.xi_insert(super::Xi203Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi203Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi203Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi203_interval_tree_overlap() {
        let mut tree = super::Xi203IntervalTree::xi_new();
        tree.xi_insert(super::Xi203Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi203Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi203Interval::xi_new(12, 20));
        let q = super::Xi203Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi203_interval_tree_remove() {
        let mut tree = super::Xi203IntervalTree::xi_new();
        tree.xi_insert(super::Xi203Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi203Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi203_interval_tree_gaps() {
        let mut tree = super::Xi203IntervalTree::xi_new();
        tree.xi_insert(super::Xi203Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi203Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi203Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi203Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi203Interval::xi_new(8, 10));
    }

    #[test]
    fn xi203_interval_tree_merge() {
        let mut tree = super::Xi203IntervalTree::xi_new();
        tree.xi_insert(super::Xi203Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi203Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi203Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi203Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi203Interval::xi_new(10, 15));
    }

    #[test]
    fn xi203_interval_tree_all() {
        let mut tree = super::Xi203IntervalTree::xi_new();
        tree.xi_insert(super::Xi203Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi203Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi203_interval_tree_empty() {
        let tree = super::Xi203IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi203_interval_tree_contains_point() {
        let iv = super::Xi203Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 203) ---

    #[test]
    fn xj_203_uf_make_and_find() {
        let mut uf = super::Xj203UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_203_uf_union_connected() {
        let mut uf = super::Xj203UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_203_uf_component_count() {
        let mut uf = super::Xj203UnionFind::xj_new();
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
    fn xj_203_uf_component_size() {
        let mut uf = super::Xj203UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_203_uf_largest_component() {
        let mut uf = super::Xj203UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_203_uf_many_elements() {
        let mut uf = super::Xj203UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_203_uf_separate_components() {
        let mut uf = super::Xj203UnionFind::xj_new();
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
    fn xj_203_uf_path_compression() {
        let mut uf = super::Xj203UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_203_bt_insert_get() {
        let mut bt = super::Xj203BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_203_bt_contains_len() {
        let mut bt = super::Xj203BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_203_bt_replace() {
        let mut bt = super::Xj203BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_203_bt_remove() {
        let mut bt = super::Xj203BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_203_bt_keys_values() {
        let mut bt = super::Xj203BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_203_bt_range() {
        let mut bt = super::Xj203BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_203_bt_min_max() {
        let mut bt = super::Xj203BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_203_bt_many_inserts() {
        let mut bt = super::Xj203BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_203 segment tree tests ---

    #[test]
    fn xk_203_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk203SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_203_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk203SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_203_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk203SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_203_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk203SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_203_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk203SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_203_st_single_element() {
        let data = vec![42];
        let st = super::Xk203SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_203_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk203SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_203_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk203SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_203 disjoint intervals tests ---

    #[test]
    fn xk_203_di_add_and_count() {
        let mut di = super::Xk203DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_203_di_merge_overlap() {
        let mut di = super::Xk203DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_203_di_contains() {
        let mut di = super::Xk203DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_203_di_remove() {
        let mut di = super::Xk203DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_203_di_covered_length() {
        let mut di = super::Xk203DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_203_di_gaps() {
        let mut di = super::Xk203DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_203_di_merge_adjacent() {
        let mut di = super::Xk203DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_203_di_empty() {
        let di = super::Xk203DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
