//! Notification model service.

/// Core type for notification_svc.
pub struct NotificationSvc {
    _initialized: bool,
}

impl NotificationSvc {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for NotificationSvc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = NotificationSvc::new();
        assert!(v._initialized);
    }
}
