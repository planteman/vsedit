//! Inter-process communication.

/// Core type for ipc.
pub struct Ipc {
    _initialized: bool,
}

impl Ipc {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Ipc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Ipc::new();
        assert!(v._initialized);
    }
}
