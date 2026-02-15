//! Input to command translation for the editor.

/// Maps keyboard input to editor commands.
pub struct EditorController {
    pub is_composing: bool,
}

impl EditorController {
    pub fn new() -> Self { Self { is_composing: false } }
}

impl Default for EditorController {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let c = EditorController::new();
        assert!(!c.is_composing);
    }
}
