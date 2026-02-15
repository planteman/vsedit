//! Task runner feature.

/// Core type for tasks_feature.
pub struct TasksFeature {
    _initialized: bool,
}

impl TasksFeature {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for TasksFeature {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = TasksFeature::new();
        assert!(v._initialized);
    }
}
