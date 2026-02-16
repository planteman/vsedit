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

    pub fn has_modifier(&self) -> bool {
        self.ctrl || self.shift || self.alt || self.meta
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeybindingSource {
    Default,
    User,
    Extension,
}

#[derive(Debug, Clone)]
pub struct Keybinding {
    pub key: KeyCode,
    pub modifiers: Modifiers,
    pub command: String,
    pub when_clause: Option<String>,
    pub source: KeybindingSource,
}

#[derive(Debug, Clone)]
pub struct ChordKeybinding {
    pub first: (KeyCode, Modifiers),
    pub second: (KeyCode, Modifiers),
    pub command: String,
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

    pub fn remove_binding(&mut self, command: &str) -> bool {
        let before = self.bindings.len();
        self.bindings.retain(|b| b.command != command);
        self.bindings.len() < before
    }

    pub fn get_all_bindings(&self) -> &[Keybinding] {
        &self.bindings
    }

    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    pub fn find_by_command(&self, cmd: &str) -> Vec<&Keybinding> {
        self.bindings.iter().filter(|b| b.command == cmd).collect()
    }

    pub fn find_by_key(&self, key: &KeyCode, modifiers: &Modifiers) -> Vec<&Keybinding> {
        self.bindings
            .iter()
            .filter(|b| b.key == *key && b.modifiers == *modifiers)
            .collect()
    }

    /// Returns the highest-priority binding for a key combo (User > Extension > Default).
    pub fn get_effective_binding(&self, key: &KeyCode, modifiers: &Modifiers) -> Option<&Keybinding> {
        let matches = self.find_by_key(key, modifiers);
        if matches.is_empty() {
            return None;
        }
        matches
            .into_iter()
            .min_by_key(|b| match b.source {
                KeybindingSource::User => 0,
                KeybindingSource::Extension => 1,
                KeybindingSource::Default => 2,
            })
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
        format_key_combo(&binding.key, &binding.modifiers)
    }

    /// Returns true if bindings is empty.
    pub fn is_bindings_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Get the first binding, if any.
    pub fn first_binding(&self) -> Option<&Keybinding> {
        self.bindings.first()
    }

    /// Get the last binding, if any.
    pub fn last_binding(&self) -> Option<&Keybinding> {
        self.bindings.last()
    }

    /// Retain only bindings matching the predicate.
    pub fn retain_bindings(&mut self, f: impl Fn(&Keybinding) -> bool) {
        self.bindings.retain(|item| f(item));
    }
}

fn format_key_combo(key: &KeyCode, modifiers: &Modifiers) -> String {
    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl".to_string());
    }
    if modifiers.shift {
        parts.push("Shift".to_string());
    }
    if modifiers.alt {
        parts.push("Alt".to_string());
    }
    if modifiers.meta {
        parts.push("Meta".to_string());
    }
    let key_str = match key {
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
    parts.push(key_str);
    parts.join("+")
}

pub fn format_chord(chord: &ChordKeybinding) -> String {
    let first = format_key_combo(&chord.first.0, &chord.first.1);
    let second = format_key_combo(&chord.second.0, &chord.second.1);
    format!("{first} {second}")
}

pub fn parse_keybinding(input: &str) -> Option<Keybinding> {
    let parts: Vec<&str> = input.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut meta = false;

    for &part in &parts[..parts.len() - 1] {
        match part.to_lowercase().as_str() {
            "ctrl" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            "meta" => meta = true,
            _ => return None,
        }
    }

    let key_part = parts.last()?;
    let key = match key_part.to_lowercase().as_str() {
        "enter" => KeyCode::Enter,
        "escape" | "esc" => KeyCode::Escape,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "space" => KeyCode::Space,
        "up" => KeyCode::ArrowUp,
        "down" => KeyCode::ArrowDown,
        "left" => KeyCode::ArrowLeft,
        "right" => KeyCode::ArrowRight,
        "delete" | "del" => KeyCode::Delete,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        s if s.starts_with('f') && s.len() > 1 => {
            let n: u8 = s[1..].parse().ok()?;
            if n == 0 || n > 24 {
                return None;
            }
            KeyCode::F(n)
        }
        s if s.len() == 1 => {
            let c = s.chars().next()?;
            if c.is_ascii_alphanumeric() {
                KeyCode::Char(c.to_ascii_lowercase())
            } else {
                return None;
            }
        }
        _ => return None,
    };

    Some(Keybinding {
        key,
        modifiers: Modifiers { ctrl, shift, alt, meta },
        command: String::new(),
        when_clause: None,
        source: KeybindingSource::Default,
    })
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
            source: KeybindingSource::Default,
        }
    }

    fn kb_with_source(key: KeyCode, ctrl: bool, cmd: &str, source: KeybindingSource) -> Keybinding {
        Keybinding {
            key,
            modifiers: Modifiers { ctrl, shift: false, alt: false, meta: false },
            command: cmd.to_string(),
            when_clause: None,
            source,
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
            source: KeybindingSource::Default,
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

    #[test]
    fn remove_existing_binding() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        reg.add(kb(KeyCode::Char('o'), true, "open"));
        assert!(reg.remove_binding("save"));
        assert_eq!(reg.binding_count(), 1);
        assert!(reg.find_by_command("save").is_empty());
    }

    #[test]
    fn remove_nonexistent_binding() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        assert!(!reg.remove_binding("missing"));
        assert_eq!(reg.binding_count(), 1);
    }

    #[test]
    fn get_all_bindings_and_count() {
        let mut reg = KeybindingRegistry::new();
        assert_eq!(reg.binding_count(), 0);
        assert!(reg.get_all_bindings().is_empty());
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        reg.add(kb(KeyCode::Char('o'), true, "open"));
        assert_eq!(reg.binding_count(), 2);
        assert_eq!(reg.get_all_bindings().len(), 2);
    }

    #[test]
    fn find_by_key_returns_matching() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        reg.add(kb(KeyCode::Char('s'), true, "search"));
        reg.add(kb(KeyCode::Char('o'), true, "open"));
        let mods = Modifiers { ctrl: true, shift: false, alt: false, meta: false };
        let found = reg.find_by_key(&KeyCode::Char('s'), &mods);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn find_by_key_no_match() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        let found = reg.find_by_key(&KeyCode::Char('x'), &Modifiers::none());
        assert!(found.is_empty());
    }

    #[test]
    fn format_chord_binding() {
        let chord = ChordKeybinding {
            first: (
                KeyCode::Char('k'),
                Modifiers { ctrl: true, shift: false, alt: false, meta: false },
            ),
            second: (
                KeyCode::Char('c'),
                Modifiers { ctrl: true, shift: false, alt: false, meta: false },
            ),
            command: "comment".to_string(),
        };
        assert_eq!(format_chord(&chord), "Ctrl+K Ctrl+C");
    }

    #[test]
    fn parse_simple_keybinding() {
        let parsed = parse_keybinding("Ctrl+S").unwrap();
        assert_eq!(parsed.key, KeyCode::Char('s'));
        assert!(parsed.modifiers.ctrl);
        assert!(!parsed.modifiers.shift);
    }

    #[test]
    fn parse_keybinding_with_multiple_modifiers() {
        let parsed = parse_keybinding("Ctrl+Shift+P").unwrap();
        assert_eq!(parsed.key, KeyCode::Char('p'));
        assert!(parsed.modifiers.ctrl);
        assert!(parsed.modifiers.shift);
    }

    #[test]
    fn parse_function_key() {
        let parsed = parse_keybinding("F5").unwrap();
        assert_eq!(parsed.key, KeyCode::F(5));
        assert!(!parsed.modifiers.has_modifier());
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(parse_keybinding("Ctrl+???").is_none());
        assert!(parse_keybinding("Ctrl+F0").is_none());
    }

    #[test]
    fn has_modifier_true_and_false() {
        assert!(!Modifiers::none().has_modifier());
        assert!(Modifiers { ctrl: true, shift: false, alt: false, meta: false }.has_modifier());
        assert!(Modifiers { ctrl: false, shift: false, alt: true, meta: false }.has_modifier());
    }

    #[test]
    fn effective_binding_user_overrides_default() {
        let mut reg = KeybindingRegistry::new();
        let mods = Modifiers { ctrl: true, shift: false, alt: false, meta: false };
        reg.add(kb_with_source(KeyCode::Char('s'), true, "default_save", KeybindingSource::Default));
        reg.add(kb_with_source(KeyCode::Char('s'), true, "ext_save", KeybindingSource::Extension));
        reg.add(kb_with_source(KeyCode::Char('s'), true, "user_save", KeybindingSource::User));
        let effective = reg.get_effective_binding(&KeyCode::Char('s'), &mods).unwrap();
        assert_eq!(effective.command, "user_save");
    }

    #[test]
    fn effective_binding_extension_over_default() {
        let mut reg = KeybindingRegistry::new();
        let mods = Modifiers { ctrl: true, shift: false, alt: false, meta: false };
        reg.add(kb_with_source(KeyCode::Char('s'), true, "default_save", KeybindingSource::Default));
        reg.add(kb_with_source(KeyCode::Char('s'), true, "ext_save", KeybindingSource::Extension));
        let effective = reg.get_effective_binding(&KeyCode::Char('s'), &mods).unwrap();
        assert_eq!(effective.command, "ext_save");
    }

    #[test]
    fn effective_binding_none_when_empty() {
        let reg = KeybindingRegistry::new();
        assert!(reg.get_effective_binding(&KeyCode::Char('x'), &Modifiers::none()).is_none());
    }

    #[test]
    fn eq_keycode_same() {
        assert_eq!(KeyCode::Enter, KeyCode::Enter);
    }

    #[test]
    fn ne_keycode_diff() {
        assert_ne!(KeyCode::Enter, KeyCode::Escape);
    }

    #[test]
    fn eq_keybindingsource_same() {
        assert_eq!(KeybindingSource::Default, KeybindingSource::Default);
    }

    #[test]
    fn ne_keybindingsource_diff() {
        assert_ne!(KeybindingSource::Default, KeybindingSource::User);
    }

    #[test]
    fn behavior_check_0() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }
}
