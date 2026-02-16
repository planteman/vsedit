//! User notification management.

use std::fmt;

/// Errors that can occur during notification operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationError {
    /// Notification with the given ID was not found.
    NotFound(u64),
    /// The notification message was empty.
    EmptyMessage,
    /// Progress value was outside the valid 0.0..=1.0 range.
    InvalidProgress(String),
    /// The notification has already been closed.
    AlreadyClosed(u64),
}

impl fmt::Display for NotificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationError::NotFound(id) => write!(f, "notification {id} not found"),
            NotificationError::EmptyMessage => write!(f, "notification message must not be empty"),
            NotificationError::InvalidProgress(v) => {
                write!(f, "progress value {v} is outside 0.0..=1.0")
            }
            NotificationError::AlreadyClosed(id) => {
                write!(f, "notification {id} is already closed")
            }
        }
    }
}

impl std::error::Error for NotificationError {}

/// Priority level for a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotificationPriority {
    Silent,
    Default,
    Urgent,
}

impl fmt::Display for NotificationPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationPriority::Default => write!(f, "Default"),
            NotificationPriority::Silent => write!(f, "Silent"),
            NotificationPriority::Urgent => write!(f, "Urgent"),
        }
    }
}

/// Source of a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationSource {
    pub id: String,
    pub label: String,
}

impl fmt::Display for NotificationSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.label, self.id)
    }
}

/// An action that can be attached to a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationAction {
    pub label: String,
    pub id: String,
}

impl fmt::Display for NotificationAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.label)
    }
}

/// A workbench notification.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkbenchNotification {
    pub id: u64,
    pub message: String,
    pub priority: NotificationPriority,
    pub source: Option<NotificationSource>,
    pub progress: Option<f64>,
    pub closeable: bool,
    pub closed: bool,
    pub actions: Vec<NotificationAction>,
}

impl fmt::Display for WorkbenchNotification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.priority, self.message)
    }
}

impl WorkbenchNotification {
    /// Returns `true` if this notification is urgent.
    pub fn is_urgent(&self) -> bool {
        self.priority == NotificationPriority::Urgent
    }

    /// Returns `true` if the notification has at least one action.
    pub fn has_actions(&self) -> bool {
        !self.actions.is_empty()
    }

    /// Returns the progress percentage (0–100), or `None` if unset.
    pub fn progress_percent(&self) -> Option<u8> {
        self.progress.map(|p| (p.clamp(0.0, 1.0) * 100.0) as u8)
    }

    /// Returns `true` if progress has reached 1.0.
    pub fn is_complete(&self) -> bool {
        matches!(self.progress, Some(p) if p >= 1.0)
    }

    /// Finds an action by its id.
    pub fn find_action(&self, action_id: &str) -> Option<&NotificationAction> {
        self.actions.iter().find(|a| a.id == action_id)
    }

    /// Returns a summary string including source and progress info.
    pub fn summary(&self) -> String {
        let mut s = format!("[{}] {}", self.priority, self.message);
        if let Some(src) = &self.source {
            s.push_str(&format!(" (from {})", src));
        }
        if let Some(pct) = self.progress_percent() {
            s.push_str(&format!(" — {}%", pct));
        }
        if self.closed {
            s.push_str(" [closed]");
        }
        s
    }
}

/// Builder for constructing a `WorkbenchNotification` with validation.
#[derive(Debug, Clone)]
pub struct NotificationBuilder {
    message: String,
    priority: NotificationPriority,
    source: Option<NotificationSource>,
    actions: Vec<NotificationAction>,
    closeable: bool,
}

impl NotificationBuilder {
    /// Creates a new builder. `message` must not be empty.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            priority: NotificationPriority::Default,
            source: None,
            actions: Vec::new(),
            closeable: true,
        }
    }

    pub fn priority(mut self, priority: NotificationPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn source(mut self, source: NotificationSource) -> Self {
        self.source = Some(source);
        self
    }

    pub fn action(mut self, action: NotificationAction) -> Self {
        self.actions.push(action);
        self
    }

    pub fn closeable(mut self, closeable: bool) -> Self {
        self.closeable = closeable;
        self
    }

    /// Validates and sends the notification through the service, returning its id.
    pub fn send(
        self,
        service: &mut NotificationWorkbenchService,
    ) -> Result<u64, NotificationError> {
        if self.message.is_empty() {
            return Err(NotificationError::EmptyMessage);
        }
        let id = service.next_id;
        service.next_id += 1;
        service.notifications.push(WorkbenchNotification {
            id,
            message: self.message,
            priority: self.priority,
            source: self.source,
            progress: None,
            closeable: self.closeable,
            closed: false,
            actions: self.actions,
        });
        Ok(id)
    }
}

/// Service for managing workbench notifications.
pub struct NotificationWorkbenchService {
    notifications: Vec<WorkbenchNotification>,
    next_id: u64,
}

impl NotificationWorkbenchService {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            next_id: 1,
        }
    }

    pub fn notify(&mut self, message: impl Into<String>, priority: NotificationPriority) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.notifications.push(WorkbenchNotification {
            id,
            message: message.into(),
            priority,
            source: None,
            progress: None,
            closeable: true,
            closed: false,
            actions: Vec::new(),
        });
        id
    }

    pub fn notify_with_source(
        &mut self,
        message: impl Into<String>,
        priority: NotificationPriority,
        source: NotificationSource,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.notifications.push(WorkbenchNotification {
            id,
            message: message.into(),
            priority,
            source: Some(source),
            progress: None,
            closeable: true,
            closed: false,
            actions: Vec::new(),
        });
        id
    }

    pub fn notify_with_actions(
        &mut self,
        message: impl Into<String>,
        priority: NotificationPriority,
        actions: Vec<NotificationAction>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.notifications.push(WorkbenchNotification {
            id,
            message: message.into(),
            priority,
            source: None,
            progress: None,
            closeable: true,
            closed: false,
            actions,
        });
        id
    }

    pub fn get_notification(&self, id: u64) -> Option<&WorkbenchNotification> {
        self.notifications.iter().find(|n| n.id == id)
    }

    pub fn update_message(&mut self, id: u64, message: &str) {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.message = message.to_string();
        }
    }

    pub fn update_progress(&mut self, id: u64, progress: f64) {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.progress = Some(progress);
        }
    }

    pub fn close(&mut self, id: u64) {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.closed = true;
        }
    }

    pub fn get_active(&self) -> Vec<&WorkbenchNotification> {
        self.notifications.iter().filter(|n| !n.closed).collect()
    }

    pub fn has_notifications(&self) -> bool {
        self.notifications.iter().any(|n| !n.closed)
    }

    pub fn active_count(&self) -> usize {
        self.notifications.iter().filter(|n| !n.closed).count()
    }

    pub fn total_count(&self) -> usize {
        self.notifications.len()
    }

    pub fn get_by_priority(&self, priority: NotificationPriority) -> Vec<&WorkbenchNotification> {
        self.notifications
            .iter()
            .filter(|n| n.priority == priority)
            .collect()
    }

    pub fn close_all(&mut self) {
        for n in &mut self.notifications {
            n.closed = true;
        }
    }

    /// Validates progress is in 0.0..=1.0 before updating.
    pub fn update_progress_checked(
        &mut self,
        id: u64,
        progress: f64,
    ) -> Result<(), NotificationError> {
        if !(0.0..=1.0).contains(&progress) {
            return Err(NotificationError::InvalidProgress(progress.to_string()));
        }
        let n = self
            .notifications
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or(NotificationError::NotFound(id))?;
        if n.closed {
            return Err(NotificationError::AlreadyClosed(id));
        }
        n.progress = Some(progress);
        Ok(())
    }

    /// Close a notification, returning an error if not found or already closed.
    pub fn close_checked(&mut self, id: u64) -> Result<(), NotificationError> {
        let n = self
            .notifications
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or(NotificationError::NotFound(id))?;
        if n.closed {
            return Err(NotificationError::AlreadyClosed(id));
        }
        n.closed = true;
        Ok(())
    }

    /// Returns the highest-priority active notification, if any.
    pub fn most_urgent(&self) -> Option<&WorkbenchNotification> {
        self.notifications
            .iter()
            .filter(|n| !n.closed)
            .max_by_key(|n| n.priority)
    }

    /// Removes all closed notifications from storage, returning how many were removed.
    pub fn purge_closed(&mut self) -> usize {
        let before = self.notifications.len();
        self.notifications.retain(|n| !n.closed);
        before - self.notifications.len()
    }

    /// Returns active notifications from a specific source id.
    pub fn get_by_source(&self, source_id: &str) -> Vec<&WorkbenchNotification> {
        self.notifications
            .iter()
            .filter(|n| {
                !n.closed && n.source.as_ref().map_or(false, |s| s.id == source_id)
            })
            .collect()
    }
}

impl Default for NotificationWorkbenchService {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for wb-notification operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbNotificationStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbNotificationStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &WbNotificationStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for WbNotificationStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbNotificationStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbNotificationStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-notification.
#[derive(Debug, Clone)]
pub struct WbNotificationValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbNotificationValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for WbNotificationValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Notification stack management
// ---------------------------------------------------------------------------

/// A stack of notifications with ordering and filtering.
#[derive(Debug, Default)]
pub struct NotificationStack {
    notifications: Vec<WorkbenchNotification>,
    max_visible: usize,
}

impl NotificationStack {
    /// Create a new notification stack with a maximum number of visible notifications.
    pub fn new(max_visible: usize) -> Self {
        Self {
            notifications: Vec::new(),
            max_visible,
        }
    }

    /// Push a notification onto the stack.
    pub fn push(&mut self, notification: WorkbenchNotification) {
        self.notifications.push(notification);
    }

    /// Remove a notification by its ID. Returns `true` if found and removed.
    pub fn remove(&mut self, id: u64) -> bool {
        let len = self.notifications.len();
        self.notifications.retain(|n| n.id != id);
        self.notifications.len() != len
    }

    /// Close a notification by its ID without removing it.
    pub fn close(&mut self, id: u64) -> bool {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.closed = true;
            true
        } else {
            false
        }
    }

    /// Return the visible (non-closed) notifications, limited to `max_visible`.
    /// Urgent notifications appear first.
    pub fn visible(&self) -> Vec<&WorkbenchNotification> {
        let mut active: Vec<&WorkbenchNotification> = self.notifications
            .iter()
            .filter(|n| !n.closed)
            .collect();
        // Sort: Urgent first, then Default, then Silent
        active.sort_by(|a, b| b.priority.cmp(&a.priority));
        active.truncate(self.max_visible);
        active
    }

    /// Total number of notifications (including closed).
    pub fn total(&self) -> usize {
        self.notifications.len()
    }

    /// Number of active (non-closed) notifications.
    pub fn active_count(&self) -> usize {
        self.notifications.iter().filter(|n| !n.closed).count()
    }

    /// Remove all closed notifications.
    pub fn prune_closed(&mut self) {
        self.notifications.retain(|n| !n.closed);
    }

    /// Close all notifications.
    pub fn close_all(&mut self) {
        for n in &mut self.notifications {
            n.closed = true;
        }
    }

    /// Check if a notification with the given ID exists in the stack.
    pub fn contains(&self, id: u64) -> bool {
        self.notifications.iter().any(|n| n.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_and_close() {
        let mut svc = NotificationWorkbenchService::new();
        let id = svc.notify("hello", NotificationPriority::Default);
        assert!(svc.has_notifications());
        assert_eq!(svc.get_active().len(), 1);
        svc.close(id);
        assert!(!svc.has_notifications());
    }

    #[test]
    fn update_progress() {
        let mut svc = NotificationWorkbenchService::new();
        let id = svc.notify("building", NotificationPriority::Silent);
        svc.update_progress(id, 0.5);
        let n = svc.get_active()[0];
        assert_eq!(n.progress, Some(0.5));
    }

    #[test]
    fn close_all() {
        let mut svc = NotificationWorkbenchService::new();
        svc.notify("a", NotificationPriority::Default);
        svc.notify("b", NotificationPriority::Urgent);
        assert_eq!(svc.get_active().len(), 2);
        svc.close_all();
        assert!(svc.get_active().is_empty());
    }

    #[test]
    fn notify_with_source() {
        let mut svc = NotificationWorkbenchService::new();
        let src = NotificationSource {
            id: "ext.test".into(),
            label: "Test Extension".into(),
        };
        let id = svc.notify_with_source("from source", NotificationPriority::Default, src);
        let n = svc.get_notification(id).unwrap();
        assert_eq!(n.source.as_ref().unwrap().id, "ext.test");
        assert_eq!(n.message, "from source");
    }

    #[test]
    fn notify_with_actions() {
        let mut svc = NotificationWorkbenchService::new();
        let actions = vec![
            NotificationAction { label: "Retry".into(), id: "retry".into() },
            NotificationAction { label: "Cancel".into(), id: "cancel".into() },
        ];
        let id = svc.notify_with_actions("failed", NotificationPriority::Urgent, actions);
        let n = svc.get_notification(id).unwrap();
        assert_eq!(n.actions.len(), 2);
        assert_eq!(n.actions[0].id, "retry");
    }

    #[test]
    fn update_message() {
        let mut svc = NotificationWorkbenchService::new();
        let id = svc.notify("old", NotificationPriority::Default);
        svc.update_message(id, "new");
        assert_eq!(svc.get_notification(id).unwrap().message, "new");
    }

    #[test]
    fn active_and_total_count() {
        let mut svc = NotificationWorkbenchService::new();
        svc.notify("a", NotificationPriority::Default);
        let id = svc.notify("b", NotificationPriority::Silent);
        svc.notify("c", NotificationPriority::Urgent);
        assert_eq!(svc.active_count(), 3);
        assert_eq!(svc.total_count(), 3);
        svc.close(id);
        assert_eq!(svc.active_count(), 2);
        assert_eq!(svc.total_count(), 3);
    }

    #[test]
    fn get_by_priority() {
        let mut svc = NotificationWorkbenchService::new();
        svc.notify("a", NotificationPriority::Default);
        svc.notify("b", NotificationPriority::Urgent);
        svc.notify("c", NotificationPriority::Default);
        let defaults = svc.get_by_priority(NotificationPriority::Default);
        assert_eq!(defaults.len(), 2);
        let urgent = svc.get_by_priority(NotificationPriority::Urgent);
        assert_eq!(urgent.len(), 1);
        assert_eq!(urgent[0].message, "b");
    }

    #[test]
    fn display_priority() {
        assert_eq!(format!("{}", NotificationPriority::Default), "Default");
        assert_eq!(format!("{}", NotificationPriority::Silent), "Silent");
        assert_eq!(format!("{}", NotificationPriority::Urgent), "Urgent");
    }

    #[test]
    fn display_notification() {
        let mut svc = NotificationWorkbenchService::new();
        let id = svc.notify("something happened", NotificationPriority::Urgent);
        let n = svc.get_notification(id).unwrap();
        assert_eq!(format!("{}", n), "[Urgent] something happened");
    }

    #[test]
    fn builder_sends_notification() {
        let mut svc = NotificationWorkbenchService::new();
        let id = NotificationBuilder::new("build started")
            .priority(NotificationPriority::Silent)
            .closeable(false)
            .send(&mut svc)
            .unwrap();
        let n = svc.get_notification(id).unwrap();
        assert_eq!(n.message, "build started");
        assert_eq!(n.priority, NotificationPriority::Silent);
        assert!(!n.closeable);
    }

    #[test]
    fn builder_rejects_empty_message() {
        let mut svc = NotificationWorkbenchService::new();
        let result = NotificationBuilder::new("").send(&mut svc);
        assert_eq!(result, Err(NotificationError::EmptyMessage));
        assert_eq!(svc.total_count(), 0);
    }

    #[test]
    fn builder_with_source_and_action() {
        let mut svc = NotificationWorkbenchService::new();
        let id = NotificationBuilder::new("lint warning")
            .priority(NotificationPriority::Default)
            .source(NotificationSource {
                id: "linter".into(),
                label: "Linter".into(),
            })
            .action(NotificationAction {
                label: "Fix".into(),
                id: "fix".into(),
            })
            .send(&mut svc)
            .unwrap();
        let n = svc.get_notification(id).unwrap();
        assert_eq!(n.source.as_ref().unwrap().label, "Linter");
        assert_eq!(n.actions.len(), 1);
    }

    #[test]
    fn update_progress_checked_validates_range() {
        let mut svc = NotificationWorkbenchService::new();
        let id = svc.notify("task", NotificationPriority::Default);
        assert!(svc.update_progress_checked(id, 0.5).is_ok());
        assert!(svc.update_progress_checked(id, 1.5).is_err());
        assert!(svc.update_progress_checked(id, -0.1).is_err());
    }

    #[test]
    fn update_progress_checked_not_found() {
        let mut svc = NotificationWorkbenchService::new();
        assert_eq!(
            svc.update_progress_checked(999, 0.5),
            Err(NotificationError::NotFound(999))
        );
    }

    #[test]
    fn close_checked_errors() {
        let mut svc = NotificationWorkbenchService::new();
        assert_eq!(
            svc.close_checked(42),
            Err(NotificationError::NotFound(42))
        );
        let id = svc.notify("x", NotificationPriority::Default);
        assert!(svc.close_checked(id).is_ok());
        assert_eq!(
            svc.close_checked(id),
            Err(NotificationError::AlreadyClosed(id))
        );
    }

    #[test]
    fn most_urgent_returns_highest_priority() {
        let mut svc = NotificationWorkbenchService::new();
        svc.notify("low", NotificationPriority::Silent);
        svc.notify("high", NotificationPriority::Urgent);
        svc.notify("mid", NotificationPriority::Default);
        let top = svc.most_urgent().unwrap();
        assert_eq!(top.message, "high");
    }

    #[test]
    fn purge_closed_removes_only_closed() {
        let mut svc = NotificationWorkbenchService::new();
        let a = svc.notify("a", NotificationPriority::Default);
        svc.notify("b", NotificationPriority::Default);
        svc.close(a);
        let purged = svc.purge_closed();
        assert_eq!(purged, 1);
        assert_eq!(svc.total_count(), 1);
        assert_eq!(svc.active_count(), 1);
    }

    #[test]
    fn get_by_source_filters_correctly() {
        let mut svc = NotificationWorkbenchService::new();
        let src = NotificationSource { id: "ext.a".into(), label: "A".into() };
        svc.notify_with_source("from a", NotificationPriority::Default, src.clone());
        svc.notify("no source", NotificationPriority::Default);
        let src_b = NotificationSource { id: "ext.b".into(), label: "B".into() };
        svc.notify_with_source("from b", NotificationPriority::Default, src_b);
        let results = svc.get_by_source("ext.a");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message, "from a");
    }

    #[test]
    fn notification_helper_methods() {
        let mut svc = NotificationWorkbenchService::new();
        let actions = vec![NotificationAction { label: "Ok".into(), id: "ok".into() }];
        let id = svc.notify_with_actions("err", NotificationPriority::Urgent, actions);
        let n = svc.get_notification(id).unwrap();
        assert!(n.is_urgent());
        assert!(n.has_actions());
        assert!(n.find_action("ok").is_some());
        assert!(n.find_action("missing").is_none());
        assert_eq!(n.progress_percent(), None);
    }

    #[test]
    fn progress_percent_and_completion() {
        let mut svc = NotificationWorkbenchService::new();
        let id = svc.notify("dl", NotificationPriority::Default);
        svc.update_progress(id, 0.75);
        let n = svc.get_notification(id).unwrap();
        assert_eq!(n.progress_percent(), Some(75));
        assert!(!n.is_complete());
        svc.update_progress(id, 1.0);
        let n = svc.get_notification(id).unwrap();
        assert!(n.is_complete());
    }

    #[test]
    fn notification_summary_formatting() {
        let mut svc = NotificationWorkbenchService::new();
        let src = NotificationSource { id: "ci".into(), label: "CI".into() };
        let id = svc.notify_with_source("building", NotificationPriority::Default, src);
        svc.update_progress(id, 0.5);
        let n = svc.get_notification(id).unwrap();
        let s = n.summary();
        assert!(s.contains("building"));
        assert!(s.contains("CI (ci)"));
        assert!(s.contains("50%"));
    }

    #[test]
    fn error_display_messages() {
        let e1 = NotificationError::NotFound(7);
        assert_eq!(e1.to_string(), "notification 7 not found");
        let e2 = NotificationError::EmptyMessage;
        assert!(e2.to_string().contains("empty"));
        let e3 = NotificationError::InvalidProgress("2.0".into());
        assert!(e3.to_string().contains("2.0"));
        let e4 = NotificationError::AlreadyClosed(3);
        assert!(e4.to_string().contains("already closed"));
    }

    #[test]
    fn display_source_and_action() {
        let src = NotificationSource { id: "x".into(), label: "X Ext".into() };
        assert_eq!(format!("{src}"), "X Ext (x)");
        let act = NotificationAction { id: "a".into(), label: "Apply".into() };
        assert_eq!(format!("{act}"), "[Apply]");
    }

    #[test]
    fn priority_ordering() {
        assert!(NotificationPriority::Silent < NotificationPriority::Default);
        assert!(NotificationPriority::Default < NotificationPriority::Urgent);
    }

    #[test]
    fn wb_notification_stats_new_defaults() {
        let stats = WbNotificationStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_notification_stats_record_success() {
        let mut stats = WbNotificationStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_notification_stats_record_failure() {
        let mut stats = WbNotificationStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_notification_stats_reset() {
        let mut stats = WbNotificationStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_notification_stats_merge() {
        let mut a = WbNotificationStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbNotificationStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn wb_notification_stats_display() {
        let mut stats = WbNotificationStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_notification_stats_default() {
        let stats = WbNotificationStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_notification_validator_accepts_valid_name() {
        let v = WbNotificationValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_notification_validator_rejects_empty() {
        let v = WbNotificationValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_notification_validator_rejects_too_long() {
        let v = WbNotificationValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_notification_validator_forbidden_prefix() {
        let v = WbNotificationValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_notification_validator_allowed_chars() {
        let v = WbNotificationValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_notification_validator_range() {
        let v = WbNotificationValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_notification_sanitize_removes_control() {
        let result = WbNotificationValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_notification_truncate_short_string() {
        assert_eq!(WbNotificationValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_notification_truncate_long_string() {
        let result = WbNotificationValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_notification_is_ascii_printable() {
        assert!(WbNotificationValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbNotificationValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn notification_stack_push_and_count() {
        let mut stack = NotificationStack::new(5);
        stack.push(WorkbenchNotification {
            id: 1,
            message: "Hello".to_string(),
            priority: NotificationPriority::Default,
            source: None,
            actions: vec![],
            progress: None,
            closeable: true,
            closed: false,
        });
        assert_eq!(stack.total(), 1);
        assert_eq!(stack.active_count(), 1);
    }

    #[test]
    fn notification_stack_remove() {
        let mut stack = NotificationStack::new(5);
        stack.push(WorkbenchNotification {
            id: 1, message: "A".to_string(), priority: NotificationPriority::Default,
            source: None, actions: vec![], progress: None, closeable: true, closed: false,
        });
        assert!(stack.remove(1));
        assert_eq!(stack.total(), 0);
        assert!(!stack.remove(999));
    }

    #[test]
    fn notification_stack_close() {
        let mut stack = NotificationStack::new(5);
        stack.push(WorkbenchNotification {
            id: 1, message: "A".to_string(), priority: NotificationPriority::Default,
            source: None, actions: vec![], progress: None, closeable: true, closed: false,
        });
        stack.close(1);
        assert_eq!(stack.active_count(), 0);
        assert_eq!(stack.total(), 1);
    }

    #[test]
    fn notification_stack_visible_priority_order() {
        let mut stack = NotificationStack::new(5);
        stack.push(WorkbenchNotification {
            id: 1, message: "Low".to_string(), priority: NotificationPriority::Silent,
            source: None, actions: vec![], progress: None, closeable: true, closed: false,
        });
        stack.push(WorkbenchNotification {
            id: 2, message: "High".to_string(), priority: NotificationPriority::Urgent,
            source: None, actions: vec![], progress: None, closeable: true, closed: false,
        });
        let visible = stack.visible();
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].priority, NotificationPriority::Urgent);
    }

    #[test]
    fn notification_stack_max_visible() {
        let mut stack = NotificationStack::new(2);
        for i in 0..5 {
            stack.push(WorkbenchNotification {
                id: i, message: format!("N{i}"), priority: NotificationPriority::Default,
                source: None, actions: vec![], progress: None, closeable: true, closed: false,
            });
        }
        assert_eq!(stack.visible().len(), 2);
        assert_eq!(stack.active_count(), 5);
    }

    #[test]
    fn notification_stack_prune_closed() {
        let mut stack = NotificationStack::new(5);
        stack.push(WorkbenchNotification {
            id: 1, message: "A".to_string(), priority: NotificationPriority::Default,
            source: None, actions: vec![], progress: None, closeable: true, closed: true,
        });
        stack.push(WorkbenchNotification {
            id: 2, message: "B".to_string(), priority: NotificationPriority::Default,
            source: None, actions: vec![], progress: None, closeable: true, closed: false,
        });
        stack.prune_closed();
        assert_eq!(stack.total(), 1);
        assert!(stack.contains(2));
    }

    #[test]
    fn notification_stack_close_all() {
        let mut stack = NotificationStack::new(5);
        for i in 0..3 {
            stack.push(WorkbenchNotification {
                id: i, message: format!("N{i}"), priority: NotificationPriority::Default,
                source: None, actions: vec![], progress: None, closeable: true, closed: false,
            });
        }
        stack.close_all();
        assert_eq!(stack.active_count(), 0);
    }
}
