//! Terminal input event dispatch.
//!
//! Converts crossterm events into VS Code-compatible key/mouse events and
//! routes them through an [`InputDispatcher`] backed by [`vsedit_events`]
//! emitters.

use vsedit_events::{Emitter, Event};
use vsedit_keycodes::{KeyCode, KeyCodeChord};

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
}
