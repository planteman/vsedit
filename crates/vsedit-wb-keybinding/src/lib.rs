//! User keybinding service.

use std::fmt;

/// Errors that can occur during keybinding operations.
#[derive(Debug, Clone, PartialEq)]
pub enum KeybindingError {
    BindingNotFound,
    DuplicateBinding,
    ConflictingBinding(String),
}

impl fmt::Display for KeybindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeybindingError::BindingNotFound => write!(f, "binding not found"),
            KeybindingError::DuplicateBinding => write!(f, "duplicate binding"),
            KeybindingError::ConflictingBinding(cmd) => {
                write!(f, "conflicting binding for command '{cmd}'")
            }
        }
    }
}

/// Modifier keys for keybindings.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyMod {
    CtrlCmd,
    Shift,
    Alt,
    WinCtrl,
}

impl fmt::Display for KeyMod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyMod::CtrlCmd => write!(f, "Ctrl"),
            KeyMod::Shift => write!(f, "Shift"),
            KeyMod::Alt => write!(f, "Alt"),
            KeyMod::WinCtrl => write!(f, "Win"),
        }
    }
}

/// Origin of a keybinding.
#[derive(Debug, Clone, PartialEq)]
pub enum KeybindingSource {
    Default,
    User,
    Extension,
}

impl fmt::Display for KeybindingSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeybindingSource::Default => write!(f, "Default"),
            KeybindingSource::User => write!(f, "User"),
            KeybindingSource::Extension => write!(f, "Extension"),
        }
    }
}

/// A fully resolved keybinding.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedKeybinding {
    pub key: String,
    pub modifiers: Vec<KeyMod>,
    pub command: String,
    pub when: Option<String>,
    pub source: KeybindingSource,
}

impl fmt::Display for ResolvedKeybinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let formatted = KeybindingService::format_binding(self);
        write!(f, "{} -> {} [{}]", formatted, self.command, self.source)
    }
}

impl ResolvedKeybinding {
    /// Check if the given key and modifiers match this binding.
    pub fn matches(&self, key: &str, modifiers: &[KeyMod]) -> bool {
        self.key == key && self.modifiers == modifiers
    }
}

/// Builder for constructing a `ResolvedKeybinding` step by step.
pub struct KeybindingBuilder {
    key: Option<String>,
    modifiers: Vec<KeyMod>,
    command: Option<String>,
    when: Option<String>,
    source: KeybindingSource,
}

impl KeybindingBuilder {
    pub fn new() -> Self {
        Self {
            key: None,
            modifiers: Vec::new(),
            command: None,
            when: None,
            source: KeybindingSource::Default,
        }
    }

    pub fn key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }

    pub fn modifier(mut self, modifier: KeyMod) -> Self {
        self.modifiers.push(modifier);
        self
    }

    pub fn command(mut self, command: &str) -> Self {
        self.command = Some(command.to_string());
        self
    }

    pub fn when(mut self, when: &str) -> Self {
        self.when = Some(when.to_string());
        self
    }

    pub fn source(mut self, source: KeybindingSource) -> Self {
        self.source = source;
        self
    }

    /// Build the `ResolvedKeybinding`. Panics if `key` or `command` is not set.
    pub fn build(self) -> ResolvedKeybinding {
        ResolvedKeybinding {
            key: self.key.expect("key is required"),
            modifiers: self.modifiers,
            command: self.command.expect("command is required"),
            when: self.when,
            source: self.source,
        }
    }
}

impl Default for KeybindingBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Service for keybinding workbench functionality.
pub struct KeybindingService {
    pub bindings: Vec<ResolvedKeybinding>,
}

impl KeybindingService {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    pub fn register(&mut self, binding: ResolvedKeybinding) {
        self.bindings.push(binding);
    }

    /// Register a binding, returning an error if a conflicting binding already exists.
    pub fn try_register(
        &mut self,
        binding: ResolvedKeybinding,
    ) -> Result<(), KeybindingError> {
        if self.has_conflict(&binding.key, &binding.modifiers) {
            let existing = self.resolve(&binding.key, &binding.modifiers);
            let cmd = existing[0].command.clone();
            return Err(KeybindingError::ConflictingBinding(cmd));
        }
        self.bindings.push(binding);
        Ok(())
    }

    pub fn resolve(&self, key: &str, modifiers: &[KeyMod]) -> Vec<&ResolvedKeybinding> {
        self.bindings
            .iter()
            .filter(|b| b.key == key && b.modifiers == modifiers)
            .collect()
    }

    pub fn get_bindings_for_command(&self, cmd: &str) -> Vec<&ResolvedKeybinding> {
        self.bindings
            .iter()
            .filter(|b| b.command == cmd)
            .collect()
    }

    pub fn remove_binding(&mut self, command: &str, key: &str) -> bool {
        let before = self.bindings.len();
        self.bindings
            .retain(|b| !(b.command == command && b.key == key));
        self.bindings.len() < before
    }

    pub fn format_binding(binding: &ResolvedKeybinding) -> String {
        let mods: Vec<&str> = binding
            .modifiers
            .iter()
            .map(|m| match m {
                KeyMod::CtrlCmd => "Ctrl",
                KeyMod::Shift => "Shift",
                KeyMod::Alt => "Alt",
                KeyMod::WinCtrl => "Win",
            })
            .collect();
        if mods.is_empty() {
            binding.key.clone()
        } else {
            format!("{}+{}", mods.join("+"), binding.key)
        }
    }

    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    /// Check if a key+modifiers combination has multiple bindings.
    pub fn has_conflict(&self, key: &str, modifiers: &[KeyMod]) -> bool {
        self.resolve(key, modifiers).len() > 1
    }

    /// Return all sets of bindings that share the same key+modifiers.
    pub fn get_conflicts(&self) -> Vec<Vec<&ResolvedKeybinding>> {
        let mut groups: Vec<Vec<&ResolvedKeybinding>> = Vec::new();
        for binding in &self.bindings {
            let found = groups.iter_mut().find(|g| {
                g[0].key == binding.key && g[0].modifiers == binding.modifiers
            });
            match found {
                Some(group) => group.push(binding),
                None => groups.push(vec![binding]),
            }
        }
        groups.into_iter().filter(|g| g.len() > 1).collect()
    }

    /// Filter bindings by source.
    pub fn get_by_source(&self, source: &KeybindingSource) -> Vec<&ResolvedKeybinding> {
        self.bindings
            .iter()
            .filter(|b| &b.source == source)
            .collect()
    }

    /// Remove all bindings.
    pub fn clear(&mut self) {
        self.bindings.clear();
    }

    /// Return all bindings as formatted strings.
    pub fn export_bindings(&self) -> Vec<String> {
        self.bindings.iter().map(|b| b.to_string()).collect()
    }
}

impl Default for KeybindingService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_binding(key: &str, command: &str, modifiers: Vec<KeyMod>) -> ResolvedKeybinding {
        ResolvedKeybinding {
            key: key.to_string(),
            modifiers,
            command: command.to_string(),
            when: None,
            source: KeybindingSource::Default,
        }
    }

    fn sample_binding_src(
        key: &str,
        command: &str,
        modifiers: Vec<KeyMod>,
        source: KeybindingSource,
    ) -> ResolvedKeybinding {
        ResolvedKeybinding {
            key: key.to_string(),
            modifiers,
            command: command.to_string(),
            when: None,
            source,
        }
    }

    #[test]
    fn register_and_resolve() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("S", "save_all", vec![KeyMod::CtrlCmd, KeyMod::Shift]));

        let found = svc.resolve("S", &[KeyMod::CtrlCmd]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].command, "save");
    }

    #[test]
    fn remove_and_count() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("P", "palette", vec![KeyMod::CtrlCmd, KeyMod::Shift]));
        svc.register(sample_binding("N", "new_file", vec![KeyMod::CtrlCmd]));
        assert_eq!(svc.binding_count(), 2);

        assert!(svc.remove_binding("palette", "P"));
        assert_eq!(svc.binding_count(), 1);
        assert!(!svc.remove_binding("palette", "P"));
    }

    #[test]
    fn format_and_lookup_by_command() {
        let mut svc = KeybindingService::new();
        let binding = sample_binding("C", "copy", vec![KeyMod::CtrlCmd]);
        svc.register(binding.clone());

        assert_eq!(KeybindingService::format_binding(&binding), "Ctrl+C");

        let results = svc.get_bindings_for_command("copy");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "C");
    }

    #[test]
    fn keybinding_error_display() {
        assert_eq!(KeybindingError::BindingNotFound.to_string(), "binding not found");
        assert_eq!(KeybindingError::DuplicateBinding.to_string(), "duplicate binding");
        assert_eq!(
            KeybindingError::ConflictingBinding("save".into()).to_string(),
            "conflicting binding for command 'save'"
        );
    }

    #[test]
    fn display_keymod() {
        assert_eq!(KeyMod::CtrlCmd.to_string(), "Ctrl");
        assert_eq!(KeyMod::Shift.to_string(), "Shift");
        assert_eq!(KeyMod::Alt.to_string(), "Alt");
        assert_eq!(KeyMod::WinCtrl.to_string(), "Win");
    }

    #[test]
    fn display_keybinding_source() {
        assert_eq!(KeybindingSource::Default.to_string(), "Default");
        assert_eq!(KeybindingSource::User.to_string(), "User");
        assert_eq!(KeybindingSource::Extension.to_string(), "Extension");
    }

    #[test]
    fn display_resolved_keybinding() {
        let b = sample_binding("S", "save", vec![KeyMod::CtrlCmd]);
        assert_eq!(b.to_string(), "Ctrl+S -> save [Default]");

        let bare = sample_binding("F5", "debug.run", vec![]);
        assert_eq!(bare.to_string(), "F5 -> debug.run [Default]");
    }

    #[test]
    fn resolved_keybinding_matches() {
        let b = sample_binding("S", "save", vec![KeyMod::CtrlCmd]);
        assert!(b.matches("S", &[KeyMod::CtrlCmd]));
        assert!(!b.matches("S", &[KeyMod::Alt]));
        assert!(!b.matches("X", &[KeyMod::CtrlCmd]));
    }

    #[test]
    fn has_conflict_and_get_conflicts() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("S", "search", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("N", "new_file", vec![KeyMod::CtrlCmd]));

        assert!(svc.has_conflict("S", &[KeyMod::CtrlCmd]));
        assert!(!svc.has_conflict("N", &[KeyMod::CtrlCmd]));

        let conflicts = svc.get_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].len(), 2);
    }

    #[test]
    fn get_by_source() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding_src("S", "save", vec![KeyMod::CtrlCmd], KeybindingSource::Default));
        svc.register(sample_binding_src("S", "custom_save", vec![KeyMod::CtrlCmd], KeybindingSource::User));
        svc.register(sample_binding_src("E", "ext_cmd", vec![KeyMod::Alt], KeybindingSource::Extension));

        assert_eq!(svc.get_by_source(&KeybindingSource::Default).len(), 1);
        assert_eq!(svc.get_by_source(&KeybindingSource::User).len(), 1);
        assert_eq!(svc.get_by_source(&KeybindingSource::Extension).len(), 1);
    }

    #[test]
    fn clear_removes_all() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("A", "cmd_a", vec![]));
        svc.register(sample_binding("B", "cmd_b", vec![]));
        assert_eq!(svc.binding_count(), 2);
        svc.clear();
        assert_eq!(svc.binding_count(), 0);
    }

    #[test]
    fn try_register_success_and_conflict() {
        let mut svc = KeybindingService::new();
        let result = svc.try_register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        assert!(result.is_ok());

        // First registration — no conflict yet (only one binding)
        let result2 = svc.try_register(sample_binding("S", "search", vec![KeyMod::CtrlCmd]));
        assert!(result2.is_ok());

        // Now there are two bindings on the same key+modifiers → conflict
        let result3 = svc.try_register(sample_binding("S", "something", vec![KeyMod::CtrlCmd]));
        assert!(result3.is_err());
        assert_eq!(
            result3.unwrap_err(),
            KeybindingError::ConflictingBinding("save".into())
        );
    }

    #[test]
    fn builder_pattern() {
        let binding = KeybindingBuilder::new()
            .key("S")
            .modifier(KeyMod::CtrlCmd)
            .modifier(KeyMod::Shift)
            .command("save_all")
            .when("editorTextFocus")
            .source(KeybindingSource::User)
            .build();

        assert_eq!(binding.key, "S");
        assert_eq!(binding.modifiers, vec![KeyMod::CtrlCmd, KeyMod::Shift]);
        assert_eq!(binding.command, "save_all");
        assert_eq!(binding.when, Some("editorTextFocus".to_string()));
        assert_eq!(binding.source, KeybindingSource::User);
    }

    #[test]
    fn builder_defaults() {
        let binding = KeybindingBuilder::default()
            .key("F5")
            .command("debug.start")
            .build();

        assert!(binding.modifiers.is_empty());
        assert_eq!(binding.when, None);
        assert_eq!(binding.source, KeybindingSource::Default);
    }

    #[test]
    fn export_bindings() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("F5", "debug.run", vec![]));

        let exported = svc.export_bindings();
        assert_eq!(exported.len(), 2);
        assert_eq!(exported[0], "Ctrl+S -> save [Default]");
        assert_eq!(exported[1], "F5 -> debug.run [Default]");
    }
}
