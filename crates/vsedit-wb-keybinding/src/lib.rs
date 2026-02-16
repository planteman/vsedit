//! User keybinding service.

/// Modifier keys for keybindings.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyMod {
    CtrlCmd,
    Shift,
    Alt,
    WinCtrl,
}

/// Origin of a keybinding.
#[derive(Debug, Clone, PartialEq)]
pub enum KeybindingSource {
    Default,
    User,
    Extension,
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
}
