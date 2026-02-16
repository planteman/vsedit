//! Keybinding chord sequences.
//!
//! Provides [`Keybinding`] (one or two chords), the [`ResolvedKeybinding`]
//! trait for platform-aware label formatting, [`SimpleResolvedKeybinding`]
//! as the default implementation, and parsing/matching utilities.
//!
//! Modeled after VS Code's `vs/base/common/keybindings.ts`.

use std::fmt;
use vsedit_keycodes::{key_code_to_string, string_to_key_code, KeyCode, KeyCodeChord};
use vsedit_platform::Platform;

// ---------------------------------------------------------------------------
// Keybinding
// ---------------------------------------------------------------------------

/// A full keybinding consisting of one or two chords.
///
/// Single-chord example: `Ctrl+S`
/// Two-chord example: `Ctrl+K Ctrl+C`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keybinding {
    pub parts: Vec<KeyCodeChord>,
}

impl Keybinding {
    /// Create a single-chord keybinding.
    pub fn new(chord: KeyCodeChord) -> Self {
        Self {
            parts: vec![chord],
        }
    }

    /// Create a two-chord keybinding (e.g., Ctrl+K Ctrl+C).
    pub fn two_chords(first: KeyCodeChord, second: KeyCodeChord) -> Self {
        Self {
            parts: vec![first, second],
        }
    }
}

impl std::fmt::Display for Keybinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{part}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ResolvedKeybinding trait
// ---------------------------------------------------------------------------

/// A keybinding resolved for a specific platform.
///
/// Provides platform-appropriate labels and dispatch strings.
pub trait ResolvedKeybinding {
    /// Human-readable label (e.g., "⌘K ⌘C" on macOS, "Ctrl+K Ctrl+C" on Windows).
    fn get_label(&self) -> String;

    /// Accessible label for screen readers.
    fn get_aria_label(&self) -> String;

    /// Electron accelerator string, if representable.
    fn get_electron_accelerator(&self) -> Option<String>;

    /// Dispatch strings for each chord (e.g., `["ctrl+k", "ctrl+c"]`).
    fn get_dispatch_chords(&self) -> Vec<String>;
}

// ---------------------------------------------------------------------------
// SimpleResolvedKeybinding
// ---------------------------------------------------------------------------

/// Default [`ResolvedKeybinding`] implementation with platform-aware formatting.
#[derive(Debug, Clone)]
pub struct SimpleResolvedKeybinding {
    binding: Keybinding,
    platform: Platform,
}

impl SimpleResolvedKeybinding {
    /// Create a resolved keybinding for the given platform.
    pub fn new(binding: Keybinding, platform: Platform) -> Self {
        Self { binding, platform }
    }

    /// Format a single chord for the given platform.
    fn format_chord(&self, chord: &KeyCodeChord) -> String {
        let mut parts = Vec::new();
        match self.platform {
            Platform::MacOS => {
                if chord.ctrl {
                    parts.push("⌃".to_string());
                }
                if chord.shift {
                    parts.push("⇧".to_string());
                }
                if chord.alt {
                    parts.push("⌥".to_string());
                }
                if chord.meta {
                    parts.push("⌘".to_string());
                }
                parts.push(key_code_to_string(chord.key_code).to_string());
                parts.join("")
            }
            _ => {
                if chord.ctrl {
                    parts.push("Ctrl".to_string());
                }
                if chord.shift {
                    parts.push("Shift".to_string());
                }
                if chord.alt {
                    parts.push("Alt".to_string());
                }
                if chord.meta {
                    parts.push("Win".to_string());
                }
                parts.push(key_code_to_string(chord.key_code).to_string());
                parts.join("+")
            }
        }
    }

    /// Format a chord for the aria label (always uses words, no symbols).
    fn format_chord_aria(&self, chord: &KeyCodeChord) -> String {
        let mut parts = Vec::new();
        match self.platform {
            Platform::MacOS => {
                if chord.ctrl {
                    parts.push("Control".to_string());
                }
                if chord.shift {
                    parts.push("Shift".to_string());
                }
                if chord.alt {
                    parts.push("Option".to_string());
                }
                if chord.meta {
                    parts.push("Command".to_string());
                }
            }
            _ => {
                if chord.ctrl {
                    parts.push("Control".to_string());
                }
                if chord.shift {
                    parts.push("Shift".to_string());
                }
                if chord.alt {
                    parts.push("Alt".to_string());
                }
                if chord.meta {
                    parts.push("Windows".to_string());
                }
            }
        }
        parts.push(key_code_to_string(chord.key_code).to_string());
        parts.join("+")
    }

    /// Format a chord as an Electron accelerator string.
    fn format_chord_electron(&self, chord: &KeyCodeChord) -> Option<String> {
        let key_str = electron_key_label(chord.key_code)?;
        let mut parts = Vec::new();
        if chord.ctrl {
            parts.push(if self.platform == Platform::MacOS {
                "Ctrl"
            } else {
                "Ctrl"
            });
        }
        if chord.alt {
            parts.push("Alt");
        }
        if chord.shift {
            parts.push("Shift");
        }
        if chord.meta {
            parts.push(if self.platform == Platform::MacOS {
                "Cmd"
            } else {
                "Super"
            });
        }
        parts.push(key_str);
        Some(parts.join("+"))
    }

    /// Format a chord as a lowercase dispatch string (e.g., "ctrl+k").
    fn format_chord_dispatch(chord: &KeyCodeChord) -> String {
        let mut parts = Vec::new();
        if chord.ctrl {
            parts.push("ctrl".to_string());
        }
        if chord.shift {
            parts.push("shift".to_string());
        }
        if chord.alt {
            parts.push("alt".to_string());
        }
        if chord.meta {
            parts.push("meta".to_string());
        }
        parts.push(key_code_to_string(chord.key_code).to_ascii_lowercase());
        parts.join("+")
    }
}

impl ResolvedKeybinding for SimpleResolvedKeybinding {
    fn get_label(&self) -> String {
        self.binding
            .parts
            .iter()
            .map(|c| self.format_chord(c))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn get_aria_label(&self) -> String {
        self.binding
            .parts
            .iter()
            .map(|c| self.format_chord_aria(c))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn get_electron_accelerator(&self) -> Option<String> {
        // Electron only supports single-chord accelerators.
        if self.binding.parts.len() != 1 {
            return None;
        }
        self.format_chord_electron(&self.binding.parts[0])
    }

    fn get_dispatch_chords(&self) -> Vec<String> {
        self.binding
            .parts
            .iter()
            .map(|c| Self::format_chord_dispatch(c))
            .collect()
    }
}

/// Map a `KeyCode` to its Electron accelerator label.
///
/// Returns `None` for keys that have no Electron accelerator equivalent.
fn electron_key_label(key_code: KeyCode) -> Option<&'static str> {
    match key_code {
        KeyCode::Backspace => Some("Backspace"),
        KeyCode::Tab => Some("Tab"),
        KeyCode::Enter => Some("Enter"),
        KeyCode::Escape => Some("Escape"),
        KeyCode::Space => Some("Space"),
        KeyCode::PageUp => Some("PageUp"),
        KeyCode::PageDown => Some("PageDown"),
        KeyCode::End => Some("End"),
        KeyCode::Home => Some("Home"),
        KeyCode::LeftArrow => Some("Left"),
        KeyCode::UpArrow => Some("Up"),
        KeyCode::RightArrow => Some("Right"),
        KeyCode::DownArrow => Some("Down"),
        KeyCode::Insert => Some("Insert"),
        KeyCode::Delete => Some("Delete"),
        KeyCode::Digit0 => Some("0"),
        KeyCode::Digit1 => Some("1"),
        KeyCode::Digit2 => Some("2"),
        KeyCode::Digit3 => Some("3"),
        KeyCode::Digit4 => Some("4"),
        KeyCode::Digit5 => Some("5"),
        KeyCode::Digit6 => Some("6"),
        KeyCode::Digit7 => Some("7"),
        KeyCode::Digit8 => Some("8"),
        KeyCode::Digit9 => Some("9"),
        KeyCode::KeyA => Some("A"),
        KeyCode::KeyB => Some("B"),
        KeyCode::KeyC => Some("C"),
        KeyCode::KeyD => Some("D"),
        KeyCode::KeyE => Some("E"),
        KeyCode::KeyF => Some("F"),
        KeyCode::KeyG => Some("G"),
        KeyCode::KeyH => Some("H"),
        KeyCode::KeyI => Some("I"),
        KeyCode::KeyJ => Some("J"),
        KeyCode::KeyK => Some("K"),
        KeyCode::KeyL => Some("L"),
        KeyCode::KeyM => Some("M"),
        KeyCode::KeyN => Some("N"),
        KeyCode::KeyO => Some("O"),
        KeyCode::KeyP => Some("P"),
        KeyCode::KeyQ => Some("Q"),
        KeyCode::KeyR => Some("R"),
        KeyCode::KeyS => Some("S"),
        KeyCode::KeyT => Some("T"),
        KeyCode::KeyU => Some("U"),
        KeyCode::KeyV => Some("V"),
        KeyCode::KeyW => Some("W"),
        KeyCode::KeyX => Some("X"),
        KeyCode::KeyY => Some("Y"),
        KeyCode::KeyZ => Some("Z"),
        KeyCode::F1 => Some("F1"),
        KeyCode::F2 => Some("F2"),
        KeyCode::F3 => Some("F3"),
        KeyCode::F4 => Some("F4"),
        KeyCode::F5 => Some("F5"),
        KeyCode::F6 => Some("F6"),
        KeyCode::F7 => Some("F7"),
        KeyCode::F8 => Some("F8"),
        KeyCode::F9 => Some("F9"),
        KeyCode::F10 => Some("F10"),
        KeyCode::F11 => Some("F11"),
        KeyCode::F12 => Some("F12"),
        KeyCode::F13 => Some("F13"),
        KeyCode::F14 => Some("F14"),
        KeyCode::F15 => Some("F15"),
        KeyCode::F16 => Some("F16"),
        KeyCode::F17 => Some("F17"),
        KeyCode::F18 => Some("F18"),
        KeyCode::F19 => Some("F19"),
        KeyCode::F20 => Some("F20"),
        KeyCode::F21 => Some("F21"),
        KeyCode::F22 => Some("F22"),
        KeyCode::F23 => Some("F23"),
        KeyCode::F24 => Some("F24"),
        KeyCode::Semicolon => Some(";"),
        KeyCode::Equal => Some("="),
        KeyCode::Comma => Some(","),
        KeyCode::Minus => Some("-"),
        KeyCode::Period => Some("."),
        KeyCode::Slash => Some("/"),
        KeyCode::Backquote => Some("`"),
        KeyCode::BracketLeft => Some("["),
        KeyCode::Backslash => Some("\\"),
        KeyCode::BracketRight => Some("]"),
        KeyCode::Quote => Some("'"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------

/// Check if a pressed chord matches at a given position in a keybinding.
///
/// Returns `true` if the chord at `chord_index` in `binding` matches `chord`.
pub fn keybinding_matches(
    binding: &Keybinding,
    chord: &KeyCodeChord,
    chord_index: usize,
) -> bool {
    if chord_index >= binding.parts.len() {
        return false;
    }
    binding.parts[chord_index] == *chord
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a keybinding string like `"ctrl+shift+k"` or `"ctrl+k ctrl+c"`.
///
/// On macOS, `"cmd"` maps to `meta` and `"ctrl"` maps to `ctrl`.
/// On other platforms, `"cmd"` and `"ctrl"` both map to `ctrl`.
///
/// Returns `None` if the string is empty or contains an unrecognised key.
pub fn parse_keybinding(input: &str, platform: Platform) -> Option<Keybinding> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let chord_strs: Vec<&str> = input.split_whitespace().collect();
    let mut parts = Vec::with_capacity(chord_strs.len());

    for chord_str in chord_strs {
        let chord = parse_chord(chord_str, platform)?;
        parts.push(chord);
    }

    if parts.is_empty() {
        return None;
    }

    Some(Keybinding { parts })
}

/// Parse a single chord string like `"ctrl+shift+k"`.
fn parse_chord(input: &str, platform: Platform) -> Option<KeyCodeChord> {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut meta = false;
    let mut key_code = KeyCode::Unknown;

    for token in input.split('+') {
        let token = token.trim();
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => {
                ctrl = true;
            }
            "shift" => {
                shift = true;
            }
            "alt" | "option" => {
                alt = true;
            }
            "meta" | "win" | "windows" | "super" => {
                meta = true;
            }
            "cmd" | "command" => {
                if platform == Platform::MacOS {
                    meta = true;
                } else {
                    ctrl = true;
                }
            }
            _ => {
                key_code = string_to_key_code(token);
                if key_code == KeyCode::Unknown {
                    return None;
                }
            }
        }
    }

    if key_code == KeyCode::Unknown {
        return None;
    }

    Some(KeyCodeChord::new(ctrl, shift, alt, meta, key_code))
}

// ---------------------------------------------------------------------------
// Conflict detection
// ---------------------------------------------------------------------------

/// A keybinding conflict between two bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingConflict {
    pub binding_a: Keybinding,
    pub binding_b: Keybinding,
}

/// Detect conflicts in a slice of keybindings.
///
/// Two bindings conflict if they have the same chord sequence.
pub fn detect_conflicts(bindings: &[Keybinding]) -> Vec<KeybindingConflict> {
    let mut conflicts = Vec::new();
    for i in 0..bindings.len() {
        for j in (i + 1)..bindings.len() {
            if bindings[i].parts == bindings[j].parts {
                conflicts.push(KeybindingConflict {
                    binding_a: bindings[i].clone(),
                    binding_b: bindings[j].clone(),
                });
            }
        }
    }
    conflicts
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

/// Serialize a keybinding to a lowercase dispatch string.
///
/// Multi-chord bindings use space as separator, e.g. `"ctrl+k ctrl+c"`.
pub fn serialize_keybinding(binding: &Keybinding) -> String {
    binding
        .parts
        .iter()
        .map(|c| SimpleResolvedKeybinding::format_chord_dispatch(c))
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Chord matching score
// ---------------------------------------------------------------------------

/// Compute a match score between a pressed chord and a target chord.
///
/// Returns 0 if they don't match, otherwise a score based on how many
/// modifiers match (each matching modifier adds 1, matching key adds 4).
pub fn chord_match_score(pressed: &KeyCodeChord, target: &KeyCodeChord) -> u32 {
    if pressed.key_code != target.key_code {
        return 0;
    }
    let mut score: u32 = 4;
    if pressed.ctrl == target.ctrl {
        score += 1;
    }
    if pressed.shift == target.shift {
        score += 1;
    }
    if pressed.alt == target.alt {
        score += 1;
    }
    if pressed.meta == target.meta {
        score += 1;
    }
    score
}

// ---------------------------------------------------------------------------
// Category grouping
// ---------------------------------------------------------------------------

/// Category for organizing keybindings in UI.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeybindingCategory {
    General,
    Editor,
    Navigation,
    Debug,
    Custom(String),
}

/// A keybinding tagged with a command and category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategorizedKeybinding {
    pub binding: Keybinding,
    pub command: String,
    pub category: KeybindingCategory,
}

/// Group categorized keybindings by their category.
pub fn group_by_category(
    bindings: &[CategorizedKeybinding],
) -> std::collections::HashMap<KeybindingCategory, Vec<&CategorizedKeybinding>> {
    let mut map = std::collections::HashMap::new();
    for b in bindings {
        map.entry(b.category.clone())
            .or_insert_with(Vec::new)
            .push(b);
    }
    map
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Keybinding construction & display --

    #[test]
    fn single_chord_display() {
        let kb = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        assert_eq!(kb.to_string(), "Ctrl+S");
    }

    #[test]
    fn two_chord_display() {
        let kb = Keybinding::two_chords(
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        );
        assert_eq!(kb.to_string(), "Ctrl+K Ctrl+C");
    }

    #[test]
    fn keybinding_parts_length() {
        let single = Keybinding::new(KeyCodeChord::just(KeyCode::Escape));
        assert_eq!(single.parts.len(), 1);

        let double = Keybinding::two_chords(
            KeyCodeChord::just(KeyCode::KeyA),
            KeyCodeChord::just(KeyCode::KeyB),
        );
        assert_eq!(double.parts.len(), 2);
    }

    // -- Parsing --

    #[test]
    fn parse_simple_chord() {
        let kb = parse_keybinding("ctrl+k", Platform::Linux).unwrap();
        assert_eq!(kb.parts.len(), 1);
        assert_eq!(kb.parts[0], KeyCodeChord::new(true, false, false, false, KeyCode::KeyK));
    }

    #[test]
    fn parse_ctrl_shift() {
        let kb = parse_keybinding("ctrl+shift+s", Platform::Linux).unwrap();
        assert_eq!(kb.parts[0], KeyCodeChord::new(true, true, false, false, KeyCode::KeyS));
    }

    #[test]
    fn parse_two_chords() {
        let kb = parse_keybinding("ctrl+k ctrl+c", Platform::Linux).unwrap();
        assert_eq!(kb.parts.len(), 2);
        assert_eq!(kb.parts[0], KeyCodeChord::new(true, false, false, false, KeyCode::KeyK));
        assert_eq!(kb.parts[1], KeyCodeChord::new(true, false, false, false, KeyCode::KeyC));
    }

    #[test]
    fn parse_cmd_on_macos() {
        let kb = parse_keybinding("cmd+k", Platform::MacOS).unwrap();
        // cmd maps to meta on macOS
        assert_eq!(kb.parts[0], KeyCodeChord::new(false, false, false, true, KeyCode::KeyK));
    }

    #[test]
    fn parse_cmd_on_linux() {
        let kb = parse_keybinding("cmd+k", Platform::Linux).unwrap();
        // cmd maps to ctrl on non-macOS
        assert_eq!(kb.parts[0], KeyCodeChord::new(true, false, false, false, KeyCode::KeyK));
    }

    #[test]
    fn parse_empty_returns_none() {
        assert!(parse_keybinding("", Platform::Linux).is_none());
        assert!(parse_keybinding("   ", Platform::Linux).is_none());
    }

    #[test]
    fn parse_unknown_key_returns_none() {
        assert!(parse_keybinding("ctrl+not_a_key", Platform::Linux).is_none());
    }

    #[test]
    fn parse_f_keys() {
        let kb = parse_keybinding("ctrl+shift+f5", Platform::Linux).unwrap();
        assert_eq!(kb.parts[0], KeyCodeChord::new(true, true, false, false, KeyCode::F5));
    }

    #[test]
    fn parse_alt_modifier() {
        let kb = parse_keybinding("alt+a", Platform::Linux).unwrap();
        assert_eq!(kb.parts[0], KeyCodeChord::new(false, false, true, false, KeyCode::KeyA));
    }

    #[test]
    fn parse_meta_modifier() {
        let kb = parse_keybinding("meta+a", Platform::Linux).unwrap();
        assert_eq!(kb.parts[0], KeyCodeChord::new(false, false, false, true, KeyCode::KeyA));
    }

    // -- Matching --

    #[test]
    fn matching_single_chord() {
        let kb = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        let pressed = KeyCodeChord::new(true, false, false, false, KeyCode::KeyS);
        assert!(keybinding_matches(&kb, &pressed, 0));
    }

    #[test]
    fn matching_wrong_chord() {
        let kb = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        let pressed = KeyCodeChord::new(true, false, false, false, KeyCode::KeyA);
        assert!(!keybinding_matches(&kb, &pressed, 0));
    }

    #[test]
    fn matching_two_chord_sequence() {
        let kb = Keybinding::two_chords(
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        );
        let first = KeyCodeChord::new(true, false, false, false, KeyCode::KeyK);
        let second = KeyCodeChord::new(true, false, false, false, KeyCode::KeyC);
        assert!(keybinding_matches(&kb, &first, 0));
        assert!(keybinding_matches(&kb, &second, 1));
        assert!(!keybinding_matches(&kb, &first, 1));
    }

    #[test]
    fn matching_out_of_bounds() {
        let kb = Keybinding::new(KeyCodeChord::just(KeyCode::KeyA));
        assert!(!keybinding_matches(&kb, &KeyCodeChord::just(KeyCode::KeyA), 1));
    }

    #[test]
    fn matching_modifier_mismatch() {
        let kb = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        let pressed = KeyCodeChord::new(true, true, false, false, KeyCode::KeyS);
        assert!(!keybinding_matches(&kb, &pressed, 0));
    }

    // -- ResolvedKeybinding / SimpleResolvedKeybinding --

    #[test]
    fn resolved_label_linux() {
        let kb = Keybinding::two_chords(
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        );
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Linux);
        assert_eq!(resolved.get_label(), "Ctrl+K Ctrl+C");
    }

    #[test]
    fn resolved_label_windows() {
        let kb = Keybinding::new(KeyCodeChord::new(true, true, false, false, KeyCode::KeyS));
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Windows);
        assert_eq!(resolved.get_label(), "Ctrl+Shift+S");
    }

    #[test]
    fn resolved_label_macos() {
        let kb = Keybinding::two_chords(
            KeyCodeChord::new(false, false, false, true, KeyCode::KeyK),
            KeyCodeChord::new(false, false, false, true, KeyCode::KeyC),
        );
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::MacOS);
        assert_eq!(resolved.get_label(), "⌘K ⌘C");
    }

    #[test]
    fn resolved_label_macos_all_modifiers() {
        let kb = Keybinding::new(KeyCodeChord::new(true, true, true, true, KeyCode::KeyA));
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::MacOS);
        assert_eq!(resolved.get_label(), "⌃⇧⌥⌘A");
    }

    #[test]
    fn resolved_label_windows_meta() {
        let kb = Keybinding::new(KeyCodeChord::new(false, false, false, true, KeyCode::KeyL));
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Windows);
        assert_eq!(resolved.get_label(), "Win+L");
    }

    #[test]
    fn resolved_aria_label_linux() {
        let kb = Keybinding::new(KeyCodeChord::new(true, true, false, false, KeyCode::KeyS));
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Linux);
        assert_eq!(resolved.get_aria_label(), "Control+Shift+S");
    }

    #[test]
    fn resolved_aria_label_macos() {
        let kb = Keybinding::new(KeyCodeChord::new(true, false, true, true, KeyCode::KeyA));
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::MacOS);
        assert_eq!(resolved.get_aria_label(), "Control+Option+Command+A");
    }

    #[test]
    fn resolved_electron_single_chord() {
        let kb = Keybinding::new(KeyCodeChord::new(true, true, false, false, KeyCode::KeyS));
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Linux);
        assert_eq!(resolved.get_electron_accelerator(), Some("Ctrl+Shift+S".to_string()));
    }

    #[test]
    fn resolved_electron_two_chords_returns_none() {
        let kb = Keybinding::two_chords(
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        );
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Linux);
        assert!(resolved.get_electron_accelerator().is_none());
    }

    #[test]
    fn resolved_electron_meta_macos() {
        let kb = Keybinding::new(KeyCodeChord::new(false, false, false, true, KeyCode::KeyS));
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::MacOS);
        assert_eq!(resolved.get_electron_accelerator(), Some("Cmd+S".to_string()));
    }

    #[test]
    fn resolved_electron_meta_linux() {
        let kb = Keybinding::new(KeyCodeChord::new(false, false, false, true, KeyCode::KeyS));
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Linux);
        assert_eq!(resolved.get_electron_accelerator(), Some("Super+S".to_string()));
    }

    #[test]
    fn resolved_dispatch_chords() {
        let kb = Keybinding::two_chords(
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        );
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Linux);
        assert_eq!(resolved.get_dispatch_chords(), vec!["ctrl+k", "ctrl+c"]);
    }

    #[test]
    fn resolved_dispatch_with_shift() {
        let kb = Keybinding::new(KeyCodeChord::new(true, true, false, false, KeyCode::KeyS));
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Linux);
        assert_eq!(resolved.get_dispatch_chords(), vec!["ctrl+shift+s"]);
    }

    #[test]
    fn resolved_dispatch_no_modifiers() {
        let kb = Keybinding::new(KeyCodeChord::just(KeyCode::F5));
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Linux);
        assert_eq!(resolved.get_dispatch_chords(), vec!["f5"]);
    }

    // -- Round-trip: parse → display --

    #[test]
    fn parse_display_roundtrip() {
        let kb = parse_keybinding("ctrl+shift+s", Platform::Linux).unwrap();
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Linux);
        assert_eq!(resolved.get_label(), "Ctrl+Shift+S");
    }

    #[test]
    fn parse_two_chord_roundtrip() {
        let kb = parse_keybinding("ctrl+k ctrl+c", Platform::Linux).unwrap();
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Linux);
        assert_eq!(resolved.get_label(), "Ctrl+K Ctrl+C");
    }

    #[test]
    fn electron_unsupported_key_returns_none() {
        let kb = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::Unknown));
        let resolved = SimpleResolvedKeybinding::new(kb, Platform::Linux);
        assert!(resolved.get_electron_accelerator().is_none());
    }

    #[test]
    fn detect_no_conflicts() {
        let bindings = vec![
            Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
            Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyK)),
        ];
        assert!(detect_conflicts(&bindings).is_empty());
    }

    #[test]
    fn detect_duplicate_conflicts() {
        let bindings = vec![
            Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
            Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
        ];
        let conflicts = detect_conflicts(&bindings);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].binding_a, conflicts[0].binding_b);
    }

    #[test]
    fn serialize_single_chord() {
        let kb = Keybinding::new(KeyCodeChord::new(true, true, false, false, KeyCode::KeyS));
        assert_eq!(serialize_keybinding(&kb), "ctrl+shift+s");
    }

    #[test]
    fn serialize_two_chord() {
        let kb = Keybinding::two_chords(
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        );
        assert_eq!(serialize_keybinding(&kb), "ctrl+k ctrl+c");
    }

    #[test]
    fn chord_match_score_exact() {
        let chord = KeyCodeChord::new(true, false, false, false, KeyCode::KeyS);
        assert_eq!(chord_match_score(&chord, &chord), 8);
    }

    #[test]
    fn chord_match_score_wrong_key() {
        let a = KeyCodeChord::new(true, false, false, false, KeyCode::KeyS);
        let b = KeyCodeChord::new(true, false, false, false, KeyCode::KeyA);
        assert_eq!(chord_match_score(&a, &b), 0);
    }

    #[test]
    fn chord_match_score_partial_modifier() {
        let pressed = KeyCodeChord::new(true, true, false, false, KeyCode::KeyS);
        let target = KeyCodeChord::new(true, false, false, false, KeyCode::KeyS);
        // key matches (4), ctrl matches (1), alt matches (1), meta matches (1) = 7
        // shift does NOT match so no +1 for shift
        assert_eq!(chord_match_score(&pressed, &target), 7);
    }

    #[test]
    fn group_by_category_works() {
        let bindings = vec![
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
                command: "save".into(),
                category: KeybindingCategory::General,
            },
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyG)),
                command: "goto".into(),
                category: KeybindingCategory::Navigation,
            },
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyN)),
                command: "new".into(),
                category: KeybindingCategory::General,
            },
        ];
        let groups = group_by_category(&bindings);
        assert_eq!(groups[&KeybindingCategory::General].len(), 2);
        assert_eq!(groups[&KeybindingCategory::Navigation].len(), 1);
    }
}
