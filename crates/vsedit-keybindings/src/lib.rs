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

    /// Returns `true` if this keybinding has a second chord.
    pub fn is_chord(&self) -> bool {
        self.parts.len() > 1
    }

    /// Returns the number of chords (1 or 2).
    pub fn chord_count(&self) -> usize {
        self.parts.len()
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

impl KeybindingCategory {
    /// Returns a human-readable label for this category.
    pub fn label(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Editor => "Editor",
            Self::Navigation => "Navigation",
            Self::Debug => "Debug",
            Self::Custom(_) => "Custom",
        }
    }

    /// Returns all built-in category variants (excludes `Custom`).
    pub fn all() -> &'static [KeybindingCategory] {
        static VARIANTS: [KeybindingCategory; 4] = [
            KeybindingCategory::General,
            KeybindingCategory::Editor,
            KeybindingCategory::Navigation,
            KeybindingCategory::Debug,
        ];
        &VARIANTS
    }
}

impl fmt::Display for KeybindingCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom(name) => write!(f, "Custom({name})"),
            _ => f.write_str(self.label()),
        }
    }
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
// Keybinding overrides (user customization)
// ---------------------------------------------------------------------------

/// Represents a user keybinding override from keybindings.json.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingOverride {
    /// The command this keybinding is bound to.
    pub command: String,
    /// The keybinding chord sequence.
    pub binding: Keybinding,
    /// Optional context condition (e.g., "editorTextFocus").
    pub when: Option<String>,
    /// If true, this override removes the keybinding instead of adding it.
    pub is_removal: bool,
}

impl KeybindingOverride {
    /// Create a new override that adds a keybinding.
    pub fn add(command: impl Into<String>, binding: Keybinding) -> Self {
        Self {
            command: command.into(),
            binding,
            when: None,
            is_removal: false,
        }
    }

    /// Create a new override that removes a keybinding.
    pub fn remove(command: impl Into<String>, binding: Keybinding) -> Self {
        Self {
            command: command.into(),
            binding,
            when: None,
            is_removal: true,
        }
    }

    /// Set the `when` context condition.
    pub fn with_when(mut self, when: impl Into<String>) -> Self {
        self.when = Some(when.into());
        self
    }

    /// Returns `true` if this override adds a keybinding.
    pub fn is_add(&self) -> bool {
        !self.is_removal
    }

    /// Returns `true` if this override removes a keybinding.
    pub fn is_remove(&self) -> bool {
        self.is_removal
    }
}

impl std::fmt::Display for KeybindingOverride {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{} -> {}{}",
            if self.is_removal { "-" } else { "" },
            self.binding,
            self.command,
            self.when
                .as_ref()
                .map_or(String::new(), |w| format!(" (when: {w})"))
        )
    }
}

/// Merge user overrides on top of a set of default categorized keybindings.
///
/// - Override entries with `is_removal == true` remove the matching command+binding
///   from the defaults.
/// - Override entries with `is_removal == false` are appended to the result.
///
/// Returns a new list of `CategorizedKeybinding`.
pub fn merge_overrides(
    defaults: &[CategorizedKeybinding],
    overrides: &[KeybindingOverride],
) -> Vec<CategorizedKeybinding> {
    let mut result: Vec<CategorizedKeybinding> = defaults.to_vec();

    for ovr in overrides {
        if ovr.is_removal {
            result.retain(|kb| {
                !(kb.command == ovr.command && kb.binding.parts == ovr.binding.parts)
            });
        } else {
            result.push(CategorizedKeybinding {
                binding: ovr.binding.clone(),
                command: ovr.command.clone(),
                category: KeybindingCategory::Custom(String::from("user")),
            });
        }
    }

    result
}

/// Count the number of overrides that are additions vs. removals.
pub fn count_override_types(overrides: &[KeybindingOverride]) -> (usize, usize) {
    let additions = overrides.iter().filter(|o| !o.is_removal).count();
    let removals = overrides.iter().filter(|o| o.is_removal).count();
    (additions, removals)
}

/// Validate overrides: checks for duplicate additions of the same binding.
pub fn validate_overrides(overrides: &[KeybindingOverride]) -> Vec<KeybindingConflict> {
    let additions: Vec<&KeybindingOverride> = overrides.iter().filter(|o| !o.is_removal).collect();
    let mut conflicts = Vec::new();
    for i in 0..additions.len() {
        for j in (i + 1)..additions.len() {
            if additions[i].binding.parts == additions[j].binding.parts {
                conflicts.push(KeybindingConflict {
                    binding_a: additions[i].binding.clone(),
                    binding_b: additions[j].binding.clone(),
                });
            }
        }
    }
    conflicts
}

// ---------------------------------------------------------------------------
// Command lookup
// ---------------------------------------------------------------------------

/// Find all keybindings bound to a given command.
pub fn find_binding_for_command<'a>(
    bindings: &'a [CategorizedKeybinding],
    command: &str,
) -> Vec<&'a CategorizedKeybinding> {
    bindings
        .iter()
        .filter(|kb| kb.command == command)
        .collect()
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

    // -- KeybindingOverride tests --

    #[test]
    fn override_add_display() {
        let kb = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        let ovr = KeybindingOverride::add("workbench.action.files.save", kb);
        let s = format!("{ovr}");
        assert!(s.contains("Ctrl+S"));
        assert!(s.contains("workbench.action.files.save"));
        assert!(!s.starts_with('-'));
    }

    #[test]
    fn override_remove_display() {
        let kb = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        let ovr = KeybindingOverride::remove("workbench.action.files.save", kb);
        let s = format!("{ovr}");
        assert!(s.starts_with('-'));
    }

    #[test]
    fn override_with_when() {
        let kb = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        let ovr = KeybindingOverride::add("save", kb).with_when("editorTextFocus");
        assert_eq!(ovr.when.as_deref(), Some("editorTextFocus"));
        let s = format!("{ovr}");
        assert!(s.contains("editorTextFocus"));
    }

    #[test]
    fn merge_overrides_adds_new_binding() {
        let defaults = vec![CategorizedKeybinding {
            binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
            command: "save".into(),
            category: KeybindingCategory::General,
        }];
        let overrides = vec![KeybindingOverride::add(
            "format",
            Keybinding::new(KeyCodeChord::new(true, true, false, false, KeyCode::KeyF)),
        )];
        let merged = merge_overrides(&defaults, &overrides);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].command, "format");
        assert_eq!(merged[1].category, KeybindingCategory::Custom("user".into()));
    }

    #[test]
    fn merge_overrides_removes_binding() {
        let defaults = vec![
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(
                    true,
                    false,
                    false,
                    false,
                    KeyCode::KeyS,
                )),
                command: "save".into(),
                category: KeybindingCategory::General,
            },
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(
                    true,
                    false,
                    false,
                    false,
                    KeyCode::KeyN,
                )),
                command: "new".into(),
                category: KeybindingCategory::General,
            },
        ];
        let overrides = vec![KeybindingOverride::remove(
            "save",
            Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
        )];
        let merged = merge_overrides(&defaults, &overrides);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command, "new");
    }

    #[test]
    fn merge_overrides_remove_and_readd() {
        let defaults = vec![CategorizedKeybinding {
            binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
            command: "save".into(),
            category: KeybindingCategory::General,
        }];
        let overrides = vec![
            KeybindingOverride::remove(
                "save",
                Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
            ),
            KeybindingOverride::add(
                "save",
                Keybinding::new(KeyCodeChord::new(true, true, false, false, KeyCode::KeyS)),
            ),
        ];
        let merged = merge_overrides(&defaults, &overrides);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command, "save");
        // Should now be Ctrl+Shift+S
        assert!(merged[0].binding.parts[0].shift);
    }

    #[test]
    fn count_override_types_works() {
        let overrides = vec![
            KeybindingOverride::add("a", Keybinding::new(KeyCodeChord::just(KeyCode::KeyA))),
            KeybindingOverride::remove("b", Keybinding::new(KeyCodeChord::just(KeyCode::KeyB))),
            KeybindingOverride::add("c", Keybinding::new(KeyCodeChord::just(KeyCode::KeyC))),
        ];
        let (adds, removes) = count_override_types(&overrides);
        assert_eq!(adds, 2);
        assert_eq!(removes, 1);
    }

    #[test]
    fn validate_overrides_no_conflicts() {
        let overrides = vec![
            KeybindingOverride::add("a", Keybinding::new(KeyCodeChord::just(KeyCode::KeyA))),
            KeybindingOverride::add("b", Keybinding::new(KeyCodeChord::just(KeyCode::KeyB))),
        ];
        assert!(validate_overrides(&overrides).is_empty());
    }

    #[test]
    fn validate_overrides_detects_conflict() {
        let kb = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        let overrides = vec![
            KeybindingOverride::add("save", kb.clone()),
            KeybindingOverride::add("other", kb),
        ];
        let conflicts = validate_overrides(&overrides);
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn merge_overrides_empty_overrides() {
        let defaults = vec![CategorizedKeybinding {
            binding: Keybinding::new(KeyCodeChord::just(KeyCode::KeyA)),
            command: "a".into(),
            category: KeybindingCategory::General,
        }];
        let merged = merge_overrides(&defaults, &[]);
        assert_eq!(merged.len(), 1);
    }

    // -- New functionality tests --

    #[test]
    fn keybinding_is_chord() {
        let single = Keybinding::new(KeyCodeChord::just(KeyCode::KeyA));
        assert!(!single.is_chord());

        let double = Keybinding::two_chords(
            KeyCodeChord::just(KeyCode::KeyA),
            KeyCodeChord::just(KeyCode::KeyB),
        );
        assert!(double.is_chord());
    }

    #[test]
    fn keybinding_chord_count() {
        let single = Keybinding::new(KeyCodeChord::just(KeyCode::KeyA));
        assert_eq!(single.chord_count(), 1);

        let double = Keybinding::two_chords(
            KeyCodeChord::just(KeyCode::KeyK),
            KeyCodeChord::just(KeyCode::KeyC),
        );
        assert_eq!(double.chord_count(), 2);
    }

    #[test]
    fn category_label() {
        assert_eq!(KeybindingCategory::General.label(), "General");
        assert_eq!(KeybindingCategory::Editor.label(), "Editor");
        assert_eq!(KeybindingCategory::Navigation.label(), "Navigation");
        assert_eq!(KeybindingCategory::Debug.label(), "Debug");
        assert_eq!(KeybindingCategory::Custom("foo".into()).label(), "Custom");
    }

    #[test]
    fn category_all_variants() {
        let all = KeybindingCategory::all();
        assert_eq!(all.len(), 4);
        assert_eq!(all[0], KeybindingCategory::General);
        assert_eq!(all[1], KeybindingCategory::Editor);
        assert_eq!(all[2], KeybindingCategory::Navigation);
        assert_eq!(all[3], KeybindingCategory::Debug);
    }

    #[test]
    fn override_is_add_and_is_remove() {
        let kb = Keybinding::new(KeyCodeChord::just(KeyCode::KeyS));
        let add = KeybindingOverride::add("save", kb.clone());
        assert!(add.is_add());
        assert!(!add.is_remove());

        let rm = KeybindingOverride::remove("save", kb);
        assert!(!rm.is_add());
        assert!(rm.is_remove());
    }

    #[test]
    fn find_binding_for_command_works() {
        let bindings = vec![
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
                command: "save".into(),
                category: KeybindingCategory::General,
            },
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, true, false, false, KeyCode::KeyS)),
                command: "save".into(),
                category: KeybindingCategory::General,
            },
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyN)),
                command: "new".into(),
                category: KeybindingCategory::General,
            },
        ];
        let found = find_binding_for_command(&bindings, "save");
        assert_eq!(found.len(), 2);

        let none = find_binding_for_command(&bindings, "nonexistent");
        assert!(none.is_empty());
    }

    #[test]
    fn category_display() {
        assert_eq!(format!("{}", KeybindingCategory::General), "General");
        assert_eq!(format!("{}", KeybindingCategory::Editor), "Editor");
        assert_eq!(format!("{}", KeybindingCategory::Navigation), "Navigation");
        assert_eq!(format!("{}", KeybindingCategory::Debug), "Debug");
        assert_eq!(
            format!("{}", KeybindingCategory::Custom("user".into())),
            "Custom(user)"
        );
    }
}
