//! Persistent application state.

/// Core type for state.
pub struct State {
    _initialized: bool,
}

impl State {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = State::new();
        assert!(v._initialized);
    }
}
