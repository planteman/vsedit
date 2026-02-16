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
// Tests
// ---------------------------------------------------------------------------

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
}
