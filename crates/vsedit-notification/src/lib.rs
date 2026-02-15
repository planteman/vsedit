//! Notification service model.
//!
//! Equivalent to VS Code's `vs/platform/notification/common/notification.ts`.
//! Provides the data model for toast notifications.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Notification severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSeverity {
    Info,
    Warning,
    Error,
}

/// A notification action button.
#[derive(Debug, Clone)]
pub struct NotificationAction {
    pub label: String,
    pub id: String,
}

/// A notification.
#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u64,
    pub severity: NotificationSeverity,
    pub message: String,
    pub source: Option<String>,
    pub actions: Vec<NotificationAction>,
    pub progress: Option<NotificationProgress>,
    pub sticky: bool,
}

/// Progress state for a notification.
#[derive(Debug, Clone)]
pub struct NotificationProgress {
    pub infinite: bool,
    pub total: Option<u64>,
    pub worked: Option<u64>,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

impl Notification {
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(NotificationSeverity::Info, message)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(NotificationSeverity::Warning, message)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(NotificationSeverity::Error, message)
    }

    pub fn new(severity: NotificationSeverity, message: impl Into<String>) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            severity,
            message: message.into(),
            source: None,
            actions: Vec::new(),
            progress: None,
            sticky: false,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_action(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.actions.push(NotificationAction {
            id: id.into(),
            label: label.into(),
        });
        self
    }

    pub fn with_sticky(mut self) -> Self {
        self.sticky = true;
        self
    }
}

/// Notification service that manages active notifications.
pub struct NotificationService {
    notifications: Mutex<Vec<Notification>>,
}

impl NotificationService {
    pub fn new() -> Self {
        Self {
            notifications: Mutex::new(Vec::new()),
        }
    }

    /// Show a notification.
    pub fn notify(&self, notification: Notification) -> u64 {
        let id = notification.id;
        self.notifications.lock().unwrap().push(notification);
        id
    }

    /// Dismiss a notification by ID.
    pub fn dismiss(&self, id: u64) {
        self.notifications.lock().unwrap().retain(|n| n.id != id);
    }

    /// Get all active notifications.
    pub fn get_notifications(&self) -> Vec<Notification> {
        self.notifications.lock().unwrap().clone()
    }

    /// Clear all notifications.
    pub fn clear(&self) {
        self.notifications.lock().unwrap().clear();
    }

    /// Get the count of active notifications.
    pub fn count(&self) -> usize {
        self.notifications.lock().unwrap().len()
    }
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_notifications() {
        let info = Notification::info("Build succeeded");
        assert_eq!(info.severity, NotificationSeverity::Info);
        assert_eq!(info.message, "Build succeeded");

        let err = Notification::error("Compile error")
            .with_source("Rust")
            .with_action("show", "Show Output");
        assert_eq!(err.severity, NotificationSeverity::Error);
        assert_eq!(err.source, Some("Rust".to_string()));
        assert_eq!(err.actions.len(), 1);
    }

    #[test]
    fn notification_service_basic() {
        let svc = NotificationService::new();
        let id = svc.notify(Notification::info("hello"));
        assert_eq!(svc.count(), 1);

        svc.dismiss(id);
        assert_eq!(svc.count(), 0);
    }

    #[test]
    fn notification_service_clear() {
        let svc = NotificationService::new();
        svc.notify(Notification::info("a"));
        svc.notify(Notification::warning("b"));
        svc.notify(Notification::error("c"));
        assert_eq!(svc.count(), 3);
        svc.clear();
        assert_eq!(svc.count(), 0);
    }

    #[test]
    fn unique_ids() {
        let n1 = Notification::info("a");
        let n2 = Notification::info("b");
        assert_ne!(n1.id, n2.id);
    }

    #[test]
    fn sticky_notification() {
        let n = Notification::info("long running").with_sticky();
        assert!(n.sticky);
    }
}
