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
}
