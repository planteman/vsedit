//! User notification management.

use std::fmt;

/// Priority level for a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationPriority {
    Default,
    Silent,
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
#[derive(Debug, Clone)]
pub struct NotificationSource {
    pub id: String,
    pub label: String,
}

/// An action that can be attached to a notification.
#[derive(Debug, Clone)]
pub struct NotificationAction {
    pub label: String,
    pub id: String,
}

/// A workbench notification.
#[derive(Debug, Clone)]
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
}

impl Default for NotificationWorkbenchService {
    fn default() -> Self {
        Self::new()
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
}
