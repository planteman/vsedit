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
// Conflict severity
// ---------------------------------------------------------------------------

/// Severity level of a keybinding conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConflictSeverity {
    /// Both bindings share the exact same chord sequence.
    Exact,
    /// One binding is a prefix of another (chord shadowing).
    Prefix,
    /// Bindings share the first chord only.
    Partial,
}

impl fmt::Display for ConflictSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConflictSeverity::Exact => write!(f, "Exact"),
            ConflictSeverity::Prefix => write!(f, "Prefix"),
            ConflictSeverity::Partial => write!(f, "Partial"),
        }
    }
}

/// A keybinding conflict with severity information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeverityConflict {
    pub binding_a: Keybinding,
    pub binding_b: Keybinding,
    pub severity: ConflictSeverity,
}

/// Detect conflicts among categorized keybindings, annotated with severity.
pub fn detect_conflicts_with_severity(bindings: &[CategorizedKeybinding]) -> Vec<SeverityConflict> {
    let mut conflicts = Vec::new();
    for i in 0..bindings.len() {
        for j in (i + 1)..bindings.len() {
            let a = &bindings[i].binding;
            let b = &bindings[j].binding;
            if a.parts == b.parts {
                conflicts.push(SeverityConflict {
                    binding_a: a.clone(),
                    binding_b: b.clone(),
                    severity: ConflictSeverity::Exact,
                });
            } else if a.parts.len() < b.parts.len() && b.parts.starts_with(&a.parts) {
                conflicts.push(SeverityConflict {
                    binding_a: a.clone(),
                    binding_b: b.clone(),
                    severity: ConflictSeverity::Prefix,
                });
            } else if b.parts.len() < a.parts.len() && a.parts.starts_with(&b.parts) {
                conflicts.push(SeverityConflict {
                    binding_a: a.clone(),
                    binding_b: b.clone(),
                    severity: ConflictSeverity::Prefix,
                });
            } else if !a.parts.is_empty()
                && !b.parts.is_empty()
                && a.parts[0] == b.parts[0]
            {
                conflicts.push(SeverityConflict {
                    binding_a: a.clone(),
                    binding_b: b.clone(),
                    severity: ConflictSeverity::Partial,
                });
            }
        }
    }
    conflicts
}

// ---------------------------------------------------------------------------
// Cheat sheet generation
// ---------------------------------------------------------------------------

/// A single entry in a keybinding cheat sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheatSheetEntry {
    pub command: String,
    pub binding_label: String,
    pub category: String,
}

/// A generated cheat sheet organized by category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingCheatSheet {
    pub entries: Vec<CheatSheetEntry>,
}

impl KeybindingCheatSheet {
    /// Generate a cheat sheet from a slice of categorized keybindings.
    pub fn generate(bindings: &[CategorizedKeybinding]) -> Self {
        let mut entries: Vec<CheatSheetEntry> = bindings
            .iter()
            .map(|kb| CheatSheetEntry {
                command: kb.command.clone(),
                binding_label: kb.binding.to_string(),
                category: format!("{}", kb.category),
            })
            .collect();
        entries.sort_by(|a, b| a.category.cmp(&b.category).then(a.command.cmp(&b.command)));
        Self { entries }
    }

    /// Render the cheat sheet as a human-readable multi-line string.
    pub fn render(&self) -> String {
        let mut result = String::new();
        let mut current_cat = String::new();
        for entry in &self.entries {
            if entry.category != current_cat {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(&format!("[{}]\n", entry.category));
                current_cat = entry.category.clone();
            }
            result.push_str(&format!("  {:30} {}\n", entry.command, entry.binding_label));
        }
        result
    }

    /// Return only entries matching a given category string.
    pub fn filter_category(&self, category: &str) -> Vec<&CheatSheetEntry> {
        self.entries.iter().filter(|e| e.category == category).collect()
    }

    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if the cheat sheet has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Key sequence recorder
// ---------------------------------------------------------------------------

/// Records a sequence of key chords for macro creation.
#[derive(Debug, Clone)]
pub struct KeySequenceRecorder {
    chords: Vec<KeyCodeChord>,
    max_length: usize,
    recording: bool,
}

impl KeySequenceRecorder {
    /// Create a new recorder with the given maximum chord count.
    pub fn new(max_length: usize) -> Self {
        Self {
            chords: Vec::new(),
            max_length,
            recording: false,
        }
    }

    /// Start recording.
    pub fn start(&mut self) {
        self.chords.clear();
        self.recording = true;
    }

    /// Stop recording and return the captured chords.
    pub fn stop(&mut self) -> Vec<KeyCodeChord> {
        self.recording = false;
        self.chords.clone()
    }

    /// Returns `true` if currently recording.
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Record a chord. Returns `false` if the recorder is full or not recording.
    pub fn record(&mut self, chord: KeyCodeChord) -> bool {
        if !self.recording || self.chords.len() >= self.max_length {
            return false;
        }
        self.chords.push(chord);
        true
    }

    /// Return the number of recorded chords.
    pub fn len(&self) -> usize {
        self.chords.len()
    }

    /// Return `true` if no chords have been recorded.
    pub fn is_empty(&self) -> bool {
        self.chords.is_empty()
    }

    /// Build a `Keybinding` from the first one or two recorded chords.
    pub fn to_keybinding(&self) -> Option<Keybinding> {
        match self.chords.len() {
            0 => None,
            1 => Some(Keybinding::new(self.chords[0].clone())),
            _ => Some(Keybinding::two_chords(
                self.chords[0].clone(),
                self.chords[1].clone(),
            )),
        }
    }

    /// Reset the recorder, clearing all recorded chords.
    pub fn reset(&mut self) {
        self.chords.clear();
        self.recording = false;
    }
}

// ---------------------------------------------------------------------------
// KeybindingSearch – search and filter keybindings
// ---------------------------------------------------------------------------

/// Criteria for searching/filtering keybindings.
#[derive(Debug, Clone, Default)]
pub struct KeybindingSearch {
    /// Substring to match against the command name (case-insensitive).
    pub command_contains: Option<String>,
    /// If set, only match keybindings for this platform.
    pub platform: Option<Platform>,
    /// If set, only match keybindings with this many chords.
    pub chord_count: Option<usize>,
    /// If set, only match keybindings that use this modifier.
    pub requires_ctrl: Option<bool>,
}

impl KeybindingSearch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn command_contains(mut self, sub: impl Into<String>) -> Self {
        self.command_contains = Some(sub.into());
        self
    }

    pub fn platform(mut self, p: Platform) -> Self {
        self.platform = Some(p);
        self
    }

    pub fn chord_count(mut self, n: usize) -> Self {
        self.chord_count = Some(n);
        self
    }

    pub fn requires_ctrl(mut self, val: bool) -> Self {
        self.requires_ctrl = Some(val);
        self
    }

    /// Test whether a command string and keybinding match all active criteria.
    pub fn matches(&self, command: &str, binding: &Keybinding) -> bool {
        if let Some(ref sub) = self.command_contains {
            if !command.to_lowercase().contains(&sub.to_lowercase()) {
                return false;
            }
        }
        if let Some(n) = self.chord_count {
            if binding.chord_count() != n {
                return false;
            }
        }
        if let Some(needs_ctrl) = self.requires_ctrl {
            let has_ctrl = binding.parts.iter().any(|c| c.ctrl);
            if has_ctrl != needs_ctrl {
                return false;
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// KeybindingStats – statistics about a set of keybindings
// ---------------------------------------------------------------------------

/// Statistics computed from a collection of keybindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingStats {
    pub total: usize,
    pub single_chord: usize,
    pub multi_chord: usize,
    pub with_ctrl: usize,
    pub with_shift: usize,
    pub with_alt: usize,
    pub with_meta: usize,
}

impl KeybindingStats {
    /// Compute statistics from a slice of keybindings.
    pub fn from_bindings(bindings: &[Keybinding]) -> Self {
        let total = bindings.len();
        let single_chord = bindings.iter().filter(|b| b.chord_count() == 1).count();
        let multi_chord = total - single_chord;
        let with_ctrl = bindings.iter().filter(|b| b.parts.iter().any(|c| c.ctrl)).count();
        let with_shift = bindings.iter().filter(|b| b.parts.iter().any(|c| c.shift)).count();
        let with_alt = bindings.iter().filter(|b| b.parts.iter().any(|c| c.alt)).count();
        let with_meta = bindings.iter().filter(|b| b.parts.iter().any(|c| c.meta)).count();
        Self { total, single_chord, multi_chord, with_ctrl, with_shift, with_alt, with_meta }
    }
}

impl fmt::Display for KeybindingStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Total: {}, Single: {}, Multi: {}, Ctrl: {}, Shift: {}, Alt: {}, Meta: {}",
            self.total, self.single_chord, self.multi_chord,
            self.with_ctrl, self.with_shift, self.with_alt, self.with_meta,
        )
    }
}

/// Check whether a keybinding is a prefix of another (i.e. the first chord matches).
pub fn is_prefix_of(prefix: &Keybinding, full: &Keybinding) -> bool {
    if prefix.parts.len() >= full.parts.len() {
        return false;
    }
    prefix.parts.iter().zip(full.parts.iter()).all(|(a, b)| a == b)
}

// ---------------------------------------------------------------------------
// KeybindingConflictDetector – advanced conflict detection
// ---------------------------------------------------------------------------

/// Advanced conflict detector that operates on a stored set of keybindings.
///
/// Provides both exact-match and prefix-based conflict detection in a single
/// pass-friendly structure.
#[derive(Debug, Clone)]
pub struct KeybindingConflictDetector {
    bindings: Vec<Keybinding>,
}

impl KeybindingConflictDetector {
    /// Create a new detector from a list of keybindings.
    pub fn new(bindings: Vec<Keybinding>) -> Self {
        Self { bindings }
    }

    /// Find bindings with identical chord sequences.
    pub fn find_exact_conflicts(&self) -> Vec<KeybindingConflict> {
        let mut conflicts = Vec::new();
        for i in 0..self.bindings.len() {
            for j in (i + 1)..self.bindings.len() {
                if self.bindings[i].parts == self.bindings[j].parts {
                    conflicts.push(KeybindingConflict {
                        binding_a: self.bindings[i].clone(),
                        binding_b: self.bindings[j].clone(),
                    });
                }
            }
        }
        conflicts
    }

    /// Find bindings where one is a strict prefix of another.
    pub fn find_prefix_conflicts(&self) -> Vec<KeybindingConflict> {
        let mut conflicts = Vec::new();
        for i in 0..self.bindings.len() {
            for j in (i + 1)..self.bindings.len() {
                if is_prefix_of(&self.bindings[i], &self.bindings[j])
                    || is_prefix_of(&self.bindings[j], &self.bindings[i])
                {
                    conflicts.push(KeybindingConflict {
                        binding_a: self.bindings[i].clone(),
                        binding_b: self.bindings[j].clone(),
                    });
                }
            }
        }
        conflicts
    }

    /// Total number of conflicts (exact + prefix).
    pub fn conflict_count(&self) -> usize {
        self.find_exact_conflicts().len() + self.find_prefix_conflicts().len()
    }
}

// ---------------------------------------------------------------------------
// KeybindingFuzzySearch – fuzzy matching for keybinding search
// ---------------------------------------------------------------------------

/// Indicates whether a fuzzy-search match was on the command name or key label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzyMatchField {
    /// The query matched inside the command name.
    Command,
    /// The query matched inside the serialized key string.
    Key,
}

/// A single result from a fuzzy keybinding search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingSearchResult {
    /// Index of the matching binding in the original slice.
    pub binding_index: usize,
    /// Relevance score (higher is better).
    pub score: u32,
    /// Which field the query matched against.
    pub matched_on: FuzzyMatchField,
}

/// Fuzzy search across keybindings by command name or serialized key.
pub struct KeybindingFuzzySearch;

impl KeybindingFuzzySearch {
    /// Search categorized keybindings for `query`.
    ///
    /// Returns results sorted by descending score. A case-insensitive
    /// substring match is performed against the command name and the
    /// serialized chord string. An exact-start match scores higher than
    /// a mid-string match.
    pub fn search(
        bindings: &[CategorizedKeybinding],
        query: &str,
    ) -> Vec<KeybindingSearchResult> {
        if query.is_empty() {
            return Vec::new();
        }
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for (i, kb) in bindings.iter().enumerate() {
            let cmd_lower = kb.command.to_lowercase();
            let key_str = serialize_keybinding(&kb.binding).to_lowercase();

            let cmd_score = Self::substring_score(&cmd_lower, &query_lower);
            let key_score = Self::substring_score(&key_str, &query_lower);

            if cmd_score > 0 || key_score > 0 {
                let (score, matched_on) = if cmd_score >= key_score {
                    (cmd_score, FuzzyMatchField::Command)
                } else {
                    (key_score, FuzzyMatchField::Key)
                };
                results.push(KeybindingSearchResult {
                    binding_index: i,
                    score,
                    matched_on,
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Score a `haystack` against a `needle`.
    ///
    /// Returns 0 for no match, 2 for a prefix match, and 1 for a mid-string match.
    fn substring_score(haystack: &str, needle: &str) -> u32 {
        if haystack.starts_with(needle) {
            2
        } else if haystack.contains(needle) {
            1
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// KeybindingRecordingSession – interactive chord recording
// ---------------------------------------------------------------------------

/// Records individual key chords and converts them into a [`Keybinding`].
///
/// At most two chords are kept; additional chords are silently dropped.
#[derive(Debug, Clone)]
pub struct KeybindingRecordingSession {
    chords: Vec<KeyCodeChord>,
}

impl KeybindingRecordingSession {
    /// Create a new, empty recording session.
    pub fn new() -> Self {
        Self {
            chords: Vec::new(),
        }
    }

    /// Record a chord. Only the first two chords are stored.
    pub fn record_chord(&mut self, chord: KeyCodeChord) {
        if self.chords.len() < 2 {
            self.chords.push(chord);
        }
    }

    /// Convert the recorded chords into a [`Keybinding`].
    ///
    /// Returns `None` if no chords have been recorded.
    pub fn to_keybinding(&self) -> Option<Keybinding> {
        match self.chords.len() {
            0 => None,
            1 => Some(Keybinding::new(self.chords[0].clone())),
            _ => Some(Keybinding::two_chords(
                self.chords[0].clone(),
                self.chords[1].clone(),
            )),
        }
    }

    /// Number of chords recorded so far.
    pub fn recorded_count(&self) -> usize {
        self.chords.len()
    }

    /// Discard all recorded chords.
    pub fn clear(&mut self) {
        self.chords.clear();
    }
}

impl Default for KeybindingRecordingSession {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PlatformKeybindingVariant – per-platform keybinding alternatives
// ---------------------------------------------------------------------------

/// Stores platform-specific keybinding variants for a single command.
///
/// For example, `Ctrl+C` on Linux/Windows but `⌘C` on macOS.
#[derive(Debug, Clone)]
pub struct PlatformKeybindingVariant {
    command: String,
    variants: std::collections::HashMap<Platform, Keybinding>,
}

impl PlatformKeybindingVariant {
    /// Create a new variant set for `command` with no platform bindings.
    pub fn new(command: &str) -> Self {
        Self {
            command: command.to_string(),
            variants: std::collections::HashMap::new(),
        }
    }

    /// Set (or replace) the keybinding for a given platform.
    pub fn set_variant(&mut self, platform: Platform, binding: Keybinding) {
        self.variants.insert(platform, binding);
    }

    /// Get the keybinding for a platform, if one has been set.
    pub fn get_variant(&self, platform: Platform) -> Option<&Keybinding> {
        self.variants.get(&platform)
    }

    /// Return all platforms that have a binding set, in arbitrary order.
    pub fn platforms_with_bindings(&self) -> Vec<Platform> {
        self.variants.keys().copied().collect()
    }

    /// The command name this variant set is associated with.
    pub fn command(&self) -> &str {
        &self.command
    }
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

    // -- Conflict severity --

    #[test]
    fn detect_exact_conflict_severity() {
        let chord = KeyCodeChord::new(true, false, false, false, KeyCode::KeyS);
        let bindings = vec![
            CategorizedKeybinding {
                binding: Keybinding::new(chord.clone()),
                command: "save".into(),
                category: KeybindingCategory::General,
            },
            CategorizedKeybinding {
                binding: Keybinding::new(chord.clone()),
                command: "save_all".into(),
                category: KeybindingCategory::General,
            },
        ];
        let conflicts = detect_conflicts_with_severity(&bindings);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].severity, ConflictSeverity::Exact);
    }

    #[test]
    fn detect_prefix_conflict_severity() {
        let k = KeyCodeChord::new(true, false, false, false, KeyCode::KeyK);
        let c = KeyCodeChord::new(true, false, false, false, KeyCode::KeyC);
        let bindings = vec![
            CategorizedKeybinding {
                binding: Keybinding::new(k.clone()),
                command: "cut".into(),
                category: KeybindingCategory::Editor,
            },
            CategorizedKeybinding {
                binding: Keybinding::two_chords(k.clone(), c.clone()),
                command: "comment".into(),
                category: KeybindingCategory::Editor,
            },
        ];
        let conflicts = detect_conflicts_with_severity(&bindings);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].severity, ConflictSeverity::Prefix);
    }

    #[test]
    fn conflict_severity_display() {
        assert_eq!(format!("{}", ConflictSeverity::Exact), "Exact");
        assert_eq!(format!("{}", ConflictSeverity::Prefix), "Prefix");
        assert_eq!(format!("{}", ConflictSeverity::Partial), "Partial");
    }

    // -- Cheat sheet --

    #[test]
    fn cheat_sheet_generate_and_render() {
        let bindings = vec![
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
                command: "save".into(),
                category: KeybindingCategory::General,
            },
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyZ)),
                command: "undo".into(),
                category: KeybindingCategory::Editor,
            },
        ];
        let sheet = KeybindingCheatSheet::generate(&bindings);
        assert_eq!(sheet.len(), 2);
        assert!(!sheet.is_empty());
        let rendered = sheet.render();
        assert!(rendered.contains("[Editor]"));
        assert!(rendered.contains("[General]"));
        assert!(rendered.contains("save"));
        assert!(rendered.contains("undo"));
    }

    #[test]
    fn cheat_sheet_filter_category() {
        let bindings = vec![
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
                command: "save".into(),
                category: KeybindingCategory::General,
            },
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyZ)),
                command: "undo".into(),
                category: KeybindingCategory::Editor,
            },
        ];
        let sheet = KeybindingCheatSheet::generate(&bindings);
        let general = sheet.filter_category("General");
        assert_eq!(general.len(), 1);
        assert_eq!(general[0].command, "save");
    }

    // -- Key sequence recorder --

    #[test]
    fn recorder_basic_flow() {
        let mut rec = KeySequenceRecorder::new(4);
        assert!(!rec.is_recording());
        assert!(rec.is_empty());

        rec.start();
        assert!(rec.is_recording());

        let chord = KeyCodeChord::new(true, false, false, false, KeyCode::KeyA);
        assert!(rec.record(chord.clone()));
        assert_eq!(rec.len(), 1);

        let chords = rec.stop();
        assert_eq!(chords.len(), 1);
        assert!(!rec.is_recording());

        let kb = rec.to_keybinding();
        assert!(kb.is_some());
        assert_eq!(kb.unwrap().chord_count(), 1);
    }

    #[test]
    fn recorder_max_length_enforced() {
        let mut rec = KeySequenceRecorder::new(2);
        rec.start();
        let c1 = KeyCodeChord::new(true, false, false, false, KeyCode::KeyA);
        let c2 = KeyCodeChord::new(true, false, false, false, KeyCode::KeyB);
        let c3 = KeyCodeChord::new(true, false, false, false, KeyCode::KeyC);
        assert!(rec.record(c1));
        assert!(rec.record(c2));
        assert!(!rec.record(c3));
        assert_eq!(rec.len(), 2);
    }

    #[test]
    fn recorder_reset() {
        let mut rec = KeySequenceRecorder::new(4);
        rec.start();
        rec.record(KeyCodeChord::new(false, false, false, false, KeyCode::KeyX));
        rec.reset();
        assert!(!rec.is_recording());
        assert!(rec.is_empty());
        assert!(rec.to_keybinding().is_none());
    }

    #[test]
    fn keybinding_search_by_command() {
        let search = KeybindingSearch::new().command_contains("save");
        let kb = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        assert!(search.matches("editor.action.save", &kb));
        assert!(!search.matches("editor.action.copy", &kb));
    }

    #[test]
    fn keybinding_search_by_chord_count() {
        let search = KeybindingSearch::new().chord_count(2);
        let single = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        let double = Keybinding::two_chords(
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        );
        assert!(!search.matches("cmd", &single));
        assert!(search.matches("cmd", &double));
    }

    #[test]
    fn keybinding_search_requires_ctrl() {
        let search = KeybindingSearch::new().requires_ctrl(true);
        let with = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyA));
        let without = Keybinding::new(KeyCodeChord::new(false, false, false, false, KeyCode::KeyA));
        assert!(search.matches("cmd", &with));
        assert!(!search.matches("cmd", &without));
    }

    #[test]
    fn keybinding_stats_from_bindings() {
        let bindings = vec![
            Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
            Keybinding::new(KeyCodeChord::new(false, true, false, false, KeyCode::KeyA)),
            Keybinding::two_chords(
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ),
        ];
        let stats = KeybindingStats::from_bindings(&bindings);
        assert_eq!(stats.total, 3);
        assert_eq!(stats.single_chord, 2);
        assert_eq!(stats.multi_chord, 1);
        assert_eq!(stats.with_ctrl, 2);
        assert_eq!(stats.with_shift, 1);
    }

    #[test]
    fn keybinding_stats_display() {
        let stats = KeybindingStats {
            total: 5, single_chord: 3, multi_chord: 2,
            with_ctrl: 4, with_shift: 1, with_alt: 0, with_meta: 0,
        };
        let display = stats.to_string();
        assert!(display.contains("Total: 5"));
        assert!(display.contains("Single: 3"));
    }

    #[test]
    fn is_prefix_of_works() {
        let prefix = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyK));
        let full = Keybinding::two_chords(
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        );
        assert!(is_prefix_of(&prefix, &full));
        assert!(!is_prefix_of(&full, &prefix));
        assert!(!is_prefix_of(&prefix, &prefix));
    }

    // -- KeybindingConflictDetector --

    #[test]
    fn conflict_detector_exact() {
        let a = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        let b = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        let c = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyD));
        let det = KeybindingConflictDetector::new(vec![a, b, c]);
        assert_eq!(det.find_exact_conflicts().len(), 1);
    }

    #[test]
    fn conflict_detector_prefix() {
        let short = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyK));
        let long = Keybinding::two_chords(
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        );
        let det = KeybindingConflictDetector::new(vec![short, long]);
        assert_eq!(det.find_prefix_conflicts().len(), 1);
        assert_eq!(det.find_exact_conflicts().len(), 0);
    }

    #[test]
    fn conflict_detector_count() {
        let a = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
        let b = a.clone();
        let short = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyK));
        let long = Keybinding::two_chords(
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        );
        let det = KeybindingConflictDetector::new(vec![a, b, short, long]);
        assert_eq!(det.conflict_count(), 2); // 1 exact + 1 prefix
    }

    // -- KeybindingFuzzySearch --

    #[test]
    fn fuzzy_search_by_command() {
        let bindings = vec![
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
                command: "editor.action.save".into(),
                category: KeybindingCategory::Editor,
            },
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyF)),
                command: "editor.action.find".into(),
                category: KeybindingCategory::Editor,
            },
        ];
        let results = KeybindingFuzzySearch::search(&bindings, "save");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].binding_index, 0);
        assert_eq!(results[0].matched_on, FuzzyMatchField::Command);
    }

    #[test]
    fn fuzzy_search_by_key() {
        let bindings = vec![CategorizedKeybinding {
            binding: Keybinding::new(KeyCodeChord::new(true, true, false, false, KeyCode::KeyP)),
            command: "workbench.openCommandPalette".into(),
            category: KeybindingCategory::General,
        }];
        let results = KeybindingFuzzySearch::search(&bindings, "shift");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matched_on, FuzzyMatchField::Key);
    }

    #[test]
    fn fuzzy_search_empty_query() {
        let bindings = vec![CategorizedKeybinding {
            binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)),
            command: "save".into(),
            category: KeybindingCategory::General,
        }];
        assert!(KeybindingFuzzySearch::search(&bindings, "").is_empty());
    }

    #[test]
    fn fuzzy_search_prefix_scores_higher() {
        let bindings = vec![
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyA)),
                command: "go.to.line".into(),
                category: KeybindingCategory::Navigation,
            },
            CategorizedKeybinding {
                binding: Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyB)),
                command: "editor.goto".into(),
                category: KeybindingCategory::Editor,
            },
        ];
        let results = KeybindingFuzzySearch::search(&bindings, "go");
        assert_eq!(results.len(), 2);
        // "go.to.line" starts with "go" so should rank first
        assert_eq!(results[0].binding_index, 0);
        assert!(results[0].score > results[1].score);
    }

    // -- KeybindingRecordingSession --

    #[test]
    fn recording_session_basic() {
        let mut session = KeybindingRecordingSession::new();
        assert_eq!(session.recorded_count(), 0);
        assert!(session.to_keybinding().is_none());

        session.record_chord(KeyCodeChord::new(true, false, false, false, KeyCode::KeyK));
        assert_eq!(session.recorded_count(), 1);

        let kb = session.to_keybinding().unwrap();
        assert_eq!(kb.chord_count(), 1);
    }

    #[test]
    fn recording_session_two_chords() {
        let mut session = KeybindingRecordingSession::new();
        session.record_chord(KeyCodeChord::new(true, false, false, false, KeyCode::KeyK));
        session.record_chord(KeyCodeChord::new(true, false, false, false, KeyCode::KeyC));
        let kb = session.to_keybinding().unwrap();
        assert_eq!(kb.chord_count(), 2);
    }

    #[test]
    fn recording_session_drops_extra_chords() {
        let mut session = KeybindingRecordingSession::new();
        session.record_chord(KeyCodeChord::new(true, false, false, false, KeyCode::KeyK));
        session.record_chord(KeyCodeChord::new(true, false, false, false, KeyCode::KeyC));
        session.record_chord(KeyCodeChord::new(true, false, false, false, KeyCode::KeyD));
        assert_eq!(session.recorded_count(), 2);
    }

    #[test]
    fn recording_session_clear() {
        let mut session = KeybindingRecordingSession::new();
        session.record_chord(KeyCodeChord::new(true, false, false, false, KeyCode::KeyA));
        session.clear();
        assert_eq!(session.recorded_count(), 0);
        assert!(session.to_keybinding().is_none());
    }

    // -- PlatformKeybindingVariant --

    #[test]
    fn platform_variant_set_and_get() {
        let mut variant = PlatformKeybindingVariant::new("editor.copy");
        let linux_kb = Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyC));
        let mac_kb = Keybinding::new(KeyCodeChord::new(false, false, false, true, KeyCode::KeyC));
        variant.set_variant(Platform::Linux, linux_kb.clone());
        variant.set_variant(Platform::MacOS, mac_kb.clone());

        assert_eq!(variant.get_variant(Platform::Linux), Some(&linux_kb));
        assert_eq!(variant.get_variant(Platform::MacOS), Some(&mac_kb));
        assert_eq!(variant.get_variant(Platform::Windows), None);
        assert_eq!(variant.command(), "editor.copy");
    }

    #[test]
    fn platform_variant_platforms_list() {
        let mut variant = PlatformKeybindingVariant::new("test");
        variant.set_variant(
            Platform::Linux,
            Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyA)),
        );
        variant.set_variant(
            Platform::Windows,
            Keybinding::new(KeyCodeChord::new(true, false, false, false, KeyCode::KeyA)),
        );
        let platforms = variant.platforms_with_bindings();
        assert_eq!(platforms.len(), 2);
        assert!(platforms.contains(&Platform::Linux));
        assert!(platforms.contains(&Platform::Windows));
    }
}
