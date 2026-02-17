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
    fn count_by_severity() {
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

}
