//! Notification model service.

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Severity level for a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotificationSeverity {
    Info,
    Warning,
    Error,
}

/// An action button that can be attached to a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationAction {
    pub label: String,
    pub id: String,
}

/// A single notification entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub id: u64,
    pub message: String,
    pub severity: NotificationSeverity,
    pub source: Option<String>,
    pub actions: Vec<NotificationAction>,
    pub sticky: bool,
    pub dismissed: bool,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Manages the lifecycle of user-facing notifications.
#[derive(Debug)]
pub struct NotificationService {
    notifications: Vec<Notification>,
    next_id: u64,
}

impl NotificationService {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            next_id: 1,
        }
    }

    /// Shows an informational notification. Returns its id.
    pub fn info(&mut self, msg: impl Into<String>) -> u64 {
        self.add(msg.into(), NotificationSeverity::Info)
    }

    /// Shows a warning notification. Returns its id.
    pub fn warn(&mut self, msg: impl Into<String>) -> u64 {
        self.add(msg.into(), NotificationSeverity::Warning)
    }

    /// Shows an error notification. Returns its id.
    pub fn error(&mut self, msg: impl Into<String>) -> u64 {
        self.add(msg.into(), NotificationSeverity::Error)
    }

    /// Dismisses the notification with the given id.
    pub fn dismiss(&mut self, id: u64) {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.dismissed = true;
        }
    }

    /// Dismisses all active notifications.
    pub fn dismiss_all(&mut self) {
        for n in &mut self.notifications {
            n.dismissed = true;
        }
    }

    /// Returns references to all non-dismissed notifications.
    pub fn get_active(&self) -> Vec<&Notification> {
        self.notifications.iter().filter(|n| !n.dismissed).collect()
    }

    /// Returns `true` if there is at least one non-dismissed notification.
    pub fn has_pending(&self) -> bool {
        self.notifications.iter().any(|n| !n.dismissed)
    }

    /// Total number of notifications (including dismissed).
    pub fn notification_count(&self) -> usize {
        self.notifications.len()
    }

    // -- internal -----------------------------------------------------------

    fn add(&mut self, message: String, severity: NotificationSeverity) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.notifications.push(Notification {
            id,
            message,
            severity,
            source: None,
            actions: Vec::new(),
            sticky: false,
            dismissed: false,
        });
        id
    }
}

impl Default for NotificationService {
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

    #[test]
    fn info_warn_error() {
        let mut svc = NotificationService::new();
        let i = svc.info("hello");
        let w = svc.warn("careful");
        let e = svc.error("boom");
        assert_eq!(svc.notification_count(), 3);
        assert_eq!(svc.notifications[0].severity, NotificationSeverity::Info);
        assert_eq!(svc.notifications[1].severity, NotificationSeverity::Warning);
        assert_eq!(svc.notifications[2].severity, NotificationSeverity::Error);
        // ids are sequential
        assert_eq!(i, 1);
        assert_eq!(w, 2);
        assert_eq!(e, 3);
    }

    #[test]
    fn dismiss_and_active() {
        let mut svc = NotificationService::new();
        let id = svc.info("a");
        svc.info("b");
        assert_eq!(svc.get_active().len(), 2);
        assert!(svc.has_pending());

        svc.dismiss(id);
        assert_eq!(svc.get_active().len(), 1);
        assert!(svc.has_pending());

        svc.dismiss_all();
        assert!(svc.get_active().is_empty());
        assert!(!svc.has_pending());
    }

    #[test]
    fn dismiss_nonexistent_is_noop() {
        let mut svc = NotificationService::new();
        svc.dismiss(999);
        assert_eq!(svc.notification_count(), 0);
    }

    #[test]
    fn notification_count_includes_dismissed() {
        let mut svc = NotificationService::new();
        let id = svc.info("x");
        svc.dismiss(id);
        assert_eq!(svc.notification_count(), 1);
    }
}
