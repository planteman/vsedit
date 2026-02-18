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
use std::fmt;
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

// ---------------------------------------------------------------------------
// KeyCodeChord helpers
// ---------------------------------------------------------------------------

impl KeyCodeChord {
    /// Returns `true` if any modifier key is active.
    pub fn has_modifier(&self) -> bool {
        self.ctrl || self.shift || self.alt || self.meta
    }

    /// Count the number of active modifier keys (0–4).
    pub fn modifier_count(&self) -> u8 {
        self.ctrl as u8 + self.shift as u8 + self.alt as u8 + self.meta as u8
    }

    /// Returns `true` if no modifier keys are active.
    pub fn is_plain(&self) -> bool {
        !self.has_modifier()
    }

    /// Builder: set ctrl and return self.
    pub fn with_ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    /// Builder: set shift and return self.
    pub fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }

    /// Builder: set alt and return self.
    pub fn with_alt(mut self) -> Self {
        self.alt = true;
        self
    }

    /// Builder: set meta and return self.
    pub fn with_meta(mut self) -> Self {
        self.meta = true;
        self
    }
}

// ---------------------------------------------------------------------------
// KeyCombo helpers
// ---------------------------------------------------------------------------

impl KeyCombo {
    /// Returns `true` if the combo has no chords.
    pub fn is_empty(&self) -> bool {
        self.chords.is_empty()
    }
}

// ---------------------------------------------------------------------------
// KeyCode classification helpers
// ---------------------------------------------------------------------------

impl KeyCode {
    /// Returns `true` if this is a letter key (A-Z).
    pub fn is_letter(self) -> bool {
        matches!(self.category(), KeyCategory::Letter)
    }

    /// Returns `true` if this is a digit key (0-9).
    pub fn is_digit(self) -> bool {
        matches!(self.category(), KeyCategory::Digit)
    }

    /// Returns `true` if this is a function key (F1-F24).
    pub fn is_function_key(self) -> bool {
        matches!(self.category(), KeyCategory::Function)
    }
}

// ---------------------------------------------------------------------------
// KeyCategory helpers
// ---------------------------------------------------------------------------

impl KeyCategory {
    /// Returns `true` for navigation keys (arrows, Home, End, PageUp, PageDown).
    pub fn is_navigation(&self) -> bool {
        matches!(self, KeyCategory::Navigation)
    }
}

// ---------------------------------------------------------------------------
// Modifier mask operations
// ---------------------------------------------------------------------------

/// A bitmask representation of modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModifierMask(u8);

impl ModifierMask {
    pub const NONE: Self = Self(0);
    pub const CTRL: Self = Self(1 << 0);
    pub const SHIFT: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const META: Self = Self(1 << 3);

    /// Create a mask from a chord's modifiers.
    pub fn from_chord(chord: &KeyCodeChord) -> Self {
        let mut mask = 0u8;
        if chord.ctrl { mask |= 1 << 0; }
        if chord.shift { mask |= 1 << 1; }
        if chord.alt { mask |= 1 << 2; }
        if chord.meta { mask |= 1 << 3; }
        Self(mask)
    }

    /// Check if this mask contains all modifiers of `other`.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Combine two masks (union).
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Intersect two masks.
    pub fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Number of active modifier bits.
    pub fn count(self) -> u8 {
        self.0.count_ones() as u8
    }

    /// Whether no modifiers are active.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Display for ModifierMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if self.contains(Self::CTRL) { parts.push("Ctrl"); }
        if self.contains(Self::SHIFT) { parts.push("Shift"); }
        if self.contains(Self::ALT) { parts.push("Alt"); }
        if self.contains(Self::META) { parts.push("Meta"); }
        if parts.is_empty() {
            write!(f, "(none)")
        } else {
            write!(f, "{}", parts.join("+"))
        }
    }
}

// ---------------------------------------------------------------------------
// Key sequence matching
// ---------------------------------------------------------------------------

impl KeyCombo {
    /// Check if this combo matches a prefix of `input` chords.
    /// Returns `true` if all chords in `self` match the corresponding chords
    /// at the beginning of `input`.
    pub fn is_prefix_of(&self, input: &[KeyCodeChord]) -> bool {
        if self.chords.len() > input.len() {
            return false;
        }
        self.chords.iter().zip(input.iter()).all(|(a, b)| a == b)
    }

    /// Check if this combo exactly matches the given chord slice.
    pub fn matches(&self, input: &[KeyCodeChord]) -> bool {
        self.chords.len() == input.len() && self.is_prefix_of(input)
    }
}

// ---------------------------------------------------------------------------
// Key combination display formatting
// ---------------------------------------------------------------------------

/// Format a key chord for display using platform-specific modifier symbols.
pub fn format_chord_display(chord: &KeyCodeChord, use_symbols: bool) -> String {
    let mut parts = Vec::new();
    if chord.ctrl {
        parts.push(if use_symbols { "⌃" } else { "Ctrl" });
    }
    if chord.shift {
        parts.push(if use_symbols { "⇧" } else { "Shift" });
    }
    if chord.alt {
        parts.push(if use_symbols { "⌥" } else { "Alt" });
    }
    if chord.meta {
        parts.push(if use_symbols { "⌘" } else { "Meta" });
    }
    parts.push(chord.key_code.display_name());
    parts.join(if use_symbols { "" } else { "+" })
}

/// Format a key combo for display.
pub fn format_combo_display(combo: &KeyCombo, use_symbols: bool) -> String {
    combo
        .chords
        .iter()
        .map(|c| format_chord_display(c, use_symbols))
        .collect::<Vec<_>>()
        .join(" ")
}

impl KeyCodeChord {
    /// Returns true if the chord uses only Ctrl (no Shift/Alt/Meta).
    pub fn is_ctrl_only(&self) -> bool {
        self.ctrl && !self.shift && !self.alt && !self.meta
    }

    /// Returns true if the chord uses Ctrl+Shift without Alt/Meta.
    pub fn is_ctrl_shift(&self) -> bool {
        self.ctrl && self.shift && !self.alt && !self.meta
    }

    /// Create a chord with the same modifiers but a different key code.
    pub fn with_key(self, key_code: KeyCode) -> Self {
        Self { key_code, ..self }
    }

    /// Returns true if two chords have the same modifier state.
    pub fn same_modifiers(&self, other: &KeyCodeChord) -> bool {
        self.ctrl == other.ctrl && self.shift == other.shift
            && self.alt == other.alt && self.meta == other.meta
    }
}

impl KeyCombo {
    /// Returns the last chord in the combo, if any.
    pub fn last(&self) -> Option<&KeyCodeChord> {
        self.chords.last()
    }

    /// Returns true if any chord in the combo has the given key code.
    pub fn contains_key(&self, key_code: KeyCode) -> bool {
        self.chords.iter().any(|c| c.key_code == key_code)
    }

    /// Returns the total number of modifier keys used across all chords.
    pub fn total_modifier_count(&self) -> u8 {
        self.chords.iter().map(|c| c.modifier_count()).sum()
    }
}

/// Collect all unique key codes from a slice of chords.
pub fn unique_key_codes(chords: &[KeyCodeChord]) -> Vec<KeyCode> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for chord in chords {
        if seen.insert(chord.key_code as u16) {
            result.push(chord.key_code);
        }
    }
    result
}

/// Returns true if the key code represents a whitespace key (Space, Tab, Enter).
pub fn is_whitespace_key(key_code: KeyCode) -> bool {
    matches!(key_code, KeyCode::Space | KeyCode::Tab | KeyCode::Enter)
}

/// Count how many chords in a slice use the given modifier.
pub fn count_chords_with_ctrl(chords: &[KeyCodeChord]) -> usize {
    chords.iter().filter(|c| c.ctrl).count()
}

// ---------------------------------------------------------------------------
// KeyCode → letter/digit conversion
// ---------------------------------------------------------------------------

impl KeyCode {
    /// Convert a letter key code to its lowercase ASCII character.
    /// Returns `None` for non-letter key codes.
    pub fn to_char(self) -> Option<char> {
        if self.is_letter() {
            let offset = self as u16 - KeyCode::KeyA as u16;
            Some((b'a' + offset as u8) as char)
        } else if self.is_digit() {
            let offset = self as u16 - KeyCode::Digit0 as u16;
            Some((b'0' + offset as u8) as char)
        } else {
            None
        }
    }

    /// Create a `KeyCode` from an ASCII character.
    /// Supports `a`–`z`, `A`–`Z`, and `0`–`9`.
    pub fn from_char(ch: char) -> Option<Self> {
        match ch {
            'a'..='z' => {
                let offset = ch as u16 - b'a' as u16;
                Some(KeyCode::from_u16(KeyCode::KeyA as u16 + offset))
            }
            'A'..='Z' => {
                let offset = ch as u16 - b'A' as u16;
                Some(KeyCode::from_u16(KeyCode::KeyA as u16 + offset))
            }
            '0'..='9' => {
                let offset = ch as u16 - b'0' as u16;
                Some(KeyCode::from_u16(KeyCode::Digit0 as u16 + offset))
            }
            _ => None,
        }
    }

    /// Return the function key number (1–24) if this is an F-key, else `None`.
    pub fn function_key_number(self) -> Option<u8> {
        if self.is_function_key() {
            Some((self as u16 - KeyCode::F1 as u16 + 1) as u8)
        } else {
            None
        }
    }

    /// Create a function key code from a number (1–24).
    pub fn from_function_key(n: u8) -> Option<Self> {
        if (1..=24).contains(&n) {
            Some(KeyCode::from_u16(KeyCode::F1 as u16 + n as u16 - 1))
        } else {
            None
        }
    }

    /// Return the numpad digit (0–9) if this is a numpad digit key, else `None`.
    pub fn numpad_digit(self) -> Option<u8> {
        let v = self as u16;
        let start = KeyCode::Numpad0 as u16;
        let end = KeyCode::Numpad9 as u16;
        if v >= start && v <= end {
            Some((v - start) as u8)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// KeyCodeChord ↔ KeyMod encoded integer
// ---------------------------------------------------------------------------

impl KeyCodeChord {
    /// Construct a chord from a combined `KeyMod` | `KeyCode` integer,
    /// matching VS Code's `KeyMod.CtrlCmd | KeyCode.KeyS` pattern.
    pub fn from_keymod_value(value: u32) -> Self {
        Self {
            ctrl: value & KeyMod::CTRL_CMD != 0,
            shift: value & KeyMod::SHIFT != 0,
            alt: value & KeyMod::ALT != 0,
            meta: value & KeyMod::WIN_CTRL != 0,
            key_code: KeyCode::from_u16((value & 0xFF) as u16),
        }
    }

    /// Encode this chord using the `KeyMod` bit layout (high-nibble modifiers).
    pub fn to_keymod_value(&self) -> u32 {
        let mut v = self.key_code as u32 & 0xFF;
        if self.ctrl { v |= KeyMod::CTRL_CMD; }
        if self.shift { v |= KeyMod::SHIFT; }
        if self.alt { v |= KeyMod::ALT; }
        if self.meta { v |= KeyMod::WIN_CTRL; }
        v
    }

    /// Strip all modifiers, returning a plain chord with the same key.
    pub fn strip_modifiers(&self) -> Self {
        Self::just(self.key_code)
    }
}

// ---------------------------------------------------------------------------
// Batch encoding helpers
// ---------------------------------------------------------------------------

/// Encode a slice of chords into a `Vec<u32>`.
pub fn encode_chords(chords: &[KeyCodeChord]) -> Vec<u32> {
    chords.iter().map(encode_chord).collect()
}

/// Decode a slice of `u32` values into chords.
pub fn decode_chords(values: &[u32]) -> Vec<KeyCodeChord> {
    values.iter().map(|&v| decode_chord(v)).collect()
}

/// Split a combined two-chord value produced by [`key_chord`] back into
/// its two encoded parts. Returns `(first, second)` where `second` is 0
/// if the value only encodes a single chord.
pub fn split_key_chord(combined: u32) -> (u32, u32) {
    let first = combined & 0x0000_FFFF;
    let second = (combined >> 16) & 0x0000_FFFF;
    (first, second)
}

// ---------------------------------------------------------------------------
// KeyCombo encoding
// ---------------------------------------------------------------------------

impl KeyCombo {
    /// Encode the combo into a vector of `u32` values (one per chord).
    pub fn encode(&self) -> Vec<u32> {
        encode_chords(&self.chords)
    }

    /// Decode a combo from a slice of encoded `u32` values.
    pub fn decode(values: &[u32]) -> Self {
        Self::new(decode_chords(values))
    }

    /// Push a new chord onto this combo.
    pub fn push(&mut self, chord: KeyCodeChord) {
        self.chords.push(chord);
    }

    /// Return the combo with an additional chord appended.
    pub fn then(mut self, chord: KeyCodeChord) -> Self {
        self.chords.push(chord);
        self
    }

    /// Returns true if every chord in the combo uses no modifiers.
    pub fn is_all_plain(&self) -> bool {
        self.chords.iter().all(|c| c.is_plain())
    }
}


// ---------------------------------------------------------------------------
// KeyCodeLabel -- display strings for key codes
// ---------------------------------------------------------------------------

pub struct KeyCodeLabel;

impl KeyCodeLabel {
    pub fn label(kc: KeyCode) -> &'static str {
        match kc {
            KeyCode::Backspace => "Backspace",
            KeyCode::Tab => "Tab",
            KeyCode::Enter => "Enter",
            KeyCode::Escape => "Esc",
            KeyCode::Space => "Space",
            KeyCode::Delete => "Del",
            KeyCode::Home => "Home",
            KeyCode::End => "End",
            KeyCode::PageUp => "PgUp",
            KeyCode::PageDown => "PgDn",
            KeyCode::UpArrow => "Up",
            KeyCode::DownArrow => "Down",
            KeyCode::LeftArrow => "Left",
            KeyCode::RightArrow => "Right",
            KeyCode::Insert => "Ins",
            _ => kc.display_name(),
        }
    }

    pub fn modifier_symbol(kc: KeyCode) -> Option<char> {
        match kc {
            KeyCode::Ctrl => Some('^'),
            KeyCode::Shift => Some('S'),
            KeyCode::Alt => Some('A'),
            KeyCode::Meta => Some('M'),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// KeyCodeCategoryMap -- grouping by category
// ---------------------------------------------------------------------------

pub struct KeyCodeCategoryMap;

impl KeyCodeCategoryMap {
    pub fn codes_for_category(cat: KeyCategory) -> Vec<KeyCode> {
        let all_codes = [
            KeyCode::Ctrl, KeyCode::Shift, KeyCode::Alt, KeyCode::Meta,
            KeyCode::UpArrow, KeyCode::DownArrow, KeyCode::LeftArrow, KeyCode::RightArrow,
            KeyCode::Home, KeyCode::End, KeyCode::PageUp, KeyCode::PageDown,
            KeyCode::F1, KeyCode::F2, KeyCode::F3, KeyCode::F4,
            KeyCode::F5, KeyCode::F6, KeyCode::F7, KeyCode::F8,
            KeyCode::F9, KeyCode::F10, KeyCode::F11, KeyCode::F12,
            KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD,
            KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3,
            KeyCode::Backspace, KeyCode::Tab, KeyCode::Enter, KeyCode::Escape,
            KeyCode::Space, KeyCode::Delete, KeyCode::Insert,
        ];
        all_codes.iter().filter(|c| c.category() == cat).copied().collect()
    }

    pub fn same_category(a: KeyCode, b: KeyCode) -> bool {
        a.category() == b.category()
    }

    pub fn category_name(cat: KeyCategory) -> &'static str {
        match cat {
            KeyCategory::Modifier => "Modifier",
            KeyCategory::Navigation => "Navigation",
            KeyCategory::Function => "Function",
            KeyCategory::Letter => "Letter",
            KeyCategory::Digit => "Digit",
            KeyCategory::Navigation => "Navigation",
            KeyCategory::Numpad => "Numpad",
            KeyCategory::Punctuation => "Punctuation",
            KeyCategory::Media => "Media",
            KeyCategory::Other => "Other",
        }
    }
}

// ---------------------------------------------------------------------------
// ScanCode + ScanCodeMapping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanCode(pub u16);

impl ScanCode {
    pub fn from_key_code(kc: KeyCode) -> Self { ScanCode(kc as u16) }
    pub fn value(self) -> u16 { self.0 }
}

impl std::fmt::Display for ScanCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ScanCode(0x{:04X})", self.0)
    }
}

pub struct ScanCodeMapping {
    entries: Vec<(KeyCode, ScanCode)>,
}

impl ScanCodeMapping {
    pub fn new() -> Self { Self { entries: Vec::new() } }
    pub fn add(&mut self, kc: KeyCode, sc: ScanCode) { self.entries.push((kc, sc)); }
    pub fn to_scan_code(&self, kc: KeyCode) -> Option<ScanCode> {
        self.entries.iter().find(|(k, _)| *k == kc).map(|(_, s)| *s)
    }
    pub fn from_scan_code(&self, sc: ScanCode) -> Option<KeyCode> {
        self.entries.iter().find(|(_, s)| *s == sc).map(|(k, _)| *k)
    }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}

impl Default for ScanCodeMapping {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// KeyStringParser
// ---------------------------------------------------------------------------

pub struct KeyStringParser;

impl KeyStringParser {
    pub fn parse_modifiers(s: &str) -> (bool, bool, bool, bool) {
        let lower = s.to_lowercase();
        let parts: Vec<&str> = lower.split('+').collect();
        let ctrl = parts.iter().any(|p| *p == "ctrl" || *p == "control");
        let shift = parts.iter().any(|p| *p == "shift");
        let alt = parts.iter().any(|p| *p == "alt" || *p == "option");
        let meta = parts.iter().any(|p| *p == "meta" || *p == "cmd" || *p == "command" || *p == "win");
        (ctrl, shift, alt, meta)
    }

    pub fn parse_key(s: &str) -> KeyCode { string_to_key_code(s) }

    pub fn is_valid_key_string(s: &str) -> bool {
        if s.is_empty() { return false; }
        let parts: Vec<&str> = s.split('+').collect();
        !parts.is_empty() && parts.iter().all(|p| !p.is_empty())
    }

    pub fn normalize(s: &str) -> String {
        let parts: Vec<&str> = s.split('+').collect();
        let mut modifiers = Vec::new();
        let mut keys = Vec::new();
        for part in parts {
            let lower = part.to_lowercase();
            match lower.as_str() {
                "ctrl" | "control" => modifiers.push("Ctrl"),
                "shift" => modifiers.push("Shift"),
                "alt" | "option" => modifiers.push("Alt"),
                "meta" | "cmd" | "command" | "win" => modifiers.push("Meta"),
                _ => keys.push(part),
            }
        }
        modifiers.sort();
        let mut result: Vec<&str> = modifiers;
        result.extend(keys);
        result.join("+")
    }
}


// ── Keycode Modifier Combiner ──

/// Represents a set of modifier keys combined into a single value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModifierSet {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl ModifierSet {
    pub const NONE: Self = Self { ctrl: false, shift: false, alt: false, meta: false };

    pub fn new() -> Self {
        Self::NONE
    }

    pub fn with_ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    pub fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }

    pub fn with_alt(mut self) -> Self {
        self.alt = true;
        self
    }

    pub fn with_meta(mut self) -> Self {
        self.meta = true;
        self
    }

    /// Count how many modifiers are active.
    pub fn count(&self) -> u8 {
        self.ctrl as u8 + self.shift as u8 + self.alt as u8 + self.meta as u8
    }

    /// Check if no modifiers are set.
    pub fn is_empty(&self) -> bool {
        !self.ctrl && !self.shift && !self.alt && !self.meta
    }

    /// Convert to a bitmask (ctrl=1, shift=2, alt=4, meta=8).
    pub fn to_bitmask(&self) -> u8 {
        (self.ctrl as u8) | ((self.shift as u8) << 1) | ((self.alt as u8) << 2) | ((self.meta as u8) << 3)
    }

    /// Create from a bitmask.
    pub fn from_bitmask(mask: u8) -> Self {
        Self {
            ctrl: mask & 1 != 0,
            shift: mask & 2 != 0,
            alt: mask & 4 != 0,
            meta: mask & 8 != 0,
        }
    }
}

impl Default for ModifierSet {
    fn default() -> Self {
        Self::NONE
    }
}

/// Combines multiple modifier keys into a combined chord representation.
pub struct KeycodeModifierCombiner;

impl KeycodeModifierCombiner {
    /// Parse a modifier string like "Ctrl+Shift" into a ModifierSet.
    pub fn parse_modifiers(s: &str) -> ModifierSet {
        let mut set = ModifierSet::new();
        for part in s.split('+') {
            match part.trim().to_lowercase().as_str() {
                "ctrl" | "control" => set.ctrl = true,
                "shift" => set.shift = true,
                "alt" | "option" => set.alt = true,
                "meta" | "cmd" | "command" | "win" | "super" => set.meta = true,
                _ => {}
            }
        }
        set
    }

    /// Combine two modifier sets (union).
    pub fn combine(a: ModifierSet, b: ModifierSet) -> ModifierSet {
        ModifierSet {
            ctrl: a.ctrl || b.ctrl,
            shift: a.shift || b.shift,
            alt: a.alt || b.alt,
            meta: a.meta || b.meta,
        }
    }

    /// Intersect two modifier sets.
    pub fn intersect(a: ModifierSet, b: ModifierSet) -> ModifierSet {
        ModifierSet {
            ctrl: a.ctrl && b.ctrl,
            shift: a.shift && b.shift,
            alt: a.alt && b.alt,
            meta: a.meta && b.meta,
        }
    }

    /// Check if set `a` is a subset of set `b`.
    pub fn is_subset(a: ModifierSet, b: ModifierSet) -> bool {
        (!a.ctrl || b.ctrl) && (!a.shift || b.shift) && (!a.alt || b.alt) && (!a.meta || b.meta)
    }

    /// Remove modifiers in `remove` from `base`.
    pub fn subtract(base: ModifierSet, remove: ModifierSet) -> ModifierSet {
        ModifierSet {
            ctrl: base.ctrl && !remove.ctrl,
            shift: base.shift && !remove.shift,
            alt: base.alt && !remove.alt,
            meta: base.meta && !remove.meta,
        }
    }
}

// ── Keycode Display Formatter ──

/// Platform-specific display conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDisplayPlatform {
    Windows,
    Mac,
    Linux,
}

/// Formats key combinations for display in the UI.
pub struct KeycodeDisplayFormatter {
    platform: KeyDisplayPlatform,
    use_symbols: bool,
}

impl KeycodeDisplayFormatter {
    pub fn new(platform: KeyDisplayPlatform) -> Self {
        Self {
            platform,
            use_symbols: matches!(platform, KeyDisplayPlatform::Mac),
        }
    }

    pub fn with_symbols(mut self, use_symbols: bool) -> Self {
        self.use_symbols = use_symbols;
        self
    }

    /// Format a modifier set as a display string.
    pub fn format_modifiers(&self, mods: ModifierSet) -> String {
        let mut parts = Vec::new();
        if self.use_symbols {
            if mods.ctrl { parts.push("⌃"); }
            if mods.alt { parts.push("⌥"); }
            if mods.shift { parts.push("⇧"); }
            if mods.meta { parts.push("⌘"); }
        } else {
            if mods.ctrl { parts.push("Ctrl"); }
            if mods.alt { parts.push("Alt"); }
            if mods.shift { parts.push("Shift"); }
            if mods.meta {
                match self.platform {
                    KeyDisplayPlatform::Windows => parts.push("Win"),
                    KeyDisplayPlatform::Mac => parts.push("Cmd"),
                    KeyDisplayPlatform::Linux => parts.push("Super"),
                };
            }
        }
        if self.use_symbols {
            parts.join("")
        } else {
            parts.join("+")
        }
    }

    /// Format a full key combination (modifiers + key name).
    pub fn format_keybinding(&self, mods: ModifierSet, key_name: &str) -> String {
        let mod_str = self.format_modifiers(mods);
        if mod_str.is_empty() {
            return key_name.to_string();
        }
        if self.use_symbols {
            format!("{}{}", mod_str, key_name)
        } else {
            format!("{}+{}", mod_str, key_name)
        }
    }

    /// Format a chord (sequence of key combinations).
    pub fn format_chord(&self, bindings: &[(ModifierSet, &str)]) -> String {
        bindings
            .iter()
            .map(|(mods, key)| self.format_keybinding(*mods, key))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Get the platform-specific name for a key.
    pub fn platform_key_name<'a>(&self, key_name: &'a str) -> &'a str {
        match (self.platform, key_name) {
            (KeyDisplayPlatform::Mac, "Backspace") => "Delete",
            (KeyDisplayPlatform::Mac, "Delete") => "Fn+Delete",
            (KeyDisplayPlatform::Mac, "Enter") => "Return",
            _ => key_name,
        }
    }

    pub fn platform(&self) -> KeyDisplayPlatform {
        self.platform
    }
}



// ─── KeyBind Builder & Validator ─────────────────────────────

/// Builder for constructing key binding configurations.
#[derive(Debug, Clone)]
pub struct KeyBindBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl KeyBindBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<KeyBindCfg, KeyBindBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(KeyBindBuildErr { errors }); }
        Ok(KeyBindCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated key binding configuration.
#[derive(Debug, Clone)]
pub struct KeyBindCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl KeyBindCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &KeyBindCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for KeyBindCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KeyBindCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct KeyBindBuildErr { pub errors: Vec<String> }

impl fmt::Display for KeyBindBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KeyBindBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for KeyBindBuildErr {}

// ─── KeyBind Formatter ───────────────────────────────────────

/// Formatting options for key code output.
#[derive(Debug, Clone)]
pub struct KeyBindFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for KeyBindFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl KeyBindFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for key code data.
pub struct KeyBindFmt {
    options: KeyBindFmtOpts,
}

impl KeyBindFmt {
    pub fn new(options: KeyBindFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: KeyBindFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}


/// Keycode configuration manager.
#[derive(Debug, Clone)]
pub struct KeycodesConfig {
    entries: Vec<KeycodesEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single keycode entry.
#[derive(Debug, Clone, PartialEq)]
pub struct KeycodesEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl KeycodesEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl KeycodesConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: KeycodesEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&KeycodesEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut KeycodesEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&KeycodesEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&KeycodesEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&KeycodesEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<KeycodesEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for keycodes
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaKeycodesRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaKeycodesRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaKeycodesCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaKeycodesCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaKeycodesCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 105
// ---------------------------------------------------------------------------

/// Generic object pool `Xc105Pool<T>`.
pub struct Xc105Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc105Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc105PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc105Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc105PoolStats {
        Xc105PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc105Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc105Scheduler`.
pub struct Xc105Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc105Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc105Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_105 hash for the given byte slice.
pub fn xc_105_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_105 convention.
pub fn xc_105_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_74 deepening: state machine + event bus ---

/// States for the Xd74 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd74State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd74State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd74Transition {
    pub from: Xd74State,
    pub to: Xd74State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd74StateMachine {
    current: Xd74State,
    history: Vec<Xd74Transition>,
    step_counter: usize,
}

impl Xd74StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd74State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd74State {
        self.current
    }

    pub fn history(&self) -> &[Xd74Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd74State) -> Result<Xd74State, String> {
        let allowed = match (self.current, target) {
            (Xd74State::Idle, Xd74State::Running) => true,
            (Xd74State::Running, Xd74State::Paused) => true,
            (Xd74State::Running, Xd74State::Done) => true,
            (Xd74State::Paused, Xd74State::Running) => true,
            (Xd74State::Paused, Xd74State::Done) => true,
            (Xd74State::Done, Xd74State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_74: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd74Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd74SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd74State> {
        let prefix = "Xd74SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd74State::Idle),
            "Running" => Some(Xd74State::Running),
            "Paused" => Some(Xd74State::Paused),
            "Done" => Some(Xd74State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd74State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd74 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd74Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd74Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd74HandlerFn = Box<dyn Fn(&Xd74Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd74EventBus {
    handlers: Vec<(usize, Option<String>, Xd74HandlerFn)>,
    next_id: usize,
    published: Vec<Xd74Event>,
}

impl Xd74EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd74Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd74Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd74Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd74Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #92
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf92Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf92TrieNode {
    children: std::collections::HashMap<char, Xf92TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf92Trie {
    root: Xf92TrieNode,
    count: usize,
}

impl Xf92Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf92TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf92TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf92TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf92BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf92BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 104).
pub struct Xh104SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh104SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 146 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 104).
pub struct Xh104BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh104BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 104).
pub struct Xi104Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi104Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi104Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi104Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 104).
pub struct Xi104IntervalTree {
    xi_intervals: Vec<Xi104Interval>,
}

impl Xi104IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi104Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi104Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi104Interval) -> Vec<&Xi104Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi104Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi104Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi104Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi104Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi104Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi104Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 104) ---

/// Disjoint set / union-find for crate 104.
pub struct Xj104UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj104UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ104_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 104.
pub struct Xj104BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj104BTreeNode<K, V>>>,
    len: usize,
}

struct Xj104BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj104BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj104BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ104_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ104_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj104BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj104BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj104BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj104BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_104 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk104SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk104SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk104DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk104DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
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

    // -- New tests ----------------------------------------------------------

    #[test]
    fn chord_has_modifier() {
        let chord = KeyCodeChord::new(true, false, false, false, KeyCode::KeyA);
        assert!(chord.has_modifier());
        let plain = KeyCodeChord::just(KeyCode::Enter);
        assert!(!plain.has_modifier());
    }

    #[test]
    fn chord_modifier_count() {
        let chord = KeyCodeChord::new(true, true, true, true, KeyCode::KeyA);
        assert_eq!(chord.modifier_count(), 4);
        let chord2 = KeyCodeChord::new(true, false, true, false, KeyCode::KeyS);
        assert_eq!(chord2.modifier_count(), 2);
        assert_eq!(KeyCodeChord::just(KeyCode::Space).modifier_count(), 0);
    }

    #[test]
    fn chord_is_plain() {
        assert!(KeyCodeChord::just(KeyCode::F5).is_plain());
        assert!(!KeyCodeChord::new(true, false, false, false, KeyCode::KeyC).is_plain());
    }

    #[test]
    fn chord_builder_with_modifiers() {
        let chord = KeyCodeChord::just(KeyCode::KeyS).with_ctrl().with_shift();
        assert!(chord.ctrl);
        assert!(chord.shift);
        assert!(!chord.alt);
        assert!(!chord.meta);
        assert_eq!(chord.key_code, KeyCode::KeyS);
        assert_eq!(chord.to_string(), "Ctrl+Shift+S");
    }

    #[test]
    fn key_combo_is_empty() {
        let empty = KeyCombo::new(vec![]);
        assert!(empty.is_empty());
        let non_empty = KeyCombo::single(KeyCodeChord::just(KeyCode::KeyA));
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn keycode_is_letter() {
        assert!(KeyCode::KeyA.is_letter());
        assert!(KeyCode::KeyZ.is_letter());
        assert!(!KeyCode::Digit0.is_letter());
        assert!(!KeyCode::F1.is_letter());
    }

    #[test]
    fn keycode_is_digit() {
        assert!(KeyCode::Digit0.is_digit());
        assert!(KeyCode::Digit9.is_digit());
        assert!(!KeyCode::KeyA.is_digit());
    }

    #[test]
    fn keycode_is_function_key() {
        assert!(KeyCode::F1.is_function_key());
        assert!(KeyCode::F24.is_function_key());
        assert!(!KeyCode::KeyA.is_function_key());
        assert!(!KeyCode::Escape.is_function_key());
    }

    #[test]
    fn key_category_is_navigation() {
        assert!(KeyCategory::Navigation.is_navigation());
        assert!(!KeyCategory::Letter.is_navigation());
        assert!(!KeyCategory::Modifier.is_navigation());
    }

    #[test]
    fn modifier_mask_from_chord() {
        let chord = KeyCodeChord::just(KeyCode::KeyA).with_ctrl().with_shift();
        let mask = ModifierMask::from_chord(&chord);
        assert!(mask.contains(ModifierMask::CTRL));
        assert!(mask.contains(ModifierMask::SHIFT));
        assert!(!mask.contains(ModifierMask::ALT));
        assert_eq!(mask.count(), 2);
    }

    #[test]
    fn modifier_mask_union_and_intersection() {
        let a = ModifierMask::CTRL.union(ModifierMask::SHIFT);
        let b = ModifierMask::SHIFT.union(ModifierMask::ALT);
        let union = a.union(b);
        assert_eq!(union.count(), 3);
        let inter = a.intersection(b);
        assert!(inter.contains(ModifierMask::SHIFT));
        assert!(!inter.contains(ModifierMask::CTRL));
    }

    #[test]
    fn modifier_mask_display() {
        let mask = ModifierMask::CTRL.union(ModifierMask::ALT);
        let s = format!("{}", mask);
        assert!(s.contains("Ctrl"));
        assert!(s.contains("Alt"));
        assert_eq!(format!("{}", ModifierMask::NONE), "(none)");
    }

    #[test]
    fn key_combo_matches_exact() {
        let combo = KeyCombo::new(vec![
            KeyCodeChord::just(KeyCode::KeyK).with_ctrl(),
            KeyCodeChord::just(KeyCode::KeyC).with_ctrl(),
        ]);
        let input = vec![
            KeyCodeChord::just(KeyCode::KeyK).with_ctrl(),
            KeyCodeChord::just(KeyCode::KeyC).with_ctrl(),
        ];
        assert!(combo.matches(&input));
    }

    #[test]
    fn key_combo_is_prefix() {
        let combo = KeyCombo::single(KeyCodeChord::just(KeyCode::KeyK).with_ctrl());
        let input = vec![
            KeyCodeChord::just(KeyCode::KeyK).with_ctrl(),
            KeyCodeChord::just(KeyCode::KeyC).with_ctrl(),
        ];
        assert!(combo.is_prefix_of(&input));
        assert!(!combo.matches(&input));
    }

    #[test]
    fn key_combo_no_match() {
        let combo = KeyCombo::single(KeyCodeChord::just(KeyCode::KeyA));
        let input = vec![KeyCodeChord::just(KeyCode::KeyB)];
        assert!(!combo.matches(&input));
    }

    #[test]
    fn format_chord_display_text() {
        let chord = KeyCodeChord::just(KeyCode::KeyP).with_ctrl().with_shift();
        let s = format_chord_display(&chord, false);
        assert_eq!(s, "Ctrl+Shift+P");
    }

    #[test]
    fn format_chord_display_symbols() {
        let chord = KeyCodeChord::just(KeyCode::KeyP).with_ctrl().with_shift();
        let s = format_chord_display(&chord, true);
        assert!(s.contains('⌃'));
        assert!(s.contains('⇧'));
        assert!(s.contains('P'));
    }

    #[test]
    fn format_combo_display_multi_chord() {
        let combo = KeyCombo::new(vec![
            KeyCodeChord::just(KeyCode::KeyK).with_ctrl(),
            KeyCodeChord::just(KeyCode::KeyC).with_ctrl(),
        ]);
        let s = format_combo_display(&combo, false);
        assert!(s.contains("Ctrl+K"));
        assert!(s.contains("Ctrl+C"));
    }

    #[test]
    fn format_chord_plain_key() {
        let chord = KeyCodeChord::just(KeyCode::Escape);
        let s = format_chord_display(&chord, false);
        assert_eq!(s, "Escape");
    }

    #[test]
    fn chord_is_ctrl_only() {
        let chord = KeyCodeChord::new(true, false, false, false, KeyCode::KeyC);
        assert!(chord.is_ctrl_only());
        let chord2 = KeyCodeChord::new(true, true, false, false, KeyCode::KeyC);
        assert!(!chord2.is_ctrl_only());
    }

    #[test]
    fn chord_is_ctrl_shift() {
        let chord = KeyCodeChord::new(true, true, false, false, KeyCode::KeyP);
        assert!(chord.is_ctrl_shift());
        let plain = KeyCodeChord::just(KeyCode::KeyP);
        assert!(!plain.is_ctrl_shift());
    }

    #[test]
    fn chord_with_key_changes_key() {
        let chord = KeyCodeChord::new(true, false, false, false, KeyCode::KeyC);
        let new_chord = chord.with_key(KeyCode::KeyV);
        assert_eq!(new_chord.key_code, KeyCode::KeyV);
        assert!(new_chord.ctrl);
    }

    #[test]
    fn chord_same_modifiers() {
        let a = KeyCodeChord::new(true, true, false, false, KeyCode::KeyA);
        let b = KeyCodeChord::new(true, true, false, false, KeyCode::KeyB);
        assert!(a.same_modifiers(&b));
        let c = KeyCodeChord::just(KeyCode::KeyA);
        assert!(!a.same_modifiers(&c));
    }

    #[test]
    fn combo_last_and_contains_key() {
        let combo = KeyCombo::new(vec![
            KeyCodeChord::just(KeyCode::KeyA),
            KeyCodeChord::just(KeyCode::KeyB),
        ]);
        assert_eq!(combo.last().unwrap().key_code, KeyCode::KeyB);
        assert!(combo.contains_key(KeyCode::KeyA));
        assert!(!combo.contains_key(KeyCode::KeyC));
    }

    #[test]
    fn combo_total_modifier_count() {
        let combo = KeyCombo::new(vec![
            KeyCodeChord::new(true, true, false, false, KeyCode::KeyA),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyB),
        ]);
        assert_eq!(combo.total_modifier_count(), 3);
    }

    #[test]
    fn unique_key_codes_deduplicates() {
        let chords = vec![
            KeyCodeChord::just(KeyCode::KeyA),
            KeyCodeChord::just(KeyCode::KeyB),
            KeyCodeChord::just(KeyCode::KeyA),
        ];
        let codes = unique_key_codes(&chords);
        assert_eq!(codes.len(), 2);
    }

    #[test]
    fn is_whitespace_key_check() {
        assert!(is_whitespace_key(KeyCode::Space));
        assert!(is_whitespace_key(KeyCode::Tab));
        assert!(is_whitespace_key(KeyCode::Enter));
        assert!(!is_whitespace_key(KeyCode::KeyA));
    }

    #[test]
    fn count_chords_with_ctrl_counts() {
        let chords = vec![
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyA),
            KeyCodeChord::just(KeyCode::KeyB),
            KeyCodeChord::new(true, true, false, false, KeyCode::KeyC),
        ];
        assert_eq!(count_chords_with_ctrl(&chords), 2);
    }

    // -- KeyCode char/function conversions ----------------------------------

    #[test]
    fn keycode_to_char_letters() {
        assert_eq!(KeyCode::KeyA.to_char(), Some('a'));
        assert_eq!(KeyCode::KeyZ.to_char(), Some('z'));
        assert_eq!(KeyCode::KeyM.to_char(), Some('m'));
    }

    #[test]
    fn keycode_to_char_digits() {
        assert_eq!(KeyCode::Digit0.to_char(), Some('0'));
        assert_eq!(KeyCode::Digit9.to_char(), Some('9'));
    }

    #[test]
    fn keycode_to_char_non_printable_returns_none() {
        assert_eq!(KeyCode::Ctrl.to_char(), None);
        assert_eq!(KeyCode::F1.to_char(), None);
        assert_eq!(KeyCode::LeftArrow.to_char(), None);
    }

    #[test]
    fn keycode_from_char_roundtrip() {
        for ch in 'a'..='z' {
            let kc = KeyCode::from_char(ch).unwrap();
            assert_eq!(kc.to_char(), Some(ch));
        }
        for ch in 'A'..='Z' {
            let kc = KeyCode::from_char(ch).unwrap();
            assert_eq!(kc.to_char(), Some(ch.to_ascii_lowercase()));
        }
        for ch in '0'..='9' {
            let kc = KeyCode::from_char(ch).unwrap();
            assert_eq!(kc.to_char(), Some(ch));
        }
        assert_eq!(KeyCode::from_char('!'), None);
        assert_eq!(KeyCode::from_char(' '), None);
    }

    #[test]
    fn function_key_number_roundtrip() {
        for n in 1..=24u8 {
            let kc = KeyCode::from_function_key(n).unwrap();
            assert_eq!(kc.function_key_number(), Some(n));
        }
        assert_eq!(KeyCode::from_function_key(0), None);
        assert_eq!(KeyCode::from_function_key(25), None);
        assert_eq!(KeyCode::KeyA.function_key_number(), None);
    }

    #[test]
    fn numpad_digit_values() {
        assert_eq!(KeyCode::Numpad0.numpad_digit(), Some(0));
        assert_eq!(KeyCode::Numpad9.numpad_digit(), Some(9));
        assert_eq!(KeyCode::NumpadAdd.numpad_digit(), None);
        assert_eq!(KeyCode::KeyA.numpad_digit(), None);
    }

    // -- KeyCodeChord KeyMod encoding ---------------------------------------

    #[test]
    fn chord_keymod_value_roundtrip() {
        let chord = KeyCodeChord::new(true, true, false, false, KeyCode::KeyS);
        let val = chord.to_keymod_value();
        let decoded = KeyCodeChord::from_keymod_value(val);
        assert_eq!(decoded, chord);
    }

    #[test]
    fn chord_strip_modifiers() {
        let chord = KeyCodeChord::new(true, true, true, true, KeyCode::KeyP);
        let stripped = chord.strip_modifiers();
        assert!(stripped.is_plain());
        assert_eq!(stripped.key_code, KeyCode::KeyP);
    }

    // -- Batch encode/decode ------------------------------------------------

    #[test]
    fn encode_decode_chords_batch() {
        let chords = vec![
            KeyCodeChord::just(KeyCode::KeyA).with_ctrl(),
            KeyCodeChord::just(KeyCode::KeyB).with_shift(),
            KeyCodeChord::just(KeyCode::Escape),
        ];
        let encoded = encode_chords(&chords);
        assert_eq!(encoded.len(), 3);
        let decoded = decode_chords(&encoded);
        assert_eq!(decoded, chords);
    }

    #[test]
    fn split_key_chord_roundtrip() {
        let first = encode_chord(&KeyCodeChord::just(KeyCode::KeyK).with_ctrl());
        let second = encode_chord(&KeyCodeChord::just(KeyCode::KeyC).with_ctrl());
        let combined = key_chord(first, second);
        let (f, s) = split_key_chord(combined);
        assert_eq!(f, first);
        assert_eq!(s, second);
    }

    #[test]
    fn split_key_chord_single() {
        let first = encode_chord(&KeyCodeChord::just(KeyCode::F5));
        let (f, s) = split_key_chord(first);
        assert_eq!(f, first);
        assert_eq!(s, 0);
    }

    // -- KeyCombo encoding / builder ----------------------------------------

    #[test]
    fn key_combo_encode_decode_roundtrip() {
        let combo = KeyCombo::new(vec![
            KeyCodeChord::just(KeyCode::KeyK).with_ctrl(),
            KeyCodeChord::just(KeyCode::KeyC).with_ctrl(),
        ]);
        let encoded = combo.encode();
        let decoded = KeyCombo::decode(&encoded);
        assert_eq!(decoded, combo);
    }

    #[test]
    fn key_combo_then_builder() {
        let combo = KeyCombo::single(KeyCodeChord::just(KeyCode::KeyK).with_ctrl())
            .then(KeyCodeChord::just(KeyCode::KeyC).with_ctrl());
        assert_eq!(combo.len(), 2);
        assert_eq!(combo.chords[1].key_code, KeyCode::KeyC);
    }

    #[test]
    fn key_combo_is_all_plain() {
        let plain = KeyCombo::new(vec![
            KeyCodeChord::just(KeyCode::KeyA),
            KeyCodeChord::just(KeyCode::KeyB),
        ]);
        assert!(plain.is_all_plain());

        let modified = KeyCombo::new(vec![
            KeyCodeChord::just(KeyCode::KeyA),
            KeyCodeChord::just(KeyCode::KeyB).with_ctrl(),
        ]);
        assert!(!modified.is_all_plain());
    }

    #[test]
    fn key_combo_push() {
        let mut combo = KeyCombo::single(KeyCodeChord::just(KeyCode::KeyA));
        combo.push(KeyCodeChord::just(KeyCode::KeyB));
        assert_eq!(combo.len(), 2);
    }


    #[test]
    fn key_code_label_common() {
        assert_eq!(KeyCodeLabel::label(KeyCode::Backspace), "Backspace");
        assert_eq!(KeyCodeLabel::label(KeyCode::Escape), "Esc");
        assert_eq!(KeyCodeLabel::label(KeyCode::Enter), "Enter");
    }

    #[test]
    fn key_code_label_modifier_symbol() {
        assert_eq!(KeyCodeLabel::modifier_symbol(KeyCode::Ctrl), Some('^'));
        assert_eq!(KeyCodeLabel::modifier_symbol(KeyCode::KeyA), None);
    }

    #[test]
    fn category_map_codes() {
        let mods = KeyCodeCategoryMap::codes_for_category(KeyCategory::Modifier);
        assert!(mods.contains(&KeyCode::Ctrl));
    }

    #[test]
    fn category_map_same() {
        assert!(KeyCodeCategoryMap::same_category(KeyCode::KeyA, KeyCode::KeyB));
        assert!(!KeyCodeCategoryMap::same_category(KeyCode::KeyA, KeyCode::Ctrl));
    }

    #[test]
    fn category_map_name() {
        assert_eq!(KeyCodeCategoryMap::category_name(KeyCategory::Letter), "Letter");
    }

    #[test]
    fn scan_code_from_key() {
        let sc = ScanCode::from_key_code(KeyCode::KeyA);
        assert!(sc.value() > 0);
    }

    #[test]
    fn scan_code_display() {
        let sc = ScanCode(0x1E);
        assert!(format!("{sc}").contains("001E"));
    }

    #[test]
    fn scan_code_mapping_basic() {
        let mut m = ScanCodeMapping::new();
        m.add(KeyCode::KeyA, ScanCode(30));
        assert_eq!(m.to_scan_code(KeyCode::KeyA), Some(ScanCode(30)));
        assert_eq!(m.from_scan_code(ScanCode(30)), Some(KeyCode::KeyA));
    }

    #[test]
    fn key_string_parser_modifiers() {
        let (ctrl, shift, alt, meta) = KeyStringParser::parse_modifiers("ctrl+shift+a");
        assert!(ctrl && shift && !alt && !meta);
    }

    #[test]
    fn key_string_parser_valid() {
        assert!(KeyStringParser::is_valid_key_string("ctrl+a"));
        assert!(!KeyStringParser::is_valid_key_string(""));
    }

    #[test]
    fn key_string_parser_normalize() {
        assert_eq!(KeyStringParser::normalize("shift+ctrl+a"), "Ctrl+Shift+a");
    }

    #[test]
    fn key_string_parser_parse_key() {
        assert_eq!(KeyStringParser::parse_key("a"), KeyCode::KeyA);
    }


    #[test]
    fn modifier_set_new_empty() {
        let m = ModifierSet::new();
        assert!(m.is_empty());
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn modifier_set_builders() {
        let m = ModifierSet::new().with_ctrl().with_shift();
        assert!(m.ctrl);
        assert!(m.shift);
        assert!(!m.alt);
        assert_eq!(m.count(), 2);
    }

    #[test]
    fn modifier_set_bitmask_roundtrip() {
        let m = ModifierSet::new().with_ctrl().with_alt().with_meta();
        let mask = m.to_bitmask();
        let m2 = ModifierSet::from_bitmask(mask);
        assert_eq!(m, m2);
    }

    #[test]
    fn combiner_parse_modifiers() {
        let m = KeycodeModifierCombiner::parse_modifiers("Ctrl+Shift");
        assert!(m.ctrl);
        assert!(m.shift);
        assert!(!m.alt);
    }

    #[test]
    fn combiner_parse_mac_modifiers() {
        let m = KeycodeModifierCombiner::parse_modifiers("Cmd+Option");
        assert!(m.meta);
        assert!(m.alt);
    }

    #[test]
    fn combiner_combine() {
        let a = ModifierSet::new().with_ctrl();
        let b = ModifierSet::new().with_shift();
        let c = KeycodeModifierCombiner::combine(a, b);
        assert!(c.ctrl && c.shift);
    }

    #[test]
    fn combiner_intersect() {
        let a = ModifierSet::new().with_ctrl().with_shift();
        let b = ModifierSet::new().with_ctrl().with_alt();
        let c = KeycodeModifierCombiner::intersect(a, b);
        assert!(c.ctrl);
        assert!(!c.shift);
        assert!(!c.alt);
    }

    #[test]
    fn combiner_subset() {
        let a = ModifierSet::new().with_ctrl();
        let b = ModifierSet::new().with_ctrl().with_shift();
        assert!(KeycodeModifierCombiner::is_subset(a, b));
        assert!(!KeycodeModifierCombiner::is_subset(b, a));
    }

    #[test]
    fn formatter_windows_keybinding() {
        let fmt = KeycodeDisplayFormatter::new(KeyDisplayPlatform::Windows);
        let mods = ModifierSet::new().with_ctrl().with_shift();
        let result = fmt.format_keybinding(mods, "A");
        assert_eq!(result, "Ctrl+Shift+A");
    }

    #[test]
    fn formatter_mac_symbols() {
        let fmt = KeycodeDisplayFormatter::new(KeyDisplayPlatform::Mac);
        let mods = ModifierSet::new().with_ctrl().with_meta();
        let result = fmt.format_keybinding(mods, "A");
        assert_eq!(result, "⌃⌘A");
    }

    #[test]
    fn formatter_chord() {
        let fmt = KeycodeDisplayFormatter::new(KeyDisplayPlatform::Linux);
        let bindings = vec![
            (ModifierSet::new().with_ctrl(), "K"),
            (ModifierSet::new().with_ctrl(), "C"),
        ];
        let result = fmt.format_chord(&bindings);
        assert_eq!(result, "Ctrl+K Ctrl+C");
    }

    #[test]
    fn formatter_platform_key_name_mac() {
        let fmt = KeycodeDisplayFormatter::new(KeyDisplayPlatform::Mac);
        assert_eq!(fmt.platform_key_name("Backspace"), "Delete");
        assert_eq!(fmt.platform_key_name("Enter"), "Return");
    }



    #[test]
    fn keybind_builder_valid() {
        let cfg = KeyBindBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn keybind_builder_empty_name() {
        let r = KeyBindBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn keybind_builder_bad_priority() {
        assert!(KeyBindBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn keybind_builder_zero_max() {
        assert!(KeyBindBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn keybind_cfg_merge() {
        let mut a = KeyBindBuilder::new("a").property("x", "1").build().unwrap();
        let b = KeyBindBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn keybind_cfg_display() {
        let cfg = KeyBindBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }

    #[test]
    fn keybind_fmt_list() {
        let f = KeyBindFmt::new(KeyBindFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn keybind_fmt_kv() {
        let f = KeyBindFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn keybind_fmt_section() {
        let f = KeyBindFmt::new(KeyBindFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn keybind_fmt_truncate() {
        let f = KeyBindFmt::new(KeyBindFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn keybind_fmt_opts_defaults() {
        let o = KeyBindFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn keycodes_entry_creation() {
        let e = KeycodesEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn keycodes_entry_with_priority() {
        let e = KeycodesEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn keycodes_entry_metadata() {
        let e = KeycodesEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn keycodes_entry_remove_meta() {
        let mut e = KeycodesEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn keycodes_entry_activate_deactivate() {
        let mut e = KeycodesEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn keycodes_config_add_sorted() {
        let mut c = KeycodesConfig::new(10);
        c.add(KeycodesEntry::new("lo", "Lo").with_priority(1));
        c.add(KeycodesEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn keycodes_config_capacity() {
        let mut c = KeycodesConfig::new(1);
        assert!(c.add(KeycodesEntry::new("a", "A")));
        assert!(!c.add(KeycodesEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn keycodes_config_remove() {
        let mut c = KeycodesConfig::new(10);
        c.add(KeycodesEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn keycodes_config_get() {
        let mut c = KeycodesConfig::new(10);
        c.add(KeycodesEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn keycodes_config_active_entries() {
        let mut c = KeycodesConfig::new(10);
        c.add(KeycodesEntry::new("a", "A"));
        c.add(KeycodesEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn keycodes_config_enable_disable() {
        let mut c = KeycodesConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn keycodes_config_clear() {
        let mut c = KeycodesConfig::new(10);
        c.add(KeycodesEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn keycodes_config_find_by_label() {
        let mut c = KeycodesConfig::new(10);
        c.add(KeycodesEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn keycodes_config_top_n() {
        let mut c = KeycodesConfig::new(10);
        c.add(KeycodesEntry::new("a", "A").with_priority(1));
        c.add(KeycodesEntry::new("b", "B").with_priority(2));
        c.add(KeycodesEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn keycodes_config_deactivate_activate_all() {
        let mut c = KeycodesConfig::new(10);
        c.add(KeycodesEntry::new("a", "A"));
        c.add(KeycodesEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn keycodes_config_highest_priority() {
        let mut c = KeycodesConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(KeycodesEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn keycodes_config_contains() {
        let mut c = KeycodesConfig::new(10);
        c.add(KeycodesEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn keycodes_config_labels() {
        let mut c = KeycodesConfig::new(10);
        c.add(KeycodesEntry::new("a", "Alpha"));
        c.add(KeycodesEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn keycodes_config_drain_inactive() {
        let mut c = KeycodesConfig::new(10);
        c.add(KeycodesEntry::new("a", "A"));
        c.add(KeycodesEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for keycodes
    #[test]
    fn xa_keycodes_ring_new() {
        let rb = super::XaKeycodesRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_keycodes_ring_push_len() {
        let mut rb = super::XaKeycodesRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_keycodes_ring_wrap() {
        let mut rb = super::XaKeycodesRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_keycodes_ring_mean_empty() {
        let rb = super::XaKeycodesRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_keycodes_ring_mean_values() {
        let mut rb = super::XaKeycodesRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_keycodes_ring_min_max() {
        let mut rb = super::XaKeycodesRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_keycodes_ring_iter() {
        let mut rb = super::XaKeycodesRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_keycodes_counter_new() {
        let c = super::XaKeycodesCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_keycodes_counter_inc() {
        let mut c = super::XaKeycodesCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_keycodes_counter_inc_by() {
        let mut c = super::XaKeycodesCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_keycodes_counter_reset() {
        let mut c = super::XaKeycodesCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_keycodes_counter_clear() {
        let mut c = super::XaKeycodesCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_keycodes_counter_default() {
        let c = super::XaKeycodesCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 105 ----

    #[test]
    fn xc_105_pool_new_empty() {
        let pool: super::Xc105Pool<i32> = super::Xc105Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_105_pool_release_acquire() {
        let mut pool = super::Xc105Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_105_pool_acquire_empty() {
        let mut pool: super::Xc105Pool<i32> = super::Xc105Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_105_pool_full() {
        let mut pool = super::Xc105Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_105_pool_drain() {
        let mut pool = super::Xc105Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_105_pool_stats() {
        let mut pool = super::Xc105Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_105_pool_clear() {
        let mut pool = super::Xc105Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_105_pool_shrink() {
        let mut pool = super::Xc105Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_105_pool_default() {
        let pool: super::Xc105Pool<String> = super::Xc105Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_105_pool_extend() {
        let mut pool = super::Xc105Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_105_pool_retain() {
        let mut pool = super::Xc105Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_105_scheduler_round_robin() {
        let mut sched = super::Xc105Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_105_scheduler_empty() {
        let mut sched = super::Xc105Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_105_scheduler_reset() {
        let mut sched = super::Xc105Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_105_scheduler_add_remove() {
        let mut sched = super::Xc105Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_105_scheduler_targets() {
        let sched = super::Xc105Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_105_hash_empty() {
        assert_eq!(super::xc_105_hash(b""), 5381);
    }

    #[test]
    fn xc_105_hash_data() {
        let h = super::xc_105_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_105_hash(b"hello"), h);
    }

    #[test]
    fn xc_105_reverse_str() {
        assert_eq!(super::xc_105_reverse("abc"), "cba");
        assert_eq!(super::xc_105_reverse(""), "");
    }


    // --- xd_74 deepening tests ---

    #[test]
    fn xd_74_sm_initial_state() {
        let sm = Xd74StateMachine::new();
        assert_eq!(sm.current_state(), Xd74State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_74_sm_valid_idle_to_running() {
        let mut sm = Xd74StateMachine::new();
        assert!(sm.transition(Xd74State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd74State::Running);
    }

    #[test]
    fn xd_74_sm_valid_running_to_paused() {
        let mut sm = Xd74StateMachine::new();
        sm.transition(Xd74State::Running).unwrap();
        assert!(sm.transition(Xd74State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd74State::Paused);
    }

    #[test]
    fn xd_74_sm_valid_running_to_done() {
        let mut sm = Xd74StateMachine::new();
        sm.transition(Xd74State::Running).unwrap();
        assert!(sm.transition(Xd74State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd74State::Done);
    }

    #[test]
    fn xd_74_sm_valid_paused_to_running() {
        let mut sm = Xd74StateMachine::new();
        sm.transition(Xd74State::Running).unwrap();
        sm.transition(Xd74State::Paused).unwrap();
        assert!(sm.transition(Xd74State::Running).is_ok());
    }

    #[test]
    fn xd_74_sm_valid_done_to_idle() {
        let mut sm = Xd74StateMachine::new();
        sm.transition(Xd74State::Running).unwrap();
        sm.transition(Xd74State::Done).unwrap();
        assert!(sm.transition(Xd74State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd74State::Idle);
    }

    #[test]
    fn xd_74_sm_invalid_idle_to_done() {
        let mut sm = Xd74StateMachine::new();
        assert!(sm.transition(Xd74State::Done).is_err());
    }

    #[test]
    fn xd_74_sm_invalid_idle_to_paused() {
        let mut sm = Xd74StateMachine::new();
        assert!(sm.transition(Xd74State::Paused).is_err());
    }

    #[test]
    fn xd_74_sm_history_tracking() {
        let mut sm = Xd74StateMachine::new();
        sm.transition(Xd74State::Running).unwrap();
        sm.transition(Xd74State::Paused).unwrap();
        sm.transition(Xd74State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd74State::Idle);
        assert_eq!(sm.history()[0].to, Xd74State::Running);
        assert_eq!(sm.history()[1].from, Xd74State::Running);
        assert_eq!(sm.history()[2].to, Xd74State::Done);
    }

    #[test]
    fn xd_74_sm_serialize_deserialize() {
        let mut sm = Xd74StateMachine::new();
        sm.transition(Xd74State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd74StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd74State::Running));
    }

    #[test]
    fn xd_74_sm_deserialize_invalid() {
        assert_eq!(Xd74StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_74_sm_reset() {
        let mut sm = Xd74StateMachine::new();
        sm.transition(Xd74State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd74State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_74_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd74EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd74Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_74_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd74EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd74Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd74Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_74_bus_unsubscribe() {
        let mut bus = Xd74EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_74_event_kind_and_payload() {
        let e = Xd74Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd74Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_74_bus_clear_history() {
        let mut bus = Xd74EventBus::new();
        bus.publish(Xd74Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_74_sm_step_counter_increments() {
        let mut sm = Xd74StateMachine::new();
        sm.transition(Xd74State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd74State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #92 --

    #[test]
    fn xf92_trie_insert_search() {
        let mut t = Xf92Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf92_trie_starts_with() {
        let mut t = Xf92Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf92_trie_remove() {
        let mut t = Xf92Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf92_trie_word_count() {
        let mut t = Xf92Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf92_trie_longest_prefix() {
        let mut t = Xf92Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf92_trie_all_words() {
        let mut t = Xf92Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf92_trie_autocomplete() {
        let mut t = Xf92Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf92_trie_empty_search() {
        let t = Xf92Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf92_bloom_add_contains() {
        let mut bf = Xf92BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf92_bloom_probably_absent() {
        let bf = Xf92BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf92_bloom_false_positive_rate() {
        let mut bf = Xf92BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf92_bloom_clear() {
        let mut bf = Xf92BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf92_bloom_union() {
        let mut a = Xf92BloomFilter::xf_new(512, 2);
        let mut b = Xf92BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf92_bloom_intersection_estimate() {
        let mut a = Xf92BloomFilter::xf_new(512, 2);
        let mut b = Xf92BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf92_bloom_union_size_mismatch() {
        let a = Xf92BloomFilter::xf_new(256, 2);
        let b = Xf92BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh104_skip_insert_contains() {
        let mut sl = super::Xh104SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh104_skip_remove() {
        let mut sl = super::Xh104SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh104_skip_len() {
        let mut sl = super::Xh104SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh104_skip_range_query() {
        let mut sl = super::Xh104SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh104_skip_floor_ceiling() {
        let mut sl = super::Xh104SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh104_skip_rank() {
        let mut sl = super::Xh104SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh104_skip_empty() {
        let sl = super::Xh104SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh104_skip_duplicates() {
        let mut sl = super::Xh104SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh104_bitset_set_test() {
        let mut bs = super::Xh104BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh104_bitset_clear_count() {
        let mut bs = super::Xh104BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh104_bitset_and_or_xor() {
        let mut a = super::Xh104BitSet::xh_new(128);
        let mut b = super::Xh104BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh104_bitset_iter_ones() {
        let mut bs = super::Xh104BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh104_bitset_first_last() {
        let mut bs = super::Xh104BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh104_bitset_empty() {
        let bs = super::Xh104BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi104_deque_push_pop_back() {
        let mut dq = super::Xi104Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi104_deque_push_pop_front() {
        let mut dq = super::Xi104Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi104_deque_mixed_ops() {
        let mut dq = super::Xi104Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi104_deque_get_and_split() {
        let mut dq = super::Xi104Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi104_deque_rotate_left() {
        let mut dq = super::Xi104Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi104_deque_rotate_right() {
        let mut dq = super::Xi104Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi104_deque_grow() {
        let mut dq = super::Xi104Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi104_deque_empty() {
        let dq = super::Xi104Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi104_interval_tree_insert_query() {
        let mut tree = super::Xi104IntervalTree::xi_new();
        tree.xi_insert(super::Xi104Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi104Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi104Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi104_interval_tree_overlap() {
        let mut tree = super::Xi104IntervalTree::xi_new();
        tree.xi_insert(super::Xi104Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi104Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi104Interval::xi_new(12, 20));
        let q = super::Xi104Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi104_interval_tree_remove() {
        let mut tree = super::Xi104IntervalTree::xi_new();
        tree.xi_insert(super::Xi104Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi104Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi104_interval_tree_gaps() {
        let mut tree = super::Xi104IntervalTree::xi_new();
        tree.xi_insert(super::Xi104Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi104Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi104Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi104Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi104Interval::xi_new(8, 10));
    }

    #[test]
    fn xi104_interval_tree_merge() {
        let mut tree = super::Xi104IntervalTree::xi_new();
        tree.xi_insert(super::Xi104Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi104Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi104Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi104Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi104Interval::xi_new(10, 15));
    }

    #[test]
    fn xi104_interval_tree_all() {
        let mut tree = super::Xi104IntervalTree::xi_new();
        tree.xi_insert(super::Xi104Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi104Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi104_interval_tree_empty() {
        let tree = super::Xi104IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi104_interval_tree_contains_point() {
        let iv = super::Xi104Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 104) ---

    #[test]
    fn xj_104_uf_make_and_find() {
        let mut uf = super::Xj104UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_104_uf_union_connected() {
        let mut uf = super::Xj104UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_104_uf_component_count() {
        let mut uf = super::Xj104UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_104_uf_component_size() {
        let mut uf = super::Xj104UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_104_uf_largest_component() {
        let mut uf = super::Xj104UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_104_uf_many_elements() {
        let mut uf = super::Xj104UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_104_uf_separate_components() {
        let mut uf = super::Xj104UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_104_uf_path_compression() {
        let mut uf = super::Xj104UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_104_bt_insert_get() {
        let mut bt = super::Xj104BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_104_bt_contains_len() {
        let mut bt = super::Xj104BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_104_bt_replace() {
        let mut bt = super::Xj104BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_104_bt_remove() {
        let mut bt = super::Xj104BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_104_bt_keys_values() {
        let mut bt = super::Xj104BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_104_bt_range() {
        let mut bt = super::Xj104BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_104_bt_min_max() {
        let mut bt = super::Xj104BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_104_bt_many_inserts() {
        let mut bt = super::Xj104BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_104 segment tree tests ---

    #[test]
    fn xk_104_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk104SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_104_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk104SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_104_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk104SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_104_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk104SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_104_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk104SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_104_st_single_element() {
        let data = vec![42];
        let st = super::Xk104SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_104_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk104SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_104_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk104SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_104 disjoint intervals tests ---

    #[test]
    fn xk_104_di_add_and_count() {
        let mut di = super::Xk104DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_104_di_merge_overlap() {
        let mut di = super::Xk104DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_104_di_contains() {
        let mut di = super::Xk104DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_104_di_remove() {
        let mut di = super::Xk104DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_104_di_covered_length() {
        let mut di = super::Xk104DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_104_di_gaps() {
        let mut di = super::Xk104DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_104_di_merge_adjacent() {
        let mut di = super::Xk104DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_104_di_empty() {
        let di = super::Xk104DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}