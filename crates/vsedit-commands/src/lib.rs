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

}
