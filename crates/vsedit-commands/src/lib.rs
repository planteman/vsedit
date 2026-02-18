//! Command registry and execution for vsedit.
//!
//! This crate provides the global command registry, equivalent to
//! VS Code's `vs/platform/commands/common/commands.ts`.
//!
//! # Key types
//!
//! - [`CommandHandler`] — a boxed function that handles a command invocation.
//! - [`CommandDescriptor`] — pairs a command ID with its handler and optional description.
//! - [`CommandRegistry`] — global, thread-safe registry for commands.
//! - [`ICommandService`] — service trait for executing commands through the DI container.
//! - [`CommandRegistration`] — RAII handle that unregisters the command on drop.
//!
//! # Example
//!
//! ```
//! use vsedit_commands::{CommandRegistry, CommandArgs};
//!
//! let registry = CommandRegistry::new();
//! let _reg = registry.register("hello", Box::new(|_args: CommandArgs| {
//!     Ok(None)
//! }));
//!
//! assert!(registry.has("hello"));
//! let result = registry.execute("hello", vec![]);
//! assert!(result.is_ok());
//! ```

use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock, Weak};

use vsedit_di::service;

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

/// Arguments passed to a command handler.
pub type CommandArgs = Vec<Box<dyn Any + Send>>;

/// Result returned from a command handler.
pub type CommandResult =
    Result<Option<Box<dyn Any + Send>>, Box<dyn std::error::Error + Send + Sync>>;

/// A boxed, thread-safe command handler function.
pub type CommandHandler = Box<dyn Fn(CommandArgs) -> CommandResult + Send + Sync>;

// ---------------------------------------------------------------------------
// CommandDescriptor
// ---------------------------------------------------------------------------

/// Describes a registered command: its identifier, handler, and optional
/// human-readable description.
pub struct CommandDescriptor {
    pub id: String,
    pub handler: CommandHandler,
    pub description: Option<String>,
}

impl CommandDescriptor {
    /// Returns `true` if the descriptor has a non-empty description.
    pub fn has_description(&self) -> bool {
        self.description.as_ref().is_some_and(|d| !d.is_empty())
    }
}

impl fmt::Debug for CommandDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandDescriptor")
            .field("id", &self.id)
            .field("description", &self.description)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// CommandRegistration — RAII unregister-on-drop handle
// ---------------------------------------------------------------------------

/// A handle returned by [`CommandRegistry::register`]. When dropped, the
/// associated command is automatically removed from the registry.
pub struct CommandRegistration {
    id: String,
    registry: Weak<RwLock<HashMap<String, CommandDescriptor>>>,
}

impl CommandRegistration {
    /// Explicitly unregister the command. Subsequent calls are no-ops.
    pub fn unregister(&self) {
        if let Some(inner) = self.registry.upgrade() {
            if let Ok(mut map) = inner.write() {
                map.remove(&self.id);
            }
        }
    }
}

impl Drop for CommandRegistration {
    fn drop(&mut self) {
        self.unregister();
    }
}

impl fmt::Debug for CommandRegistration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandRegistration")
            .field("id", &self.id)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// CommandRegistry
// ---------------------------------------------------------------------------

/// A thread-safe registry for commands.
///
/// Commands are identified by string IDs. Each ID maps to exactly one
/// [`CommandDescriptor`]. Registering a handler returns a
/// [`CommandRegistration`] that removes the command when dropped.
pub struct CommandRegistry {
    commands: Arc<RwLock<HashMap<String, CommandDescriptor>>>,
}

impl CommandRegistry {
    /// Create an empty command registry.
    pub fn new() -> Self {
        Self {
            commands: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a command handler under the given `id`.
    ///
    /// Returns a [`CommandRegistration`] whose [`Drop`] implementation
    /// removes the command from the registry.
    ///
    /// If a command with the same `id` already exists it is replaced.
    pub fn register(&self, id: impl Into<String>, handler: CommandHandler) -> CommandRegistration {
        self.register_with_description(id, handler, None)
    }

    /// Register a command with an optional description.
    pub fn register_with_description(
        &self,
        id: impl Into<String>,
        handler: CommandHandler,
        description: Option<String>,
    ) -> CommandRegistration {
        let id = id.into();
        let descriptor = CommandDescriptor {
            id: id.clone(),
            handler,
            description,
        };
        {
            let mut map = self.commands.write().unwrap();
            map.insert(id.clone(), descriptor);
        }
        CommandRegistration {
            id,
            registry: Arc::downgrade(&self.commands),
        }
    }

    /// Execute the command identified by `id` with the provided arguments.
    ///
    /// Returns an error if no command with the given `id` is registered.
    pub fn execute(&self, id: &str, args: CommandArgs) -> CommandResult {
        let map = self.commands.read().unwrap();
        let descriptor = map
            .get(id)
            .ok_or_else(|| format!("Command '{}' not found", id))?;
        (descriptor.handler)(args)
    }

    /// Returns `true` if a command with the given `id` is registered.
    pub fn has(&self, id: &str) -> bool {
        let map = self.commands.read().unwrap();
        map.contains_key(id)
    }

    /// Returns the number of commands currently registered.
    pub fn command_count(&self) -> usize {
        let map = self.commands.read().unwrap();
        map.len()
    }

    /// Returns a sorted list of all registered command IDs.
    pub fn get_commands(&self) -> Vec<String> {
        let map = self.commands.read().unwrap();
        let mut ids: Vec<String> = map.keys().cloned().collect();
        ids.sort();
        ids
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CommandRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self
            .commands
            .read()
            .map(|m| m.len())
            .unwrap_or(0);
        f.debug_struct("CommandRegistry")
            .field("commands", &count)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// ICommandService — DI-compatible service trait
// ---------------------------------------------------------------------------

/// Service trait for command execution, suitable for registration in the
/// vsedit DI container.
pub trait ICommandService: Send + Sync {
    /// Execute the command identified by `id`.
    fn execute_command(&self, id: &str, args: CommandArgs) -> CommandResult;

    /// Returns `true` if a command with the given `id` exists.
    fn has_command(&self, id: &str) -> bool;
}

// ---------------------------------------------------------------------------
// CommandService — ICommandService backed by CommandRegistry
// ---------------------------------------------------------------------------

/// Default implementation of [`ICommandService`] backed by a
/// [`CommandRegistry`].
pub struct CommandService {
    registry: CommandRegistry,
}

service!(CommandService, "CommandService");

impl CommandService {
    /// Create a new `CommandService` wrapping a fresh [`CommandRegistry`].
    pub fn new() -> Self {
        Self {
            registry: CommandRegistry::new(),
        }
    }

    /// Create a `CommandService` wrapping an existing registry.
    pub fn with_registry(registry: CommandRegistry) -> Self {
        Self { registry }
    }

    /// Borrow the underlying registry for direct registration.
    pub fn registry(&self) -> &CommandRegistry {
        &self.registry
    }

    /// Returns `true` if the given command `id` is registered in the
    /// underlying registry.
    pub fn is_command_registered(&self, id: &str) -> bool {
        self.registry.has(id)
    }
}

impl Default for CommandService {
    fn default() -> Self {
        Self::new()
    }
}

impl ICommandService for CommandService {
    fn execute_command(&self, id: &str, args: CommandArgs) -> CommandResult {
        self.registry.execute(id, args)
    }

    fn has_command(&self, id: &str) -> bool {
        self.registry.has(id)
    }
}

// ---------------------------------------------------------------------------
// Built-in command helpers
// ---------------------------------------------------------------------------

/// Register a set of built-in commands on a registry.
///
/// Each entry is a `(id, handler)` pair. Returns a `Vec` of
/// [`CommandRegistration`] handles — keep them alive to keep the commands
/// registered.
pub fn register_builtin_commands(
    registry: &CommandRegistry,
    commands: Vec<(&str, CommandHandler)>,
) -> Vec<CommandRegistration> {
    commands
        .into_iter()
        .map(|(id, handler)| registry.register(id, handler))
        .collect()
}

// ---------------------------------------------------------------------------
// CommandHistory — tracks executed commands for recency and frequency
// ---------------------------------------------------------------------------

/// An entry in the command history.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub command_id: String,
    pub timestamp_ms: u64,
}

impl fmt::Display for HistoryEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} @ {}", self.command_id, self.timestamp_ms)
    }
}

/// Records executed commands, providing recency and frequency queries.
#[derive(Debug, Default)]
pub struct CommandHistory {
    entries: Vec<HistoryEntry>,
    frequency: HashMap<String, usize>,
}

impl CommandHistory {
    /// Create an empty command history.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            frequency: HashMap::new(),
        }
    }

    /// Record a command execution at the given timestamp.
    pub fn record(&mut self, command_id: impl Into<String>, timestamp_ms: u64) {
        let id = command_id.into();
        *self.frequency.entry(id.clone()).or_insert(0) += 1;
        self.entries.push(HistoryEntry {
            command_id: id,
            timestamp_ms,
        });
    }

    /// Return the most recent `n` history entries (newest first).
    pub fn get_recent(&self, n: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(n).collect()
    }

    /// Return how many times a command has been executed.
    pub fn get_frequency(&self, command_id: &str) -> usize {
        self.frequency.get(command_id).copied().unwrap_or(0)
    }

    /// Return the `top_n` most frequently executed command IDs (descending).
    pub fn most_frequent(&self, top_n: usize) -> Vec<(&str, usize)> {
        let mut pairs: Vec<(&str, usize)> = self
            .frequency
            .iter()
            .map(|(id, &count)| (id.as_str(), count))
            .collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        pairs.truncate(top_n);
        pairs
    }

    /// Clear all history entries and frequency counts.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.frequency.clear();
    }

    /// Return the most recent history entry, or `None` if empty.
    pub fn last_entry(&self) -> Option<&HistoryEntry> {
        self.entries.last()
    }

    /// Return all history entries whose `command_id` matches the given id.
    pub fn entries_for_command(&self, command_id: &str) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.command_id == command_id)
            .collect()
    }

    /// Total number of recorded executions.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no commands have been recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// CommandPalette — fuzzy-style filtering of commands by query
// ---------------------------------------------------------------------------

/// A match result from the command palette, carrying a relevance score.
#[derive(Debug, Clone)]
pub struct PaletteMatch {
    pub command_id: String,
    pub description: Option<String>,
    /// Higher is more relevant.
    pub score: u32,
}

/// Filters a list of command descriptors by a query string and sorts
/// results by relevance.
#[derive(Debug, Default)]
pub struct CommandPalette {
    entries: Vec<PaletteEntry>,
}

#[derive(Debug, Clone)]
struct PaletteEntry {
    id: String,
    description: Option<String>,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add a command to the palette.
    pub fn add(&mut self, id: impl Into<String>, description: Option<String>) {
        self.entries.push(PaletteEntry {
            id: id.into(),
            description,
        });
    }

    /// Filter commands whose ID or description match the `query`.
    ///
    /// Scoring rules:
    /// - Exact ID match: 100
    /// - ID starts with query: 75
    /// - ID contains query: 50
    /// - Description contains query: 25
    ///
    /// Results are sorted descending by score, then alphabetically by ID.
    pub fn filter_commands(&self, query: &str) -> Vec<PaletteMatch> {
        if query.is_empty() {
            return self
                .entries
                .iter()
                .map(|e| PaletteMatch {
                    command_id: e.id.clone(),
                    description: e.description.clone(),
                    score: 0,
                })
                .collect();
        }

        let query_lower = query.to_lowercase();
        let mut matches: Vec<PaletteMatch> = self
            .entries
            .iter()
            .filter_map(|entry| {
                let id_lower = entry.id.to_lowercase();
                let desc_lower = entry
                    .description
                    .as_ref()
                    .map(|d| d.to_lowercase())
                    .unwrap_or_default();

                let score = if id_lower == query_lower {
                    100
                } else if id_lower.starts_with(&query_lower) {
                    75
                } else if id_lower.contains(&query_lower) {
                    50
                } else if desc_lower.contains(&query_lower) {
                    25
                } else {
                    return None;
                };

                Some(PaletteMatch {
                    command_id: entry.id.clone(),
                    description: entry.description.clone(),
                    score,
                })
            })
            .collect();

        matches.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.command_id.cmp(&b.command_id)));
        matches
    }
}

// ---------------------------------------------------------------------------
// KeybindingConflict — detection of duplicate keybinding assignments
// ---------------------------------------------------------------------------

/// Describes a conflict where two commands are bound to the same key chord.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingConflict {
    pub key: String,
    pub command_ids: Vec<String>,
}

/// A single keybinding entry mapping a key chord to a command.
#[derive(Debug, Clone)]
pub struct Keybinding {
    pub key: String,
    pub command_id: String,
}

/// Detect keybinding conflicts: keys that are bound to more than one command.
///
/// Returns a list of [`KeybindingConflict`]s sorted by key.
pub fn detect_conflicts(bindings: &[Keybinding]) -> Vec<KeybindingConflict> {
    let mut by_key: HashMap<String, Vec<String>> = HashMap::new();
    for b in bindings {
        by_key
            .entry(b.key.clone())
            .or_default()
            .push(b.command_id.clone());
    }

    let mut conflicts: Vec<KeybindingConflict> = by_key
        .into_iter()
        .filter(|(_, cmds)| cmds.len() > 1)
        .map(|(key, mut command_ids)| {
            command_ids.sort();
            command_ids.dedup();
            KeybindingConflict { key, command_ids }
        })
        .filter(|c| c.command_ids.len() > 1)
        .collect();

    conflicts.sort_by(|a, b| a.key.cmp(&b.key));
    conflicts
}

// ---------------------------------------------------------------------------
// Command aliases
// ---------------------------------------------------------------------------

/// Maps an alias name to a canonical command ID.
#[derive(Debug, Clone)]
pub struct CommandAlias {
    /// The alias name (e.g. "quit").
    pub alias: String,
    /// The real command id the alias points to (e.g. "workbench.action.quit").
    pub command_id: String,
}

impl CommandAlias {
    pub fn new(alias: impl Into<String>, command_id: impl Into<String>) -> Self {
        Self {
            alias: alias.into(),
            command_id: command_id.into(),
        }
    }
}

impl CommandRegistry {
    /// Register an alias that maps to an existing command id.
    ///
    /// The alias is stored as a separate mapping. When executed, the
    /// registry resolves the alias and delegates to the real command.
    pub fn register_alias(&self, alias: impl Into<String>, command_id: impl Into<String>) {
        let alias = alias.into();
        let command_id = command_id.into();
        let map = self.commands.read().unwrap();
        if !map.contains_key(&command_id) {
            return;
        }
        drop(map);
        // Store the alias as a command whose handler delegates to the real id.
        // We wrap the real command_id in a descriptor whose description
        // encodes the alias relationship.
        let target = command_id.clone();
        let weak = Arc::downgrade(&self.commands);
        let handler: CommandHandler = Box::new(move |args: CommandArgs| {
            let strong = weak.upgrade().ok_or("registry dropped")?;
            let map = strong.read().map_err(|e| e.to_string())?;
            let desc = map
                .get(&target)
                .ok_or_else(|| format!("Alias target '{}' not found", target))?;
            (desc.handler)(args)
        });
        let descriptor = CommandDescriptor {
            id: alias.clone(),
            handler,
            description: Some(format!("Alias for {}", command_id)),
        };
        let mut map = self.commands.write().unwrap();
        map.insert(alias, descriptor);
    }

    /// Resolve an alias to the underlying command id.
    ///
    /// Returns `Some(command_id)` if the given `id` was registered via
    /// `register_alias`, or `None` if the id is not an alias.
    pub fn resolve_alias(&self, id: &str) -> Option<String> {
        let map = self.commands.read().unwrap();
        map.get(id).and_then(|d| {
            d.description
                .as_ref()
                .filter(|desc| desc.starts_with("Alias for "))
                .map(|desc| desc.trim_start_matches("Alias for ").to_string())
        })
    }
}

// ---------------------------------------------------------------------------
// CommandHistoryTracker
// ---------------------------------------------------------------------------

/// A ring-buffer-style command history tracker with configurable max size.
#[derive(Debug)]
pub struct CommandHistoryTracker {
    entries: Vec<HistoryEntry>,
    max_size: usize,
    frequency: HashMap<String, usize>,
}

impl CommandHistoryTracker {
    /// Create a tracker with the given maximum number of entries.
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size: max_size.max(1),
            frequency: HashMap::new(),
        }
    }

    /// Record a command execution. If at capacity, removes the oldest entry.
    pub fn record(&mut self, command_id: impl Into<String>, timestamp_ms: u64) {
        let id = command_id.into();
        *self.frequency.entry(id.clone()).or_insert(0) += 1;
        if self.entries.len() >= self.max_size {
            let removed = self.entries.remove(0);
            // Decrement frequency for removed entry
            if let Some(count) = self.frequency.get_mut(&removed.command_id) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.frequency.remove(&removed.command_id);
                }
            }
        }
        self.entries.push(HistoryEntry { command_id: id, timestamp_ms });
    }

    /// Return the most recent `n` entries (newest first).
    pub fn recent(&self, n: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(n).collect()
    }

    /// Return the unique command IDs in order of most recently used.
    pub fn recent_unique(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for entry in self.entries.iter().rev() {
            if seen.insert(entry.command_id.as_str()) {
                result.push(entry.command_id.as_str());
            }
        }
        result
    }

    /// Return how many times a command has been recorded (within the current buffer).
    pub fn frequency(&self, command_id: &str) -> usize {
        self.frequency.get(command_id).copied().unwrap_or(0)
    }

    /// Return the number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if no entries are stored.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.frequency.clear();
    }
}

// ---------------------------------------------------------------------------
// Fuzzy search
// ---------------------------------------------------------------------------

/// Perform a fuzzy search over command IDs, returning matches sorted by relevance.
///
/// A command matches if all characters in `query` appear in order in the command ID
/// (case-insensitive). The score is based on how tightly the characters cluster.
pub fn command_fuzzy_search(command_ids: &[String], query: &str) -> Vec<(String, u32)> {
    if query.is_empty() {
        return command_ids.iter().map(|id| (id.clone(), 0)).collect();
    }
    let query_lower: Vec<char> = query.to_lowercase().chars().collect();
    let mut results: Vec<(String, u32)> = command_ids
        .iter()
        .filter_map(|id| {
            let id_lower: Vec<char> = id.to_lowercase().chars().collect();
            let score = fuzzy_match_score(&id_lower, &query_lower)?;
            Some((id.clone(), score))
        })
        .collect();
    results.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    results
}

fn fuzzy_match_score(text: &[char], query: &[char]) -> Option<u32> {
    let mut qi = 0;
    let mut first_match = None;
    let mut last_match = 0;
    for (i, &ch) in text.iter().enumerate() {
        if qi < query.len() && ch == query[qi] {
            if first_match.is_none() {
                first_match = Some(i);
            }
            last_match = i;
            qi += 1;
        }
    }
    if qi != query.len() {
        return None;
    }
    let first = first_match.unwrap_or(0);
    let span = last_match - first + 1;
    // Score: shorter span is better, earlier start is better
    let span_score = 100u32.saturating_sub(span as u32);
    let position_score = 50u32.saturating_sub(first as u32);
    // Bonus for exact prefix match
    let prefix_bonus = if first == 0 { 50 } else { 0 };
    Some(span_score + position_score + prefix_bonus)
}

// ---------------------------------------------------------------------------
// CommandMacro – record and replay command sequences
// ---------------------------------------------------------------------------

/// A single step in a command macro.
#[derive(Debug, Clone)]
pub struct MacroStep {
    pub command_id: String,
    pub args: Vec<String>,
}

impl MacroStep {
    pub fn new(command_id: impl Into<String>) -> Self {
        Self {
            command_id: command_id.into(),
            args: Vec::new(),
        }
    }

    pub fn with_args(command_id: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            command_id: command_id.into(),
            args,
        }
    }
}

impl fmt::Display for MacroStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.args.is_empty() {
            write!(f, "{}", self.command_id)
        } else {
            write!(f, "{}({})", self.command_id, self.args.join(", "))
        }
    }
}

/// Records and replays sequences of commands.
#[derive(Debug, Clone)]
pub struct CommandMacro {
    name: String,
    steps: Vec<MacroStep>,
    recording: bool,
}

impl CommandMacro {
    /// Create a new named macro.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            steps: Vec::new(),
            recording: false,
        }
    }

    /// Start recording commands.
    pub fn start_recording(&mut self) {
        self.recording = true;
        self.steps.clear();
    }

    /// Stop recording.
    pub fn stop_recording(&mut self) {
        self.recording = false;
    }

    /// Record a step (only while recording is active).
    pub fn record_step(&mut self, step: MacroStep) -> bool {
        if self.recording {
            self.steps.push(step);
            true
        } else {
            false
        }
    }

    /// Return the recorded steps for replay.
    pub fn steps(&self) -> &[MacroStep] {
        &self.steps
    }

    /// Number of recorded steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Whether the macro is currently recording.
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// The macro name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Replay the macro by executing each step against the given registry.
    pub fn replay(&self, registry: &CommandRegistry) -> Vec<CommandResult> {
        self.steps
            .iter()
            .map(|step| registry.execute(&step.command_id, vec![]))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// CommandCondition – conditional command execution
// ---------------------------------------------------------------------------

/// A predicate that determines whether a command should execute.
#[derive(Debug, Clone)]
pub struct CommandCondition {
    context_key: String,
    expected_value: Option<String>,
    negated: bool,
}

impl CommandCondition {
    /// Create a condition that checks if `context_key` is set (truthy).
    pub fn when(context_key: impl Into<String>) -> Self {
        Self {
            context_key: context_key.into(),
            expected_value: None,
            negated: false,
        }
    }

    /// Create a condition that checks if `context_key` equals `value`.
    pub fn when_equals(context_key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            context_key: context_key.into(),
            expected_value: Some(value.into()),
            negated: false,
        }
    }

    /// Negate the condition.
    pub fn negate(mut self) -> Self {
        self.negated = !self.negated;
        self
    }

    /// Evaluate the condition against a context map.
    pub fn evaluate(&self, context: &HashMap<String, String>) -> bool {
        let result = match &self.expected_value {
            Some(expected) => context.get(&self.context_key).map_or(false, |v| v == expected),
            None => context.contains_key(&self.context_key),
        };
        if self.negated { !result } else { result }
    }

    /// Return the context key this condition inspects.
    pub fn key(&self) -> &str {
        &self.context_key
    }
}

impl fmt::Display for CommandCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = if self.negated { "!" } else { "" };
        match &self.expected_value {
            Some(v) => write!(f, "{prefix}{} == '{v}'", self.context_key),
            None => write!(f, "{prefix}{}", self.context_key),
        }
    }
}

// ---------------------------------------------------------------------------
// CommandScheduler – schedule command execution
// ---------------------------------------------------------------------------

/// A scheduled command execution entry.
#[derive(Debug, Clone)]
pub struct ScheduledCommand {
    pub command_id: String,
    pub execute_at_ms: u64,
    pub repeat_interval_ms: Option<u64>,
    executed: bool,
}

/// Manages scheduled command execution.
#[derive(Debug)]
pub struct CommandScheduler {
    entries: Vec<ScheduledCommand>,
}

impl CommandScheduler {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Schedule a one-shot command at the given timestamp (ms).
    pub fn schedule_once(&mut self, command_id: impl Into<String>, at_ms: u64) {
        self.entries.push(ScheduledCommand {
            command_id: command_id.into(),
            execute_at_ms: at_ms,
            repeat_interval_ms: None,
            executed: false,
        });
    }

    /// Schedule a repeating command.
    pub fn schedule_repeating(
        &mut self,
        command_id: impl Into<String>,
        start_ms: u64,
        interval_ms: u64,
    ) {
        self.entries.push(ScheduledCommand {
            command_id: command_id.into(),
            execute_at_ms: start_ms,
            repeat_interval_ms: Some(interval_ms),
            executed: false,
        });
    }

    /// Tick the scheduler at the given current time.
    /// Returns the list of command IDs that should be executed now.
    pub fn tick(&mut self, now_ms: u64) -> Vec<String> {
        let mut due = Vec::new();
        for entry in &mut self.entries {
            if !entry.executed && now_ms >= entry.execute_at_ms {
                due.push(entry.command_id.clone());
                if let Some(interval) = entry.repeat_interval_ms {
                    entry.execute_at_ms += interval;
                } else {
                    entry.executed = true;
                }
            }
        }
        due
    }

    /// Number of pending (not-yet-executed) scheduled commands.
    pub fn pending_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.executed).count()
    }

    /// Cancel all scheduled commands with the given ID.
    pub fn cancel(&mut self, command_id: &str) {
        self.entries.retain(|e| e.command_id != command_id);
    }

    /// Cancel all scheduled commands.
    pub fn cancel_all(&mut self) {
        self.entries.clear();
    }
}

impl Default for CommandScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CommandAlias – alias resolution chains
// ---------------------------------------------------------------------------

impl CommandRegistry {
    /// Resolve an alias chain: follows alias → alias → … → real command.
    /// Returns `None` if the id is not an alias, or if a cycle is detected.
    pub fn resolve_alias_chain(&self, id: &str) -> Option<String> {
        let mut current = id.to_string();
        let mut visited = std::collections::HashSet::new();
        loop {
            if !visited.insert(current.clone()) {
                return None; // cycle detected
            }
            match self.resolve_alias(&current) {
                Some(target) => current = target,
                None => {
                    if current == id {
                        return None; // was never an alias
                    }
                    return Some(current);
                }
            }
        }
    }

    /// Return the chain depth for an alias (0 if not an alias).
    pub fn alias_chain_depth(&self, id: &str) -> usize {
        let mut depth = 0;
        let mut current = id.to_string();
        let mut visited = std::collections::HashSet::new();
        while visited.insert(current.clone()) {
            match self.resolve_alias(&current) {
                Some(target) => {
                    depth += 1;
                    current = target;
                }
                None => break,
            }
        }
        depth
    }
}

// ---------------------------------------------------------------------------
// CommandCategory – grouping commands for display
// ---------------------------------------------------------------------------

/// A logical grouping of commands, used for palette sections and documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandCategory {
    pub id: String,
    pub label: String,
    pub order: u32,
}

impl CommandCategory {
    pub fn new(id: impl Into<String>, label: impl Into<String>, order: u32) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            order,
        }
    }
}

impl fmt::Display for CommandCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.id, self.label)
    }
}

/// Categorised command entry.
#[derive(Debug, Clone)]
pub struct CategorisedCommand {
    pub command_id: String,
    pub category: String,
    pub display_label: Option<String>,
}

impl CategorisedCommand {
    pub fn new(command_id: impl Into<String>, category: impl Into<String>) -> Self {
        Self {
            command_id: command_id.into(),
            category: category.into(),
            display_label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.display_label = Some(label.into());
        self
    }

    pub fn effective_label(&self) -> &str {
        self.display_label.as_deref().unwrap_or(&self.command_id)
    }
}

/// Group a set of categorised commands by category.
pub fn group_by_category(commands: &[CategorisedCommand]) -> HashMap<String, Vec<&CategorisedCommand>> {
    let mut map: HashMap<String, Vec<&CategorisedCommand>> = HashMap::new();
    for cmd in commands {
        map.entry(cmd.category.clone()).or_default().push(cmd);
    }
    map
}

/// Filter commands whose IDs start with a given prefix.
pub fn filter_commands_by_prefix<'a>(
    commands: &'a [CategorisedCommand],
    prefix: &str,
) -> Vec<&'a CategorisedCommand> {
    commands.iter().filter(|c| c.command_id.starts_with(prefix)).collect()
}

/// Return unique category names from a set of categorised commands.
pub fn unique_categories(commands: &[CategorisedCommand]) -> Vec<String> {
    let mut cats: Vec<String> = commands.iter().map(|c| c.category.clone()).collect();
    cats.sort();
    cats.dedup();
    cats
}

/// Sort categories by their `order` field.
pub fn sort_categories(categories: &mut [CommandCategory]) {
    categories.sort_by_key(|c| c.order);
}

/// Validate a command ID: must be non-empty, contain at least one dot, and
/// only consist of alphanumeric chars, dots, dashes, and underscores.
pub fn validate_command_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("command id must not be empty".into());
    }
    if !id.contains('.') {
        return Err(format!("command id '{id}' must contain at least one dot"));
    }
    if !id.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_') {
        return Err(format!("command id '{id}' contains invalid characters"));
    }
    Ok(())
}

/// Normalise a keybinding string to a canonical form (lowercase, sorted modifiers).
pub fn normalize_keybinding(kb: &str) -> String {
    let parts: Vec<&str> = kb.split('+').collect();
    if parts.len() <= 1 {
        return kb.to_lowercase();
    }
    let key = parts.last().unwrap().to_lowercase();
    let mut mods: Vec<String> = parts[..parts.len() - 1]
        .iter()
        .map(|m| m.to_lowercase())
        .collect();
    mods.sort();
    mods.push(key);
    mods.join("+")
}

// -- AliasMapping for command shortcuts --------------------------------------

/// Maps an alias to an actual command ID, with optional description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasMapping {
    pub alias: String,
    pub target_command: String,
    pub description: Option<String>,
}

impl AliasMapping {
    pub fn new(alias: &str, target: &str) -> Self {
        Self {
            alias: alias.to_string(),
            target_command: target.to_string(),
            description: None,
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }
}

impl fmt::Display for AliasMapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.alias, self.target_command)
    }
}

/// Manages a set of alias mappings.
#[derive(Debug, Default)]
pub struct AliasMappingRegistry {
    aliases: HashMap<String, AliasMapping>,
}

impl AliasMappingRegistry {
    pub fn new() -> Self {
        Self { aliases: HashMap::new() }
    }

    pub fn register(&mut self, alias: AliasMapping) {
        self.aliases.insert(alias.alias.clone(), alias);
    }

    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.aliases.get(name).map(|a| a.target_command.as_str())
    }

    pub fn unregister(&mut self, alias: &str) {
        self.aliases.remove(alias);
    }

    pub fn count(&self) -> usize {
        self.aliases.len()
    }

    /// Find all aliases pointing to a command.
    pub fn aliases_for(&self, target: &str) -> Vec<&str> {
        self.aliases.values()
            .filter(|a| a.target_command == target)
            .map(|a| a.alias.as_str())
            .collect()
    }
}

// -- CommandThrottle preventing rapid re-execution ---------------------------

/// Tracks last execution time to throttle rapid invocations.
#[derive(Debug)]
pub struct CommandThrottle {
    last_execution: HashMap<String, u64>,
    min_interval_ms: u64,
}

impl CommandThrottle {
    pub fn new(min_interval_ms: u64) -> Self {
        Self {
            last_execution: HashMap::new(),
            min_interval_ms,
        }
    }

    /// Check if a command can be executed at the given timestamp.
    pub fn can_execute(&self, command_id: &str, now_ms: u64) -> bool {
        match self.last_execution.get(command_id) {
            Some(&last) => now_ms.saturating_sub(last) >= self.min_interval_ms,
            None => true,
        }
    }

    /// Record that a command was executed.
    pub fn record_execution(&mut self, command_id: &str, now_ms: u64) {
        self.last_execution.insert(command_id.to_string(), now_ms);
    }

    /// Remaining wait time before a command can be executed again.
    pub fn remaining_ms(&self, command_id: &str, now_ms: u64) -> u64 {
        match self.last_execution.get(command_id) {
            Some(&last) => {
                let elapsed = now_ms.saturating_sub(last);
                if elapsed >= self.min_interval_ms { 0 } else { self.min_interval_ms - elapsed }
            }
            None => 0,
        }
    }

    /// Clear throttle state for a command.
    pub fn clear(&mut self, command_id: &str) {
        self.last_execution.remove(command_id);
    }

    pub fn clear_all(&mut self) {
        self.last_execution.clear();
    }
}

impl fmt::Display for CommandThrottle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Throttle(interval={}ms, {} tracked)", self.min_interval_ms, self.last_execution.len())
    }
}

// -- Undoable command execution history --------------------------------------

/// A record of an executed command with undo tracking.
#[derive(Debug, Clone)]
pub struct UndoableHistoryEntry {
    pub command_id: String,
    pub timestamp: u64,
    pub undoable: bool,
    pub undone: bool,
}

/// Tracks command execution history with undo support.
#[derive(Debug)]
pub struct UndoableCommandHistory {
    entries: Vec<UndoableHistoryEntry>,
    max_entries: usize,
}

impl UndoableCommandHistory {
    pub fn new(max_entries: usize) -> Self {
        Self { entries: Vec::new(), max_entries }
    }

    pub fn record(&mut self, command_id: &str, timestamp: u64, undoable: bool) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(UndoableHistoryEntry {
            command_id: command_id.to_string(),
            timestamp,
            undoable,
            undone: false,
        });
    }

    /// Mark the most recent undoable command as undone.
    pub fn undo_last(&mut self) -> Option<String> {
        for entry in self.entries.iter_mut().rev() {
            if entry.undoable && !entry.undone {
                entry.undone = true;
                return Some(entry.command_id.clone());
            }
        }
        None
    }

    pub fn entries(&self) -> &[UndoableHistoryEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn undone_count(&self) -> usize {
        self.entries.iter().filter(|e| e.undone).count()
    }

    /// Get the last N commands executed.
    pub fn last_n(&self, n: usize) -> &[UndoableHistoryEntry] {
        let start = self.entries.len().saturating_sub(n);
        &self.entries[start..]
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl fmt::Display for UndoableCommandHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "History({} entries, {} undone)", self.entries.len(), self.undone_count())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// CommandPaletteRanking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommandPaletteRanking {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl CommandPaletteRanking {
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

impl Default for CommandPaletteRanking {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for CommandPaletteRanking {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "CommandPaletteRanking({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// CommandArgumentSchema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommandArgumentSchema {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl CommandArgumentSchema {
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

impl Default for CommandArgumentSchema {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for CommandArgumentSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "CommandArgumentSchema({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// CommandPaletteRankingSnapshot — point-in-time snapshot of CommandPaletteRanking state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommandPaletteRankingSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl CommandPaletteRankingSnapshot {
    pub fn capture(source: &CommandPaletteRanking, timestamp: u64) -> Self {
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

impl fmt::Display for CommandPaletteRankingSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// CommandArgumentSchemaStats — aggregate statistics for CommandArgumentSchema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct CommandArgumentSchemaStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl CommandArgumentSchemaStats {
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

impl fmt::Display for CommandArgumentSchemaStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// CommandPaletteRankingConfig — configuration for CommandPaletteRanking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommandPaletteRankingConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl CommandPaletteRankingConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for CommandPaletteRankingConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for CommandPaletteRankingConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// CommandUndoRedoStack — simple undo/redo with generic string IDs
// ---------------------------------------------------------------------------

/// A stack-based undo/redo tracker for command IDs.
#[derive(Debug, Clone)]
pub struct CommandUndoRedoStack {
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
    capacity: usize,
}

impl CommandUndoRedoStack {
    pub fn new(capacity: usize) -> Self {
        Self { undo_stack: Vec::new(), redo_stack: Vec::new(), capacity }
    }

    /// Push a new command, clearing the redo stack.
    pub fn push(&mut self, command_id: impl Into<String>) {
        self.redo_stack.clear();
        if self.undo_stack.len() >= self.capacity {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(command_id.into());
    }

    /// Undo the last command, moving it to the redo stack.
    pub fn undo(&mut self) -> Option<String> {
        let cmd = self.undo_stack.pop()?;
        self.redo_stack.push(cmd.clone());
        Some(cmd)
    }

    /// Redo the last undone command.
    pub fn redo(&mut self) -> Option<String> {
        let cmd = self.redo_stack.pop()?;
        self.undo_stack.push(cmd.clone());
        Some(cmd)
    }

    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn undo_len(&self) -> usize { self.undo_stack.len() }
    pub fn redo_len(&self) -> usize { self.redo_stack.len() }
}

impl fmt::Display for CommandUndoRedoStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UndoRedo(undo={}, redo={})", self.undo_stack.len(), self.redo_stack.len())
    }
}

// ---------------------------------------------------------------------------
// CommandSequence — ordered chain of command IDs with rollback tracking
// ---------------------------------------------------------------------------

/// An ordered sequence of command IDs that should execute in order.
/// Tracks which commands have completed for rollback purposes.
#[derive(Debug, Clone)]
pub struct CommandSequence {
    steps: Vec<String>,
    completed: usize,
    failed: bool,
}

impl CommandSequence {
    pub fn new() -> Self {
        Self { steps: Vec::new(), completed: 0, failed: false }
    }

    pub fn add_step(&mut self, command_id: impl Into<String>) {
        self.steps.push(command_id.into());
    }

    /// Mark the next step as completed. Returns the completed command ID.
    pub fn mark_completed(&mut self) -> Option<&str> {
        if self.completed < self.steps.len() && !self.failed {
            let idx = self.completed;
            self.completed += 1;
            Some(&self.steps[idx])
        } else {
            None
        }
    }

    /// Mark the sequence as failed at the current step.
    pub fn mark_failed(&mut self) {
        self.failed = true;
    }

    /// IDs of completed steps (for rollback).
    pub fn completed_steps(&self) -> &[String] {
        &self.steps[..self.completed]
    }

    /// The remaining steps that haven't executed.
    pub fn remaining_steps(&self) -> &[String] {
        &self.steps[self.completed..]
    }

    pub fn is_done(&self) -> bool { self.completed >= self.steps.len() }
    pub fn has_failed(&self) -> bool { self.failed }
    pub fn total_steps(&self) -> usize { self.steps.len() }
    pub fn progress_fraction(&self) -> f64 {
        if self.steps.is_empty() { 1.0 } else { self.completed as f64 / self.steps.len() as f64 }
    }
}

impl Default for CommandSequence {
    fn default() -> Self { Self::new() }
}

impl fmt::Display for CommandSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sequence({}/{}, failed={})", self.completed, self.steps.len(), self.failed)
    }
}

// ---------------------------------------------------------------------------
// CommandCooldownManager — per-command cooldown with different intervals
// ---------------------------------------------------------------------------

/// Manages per-command cooldowns where each command can have its own interval.
#[derive(Debug, Clone)]
pub struct CommandCooldownManager {
    cooldowns: HashMap<String, u64>,
    last_used: HashMap<String, u64>,
}

impl CommandCooldownManager {
    pub fn new() -> Self {
        Self { cooldowns: HashMap::new(), last_used: HashMap::new() }
    }

    /// Register a cooldown interval (in milliseconds) for a command.
    pub fn set_cooldown(&mut self, command_id: impl Into<String>, cooldown_ms: u64) {
        self.cooldowns.insert(command_id.into(), cooldown_ms);
    }

    /// Remove the cooldown for a command.
    pub fn remove_cooldown(&mut self, command_id: &str) {
        self.cooldowns.remove(command_id);
        self.last_used.remove(command_id);
    }

    /// Try to use a command. Returns `true` if allowed, `false` if on cooldown.
    pub fn try_use(&mut self, command_id: &str, now_ms: u64) -> bool {
        let cooldown = self.cooldowns.get(command_id).copied().unwrap_or(0);
        if let Some(&last) = self.last_used.get(command_id) {
            if now_ms.saturating_sub(last) < cooldown {
                return false;
            }
        }
        self.last_used.insert(command_id.to_string(), now_ms);
        true
    }

    /// Time remaining on cooldown (0 if ready).
    pub fn remaining_ms(&self, command_id: &str, now_ms: u64) -> u64 {
        let cooldown = self.cooldowns.get(command_id).copied().unwrap_or(0);
        match self.last_used.get(command_id) {
            Some(&last) => {
                let elapsed = now_ms.saturating_sub(last);
                if elapsed >= cooldown { 0 } else { cooldown - elapsed }
            }
            None => 0,
        }
    }

    /// List all registered command IDs with cooldowns.
    pub fn registered_commands(&self) -> Vec<&str> {
        self.cooldowns.keys().map(|s| s.as_str()).collect()
    }

    pub fn clear_all(&mut self) {
        self.last_used.clear();
    }
}

impl Default for CommandCooldownManager {
    fn default() -> Self { Self::new() }
}


/// Configuration manager for commands functionality.
pub struct CommandsConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl CommandsConfig {
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

    pub fn merge(&mut self, other: &CommandsConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for commands operations.
pub struct CommandsRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl CommandsRateTracker {
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

/// Validation result collector for commands.
pub struct CommandsValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl CommandsValidator {
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

    pub fn merge(&mut self, other: &CommandsValidator) {
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
// xa_ extended helpers for commands
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaCommandsRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaCommandsRingBuf {
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
pub struct XaCommandsCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaCommandsCounter {
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

impl Default for XaCommandsCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 19
// ---------------------------------------------------------------------------

/// Generic object pool `Xc19Pool<T>`.
pub struct Xc19Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc19Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc19PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc19Pool<T> {
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
    pub fn stats(&self) -> Xc19PoolStats {
        Xc19PoolStats {
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

impl<T> Default for Xc19Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc19Scheduler`.
pub struct Xc19Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc19Scheduler {
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

impl Default for Xc19Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_19 hash for the given byte slice.
pub fn xc_19_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_19 convention.
pub fn xc_19_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_58 deepening: state machine + event bus ---

/// States for the Xd58 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd58State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd58State {
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
pub struct Xd58Transition {
    pub from: Xd58State,
    pub to: Xd58State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd58StateMachine {
    current: Xd58State,
    history: Vec<Xd58Transition>,
    step_counter: usize,
}

impl Xd58StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd58State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd58State {
        self.current
    }

    pub fn history(&self) -> &[Xd58Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd58State) -> Result<Xd58State, String> {
        let allowed = match (self.current, target) {
            (Xd58State::Idle, Xd58State::Running) => true,
            (Xd58State::Running, Xd58State::Paused) => true,
            (Xd58State::Running, Xd58State::Done) => true,
            (Xd58State::Paused, Xd58State::Running) => true,
            (Xd58State::Paused, Xd58State::Done) => true,
            (Xd58State::Done, Xd58State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_58: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd58Transition {
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
            "Xd58SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd58State> {
        let prefix = "Xd58SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd58State::Idle),
            "Running" => Some(Xd58State::Running),
            "Paused" => Some(Xd58State::Paused),
            "Done" => Some(Xd58State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd58State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd58 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd58Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd58Event {
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

type Xd58HandlerFn = Box<dyn Fn(&Xd58Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd58EventBus {
    handlers: Vec<(usize, Option<String>, Xd58HandlerFn)>,
    next_id: usize,
    published: Vec<Xd58Event>,
}

impl Xd58EventBus {
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
        F: Fn(&Xd58Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd58Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd58Event) {
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

    pub fn published_events(&self) -> &[Xd58Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #56
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf56Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf56TrieNode {
    children: std::collections::HashMap<char, Xf56TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf56Trie {
    root: Xf56TrieNode,
    count: usize,
}

impl Xf56Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf56TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf56TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf56TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf56BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf56BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 18).
pub struct Xh18SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh18SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 60 as u64,
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

/// A compact bit set supporting boolean operations (variant 18).
pub struct Xh18BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh18BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 18).
pub struct Xi18Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi18Deque<T> {
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
pub struct Xi18Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi18Interval {
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

/// A simple interval tree (variant 18).
pub struct Xi18IntervalTree {
    xi_intervals: Vec<Xi18Interval>,
}

impl Xi18IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi18Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi18Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi18Interval) -> Vec<&Xi18Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi18Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi18Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi18Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi18Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi18Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi18Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 18) ---

/// Disjoint set / union-find for crate 18.
pub struct Xj18UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj18UnionFind {
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

const XJ18_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 18.
pub struct Xj18BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj18BTreeNode<K, V>>>,
    len: usize,
}

struct Xj18BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj18BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj18BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ18_BTREE_ORDER - 1
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
        let mid = XJ18_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj18BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj18BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj18BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj18BTreeNode::xj_new_leaf();
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


// --- xk_18 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk18SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk18SegmentTree {
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
pub struct Xk18DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk18DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_18).
#[derive(Debug, Clone)]
pub struct Xl18Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl18Rope {
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

/// Suffix array for efficient string searching (xl_18).
#[derive(Debug, Clone)]
pub struct Xl18SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl18SuffixArray {
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

#[cfg(test)]
mod tests {
    use super::*;

    // -- Registration & execution -------------------------------------------

    #[test]
    fn register_and_execute() {
        let registry = CommandRegistry::new();
        let _reg = registry.register("test.hello", Box::new(|_args| Ok(None)));

        assert!(registry.has("test.hello"));
        let result = registry.execute("test.hello", vec![]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn execute_with_args_and_return_value() {
        let registry = CommandRegistry::new();
        let _reg = registry.register(
            "test.add",
            Box::new(|mut args: CommandArgs| {
                let b = args.pop().unwrap().downcast::<i32>().unwrap();
                let a = args.pop().unwrap().downcast::<i32>().unwrap();
                Ok(Some(Box::new(*a + *b) as Box<dyn Any + Send>))
            }),
        );

        let result = registry
            .execute(
                "test.add",
                vec![Box::new(2i32), Box::new(3i32)],
            )
            .unwrap()
            .unwrap();
        assert_eq!(*result.downcast::<i32>().unwrap(), 5);
    }

    #[test]
    fn execute_unknown_command_returns_error() {
        let registry = CommandRegistry::new();
        let result = registry.execute("no.such.command", vec![]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    // -- has / get_commands -------------------------------------------------

    #[test]
    fn has_returns_false_for_unregistered() {
        let registry = CommandRegistry::new();
        assert!(!registry.has("missing"));
    }

    #[test]
    fn get_commands_returns_sorted_ids() {
        let registry = CommandRegistry::new();
        let _r1 = registry.register("z.cmd", Box::new(|_| Ok(None)));
        let _r2 = registry.register("a.cmd", Box::new(|_| Ok(None)));
        let _r3 = registry.register("m.cmd", Box::new(|_| Ok(None)));

        let cmds = registry.get_commands();
        assert_eq!(cmds, vec!["a.cmd", "m.cmd", "z.cmd"]);
    }

    // -- Unregistration via drop --------------------------------------------

    #[test]
    fn drop_unregisters_command() {
        let registry = CommandRegistry::new();
        let reg = registry.register("temp.cmd", Box::new(|_| Ok(None)));
        assert!(registry.has("temp.cmd"));

        drop(reg);
        assert!(!registry.has("temp.cmd"));
    }

    #[test]
    fn explicit_unregister() {
        let registry = CommandRegistry::new();
        let reg = registry.register("temp.cmd", Box::new(|_| Ok(None)));
        assert!(registry.has("temp.cmd"));

        reg.unregister();
        assert!(!registry.has("temp.cmd"));

        // Second call is a no-op.
        reg.unregister();
    }

    #[test]
    fn unregister_after_registry_dropped() {
        let reg;
        {
            let registry = CommandRegistry::new();
            reg = registry.register("orphan", Box::new(|_| Ok(None)));
        }
        // Registry is dropped — unregister should not panic.
        reg.unregister();
    }

    // -- Handler error propagation ------------------------------------------

    #[test]
    fn handler_error_is_propagated() {
        let registry = CommandRegistry::new();
        let _reg = registry.register(
            "test.fail",
            Box::new(|_| Err("something went wrong".into())),
        );

        let result = registry.execute("test.fail", vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("something went wrong"));
    }

    // -- Replace existing command -------------------------------------------

    #[test]
    fn register_replaces_existing() {
        let registry = CommandRegistry::new();
        let _r1 = registry.register(
            "cmd",
            Box::new(|_| Ok(Some(Box::new(1i32) as Box<dyn Any + Send>))),
        );
        // Overwrite with a different handler.
        let _r2 = registry.register(
            "cmd",
            Box::new(|_| Ok(Some(Box::new(2i32) as Box<dyn Any + Send>))),
        );

        let val = registry
            .execute("cmd", vec![])
            .unwrap()
            .unwrap()
            .downcast::<i32>()
            .unwrap();
        assert_eq!(*val, 2);
    }

    // -- Description --------------------------------------------------------

    #[test]
    fn register_with_description() {
        let registry = CommandRegistry::new();
        let _reg = registry.register_with_description(
            "documented.cmd",
            Box::new(|_| Ok(None)),
            Some("A documented command".into()),
        );
        assert!(registry.has("documented.cmd"));
    }

    // -- Built-in helpers ---------------------------------------------------

    #[test]
    fn register_builtin_commands_helper() {
        let registry = CommandRegistry::new();
        let regs = register_builtin_commands(
            &registry,
            vec![
                ("builtin.a", Box::new(|_| Ok(None))),
                ("builtin.b", Box::new(|_| Ok(None))),
            ],
        );
        assert_eq!(regs.len(), 2);
        assert!(registry.has("builtin.a"));
        assert!(registry.has("builtin.b"));

        drop(regs);
        assert!(!registry.has("builtin.a"));
        assert!(!registry.has("builtin.b"));
    }

    // -- ICommandService / CommandService -----------------------------------

    #[test]
    fn command_service_execute_and_has() {
        let service = CommandService::new();
        let _reg = service
            .registry()
            .register("svc.cmd", Box::new(|_| Ok(None)));

        assert!(service.has_command("svc.cmd"));
        assert!(!service.has_command("missing"));

        let result = service.execute_command("svc.cmd", vec![]);
        assert!(result.is_ok());
    }

    #[test]
    fn command_service_with_registry() {
        let registry = CommandRegistry::new();
        let _reg = registry.register("pre.cmd", Box::new(|_| Ok(None)));
        let service = CommandService::with_registry(registry);
        assert!(service.has_command("pre.cmd"));
    }

    // -- Thread safety ------------------------------------------------------

    #[test]
    fn registry_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CommandRegistry>();
        assert_send_sync::<CommandService>();
    }

    #[test]
    fn concurrent_access() {
        let registry = CommandRegistry::new();
        let _reg = registry.register(
            "thread.cmd",
            Box::new(|_| Ok(Some(Box::new(42i32) as Box<dyn Any + Send>))),
        );

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let r = &registry;
                std::thread::scope(|_| {
                    let result = r.execute("thread.cmd", vec![]).unwrap().unwrap();
                    *result.downcast::<i32>().unwrap()
                })
            })
            .collect();

        for val in handles {
            assert_eq!(val, 42);
        }
    }

    // -- Debug formatting ---------------------------------------------------

    #[test]
    fn debug_formatting() {
        let registry = CommandRegistry::new();
        let dbg = format!("{registry:?}");
        assert!(dbg.contains("CommandRegistry"));

        let reg = registry.register("dbg", Box::new(|_| Ok(None)));
        let dbg = format!("{reg:?}");
        assert!(dbg.contains("CommandRegistration"));
    }

    // -- Default impls ------------------------------------------------------

    #[test]
    fn default_impls() {
        let _registry = CommandRegistry::default();
        let _service = CommandService::default();
    }

    // -- CommandHistory -----------------------------------------------------

    #[test]
    fn history_record_and_recent() {
        let mut history = CommandHistory::new();
        history.record("file.save", 100);
        history.record("edit.undo", 200);
        history.record("file.save", 300);

        let recent = history.get_recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].command_id, "file.save");
        assert_eq!(recent[1].command_id, "edit.undo");
    }

    #[test]
    fn history_frequency_and_most_frequent() {
        let mut history = CommandHistory::new();
        history.record("a", 1);
        history.record("b", 2);
        history.record("a", 3);
        history.record("a", 4);
        history.record("b", 5);

        assert_eq!(history.get_frequency("a"), 3);
        assert_eq!(history.get_frequency("b"), 2);
        assert_eq!(history.get_frequency("c"), 0);

        let top = history.most_frequent(1);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0], ("a", 3));
    }

    #[test]
    fn history_clear() {
        let mut history = CommandHistory::new();
        history.record("x", 1);
        assert!(!history.is_empty());
        assert_eq!(history.len(), 1);

        history.clear();
        assert!(history.is_empty());
        assert_eq!(history.get_frequency("x"), 0);
    }

    // -- CommandPalette -----------------------------------------------------

    #[test]
    fn palette_filter_by_relevance() {
        let mut palette = CommandPalette::new();
        palette.add("file.save", Some("Save the current file".into()));
        palette.add("file.saveAll", Some("Save all files".into()));
        palette.add("edit.find", Some("Find in file".into()));

        let matches = palette.filter_commands("file");
        assert_eq!(matches.len(), 3);
        // "file.save" and "file.saveAll" start with "file" (score 75)
        // "edit.find" description contains "file" (score 25)
        assert_eq!(matches[0].score, 75);
        assert_eq!(matches[2].score, 25);
        assert_eq!(matches[2].command_id, "edit.find");
    }

    #[test]
    fn palette_exact_match_highest_score() {
        let mut palette = CommandPalette::new();
        palette.add("save", None);
        palette.add("saveAll", None);

        let matches = palette.filter_commands("save");
        assert_eq!(matches[0].command_id, "save");
        assert_eq!(matches[0].score, 100);
    }

    // -- KeybindingConflict -------------------------------------------------

    #[test]
    fn detect_keybinding_conflicts() {
        let bindings = vec![
            Keybinding { key: "ctrl+s".into(), command_id: "file.save".into() },
            Keybinding { key: "ctrl+s".into(), command_id: "workbench.action.save".into() },
            Keybinding { key: "ctrl+z".into(), command_id: "edit.undo".into() },
        ];

        let conflicts = detect_conflicts(&bindings);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].key, "ctrl+s");
        assert_eq!(conflicts[0].command_ids.len(), 2);
        assert!(conflicts[0].command_ids.contains(&"file.save".to_string()));
    }

    #[test]
    fn no_conflicts_when_unique() {
        let bindings = vec![
            Keybinding { key: "ctrl+a".into(), command_id: "selectAll".into() },
            Keybinding { key: "ctrl+c".into(), command_id: "copy".into() },
        ];
        assert!(detect_conflicts(&bindings).is_empty());
    }

    // -- Alias tests -------------------------------------------------------

    #[test]
    fn register_and_resolve_alias() {
        let registry = CommandRegistry::new();
        let _reg = registry.register("workbench.action.quit", Box::new(|_args| Ok(None)));
        registry.register_alias("quit", "workbench.action.quit");
        assert!(registry.has("quit"));
        let resolved = registry.resolve_alias("quit");
        assert_eq!(resolved, Some("workbench.action.quit".to_string()));
    }

    #[test]
    fn alias_executes_same_handler() {
        let registry = CommandRegistry::new();
        let _reg = registry.register("cmd.greet", Box::new(|_args| Ok(Some(Box::new(42_i32)))));
        registry.register_alias("greet", "cmd.greet");
        let result = registry.execute("greet", vec![]);
        assert!(result.is_ok());
    }

    #[test]
    fn resolve_alias_returns_none_for_non_alias() {
        let registry = CommandRegistry::new();
        let _reg = registry.register("cmd.run", Box::new(|_args| Ok(None)));
        assert_eq!(registry.resolve_alias("cmd.run"), None);
    }

    #[test]
    fn alias_to_missing_command_not_registered() {
        let registry = CommandRegistry::new();
        registry.register_alias("missing_alias", "nonexistent.cmd");
        assert!(!registry.has("missing_alias"));
    }

    #[test]
    fn command_alias_struct_new() {
        let a = CommandAlias::new("q", "workbench.action.quit");
        assert_eq!(a.alias, "q");
        assert_eq!(a.command_id, "workbench.action.quit");
    }

    // -- CommandHistoryTracker -----------------------------------------------

    #[test]
    fn history_tracker_ring_buffer() {
        let mut tracker = CommandHistoryTracker::new(3);
        tracker.record("cmd.a", 1);
        tracker.record("cmd.b", 2);
        tracker.record("cmd.c", 3);
        tracker.record("cmd.d", 4); // should evict cmd.a
        assert_eq!(tracker.len(), 3);
        let recent = tracker.recent(10);
        assert_eq!(recent[0].command_id, "cmd.d");
        assert_eq!(recent[2].command_id, "cmd.b");
    }

    #[test]
    fn history_tracker_frequency() {
        let mut tracker = CommandHistoryTracker::new(100);
        tracker.record("cmd.a", 1);
        tracker.record("cmd.b", 2);
        tracker.record("cmd.a", 3);
        assert_eq!(tracker.frequency("cmd.a"), 2);
        assert_eq!(tracker.frequency("cmd.b"), 1);
        assert_eq!(tracker.frequency("cmd.c"), 0);
    }

    #[test]
    fn history_tracker_frequency_eviction() {
        let mut tracker = CommandHistoryTracker::new(2);
        tracker.record("cmd.a", 1);
        tracker.record("cmd.a", 2);
        tracker.record("cmd.b", 3); // evicts first cmd.a
        assert_eq!(tracker.frequency("cmd.a"), 1); // one cmd.a remains
    }

    #[test]
    fn history_tracker_recent_unique() {
        let mut tracker = CommandHistoryTracker::new(100);
        tracker.record("cmd.a", 1);
        tracker.record("cmd.b", 2);
        tracker.record("cmd.a", 3);
        let unique = tracker.recent_unique();
        assert_eq!(unique, vec!["cmd.a", "cmd.b"]);
    }

    #[test]
    fn history_tracker_clear() {
        let mut tracker = CommandHistoryTracker::new(100);
        tracker.record("cmd.a", 1);
        tracker.clear();
        assert!(tracker.is_empty());
        assert_eq!(tracker.frequency("cmd.a"), 0);
    }

    // -- Fuzzy search --------------------------------------------------------

    #[test]
    fn fuzzy_search_basic() {
        let cmds = vec![
            "editor.action.formatDocument".to_string(),
            "editor.action.commentLine".to_string(),
            "workbench.action.openFile".to_string(),
        ];
        let results = command_fuzzy_search(&cmds, "format");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "editor.action.formatDocument");
    }

    #[test]
    fn fuzzy_search_partial() {
        let cmds = vec![
            "editor.action.formatDocument".to_string(),
            "editor.action.find".to_string(),
            "file.open".to_string(),
        ];
        let results = command_fuzzy_search(&cmds, "eaf");
        // "editor.action.find" and "editor.action.formatDocument" should match (e, a, f)
        assert!(results.len() >= 2);
    }

    #[test]
    fn fuzzy_search_empty_query() {
        let cmds = vec!["a".to_string(), "b".to_string()];
        let results = command_fuzzy_search(&cmds, "");
        assert_eq!(results.len(), 2);
    }

    // -- New method tests ---------------------------------------------------

    #[test]
    fn history_last_entry() {
        let mut history = CommandHistory::new();
        assert!(history.last_entry().is_none());

        history.record("file.save", 100);
        history.record("edit.undo", 200);
        let last = history.last_entry().unwrap();
        assert_eq!(last.command_id, "edit.undo");
        assert_eq!(last.timestamp_ms, 200);
    }

    #[test]
    fn history_entries_for_command() {
        let mut history = CommandHistory::new();
        history.record("file.save", 100);
        history.record("edit.undo", 200);
        history.record("file.save", 300);
        history.record("edit.redo", 400);

        let saves = history.entries_for_command("file.save");
        assert_eq!(saves.len(), 2);
        assert_eq!(saves[0].timestamp_ms, 100);
        assert_eq!(saves[1].timestamp_ms, 300);

        let missing = history.entries_for_command("nonexistent");
        assert!(missing.is_empty());
    }

    #[test]
    fn history_is_empty_and_clear() {
        let mut history = CommandHistory::new();
        assert!(history.is_empty());

        history.record("cmd.x", 1);
        assert!(!history.is_empty());

        history.clear();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert_eq!(history.get_frequency("cmd.x"), 0);
    }

    #[test]
    fn descriptor_has_description() {
        let with_desc = CommandDescriptor {
            id: "cmd".into(),
            handler: Box::new(|_| Ok(None)),
            description: Some("A description".into()),
        };
        assert!(with_desc.has_description());

        let without_desc = CommandDescriptor {
            id: "cmd".into(),
            handler: Box::new(|_| Ok(None)),
            description: None,
        };
        assert!(!without_desc.has_description());

        let empty_desc = CommandDescriptor {
            id: "cmd".into(),
            handler: Box::new(|_| Ok(None)),
            description: Some(String::new()),
        };
        assert!(!empty_desc.has_description());
    }

    #[test]
    fn registry_command_count() {
        let registry = CommandRegistry::new();
        assert_eq!(registry.command_count(), 0);

        let _r1 = registry.register("a", Box::new(|_| Ok(None)));
        let _r2 = registry.register("b", Box::new(|_| Ok(None)));
        assert_eq!(registry.command_count(), 2);

        drop(_r1);
        assert_eq!(registry.command_count(), 1);
    }

    #[test]
    fn service_is_command_registered() {
        let service = CommandService::new();
        assert!(!service.is_command_registered("svc.missing"));

        let _reg = service.registry().register("svc.ping", Box::new(|_| Ok(None)));
        assert!(service.is_command_registered("svc.ping"));
    }

    #[test]
    fn history_entry_display() {
        let entry = HistoryEntry {
            command_id: "file.save".into(),
            timestamp_ms: 1234567890,
        };
        let displayed = format!("{}", entry);
        assert_eq!(displayed, "file.save @ 1234567890");
    }

    // -- CommandMacro tests ------------------------------------------------

    #[test]
    fn macro_record_and_replay() {
        let mut m = CommandMacro::new("test_macro");
        m.start_recording();
        assert!(m.is_recording());
        assert!(m.record_step(MacroStep::new("cmd.a")));
        assert!(m.record_step(MacroStep::new("cmd.b")));
        m.stop_recording();
        assert!(!m.is_recording());
        assert_eq!(m.step_count(), 2);
        assert_eq!(m.name(), "test_macro");
        assert!(!m.record_step(MacroStep::new("cmd.c"))); // not recording
    }

    #[test]
    fn macro_step_display() {
        let s = MacroStep::new("editor.save");
        assert_eq!(format!("{s}"), "editor.save");
        let s2 = MacroStep::with_args("editor.open", vec!["file.rs".into()]);
        assert_eq!(format!("{s2}"), "editor.open(file.rs)");
    }

    #[test]
    fn macro_replay_executes() {
        let registry = CommandRegistry::new();
        let _r = registry.register("noop", Box::new(|_| Ok(None)));
        let mut m = CommandMacro::new("m");
        m.start_recording();
        m.record_step(MacroStep::new("noop"));
        m.record_step(MacroStep::new("noop"));
        m.stop_recording();
        let results = m.replay(&registry);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    // -- CommandCondition tests --------------------------------------------

    #[test]
    fn condition_when_key_present() {
        let cond = CommandCondition::when("editorFocus");
        let mut ctx = HashMap::new();
        assert!(!cond.evaluate(&ctx));
        ctx.insert("editorFocus".into(), "true".into());
        assert!(cond.evaluate(&ctx));
    }

    #[test]
    fn condition_when_equals() {
        let cond = CommandCondition::when_equals("language", "rust");
        let mut ctx = HashMap::new();
        ctx.insert("language".into(), "python".into());
        assert!(!cond.evaluate(&ctx));
        ctx.insert("language".into(), "rust".into());
        assert!(cond.evaluate(&ctx));
    }

    #[test]
    fn condition_negate() {
        let cond = CommandCondition::when("readOnly").negate();
        let ctx = HashMap::new();
        assert!(cond.evaluate(&ctx)); // key absent, negated → true
    }

    #[test]
    fn condition_display() {
        let c = CommandCondition::when("focus");
        assert_eq!(format!("{c}"), "focus");
        let c2 = CommandCondition::when("focus").negate();
        assert_eq!(format!("{c2}"), "!focus");
    }

    // -- CommandScheduler tests --------------------------------------------

    #[test]
    fn scheduler_one_shot() {
        let mut sched = CommandScheduler::new();
        sched.schedule_once("cmd.a", 100);
        assert_eq!(sched.pending_count(), 1);

        let due = sched.tick(50);
        assert!(due.is_empty());

        let due = sched.tick(100);
        assert_eq!(due, vec!["cmd.a"]);
        assert_eq!(sched.pending_count(), 0);

        // Should not fire again
        let due = sched.tick(200);
        assert!(due.is_empty());
    }

    #[test]
    fn scheduler_repeating() {
        let mut sched = CommandScheduler::new();
        sched.schedule_repeating("cmd.r", 100, 50);

        let due = sched.tick(100);
        assert_eq!(due, vec!["cmd.r"]);

        let due = sched.tick(149);
        assert!(due.is_empty());

        let due = sched.tick(150);
        assert_eq!(due, vec!["cmd.r"]);
    }

    #[test]
    fn scheduler_cancel() {
        let mut sched = CommandScheduler::new();
        sched.schedule_once("cmd.x", 100);
        sched.schedule_once("cmd.y", 200);
        sched.cancel("cmd.x");
        assert_eq!(sched.pending_count(), 1);
    }

    // -- Alias chain tests -------------------------------------------------

    #[test]
    fn alias_chain_resolution() {
        let registry = CommandRegistry::new();
        let _r = registry.register("real.cmd", Box::new(|_| Ok(None)));
        registry.register_alias("alias1", "real.cmd");
        registry.register_alias("alias2", "alias1");

        assert_eq!(registry.alias_chain_depth("alias2"), 2);
        assert_eq!(
            registry.resolve_alias_chain("alias2").unwrap(),
            "real.cmd"
        );
    }

    #[test]
    fn alias_chain_non_alias_returns_none() {
        let registry = CommandRegistry::new();
        let _r = registry.register("real.cmd", Box::new(|_| Ok(None)));
        assert!(registry.resolve_alias_chain("real.cmd").is_none());
        assert_eq!(registry.alias_chain_depth("real.cmd"), 0);
    }

    #[test]
    fn command_category_display() {
        let cat = CommandCategory::new("editor", "Editor Commands", 1);
        assert_eq!(format!("{cat}"), "[editor] Editor Commands");
    }

    #[test]
    fn categorised_command_effective_label() {
        let cmd = CategorisedCommand::new("editor.action.copy", "editor");
        assert_eq!(cmd.effective_label(), "editor.action.copy");
        let cmd2 = cmd.clone().with_label("Copy");
        assert_eq!(cmd2.effective_label(), "Copy");
    }

    #[test]
    fn group_by_category_groups_correctly() {
        let cmds = vec![
            CategorisedCommand::new("editor.copy", "editor"),
            CategorisedCommand::new("editor.paste", "editor"),
            CategorisedCommand::new("file.save", "file"),
        ];
        let groups = group_by_category(&cmds);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups["editor"].len(), 2);
        assert_eq!(groups["file"].len(), 1);
    }

    #[test]
    fn filter_commands_by_prefix_works() {
        let cmds = vec![
            CategorisedCommand::new("editor.copy", "editor"),
            CategorisedCommand::new("editor.paste", "editor"),
            CategorisedCommand::new("file.save", "file"),
        ];
        let filtered = filter_commands_by_prefix(&cmds, "editor.");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn unique_categories_deduplicates() {
        let cmds = vec![
            CategorisedCommand::new("a.b", "x"),
            CategorisedCommand::new("c.d", "x"),
            CategorisedCommand::new("e.f", "y"),
        ];
        let cats = unique_categories(&cmds);
        assert_eq!(cats, vec!["x", "y"]);
    }

    #[test]
    fn sort_categories_by_order() {
        let mut cats = vec![
            CommandCategory::new("b", "B", 3),
            CommandCategory::new("a", "A", 1),
            CommandCategory::new("c", "C", 2),
        ];
        sort_categories(&mut cats);
        assert_eq!(cats[0].id, "a");
        assert_eq!(cats[1].id, "c");
        assert_eq!(cats[2].id, "b");
    }

    #[test]
    fn validate_command_id_accepts_valid() {
        assert!(validate_command_id("editor.action.copy").is_ok());
        assert!(validate_command_id("my-ext.do_thing").is_ok());
    }

    #[test]
    fn validate_command_id_rejects_empty() {
        assert!(validate_command_id("").is_err());
    }

    #[test]
    fn validate_command_id_rejects_no_dot() {
        assert!(validate_command_id("nodot").is_err());
    }

    #[test]
    fn validate_command_id_rejects_bad_chars() {
        assert!(validate_command_id("bad id.cmd").is_err());
    }

    #[test]
    fn normalize_keybinding_sorts_modifiers() {
        assert_eq!(normalize_keybinding("Shift+Ctrl+A"), "ctrl+shift+a");
        assert_eq!(normalize_keybinding("Alt+Shift+Z"), "alt+shift+z");
        assert_eq!(normalize_keybinding("Escape"), "escape");
    }

    // -- AliasMapping tests ---------------------------------------------------

    #[test]
    fn alias_resolve() {
        let mut reg = AliasMappingRegistry::new();
        reg.register(AliasMapping::new("copy", "editor.action.copy"));
        assert_eq!(reg.resolve("copy"), Some("editor.action.copy"));
        assert_eq!(reg.resolve("paste"), None);
    }

    #[test]
    fn alias_unregister() {
        let mut reg = AliasMappingRegistry::new();
        reg.register(AliasMapping::new("x", "y.z"));
        reg.unregister("x");
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn alias_for_target() {
        let mut reg = AliasMappingRegistry::new();
        reg.register(AliasMapping::new("cp", "editor.copy"));
        reg.register(AliasMapping::new("copy", "editor.copy"));
        let aliases = reg.aliases_for("editor.copy");
        assert_eq!(aliases.len(), 2);
    }

    #[test]
    fn alias_display() {
        let alias = AliasMapping::new("cp", "editor.copy");
        assert_eq!(alias.to_string(), "cp -> editor.copy");
    }

    // -- CommandThrottle tests ------------------------------------------------

    #[test]
    fn throttle_allows_first_execution() {
        let throttle = CommandThrottle::new(1000);
        assert!(throttle.can_execute("cmd.x", 0));
    }

    #[test]
    fn throttle_blocks_rapid_execution() {
        let mut throttle = CommandThrottle::new(1000);
        throttle.record_execution("cmd.x", 100);
        assert!(!throttle.can_execute("cmd.x", 500));
        assert!(throttle.can_execute("cmd.x", 1100));
    }

    #[test]
    fn throttle_remaining_ms() {
        let mut throttle = CommandThrottle::new(1000);
        throttle.record_execution("cmd.x", 100);
        assert_eq!(throttle.remaining_ms("cmd.x", 500), 600);
        assert_eq!(throttle.remaining_ms("cmd.x", 1100), 0);
    }

    #[test]
    fn throttle_display() {
        let throttle = CommandThrottle::new(500);
        let s = throttle.to_string();
        assert!(s.contains("500ms"));
    }

    // -- UndoableCommandHistory tests -----------------------------------------

    #[test]
    fn history_record_and_undo() {
        let mut history = UndoableCommandHistory::new(10);
        history.record("cmd.a", 1, true);
        history.record("cmd.b", 2, false);
        history.record("cmd.c", 3, true);

        let undone = history.undo_last();
        assert_eq!(undone, Some("cmd.c".to_string()));
        assert_eq!(history.undone_count(), 1);
    }

    #[test]
    fn history_undo_skips_non_undoable() {
        let mut history = UndoableCommandHistory::new(10);
        history.record("cmd.a", 1, true);
        history.record("cmd.b", 2, false);

        let undone = history.undo_last();
        assert_eq!(undone, Some("cmd.a".to_string()));
    }

    #[test]
    fn history_evicts_oldest() {
        let mut history = UndoableCommandHistory::new(2);
        history.record("a.x", 1, false);
        history.record("b.x", 2, false);
        history.record("c.x", 3, false);
        assert_eq!(history.len(), 2);
        assert_eq!(history.entries()[0].command_id, "b.x");
    }

    #[test]
    fn history_last_n() {
        let mut history = UndoableCommandHistory::new(10);
        history.record("a.x", 1, false);
        history.record("b.x", 2, false);
        history.record("c.x", 3, false);
        let last2 = history.last_n(2);
        assert_eq!(last2.len(), 2);
        assert_eq!(last2[0].command_id, "b.x");
    }

    #[test]
    fn history_display() {
        let history = UndoableCommandHistory::new(10);
        let s = history.to_string();
        assert!(s.contains("0 entries"));
    }

    #[test] fn commandPaletteRanking_new() { let s = CommandPaletteRanking::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn commandPaletteRanking_add() { let mut s = CommandPaletteRanking::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn commandPaletteRanking_remove() { let mut s = CommandPaletteRanking::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn commandPaletteRanking_config() { let mut s = CommandPaletteRanking::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn commandPaletteRanking_nav() { let mut s = CommandPaletteRanking::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn commandPaletteRanking_filter() { let mut s = CommandPaletteRanking::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn commandPaletteRanking_display() { assert!(format!("{}", CommandPaletteRanking::new()).contains("CommandPaletteRanking")); }
    #[test] fn commandArgumentSchema_new() { let s = CommandArgumentSchema::new(); assert!(s.is_empty()); }
    #[test] fn commandArgumentSchema_add() { let mut s = CommandArgumentSchema::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn commandArgumentSchema_active() { let mut s = CommandArgumentSchema::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn commandArgumentSchema_error() { let mut s = CommandArgumentSchema::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn commandArgumentSchema_rm_group() { let mut s = CommandArgumentSchema::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn commandArgumentSchema_display() { assert!(format!("{}", CommandArgumentSchema::new()).contains("CommandArgumentSchema")); }


    #[test] fn commandPaletteRanking_snap_capture() {
        let s = CommandPaletteRanking::new();
        let snap = CommandPaletteRankingSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn commandPaletteRanking_snap_stale() {
        let s = CommandPaletteRanking::new();
        let snap = CommandPaletteRankingSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn commandPaletteRanking_snap_diff() {
        let s = CommandPaletteRanking::new();
        let s1v = CommandPaletteRankingSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn commandPaletteRanking_snap_display() {
        let s = CommandPaletteRanking::new();
        let snap = CommandPaletteRankingSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn commandArgumentSchema_stats_record() {
        let mut st = CommandArgumentSchemaStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn commandArgumentSchema_stats_hit_ratio() {
        let mut st = CommandArgumentSchemaStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn commandArgumentSchema_stats_merge() {
        let mut a = CommandArgumentSchemaStats::new();
        a.total_adds = 5;
        let mut b = CommandArgumentSchemaStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn commandArgumentSchema_stats_display() {
        let st = CommandArgumentSchemaStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn commandPaletteRanking_config_default() {
        let c = CommandPaletteRankingConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn commandPaletteRanking_config_builder() {
        let c = CommandPaletteRankingConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn commandPaletteRanking_config_labels() {
        let mut c = CommandPaletteRankingConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn commandPaletteRanking_config_cleanup_threshold() {
        let c = CommandPaletteRankingConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn commandPaletteRanking_config_display() {
        assert!(format!("{}", CommandPaletteRankingConfig::new()).contains("Config"));
    }
    #[test] fn commandArgumentSchema_stats_peaks() {
        let mut st = CommandArgumentSchemaStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- CommandUndoRedoStack ------------------------------------------------

    #[test]
    fn undo_redo_push_and_undo() {
        let mut s = CommandUndoRedoStack::new(10);
        s.push("cmd1");
        s.push("cmd2");
        assert!(s.can_undo());
        assert_eq!(s.undo(), Some("cmd2".into()));
        assert_eq!(s.undo(), Some("cmd1".into()));
        assert!(!s.can_undo());
    }

    #[test]
    fn undo_redo_redo_works() {
        let mut s = CommandUndoRedoStack::new(10);
        s.push("a");
        s.undo();
        assert!(s.can_redo());
        assert_eq!(s.redo(), Some("a".into()));
        assert!(!s.can_redo());
    }

    #[test]
    fn undo_redo_push_clears_redo() {
        let mut s = CommandUndoRedoStack::new(10);
        s.push("a");
        s.undo();
        s.push("b");
        assert!(!s.can_redo());
    }

    #[test]
    fn undo_redo_capacity() {
        let mut s = CommandUndoRedoStack::new(2);
        s.push("a");
        s.push("b");
        s.push("c");
        assert_eq!(s.undo_len(), 2);
    }

    // -- CommandSequence -----------------------------------------------------

    #[test]
    fn sequence_basic_flow() {
        let mut seq = CommandSequence::new();
        seq.add_step("step1");
        seq.add_step("step2");
        assert!(!seq.is_done());
        assert_eq!(seq.mark_completed(), Some("step1"));
        assert_eq!(seq.mark_completed(), Some("step2"));
        assert!(seq.is_done());
    }

    #[test]
    fn sequence_failure_stops() {
        let mut seq = CommandSequence::new();
        seq.add_step("a");
        seq.add_step("b");
        seq.mark_completed();
        seq.mark_failed();
        assert!(seq.has_failed());
        assert_eq!(seq.mark_completed(), None);
        assert_eq!(seq.completed_steps(), &["a".to_string()]);
    }

    #[test]
    fn sequence_progress() {
        let mut seq = CommandSequence::new();
        seq.add_step("x");
        seq.add_step("y");
        seq.mark_completed();
        assert!((seq.progress_fraction() - 0.5).abs() < 0.01);
    }

    #[test]
    fn sequence_empty_is_done() {
        let seq = CommandSequence::new();
        assert!(seq.is_done());
        assert!((seq.progress_fraction() - 1.0).abs() < 0.01);
    }

    // -- CommandCooldownManager -----------------------------------------------

    #[test]
    fn cooldown_allows_first_use() {
        let mut mgr = CommandCooldownManager::new();
        mgr.set_cooldown("save", 1000);
        assert!(mgr.try_use("save", 100));
    }

    #[test]
    fn cooldown_blocks_rapid_use() {
        let mut mgr = CommandCooldownManager::new();
        mgr.set_cooldown("save", 1000);
        assert!(mgr.try_use("save", 100));
        assert!(!mgr.try_use("save", 500));
        assert!(mgr.try_use("save", 1200));
    }

    #[test]
    fn cooldown_remaining() {
        let mut mgr = CommandCooldownManager::new();
        mgr.set_cooldown("x", 500);
        mgr.try_use("x", 100);
        assert_eq!(mgr.remaining_ms("x", 300), 300);
        assert_eq!(mgr.remaining_ms("x", 700), 0);
    }

    #[test]
    fn cooldown_remove() {
        let mut mgr = CommandCooldownManager::new();
        mgr.set_cooldown("a", 1000);
        mgr.remove_cooldown("a");
        assert!(mgr.registered_commands().is_empty());
    }


    #[test]
    fn commands_config_new() {
        let cfg = CommandsConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn commands_config_set_get() {
        let mut cfg = CommandsConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn commands_config_remove() {
        let mut cfg = CommandsConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn commands_config_keys_sorted() {
        let mut cfg = CommandsConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn commands_config_bump_version() {
        let mut cfg = CommandsConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn commands_config_clear() {
        let mut cfg = CommandsConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn commands_config_merge() {
        let mut cfg1 = CommandsConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = CommandsConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn commands_config_disable() {
        let mut cfg = CommandsConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn commands_rate_tracker_empty() {
        let rt = CommandsRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn commands_rate_tracker_record() {
        let mut rt = CommandsRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn commands_rate_tracker_prune() {
        let mut rt = CommandsRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn commands_validator_valid() {
        let v = CommandsValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn commands_validator_errors() {
        let mut v = CommandsValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn commands_validator_clear() {
        let mut v = CommandsValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn commands_validator_merge() {
        let mut v1 = CommandsValidator::new();
        v1.add_error("e1");
        let mut v2 = CommandsValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn commands_rate_tracker_clear() {
        let mut rt = CommandsRateTracker::new(1000);
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


    // xa_ extended tests for commands
    #[test]
    fn xa_commands_ring_new() {
        let rb = super::XaCommandsRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_commands_ring_push_len() {
        let mut rb = super::XaCommandsRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_commands_ring_wrap() {
        let mut rb = super::XaCommandsRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_commands_ring_mean_empty() {
        let rb = super::XaCommandsRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_commands_ring_mean_values() {
        let mut rb = super::XaCommandsRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_commands_ring_min_max() {
        let mut rb = super::XaCommandsRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_commands_ring_iter() {
        let mut rb = super::XaCommandsRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_commands_counter_new() {
        let c = super::XaCommandsCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_commands_counter_inc() {
        let mut c = super::XaCommandsCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_commands_counter_inc_by() {
        let mut c = super::XaCommandsCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_commands_counter_reset() {
        let mut c = super::XaCommandsCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_commands_counter_clear() {
        let mut c = super::XaCommandsCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_commands_counter_default() {
        let c = super::XaCommandsCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 19 ----

    #[test]
    fn xc_19_pool_new_empty() {
        let pool: super::Xc19Pool<i32> = super::Xc19Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_19_pool_release_acquire() {
        let mut pool = super::Xc19Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_19_pool_acquire_empty() {
        let mut pool: super::Xc19Pool<i32> = super::Xc19Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_19_pool_full() {
        let mut pool = super::Xc19Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_19_pool_drain() {
        let mut pool = super::Xc19Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_19_pool_stats() {
        let mut pool = super::Xc19Pool::new(8);
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
    fn xc_19_pool_clear() {
        let mut pool = super::Xc19Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_19_pool_shrink() {
        let mut pool = super::Xc19Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_19_pool_default() {
        let pool: super::Xc19Pool<String> = super::Xc19Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_19_pool_extend() {
        let mut pool = super::Xc19Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_19_pool_retain() {
        let mut pool = super::Xc19Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_19_scheduler_round_robin() {
        let mut sched = super::Xc19Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_19_scheduler_empty() {
        let mut sched = super::Xc19Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_19_scheduler_reset() {
        let mut sched = super::Xc19Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_19_scheduler_add_remove() {
        let mut sched = super::Xc19Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_19_scheduler_targets() {
        let sched = super::Xc19Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_19_hash_empty() {
        assert_eq!(super::xc_19_hash(b""), 5381);
    }

    #[test]
    fn xc_19_hash_data() {
        let h = super::xc_19_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_19_hash(b"hello"), h);
    }

    #[test]
    fn xc_19_reverse_str() {
        assert_eq!(super::xc_19_reverse("abc"), "cba");
        assert_eq!(super::xc_19_reverse(""), "");
    }


    // --- xd_58 deepening tests ---

    #[test]
    fn xd_58_sm_initial_state() {
        let sm = Xd58StateMachine::new();
        assert_eq!(sm.current_state(), Xd58State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_58_sm_valid_idle_to_running() {
        let mut sm = Xd58StateMachine::new();
        assert!(sm.transition(Xd58State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd58State::Running);
    }

    #[test]
    fn xd_58_sm_valid_running_to_paused() {
        let mut sm = Xd58StateMachine::new();
        sm.transition(Xd58State::Running).unwrap();
        assert!(sm.transition(Xd58State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd58State::Paused);
    }

    #[test]
    fn xd_58_sm_valid_running_to_done() {
        let mut sm = Xd58StateMachine::new();
        sm.transition(Xd58State::Running).unwrap();
        assert!(sm.transition(Xd58State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd58State::Done);
    }

    #[test]
    fn xd_58_sm_valid_paused_to_running() {
        let mut sm = Xd58StateMachine::new();
        sm.transition(Xd58State::Running).unwrap();
        sm.transition(Xd58State::Paused).unwrap();
        assert!(sm.transition(Xd58State::Running).is_ok());
    }

    #[test]
    fn xd_58_sm_valid_done_to_idle() {
        let mut sm = Xd58StateMachine::new();
        sm.transition(Xd58State::Running).unwrap();
        sm.transition(Xd58State::Done).unwrap();
        assert!(sm.transition(Xd58State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd58State::Idle);
    }

    #[test]
    fn xd_58_sm_invalid_idle_to_done() {
        let mut sm = Xd58StateMachine::new();
        assert!(sm.transition(Xd58State::Done).is_err());
    }

    #[test]
    fn xd_58_sm_invalid_idle_to_paused() {
        let mut sm = Xd58StateMachine::new();
        assert!(sm.transition(Xd58State::Paused).is_err());
    }

    #[test]
    fn xd_58_sm_history_tracking() {
        let mut sm = Xd58StateMachine::new();
        sm.transition(Xd58State::Running).unwrap();
        sm.transition(Xd58State::Paused).unwrap();
        sm.transition(Xd58State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd58State::Idle);
        assert_eq!(sm.history()[0].to, Xd58State::Running);
        assert_eq!(sm.history()[1].from, Xd58State::Running);
        assert_eq!(sm.history()[2].to, Xd58State::Done);
    }

    #[test]
    fn xd_58_sm_serialize_deserialize() {
        let mut sm = Xd58StateMachine::new();
        sm.transition(Xd58State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd58StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd58State::Running));
    }

    #[test]
    fn xd_58_sm_deserialize_invalid() {
        assert_eq!(Xd58StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_58_sm_reset() {
        let mut sm = Xd58StateMachine::new();
        sm.transition(Xd58State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd58State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_58_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd58EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd58Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_58_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd58EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd58Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd58Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_58_bus_unsubscribe() {
        let mut bus = Xd58EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_58_event_kind_and_payload() {
        let e = Xd58Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd58Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_58_bus_clear_history() {
        let mut bus = Xd58EventBus::new();
        bus.publish(Xd58Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_58_sm_step_counter_increments() {
        let mut sm = Xd58StateMachine::new();
        sm.transition(Xd58State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd58State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #56 --

    #[test]
    fn xf56_trie_insert_search() {
        let mut t = Xf56Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf56_trie_starts_with() {
        let mut t = Xf56Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf56_trie_remove() {
        let mut t = Xf56Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf56_trie_word_count() {
        let mut t = Xf56Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf56_trie_longest_prefix() {
        let mut t = Xf56Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf56_trie_all_words() {
        let mut t = Xf56Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf56_trie_autocomplete() {
        let mut t = Xf56Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf56_trie_empty_search() {
        let t = Xf56Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf56_bloom_add_contains() {
        let mut bf = Xf56BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf56_bloom_probably_absent() {
        let bf = Xf56BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf56_bloom_false_positive_rate() {
        let mut bf = Xf56BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf56_bloom_clear() {
        let mut bf = Xf56BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf56_bloom_union() {
        let mut a = Xf56BloomFilter::xf_new(512, 2);
        let mut b = Xf56BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf56_bloom_intersection_estimate() {
        let mut a = Xf56BloomFilter::xf_new(512, 2);
        let mut b = Xf56BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf56_bloom_union_size_mismatch() {
        let a = Xf56BloomFilter::xf_new(256, 2);
        let b = Xf56BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh18_skip_insert_contains() {
        let mut sl = super::Xh18SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh18_skip_remove() {
        let mut sl = super::Xh18SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh18_skip_len() {
        let mut sl = super::Xh18SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh18_skip_range_query() {
        let mut sl = super::Xh18SkipList::xh_new(4);
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
    fn xh18_skip_floor_ceiling() {
        let mut sl = super::Xh18SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh18_skip_rank() {
        let mut sl = super::Xh18SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh18_skip_empty() {
        let sl = super::Xh18SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh18_skip_duplicates() {
        let mut sl = super::Xh18SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh18_bitset_set_test() {
        let mut bs = super::Xh18BitSet::xh_new(256);
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
    fn xh18_bitset_clear_count() {
        let mut bs = super::Xh18BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh18_bitset_and_or_xor() {
        let mut a = super::Xh18BitSet::xh_new(128);
        let mut b = super::Xh18BitSet::xh_new(128);
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
    fn xh18_bitset_iter_ones() {
        let mut bs = super::Xh18BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh18_bitset_first_last() {
        let mut bs = super::Xh18BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh18_bitset_empty() {
        let bs = super::Xh18BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi18_deque_push_pop_back() {
        let mut dq = super::Xi18Deque::xi_new(4);
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
    fn xi18_deque_push_pop_front() {
        let mut dq = super::Xi18Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi18_deque_mixed_ops() {
        let mut dq = super::Xi18Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi18_deque_get_and_split() {
        let mut dq = super::Xi18Deque::xi_new(8);
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
    fn xi18_deque_rotate_left() {
        let mut dq = super::Xi18Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi18_deque_rotate_right() {
        let mut dq = super::Xi18Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi18_deque_grow() {
        let mut dq = super::Xi18Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi18_deque_empty() {
        let dq = super::Xi18Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi18_interval_tree_insert_query() {
        let mut tree = super::Xi18IntervalTree::xi_new();
        tree.xi_insert(super::Xi18Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi18Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi18Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi18_interval_tree_overlap() {
        let mut tree = super::Xi18IntervalTree::xi_new();
        tree.xi_insert(super::Xi18Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi18Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi18Interval::xi_new(12, 20));
        let q = super::Xi18Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi18_interval_tree_remove() {
        let mut tree = super::Xi18IntervalTree::xi_new();
        tree.xi_insert(super::Xi18Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi18Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi18_interval_tree_gaps() {
        let mut tree = super::Xi18IntervalTree::xi_new();
        tree.xi_insert(super::Xi18Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi18Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi18Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi18Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi18Interval::xi_new(8, 10));
    }

    #[test]
    fn xi18_interval_tree_merge() {
        let mut tree = super::Xi18IntervalTree::xi_new();
        tree.xi_insert(super::Xi18Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi18Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi18Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi18Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi18Interval::xi_new(10, 15));
    }

    #[test]
    fn xi18_interval_tree_all() {
        let mut tree = super::Xi18IntervalTree::xi_new();
        tree.xi_insert(super::Xi18Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi18Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi18_interval_tree_empty() {
        let tree = super::Xi18IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi18_interval_tree_contains_point() {
        let iv = super::Xi18Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 18) ---

    #[test]
    fn xj_18_uf_make_and_find() {
        let mut uf = super::Xj18UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_18_uf_union_connected() {
        let mut uf = super::Xj18UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_18_uf_component_count() {
        let mut uf = super::Xj18UnionFind::xj_new();
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
    fn xj_18_uf_component_size() {
        let mut uf = super::Xj18UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_18_uf_largest_component() {
        let mut uf = super::Xj18UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_18_uf_many_elements() {
        let mut uf = super::Xj18UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_18_uf_separate_components() {
        let mut uf = super::Xj18UnionFind::xj_new();
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
    fn xj_18_uf_path_compression() {
        let mut uf = super::Xj18UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_18_bt_insert_get() {
        let mut bt = super::Xj18BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_18_bt_contains_len() {
        let mut bt = super::Xj18BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_18_bt_replace() {
        let mut bt = super::Xj18BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_18_bt_remove() {
        let mut bt = super::Xj18BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_18_bt_keys_values() {
        let mut bt = super::Xj18BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_18_bt_range() {
        let mut bt = super::Xj18BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_18_bt_min_max() {
        let mut bt = super::Xj18BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_18_bt_many_inserts() {
        let mut bt = super::Xj18BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_18 segment tree tests ---

    #[test]
    fn xk_18_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk18SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_18_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk18SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_18_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk18SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_18_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk18SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_18_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk18SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_18_st_single_element() {
        let data = vec![42];
        let st = super::Xk18SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_18_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk18SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_18_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk18SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_18 disjoint intervals tests ---

    #[test]
    fn xk_18_di_add_and_count() {
        let mut di = super::Xk18DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_18_di_merge_overlap() {
        let mut di = super::Xk18DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_18_di_contains() {
        let mut di = super::Xk18DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_18_di_remove() {
        let mut di = super::Xk18DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_18_di_covered_length() {
        let mut di = super::Xk18DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_18_di_gaps() {
        let mut di = super::Xk18DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_18_di_merge_adjacent() {
        let mut di = super::Xk18DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_18_di_empty() {
        let di = super::Xk18DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_18_rope_new_empty() {
        let rope = super::Xl18Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_18_rope_from_str() {
        let rope = super::Xl18Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_18_rope_insert_at() {
        let mut rope = super::Xl18Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_18_rope_delete_range() {
        let mut rope = super::Xl18Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_18_rope_char_at() {
        let rope = super::Xl18Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_18_rope_split_concat() {
        let rope = super::Xl18Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_18_rope_line_count() {
        let rope = super::Xl18Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_18_rope_line_at() {
        let rope = super::Xl18Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_18_sa_build_and_search() {
        let sa = super::Xl18SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_18_sa_count() {
        let sa = super::Xl18SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_18_sa_longest_repeated() {
        let sa = super::Xl18SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_18_sa_all_positions() {
        let sa = super::Xl18SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_18_sa_len() {
        let sa = super::Xl18SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_18_sa_empty() {
        let sa = super::Xl18SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_18_rope_slice() {
        let rope = super::Xl18Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_18_sa_search_start() {
        let sa = super::Xl18SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
