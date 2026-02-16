//! Keybinding editor UI – key codes, modifiers, bindings, and conflict detection.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Enter,
    Escape,
    Tab,
    Backspace,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Char(char),
    F(u8),
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub fn none() -> Self {
        Self { ctrl: false, shift: false, alt: false, meta: false }
    }
}

#[derive(Debug, Clone)]
pub struct Keybinding {
    pub key: KeyCode,
    pub modifiers: Modifiers,
    pub command: String,
    pub when_clause: Option<String>,
}

#[derive(Debug)]
pub struct KeybindingConflict {
    pub bindings: Vec<Keybinding>,
}

pub struct KeybindingRegistry {
    bindings: Vec<Keybinding>,
}

impl KeybindingRegistry {
    pub fn new() -> Self {
        Self { bindings: Vec::new() }
    }

    pub fn add(&mut self, binding: Keybinding) {
        self.bindings.push(binding);
    }

    pub fn find_by_command(&self, cmd: &str) -> Vec<&Keybinding> {
        self.bindings.iter().filter(|b| b.command == cmd).collect()
    }

    /// Find groups of bindings that share the same key + modifiers.
    pub fn find_conflicts(&self) -> Vec<KeybindingConflict> {
        use std::collections::HashMap;
        let mut groups: HashMap<(KeyCode, Modifiers), Vec<&Keybinding>> = HashMap::new();
        for b in &self.bindings {
            groups
                .entry((b.key.clone(), b.modifiers.clone()))
                .or_default()
                .push(b);
        }
        groups
            .into_values()
            .filter(|g| g.len() > 1)
            .map(|g| KeybindingConflict {
                bindings: g.into_iter().cloned().collect(),
            })
            .collect()
    }

    pub fn format_keybinding(binding: &Keybinding) -> String {
        let mut parts = Vec::new();
        if binding.modifiers.ctrl {
            parts.push("Ctrl");
        }
        if binding.modifiers.shift {
            parts.push("Shift");
        }
        if binding.modifiers.alt {
            parts.push("Alt");
        }
        if binding.modifiers.meta {
            parts.push("Meta");
        }
        let key_str = match &binding.key {
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Escape => "Escape".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Space => "Space".to_string(),
            KeyCode::ArrowUp => "Up".to_string(),
            KeyCode::ArrowDown => "Down".to_string(),
            KeyCode::ArrowLeft => "Left".to_string(),
            KeyCode::ArrowRight => "Right".to_string(),
            KeyCode::Char(c) => c.to_uppercase().to_string(),
            KeyCode::F(n) => format!("F{n}"),
            KeyCode::Delete => "Delete".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "PageUp".to_string(),
            KeyCode::PageDown => "PageDown".to_string(),
        };
        parts.push(&key_str);
        parts.join("+")
    }
}

impl Default for KeybindingRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb(key: KeyCode, ctrl: bool, cmd: &str) -> Keybinding {
        Keybinding {
            key,
            modifiers: Modifiers { ctrl, shift: false, alt: false, meta: false },
            command: cmd.to_string(),
            when_clause: None,
        }
    }

    #[test]
    fn find_by_command() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        reg.add(kb(KeyCode::Char('o'), true, "open"));
        let found = reg.find_by_command("save");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].command, "save");
    }

    #[test]
    fn detect_conflicts() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        reg.add(kb(KeyCode::Char('s'), true, "search"));
        let conflicts = reg.find_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].bindings.len(), 2);
    }

    #[test]
    fn format_keybinding() {
        let binding = Keybinding {
            key: KeyCode::Char('s'),
            modifiers: Modifiers { ctrl: true, shift: true, alt: false, meta: false },
            command: "saveAs".to_string(),
            when_clause: None,
        };
        assert_eq!(KeybindingRegistry::format_keybinding(&binding), "Ctrl+Shift+S");
    }

    #[test]
    fn no_conflicts_different_keys() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        reg.add(kb(KeyCode::Char('o'), true, "open"));
        assert!(reg.find_conflicts().is_empty());
    }
}
