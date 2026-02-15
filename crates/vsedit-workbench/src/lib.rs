//! Main workbench assembly.

/// The main workbench state.
pub struct Workbench {
    started: bool,
}

impl Workbench {
    pub fn new() -> Self { Self { started: false } }
    pub fn start(&mut self) { self.started = true; }
    pub fn is_started(&self) -> bool { self.started }
}

impl Default for Workbench {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workbench_lifecycle() {
        let mut wb = Workbench::new();
        assert!(!wb.is_started());
        wb.start();
        assert!(wb.is_started());
    }
}
