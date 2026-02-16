//! Terminal input event dispatch.
//!
//! Converts crossterm events into VS Code-compatible key/mouse events and
//! routes them through an [`InputDispatcher`] backed by [`vsedit_events`]
//! emitters.

use std::fmt;
use vsedit_events::{Emitter, Event};
use vsedit_keycodes::{KeyChordParser, KeyCode, KeyCodeChord};

// ---------------------------------------------------------------------------
// MouseButton / MouseAction
// ---------------------------------------------------------------------------

/// Which mouse button was involved in an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    None,
}

/// What kind of mouse action occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseAction {
    Down,
    Up,
    Drag,
    ScrollUp,
    ScrollDown,
    Move,
}

// ---------------------------------------------------------------------------
// KeyInput
// ---------------------------------------------------------------------------

/// A key press with modifier state, using VS Code-compatible key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyInput {
    pub key_code: KeyCode,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

// ---------------------------------------------------------------------------
// MouseInput
// ---------------------------------------------------------------------------

/// A mouse event with position and modifier state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseInput {
    pub action: MouseAction,
    pub button: MouseButton,
    pub column: u16,
    pub row: u16,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

// ---------------------------------------------------------------------------
// InputEvent
// ---------------------------------------------------------------------------

/// Unified input events produced from crossterm's raw terminal events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Key(KeyInput),
    Mouse(MouseInput),
    Paste(String),
    Resize { width: u16, height: u16 },
}

// ---------------------------------------------------------------------------
// Crossterm → vsedit conversion: keys
// ---------------------------------------------------------------------------

/// Map a crossterm [`KeyEvent`](crossterm::event::KeyEvent) to a [`KeyInput`].
pub fn from_crossterm_key(key: crossterm::event::KeyEvent) -> KeyInput {
    use crossterm::event::{KeyCode as CtKey, KeyModifiers};

    let mods = key.modifiers;
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let shift = mods.contains(KeyModifiers::SHIFT);
    let alt = mods.contains(KeyModifiers::ALT);
    let meta = mods.contains(KeyModifiers::SUPER);

    let key_code = match key.code {
        CtKey::Backspace => KeyCode::Backspace,
        CtKey::Enter => KeyCode::Enter,
        CtKey::Left => KeyCode::LeftArrow,
        CtKey::Right => KeyCode::RightArrow,
        CtKey::Up => KeyCode::UpArrow,
        CtKey::Down => KeyCode::DownArrow,
        CtKey::Home => KeyCode::Home,
        CtKey::End => KeyCode::End,
        CtKey::PageUp => KeyCode::PageUp,
        CtKey::PageDown => KeyCode::PageDown,
        CtKey::Tab => KeyCode::Tab,
        CtKey::BackTab => KeyCode::Tab, // Shift+Tab
        CtKey::Delete => KeyCode::Delete,
        CtKey::Insert => KeyCode::Insert,
        CtKey::Esc => KeyCode::Escape,
        CtKey::CapsLock => KeyCode::CapsLock,
        CtKey::ScrollLock => KeyCode::ScrollLock,
        CtKey::NumLock => KeyCode::NumLock,
        CtKey::Pause => KeyCode::PauseBreak,
        CtKey::Menu => KeyCode::ContextMenu,
        CtKey::F(1) => KeyCode::F1,
        CtKey::F(2) => KeyCode::F2,
        CtKey::F(3) => KeyCode::F3,
        CtKey::F(4) => KeyCode::F4,
        CtKey::F(5) => KeyCode::F5,
        CtKey::F(6) => KeyCode::F6,
        CtKey::F(7) => KeyCode::F7,
        CtKey::F(8) => KeyCode::F8,
        CtKey::F(9) => KeyCode::F9,
        CtKey::F(10) => KeyCode::F10,
        CtKey::F(11) => KeyCode::F11,
        CtKey::F(12) => KeyCode::F12,
        CtKey::F(13) => KeyCode::F13,
        CtKey::F(14) => KeyCode::F14,
        CtKey::F(15) => KeyCode::F15,
        CtKey::F(16) => KeyCode::F16,
        CtKey::F(17) => KeyCode::F17,
        CtKey::F(18) => KeyCode::F18,
        CtKey::F(19) => KeyCode::F19,
        CtKey::F(20) => KeyCode::F20,
        CtKey::F(21) => KeyCode::F21,
        CtKey::F(22) => KeyCode::F22,
        CtKey::F(23) => KeyCode::F23,
        CtKey::F(24) => KeyCode::F24,
        CtKey::Char(' ') => KeyCode::Space,
        CtKey::Char(c @ 'a'..='z') => {
            // KeyA = 31, offset from 'a'
            KeyCode::from_u16(KeyCode::KeyA as u16 + (c as u16 - b'a' as u16))
        }
        CtKey::Char(c @ 'A'..='Z') => {
            KeyCode::from_u16(KeyCode::KeyA as u16 + (c as u16 - b'A' as u16))
        }
        CtKey::Char(c @ '0'..='9') => {
            KeyCode::from_u16(KeyCode::Digit0 as u16 + (c as u16 - b'0' as u16))
        }
        CtKey::Char(';') => KeyCode::Semicolon,
        CtKey::Char('=') => KeyCode::Equal,
        CtKey::Char(',') => KeyCode::Comma,
        CtKey::Char('-') => KeyCode::Minus,
        CtKey::Char('.') => KeyCode::Period,
        CtKey::Char('/') => KeyCode::Slash,
        CtKey::Char('`') => KeyCode::Backquote,
        CtKey::Char('[') => KeyCode::BracketLeft,
        CtKey::Char('\\') => KeyCode::Backslash,
        CtKey::Char(']') => KeyCode::BracketRight,
        CtKey::Char('\'') => KeyCode::Quote,
        _ => KeyCode::Unknown,
    };

    KeyInput {
        key_code,
        ctrl,
        shift,
        alt,
        meta,
    }
}

// ---------------------------------------------------------------------------
// Crossterm → vsedit conversion: mouse
// ---------------------------------------------------------------------------

/// Map a crossterm [`MouseEvent`](crossterm::event::MouseEvent) to a [`MouseInput`].
pub fn from_crossterm_mouse(mouse: crossterm::event::MouseEvent) -> MouseInput {
    use crossterm::event::{KeyModifiers, MouseEventKind};

    let mods = mouse.modifiers;
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let shift = mods.contains(KeyModifiers::SHIFT);
    let alt = mods.contains(KeyModifiers::ALT);

    let (action, button) = match mouse.kind {
        MouseEventKind::Down(b) => (MouseAction::Down, ct_button(b)),
        MouseEventKind::Up(b) => (MouseAction::Up, ct_button(b)),
        MouseEventKind::Drag(b) => (MouseAction::Drag, ct_button(b)),
        MouseEventKind::Moved => (MouseAction::Move, MouseButton::None),
        MouseEventKind::ScrollUp => (MouseAction::ScrollUp, MouseButton::None),
        MouseEventKind::ScrollDown => (MouseAction::ScrollDown, MouseButton::None),
        MouseEventKind::ScrollLeft => (MouseAction::ScrollUp, MouseButton::None),
        MouseEventKind::ScrollRight => (MouseAction::ScrollDown, MouseButton::None),
    };

    MouseInput {
        action,
        button,
        column: mouse.column,
        row: mouse.row,
        ctrl,
        shift,
        alt,
    }
}

fn ct_button(b: crossterm::event::MouseButton) -> MouseButton {
    match b {
        crossterm::event::MouseButton::Left => MouseButton::Left,
        crossterm::event::MouseButton::Right => MouseButton::Right,
        crossterm::event::MouseButton::Middle => MouseButton::Middle,
    }
}

// ---------------------------------------------------------------------------
// KeyInput → KeyCodeChord
// ---------------------------------------------------------------------------

/// Convert a [`KeyInput`] into a [`KeyCodeChord`] for keybinding matching.
pub fn key_input_to_chord(input: KeyInput) -> KeyCodeChord {
    KeyCodeChord::new(input.ctrl, input.shift, input.alt, input.meta, input.key_code)
}

// ---------------------------------------------------------------------------
// InputDispatcher
// ---------------------------------------------------------------------------

/// Routes [`InputEvent`]s to typed event handlers.
pub struct InputDispatcher {
    on_key: Emitter<KeyInput>,
    on_mouse: Emitter<MouseInput>,
}

impl InputDispatcher {
    /// Create a new dispatcher with no listeners.
    pub fn new() -> Self {
        Self {
            on_key: Emitter::new(),
            on_mouse: Emitter::new(),
        }
    }

    /// Dispatch an [`InputEvent`] to the appropriate emitter.
    pub fn dispatch(&self, event: InputEvent) {
        match event {
            InputEvent::Key(key) => self.on_key.fire(&key),
            InputEvent::Mouse(mouse) => self.on_mouse.fire(&mouse),
            InputEvent::Paste(_) | InputEvent::Resize { .. } => {}
        }
    }

    /// The key-down event stream.
    pub fn on_key_down(&self) -> Event<KeyInput> {
        self.on_key.event()
    }

    /// The mouse event stream.
    pub fn on_mouse_event(&self) -> Event<MouseInput> {
        self.on_mouse.event()
    }
}

impl Default for InputDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for input operations.
#[derive(Debug, Clone, PartialEq)]
pub struct InputStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl InputStats {
    /// Create a new empty statistics tracker.
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

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &InputStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for InputStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for InputStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InputStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for input.
#[derive(Debug, Clone)]
pub struct InputValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl InputValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for InputValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// InputEventBatcher
// ---------------------------------------------------------------------------

/// Batches rapid key events within a time window.
pub struct InputEventBatcher {
    pending: Vec<KeyInput>,
    batch_window_ms: u64,
    batch_start_ms: u64,
    last_event_ms: u64,
    total_batches: u64,
    total_events: u64,
}

impl InputEventBatcher {
    /// Create a new batcher with the given window in milliseconds.
    pub fn new(batch_window_ms: u64) -> Self {
        Self {
            pending: Vec::new(),
            batch_window_ms,
            batch_start_ms: 0,
            last_event_ms: 0,
            total_batches: 0,
            total_events: 0,
        }
    }

    /// Push a key event. Returns `Some(batch)` if the new event falls outside
    /// the window measured from the first pending event, flushing the previous
    /// batch and starting a new one with the current event.
    pub fn push(&mut self, key: KeyInput, timestamp_ms: u64) -> Option<Vec<KeyInput>> {
        self.total_events += 1;
        self.last_event_ms = timestamp_ms;

        if self.pending.is_empty() {
            self.batch_start_ms = timestamp_ms;
            self.pending.push(key);
            return None;
        }

        if timestamp_ms.saturating_sub(self.batch_start_ms) > self.batch_window_ms {
            self.total_batches += 1;
            let batch = std::mem::take(&mut self.pending);
            self.batch_start_ms = timestamp_ms;
            self.pending.push(key);
            return Some(batch);
        }

        self.pending.push(key);
        None
    }

    /// Drain all pending events.
    pub fn flush(&mut self) -> Vec<KeyInput> {
        if !self.pending.is_empty() {
            self.total_batches += 1;
        }
        std::mem::take(&mut self.pending)
    }

    /// Number of events waiting in the current batch.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Total batches produced so far.
    pub fn total_batches(&self) -> u64 {
        self.total_batches
    }

    /// Total events received so far.
    pub fn total_events(&self) -> u64 {
        self.total_events
    }

    /// Whether there are no pending events.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

// ---------------------------------------------------------------------------
// GestureRecognizer
// ---------------------------------------------------------------------------

/// Detected gesture types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    SingleClick,
    DoubleClick,
    TripleClick,
}

/// Recognizes multi-click gestures from mouse down events.
pub struct GestureRecognizer {
    last_click_ms: u64,
    second_click_ms: u64,
    click_count: u32,
    max_interval_ms: u64,
    last_col: u16,
    last_row: u16,
    max_distance: u16,
}

impl GestureRecognizer {
    /// Create a new recognizer with custom interval and distance thresholds.
    pub fn new(max_interval_ms: u64, max_distance: u16) -> Self {
        Self {
            last_click_ms: 0,
            second_click_ms: 0,
            click_count: 0,
            max_interval_ms,
            last_col: 0,
            last_row: 0,
            max_distance,
        }
    }

    /// Reset the recognizer state.
    pub fn reset(&mut self) {
        self.last_click_ms = 0;
        self.second_click_ms = 0;
        self.click_count = 0;
        self.last_col = 0;
        self.last_row = 0;
    }

    /// Current click count in the active gesture sequence.
    pub fn click_count(&self) -> u32 {
        self.click_count
    }

    /// Process a mouse-down event and return the detected gesture.
    pub fn on_mouse_down(&mut self, col: u16, row: u16, timestamp_ms: u64) -> Gesture {
        let col_diff = (col as i32 - self.last_col as i32).unsigned_abs() as u16;
        let row_diff = (row as i32 - self.last_row as i32).unsigned_abs() as u16;
        let distance = col_diff.max(row_diff);

        let within_interval = timestamp_ms.saturating_sub(self.last_click_ms) <= self.max_interval_ms;
        let within_distance = distance <= self.max_distance;

        if within_interval && within_distance && self.click_count > 0 {
            self.click_count += 1;
        } else {
            self.click_count = 1;
        }

        self.last_col = col;
        self.last_row = row;

        let gesture = match self.click_count {
            2 => {
                self.second_click_ms = timestamp_ms;
                Gesture::DoubleClick
            }
            3 => {
                self.click_count = 0; // reset after triple
                Gesture::TripleClick
            }
            _ => Gesture::SingleClick,
        };

        self.last_click_ms = timestamp_ms;
        gesture
    }
}

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new(300, 3)
    }
}

// ---------------------------------------------------------------------------
// input_chord_builder
// ---------------------------------------------------------------------------

/// Builds a multi-key chord from a string representation.
/// Format: `"ctrl+shift+k"` or `"ctrl+k ctrl+d"` (two-part chord).
/// Returns the parsed chords or an error.
pub fn input_chord_builder(chord_str: &str) -> Result<Vec<KeyCodeChord>, String> {
    let chord_str = chord_str.trim();
    if chord_str.is_empty() {
        return Err("empty chord string".to_string());
    }

    let parts: Vec<&str> = chord_str.split_whitespace().collect();
    let mut chords = Vec::with_capacity(parts.len());

    for part in parts {
        let chord = KeyChordParser::parse(part).map_err(|e| format!("{e:?}"))?;
        chords.push(chord);
    }

    Ok(chords)
}



// ---------------------------------------------------------------------------
// InputSequence
// ---------------------------------------------------------------------------

/// An ordered sequence of key inputs, useful for multi-key chord matching.
#[derive(Debug, Clone, Default)]
pub struct InputSequence {
    keys: Vec<KeyInput>,
}

impl InputSequence {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, key: KeyInput) {
        self.keys.push(key);
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Check if `prefix` matches the start of this sequence.
    pub fn matches_prefix(&self, prefix: &[KeyInput]) -> bool {
        if prefix.len() > self.keys.len() {
            return false;
        }
        self.keys[..prefix.len()] == *prefix
    }

    /// Build a human-readable chord string like "ctrl+a ctrl+b".
    pub fn to_chord_string(&self) -> String {
        self.keys
            .iter()
            .map(|k| {
                let mut parts = Vec::new();
                if k.ctrl { parts.push("ctrl"); }
                if k.shift { parts.push("shift"); }
                if k.alt { parts.push("alt"); }
                if k.meta { parts.push("meta"); }
                parts.push(match k.key_code {
                    KeyCode::KeyA => "a", KeyCode::KeyB => "b", KeyCode::KeyC => "c",
                    KeyCode::KeyD => "d", KeyCode::KeyE => "e", KeyCode::KeyF => "f",
                    KeyCode::KeyG => "g", KeyCode::KeyH => "h", KeyCode::KeyI => "i",
                    KeyCode::KeyJ => "j", KeyCode::KeyK => "k", KeyCode::KeyL => "l",
                    KeyCode::KeyM => "m", KeyCode::KeyN => "n", KeyCode::KeyO => "o",
                    KeyCode::KeyP => "p", KeyCode::KeyQ => "q", KeyCode::KeyR => "r",
                    KeyCode::KeyS => "s", KeyCode::KeyT => "t", KeyCode::KeyU => "u",
                    KeyCode::KeyV => "v", KeyCode::KeyW => "w", KeyCode::KeyX => "x",
                    KeyCode::KeyY => "y", KeyCode::KeyZ => "z",
                    KeyCode::Enter => "enter", KeyCode::Tab => "tab",
                    KeyCode::Escape => "escape", KeyCode::Space => "space",
                    _ => "?",
                });
                parts.join("+")
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

// ---------------------------------------------------------------------------
// InputHistory
// ---------------------------------------------------------------------------

/// A history buffer of recent key inputs.
#[derive(Debug, Clone, Default)]
pub struct InputHistory {
    entries: Vec<KeyInput>,
}

impl InputHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, key: KeyInput) {
        self.entries.push(key);
    }

    pub fn last(&self) -> Option<&KeyInput> {
        self.entries.last()
    }

    /// Return the `n` most recent inputs (oldest first).
    pub fn recent(&self, n: usize) -> &[KeyInput] {
        let start = self.entries.len().saturating_sub(n);
        &self.entries[start..]
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// KeyGestureRecognizer
// ---------------------------------------------------------------------------

/// Recognizes multi-key gesture patterns from a stream of keyboard inputs.
/// Unlike `GestureRecognizer` (which handles mouse clicks), this matches
/// sequential key press patterns.
#[derive(Debug, Clone)]
pub struct KeyGestureRecognizer {
    patterns: Vec<(String, Vec<KeyInput>)>,
    buffer: Vec<KeyInput>,
}

impl KeyGestureRecognizer {
    pub fn new() -> Self {
        Self { patterns: Vec::new(), buffer: Vec::new() }
    }

    pub fn add_pattern(&mut self, name: impl Into<String>, keys: Vec<KeyInput>) {
        self.patterns.push((name.into(), keys));
    }

    /// Feed a key and check if any pattern matches.
    pub fn recognize(&mut self, key: KeyInput) -> Option<String> {
        self.buffer.push(key);
        for (name, pattern) in &self.patterns {
            if self.buffer.len() >= pattern.len() {
                let start = self.buffer.len() - pattern.len();
                if &self.buffer[start..] == pattern.as_slice() {
                    let matched = name.clone();
                    self.buffer.clear();
                    return Some(matched);
                }
            }
        }
        if self.buffer.len() > 32 {
            self.buffer.drain(..16);
        }
        None
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
    }
}

impl Default for KeyGestureRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// KeyPressCounter
// ---------------------------------------------------------------------------

/// Tracks how many times each key code has been pressed.
#[derive(Debug, Clone, Default)]
pub struct KeyPressCounter {
    counts: std::collections::HashMap<KeyCode, u64>,
}

impl KeyPressCounter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, key: KeyCode) {
        *self.counts.entry(key).or_insert(0) += 1;
    }

    /// Return the `n` most frequently pressed keys, sorted descending.
    pub fn top_keys(&self, n: usize) -> Vec<(KeyCode, u64)> {
        let mut entries: Vec<_> = self.counts.iter().map(|(&k, &v)| (k, v)).collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(n);
        entries
    }

    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{
        KeyCode as CtKey, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers,
        MouseButton as CtBtn, MouseEvent, MouseEventKind,
    };
    use std::sync::{Arc, Mutex};

    fn make_key(code: CtKey, mods: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: mods,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn make_mouse(kind: MouseEventKind, col: u16, row: u16, mods: KeyModifiers) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: mods,
        }
    }

    // -- Key mapping tests --------------------------------------------------

    #[test]
    fn map_letter_keys() {
        for (ch, expected) in [('a', KeyCode::KeyA), ('m', KeyCode::KeyM), ('z', KeyCode::KeyZ)] {
            let input = from_crossterm_key(make_key(CtKey::Char(ch), KeyModifiers::NONE));
            assert_eq!(input.key_code, expected, "failed for '{ch}'");
            assert!(!input.ctrl && !input.shift && !input.alt && !input.meta);
        }
    }

    #[test]
    fn map_uppercase_letters() {
        let input = from_crossterm_key(make_key(CtKey::Char('A'), KeyModifiers::SHIFT));
        assert_eq!(input.key_code, KeyCode::KeyA);
        assert!(input.shift);
    }

    #[test]
    fn map_digit_keys() {
        for (ch, expected) in [('0', KeyCode::Digit0), ('5', KeyCode::Digit5), ('9', KeyCode::Digit9)] {
            let input = from_crossterm_key(make_key(CtKey::Char(ch), KeyModifiers::NONE));
            assert_eq!(input.key_code, expected, "failed for '{ch}'");
        }
    }

    #[test]
    fn map_function_keys() {
        for (n, expected) in [
            (1, KeyCode::F1), (5, KeyCode::F5), (12, KeyCode::F12), (24, KeyCode::F24),
        ] {
            let input = from_crossterm_key(make_key(CtKey::F(n), KeyModifiers::NONE));
            assert_eq!(input.key_code, expected, "failed for F{n}");
        }
    }

    #[test]
    fn map_arrow_keys() {
        assert_eq!(
            from_crossterm_key(make_key(CtKey::Left, KeyModifiers::NONE)).key_code,
            KeyCode::LeftArrow
        );
        assert_eq!(
            from_crossterm_key(make_key(CtKey::Right, KeyModifiers::NONE)).key_code,
            KeyCode::RightArrow
        );
        assert_eq!(
            from_crossterm_key(make_key(CtKey::Up, KeyModifiers::NONE)).key_code,
            KeyCode::UpArrow
        );
        assert_eq!(
            from_crossterm_key(make_key(CtKey::Down, KeyModifiers::NONE)).key_code,
            KeyCode::DownArrow
        );
    }

    #[test]
    fn map_special_keys() {
        assert_eq!(
            from_crossterm_key(make_key(CtKey::Backspace, KeyModifiers::NONE)).key_code,
            KeyCode::Backspace
        );
        assert_eq!(
            from_crossterm_key(make_key(CtKey::Enter, KeyModifiers::NONE)).key_code,
            KeyCode::Enter
        );
        assert_eq!(
            from_crossterm_key(make_key(CtKey::Tab, KeyModifiers::NONE)).key_code,
            KeyCode::Tab
        );
        assert_eq!(
            from_crossterm_key(make_key(CtKey::Esc, KeyModifiers::NONE)).key_code,
            KeyCode::Escape
        );
        assert_eq!(
            from_crossterm_key(make_key(CtKey::Delete, KeyModifiers::NONE)).key_code,
            KeyCode::Delete
        );
        assert_eq!(
            from_crossterm_key(make_key(CtKey::Insert, KeyModifiers::NONE)).key_code,
            KeyCode::Insert
        );
        assert_eq!(
            from_crossterm_key(make_key(CtKey::Home, KeyModifiers::NONE)).key_code,
            KeyCode::Home
        );
        assert_eq!(
            from_crossterm_key(make_key(CtKey::End, KeyModifiers::NONE)).key_code,
            KeyCode::End
        );
        assert_eq!(
            from_crossterm_key(make_key(CtKey::PageUp, KeyModifiers::NONE)).key_code,
            KeyCode::PageUp
        );
        assert_eq!(
            from_crossterm_key(make_key(CtKey::PageDown, KeyModifiers::NONE)).key_code,
            KeyCode::PageDown
        );
    }

    #[test]
    fn map_punctuation() {
        let cases = [
            (';', KeyCode::Semicolon),
            ('=', KeyCode::Equal),
            (',', KeyCode::Comma),
            ('-', KeyCode::Minus),
            ('.', KeyCode::Period),
            ('/', KeyCode::Slash),
            ('`', KeyCode::Backquote),
            ('[', KeyCode::BracketLeft),
            ('\\', KeyCode::Backslash),
            (']', KeyCode::BracketRight),
            ('\'', KeyCode::Quote),
        ];
        for (ch, expected) in cases {
            let input = from_crossterm_key(make_key(CtKey::Char(ch), KeyModifiers::NONE));
            assert_eq!(input.key_code, expected, "failed for '{ch}'");
        }
    }

    #[test]
    fn map_space() {
        let input = from_crossterm_key(make_key(CtKey::Char(' '), KeyModifiers::NONE));
        assert_eq!(input.key_code, KeyCode::Space);
    }

    #[test]
    fn map_modifiers() {
        let input = from_crossterm_key(make_key(
            CtKey::Char('s'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ));
        assert_eq!(input.key_code, KeyCode::KeyS);
        assert!(input.ctrl);
        assert!(input.shift);
        assert!(!input.alt);
        assert!(!input.meta);
    }

    #[test]
    fn map_alt_modifier() {
        let input = from_crossterm_key(make_key(CtKey::Char('f'), KeyModifiers::ALT));
        assert!(input.alt);
        assert!(!input.ctrl);
    }

    #[test]
    fn map_super_modifier() {
        let input = from_crossterm_key(make_key(CtKey::Char('a'), KeyModifiers::SUPER));
        assert!(input.meta);
    }

    #[test]
    fn map_backtab_is_shift_tab() {
        let input = from_crossterm_key(make_key(CtKey::BackTab, KeyModifiers::SHIFT));
        assert_eq!(input.key_code, KeyCode::Tab);
        assert!(input.shift);
    }

    #[test]
    fn map_unknown_key() {
        let input = from_crossterm_key(make_key(CtKey::Null, KeyModifiers::NONE));
        assert_eq!(input.key_code, KeyCode::Unknown);
    }

    // -- Mouse mapping tests ------------------------------------------------

    #[test]
    fn map_mouse_down() {
        let input = from_crossterm_mouse(make_mouse(
            MouseEventKind::Down(CtBtn::Left),
            10, 20, KeyModifiers::NONE,
        ));
        assert_eq!(input.action, MouseAction::Down);
        assert_eq!(input.button, MouseButton::Left);
        assert_eq!(input.column, 10);
        assert_eq!(input.row, 20);
    }

    #[test]
    fn map_mouse_up() {
        let input = from_crossterm_mouse(make_mouse(
            MouseEventKind::Up(CtBtn::Right),
            5, 15, KeyModifiers::NONE,
        ));
        assert_eq!(input.action, MouseAction::Up);
        assert_eq!(input.button, MouseButton::Right);
    }

    #[test]
    fn map_mouse_drag() {
        let input = from_crossterm_mouse(make_mouse(
            MouseEventKind::Drag(CtBtn::Middle),
            3, 7, KeyModifiers::NONE,
        ));
        assert_eq!(input.action, MouseAction::Drag);
        assert_eq!(input.button, MouseButton::Middle);
    }

    #[test]
    fn map_mouse_scroll() {
        let up = from_crossterm_mouse(make_mouse(
            MouseEventKind::ScrollUp, 0, 0, KeyModifiers::NONE,
        ));
        assert_eq!(up.action, MouseAction::ScrollUp);
        assert_eq!(up.button, MouseButton::None);

        let down = from_crossterm_mouse(make_mouse(
            MouseEventKind::ScrollDown, 0, 0, KeyModifiers::NONE,
        ));
        assert_eq!(down.action, MouseAction::ScrollDown);
    }

    #[test]
    fn map_mouse_move() {
        let input = from_crossterm_mouse(make_mouse(
            MouseEventKind::Moved, 42, 13, KeyModifiers::NONE,
        ));
        assert_eq!(input.action, MouseAction::Move);
        assert_eq!(input.button, MouseButton::None);
        assert_eq!(input.column, 42);
        assert_eq!(input.row, 13);
    }

    #[test]
    fn map_mouse_modifiers() {
        let input = from_crossterm_mouse(make_mouse(
            MouseEventKind::Down(CtBtn::Left),
            0, 0,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT | KeyModifiers::ALT,
        ));
        assert!(input.ctrl);
        assert!(input.shift);
        assert!(input.alt);
    }

    // -- key_input_to_chord -------------------------------------------------

    #[test]
    fn key_input_to_chord_basic() {
        let input = KeyInput {
            key_code: KeyCode::KeyS,
            ctrl: true,
            shift: false,
            alt: false,
            meta: false,
        };
        let chord = key_input_to_chord(input);
        assert_eq!(chord, KeyCodeChord::new(true, false, false, false, KeyCode::KeyS));
    }

    #[test]
    fn key_input_to_chord_all_modifiers() {
        let input = KeyInput {
            key_code: KeyCode::F5,
            ctrl: true,
            shift: true,
            alt: true,
            meta: true,
        };
        let chord = key_input_to_chord(input);
        assert!(chord.ctrl && chord.shift && chord.alt && chord.meta);
        assert_eq!(chord.key_code, KeyCode::F5);
    }

    // -- InputDispatcher tests ----------------------------------------------

    #[test]
    fn dispatcher_routes_key_events() {
        let dispatcher = InputDispatcher::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = dispatcher.on_key_down().on(move |k: &KeyInput| {
            r.lock().unwrap().push(k.key_code);
        });

        let key = KeyInput {
            key_code: KeyCode::KeyA,
            ctrl: false, shift: false, alt: false, meta: false,
        };
        dispatcher.dispatch(InputEvent::Key(key));

        assert_eq!(*received.lock().unwrap(), vec![KeyCode::KeyA]);
    }

    #[test]
    fn dispatcher_routes_mouse_events() {
        let dispatcher = InputDispatcher::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = dispatcher.on_mouse_event().on(move |m: &MouseInput| {
            r.lock().unwrap().push(m.action);
        });

        let mouse = MouseInput {
            action: MouseAction::Down,
            button: MouseButton::Left,
            column: 0, row: 0,
            ctrl: false, shift: false, alt: false,
        };
        dispatcher.dispatch(InputEvent::Mouse(mouse));

        assert_eq!(*received.lock().unwrap(), vec![MouseAction::Down]);
    }

    #[test]
    fn dispatcher_ignores_paste_and_resize() {
        let dispatcher = InputDispatcher::new();
        let count = Arc::new(Mutex::new(0u32));
        let c = count.clone();
        let _h = dispatcher.on_key_down().on(move |_: &KeyInput| {
            *c.lock().unwrap() += 1;
        });

        dispatcher.dispatch(InputEvent::Paste("hello".into()));
        dispatcher.dispatch(InputEvent::Resize { width: 80, height: 24 });

        assert_eq!(*count.lock().unwrap(), 0);
    }

    #[test]
    fn dispatcher_default_impl() {
        let _d: InputDispatcher = Default::default();
    }

    // -- InputEvent derives -------------------------------------------------

    #[test]
    fn input_event_clone_and_eq() {
        let e1 = InputEvent::Resize { width: 80, height: 24 };
        let e2 = e1.clone();
        assert_eq!(e1, e2);
    }

    #[test]
    fn input_event_debug() {
        let e = InputEvent::Paste("test".into());
        let dbg = format!("{e:?}");
        assert!(dbg.contains("Paste"));
    }

    #[test]
    fn input_stats_new_defaults() {
        let stats = InputStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn input_stats_record_success() {
        let mut stats = InputStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn input_stats_record_failure() {
        let mut stats = InputStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn input_stats_reset() {
        let mut stats = InputStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn input_stats_merge() {
        let mut a = InputStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = InputStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn input_stats_display() {
        let mut stats = InputStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn input_stats_default() {
        let stats = InputStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn input_validator_accepts_valid_name() {
        let v = InputValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn input_validator_rejects_empty() {
        let v = InputValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn input_validator_rejects_too_long() {
        let v = InputValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn input_validator_forbidden_prefix() {
        let v = InputValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn input_validator_allowed_chars() {
        let v = InputValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn input_validator_range() {
        let v = InputValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn input_sanitize_removes_control() {
        let result = InputValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn input_truncate_short_string() {
        assert_eq!(InputValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn input_truncate_long_string() {
        let result = InputValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn input_is_ascii_printable() {
        assert!(InputValidator::is_ascii_printable("Hello World 123"));
        assert!(!InputValidator::is_ascii_printable("Hello\x00World"));
    }

    // -----------------------------------------------------------------------
    // InputEventBatcher tests
    // -----------------------------------------------------------------------

    fn key_a() -> KeyInput {
        KeyInput {
            key_code: KeyCode::KeyA,
            ctrl: false,
            shift: false,
            alt: false,
            meta: false,
        }
    }

    fn key_b() -> KeyInput {
        KeyInput {
            key_code: KeyCode::KeyB,
            ctrl: false,
            shift: false,
            alt: false,
            meta: false,
        }
    }

    #[test]
    fn batcher_new_is_empty() {
        let b = InputEventBatcher::new(50);
        assert!(b.is_empty());
        assert_eq!(b.pending_count(), 0);
        assert_eq!(b.total_batches(), 0);
        assert_eq!(b.total_events(), 0);
    }

    #[test]
    fn batcher_first_push_returns_none() {
        let mut b = InputEventBatcher::new(50);
        assert!(b.push(key_a(), 100).is_none());
        assert_eq!(b.pending_count(), 1);
        assert!(!b.is_empty());
    }

    #[test]
    fn batcher_within_window_returns_none() {
        let mut b = InputEventBatcher::new(50);
        assert!(b.push(key_a(), 100).is_none());
        assert!(b.push(key_b(), 130).is_none());
        assert_eq!(b.pending_count(), 2);
    }

    #[test]
    fn batcher_outside_window_returns_batch() {
        let mut b = InputEventBatcher::new(50);
        b.push(key_a(), 100);
        b.push(key_b(), 120);
        let batch = b.push(key_a(), 200); // 200 - 100 = 100 > 50
        assert!(batch.is_some());
        let batch = batch.unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].key_code, KeyCode::KeyA);
        assert_eq!(batch[1].key_code, KeyCode::KeyB);
        assert_eq!(b.pending_count(), 1); // new event started new batch
    }

    #[test]
    fn batcher_flush_drains_pending() {
        let mut b = InputEventBatcher::new(50);
        b.push(key_a(), 10);
        b.push(key_b(), 20);
        let flushed = b.flush();
        assert_eq!(flushed.len(), 2);
        assert!(b.is_empty());
    }

    #[test]
    fn batcher_flush_empty_returns_empty_vec() {
        let mut b = InputEventBatcher::new(50);
        let flushed = b.flush();
        assert!(flushed.is_empty());
        assert_eq!(b.total_batches(), 0);
    }

    #[test]
    fn batcher_total_events_counted() {
        let mut b = InputEventBatcher::new(50);
        b.push(key_a(), 0);
        b.push(key_b(), 10);
        b.push(key_a(), 20);
        assert_eq!(b.total_events(), 3);
    }

    #[test]
    fn batcher_total_batches_counted() {
        let mut b = InputEventBatcher::new(50);
        b.push(key_a(), 0);
        b.push(key_b(), 10);
        // flush produces batch 1
        b.flush();
        assert_eq!(b.total_batches(), 1);
        // push outside window produces batch 2
        b.push(key_a(), 100);
        b.push(key_b(), 200); // 200 - 100 = 100 > 50
        assert_eq!(b.total_batches(), 2);
    }

    #[test]
    fn batcher_boundary_exact_window() {
        let mut b = InputEventBatcher::new(50);
        b.push(key_a(), 100);
        // exactly at boundary (100 + 50 = 150) is still within window
        let result = b.push(key_b(), 150);
        assert!(result.is_none());
        assert_eq!(b.pending_count(), 2);
    }

    // -----------------------------------------------------------------------
    // GestureRecognizer tests
    // -----------------------------------------------------------------------

    #[test]
    fn gesture_single_click() {
        let mut g = GestureRecognizer::default();
        assert_eq!(g.on_mouse_down(10, 5, 1000), Gesture::SingleClick);
        assert_eq!(g.click_count(), 1);
    }

    #[test]
    fn gesture_double_click() {
        let mut g = GestureRecognizer::default();
        g.on_mouse_down(10, 5, 1000);
        assert_eq!(g.on_mouse_down(10, 5, 1100), Gesture::DoubleClick);
    }

    #[test]
    fn gesture_triple_click() {
        let mut g = GestureRecognizer::default();
        g.on_mouse_down(10, 5, 1000);
        g.on_mouse_down(10, 5, 1100);
        assert_eq!(g.on_mouse_down(10, 5, 1200), Gesture::TripleClick);
    }

    #[test]
    fn gesture_resets_after_triple() {
        let mut g = GestureRecognizer::default();
        g.on_mouse_down(10, 5, 1000);
        g.on_mouse_down(10, 5, 1100);
        g.on_mouse_down(10, 5, 1200); // triple, resets
        // next click within interval is a fresh single
        assert_eq!(g.on_mouse_down(10, 5, 1300), Gesture::SingleClick);
    }

    #[test]
    fn gesture_too_slow_resets() {
        let mut g = GestureRecognizer::default(); // 300ms max
        g.on_mouse_down(10, 5, 1000);
        // 400ms later, outside interval
        assert_eq!(g.on_mouse_down(10, 5, 1400), Gesture::SingleClick);
    }

    #[test]
    fn gesture_too_far_resets() {
        let mut g = GestureRecognizer::default(); // 3px max distance
        g.on_mouse_down(10, 5, 1000);
        // moved 10 columns away
        assert_eq!(g.on_mouse_down(20, 5, 1100), Gesture::SingleClick);
    }

    #[test]
    fn gesture_reset_method() {
        let mut g = GestureRecognizer::default();
        g.on_mouse_down(10, 5, 1000);
        g.on_mouse_down(10, 5, 1100);
        g.reset();
        assert_eq!(g.click_count(), 0);
        assert_eq!(g.on_mouse_down(10, 5, 1200), Gesture::SingleClick);
    }

    #[test]
    fn gesture_custom_interval() {
        let mut g = GestureRecognizer::new(100, 3);
        g.on_mouse_down(5, 5, 1000);
        // 150ms > 100ms custom interval
        assert_eq!(g.on_mouse_down(5, 5, 1150), Gesture::SingleClick);
    }

    #[test]
    fn gesture_within_distance_threshold() {
        let mut g = GestureRecognizer::new(300, 3);
        g.on_mouse_down(10, 10, 1000);
        // moved 2 cols and 1 row, max distance = 2 <= 3
        assert_eq!(g.on_mouse_down(12, 11, 1100), Gesture::DoubleClick);
    }

    // -----------------------------------------------------------------------
    // input_chord_builder tests
    // -----------------------------------------------------------------------

    #[test]
    fn chord_single_key() {
        let chords = input_chord_builder("a").unwrap();
        assert_eq!(chords.len(), 1);
        assert_eq!(chords[0].key_code, KeyCode::KeyA);
        assert!(!chords[0].ctrl);
    }

    #[test]
    fn chord_with_ctrl() {
        let chords = input_chord_builder("ctrl+s").unwrap();
        assert_eq!(chords.len(), 1);
        assert!(chords[0].ctrl);
        assert_eq!(chords[0].key_code, KeyCode::KeyS);
    }

    #[test]
    fn chord_ctrl_shift() {
        let chords = input_chord_builder("ctrl+shift+k").unwrap();
        assert_eq!(chords.len(), 1);
        assert!(chords[0].ctrl);
        assert!(chords[0].shift);
        assert_eq!(chords[0].key_code, KeyCode::KeyK);
    }

    #[test]
    fn chord_two_part() {
        let chords = input_chord_builder("ctrl+k ctrl+d").unwrap();
        assert_eq!(chords.len(), 2);
        assert!(chords[0].ctrl);
        assert_eq!(chords[0].key_code, KeyCode::KeyK);
        assert!(chords[1].ctrl);
        assert_eq!(chords[1].key_code, KeyCode::KeyD);
    }

    #[test]
    fn chord_function_key() {
        let chords = input_chord_builder("f5").unwrap();
        assert_eq!(chords.len(), 1);
        assert_eq!(chords[0].key_code, KeyCode::F5);
    }

    #[test]
    fn chord_escape() {
        let chords = input_chord_builder("escape").unwrap();
        assert_eq!(chords.len(), 1);
        assert_eq!(chords[0].key_code, KeyCode::Escape);
    }

    #[test]
    fn chord_empty_string_errors() {
        assert!(input_chord_builder("").is_err());
    }

    #[test]
    fn chord_unknown_key_errors() {
        assert!(input_chord_builder("ctrl+nonsense_key_xyz").is_err());
    }

    #[test]
    fn chord_alt_modifier() {
        let chords = input_chord_builder("alt+tab").unwrap();
        assert_eq!(chords.len(), 1);
        assert!(chords[0].alt);
        assert_eq!(chords[0].key_code, KeyCode::Tab);
    }

    #[test]
    fn chord_digit_key() {
        let chords = input_chord_builder("ctrl+1").unwrap();
        assert_eq!(chords.len(), 1);
        assert!(chords[0].ctrl);
        assert_eq!(chords[0].key_code, KeyCode::Digit1);
    }

    // --- new tests ---

    fn make_simple_key(code: KeyCode) -> KeyInput {
        KeyInput { key_code: code, ctrl: false, shift: false, alt: false, meta: false }
    }

    fn make_simple_ctrl_key(code: KeyCode) -> KeyInput {
        KeyInput { key_code: code, ctrl: true, shift: false, alt: false, meta: false }
    }

    #[test]
    fn input_sequence_push_and_chord_string() {
        let mut seq = InputSequence::new();
        seq.push(make_simple_ctrl_key(KeyCode::KeyA));
        seq.push(make_simple_key(KeyCode::KeyB));
        assert_eq!(seq.len(), 2);
        let s = seq.to_chord_string();
        assert!(s.contains("ctrl+a"));
        assert!(s.contains("b"));
    }

    #[test]
    fn input_sequence_matches_prefix() {
        let mut seq = InputSequence::new();
        seq.push(make_simple_key(KeyCode::KeyA));
        seq.push(make_simple_key(KeyCode::KeyB));
        seq.push(make_simple_key(KeyCode::KeyC));
        assert!(seq.matches_prefix(&[make_simple_key(KeyCode::KeyA), make_simple_key(KeyCode::KeyB)]));
        assert!(!seq.matches_prefix(&[make_simple_key(KeyCode::KeyB)]));
    }

    #[test]
    fn input_history_recent() {
        let mut hist = InputHistory::new();
        hist.push(make_simple_key(KeyCode::KeyA));
        hist.push(make_simple_key(KeyCode::KeyB));
        hist.push(make_simple_key(KeyCode::KeyC));
        assert_eq!(hist.len(), 3);
        let recent = hist.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].key_code, KeyCode::KeyB);
        assert_eq!(hist.last().unwrap().key_code, KeyCode::KeyC);
        hist.clear();
        assert!(hist.is_empty());
    }

    #[test]
    fn key_gesture_recognizer_match() {
        let mut gr = KeyGestureRecognizer::new();
        gr.add_pattern("save", vec![make_simple_ctrl_key(KeyCode::KeyS)]);
        assert!(gr.recognize(make_simple_key(KeyCode::KeyA)).is_none());
        assert_eq!(gr.recognize(make_simple_ctrl_key(KeyCode::KeyS)).unwrap(), "save");
    }

    #[test]
    fn key_gesture_recognizer_multi_key() {
        let mut gr = KeyGestureRecognizer::new();
        gr.add_pattern("quit", vec![make_simple_key(KeyCode::KeyQ), make_simple_key(KeyCode::KeyQ)]);
        assert!(gr.recognize(make_simple_key(KeyCode::KeyQ)).is_none());
        assert_eq!(gr.recognize(make_simple_key(KeyCode::KeyQ)).unwrap(), "quit");
    }

    #[test]
    fn key_press_counter_top_keys() {
        let mut stats = KeyPressCounter::new();
        for _ in 0..5 { stats.record(KeyCode::KeyA); }
        for _ in 0..3 { stats.record(KeyCode::KeyB); }
        stats.record(KeyCode::KeyC);
        assert_eq!(stats.total(), 9);
        let top = stats.top_keys(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, KeyCode::KeyA);
        assert_eq!(top[0].1, 5);
    }

}
