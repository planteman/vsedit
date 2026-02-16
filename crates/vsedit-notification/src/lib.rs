//! Notification service model.
//!
//! Equivalent to VS Code's `vs/platform/notification/common/notification.ts`.
//! Provides the data model for toast notifications, a service managing their
//! lifecycle (auto-dismiss, max visible, queueing), events, and rendering.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use vsedit_events::{Emitter, Event};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of visible toast notifications at once.
pub const MAX_VISIBLE: usize = 5;

/// Auto-dismiss duration for info notifications.
pub const INFO_DISMISS_SECS: u64 = 5;
/// Auto-dismiss duration for warning notifications.
pub const WARNING_DISMISS_SECS: u64 = 10;

// ---------------------------------------------------------------------------
// Model types
// ---------------------------------------------------------------------------

/// Notification severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSeverity {
    Info,
    Warning,
    Error,
}

impl NotificationSeverity {
    /// Returns the icon character for this severity.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Info => "ℹ",
            Self::Warning => "⚠",
            Self::Error => "✖",
        }
    }

    /// Auto-dismiss duration, if any.
    pub fn auto_dismiss_duration(&self) -> Option<Duration> {
        match self {
            Self::Info => Some(Duration::from_secs(INFO_DISMISS_SECS)),
            Self::Warning => Some(Duration::from_secs(WARNING_DISMISS_SECS)),
            Self::Error => None,
        }
    }
}

/// A notification action button.
#[derive(Debug, Clone)]
pub struct NotificationAction {
    pub label: String,
    pub id: String,
    pub is_primary: bool,
}

/// Progress state for a notification.
#[derive(Debug, Clone)]
pub struct NotificationProgress {
    pub infinite: bool,
    pub total: Option<u64>,
    pub worked: Option<u64>,
}

impl NotificationProgress {
    /// Returns the completion fraction in `0.0..=1.0`, or `None` for infinite.
    pub fn fraction(&self) -> Option<f64> {
        if self.infinite {
            return None;
        }
        match (self.total, self.worked) {
            (Some(t), Some(w)) if t > 0 => Some((w as f64 / t as f64).min(1.0)),
            _ => Some(0.0),
        }
    }

    /// Render a progress bar string like `[████░░░░░░] 40%`.
    pub fn render_bar(&self, width: usize) -> String {
        match self.fraction() {
            Some(frac) => {
                let filled = (frac * width as f64).round() as usize;
                let empty = width.saturating_sub(filled);
                let pct = (frac * 100.0).round() as u32;
                format!(
                    "[{}{}] {}%",
                    "█".repeat(filled),
                    "░".repeat(empty),
                    pct
                )
            }
            None => {
                format!("[{}]", "⣾".repeat(width.min(3)))
            }
        }
    }
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
    pub is_silent: bool,
    pub timestamp: Instant,
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
            is_silent: false,
            timestamp: Instant::now(),
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
            is_primary: self.actions.is_empty(),
        });
        self
    }

    pub fn with_sticky(mut self) -> Self {
        self.sticky = true;
        self
    }

    pub fn with_silent(mut self) -> Self {
        self.is_silent = true;
        self
    }

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

    /// Returns `true` if this notification should auto-dismiss.
    pub fn should_auto_dismiss(&self) -> bool {
        if self.sticky {
            return false;
        }
        self.severity.auto_dismiss_duration().is_some()
    }

    /// Returns `true` if the auto-dismiss timer has expired.
    pub fn is_expired(&self) -> bool {
        if !self.should_auto_dismiss() {
            return false;
        }
        if let Some(dur) = self.severity.auto_dismiss_duration() {
            self.timestamp.elapsed() >= dur
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// NotificationHandle
// ---------------------------------------------------------------------------

/// Handle returned from `NotificationService::show_*` that allows updating
/// or closing the associated notification.
#[derive(Clone)]
pub struct NotificationHandle {
    id: u64,
    inner: Arc<Mutex<NotificationServiceInner>>,
}

impl NotificationHandle {
    /// Update the message of this notification.
    pub fn update_message(&self, message: impl Into<String>) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(n) = inner.visible.iter_mut().find(|n| n.id == self.id) {
            n.message = message.into();
        }
    }

    /// Update progress on this notification.
    pub fn update_progress(&self, worked: u64) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(n) = inner.visible.iter_mut().find(|n| n.id == self.id) {
            if let Some(ref mut p) = n.progress {
                p.worked = Some(worked);
            }
        }
    }

    /// Close/dismiss this notification.
    pub fn close(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.remove(self.id);
    }

    /// Returns the notification ID.
    pub fn id(&self) -> u64 {
        self.id
    }
}

// ---------------------------------------------------------------------------
// Filter / Stats
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

struct NotificationServiceInner {
    visible: Vec<Notification>,
    queue: Vec<Notification>,
    on_added: Emitter<Notification>,
    on_dismissed: Emitter<u64>,
}

impl NotificationServiceInner {
    fn new() -> Self {
        Self {
            visible: Vec::new(),
            queue: Vec::new(),
            on_added: Emitter::new(),
            on_dismissed: Emitter::new(),
        }
    }

    fn push(&mut self, notification: Notification) {
        if self.visible.len() < MAX_VISIBLE {
            self.on_added.fire(&notification);
            self.visible.push(notification);
        } else {
            self.queue.push(notification);
        }
    }

    fn remove(&mut self, id: u64) {
        if let Some(pos) = self.visible.iter().position(|n| n.id == id) {
            self.visible.remove(pos);
            self.on_dismissed.fire(&id);
            if !self.queue.is_empty() && self.visible.len() < MAX_VISIBLE {
                let queued = self.queue.remove(0);
                self.on_added.fire(&queued);
                self.visible.push(queued);
            }
        } else {
            self.queue.retain(|n| n.id != id);
        }
    }
}

// ---------------------------------------------------------------------------
// NotificationService
// ---------------------------------------------------------------------------

/// Notification service that manages active notifications with auto-dismiss,
/// max visible count, queueing, and events.
pub struct NotificationService {
    inner: Arc<Mutex<NotificationServiceInner>>,
}

impl NotificationService {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(NotificationServiceInner::new())),
        }
    }

    /// Show an info notification. Returns a handle.
    pub fn show_info(
        &self,
        message: impl Into<String>,
        actions: Vec<NotificationAction>,
    ) -> NotificationHandle {
        let mut n = Notification::info(message);
        n.actions = actions;
        self.show(n)
    }

    /// Show a warning notification. Returns a handle.
    pub fn show_warning(
        &self,
        message: impl Into<String>,
        actions: Vec<NotificationAction>,
    ) -> NotificationHandle {
        let mut n = Notification::warning(message);
        n.actions = actions;
        self.show(n)
    }

    /// Show an error notification. Returns a handle.
    pub fn show_error(
        &self,
        message: impl Into<String>,
        actions: Vec<NotificationAction>,
    ) -> NotificationHandle {
        let mut n = Notification::error(message);
        n.actions = actions;
        self.show(n)
    }

    /// Show a progress notification. Returns handle.
    pub fn show_with_progress(
        &self,
        message: impl Into<String>,
        total: u64,
    ) -> NotificationHandle {
        let n = Notification::info(message).with_finite_progress(total);
        self.show(n)
    }

    /// Show an arbitrary notification. Returns a handle.
    pub fn show(&self, notification: Notification) -> NotificationHandle {
        let id = notification.id;
        self.notify(notification);
        NotificationHandle {
            id,
            inner: Arc::clone(&self.inner),
        }
    }

    /// Show a notification (original API, returns id).
    pub fn notify(&self, notification: Notification) -> u64 {
        let id = notification.id;
        self.inner.lock().unwrap().push(notification);
        id
    }

    /// Dismiss a notification by ID.
    pub fn dismiss(&self, id: u64) {
        self.inner.lock().unwrap().remove(id);
    }

    /// Dismiss all notifications.
    pub fn dismiss_all(&self) {
        let mut inner = self.inner.lock().unwrap();
        let ids: Vec<u64> = inner.visible.iter().map(|n| n.id).collect();
        for id in ids {
            inner.on_dismissed.fire(&id);
        }
        inner.visible.clear();
        inner.queue.clear();
    }

    /// Clear all notifications (alias for dismiss_all).
    pub fn clear(&self) {
        self.dismiss_all();
    }

    /// Get all active (visible) notifications.
    pub fn active_notifications(&self) -> Vec<Notification> {
        self.inner.lock().unwrap().visible.clone()
    }

    /// Get all active notifications (alias).
    pub fn get_notifications(&self) -> Vec<Notification> {
        self.active_notifications()
    }

    /// Get the count of visible notifications.
    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().visible.len()
    }

    /// Get the count of queued notifications.
    pub fn queue_count(&self) -> usize {
        self.inner.lock().unwrap().queue.len()
    }

    /// Tick the auto-dismiss timers, removing expired notifications.
    pub fn tick(&self) {
        let mut inner = self.inner.lock().unwrap();
        let expired: Vec<u64> = inner
            .visible
            .iter()
            .filter(|n| n.is_expired())
            .map(|n| n.id)
            .collect();
        drop(inner);
        for id in expired {
            self.dismiss(id);
        }
    }

    /// Event fired when a notification is added to visible list.
    pub fn on_notification_added(&self) -> Event<Notification> {
        self.inner.lock().unwrap().on_added.event()
    }

    /// Event fired when a notification is dismissed.
    pub fn on_notification_dismissed(&self) -> Event<u64> {
        self.inner.lock().unwrap().on_dismissed.event()
    }

    /// Update progress on an existing notification.
    pub fn update_progress(&self, id: u64, worked: u64) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(n) = inner.visible.iter_mut().find(|n| n.id == id) {
            if let Some(ref mut progress) = n.progress {
                progress.worked = Some(worked);
            }
        }
    }

    /// Get all notifications with a specific severity.
    pub fn get_by_severity(&self, severity: NotificationSeverity) -> Vec<Notification> {
        self.inner
            .lock()
            .unwrap()
            .visible
            .iter()
            .filter(|n| n.severity == severity)
            .cloned()
            .collect()
    }

    /// Get all notifications from a specific source.
    pub fn get_by_source(&self, source: &str) -> Vec<Notification> {
        self.inner
            .lock()
            .unwrap()
            .visible
            .iter()
            .filter(|n| n.source.as_deref() == Some(source))
            .cloned()
            .collect()
    }

    /// Check whether any active notification has `Error` severity.
    pub fn has_errors(&self) -> bool {
        self.inner
            .lock()
            .unwrap()
            .visible
            .iter()
            .any(|n| n.severity == NotificationSeverity::Error)
    }

    /// Dismiss all notifications from the given source.
    pub fn dismiss_by_source(&self, source: &str) {
        let mut inner = self.inner.lock().unwrap();
        let ids: Vec<u64> = inner
            .visible
            .iter()
            .filter(|n| n.source.as_deref() == Some(source))
            .map(|n| n.id)
            .collect();
        drop(inner);
        for id in ids {
            self.dismiss(id);
        }
    }

    /// Query notifications using a filter.
    pub fn get_filtered(&self, filter: &NotificationFilter) -> Vec<Notification> {
        self.inner
            .lock()
            .unwrap()
            .visible
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
        let inner = self.inner.lock().unwrap();
        let mut stats = NotificationStats {
            total: inner.visible.len(),
            info_count: 0,
            warning_count: 0,
            error_count: 0,
        };
        for n in inner.visible.iter() {
            match n.severity {
                NotificationSeverity::Info => stats.info_count += 1,
                NotificationSeverity::Warning => stats.warning_count += 1,
                NotificationSeverity::Error => stats.error_count += 1,
            }
        }
        stats
    }
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Rendering — toast overlays in the bottom-right corner
// ---------------------------------------------------------------------------

const TOAST_HEIGHT: u16 = 3;
const TOAST_WIDTH: u16 = 50;

/// Render notifications as toast overlays stacked from the bottom-right.
pub fn render_notifications(area: Rect, buf: &mut Buffer, notifications: &[Notification]) {
    if notifications.is_empty() || area.width < TOAST_WIDTH || area.height < TOAST_HEIGHT {
        return;
    }

    let max_toasts = (area.height / TOAST_HEIGHT).min(MAX_VISIBLE as u16) as usize;
    let to_render = &notifications[..notifications.len().min(max_toasts)];

    for (i, notif) in to_render.iter().enumerate() {
        let y_offset = area.height.saturating_sub((i as u16 + 1) * TOAST_HEIGHT);
        let x = area.x + area.width.saturating_sub(TOAST_WIDTH);
        let toast_area = Rect::new(x, area.y + y_offset, TOAST_WIDTH, TOAST_HEIGHT);
        render_single_toast(toast_area, buf, notif);
    }
}

fn render_single_toast(area: Rect, buf: &mut Buffer, notif: &Notification) {
    if area.width < 4 || area.height < 1 {
        return;
    }

    let bg = match notif.severity {
        NotificationSeverity::Info => Color::Blue,
        NotificationSeverity::Warning => Color::Yellow,
        NotificationSeverity::Error => Color::Red,
    };
    let fg = Color::White;

    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(Style::default().bg(bg).fg(fg));
                cell.set_char(' ');
            }
        }
    }

    let icon = notif.severity.icon();
    let content_y = area.y + (area.height > 2).then_some(1).unwrap_or(0);
    let content_x = area.x + 1;
    let max_width = (area.width as usize).saturating_sub(2);

    let line_text = format!("{} {}", icon, notif.message);
    let truncated = if line_text.len() > max_width {
        format!("{}…", &line_text[..max_width.saturating_sub(1)])
    } else {
        line_text
    };

    let line = Line::from(vec![Span::styled(
        truncated,
        Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD),
    )]);
    let line_area = Rect::new(content_x, content_y, area.width.saturating_sub(2), 1);
    line.render(line_area, buf);

    if let Some(ref progress) = notif.progress {
        let bar_y = content_y + 1;
        if bar_y < area.y + area.height {
            let bar_width = max_width.saturating_sub(2).min(20);
            let bar_str = progress.render_bar(bar_width);
            let bar_line = Line::from(vec![Span::styled(
                bar_str,
                Style::default().bg(bg).fg(fg),
            )]);
            let bar_area = Rect::new(content_x, bar_y, area.width.saturating_sub(2), 1);
            bar_line.render(bar_area, buf);
        }
    }

    if !notif.actions.is_empty() && notif.progress.is_none() {
        let actions_y = content_y + 1;
        if actions_y < area.y + area.height {
            let mut actions_str = String::new();
            for (i, action) in notif.actions.iter().enumerate() {
                if i > 0 {
                    actions_str.push_str("  ");
                }
                actions_str.push_str(&format!("[{}]", action.label));
            }
            if actions_str.len() > max_width {
                actions_str.truncate(max_width);
            }
            let action_line = Line::from(vec![Span::styled(
                actions_str,
                Style::default().bg(bg).fg(fg),
            )]);
            let action_area = Rect::new(content_x, actions_y, area.width.saturating_sub(2), 1);
            action_line.render(action_area, buf);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
        assert!(!n.should_auto_dismiss());
    }

    #[test]
    fn progress_builders() {
        let n = Notification::info("Installing...").with_progress(true);
        assert!(n.progress.as_ref().unwrap().infinite);

        let n2 = Notification::info("Downloading...").with_finite_progress(100);
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
        assert_eq!(
            stats,
            NotificationStats {
                total: 4,
                info_count: 2,
                warning_count: 1,
                error_count: 1,
            }
        );
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

    #[test]
    fn eq_notificationseverity_same() {
        assert_eq!(NotificationSeverity::Info, NotificationSeverity::Info);
    }

    #[test]
    fn ne_notificationseverity_diff() {
        assert_ne!(NotificationSeverity::Info, NotificationSeverity::Warning);
    }

    // -- New tests --

    #[test]
    fn is_silent_flag() {
        let n = Notification::info("quiet").with_silent();
        assert!(n.is_silent);
    }

    #[test]
    fn notification_has_timestamp() {
        let before = Instant::now();
        let n = Notification::info("timed");
        assert!(n.timestamp >= before);
        assert!(n.timestamp <= Instant::now());
    }

    #[test]
    fn action_is_primary() {
        let n = Notification::info("test")
            .with_action("a1", "First")
            .with_action("a2", "Second");
        assert!(n.actions[0].is_primary);
        assert!(!n.actions[1].is_primary);
    }

    #[test]
    fn max_visible_queue_overflow() {
        let svc = NotificationService::new();
        for i in 0..7 {
            svc.notify(Notification::info(format!("n{i}")));
        }
        assert_eq!(svc.count(), MAX_VISIBLE);
        assert_eq!(svc.queue_count(), 2);
    }

    #[test]
    fn dismiss_promotes_from_queue() {
        let svc = NotificationService::new();
        let mut ids = Vec::new();
        for i in 0..6 {
            ids.push(svc.notify(Notification::info(format!("n{i}"))));
        }
        assert_eq!(svc.count(), MAX_VISIBLE);
        assert_eq!(svc.queue_count(), 1);

        svc.dismiss(ids[0]);
        assert_eq!(svc.count(), MAX_VISIBLE);
        assert_eq!(svc.queue_count(), 0);
    }

    #[test]
    fn on_notification_added_event() {
        let svc = NotificationService::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = svc.on_notification_added().on(move |n: &Notification| {
            r.lock().unwrap().push(n.message.clone());
        });
        svc.notify(Notification::info("event test"));
        let msgs = received.lock().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0], "event test");
    }

    #[test]
    fn on_notification_dismissed_event() {
        let svc = NotificationService::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = svc.on_notification_dismissed().on(move |id: &u64| {
            r.lock().unwrap().push(*id);
        });
        let id = svc.notify(Notification::info("bye"));
        svc.dismiss(id);
        let ids = received.lock().unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], id);
    }

    #[test]
    fn show_info_returns_handle() {
        let svc = NotificationService::new();
        let handle = svc.show_info("hello handle", vec![]);
        assert_eq!(svc.count(), 1);
        handle.close();
        assert_eq!(svc.count(), 0);
    }

    #[test]
    fn show_warning_returns_handle() {
        let svc = NotificationService::new();
        let handle = svc.show_warning("warn handle", vec![]);
        assert_eq!(svc.count(), 1);
        let notifs = svc.active_notifications();
        assert_eq!(notifs[0].severity, NotificationSeverity::Warning);
        handle.close();
    }

    #[test]
    fn show_error_returns_handle() {
        let svc = NotificationService::new();
        let handle = svc.show_error("err handle", vec![]);
        assert_eq!(svc.count(), 1);
        let notifs = svc.active_notifications();
        assert_eq!(notifs[0].severity, NotificationSeverity::Error);
        handle.close();
    }

    #[test]
    fn handle_update_message() {
        let svc = NotificationService::new();
        let handle = svc.show_info("original", vec![]);
        handle.update_message("updated");
        let notifs = svc.active_notifications();
        assert_eq!(notifs[0].message, "updated");
    }

    #[test]
    fn handle_update_progress() {
        let svc = NotificationService::new();
        let handle = svc.show_with_progress("downloading", 100);
        handle.update_progress(50);
        let notifs = svc.active_notifications();
        let p = notifs[0].progress.as_ref().unwrap();
        assert_eq!(p.worked, Some(50));
    }

    #[test]
    fn progress_fraction() {
        let p = NotificationProgress {
            infinite: false,
            total: Some(100),
            worked: Some(40),
        };
        assert!((p.fraction().unwrap() - 0.4).abs() < 0.001);

        let inf = NotificationProgress {
            infinite: true,
            total: None,
            worked: None,
        };
        assert!(inf.fraction().is_none());
    }

    #[test]
    fn progress_render_bar() {
        let p = NotificationProgress {
            infinite: false,
            total: Some(100),
            worked: Some(40),
        };
        let bar = p.render_bar(10);
        assert!(bar.contains("40%"));
        assert!(bar.starts_with('['));
        assert!(bar.contains(']'));
    }

    #[test]
    fn severity_icon() {
        assert_eq!(NotificationSeverity::Info.icon(), "ℹ");
        assert_eq!(NotificationSeverity::Warning.icon(), "⚠");
        assert_eq!(NotificationSeverity::Error.icon(), "✖");
    }

    #[test]
    fn auto_dismiss_durations() {
        assert!(NotificationSeverity::Info.auto_dismiss_duration().is_some());
        assert!(NotificationSeverity::Warning.auto_dismiss_duration().is_some());
        assert!(NotificationSeverity::Error.auto_dismiss_duration().is_none());
    }

    #[test]
    fn error_does_not_auto_dismiss() {
        let n = Notification::error("stays");
        assert!(!n.should_auto_dismiss());
        assert!(!n.is_expired());
    }

    #[test]
    fn dismiss_all_clears_queue_too() {
        let svc = NotificationService::new();
        for i in 0..8 {
            svc.notify(Notification::info(format!("n{i}")));
        }
        assert!(svc.queue_count() > 0);
        svc.dismiss_all();
        assert_eq!(svc.count(), 0);
        assert_eq!(svc.queue_count(), 0);
    }

    #[test]
    fn render_notifications_empty() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        render_notifications(area, &mut buf, &[]);
    }

    #[test]
    fn render_notifications_single() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let notifs = vec![Notification::info("Test toast")];
        render_notifications(area, &mut buf, &notifs);
        let toast_y = area.height - TOAST_HEIGHT + 1;
        let cell = buf.cell((area.width - TOAST_WIDTH + 1, toast_y)).unwrap();
        assert_ne!(cell.symbol(), " ");
    }

    #[test]
    fn render_notifications_with_progress() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let notifs = vec![Notification::info("Downloading").with_finite_progress(100)];
        render_notifications(area, &mut buf, &notifs);
    }

    #[test]
    fn render_notifications_too_small() {
        let area = Rect::new(0, 0, 10, 2);
        let mut buf = Buffer::empty(area);
        let notifs = vec![Notification::info("Test")];
        render_notifications(area, &mut buf, &notifs);
    }

    #[test]
    fn handle_id_returns_correct_id() {
        let svc = NotificationService::new();
        let handle = svc.show_info("test", vec![]);
        let id = handle.id();
        let notifs = svc.active_notifications();
        assert_eq!(notifs[0].id, id);
    }
}
