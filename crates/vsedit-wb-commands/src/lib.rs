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
}
