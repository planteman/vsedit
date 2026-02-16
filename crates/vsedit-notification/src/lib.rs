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

impl Notification {
    /// Attach infinite or finite progress to the notification.
    pub fn with_progress(mut self, infinite: bool) -> Self {
        self.progress = Some(NotificationProgress {
            infinite,
            total: None,
            worked: None,
        });
        self
    }

    /// Attach finite progress with a known total to the notification.
    pub fn with_finite_progress(mut self, total: u64) -> Self {
        self.progress = Some(NotificationProgress {
            infinite: false,
            total: Some(total),
            worked: Some(0),
        });
        self
    }
}

/// Filter criteria for querying notifications.
#[derive(Debug, Clone, Default)]
pub struct NotificationFilter {
    pub severity: Option<NotificationSeverity>,
    pub source: Option<String>,
    pub sticky_only: bool,
}

/// Aggregate statistics about active notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationStats {
    pub total: usize,
    pub info_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
}

impl NotificationService {
    /// Update progress on an existing notification.
    pub fn update_progress(&self, id: u64, worked: u64) {
        let mut notifications = self.notifications.lock().unwrap();
        if let Some(n) = notifications.iter_mut().find(|n| n.id == id) {
            if let Some(ref mut progress) = n.progress {
                progress.worked = Some(worked);
            }
        }
    }

    /// Get all notifications with a specific severity.
    pub fn get_by_severity(&self, severity: NotificationSeverity) -> Vec<Notification> {
        self.notifications
            .lock()
            .unwrap()
            .iter()
            .filter(|n| n.severity == severity)
            .cloned()
            .collect()
    }

    /// Get all notifications from a specific source.
    pub fn get_by_source(&self, source: &str) -> Vec<Notification> {
        self.notifications
            .lock()
            .unwrap()
            .iter()
            .filter(|n| n.source.as_deref() == Some(source))
            .cloned()
            .collect()
    }

    /// Check whether any active notification has `Error` severity.
    pub fn has_errors(&self) -> bool {
        self.notifications
            .lock()
            .unwrap()
            .iter()
            .any(|n| n.severity == NotificationSeverity::Error)
    }

    /// Dismiss all notifications from the given source.
    pub fn dismiss_by_source(&self, source: &str) {
        self.notifications
            .lock()
            .unwrap()
            .retain(|n| n.source.as_deref() != Some(source));
    }

    /// Query notifications using a filter.
    pub fn get_filtered(&self, filter: &NotificationFilter) -> Vec<Notification> {
        self.notifications
            .lock()
            .unwrap()
            .iter()
            .filter(|n| {
                if let Some(sev) = filter.severity {
                    if n.severity != sev {
                        return false;
                    }
                }
                if let Some(ref src) = filter.source {
                    if n.source.as_deref() != Some(src.as_str()) {
                        return false;
                    }
                }
                if filter.sticky_only && !n.sticky {
                    return false;
                }
                true
            })
            .cloned()
            .collect()
    }

    /// Compute aggregate statistics about active notifications.
    pub fn get_stats(&self) -> NotificationStats {
        let notifications = self.notifications.lock().unwrap();
        let mut stats = NotificationStats {
            total: notifications.len(),
            info_count: 0,
            warning_count: 0,
            error_count: 0,
        };
        for n in notifications.iter() {
            match n.severity {
                NotificationSeverity::Info => stats.info_count += 1,
                NotificationSeverity::Warning => stats.warning_count += 1,
                NotificationSeverity::Error => stats.error_count += 1,
            }
        }
        stats
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

    #[test]
    fn progress_builders() {
        let n = Notification::info("Installing...")
            .with_progress(true);
        assert!(n.progress.as_ref().unwrap().infinite);

        let n2 = Notification::info("Downloading...")
            .with_finite_progress(100);
        let p = n2.progress.as_ref().unwrap();
        assert!(!p.infinite);
        assert_eq!(p.total, Some(100));
        assert_eq!(p.worked, Some(0));
    }

    #[test]
    fn update_progress() {
        let svc = NotificationService::new();
        let n = Notification::info("task").with_finite_progress(50);
        let id = svc.notify(n);
        svc.update_progress(id, 25);
        let all = svc.get_notifications();
        let found = all.iter().find(|n| n.id == id).unwrap();
        assert_eq!(found.progress.as_ref().unwrap().worked, Some(25));
    }

    #[test]
    fn get_by_severity() {
        let svc = NotificationService::new();
        svc.notify(Notification::info("a"));
        svc.notify(Notification::error("b"));
        svc.notify(Notification::error("c"));
        assert_eq!(svc.get_by_severity(NotificationSeverity::Error).len(), 2);
        assert_eq!(svc.get_by_severity(NotificationSeverity::Info).len(), 1);
        assert_eq!(svc.get_by_severity(NotificationSeverity::Warning).len(), 0);
    }

    #[test]
    fn get_by_source_and_dismiss() {
        let svc = NotificationService::new();
        svc.notify(Notification::info("a").with_source("rust-analyzer"));
        svc.notify(Notification::warning("b").with_source("rust-analyzer"));
        svc.notify(Notification::error("c").with_source("clippy"));
        assert_eq!(svc.get_by_source("rust-analyzer").len(), 2);
        svc.dismiss_by_source("rust-analyzer");
        assert_eq!(svc.count(), 1);
        assert_eq!(svc.get_by_source("rust-analyzer").len(), 0);
    }

    #[test]
    fn has_errors_check() {
        let svc = NotificationService::new();
        svc.notify(Notification::info("ok"));
        assert!(!svc.has_errors());
        svc.notify(Notification::error("fail"));
        assert!(svc.has_errors());
    }

    #[test]
    fn notification_filter() {
        let svc = NotificationService::new();
        svc.notify(Notification::info("a").with_source("src1").with_sticky());
        svc.notify(Notification::error("b").with_source("src1"));
        svc.notify(Notification::info("c").with_source("src2"));

        let filter = NotificationFilter {
            severity: Some(NotificationSeverity::Info),
            source: Some("src1".into()),
            sticky_only: false,
        };
        assert_eq!(svc.get_filtered(&filter).len(), 1);

        let sticky_filter = NotificationFilter {
            sticky_only: true,
            ..Default::default()
        };
        assert_eq!(svc.get_filtered(&sticky_filter).len(), 1);
    }

    #[test]
    fn notification_stats() {
        let svc = NotificationService::new();
        svc.notify(Notification::info("a"));
        svc.notify(Notification::info("b"));
        svc.notify(Notification::warning("c"));
        svc.notify(Notification::error("d"));
        let stats = svc.get_stats();
        assert_eq!(stats, NotificationStats {
            total: 4,
            info_count: 2,
            warning_count: 1,
            error_count: 1,
        });
    }

    #[test]
    fn update_progress_nonexistent() {
        let svc = NotificationService::new();
        svc.update_progress(9999, 50);
        assert_eq!(svc.count(), 0);
    }

    #[test]
    fn get_filtered_empty() {
        let svc = NotificationService::new();
        let filter = NotificationFilter::default();
        assert!(svc.get_filtered(&filter).is_empty());
    }
}
