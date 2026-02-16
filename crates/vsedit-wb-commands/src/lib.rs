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
    fn unregister() {
        let mut reg = CommandRegistry::new();
        reg.register(make_cmd("cmd1", "Command 1"));
        assert!(reg.unregister("cmd1"));
        assert!(!reg.unregister("cmd1"));
        assert_eq!(reg.command_count(), 0);
    }

    #[test]
    fn find_commands() {
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
    fn get_by_category() {
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
    fn has_command() {
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
    fn get_by_source() {
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
}
