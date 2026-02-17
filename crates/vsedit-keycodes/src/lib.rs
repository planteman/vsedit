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


}
