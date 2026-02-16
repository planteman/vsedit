//! Workbench command execution.

/// Origin of a command invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSource {
    User,
    Extension,
    System,
    Keybinding,
}

/// Descriptor for a registered command.
#[derive(Debug, Clone)]
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
}
