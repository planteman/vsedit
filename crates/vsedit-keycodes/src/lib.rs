//! Virtual key codes and binary encoding.
//!
//! Faithfully replicates VS Code's `vs/base/common/keyCodes.ts`, providing a
//! [`KeyCode`] enum with all 130+ virtual key codes, modifier flags via
//! [`KeyMod`], a [`KeyCodeChord`] struct for key-plus-modifier combinations,
//! and binary encoding/decoding routines that match VS Code's bit layout.

// ---------------------------------------------------------------------------
// KeyCode
// ---------------------------------------------------------------------------

/// Virtual key codes whose numeric values carry no inherent meaning.
///
/// The discriminant order matches VS Code's `KeyCode` enum exactly.
/// `DependsOnKbLayout` is intentionally omitted because Rust enums cannot have
/// a discriminant of −1; callers that need that sentinel should use
/// `Option<KeyCode>` with `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum KeyCode {
    /// Placed first to cover the 0 value of the enum.
    Unknown = 0,

    Backspace,
    Tab,
    Enter,
    Shift,
    Ctrl,
    Alt,
    PauseBreak,
    CapsLock,
    Escape,
    Space,
    PageUp,
    PageDown,
    End,
    Home,
    LeftArrow,
    UpArrow,
    RightArrow,
    DownArrow,
    Insert,
    Delete,

    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,

    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,

    Meta,
    ContextMenu,

    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,

    NumLock,
    ScrollLock,

    /// US keyboard `;:` key.
    Semicolon,
    /// US keyboard `=+` key.
    Equal,
    /// US keyboard `,<` key.
    Comma,
    /// US keyboard `-_` key.
    Minus,
    /// US keyboard `.>` key.
    Period,
    /// US keyboard `/?` key.
    Slash,
    /// US keyboard `` `~ `` key.
    Backquote,
    /// US keyboard `[{` key.
    BracketLeft,
    /// US keyboard `\|` key.
    Backslash,
    /// US keyboard `]}` key.
    BracketRight,
    /// US keyboard `'"` key.
    Quote,
    /// Miscellaneous OEM key (OEM_8).
    OEM_8,
    /// Angle-bracket / backslash on the RT 102-key keyboard.
    IntlBackslash,

    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,

    NumpadMultiply,
    NumpadAdd,
    NUMPAD_SEPARATOR,
    NumpadSubtract,
    NumpadDecimal,
    NumpadDivide,

    /// Covers all key codes when IME is processing input.
    KEY_IN_COMPOSITION,

    /// Brazilian (ABNT) keyboard.
    ABNT_C1,
    /// Brazilian (ABNT) keyboard.
    ABNT_C2,

    AudioVolumeMute,
    AudioVolumeUp,
    AudioVolumeDown,

    BrowserSearch,
    BrowserHome,
    BrowserBack,
    BrowserForward,

    MediaTrackNext,
    MediaTrackPrevious,
    MediaStop,
    MediaPlayPause,
    LaunchMediaPlayer,
    LaunchMail,
    LaunchApp2,

    /// VK_CLEAR, 0x0C.
    Clear,

    /// Placed last to cover the length of the enum.
    MAX_VALUE,
}

impl KeyCode {
    /// Total number of key codes (excluding `MAX_VALUE` itself).
    pub const COUNT: usize = Self::MAX_VALUE as usize;

    /// Convert a raw `u16` discriminant back to a `KeyCode`.
    ///
    /// Returns `KeyCode::Unknown` for out-of-range values.
    pub fn from_u16(value: u16) -> Self {
        if value < Self::MAX_VALUE as u16 {
            // SAFETY: all discriminants from 0..MAX_VALUE are valid enum variants
            // because the enum is `repr(u16)` and sequential.
            unsafe { std::mem::transmute(value) }
        } else {
            Self::Unknown
        }
    }

    /// Returns `true` if the key code represents a modifier key.
    pub fn is_modifier(self) -> bool {
        matches!(self, Self::Ctrl | Self::Shift | Self::Alt | Self::Meta)
    }
}

// ---------------------------------------------------------------------------
// KeyCode ↔ String
// ---------------------------------------------------------------------------

/// Return the canonical string representation of a [`KeyCode`].
///
/// This mirrors VS Code's `KeyCodeUtils.toString()`.
pub fn key_code_to_string(key_code: KeyCode) -> &'static str {
    KEY_CODE_TO_STR[key_code as usize]
}

/// Parse a string into a [`KeyCode`] (case-insensitive).
///
/// Returns `KeyCode::Unknown` for unrecognised strings.
pub fn string_to_key_code(s: &str) -> KeyCode {
    let lower = s.to_ascii_lowercase();
    for (i, &name) in KEY_CODE_TO_STR.iter().enumerate() {
        if !name.is_empty() && name.to_ascii_lowercase() == lower {
            return KeyCode::from_u16(i as u16);
        }
    }
    // Check alternate names used in user settings.
    match lower.as_str() {
        "right" => KeyCode::RightArrow,
        "left" => KeyCode::LeftArrow,
        "down" => KeyCode::DownArrow,
        "up" => KeyCode::UpArrow,
        _ => KeyCode::Unknown,
    }
}

/// Canonical string names indexed by `KeyCode` discriminant.
const KEY_CODE_TO_STR: &[&str] = &[
    "unknown",    // Unknown = 0
    "Backspace",  // 1
    "Tab",        // 2
    "Enter",      // 3
    "Shift",      // 4
    "Ctrl",       // 5
    "Alt",        // 6
    "PauseBreak", // 7
    "CapsLock",   // 8
    "Escape",     // 9
    "Space",      // 10
    "PageUp",     // 11
    "PageDown",   // 12
    "End",        // 13
    "Home",       // 14
    "LeftArrow",  // 15
    "UpArrow",    // 16
    "RightArrow", // 17
    "DownArrow",  // 18
    "Insert",     // 19
    "Delete",     // 20
    "0",          // Digit0 = 21
    "1",          // Digit1
    "2",          // Digit2
    "3",          // Digit3
    "4",          // Digit4
    "5",          // Digit5
    "6",          // Digit6
    "7",          // Digit7
    "8",          // Digit8
    "9",          // Digit9 = 30
    "A",          // KeyA = 31
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "I",
    "J",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",           // KeyZ = 56
    "Meta",        // 57
    "ContextMenu", // 58
    "F1",          // 59
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    "F13",
    "F14",
    "F15",
    "F16",
    "F17",
    "F18",
    "F19",
    "F20",
    "F21",
    "F22",
    "F23",
    "F24",        // 82
    "NumLock",    // 83
    "ScrollLock", // 84
    ";",          // Semicolon = 85
    "=",          // Equal
    ",",          // Comma
    "-",          // Minus
    ".",          // Period
    "/",          // Slash
    "`",          // Backquote
    "[",          // BracketLeft
    "\\",         // Backslash
    "]",          // BracketRight
    "'",          // Quote
    "OEM_8",      // 96
    "OEM_102",    // IntlBackslash = 97
    "NumPad0",    // 98
    "NumPad1",
    "NumPad2",
    "NumPad3",
    "NumPad4",
    "NumPad5",
    "NumPad6",
    "NumPad7",
    "NumPad8",
    "NumPad9",            // 107
    "NumPad_Multiply",    // 108
    "NumPad_Add",         // 109
    "NumPad_Separator",   // 110
    "NumPad_Subtract",    // 111
    "NumPad_Decimal",     // 112
    "NumPad_Divide",      // 113
    "KeyInComposition",   // 114
    "ABNT_C1",            // 115
    "ABNT_C2",            // 116
    "AudioVolumeMute",    // 117
    "AudioVolumeUp",      // 118
    "AudioVolumeDown",    // 119
    "BrowserSearch",      // 120
    "BrowserHome",        // 121
    "BrowserBack",        // 122
    "BrowserForward",     // 123
    "MediaTrackNext",     // 124
    "MediaTrackPrevious", // 125
    "MediaStop",          // 126
    "MediaPlayPause",     // 127
    "LaunchMediaPlayer",  // 128
    "LaunchMail",         // 129
    "LaunchApp2",         // 130
    "Clear",              // 131
];

// ---------------------------------------------------------------------------
// KeyMod
// ---------------------------------------------------------------------------

/// Modifier flags that can be combined with a [`KeyCode`] to form a keybinding.
///
/// The bit positions match VS Code's `KeyMod` enum exactly.
pub struct KeyMod;

impl KeyMod {
    /// Ctrl on Windows/Linux, Cmd on macOS.
    pub const CTRL_CMD: u32 = 1 << 11;
    /// Shift modifier.
    pub const SHIFT: u32 = 1 << 10;
    /// Alt modifier.
    pub const ALT: u32 = 1 << 9;
    /// Windows key on Windows, Ctrl on macOS.
    pub const WIN_CTRL: u32 = 1 << 8;
}

// ---------------------------------------------------------------------------
// KeyCodeChord
// ---------------------------------------------------------------------------

/// A single key press combined with modifier state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyCodeChord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
    pub key_code: KeyCode,
}

impl KeyCodeChord {
    /// Create a new chord from individual modifier flags and a key code.
    pub fn new(ctrl: bool, shift: bool, alt: bool, meta: bool, key_code: KeyCode) -> Self {
        Self {
            ctrl,
            shift,
            alt,
            meta,
            key_code,
        }
    }

    /// Create a chord with no modifiers.
    pub fn just(key_code: KeyCode) -> Self {
        Self::new(false, false, false, false, key_code)
    }
}

impl std::fmt::Display for KeyCodeChord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        if self.meta {
            f.write_str("Meta+")?;
        }
        f.write_str(key_code_to_string(self.key_code))
    }
}

// ---------------------------------------------------------------------------
// Binary encoding
// ---------------------------------------------------------------------------

/// Encode a [`KeyCodeChord`] into a `u32` using VS Code's bit layout.
///
/// | Bits  | Meaning    |
/// |-------|------------|
/// | 0–7   | key code   |
/// | 8     | ctrl       |
/// | 9     | shift      |
/// | 10    | alt        |
/// | 11    | meta       |
pub fn encode_chord(chord: &KeyCodeChord) -> u32 {
    let mut bits = chord.key_code as u32 & 0xFF;
    if chord.ctrl {
        bits |= 1 << 8;
    }
    if chord.shift {
        bits |= 1 << 9;
    }
    if chord.alt {
        bits |= 1 << 10;
    }
    if chord.meta {
        bits |= 1 << 11;
    }
    bits
}

/// Decode a `u32` produced by [`encode_chord`] back into a [`KeyCodeChord`].
pub fn decode_chord(bits: u32) -> KeyCodeChord {
    KeyCodeChord {
        ctrl: bits & (1 << 8) != 0,
        shift: bits & (1 << 9) != 0,
        alt: bits & (1 << 10) != 0,
        meta: bits & (1 << 11) != 0,
        key_code: KeyCode::from_u16((bits & 0xFF) as u16),
    }
}

/// Combine two encoded chords into a single chord-sequence value, matching
/// VS Code's `KeyChord(firstPart, secondPart)`.
///
/// The first part occupies the low 16 bits and the second part the high 16.
pub fn key_chord(first: u32, second: u32) -> u32 {
    let chord_part = (second & 0x0000_FFFF) << 16;
    first | chord_part
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for keycodes operations.
#[derive(Debug, Clone, PartialEq)]
pub struct KeycodesStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl KeycodesStats {
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns { self.max_operation_ns = duration_ns; }
        if duration_ns < self.min_operation_ns { self.min_operation_ns = duration_ns; }
    }

    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns { self.max_operation_ns = duration_ns; }
        if duration_ns < self.min_operation_ns { self.min_operation_ns = duration_ns; }
    }

    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 { return 0; }
        self.total_time_ns / self.total_operations
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 { return 1.0; }
        self.successful_operations as f64 / self.total_operations as f64
    }

    pub fn failure_rate(&self) -> f64 { 1.0 - self.success_rate() }

    pub fn total(&self) -> u64 { self.total_operations }

    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 { None } else { Some(self.min_operation_ns) }
    }

    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 { None } else { Some(self.max_operation_ns) }
    }

    pub fn reset(&mut self) { *self = Self::new(); }

    pub fn merge(&mut self, other: &KeycodesStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns { self.max_operation_ns = other.max_operation_ns; }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns { self.min_operation_ns = other.min_operation_ns; }
    }
}

impl Default for KeycodesStats { fn default() -> Self { Self::new() } }

/// Classification categories for key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCategory {
    /// Modifier keys (Ctrl, Shift, Alt, Meta).
    Modifier,
    /// Alphabetic keys (A-Z).
    Letter,
    /// Digit keys (0-9).
    Digit,
    /// Function keys (F1-F24).
    Function,
    /// Navigation keys (arrows, Home, End, PageUp, PageDown).
    Navigation,
    /// Numpad keys.
    Numpad,
    /// Punctuation / symbol keys.
    Punctuation,
    /// Media and browser keys.
    Media,
    /// All other keys.
    Other,
}

impl KeyCode {
    /// Classify this key code into a [`KeyCategory`].
    pub fn category(self) -> KeyCategory {
        match self {
            Self::Ctrl | Self::Shift | Self::Alt | Self::Meta => KeyCategory::Modifier,
            Self::KeyA | Self::KeyB | Self::KeyC | Self::KeyD | Self::KeyE
            | Self::KeyF | Self::KeyG | Self::KeyH | Self::KeyI | Self::KeyJ
            | Self::KeyK | Self::KeyL | Self::KeyM | Self::KeyN | Self::KeyO
            | Self::KeyP | Self::KeyQ | Self::KeyR | Self::KeyS | Self::KeyT
            | Self::KeyU | Self::KeyV | Self::KeyW | Self::KeyX | Self::KeyY
            | Self::KeyZ => KeyCategory::Letter,
            Self::Digit0 | Self::Digit1 | Self::Digit2 | Self::Digit3 | Self::Digit4
            | Self::Digit5 | Self::Digit6 | Self::Digit7 | Self::Digit8
            | Self::Digit9 => KeyCategory::Digit,
            Self::F1 | Self::F2 | Self::F3 | Self::F4 | Self::F5 | Self::F6
            | Self::F7 | Self::F8 | Self::F9 | Self::F10 | Self::F11 | Self::F12
            | Self::F13 | Self::F14 | Self::F15 | Self::F16 | Self::F17 | Self::F18
            | Self::F19 | Self::F20 | Self::F21 | Self::F22 | Self::F23
            | Self::F24 => KeyCategory::Function,
            Self::LeftArrow | Self::UpArrow | Self::RightArrow | Self::DownArrow
            | Self::Home | Self::End | Self::PageUp | Self::PageDown => KeyCategory::Navigation,
            Self::Numpad0 | Self::Numpad1 | Self::Numpad2 | Self::Numpad3
            | Self::Numpad4 | Self::Numpad5 | Self::Numpad6 | Self::Numpad7
            | Self::Numpad8 | Self::Numpad9 | Self::NumpadMultiply | Self::NumpadAdd
            | Self::NUMPAD_SEPARATOR | Self::NumpadSubtract | Self::NumpadDecimal
            | Self::NumpadDivide => KeyCategory::Numpad,
            Self::Semicolon | Self::Equal | Self::Comma | Self::Minus | Self::Period
            | Self::Slash | Self::Backquote | Self::BracketLeft | Self::Backslash
            | Self::BracketRight | Self::Quote => KeyCategory::Punctuation,
            Self::AudioVolumeMute | Self::AudioVolumeUp | Self::AudioVolumeDown
            | Self::BrowserSearch | Self::BrowserHome | Self::BrowserBack
            | Self::BrowserForward | Self::MediaTrackNext | Self::MediaTrackPrevious
            | Self::MediaStop | Self::MediaPlayPause | Self::LaunchMediaPlayer
            | Self::LaunchMail | Self::LaunchApp2 => KeyCategory::Media,
            _ => KeyCategory::Other,
        }
    }

    /// Returns `true` if this key code produces a printable character.
    pub fn is_printable(self) -> bool {
        matches!(
            self.category(),
            KeyCategory::Letter | KeyCategory::Digit | KeyCategory::Punctuation
        ) || self == Self::Space
    }

    /// Returns a human-readable display name for this key code.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Backspace => "Backspace",
            Self::Tab => "Tab",
            Self::Enter => "Enter",
            Self::Shift => "Shift",
            Self::Ctrl => "Control",
            Self::Alt => "Alt",
            Self::Meta => "Meta",
            Self::Escape => "Escape",
            Self::Space => "Space",
            Self::Delete => "Delete",
            Self::Insert => "Insert",
            Self::Home => "Home",
            Self::End => "End",
            Self::PageUp => "Page Up",
            Self::PageDown => "Page Down",
            Self::LeftArrow => "Left Arrow",
            Self::RightArrow => "Right Arrow",
            Self::UpArrow => "Up Arrow",
            Self::DownArrow => "Down Arrow",
            Self::CapsLock => "Caps Lock",
            Self::NumLock => "Num Lock",
            Self::ScrollLock => "Scroll Lock",
            Self::PauseBreak => "Pause/Break",
            Self::ContextMenu => "Context Menu",
            _ => key_code_to_string(self),
        }
    }
}

// ---------------------------------------------------------------------------
// KeyChordParseError
// ---------------------------------------------------------------------------

/// Errors that can occur when parsing a key chord string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyChordParseError {
    /// The input string was empty or whitespace-only.
    EmptyInput,
    /// A key name was not recognised.
    UnknownKey(String),
    /// The chord string had an invalid format.
    InvalidFormat(String),
}

impl std::fmt::Display for KeyChordParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInput => f.write_str("empty input"),
            Self::UnknownKey(k) => write!(f, "unknown key: {k}"),
            Self::InvalidFormat(msg) => write!(f, "invalid format: {msg}"),
        }
    }
}

impl std::error::Error for KeyChordParseError {}

// ---------------------------------------------------------------------------
// KeyCombo
// ---------------------------------------------------------------------------

/// A sequence of one or more key chords (e.g. `Ctrl+K Ctrl+C`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub chords: Vec<KeyCodeChord>,
}

impl KeyCombo {
    pub fn new(chords: Vec<KeyCodeChord>) -> Self {
        Self { chords }
    }

    pub fn single(chord: KeyCodeChord) -> Self {
        Self {
            chords: vec![chord],
        }
    }

    pub fn len(&self) -> usize {
        self.chords.len()
    }

    pub fn is_single(&self) -> bool {
        self.chords.len() == 1
    }

    pub fn first(&self) -> Option<&KeyCodeChord> {
        self.chords.first()
    }
}

impl std::fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, chord) in self.chords.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{chord}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// KeyChordParser
// ---------------------------------------------------------------------------

/// Parses VS Code-style key chord strings such as `"ctrl+shift+p"` or
/// `"ctrl+k ctrl+c"`.
pub struct KeyChordParser;

impl KeyChordParser {
    /// Parse a single key chord like `"ctrl+shift+p"`.
    pub fn parse(input: &str) -> Result<KeyCodeChord, KeyChordParseError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(KeyChordParseError::EmptyInput);
        }

        let tokens: Vec<&str> = input.split('+').collect();
        if tokens.is_empty() || tokens.iter().any(|t| t.is_empty()) {
            return Err(KeyChordParseError::InvalidFormat(format!(
                "bad chord: {input}"
            )));
        }

        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut meta = false;

        // All tokens except the last are modifiers; the last is the key.
        let (modifier_tokens, key_token) = tokens.split_at(tokens.len() - 1);
        let key_str = key_token[0];

        for &tok in modifier_tokens {
            match tok.to_ascii_lowercase().as_str() {
                "ctrl" | "cmd" => ctrl = true,
                "shift" => shift = true,
                "alt" => alt = true,
                "meta" => meta = true,
                _ => {
                    return Err(KeyChordParseError::InvalidFormat(format!(
                        "unknown modifier: {tok}"
                    )));
                }
            }
        }

        let key_code = string_to_key_code(key_str);
        if key_code == KeyCode::Unknown && !key_str.eq_ignore_ascii_case("unknown") {
            return Err(KeyChordParseError::UnknownKey(key_str.to_string()));
        }

        Ok(KeyCodeChord::new(ctrl, shift, alt, meta, key_code))
    }

    /// Parse a multi-part chord sequence like `"ctrl+k ctrl+c"`.
    pub fn parse_sequence(input: &str) -> Result<KeyCombo, KeyChordParseError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(KeyChordParseError::EmptyInput);
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        let mut chords = Vec::with_capacity(parts.len());
        for part in parts {
            chords.push(Self::parse(part)?);
        }
        Ok(KeyCombo::new(chords))
    }
}

// ---------------------------------------------------------------------------
// Convenience wrappers
// ---------------------------------------------------------------------------

/// Return the canonical string name for a [`KeyCode`]. Delegates to
/// [`key_code_to_string`].
pub fn keycode_to_name(code: KeyCode) -> &'static str {
    key_code_to_string(code)
}

/// Look up a [`KeyCode`] by name. Returns `None` if the name is unrecognised.
/// Delegates to [`string_to_key_code`].
pub fn name_to_keycode(name: &str) -> Option<KeyCode> {
    let kc = string_to_key_code(name);
    if kc == KeyCode::Unknown && !name.eq_ignore_ascii_case("unknown") {
        None
    } else {
        Some(kc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminant_values_match_vscode() {
        assert_eq!(KeyCode::Unknown as u16, 0);
        assert_eq!(KeyCode::Backspace as u16, 1);
        assert_eq!(KeyCode::Tab as u16, 2);
        assert_eq!(KeyCode::Enter as u16, 3);
        assert_eq!(KeyCode::Shift as u16, 4);
        assert_eq!(KeyCode::Ctrl as u16, 5);
        assert_eq!(KeyCode::Alt as u16, 6);
        assert_eq!(KeyCode::Space as u16, 10);
        assert_eq!(KeyCode::Digit0 as u16, 21);
        assert_eq!(KeyCode::KeyA as u16, 31);
        assert_eq!(KeyCode::KeyZ as u16, 56);
        assert_eq!(KeyCode::Meta as u16, 57);
        assert_eq!(KeyCode::F1 as u16, 59);
        assert_eq!(KeyCode::F24 as u16, 82);
        assert_eq!(KeyCode::NumLock as u16, 83);
        assert_eq!(KeyCode::Semicolon as u16, 85);
        assert_eq!(KeyCode::Numpad0 as u16, 98);
        assert_eq!(KeyCode::NumpadDivide as u16, 113);
    }

    #[test]
    fn from_u16_roundtrip() {
        for v in 0..KeyCode::MAX_VALUE as u16 {
            let kc = KeyCode::from_u16(v);
            assert_eq!(kc as u16, v);
        }
    }

    #[test]
    fn from_u16_out_of_range() {
        assert_eq!(KeyCode::from_u16(0xFFFF), KeyCode::Unknown);
        assert_eq!(
            KeyCode::from_u16(KeyCode::MAX_VALUE as u16),
            KeyCode::Unknown
        );
    }

    #[test]
    fn key_code_string_roundtrip() {
        let codes = [
            KeyCode::Backspace,
            KeyCode::Enter,
            KeyCode::Space,
            KeyCode::F1,
            KeyCode::KeyA,
            KeyCode::LeftArrow,
            KeyCode::NumpadAdd,
            KeyCode::AudioVolumeMute,
        ];
        for kc in codes {
            let s = key_code_to_string(kc);
            assert!(!s.is_empty(), "string for {kc:?} must not be empty");
            let parsed = string_to_key_code(s);
            assert_eq!(parsed, kc, "roundtrip failed for {kc:?} → {s:?}");
        }
    }

    #[test]
    fn string_to_key_code_case_insensitive() {
        assert_eq!(string_to_key_code("enter"), KeyCode::Enter);
        assert_eq!(string_to_key_code("ENTER"), KeyCode::Enter);
        assert_eq!(string_to_key_code("Enter"), KeyCode::Enter);
    }

    #[test]
    fn string_to_key_code_unknown() {
        assert_eq!(string_to_key_code("not_a_key"), KeyCode::Unknown);
    }

    #[test]
    fn string_to_key_code_arrow_aliases() {
        assert_eq!(string_to_key_code("Right"), KeyCode::RightArrow);
        assert_eq!(string_to_key_code("Left"), KeyCode::LeftArrow);
        assert_eq!(string_to_key_code("Down"), KeyCode::DownArrow);
        assert_eq!(string_to_key_code("Up"), KeyCode::UpArrow);
    }

    #[test]
    fn is_modifier() {
        assert!(KeyCode::Ctrl.is_modifier());
        assert!(KeyCode::Shift.is_modifier());
        assert!(KeyCode::Alt.is_modifier());
        assert!(KeyCode::Meta.is_modifier());
        assert!(!KeyCode::Enter.is_modifier());
        assert!(!KeyCode::KeyA.is_modifier());
    }

    #[test]
    fn encode_decode_roundtrip() {
        let chord = KeyCodeChord::new(true, false, true, false, KeyCode::KeyS);
        let bits = encode_chord(&chord);
        let decoded = decode_chord(bits);
        assert_eq!(decoded, chord);
    }

    #[test]
    fn encode_bits_layout() {
        let chord = KeyCodeChord::new(true, true, true, true, KeyCode::KeyA);
        let bits = encode_chord(&chord);
        assert_eq!(bits & 0xFF, KeyCode::KeyA as u32);
        assert_ne!(bits & (1 << 8), 0, "ctrl bit");
        assert_ne!(bits & (1 << 9), 0, "shift bit");
        assert_ne!(bits & (1 << 10), 0, "alt bit");
        assert_ne!(bits & (1 << 11), 0, "meta bit");
    }

    #[test]
    fn encode_no_modifiers() {
        let chord = KeyCodeChord::just(KeyCode::Escape);
        let bits = encode_chord(&chord);
        assert_eq!(bits, KeyCode::Escape as u32);
    }

    #[test]
    fn key_mod_constants() {
        assert_eq!(KeyMod::CTRL_CMD, 1 << 11);
        assert_eq!(KeyMod::SHIFT, 1 << 10);
        assert_eq!(KeyMod::ALT, 1 << 9);
        assert_eq!(KeyMod::WIN_CTRL, 1 << 8);
    }

    #[test]
    fn key_chord_combines() {
        let first = encode_chord(&KeyCodeChord::new(true, false, false, false, KeyCode::KeyK));
        let second = encode_chord(&KeyCodeChord::new(true, false, false, false, KeyCode::KeyC));
        let combined = key_chord(first, second);
        assert_eq!(combined & 0x0000_FFFF, first);
        assert_eq!((combined >> 16) & 0x0000_FFFF, second);
    }

    #[test]
    fn chord_display() {
        let chord = KeyCodeChord::new(true, true, false, false, KeyCode::KeyS);
        assert_eq!(chord.to_string(), "Ctrl+Shift+S");
    }

    #[test]
    fn chord_display_no_modifiers() {
        let chord = KeyCodeChord::just(KeyCode::F5);
        assert_eq!(chord.to_string(), "F5");
    }

    #[test]
    fn key_code_to_str_table_length() {
        // Ensure the string table covers all variants up to MAX_VALUE.
        assert_eq!(KEY_CODE_TO_STR.len(), KeyCode::MAX_VALUE as usize);
    }

    #[test]
    fn punctuation_strings() {
        assert_eq!(key_code_to_string(KeyCode::Semicolon), ";");
        assert_eq!(key_code_to_string(KeyCode::Equal), "=");
        assert_eq!(key_code_to_string(KeyCode::Comma), ",");
        assert_eq!(key_code_to_string(KeyCode::Minus), "-");
        assert_eq!(key_code_to_string(KeyCode::Period), ".");
        assert_eq!(key_code_to_string(KeyCode::Slash), "/");
        assert_eq!(key_code_to_string(KeyCode::Backquote), "`");
        assert_eq!(key_code_to_string(KeyCode::BracketLeft), "[");
        assert_eq!(key_code_to_string(KeyCode::Backslash), "\\");
        assert_eq!(key_code_to_string(KeyCode::BracketRight), "]");
        assert_eq!(key_code_to_string(KeyCode::Quote), "'");
    }

    #[test]
    fn keycodes_stats_new_defaults() {
        let stats = KeycodesStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn keycodes_stats_record_success() {
        let mut stats = KeycodesStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
    }

    #[test]
    fn keycodes_stats_record_failure() {
        let mut stats = KeycodesStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn keycodes_stats_reset() {
        let mut stats = KeycodesStats::new();
        stats.record_success(500);
        stats.reset();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn category_modifier() {
        assert_eq!(KeyCode::Ctrl.category(), KeyCategory::Modifier);
        assert_eq!(KeyCode::Shift.category(), KeyCategory::Modifier);
        assert_eq!(KeyCode::Alt.category(), KeyCategory::Modifier);
        assert_eq!(KeyCode::Meta.category(), KeyCategory::Modifier);
    }

    #[test]
    fn category_letter_and_digit() {
        assert_eq!(KeyCode::KeyA.category(), KeyCategory::Letter);
        assert_eq!(KeyCode::KeyZ.category(), KeyCategory::Letter);
        assert_eq!(KeyCode::Digit0.category(), KeyCategory::Digit);
        assert_eq!(KeyCode::Digit9.category(), KeyCategory::Digit);
    }

    #[test]
    fn category_function() {
        assert_eq!(KeyCode::F1.category(), KeyCategory::Function);
        assert_eq!(KeyCode::F12.category(), KeyCategory::Function);
        assert_eq!(KeyCode::F24.category(), KeyCategory::Function);
    }

    #[test]
    fn category_navigation() {
        assert_eq!(KeyCode::LeftArrow.category(), KeyCategory::Navigation);
        assert_eq!(KeyCode::Home.category(), KeyCategory::Navigation);
        assert_eq!(KeyCode::PageUp.category(), KeyCategory::Navigation);
    }

    #[test]
    fn is_printable_letters_digits() {
        assert!(KeyCode::KeyA.is_printable());
        assert!(KeyCode::Digit5.is_printable());
        assert!(KeyCode::Space.is_printable());
        assert!(KeyCode::Comma.is_printable());
        assert!(!KeyCode::Ctrl.is_printable());
        assert!(!KeyCode::F1.is_printable());
        assert!(!KeyCode::LeftArrow.is_printable());
    }

    #[test]
    fn display_name_special_keys() {
        assert_eq!(KeyCode::PageUp.display_name(), "Page Up");
        assert_eq!(KeyCode::LeftArrow.display_name(), "Left Arrow");
        assert_eq!(KeyCode::CapsLock.display_name(), "Caps Lock");
        assert_eq!(KeyCode::Ctrl.display_name(), "Control");
    }

    #[test]
    fn category_numpad() {
        assert_eq!(KeyCode::Numpad0.category(), KeyCategory::Numpad);
        assert_eq!(KeyCode::NumpadAdd.category(), KeyCategory::Numpad);
    }

    #[test]
    fn category_media() {
        assert_eq!(KeyCode::AudioVolumeMute.category(), KeyCategory::Media);
        assert_eq!(KeyCode::BrowserBack.category(), KeyCategory::Media);
    }

    #[test]
    fn keycodes_stats_merge() {
        let mut a = KeycodesStats::new();
        a.record_success(100);
        let mut b = KeycodesStats::new();
        b.record_failure(50);
        a.merge(&b);
        assert_eq!(a.total(), 2);
        assert_eq!(a.min_time_ns(), Some(50));
    }

    // -----------------------------------------------------------------------
    // KeyChordParser / KeyCombo / convenience wrappers
    // -----------------------------------------------------------------------

    #[test]
    fn parse_simple_chord() {
        let chord = KeyChordParser::parse("ctrl+shift+p").unwrap();
        assert!(chord.ctrl);
        assert!(chord.shift);
        assert!(!chord.alt);
        assert!(!chord.meta);
        assert_eq!(chord.key_code, KeyCode::KeyP);
    }

    #[test]
    fn parse_single_key_no_modifiers() {
        let chord = KeyChordParser::parse("Escape").unwrap();
        assert!(!chord.ctrl);
        assert!(!chord.shift);
        assert_eq!(chord.key_code, KeyCode::Escape);
    }

    #[test]
    fn parse_alt_f4() {
        let chord = KeyChordParser::parse("alt+F4").unwrap();
        assert!(chord.alt);
        assert_eq!(chord.key_code, KeyCode::F4);
    }

    #[test]
    fn parse_case_insensitive_modifiers() {
        let chord = KeyChordParser::parse("CTRL+SHIFT+A").unwrap();
        assert!(chord.ctrl);
        assert!(chord.shift);
        assert_eq!(chord.key_code, KeyCode::KeyA);
    }

    #[test]
    fn parse_cmd_as_ctrl() {
        let chord = KeyChordParser::parse("cmd+c").unwrap();
        assert!(chord.ctrl);
        assert_eq!(chord.key_code, KeyCode::KeyC);
    }

    #[test]
    fn parse_meta_modifier() {
        let chord = KeyChordParser::parse("meta+s").unwrap();
        assert!(chord.meta);
        assert_eq!(chord.key_code, KeyCode::KeyS);
    }

    #[test]
    fn parse_empty_input_error() {
        assert_eq!(KeyChordParser::parse(""), Err(KeyChordParseError::EmptyInput));
        assert_eq!(KeyChordParser::parse("   "), Err(KeyChordParseError::EmptyInput));
    }

    #[test]
    fn parse_unknown_key_error() {
        let err = KeyChordParser::parse("ctrl+nonsensekey").unwrap_err();
        assert!(matches!(err, KeyChordParseError::UnknownKey(_)));
    }

    #[test]
    fn parse_sequence_two_part() {
        let combo = KeyChordParser::parse_sequence("ctrl+k ctrl+c").unwrap();
        assert_eq!(combo.len(), 2);
        assert!(!combo.is_single());
        assert_eq!(combo.chords[0].key_code, KeyCode::KeyK);
        assert_eq!(combo.chords[1].key_code, KeyCode::KeyC);
        assert!(combo.chords[0].ctrl);
        assert!(combo.chords[1].ctrl);
    }

    #[test]
    fn parse_sequence_single_part() {
        let combo = KeyChordParser::parse_sequence("shift+tab").unwrap();
        assert!(combo.is_single());
        assert_eq!(combo.first().unwrap().key_code, KeyCode::Tab);
    }

    #[test]
    fn key_combo_display() {
        let combo = KeyCombo::new(vec![
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        ]);
        assert_eq!(combo.to_string(), "Ctrl+K Ctrl+C");
    }

    #[test]
    fn keycode_to_name_and_back() {
        let name = keycode_to_name(KeyCode::Enter);
        assert_eq!(name, "Enter");
        assert_eq!(name_to_keycode("Enter"), Some(KeyCode::Enter));
    }

    #[test]
    fn name_to_keycode_unknown_returns_none() {
        assert_eq!(name_to_keycode("totallyinvalid"), None);
    }
}
