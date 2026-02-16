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
// Progress
// ---------------------------------------------------------------------------

/// Tracks progress of a long-running operation shown in a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationProgress {
    pub total: u64,
    pub worked: u64,
    pub message: Option<String>,
}

impl NotificationProgress {
    pub fn new(total: u64) -> Self {
        Self {
            total,
            worked: 0,
            message: None,
        }
    }

    /// Returns `true` when `worked >= total`.
    pub fn is_complete(&self) -> bool {
        self.worked >= self.total
    }
}

// ---------------------------------------------------------------------------
// Extended service methods
// ---------------------------------------------------------------------------

impl NotificationService {
    /// Creates a notification that carries a progress tracker.
    pub fn create_with_progress(
        &mut self,
        msg: impl Into<String>,
        severity: NotificationSeverity,
        total: u64,
    ) -> (u64, NotificationProgress) {
        let id = self.add(msg.into(), severity);
        (id, NotificationProgress::new(total))
    }

    /// Convenience helper to advance a progress tracker.
    pub fn update_progress(progress: &mut NotificationProgress, increment: u64, msg: Option<String>) {
        progress.worked = progress.worked.saturating_add(increment);
        if msg.is_some() {
            progress.message = msg;
        }
    }

    /// Appends an action button to an existing notification.
    pub fn add_action(&mut self, id: u64, label: impl Into<String>, action_id: impl Into<String>) {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.actions.push(NotificationAction {
                label: label.into(),
                id: action_id.into(),
            });
        }
    }

    /// Marks a notification as sticky so it is not auto-dismissed.
    pub fn set_sticky(&mut self, id: u64, sticky: bool) {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.sticky = sticky;
        }
    }

    /// Returns notifications matching the given severity.
    pub fn get_by_severity(&self, severity: NotificationSeverity) -> Vec<&Notification> {
        self.notifications
            .iter()
            .filter(|n| n.severity == severity)
            .collect()
    }

    /// Returns notifications whose source matches `source`.
    pub fn get_by_source(&self, source: &str) -> Vec<&Notification> {
        self.notifications
            .iter()
            .filter(|n| n.source.as_deref() == Some(source))
            .collect()
    }

    /// Removes all dismissed notifications from internal storage.
    pub fn remove_dismissed(&mut self) {
        self.notifications.retain(|n| !n.dismissed);
    }

    /// Computes aggregate statistics about current notifications.
    pub fn get_stats(&self) -> NotificationStats {
        let total = self.notifications.len();
        let active = self.notifications.iter().filter(|n| !n.dismissed).count();
        let dismissed = total - active;
        let info = self
            .notifications
            .iter()
            .filter(|n| n.severity == NotificationSeverity::Info)
            .count();
        let warnings = self
            .notifications
            .iter()
            .filter(|n| n.severity == NotificationSeverity::Warning)
            .count();
        let errors = self
            .notifications
            .iter()
            .filter(|n| n.severity == NotificationSeverity::Error)
            .count();
        NotificationStats {
            total,
            active,
            dismissed,
            info,
            warnings,
            errors,
        }
    }
}

/// Aggregate statistics about notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationStats {
    pub total: usize,
    pub active: usize,
    pub dismissed: usize,
    pub info: usize,
    pub warnings: usize,
    pub errors: usize,
}

// ---------------------------------------------------------------------------
// Handler trait
// ---------------------------------------------------------------------------

/// Trait for reacting to notification lifecycle events.
pub trait NotificationHandler {
    fn on_notification(&self, _notification: &Notification) {}
    fn on_dismiss(&self, _id: u64) {}
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

    #[test]
    fn progress_tracking() {
        let mut svc = NotificationService::new();
        let (_id, mut progress) = svc.create_with_progress("Building", NotificationSeverity::Info, 100);
        assert!(!progress.is_complete());
        NotificationService::update_progress(&mut progress, 60, Some("halfway".into()));
        assert_eq!(progress.worked, 60);
        assert_eq!(progress.message.as_deref(), Some("halfway"));
        NotificationService::update_progress(&mut progress, 40, None);
        assert!(progress.is_complete());
    }

    #[test]
    fn add_action_to_notification() {
        let mut svc = NotificationService::new();
        let id = svc.info("update available");
        svc.add_action(id, "Install", "install_update");
        let n = svc.get_active();
        assert_eq!(n[0].actions.len(), 1);
        assert_eq!(n[0].actions[0].label, "Install");
    }

    #[test]
    fn add_action_nonexistent_is_noop() {
        let mut svc = NotificationService::new();
        svc.add_action(999, "Click", "click");
        assert_eq!(svc.notification_count(), 0);
    }

    #[test]
    fn set_sticky() {
        let mut svc = NotificationService::new();
        let id = svc.warn("important");
        svc.set_sticky(id, true);
        assert!(svc.get_active()[0].sticky);
        svc.set_sticky(id, false);
        assert!(!svc.get_active()[0].sticky);
    }

    #[test]
    fn get_by_severity() {
        let mut svc = NotificationService::new();
        svc.info("a");
        svc.warn("b");
        svc.error("c");
        svc.info("d");
        assert_eq!(svc.get_by_severity(NotificationSeverity::Info).len(), 2);
        assert_eq!(svc.get_by_severity(NotificationSeverity::Error).len(), 1);
    }

    #[test]
    fn get_by_source() {
        let mut svc = NotificationService::new();
        let id = svc.info("from linter");
        // Manually set source for testing.
        svc.notifications.iter_mut().find(|n| n.id == id).unwrap().source = Some("linter".into());
        svc.info("no source");
        assert_eq!(svc.get_by_source("linter").len(), 1);
        assert_eq!(svc.get_by_source("unknown").len(), 0);
    }

    #[test]
    fn remove_dismissed() {
        let mut svc = NotificationService::new();
        let id = svc.info("gone");
        svc.info("stay");
        svc.dismiss(id);
        svc.remove_dismissed();
        assert_eq!(svc.notification_count(), 1);
        assert_eq!(svc.get_active()[0].message, "stay");
    }

    #[test]
    fn get_stats() {
        let mut svc = NotificationService::new();
        svc.info("a");
        svc.warn("b");
        svc.error("c");
        let id = svc.info("d");
        svc.dismiss(id);
        let stats = svc.get_stats();
        assert_eq!(stats.total, 4);
        assert_eq!(stats.active, 3);
        assert_eq!(stats.dismissed, 1);
        assert_eq!(stats.info, 2);
        assert_eq!(stats.warnings, 1);
        assert_eq!(stats.errors, 1);
    }

    #[test]
    fn handler_trait_defaults() {
        struct NoopHandler;
        impl NotificationHandler for NoopHandler {}
        let handler = NoopHandler;
        let n = Notification {
            id: 1,
            message: "test".into(),
            severity: NotificationSeverity::Info,
            source: None,
            actions: Vec::new(),
            sticky: false,
            dismissed: false,
        };
        handler.on_notification(&n);
        handler.on_dismiss(1);
    }

    #[test]
    fn progress_saturates() {
        let mut p = NotificationProgress::new(10);
        NotificationService::update_progress(&mut p, u64::MAX, None);
        assert!(p.is_complete());
    }
}
