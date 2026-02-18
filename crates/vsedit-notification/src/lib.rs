//! Notification service model.
//!
//! Equivalent to VS Code's `vs/platform/notification/common/notification.ts`.
//! Provides the data model for toast notifications, a service managing their
//! lifecycle (auto-dismiss, max visible, queueing), events, and rendering.

use std::fmt;
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
// Notification throttling
// ---------------------------------------------------------------------------

/// A rate-limiter that prevents duplicate or too-frequent notifications.
///
/// It tracks recently shown notification messages and suppresses repeats
/// within a configurable time window.
pub struct NotificationThrottle {
    /// Maps message text → timestamp of last display.
    recent: std::collections::HashMap<String, Instant>,
    /// Minimum time between identical notifications.
    cooldown: Duration,
}

impl NotificationThrottle {
    pub fn new(cooldown: Duration) -> Self {
        Self {
            recent: std::collections::HashMap::new(),
            cooldown,
        }
    }

    /// Check if a notification with the given message should be shown.
    /// Returns `true` if enough time has elapsed since the last identical message.
    pub fn should_show(&mut self, message: &str, now: Instant) -> bool {
        if let Some(last) = self.recent.get(message) {
            if now.duration_since(*last) < self.cooldown {
                return false;
            }
        }
        self.recent.insert(message.to_string(), now);
        true
    }

    /// Filter a list of notifications, keeping only those that pass the throttle.
    pub fn filter_notifications<'a>(&mut self, notifications: &'a [Notification], now: Instant) -> Vec<&'a Notification> {
        notifications
            .iter()
            .filter(|n| self.should_show(&n.message, now))
            .collect()
    }

    /// Remove entries older than the cooldown period to prevent memory growth.
    pub fn evict_stale(&mut self, now: Instant) {
        self.recent.retain(|_, ts| now.duration_since(*ts) < self.cooldown);
    }

    /// Number of messages currently tracked.
    pub fn tracked_count(&self) -> usize {
        self.recent.len()
    }

    /// Clear all tracked messages.
    pub fn reset(&mut self) {
        self.recent.clear();
    }
}

// ---------------------------------------------------------------------------
// Notification — additional helpers
// ---------------------------------------------------------------------------

impl Notification {
    /// Returns `true` if this notification has progress information attached.
    pub fn is_progress(&self) -> bool {
        self.progress.is_some()
    }

    /// Returns the age of this notification in seconds relative to `now`.
    pub fn age(&self, now: Instant) -> Duration {
        now.duration_since(self.timestamp)
    }
}

// ---------------------------------------------------------------------------
// NotificationSeverity — additional helpers
// ---------------------------------------------------------------------------

impl NotificationSeverity {
    /// Returns `true` for `Error` severity.
    pub fn is_critical(&self) -> bool {
        matches!(self, Self::Error)
    }

    /// Returns a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

// ---------------------------------------------------------------------------
// NotificationFilter — match helper
// ---------------------------------------------------------------------------

impl NotificationFilter {
    /// Returns `true` if `notification` satisfies every criterion in this filter.
    pub fn matches(&self, notification: &Notification) -> bool {
        if let Some(sev) = self.severity {
            if notification.severity != sev {
                return false;
            }
        }
        if let Some(ref src) = self.source {
            if notification.source.as_deref() != Some(src.as_str()) {
                return false;
            }
        }
        if self.sticky_only && !notification.sticky {
            return false;
        }
        true
    }
}

// ---------------------------------------------------------------------------
// NotificationSeverityCount
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationSeverityCount {
    pub info: usize,
    pub warning: usize,
    pub error: usize,
}

// ---------------------------------------------------------------------------
// NotificationService — additional helpers
// ---------------------------------------------------------------------------

impl NotificationService {
    /// Returns per-severity counts of the visible notifications.
    pub fn count_by_severity(&self) -> NotificationSeverityCount {
        let inner = self.inner.lock().unwrap();
        let mut counts = NotificationSeverityCount {
            info: 0,
            warning: 0,
            error: 0,
        };
        for n in inner.visible.iter() {
            match n.severity {
                NotificationSeverity::Info => counts.info += 1,
                NotificationSeverity::Warning => counts.warning += 1,
                NotificationSeverity::Error => counts.error += 1,
            }
        }
        counts
    }

    /// Number of visible (active) notifications. Alias kept for clarity.
    pub fn active_count(&self) -> usize {
        self.inner.lock().unwrap().visible.len()
    }

    /// Total number of notifications (visible + queued).
    pub fn total_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.visible.len() + inner.queue.len()
    }

    /// Returns the most common severity among visible notifications, or `None`
    /// if the service is empty.
    pub fn most_common_severity(&self) -> Option<NotificationSeverity> {
        let counts = self.count_by_severity();
        if counts.info == 0 && counts.warning == 0 && counts.error == 0 {
            return None;
        }
        if counts.error >= counts.warning && counts.error >= counts.info {
            Some(NotificationSeverity::Error)
        } else if counts.warning >= counts.info {
            Some(NotificationSeverity::Warning)
        } else {
            Some(NotificationSeverity::Info)
        }
    }
}

// ---------------------------------------------------------------------------
// Display for NotificationProgress
// ---------------------------------------------------------------------------

impl std::fmt::Display for NotificationProgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.infinite {
            return write!(f, "in progress…");
        }
        match (self.total, self.worked) {
            (Some(total), Some(worked)) if total > 0 => {
                let pct = ((worked as f64 / total as f64) * 100.0).round() as u32;
                write!(f, "{worked}/{total} ({pct}%)")
            }
            _ => write!(f, "0%"),
        }
    }
}

// ---------------------------------------------------------------------------
// NotificationBatch — bulk operations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NotificationBatch {
    items: Vec<Notification>,
}

impl NotificationBatch {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(&mut self, notification: Notification) {
        self.items.push(notification);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn severities(&self) -> Vec<NotificationSeverity> {
        self.items.iter().map(|n| n.severity).collect()
    }

    pub fn drain(self) -> Vec<Notification> {
        self.items
    }

    pub fn send_all(self, service: &NotificationService) -> Vec<u64> {
        self.items
            .into_iter()
            .map(|n| service.notify(n))
            .collect()
    }
}

impl Default for NotificationBatch {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoIterator for NotificationBatch {
    type Item = Notification;
    type IntoIter = std::vec::IntoIter<Notification>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

// ---------------------------------------------------------------------------
// NotificationHistory — bounded history of dismissed notifications
// ---------------------------------------------------------------------------

/// A bounded history buffer that records dismissed notifications.
///
/// When the history reaches `capacity`, the oldest entry is evicted to make
/// room for the new one (ring-buffer semantics).
pub struct NotificationHistory {
    entries: Vec<Notification>,
    capacity: usize,
}

impl NotificationHistory {
    /// Create a new history with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Record a notification in the history, evicting the oldest if full.
    pub fn record(&mut self, notification: Notification) {
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(notification);
    }

    /// Return all entries oldest-first.
    pub fn entries(&self) -> &[Notification] {
        &self.entries
    }

    /// Number of recorded entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all history entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Search history entries whose message contains `needle` (case-insensitive).
    pub fn search(&self, needle: &str) -> Vec<&Notification> {
        let lower = needle.to_lowercase();
        self.entries
            .iter()
            .filter(|n| n.message.to_lowercase().contains(&lower))
            .collect()
    }

    /// Return entries filtered by severity.
    pub fn by_severity(&self, severity: NotificationSeverity) -> Vec<&Notification> {
        self.entries
            .iter()
            .filter(|n| n.severity == severity)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// DoNotDisturb — suppress non-critical notifications
// ---------------------------------------------------------------------------

/// Do-not-disturb mode that suppresses notifications below a severity
/// threshold.
///
/// When enabled, only notifications at or above `min_severity` are allowed
/// through. The suppressed count is tracked so the UI can show a badge.
pub struct DoNotDisturb {
    enabled: bool,
    min_severity: NotificationSeverity,
    suppressed_count: usize,
}

impl DoNotDisturb {
    /// Create a new DND controller (initially disabled).
    pub fn new() -> Self {
        Self {
            enabled: false,
            min_severity: NotificationSeverity::Error,
            suppressed_count: 0,
        }
    }

    /// Enable do-not-disturb. Only notifications with severity >= `min_severity`
    /// will be allowed through.
    pub fn enable(&mut self, min_severity: NotificationSeverity) {
        self.enabled = true;
        self.min_severity = min_severity;
        self.suppressed_count = 0;
    }

    /// Disable do-not-disturb mode.
    pub fn disable(&mut self) {
        self.enabled = false;
        self.suppressed_count = 0;
    }

    /// Whether DND is currently active.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Number of notifications suppressed since DND was enabled.
    pub fn suppressed_count(&self) -> usize {
        self.suppressed_count
    }

    /// Returns `true` if the notification should be shown (passes DND filter).
    pub fn should_show(&mut self, notification: &Notification) -> bool {
        if !self.enabled {
            return true;
        }
        let dominated = match self.min_severity {
            NotificationSeverity::Info => true, // everything passes
            NotificationSeverity::Warning => matches!(
                notification.severity,
                NotificationSeverity::Warning | NotificationSeverity::Error
            ),
            NotificationSeverity::Error => {
                notification.severity == NotificationSeverity::Error
            }
        };
        if !dominated {
            self.suppressed_count += 1;
        }
        dominated
    }
}

impl Default for DoNotDisturb {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// NotificationDeduplicator — collapse identical messages
// ---------------------------------------------------------------------------

/// Deduplicates notifications by message text.
///
/// When a duplicate is detected, instead of showing a new toast the
/// deduplicator increments an occurrence counter and updates the timestamp.
pub struct NotificationDeduplicator {
    /// Maps message → (first notification id, occurrence count).
    seen: std::collections::HashMap<String, (u64, usize)>,
}

impl NotificationDeduplicator {
    pub fn new() -> Self {
        Self {
            seen: std::collections::HashMap::new(),
        }
    }

    /// Check whether `notification` is a duplicate. Returns `None` if it is
    /// new, or `Some((original_id, new_count))` if it has been seen before.
    pub fn check(&mut self, notification: &Notification) -> Option<(u64, usize)> {
        if let Some(entry) = self.seen.get_mut(&notification.message) {
            entry.1 += 1;
            Some((entry.0, entry.1))
        } else {
            self.seen
                .insert(notification.message.clone(), (notification.id, 1));
            None
        }
    }

    /// Number of unique messages tracked.
    pub fn unique_count(&self) -> usize {
        self.seen.len()
    }

    /// Total occurrences across all tracked messages.
    pub fn total_occurrences(&self) -> usize {
        self.seen.values().map(|(_, c)| c).sum()
    }

    /// Reset all tracking state.
    pub fn reset(&mut self) {
        self.seen.clear();
    }
}

impl Default for NotificationDeduplicator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// NotificationQueue – priority-ordered notification queue
// ---------------------------------------------------------------------------

/// A priority queue of notifications.
///
/// Error notifications have highest priority, then warnings, then info.
#[derive(Debug)]
pub struct NotificationQueue {
    queue: Vec<Notification>,
    max_size: usize,
}

impl Default for NotificationQueue {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            max_size: 100,
        }
    }
}

impl NotificationQueue {
    /// Create a queue with the given maximum size.
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: Vec::new(),
            max_size,
        }
    }

    /// Enqueue a notification, maintaining priority order.
    pub fn push(&mut self, notification: Notification) {
        if self.queue.len() >= self.max_size {
            // Drop lowest-priority (last) item
            self.queue.pop();
        }
        self.queue.push(notification);
        self.queue.sort_by_key(|n| std::cmp::Reverse(Self::priority(&n.severity)));
    }

    /// Dequeue the highest-priority notification.
    pub fn pop(&mut self) -> Option<Notification> {
        if self.queue.is_empty() {
            None
        } else {
            Some(self.queue.remove(0))
        }
    }

    /// Peek at the highest-priority notification.
    pub fn peek(&self) -> Option<&Notification> {
        self.queue.first()
    }

    /// Number of queued notifications.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Clear all notifications.
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    fn priority(severity: &NotificationSeverity) -> u8 {
        match severity {
            NotificationSeverity::Error => 3,
            NotificationSeverity::Warning => 2,
            NotificationSeverity::Info => 1,
        }
    }
}

// ---------------------------------------------------------------------------
// NotificationGroup – collapse similar notifications
// ---------------------------------------------------------------------------

/// Groups similar notifications together.
///
/// Notifications with the same source are collapsed into a single
/// group with a count.
#[derive(Debug, Clone)]
pub struct NotificationGroup {
    /// The representative notification.
    pub representative: String,
    /// Source identifier for grouping.
    pub source: String,
    /// Number of collapsed notifications.
    pub count: usize,
    /// Severity of the most severe notification in the group.
    pub max_severity: NotificationSeverity,
}

impl NotificationGroup {
    /// Create a new group from a notification.
    pub fn from_notification(notification: &Notification) -> Self {
        Self {
            representative: notification.message.clone(),
            source: notification.source.clone().unwrap_or_default(),
            count: 1,
            max_severity: notification.severity,
        }
    }

    /// Try to absorb a notification into this group.
    ///
    /// Returns `true` if the notification was absorbed.
    pub fn try_absorb(&mut self, notification: &Notification) -> bool {
        let source = notification.source.as_deref().unwrap_or("");
        if source == self.source && !self.source.is_empty() {
            self.count += 1;
            if Self::severity_ord(&notification.severity) > Self::severity_ord(&self.max_severity) {
                self.max_severity = notification.severity;
            }
            true
        } else {
            false
        }
    }

    fn severity_ord(s: &NotificationSeverity) -> u8 {
        match s {
            NotificationSeverity::Info => 0,
            NotificationSeverity::Warning => 1,
            NotificationSeverity::Error => 2,
        }
    }
}

impl std::fmt::Display for NotificationGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.count > 1 {
            write!(f, "{} (+{})", self.representative, self.count - 1)
        } else {
            write!(f, "{}", self.representative)
        }
    }
}

/// Groups a slice of notifications by source.
pub fn group_notifications(notifications: &[Notification]) -> Vec<NotificationGroup> {
    let mut groups: Vec<NotificationGroup> = Vec::new();
    for n in notifications {
        let absorbed = groups.iter_mut().any(|g| g.try_absorb(n));
        if !absorbed {
            groups.push(NotificationGroup::from_notification(n));
        }
    }
    groups
}

// ---------------------------------------------------------------------------
// Notification sound mapping
// ---------------------------------------------------------------------------

/// Sound effect associated with a notification severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSound {
    /// No sound.
    None,
    /// Gentle chime for info notifications.
    Chime,
    /// Alert tone for warnings.
    Alert,
    /// Critical alarm for errors.
    Alarm,
}

impl NotificationSound {
    /// Map a severity to a default sound.
    pub fn from_severity(severity: NotificationSeverity) -> Self {
        match severity {
            NotificationSeverity::Info => NotificationSound::Chime,
            NotificationSeverity::Warning => NotificationSound::Alert,
            NotificationSeverity::Error => NotificationSound::Alarm,
        }
    }

    /// Terminal bell sequence for this sound (if any).
    pub fn bell_sequence(&self) -> Option<&'static str> {
        match self {
            NotificationSound::None => Option::None,
            _ => Some("\x07"),
        }
    }
}

impl std::fmt::Display for NotificationSound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationSound::None => write!(f, "none"),
            NotificationSound::Chime => write!(f, "chime"),
            NotificationSound::Alert => write!(f, "alert"),
            NotificationSound::Alarm => write!(f, "alarm"),
        }
    }
}

// ---------------------------------------------------------------------------
// NotificationActionHandler – action handler chain
// ---------------------------------------------------------------------------

/// Handles notification action button clicks.
pub struct NotificationActionHandler {
    handlers: Vec<(String, Box<dyn Fn(&str) -> bool + Send + Sync>)>,
}

impl NotificationActionHandler {
    /// Create a new empty handler.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Register a named action handler.
    pub fn register(
        &mut self,
        action_id: impl Into<String>,
        handler: Box<dyn Fn(&str) -> bool + Send + Sync>,
    ) {
        self.handlers.push((action_id.into(), handler));
    }

    /// Handle an action by ID. Returns `true` if handled.
    pub fn handle(&self, action_id: &str) -> bool {
        for (id, handler) in &self.handlers {
            if id == action_id {
                return handler(action_id);
            }
        }
        false
    }

    /// Number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

impl Default for NotificationActionHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for NotificationActionHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<&str> = self.handlers.iter().map(|(id, _)| id.as_str()).collect();
        write!(f, "NotificationActionHandler({:?})", ids)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// NotificationSourceFilter – filter notifications by source
// ---------------------------------------------------------------------------

/// Filter criteria for notifications based on their source.
#[derive(Debug, Clone)]
pub struct NotificationSourceFilter {
    included_sources: Vec<String>,
    excluded_sources: Vec<String>,
    severity_filter: Option<NotificationSeverity>,
}

impl NotificationSourceFilter {
    pub fn new() -> Self {
        Self {
            included_sources: Vec::new(),
            excluded_sources: Vec::new(),
            severity_filter: None,
        }
    }

    pub fn include_source(&mut self, source: impl Into<String>) {
        let s = source.into();
        if !self.included_sources.contains(&s) {
            self.included_sources.push(s);
        }
    }

    pub fn exclude_source(&mut self, source: impl Into<String>) {
        let s = source.into();
        if !self.excluded_sources.contains(&s) {
            self.excluded_sources.push(s);
        }
    }

    pub fn set_severity_filter(&mut self, severity: NotificationSeverity) {
        self.severity_filter = Some(severity);
    }

    pub fn clear_severity_filter(&mut self) {
        self.severity_filter = None;
    }

    pub fn matches(&self, notification: &Notification) -> bool {
        if let Some(ref sev) = self.severity_filter {
            if notification.severity != *sev {
                return false;
            }
        }
        if let Some(ref src) = notification.source {
            if self.excluded_sources.contains(src) {
                return false;
            }
            if !self.included_sources.is_empty() && !self.included_sources.contains(src) {
                return false;
            }
        } else if !self.included_sources.is_empty() {
            return false;
        }
        true
    }

    pub fn filter_notifications<'a>(
        &self,
        notifications: &'a [Notification],
    ) -> Vec<&'a Notification> {
        notifications.iter().filter(|n| self.matches(n)).collect()
    }

    pub fn included_count(&self) -> usize {
        self.included_sources.len()
    }

    pub fn excluded_count(&self) -> usize {
        self.excluded_sources.len()
    }

    pub fn reset(&mut self) {
        self.included_sources.clear();
        self.excluded_sources.clear();
        self.severity_filter = None;
    }
}

impl Default for NotificationSourceFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// NotificationPersistence – store and recall notifications
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PersistedNotification {
    pub id: u64,
    pub severity: NotificationSeverity,
    pub message: String,
    pub source: Option<String>,
    pub timestamp_ms: u64,
    pub was_actioned: bool,
}

#[derive(Debug)]
pub struct NotificationPersistence {
    records: Vec<PersistedNotification>,
    max_records: usize,
}

impl NotificationPersistence {
    pub fn new(max_records: usize) -> Self {
        Self {
            records: Vec::new(),
            max_records,
        }
    }

    pub fn persist(&mut self, notification: &Notification, timestamp_ms: u64) {
        let record = PersistedNotification {
            id: notification.id,
            severity: notification.severity,
            message: notification.message.clone(),
            source: notification.source.clone(),
            timestamp_ms,
            was_actioned: false,
        };
        self.records.push(record);
        if self.records.len() > self.max_records {
            self.records.remove(0);
        }
    }

    pub fn mark_actioned(&mut self, id: u64) -> bool {
        for r in &mut self.records {
            if r.id == id {
                r.was_actioned = true;
                return true;
            }
        }
        false
    }

    pub fn all_records(&self) -> &[PersistedNotification] {
        &self.records
    }

    pub fn records_by_severity(&self, severity: NotificationSeverity) -> Vec<&PersistedNotification> {
        self.records.iter().filter(|r| r.severity == severity).collect()
    }

    pub fn records_in_range(&self, from_ms: u64, to_ms: u64) -> Vec<&PersistedNotification> {
        self.records
            .iter()
            .filter(|r| r.timestamp_ms >= from_ms && r.timestamp_ms <= to_ms)
            .collect()
    }

    pub fn unactioned_records(&self) -> Vec<&PersistedNotification> {
        self.records.iter().filter(|r| !r.was_actioned).collect()
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    pub fn prune_before(&mut self, threshold_ms: u64) -> usize {
        let before = self.records.len();
        self.records.retain(|r| r.timestamp_ms >= threshold_ms);
        before - self.records.len()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

// ---------------------------------------------------------------------------
// NotificationActionChain – chain multiple actions together
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ActionStep {
    pub label: String,
    pub command: String,
    pub args: Vec<String>,
}

impl ActionStep {
    pub fn new(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
            args: Vec::new(),
        }
    }

    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn arg_count(&self) -> usize {
        self.args.len()
    }
}

#[derive(Debug, Clone)]
pub struct NotificationActionChain {
    pub name: String,
    steps: Vec<ActionStep>,
    stop_on_failure: bool,
}

impl NotificationActionChain {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            steps: Vec::new(),
            stop_on_failure: true,
        }
    }

    pub fn add_step(&mut self, step: ActionStep) {
        self.steps.push(step);
    }

    pub fn set_stop_on_failure(&mut self, stop: bool) {
        self.stop_on_failure = stop;
    }

    pub fn stop_on_failure(&self) -> bool {
        self.stop_on_failure
    }

    pub fn steps(&self) -> &[ActionStep] {
        &self.steps
    }

    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn get_step(&self, index: usize) -> Option<&ActionStep> {
        self.steps.get(index)
    }

    pub fn remove_step(&mut self, index: usize) -> Option<ActionStep> {
        if index < self.steps.len() {
            Some(self.steps.remove(index))
        } else {
            None
        }
    }

    pub fn all_commands(&self) -> Vec<&str> {
        self.steps.iter().map(|s| s.command.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// NotificationCenterView – view model for notification center panel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationCenterViewMode {
    All,
    Unread,
    BySeverity(NotificationSeverity),
}

#[derive(Debug)]
pub struct NotificationCenterView {
    persistence: NotificationPersistence,
    filter: NotificationSourceFilter,
    view_mode: NotificationCenterViewMode,
    selected_index: Option<usize>,
    expanded_ids: Vec<u64>,
}

impl NotificationCenterView {
    pub fn new(max_history: usize) -> Self {
        Self {
            persistence: NotificationPersistence::new(max_history),
            filter: NotificationSourceFilter::new(),
            view_mode: NotificationCenterViewMode::All,
            selected_index: None,
            expanded_ids: Vec::new(),
        }
    }

    pub fn add_notification(&mut self, notification: &Notification, timestamp_ms: u64) {
        self.persistence.persist(notification, timestamp_ms);
    }

    pub fn set_view_mode(&mut self, mode: NotificationCenterViewMode) {
        self.view_mode = mode;
        self.selected_index = None;
    }

    pub fn view_mode(&self) -> NotificationCenterViewMode {
        self.view_mode
    }

    pub fn visible_records(&self) -> Vec<&PersistedNotification> {
        let all = self.persistence.all_records();
        match self.view_mode {
            NotificationCenterViewMode::All => all.iter().collect(),
            NotificationCenterViewMode::Unread => {
                all.iter().filter(|r| !r.was_actioned).collect()
            }
            NotificationCenterViewMode::BySeverity(sev) => {
                all.iter().filter(|r| r.severity == sev).collect()
            }
        }
    }

    pub fn visible_count(&self) -> usize {
        self.visible_records().len()
    }

    pub fn select(&mut self, index: usize) {
        self.selected_index = Some(index);
    }

    pub fn clear_selection(&mut self) {
        self.selected_index = None;
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn toggle_expanded(&mut self, id: u64) {
        if let Some(pos) = self.expanded_ids.iter().position(|&x| x == id) {
            self.expanded_ids.remove(pos);
        } else {
            self.expanded_ids.push(id);
        }
    }

    pub fn is_expanded(&self, id: u64) -> bool {
        self.expanded_ids.contains(&id)
    }

    pub fn mark_read(&mut self, id: u64) -> bool {
        self.persistence.mark_actioned(id)
    }

    pub fn filter_mut(&mut self) -> &mut NotificationSourceFilter {
        &mut self.filter
    }

    pub fn total_count(&self) -> usize {
        self.persistence.record_count()
    }

    pub fn clear_all(&mut self) {
        self.persistence.clear();
        self.selected_index = None;
        self.expanded_ids.clear();
    }
}


// ---------------------------------------------------------------------------
// notification – Platform service helpers
// ---------------------------------------------------------------------------

/// Capability flags for platform feature detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XNotificationCapabilities {
    flags: std::collections::HashSet<String>,
}

impl XNotificationCapabilities {
    pub fn new() -> Self {
        Self { flags: std::collections::HashSet::new() }
    }

    pub fn register(&mut self, cap: impl Into<String>) {
        self.flags.insert(cap.into());
    }

    pub fn has(&self, cap: &str) -> bool {
        self.flags.contains(cap)
    }

    pub fn len(&self) -> usize {
        self.flags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flags.is_empty()
    }

    /// Return the intersection with another capability set.
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            flags: self.flags.intersection(&other.flags).cloned().collect(),
        }
    }

    /// Return capabilities present here but not in `other`.
    pub fn diff(&self, other: &Self) -> Self {
        Self {
            flags: self.flags.difference(&other.flags).cloned().collect(),
        }
    }

    pub fn all(&self) -> Vec<&str> {
        self.flags.iter().map(|s| s.as_str()).collect()
    }
}

impl Default for XNotificationCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple service registry keyed by name.
#[derive(Debug, Default)]
pub struct XNotificationServiceRegistry {
    services: std::collections::HashMap<String, String>,
}

impl XNotificationServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a service. Returns the previous value if the key was already present.
    pub fn register(&mut self, name: impl Into<String>, descriptor: impl Into<String>) -> Option<String> {
        self.services.insert(name.into(), descriptor.into())
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.services.get(name).map(|s| s.as_str())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.services.contains_key(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<String> {
        self.services.remove(name)
    }

    pub fn len(&self) -> usize {
        self.services.len()
    }

    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.services.keys().map(|s| s.as_str()).collect()
    }
}

/// Sanitize a path-like string by collapsing repeated separators and removing trailing ones.
pub fn x_notification_sanitize_path(p: &str) -> String {
    let mut result = String::with_capacity(p.len());
    let mut last_was_sep = false;
    for ch in p.chars() {
        if ch == '/' || ch == '\\' {
            if !last_was_sep {
                result.push('/');
            }
            last_was_sep = true;
        } else {
            result.push(ch);
            last_was_sep = false;
        }
    }
    if result.len() > 1 && result.ends_with('/') {
        result.pop();
    }
    result
}



// ---------------------------------------------------------------------------
// notification – Extended notification group helpers
// ---------------------------------------------------------------------------

/// Priority levels for notification group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZNotificationPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZNotificationPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZNotificationPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZNotificationPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks notification group data.
#[derive(Debug, Clone)]
pub struct ZNotificationNotificationGroup {
    pub ids: Vec<u64>,
    pub label: String,
    pub collapsed: bool,
}

impl ZNotificationNotificationGroup {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            ids: Vec::new(),
            label: String::new(),
            collapsed: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.ids.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZNotificationNotificationGroup[label={:?}, collapsed={:?}]", self.label, self.collapsed)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.collapsed = !c.collapsed;
        c
    }
}

/// Compute a simple rolling hash for notification group.
pub fn z_notification_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_notification_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_notification_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_notification_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_notification_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_notification_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_notification_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 64
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer64 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer64 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_64<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_64<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_64(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_64(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 130
// ---------------------------------------------------------------------------

/// Generic object pool `Xc130Pool<T>`.
pub struct Xc130Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc130Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc130PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc130Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc130PoolStats {
        Xc130PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc130Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc130Scheduler`.
pub struct Xc130Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc130Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc130Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_130 hash for the given byte slice.
pub fn xc_130_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_130 convention.
pub fn xc_130_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe77 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe77Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe77PipelineError {
    pub stage: Xe77Stage,
    pub message: String,
}

impl std::fmt::Display for Xe77PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe77Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe77Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe77PipelineError>>>,
    stage_names: Vec<Xe77Stage>,
}

impl Xe77Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe77PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe77Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe77PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe77Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe77PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe77Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe77PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe77Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe77PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe77Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe77CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe77CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe77Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe77CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe77CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe77Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe77CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_77_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe77CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_77_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe77CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_77_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe77PipelineError> {
    Ok(data)
}

pub fn xe_77_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe77PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_77_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe77PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_77_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe77PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_77_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe77PipelineError> {
    Err(Xe77PipelineError {
        stage: Xe77Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_75: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg75Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg75Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg75Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_75: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg75Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg75Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg75Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg75Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 129).
pub struct Xh129SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh129SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 171 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 129).
pub struct Xh129BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh129BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 129).
pub struct Xi129Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi129Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi129Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi129Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 129).
pub struct Xi129IntervalTree {
    xi_intervals: Vec<Xi129Interval>,
}

impl Xi129IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi129Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi129Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi129Interval) -> Vec<&Xi129Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi129Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi129Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi129Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi129Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi129Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi129Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 128) ---

/// Disjoint set / union-find for crate 128.
pub struct Xj128UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj128UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ128_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 128.
pub struct Xj128BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj128BTreeNode<K, V>>>,
    len: usize,
}

struct Xj128BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj128BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj128BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ128_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ128_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj128BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj128BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj128BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj128BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_128 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk128SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk128SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk128DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk128DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_128).
#[derive(Debug, Clone)]
pub struct Xl128Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl128Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_128).
#[derive(Debug, Clone)]
pub struct Xl128SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl128SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm128MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm128MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm128Tokenizer {
    text: String,
}

impl Xm128Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 129.
pub struct Xn129Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn129Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 129 -----

#[derive(Debug, Clone)]
struct Xn129AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn129AvlNode<K, V>>>,
    right: Option<Box<Xn129AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 129.
#[derive(Debug, Clone)]
pub struct Xn129AVL<K, V> {
    root: Option<Box<Xn129AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn129AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn129AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn129AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn129AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn129AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn129AvlNode<K, V>>) -> Box<Xn129AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn129AvlNode<K, V>>) -> Box<Xn129AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn129AvlNode<K, V>>) -> Box<Xn129AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn129AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn129AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn129AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn129AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn129AvlNode<K, V>>) -> &Xn129AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn129AvlNode<K, V>>) -> (Box<Xn129AvlNode<K, V>>, Option<Box<Xn129AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn129AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn129AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn129AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn129AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn129AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn129AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn129AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo129RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo129Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo129RBNode<K, V> {
    key: K,
    value: V,
    color: Xo129Color,
    left: Option<Box<Xo129RBNode<K, V>>>,
    right: Option<Box<Xo129RBNode<K, V>>>,
}

/// A red-black tree map for crate 129.
#[derive(Debug, Clone)]
pub struct Xo129RedBlack<K, V> {
    root: Option<Box<Xo129RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo129RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo129Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo129RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo129RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo129RBNode {
                    key, value, color: Xo129Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo129RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo129Color::Red)
    }

    fn xo_balance(mut h: Box<Xo129RBNode<K, V>>) -> Box<Xo129RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo129Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo129RBNode<K, V>>) -> Box<Xo129RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo129Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo129RBNode<K, V>>) -> Box<Xo129RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo129Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo129RBNode<K, V>>) {
        h.color = Xo129Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo129Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo129Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo129Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo129RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo129RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo129RBNode<K, V>) -> (K, V, Option<Box<Xo129RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo129RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo129Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo129RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo129ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 129.
#[derive(Debug, Clone)]
pub struct Xo129ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo129ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo129#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo129#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 128).
#[derive(Debug)]
pub struct Xp128SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp128Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp128Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp128Node<K, V>>>,
    xp_right: Option<Box<Xp128Node<K, V>>>,
}

impl<K: Ord, V> Xp128Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp128SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp128SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp128Node<K, V>>>, key: &K) -> Option<Box<Xp128Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp128Node<K, V>>) -> Box<Xp128Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp128Node<K, V>>) -> Box<Xp128Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp128Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp128Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp128Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq129Treap ---------------

use std::cmp::Ordering as Xq129Ord;

struct Xq129TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq129TreapNode<K, V>>>,
    right: Option<Box<Xq129TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq129Treap<K, V> {
    root: Option<Box<Xq129TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq129TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_129_size<K, V>(node: &Option<Box<Xq129TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_129_update_size<K, V>(node: &mut Xq129TreapNode<K, V>) {
    node.size = 1 + xq_129_size(&node.left) + xq_129_size(&node.right);
}

fn xq_129_rotate_right<K, V>(mut node: Box<Xq129TreapNode<K, V>>) -> Box<Xq129TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_129_update_size(&mut node);
    left.right = Some(node);
    xq_129_update_size(&mut left);
    left
}

fn xq_129_rotate_left<K, V>(mut node: Box<Xq129TreapNode<K, V>>) -> Box<Xq129TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_129_update_size(&mut node);
    right.left = Some(node);
    xq_129_update_size(&mut right);
    right
}

fn xq_129_insert_node<K: Ord, V>(
    node: Option<Box<Xq129TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq129TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq129TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq129Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq129Ord::Less => {
                let (new_left, old) = xq_129_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_129_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_129_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq129Ord::Greater => {
                let (new_right, old) = xq_129_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_129_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_129_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_129_remove_node<K: Ord, V>(
    node: Option<Box<Xq129TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq129TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq129Ord::Less => {
                let (new_left, old) = xq_129_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_129_update_size(&mut n);
                (Some(n), old)
            }
            Xq129Ord::Greater => {
                let (new_right, old) = xq_129_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_129_update_size(&mut n);
                (Some(n), old)
            }
            Xq129Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_129_rotate_right(n);
                    let (new_right, old) = xq_129_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_129_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_129_rotate_left(n);
                    let (new_left, old) = xq_129_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_129_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_129_find_min<K, V>(node: &Option<Box<Xq129TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_129_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_129_find_max<K, V>(node: &Option<Box<Xq129TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_129_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_129_rank<K: Ord, V>(node: &Option<Box<Xq129TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq129Ord::Less => xq_129_rank(&n.left, key),
            Xq129Ord::Equal => xq_129_size(&n.left),
            Xq129Ord::Greater => 1 + xq_129_size(&n.left) + xq_129_rank(&n.right, key),
        },
    }
}

fn xq_129_kth<K, V>(node: &Option<Box<Xq129TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_129_size(&n.left);
        if k < left_size {
            xq_129_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_129_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_129_in_order<K: Clone, V>(node: &Option<Box<Xq129TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_129_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_129_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq129Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 129 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_129_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq129Ord::Equal => return Some(&n.value),
                Xq129Ord::Less => cur = &n.left,
                Xq129Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_129_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_129_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_129_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_129_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_129_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_129_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_129_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq129VEBTree ---------------

pub struct Xq129VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq129VEBTree>>,
    clusters: Vec<Option<Box<Xq129VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq129VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq129VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq129VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr129KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr129KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr129BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr129KDNode {
    xr_point: Xr129KDPoint,
    xr_left: Option<Box<Xr129KDNode>>,
    xr_right: Option<Box<Xr129KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr129KDTree {
    xr_root: Option<Box<Xr129KDNode>>,
    xr_size: usize,
}

impl Xr129KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr129KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr129KDNode>>,
        point: Xr129KDPoint,
        depth: usize,
    ) -> Box<Xr129KDNode> {
        match node {
            None => Box::new(Xr129KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr129KDPoint) -> Option<Xr129KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr129KDNode>,
        query: &Xr129KDPoint,
        depth: usize,
        best: &mut Xr129KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr129KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr129KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr129KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr129KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr129KDNode>>, pts: &mut Vec<Xr129KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr129KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr129BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr129BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs128PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs128PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs128PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs128PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs128ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs128ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs128ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs128RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs128RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs128RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs128CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs128CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs128CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
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
    fn update_progress_works() {
        let svc = NotificationService::new();
        let n = Notification::info("task").with_finite_progress(50);
        let id = svc.notify(n);
        svc.update_progress(id, 25);
        let all = svc.get_notifications();
        let found = all.iter().find(|n| n.id == id).unwrap();
        assert_eq!(found.progress.as_ref().unwrap().worked, Some(25));
    }

    #[test]
    fn get_by_severity_works() {
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

    // -- notification_throttle --

    #[test]
    fn throttle_allows_first() {
        let mut throttle = NotificationThrottle::new(Duration::from_secs(5));
        let now = Instant::now();
        assert!(throttle.should_show("hello", now));
    }

    #[test]
    fn throttle_blocks_duplicate() {
        let mut throttle = NotificationThrottle::new(Duration::from_secs(5));
        let now = Instant::now();
        assert!(throttle.should_show("hello", now));
        assert!(!throttle.should_show("hello", now));
    }

    #[test]
    fn throttle_allows_after_cooldown() {
        let mut throttle = NotificationThrottle::new(Duration::from_secs(1));
        let t1 = Instant::now();
        assert!(throttle.should_show("hello", t1));
        let t2 = t1 + Duration::from_secs(2);
        assert!(throttle.should_show("hello", t2));
    }

    #[test]
    fn throttle_different_messages() {
        let mut throttle = NotificationThrottle::new(Duration::from_secs(5));
        let now = Instant::now();
        assert!(throttle.should_show("aaa", now));
        assert!(throttle.should_show("bbb", now));
    }

    #[test]
    fn throttle_evict_stale() {
        let mut throttle = NotificationThrottle::new(Duration::from_secs(1));
        let t1 = Instant::now();
        throttle.should_show("old", t1);
        let t2 = t1 + Duration::from_secs(2);
        throttle.evict_stale(t2);
        assert_eq!(throttle.tracked_count(), 0);
    }

    #[test]
    fn throttle_reset() {
        let mut throttle = NotificationThrottle::new(Duration::from_secs(5));
        throttle.should_show("test", Instant::now());
        throttle.reset();
        assert_eq!(throttle.tracked_count(), 0);
    }

    // -- new functionality tests --

    #[test]
    fn notification_is_progress() {
        let plain = Notification::info("plain");
        assert!(!plain.is_progress());

        let with_prog = Notification::info("dl").with_finite_progress(100);
        assert!(with_prog.is_progress());

        let infinite = Notification::info("spin").with_progress(true);
        assert!(infinite.is_progress());
    }

    #[test]
    fn notification_age() {
        let n = Notification::info("old");
        std::thread::sleep(Duration::from_millis(10));
        let age = n.age(Instant::now());
        assert!(age >= Duration::from_millis(10));
    }

    #[test]
    fn severity_is_critical() {
        assert!(!NotificationSeverity::Info.is_critical());
        assert!(!NotificationSeverity::Warning.is_critical());
        assert!(NotificationSeverity::Error.is_critical());
    }

    #[test]
    fn severity_label() {
        assert_eq!(NotificationSeverity::Info.label(), "info");
        assert_eq!(NotificationSeverity::Warning.label(), "warning");
        assert_eq!(NotificationSeverity::Error.label(), "error");
    }

    #[test]
    fn filter_matches() {
        let info = Notification::info("a").with_source("src1");
        let error = Notification::error("b");
        let sticky = Notification::info("c").with_sticky();

        let sev_filter = NotificationFilter {
            severity: Some(NotificationSeverity::Info),
            ..Default::default()
        };
        assert!(sev_filter.matches(&info));
        assert!(!sev_filter.matches(&error));

        let src_filter = NotificationFilter {
            source: Some("src1".into()),
            ..Default::default()
        };
        assert!(src_filter.matches(&info));
        assert!(!src_filter.matches(&error));

        let sticky_filter = NotificationFilter {
            sticky_only: true,
            ..Default::default()
        };
        assert!(!sticky_filter.matches(&info));
        assert!(sticky_filter.matches(&sticky));

        let empty_filter = NotificationFilter::default();
        assert!(empty_filter.matches(&info));
        assert!(empty_filter.matches(&error));
    }

    #[test]
    fn count_by_severity_works() {
        let svc = NotificationService::new();
        svc.notify(Notification::info("a"));
        svc.notify(Notification::info("b"));
        svc.notify(Notification::warning("c"));
        svc.notify(Notification::error("d"));
        let counts = svc.count_by_severity();
        assert_eq!(
            counts,
            NotificationSeverityCount {
                info: 2,
                warning: 1,
                error: 1,
            }
        );
    }

    #[test]
    fn active_count_and_total_count() {
        let svc = NotificationService::new();
        for i in 0..7 {
            svc.notify(Notification::info(format!("n{i}")));
        }
        assert_eq!(svc.active_count(), MAX_VISIBLE);
        assert_eq!(svc.total_count(), 7);
    }

    #[test]
    fn most_common_severity_empty() {
        let svc = NotificationService::new();
        assert!(svc.most_common_severity().is_none());
    }

    #[test]
    fn most_common_severity_pick() {
        let svc = NotificationService::new();
        svc.notify(Notification::warning("a"));
        svc.notify(Notification::warning("b"));
        svc.notify(Notification::info("c"));
        assert_eq!(
            svc.most_common_severity(),
            Some(NotificationSeverity::Warning)
        );
    }

    #[test]
    fn display_notification_progress() {
        let finite = NotificationProgress {
            infinite: false,
            total: Some(200),
            worked: Some(100),
        };
        assert_eq!(format!("{finite}"), "100/200 (50%)");

        let inf = NotificationProgress {
            infinite: true,
            total: None,
            worked: None,
        };
        assert_eq!(format!("{inf}"), "in progress…");

        let zero = NotificationProgress {
            infinite: false,
            total: None,
            worked: None,
        };
        assert_eq!(format!("{zero}"), "0%");
    }

    #[test]
    fn notification_batch_operations() {
        let mut batch = NotificationBatch::new();
        assert!(batch.is_empty());

        batch.add(Notification::info("a"));
        batch.add(Notification::error("b"));
        batch.add(Notification::warning("c"));
        assert_eq!(batch.len(), 3);
        assert!(!batch.is_empty());

        let sevs = batch.severities();
        assert_eq!(sevs[0], NotificationSeverity::Info);
        assert_eq!(sevs[1], NotificationSeverity::Error);
        assert_eq!(sevs[2], NotificationSeverity::Warning);

        let svc = NotificationService::new();
        let ids = batch.send_all(&svc);
        assert_eq!(ids.len(), 3);
        assert_eq!(svc.active_count(), 3);
    }

    #[test]
    fn notification_batch_into_iter() {
        let mut batch = NotificationBatch::new();
        batch.add(Notification::info("x"));
        batch.add(Notification::info("y"));
        let messages: Vec<String> = batch.into_iter().map(|n| n.message).collect();
        assert_eq!(messages, vec!["x", "y"]);
    }

    // -- NotificationHistory tests --

    #[test]
    fn history_records_and_evicts() {
        let mut history = NotificationHistory::new(3);
        assert!(history.is_empty());
        assert_eq!(history.capacity(), 3);

        history.record(Notification::info("first"));
        history.record(Notification::warning("second"));
        history.record(Notification::error("third"));
        assert_eq!(history.len(), 3);

        // Adding a 4th should evict "first"
        history.record(Notification::info("fourth"));
        assert_eq!(history.len(), 3);
        assert_eq!(history.entries()[0].message, "second");
        assert_eq!(history.entries()[2].message, "fourth");
    }

    #[test]
    fn history_search_and_by_severity() {
        let mut history = NotificationHistory::new(10);
        history.record(Notification::info("Build succeeded"));
        history.record(Notification::error("Build FAILED"));
        history.record(Notification::warning("Unused import"));

        let results = history.search("build");
        assert_eq!(results.len(), 2);

        let errors = history.by_severity(NotificationSeverity::Error);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "Build FAILED");

        history.clear();
        assert!(history.is_empty());
    }

    // -- DoNotDisturb tests --

    #[test]
    fn dnd_suppresses_below_threshold() {
        let mut dnd = DoNotDisturb::new();
        assert!(!dnd.is_enabled());

        // Enable DND at Error level — only errors pass
        dnd.enable(NotificationSeverity::Error);
        assert!(dnd.is_enabled());

        let info = Notification::info("hello");
        let warn = Notification::warning("careful");
        let err = Notification::error("boom");

        assert!(!dnd.should_show(&info));
        assert!(!dnd.should_show(&warn));
        assert!(dnd.should_show(&err));
        assert_eq!(dnd.suppressed_count(), 2);

        dnd.disable();
        assert!(!dnd.is_enabled());
        assert!(dnd.should_show(&info));
    }

    #[test]
    fn dnd_warning_threshold_passes_warn_and_error() {
        let mut dnd = DoNotDisturb::new();
        dnd.enable(NotificationSeverity::Warning);

        assert!(!dnd.should_show(&Notification::info("low")));
        assert!(dnd.should_show(&Notification::warning("mid")));
        assert!(dnd.should_show(&Notification::error("high")));
        assert_eq!(dnd.suppressed_count(), 1);
    }

    // -- NotificationDeduplicator tests --

    #[test]
    fn deduplicator_detects_duplicates() {
        let mut dedup = NotificationDeduplicator::new();

        let n1 = Notification::info("file saved");
        assert!(dedup.check(&n1).is_none()); // first time → not a dup

        let n2 = Notification::info("file saved");
        let result = dedup.check(&n2);
        assert!(result.is_some());
        let (original_id, count) = result.unwrap();
        assert_eq!(original_id, n1.id);
        assert_eq!(count, 2);

        assert_eq!(dedup.unique_count(), 1);
        assert_eq!(dedup.total_occurrences(), 2);
    }

    #[test]
    fn deduplicator_different_messages_not_duplicates() {
        let mut dedup = NotificationDeduplicator::new();
        assert!(dedup.check(&Notification::info("aaa")).is_none());
        assert!(dedup.check(&Notification::info("bbb")).is_none());
        assert_eq!(dedup.unique_count(), 2);
        assert_eq!(dedup.total_occurrences(), 2);

        dedup.reset();
        assert_eq!(dedup.unique_count(), 0);
    }

    // -- NotificationQueue tests --

    #[test]
    fn queue_priority_order() {
        let mut q = NotificationQueue::new(10);
        q.push(Notification::info("low"));
        q.push(Notification::error("high"));
        q.push(Notification::warning("mid"));
        let first = q.pop().unwrap();
        assert_eq!(first.severity, NotificationSeverity::Error);
    }

    #[test]
    fn queue_capacity_limit() {
        let mut q = NotificationQueue::new(2);
        q.push(Notification::info("a"));
        q.push(Notification::info("b"));
        q.push(Notification::error("c"));
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn queue_empty() {
        let q = NotificationQueue::default();
        assert!(q.is_empty());
        assert!(q.peek().is_none());
    }

    // -- NotificationGroup tests --

    #[test]
    fn group_absorb_same_source() {
        let n1 = Notification::info("a").with_source("build");
        let n2 = Notification::warning("b").with_source("build");
        let mut g = NotificationGroup::from_notification(&n1);
        assert!(g.try_absorb(&n2));
        assert_eq!(g.count, 2);
        assert_eq!(g.max_severity, NotificationSeverity::Warning);
    }

    #[test]
    fn group_no_absorb_different_source() {
        let n1 = Notification::info("a").with_source("build");
        let n2 = Notification::info("b").with_source("lint");
        let mut g = NotificationGroup::from_notification(&n1);
        assert!(!g.try_absorb(&n2));
    }

    #[test]
    fn group_display() {
        let mut g = NotificationGroup {
            representative: "Error".into(),
            source: "build".into(),
            count: 3,
            max_severity: NotificationSeverity::Error,
        };
        assert_eq!(format!("{}", g), "Error (+2)");
        g.count = 1;
        assert_eq!(format!("{}", g), "Error");
    }

    #[test]
    fn group_notifications_fn() {
        let notifs = vec![
            Notification::info("a").with_source("build"),
            Notification::info("b").with_source("build"),
            Notification::error("c").with_source("lint"),
        ];
        let groups = group_notifications(&notifs);
        assert_eq!(groups.len(), 2);
    }

    // -- NotificationSound tests --

    #[test]
    fn sound_from_severity() {
        assert_eq!(NotificationSound::from_severity(NotificationSeverity::Info), NotificationSound::Chime);
        assert_eq!(NotificationSound::from_severity(NotificationSeverity::Error), NotificationSound::Alarm);
    }

    #[test]
    fn sound_bell() {
        assert!(NotificationSound::None.bell_sequence().is_none());
        assert!(NotificationSound::Chime.bell_sequence().is_some());
    }

    // -- NotificationActionHandler tests --

    #[test]
    fn action_handler_register_and_handle() {
        let mut h = NotificationActionHandler::new();
        h.register("retry", Box::new(|_| true));
        assert!(h.handle("retry"));
        assert!(!h.handle("unknown"));
    }

    #[test]
    fn action_handler_count() {
        let mut h = NotificationActionHandler::new();
        h.register("a", Box::new(|_| true));
        h.register("b", Box::new(|_| false));
        assert_eq!(h.handler_count(), 2);
    }
    #[test]
    fn source_filter_basic() {
        let mut filter = NotificationSourceFilter::new();
        let info = Notification::info("msg").with_source("Rust");
        assert!(filter.matches(&info));
        filter.include_source("TypeScript");
        assert!(!filter.matches(&info));
        filter.include_source("Rust");
        assert!(filter.matches(&info));
    }

    #[test]
    fn source_filter_exclude() {
        let mut filter = NotificationSourceFilter::new();
        filter.exclude_source("Rust");
        let n = Notification::info("msg").with_source("Rust");
        assert!(!filter.matches(&n));
        let n2 = Notification::info("msg").with_source("Go");
        assert!(filter.matches(&n2));
    }

    #[test]
    fn source_filter_severity() {
        let mut filter = NotificationSourceFilter::new();
        filter.set_severity_filter(NotificationSeverity::Error);
        assert!(!filter.matches(&Notification::info("msg")));
        assert!(filter.matches(&Notification::error("msg")));
        filter.clear_severity_filter();
        assert!(filter.matches(&Notification::info("msg")));
    }

    #[test]
    fn source_filter_reset() {
        let mut filter = NotificationSourceFilter::new();
        filter.include_source("X");
        filter.exclude_source("Y");
        filter.set_severity_filter(NotificationSeverity::Info);
        filter.reset();
        assert_eq!(filter.included_count(), 0);
        assert_eq!(filter.excluded_count(), 0);
    }

    #[test]
    fn persistence_basic() {
        let mut store = NotificationPersistence::new(10);
        let n = Notification::info("hello");
        store.persist(&n, 1000);
        assert_eq!(store.record_count(), 1);
        assert_eq!(store.all_records()[0].message, "hello");
    }

    #[test]
    fn persistence_max_records() {
        let mut store = NotificationPersistence::new(3);
        for i in 0..5 {
            let n = Notification::info(&format!("msg{i}"));
            store.persist(&n, i as u64 * 100);
        }
        assert_eq!(store.record_count(), 3);
        assert_eq!(store.all_records()[0].message, "msg2");
    }

    #[test]
    fn persistence_mark_actioned() {
        let mut store = NotificationPersistence::new(10);
        let n = Notification::info("x");
        let nid = n.id;
        store.persist(&n, 100);
        assert_eq!(store.unactioned_records().len(), 1);
        assert!(store.mark_actioned(nid));
        assert_eq!(store.unactioned_records().len(), 0);
    }

    #[test]
    fn persistence_prune() {
        let mut store = NotificationPersistence::new(100);
        for i in 0..5 {
            let n = Notification::info(&format!("m{i}"));
            store.persist(&n, i as u64 * 100);
        }
        assert_eq!(store.prune_before(200), 2);
        assert_eq!(store.record_count(), 3);
    }

    #[test]
    fn persistence_by_severity() {
        let mut store = NotificationPersistence::new(10);
        store.persist(&Notification::info("i"), 100);
        store.persist(&Notification::error("e"), 200);
        assert_eq!(store.records_by_severity(NotificationSeverity::Info).len(), 1);
        assert_eq!(store.records_by_severity(NotificationSeverity::Error).len(), 1);
    }

    #[test]
    fn persistence_range_query() {
        let mut store = NotificationPersistence::new(10);
        for i in 0..5 {
            store.persist(&Notification::info(&format!("m{i}")), i as u64 * 100);
        }
        assert_eq!(store.records_in_range(100, 300).len(), 3);
    }

    #[test]
    fn action_step_building() {
        let step = ActionStep::new("retry", "workbench.action.retry")
            .with_arg("force")
            .with_arg("--verbose");
        assert_eq!(step.label, "retry");
        assert_eq!(step.arg_count(), 2);
    }

    #[test]
    fn action_chain_operations() {
        let mut chain = NotificationActionChain::new("build-and-test");
        assert!(chain.is_empty());
        chain.add_step(ActionStep::new("build", "task.build"));
        chain.add_step(ActionStep::new("test", "task.test"));
        assert_eq!(chain.step_count(), 2);
        assert_eq!(chain.all_commands(), vec!["task.build", "task.test"]);
        assert!(chain.stop_on_failure());
        chain.set_stop_on_failure(false);
        assert!(!chain.stop_on_failure());
    }

    #[test]
    fn action_chain_get_remove() {
        let mut chain = NotificationActionChain::new("c");
        chain.add_step(ActionStep::new("a", "cmd.a"));
        chain.add_step(ActionStep::new("b", "cmd.b"));
        assert_eq!(chain.get_step(0).unwrap().label, "a");
        let removed = chain.remove_step(0).unwrap();
        assert_eq!(removed.label, "a");
        assert_eq!(chain.step_count(), 1);
        assert!(chain.remove_step(99).is_none());
    }

    #[test]
    fn center_view_basic() {
        let mut view = NotificationCenterView::new(100);
        view.add_notification(&Notification::info("hello"), 1000);
        assert_eq!(view.total_count(), 1);
        assert_eq!(view.visible_count(), 1);
    }

    #[test]
    fn center_view_modes() {
        let mut view = NotificationCenterView::new(100);
        view.add_notification(&Notification::info("i"), 100);
        view.add_notification(&Notification::error("e"), 200);
        view.set_view_mode(NotificationCenterViewMode::All);
        assert_eq!(view.visible_count(), 2);
        view.set_view_mode(NotificationCenterViewMode::BySeverity(NotificationSeverity::Error));
        assert_eq!(view.visible_count(), 1);
    }

    #[test]
    fn center_view_selection() {
        let mut view = NotificationCenterView::new(100);
        view.add_notification(&Notification::info("a"), 100);
        view.select(0);
        assert_eq!(view.selected_index(), Some(0));
        view.clear_selection();
        assert_eq!(view.selected_index(), None);
    }

    #[test]
    fn center_view_expand_toggle() {
        let mut view = NotificationCenterView::new(100);
        view.toggle_expanded(42);
        assert!(view.is_expanded(42));
        view.toggle_expanded(42);
        assert!(!view.is_expanded(42));
    }

    #[test]
    fn center_view_mark_read() {
        let mut view = NotificationCenterView::new(100);
        let n = Notification::info("msg");
        let nid = n.id;
        view.add_notification(&n, 100);
        view.set_view_mode(NotificationCenterViewMode::Unread);
        assert_eq!(view.visible_count(), 1);
        view.mark_read(nid);
        assert_eq!(view.visible_count(), 0);
    }

    #[test]
    fn center_view_clear_all() {
        let mut view = NotificationCenterView::new(100);
        view.add_notification(&Notification::info("a"), 100);
        view.select(0);
        view.toggle_expanded(1);
        view.clear_all();
        assert_eq!(view.total_count(), 0);
        assert_eq!(view.selected_index(), None);
    }


    // -- notification additional tests -------------------------------------------

    #[test]
    fn x_notification_capabilities_register_and_has() {
        let mut caps = XNotificationCapabilities::new();
        caps.register("clipboard");
        assert!(caps.has("clipboard"));
        assert!(!caps.has("fs"));
    }

    #[test]
    fn x_notification_capabilities_len() {
        let mut caps = XNotificationCapabilities::new();
        assert!(caps.is_empty());
        caps.register("a");
        caps.register("b");
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn x_notification_capabilities_intersect() {
        let mut a = XNotificationCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XNotificationCapabilities::new();
        b.register("y");
        b.register("z");
        let inter = a.intersect(&b);
        assert_eq!(inter.len(), 1);
        assert!(inter.has("y"));
    }

    #[test]
    fn x_notification_capabilities_diff() {
        let mut a = XNotificationCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XNotificationCapabilities::new();
        b.register("y");
        let d = a.diff(&b);
        assert_eq!(d.len(), 1);
        assert!(d.has("x"));
    }

    #[test]
    fn x_notification_service_registry_basic() {
        let mut reg = XNotificationServiceRegistry::new();
        assert!(reg.is_empty());
        reg.register("clipboard", "v1");
        assert_eq!(reg.get("clipboard"), Some("v1"));
        assert!(reg.contains("clipboard"));
    }

    #[test]
    fn x_notification_service_registry_replace() {
        let mut reg = XNotificationServiceRegistry::new();
        assert!(reg.register("svc", "old").is_none());
        assert_eq!(reg.register("svc", "new"), Some("old".into()));
        assert_eq!(reg.get("svc"), Some("new"));
    }

    #[test]
    fn x_notification_service_registry_remove() {
        let mut reg = XNotificationServiceRegistry::new();
        reg.register("svc", "v1");
        assert_eq!(reg.remove("svc"), Some("v1".into()));
        assert!(reg.is_empty());
    }

    #[test]
    fn x_notification_service_registry_names() {
        let mut reg = XNotificationServiceRegistry::new();
        reg.register("a", "1");
        reg.register("b", "2");
        let mut names = reg.names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn x_notification_sanitize_path_basic() {
        assert_eq!(x_notification_sanitize_path("/a//b///c/"), "/a/b/c");
    }

    #[test]
    fn x_notification_sanitize_path_backslash() {
        assert_eq!(x_notification_sanitize_path("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn x_notification_sanitize_path_single() {
        assert_eq!(x_notification_sanitize_path("/"), "/");
    }

    #[test]
    fn x_notification_capabilities_default() {
        let caps = XNotificationCapabilities::default();
        assert!(caps.is_empty());
    }

    #[test]
    fn x_notification_capabilities_all() {
        let mut caps = XNotificationCapabilities::new();
        caps.register("a");
        caps.register("b");
        let mut all = caps.all();
        all.sort();
        assert_eq!(all, vec!["a", "b"]);
    }


    // -- notification Z-extended tests -----------------------------------------------

    #[test]
    fn z_notification_priority_weight() {
        assert_eq!(ZNotificationPriority::Idle.weight(), 0);
        assert_eq!(ZNotificationPriority::Normal.weight(), 2);
        assert_eq!(ZNotificationPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_notification_priority_label() {
        assert_eq!(ZNotificationPriority::Low.label(), "low");
        assert_eq!(ZNotificationPriority::High.label(), "high");
    }

    #[test]
    fn z_notification_priority_is_elevated() {
        assert!(!ZNotificationPriority::Normal.is_elevated());
        assert!(ZNotificationPriority::High.is_elevated());
        assert!(ZNotificationPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_notification_priority_display() {
        assert_eq!(format!("{}", ZNotificationPriority::Idle), "idle");
    }

    #[test]
    fn z_notification_priority_all_asc() {
        let all = ZNotificationPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZNotificationPriority::Idle);
        assert_eq!(all[4], ZNotificationPriority::Realtime);
    }

    #[test]
    fn z_notification_struct_new() {
        let s = ZNotificationNotificationGroup::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_notification_struct_toggled_clone() {
        let s = ZNotificationNotificationGroup::new();
        let t = s.toggled_clone();
        assert_ne!(s.collapsed, t.collapsed);
    }

    #[test]
    fn z_notification_rolling_hash_deterministic() {
        let h1 = z_notification_rolling_hash(b"test");
        let h2 = z_notification_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_notification_rolling_hash(b"a"), z_notification_rolling_hash(b"b"));
    }

    #[test]
    fn z_notification_pad_to_basic() {
        assert_eq!(z_notification_pad_to("hi", 5), "hi   ");
        assert_eq!(z_notification_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_notification_is_identifier_basic() {
        assert!(z_notification_is_identifier("foo_bar"));
        assert!(z_notification_is_identifier("abc123"));
        assert!(!z_notification_is_identifier(""));
        assert!(!z_notification_is_identifier("has space"));
    }

    #[test]
    fn z_notification_levenshtein_basic() {
        assert_eq!(z_notification_levenshtein("", ""), 0);
        assert_eq!(z_notification_levenshtein("abc", "abc"), 0);
        assert_eq!(z_notification_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_notification_unique_words_basic() {
        let w = z_notification_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_notification_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_notification_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_notification_common_prefix_basic() {
        assert_eq!(z_notification_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_notification_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_notification_struct_clear() {
        let mut s = ZNotificationNotificationGroup::new();
        s.ids.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_notification_rolling_hash_empty() {
        let h = z_notification_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    #[test]
    fn xb_ring_buffer_64_push_and_len() {
        let mut rb = super::XbRingBuffer64::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_64_overwrite() {
        let mut rb = super::XbRingBuffer64::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_64_get_out_of_bounds() {
        let rb = super::XbRingBuffer64::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_64_drain_all() {
        let mut rb = super::XbRingBuffer64::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_64_peek_front_back() {
        let mut rb = super::XbRingBuffer64::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_64_clear() {
        let mut rb = super::XbRingBuffer64::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_64_capacity() {
        let rb = super::XbRingBuffer64::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_64_basic() {
        let h = super::xb_fnv1a_64(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_64(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_64_different_inputs() {
        let h1 = super::xb_fnv1a_64(b"abc");
        let h2 = super::xb_fnv1a_64(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_64_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_64(&data);
        let dec = super::xb_rle_decode_64(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_64_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_64(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_64(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_64_values() {
        assert!((super::xb_clamp_64(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_64(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_64(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_64_values() {
        assert!((super::xb_lerp_64(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_64(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_64(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_64_wrap_around_twice() {
        let mut rb = super::XbRingBuffer64::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 130 ----

    #[test]
    fn xc_130_pool_new_empty() {
        let pool: super::Xc130Pool<i32> = super::Xc130Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_130_pool_release_acquire() {
        let mut pool = super::Xc130Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_130_pool_acquire_empty() {
        let mut pool: super::Xc130Pool<i32> = super::Xc130Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_130_pool_full() {
        let mut pool = super::Xc130Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_130_pool_drain() {
        let mut pool = super::Xc130Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_130_pool_stats() {
        let mut pool = super::Xc130Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_130_pool_clear() {
        let mut pool = super::Xc130Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_130_pool_shrink() {
        let mut pool = super::Xc130Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_130_pool_default() {
        let pool: super::Xc130Pool<String> = super::Xc130Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_130_pool_extend() {
        let mut pool = super::Xc130Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_130_pool_retain() {
        let mut pool = super::Xc130Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_130_scheduler_round_robin() {
        let mut sched = super::Xc130Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_130_scheduler_empty() {
        let mut sched = super::Xc130Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_130_scheduler_reset() {
        let mut sched = super::Xc130Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_130_scheduler_add_remove() {
        let mut sched = super::Xc130Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_130_scheduler_targets() {
        let sched = super::Xc130Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_130_hash_empty() {
        assert_eq!(super::xc_130_hash(b""), 5381);
    }

    #[test]
    fn xc_130_hash_data() {
        let h = super::xc_130_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_130_hash(b"hello"), h);
    }

    #[test]
    fn xc_130_reverse_str() {
        assert_eq!(super::xc_130_reverse("abc"), "cba");
        assert_eq!(super::xc_130_reverse(""), "");
    }


    #[test]
    fn xe_77_pipeline_empty() {
        let p = super::Xe77Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_77_pipeline_parse_stage() {
        let p = super::Xe77Pipeline::new()
            .add_parse(super::xe_77_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_77_pipeline_transform_double() {
        let p = super::Xe77Pipeline::new()
            .add_transform(super::xe_77_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_77_pipeline_validate_reverse() {
        let p = super::Xe77Pipeline::new()
            .add_validate(super::xe_77_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_77_pipeline_emit_filter() {
        let p = super::Xe77Pipeline::new()
            .add_emit(super::xe_77_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_77_pipeline_multi_stage() {
        let p = super::Xe77Pipeline::new()
            .add_parse(super::xe_77_pipeline_identity)
            .add_transform(super::xe_77_pipeline_double)
            .add_validate(super::xe_77_pipeline_reverse)
            .add_emit(super::xe_77_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_77_pipeline_error_propagation() {
        let p = super::Xe77Pipeline::new()
            .add_parse(super::xe_77_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe77Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_77_pipeline_compose() {
        let p1 = super::Xe77Pipeline::new()
            .add_parse(super::xe_77_pipeline_identity);
        let p2 = super::Xe77Pipeline::new()
            .add_transform(super::xe_77_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_77_pipeline_error_display() {
        let e = super::Xe77PipelineError {
            stage: super::Xe77Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_77_cache_put_get() {
        let mut c = super::Xe77Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_77_cache_miss() {
        let mut c: super::Xe77Cache<&str, i32> = super::Xe77Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_77_cache_ttl_expiry() {
        let mut c = super::Xe77Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_77_cache_evict() {
        let mut c = super::Xe77Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_77_cache_capacity() {
        let mut c = super::Xe77Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_77_cache_stats() {
        let mut c = super::Xe77Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_77_cache_clear() {
        let mut c = super::Xe77Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_75 graph tests ------------------------------------------------

    #[test]
    fn xg_75_graph_empty() {
        let g = super::Xg75Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_75_graph_add_node() {
        let mut g = super::Xg75Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_75_graph_add_edge() {
        let mut g = super::Xg75Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_75_graph_neighbors() {
        let mut g = super::Xg75Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_75_graph_has_path() {
        let mut g = super::Xg75Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_75_graph_self_path() {
        let g = super::Xg75Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_75_graph_topo_sort() {
        let mut g = super::Xg75Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_75_graph_cycle_detect_false() {
        let mut g = super::Xg75Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_75_graph_cycle_detect_true() {
        let mut g = super::Xg75Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_75 heap tests -------------------------------------------------

    #[test]
    fn xg_75_heap_empty() {
        let h: super::Xg75Heap<i32> = super::Xg75Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_75_heap_push_pop() {
        let mut h = super::Xg75Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_75_heap_peek() {
        let mut h = super::Xg75Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_75_heap_drain_sorted() {
        let mut h = super::Xg75Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_75_heap_merge() {
        let mut a = super::Xg75Heap::new();
        let mut b = super::Xg75Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_75_heap_default() {
        let h: super::Xg75Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_75_graph_default() {
        let g: super::Xg75Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh129_skip_insert_contains() {
        let mut sl = super::Xh129SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh129_skip_remove() {
        let mut sl = super::Xh129SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh129_skip_len() {
        let mut sl = super::Xh129SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh129_skip_range_query() {
        let mut sl = super::Xh129SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh129_skip_floor_ceiling() {
        let mut sl = super::Xh129SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh129_skip_rank() {
        let mut sl = super::Xh129SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh129_skip_empty() {
        let sl = super::Xh129SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh129_skip_duplicates() {
        let mut sl = super::Xh129SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh129_bitset_set_test() {
        let mut bs = super::Xh129BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh129_bitset_clear_count() {
        let mut bs = super::Xh129BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh129_bitset_and_or_xor() {
        let mut a = super::Xh129BitSet::xh_new(128);
        let mut b = super::Xh129BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh129_bitset_iter_ones() {
        let mut bs = super::Xh129BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh129_bitset_first_last() {
        let mut bs = super::Xh129BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh129_bitset_empty() {
        let bs = super::Xh129BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi129_deque_push_pop_back() {
        let mut dq = super::Xi129Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi129_deque_push_pop_front() {
        let mut dq = super::Xi129Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi129_deque_mixed_ops() {
        let mut dq = super::Xi129Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi129_deque_get_and_split() {
        let mut dq = super::Xi129Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi129_deque_rotate_left() {
        let mut dq = super::Xi129Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi129_deque_rotate_right() {
        let mut dq = super::Xi129Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi129_deque_grow() {
        let mut dq = super::Xi129Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi129_deque_empty() {
        let dq = super::Xi129Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi129_interval_tree_insert_query() {
        let mut tree = super::Xi129IntervalTree::xi_new();
        tree.xi_insert(super::Xi129Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi129Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi129Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi129_interval_tree_overlap() {
        let mut tree = super::Xi129IntervalTree::xi_new();
        tree.xi_insert(super::Xi129Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi129Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi129Interval::xi_new(12, 20));
        let q = super::Xi129Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi129_interval_tree_remove() {
        let mut tree = super::Xi129IntervalTree::xi_new();
        tree.xi_insert(super::Xi129Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi129Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi129_interval_tree_gaps() {
        let mut tree = super::Xi129IntervalTree::xi_new();
        tree.xi_insert(super::Xi129Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi129Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi129Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi129Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi129Interval::xi_new(8, 10));
    }

    #[test]
    fn xi129_interval_tree_merge() {
        let mut tree = super::Xi129IntervalTree::xi_new();
        tree.xi_insert(super::Xi129Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi129Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi129Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi129Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi129Interval::xi_new(10, 15));
    }

    #[test]
    fn xi129_interval_tree_all() {
        let mut tree = super::Xi129IntervalTree::xi_new();
        tree.xi_insert(super::Xi129Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi129Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi129_interval_tree_empty() {
        let tree = super::Xi129IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi129_interval_tree_contains_point() {
        let iv = super::Xi129Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 128) ---

    #[test]
    fn xj_128_uf_make_and_find() {
        let mut uf = super::Xj128UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_128_uf_union_connected() {
        let mut uf = super::Xj128UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_128_uf_component_count() {
        let mut uf = super::Xj128UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_128_uf_component_size() {
        let mut uf = super::Xj128UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_128_uf_largest_component() {
        let mut uf = super::Xj128UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_128_uf_many_elements() {
        let mut uf = super::Xj128UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_128_uf_separate_components() {
        let mut uf = super::Xj128UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_128_uf_path_compression() {
        let mut uf = super::Xj128UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_128_bt_insert_get() {
        let mut bt = super::Xj128BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_128_bt_contains_len() {
        let mut bt = super::Xj128BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_128_bt_replace() {
        let mut bt = super::Xj128BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_128_bt_remove() {
        let mut bt = super::Xj128BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_128_bt_keys_values() {
        let mut bt = super::Xj128BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_128_bt_range() {
        let mut bt = super::Xj128BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_128_bt_min_max() {
        let mut bt = super::Xj128BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_128_bt_many_inserts() {
        let mut bt = super::Xj128BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_128 segment tree tests ---

    #[test]
    fn xk_128_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk128SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_128_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk128SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_128_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk128SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_128_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk128SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_128_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk128SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_128_st_single_element() {
        let data = vec![42];
        let st = super::Xk128SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_128_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk128SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_128_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk128SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_128 disjoint intervals tests ---

    #[test]
    fn xk_128_di_add_and_count() {
        let mut di = super::Xk128DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_128_di_merge_overlap() {
        let mut di = super::Xk128DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_128_di_contains() {
        let mut di = super::Xk128DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_128_di_remove() {
        let mut di = super::Xk128DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_128_di_covered_length() {
        let mut di = super::Xk128DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_128_di_gaps() {
        let mut di = super::Xk128DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_128_di_merge_adjacent() {
        let mut di = super::Xk128DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_128_di_empty() {
        let di = super::Xk128DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_128_rope_new_empty() {
        let rope = super::Xl128Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_128_rope_from_str() {
        let rope = super::Xl128Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_128_rope_insert_at() {
        let mut rope = super::Xl128Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_128_rope_delete_range() {
        let mut rope = super::Xl128Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_128_rope_char_at() {
        let rope = super::Xl128Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_128_rope_split_concat() {
        let rope = super::Xl128Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_128_rope_line_count() {
        let rope = super::Xl128Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_128_rope_line_at() {
        let rope = super::Xl128Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_128_sa_build_and_search() {
        let sa = super::Xl128SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_128_sa_count() {
        let sa = super::Xl128SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_128_sa_longest_repeated() {
        let sa = super::Xl128SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_128_sa_all_positions() {
        let sa = super::Xl128SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_128_sa_len() {
        let sa = super::Xl128SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_128_sa_empty() {
        let sa = super::Xl128SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_128_rope_slice() {
        let rope = super::Xl128Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_128_sa_search_start() {
        let sa = super::Xl128SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_128_sparse_set_get() {
        let mut m = super::Xm128MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_128_sparse_row_col() {
        let mut m = super::Xm128MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_128_sparse_transpose() {
        let mut m = super::Xm128MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_128_sparse_multiply_vec() {
        let mut m = super::Xm128MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_128_sparse_nnz_density() {
        let mut m = super::Xm128MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_128_sparse_clear() {
        let mut m = super::Xm128MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_128_sparse_overwrite_zero() {
        let mut m = super::Xm128MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_128_tokenizer_basic() {
        let t = super::Xm128Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_128_tokenizer_count() {
        let t = super::Xm128Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_128_tokenizer_unique() {
        let t = super::Xm128Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_128_tokenizer_frequency() {
        let t = super::Xm128Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_128_tokenizer_delimiter() {
        let t = super::Xm128Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_128_tokenizer_whitespace() {
        let t = super::Xm128Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_128_tokenizer_empty() {
        let t = super::Xm128Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 129 ----

    #[test]
    fn xn_129_fenwick_prefix_sum() {
        let mut ft = super::Xn129Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_129_fenwick_range_sum() {
        let mut ft = super::Xn129Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_129_fenwick_point_query() {
        let mut ft = super::Xn129Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_129_fenwick_len() {
        let ft = super::Xn129Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_129_fenwick_multiple_updates() {
        let mut ft = super::Xn129Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_129_fenwick_single_element() {
        let mut ft = super::Xn129Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_129_fenwick_find_kth() {
        let mut ft = super::Xn129Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_129_fenwick_negative_delta() {
        let mut ft = super::Xn129Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 129 ----

    #[test]
    fn xn_129_avl_insert_get() {
        let mut m = super::Xn129AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_129_avl_remove() {
        let mut m = super::Xn129AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_129_avl_in_order() {
        let mut m = super::Xn129AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_129_avl_min_max() {
        let mut m = super::Xn129AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_129_avl_floor_ceiling() {
        let mut m = super::Xn129AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_129_avl_height_balanced() {
        let mut m = super::Xn129AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_129_avl_overwrite() {
        let mut m = super::Xn129AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_129_avl_empty() {
        let m: super::Xn129AVL<i32, i32> = super::Xn129AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo129RedBlack tests ---

    #[test]
    fn xo_129_rb_insert_and_get() {
        let mut tree = super::Xo129RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_129_rb_len_and_empty() {
        let mut tree = super::Xo129RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_129_rb_min_max() {
        let mut tree = super::Xo129RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_129_rb_contains() {
        let mut tree = super::Xo129RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_129_rb_remove() {
        let mut tree = super::Xo129RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_129_rb_in_order() {
        let mut tree = super::Xo129RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_129_rb_black_height() {
        let mut tree = super::Xo129RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_129_rb_overwrite() {
        let mut tree = super::Xo129RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo129ConsistentHash tests ---

    #[test]
    fn xo_129_ch_add_and_count() {
        let mut ring = super::Xo129ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_129_ch_remove_node() {
        let mut ring = super::Xo129ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_129_ch_get_node() {
        let mut ring = super::Xo129ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_129_ch_empty_ring() {
        let ring = super::Xo129ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_129_ch_distribution() {
        let mut ring = super::Xo129ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_129_ch_rebalance() {
        let mut ring = super::Xo129ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_129_ch_virtual_nodes() {
        let mut ring = super::Xo129ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_129_ch_consistent_lookup() {
        let mut ring = super::Xo129ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_128_splay_insert_get() {
        let mut t = super::Xp128SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_128_splay_remove() {
        let mut t = super::Xp128SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_128_splay_count_increases() {
        let mut t = super::Xp128SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_128_splay_depth() {
        let mut t = super::Xp128SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_128_splay_len_empty() {
        let t = super::Xp128SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_128_splay_min_max() {
        let mut t = super::Xp128SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_128_splay_overwrite() {
        let mut t = super::Xp128SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_128_splay_remove_missing() {
        let mut t = super::Xp128SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_129 treap tests ----
    #[test]
    fn xq_129_treap_empty() {
        let t = super::Xq129Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_129_treap_insert_get() {
        let mut t = super::Xq129Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_129_treap_overwrite() {
        let mut t = super::Xq129Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_129_treap_remove() {
        let mut t = super::Xq129Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_129_treap_min_max() {
        let mut t = super::Xq129Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_129_treap_rank() {
        let mut t = super::Xq129Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_129_treap_kth() {
        let mut t = super::Xq129Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_129_treap_in_order() {
        let mut t = super::Xq129Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_129 VEB tree tests ----
    #[test]
    fn xq_129_veb_empty() {
        let v = super::Xq129VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_129_veb_insert_contains() {
        let mut v = super::Xq129VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_129_veb_min_max() {
        let mut v = super::Xq129VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_129_veb_delete() {
        let mut v = super::Xq129VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_129_veb_successor() {
        let mut v = super::Xq129VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_129_veb_predecessor() {
        let mut v = super::Xq129VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_129_veb_count() {
        let mut v = super::Xq129VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_129_veb_duplicate_insert() {
        let mut v = super::Xq129VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_129_kdtree_empty() {
        let tree = super::Xr129KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_129_kdtree_insert_one() {
        let mut tree = super::Xr129KDTree::xr_new();
        tree.xr_insert(super::Xr129KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_129_kdtree_insert_multiple() {
        let mut tree = super::Xr129KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr129KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_129_kdtree_nearest_neighbor() {
        let mut tree = super::Xr129KDTree::xr_new();
        tree.xr_insert(super::Xr129KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr129KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr129KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_129_kdtree_nn_empty() {
        let tree = super::Xr129KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr129KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_129_kdtree_range_search() {
        let mut tree = super::Xr129KDTree::xr_new();
        tree.xr_insert(super::Xr129KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr129KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr129KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_129_kdtree_range_empty() {
        let mut tree = super::Xr129KDTree::xr_new();
        tree.xr_insert(super::Xr129KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_129_kdtree_all_points() {
        let mut tree = super::Xr129KDTree::xr_new();
        tree.xr_insert(super::Xr129KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr129KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_129_kdtree_depth() {
        let mut tree = super::Xr129KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr129KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_129_kdtree_bounding_box() {
        let mut tree = super::Xr129KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr129KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr129KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_128_persistent_array_new() {
        let arr = super::Xs128PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_128_persistent_array_push() {
        let mut arr = super::Xs128PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_128_persistent_array_set() {
        let mut arr = super::Xs128PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_128_persistent_array_diff() {
        let mut arr = super::Xs128PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_128_persistent_array_rollback() {
        let mut arr = super::Xs128PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_128_persistent_array_history() {
        let mut arr = super::Xs128PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_128_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs128PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_128_persistent_array_from_vec() {
        let arr = super::Xs128PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_128_concurrent_queue_new() {
        let q = super::Xs128ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_128_concurrent_queue_push_pop() {
        let mut q = super::Xs128ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_128_concurrent_queue_full() {
        let mut q = super::Xs128ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_128_concurrent_queue_drain() {
        let mut q = super::Xs128ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_128_concurrent_queue_try_pop() {
        let mut q = super::Xs128ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_128_concurrent_queue_clear() {
        let mut q = super::Xs128ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_128_range_map_new() {
        let rm = super::Xs128RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_128_range_map_insert_get() {
        let mut rm = super::Xs128RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_128_range_map_overlap() {
        let mut rm = super::Xs128RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_128_range_map_remove() {
        let mut rm = super::Xs128RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_128_range_map_gaps() {
        let mut rm = super::Xs128RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_128_range_map_coverage() {
        let mut rm = super::Xs128RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_128_range_map_contains() {
        let mut rm = super::Xs128RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_128_range_map_clear() {
        let mut rm = super::Xs128RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_128_circular_buffer_new() {
        let buf = super::Xs128CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_128_circular_buffer_push_pop() {
        let mut buf = super::Xs128CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_128_circular_buffer_overwrite() {
        let mut buf = super::Xs128CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_128_circular_buffer_peek() {
        let mut buf = super::Xs128CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_128_circular_buffer_is_full() {
        let mut buf = super::Xs128CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_128_circular_buffer_iter() {
        let mut buf = super::Xs128CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_128_circular_buffer_clear() {
        let mut buf = super::Xs128CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_128_circular_buffer_to_vec() {
        let mut buf = super::Xs128CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

}
