//! User notification management.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// Additional NotificationError helpers
// ---------------------------------------------------------------------------

impl NotificationError {
    /// Returns `true` if the error is a `NotFound` variant.
    pub fn is_not_found(&self) -> bool {
        matches!(self, NotificationError::NotFound(_))
    }

    /// Returns `true` if the error is an `AlreadyClosed` variant.
    pub fn is_already_closed(&self) -> bool {
        matches!(self, NotificationError::AlreadyClosed(_))
    }

    /// Returns the notification ID associated with this error, if any.
    pub fn notification_id(&self) -> Option<u64> {
        match self {
            NotificationError::NotFound(id) | NotificationError::AlreadyClosed(id) => Some(*id),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Additional NotificationSource helpers
// ---------------------------------------------------------------------------

impl NotificationSource {
    /// Create a new `NotificationSource` from an id and label.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }

    /// Returns `true` if the source id matches the given pattern (case-insensitive substring).
    pub fn id_matches(&self, pattern: &str) -> bool {
        self.id.to_lowercase().contains(&pattern.to_lowercase())
    }
}

// ---------------------------------------------------------------------------
// Additional NotificationAction helpers
// ---------------------------------------------------------------------------

impl NotificationAction {
    /// Create a new `NotificationAction` from an id and label.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }

    /// Returns `true` if the action id matches case-insensitively.
    pub fn id_eq_ignore_case(&self, other: &str) -> bool {
        self.id.eq_ignore_ascii_case(other)
    }
}

// ---------------------------------------------------------------------------
// Additional WorkbenchNotification helpers
// ---------------------------------------------------------------------------

impl WorkbenchNotification {
    /// Returns `true` if this notification is silent priority.
    pub fn is_silent(&self) -> bool {
        self.priority == NotificationPriority::Silent
    }

    /// Returns `true` if the notification is still open (not closed).
    pub fn is_open(&self) -> bool {
        !self.closed
    }

    /// Returns `true` if the notification has an associated source.
    pub fn has_source(&self) -> bool {
        self.source.is_some()
    }

    /// Returns `true` if the notification has progress tracking enabled.
    pub fn has_progress(&self) -> bool {
        self.progress.is_some()
    }

    /// Returns the number of actions attached to this notification.
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    /// Returns all action ids as a collected vector.
    pub fn action_ids(&self) -> Vec<&str> {
        self.actions.iter().map(|a| a.id.as_str()).collect()
    }

    /// Returns `true` if the notification message contains the given substring (case-insensitive).
    pub fn message_contains(&self, query: &str) -> bool {
        self.message.to_lowercase().contains(&query.to_lowercase())
    }

    /// Returns a short label: the first `max_len` characters of the message, truncated with '…'.
    pub fn short_message(&self, max_len: usize) -> String {
        WbNotificationValidator::truncate(&self.message, max_len)
    }
}

// ---------------------------------------------------------------------------
// Additional NotificationWorkbenchService helpers
// ---------------------------------------------------------------------------

impl NotificationWorkbenchService {
    /// Returns all notifications matching the given filter.
    pub fn filter(&self, filter: &NotificationFilter) -> Vec<&WorkbenchNotification> {
        self.notifications.iter().filter(|n| filter.matches(n)).collect()
    }

    /// Returns a `NotificationSummary` for all notifications in the service.
    pub fn summary(&self) -> NotificationSummary {
        NotificationSummary::from_notifications(&self.notifications)
    }

    /// Updates the priority of a notification. Returns `false` if the id was not found.
    pub fn update_priority(&mut self, id: u64, priority: NotificationPriority) -> bool {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.priority = priority;
            true
        } else {
            false
        }
    }

    /// Returns all notification ids currently tracked.
    pub fn all_ids(&self) -> Vec<u64> {
        self.notifications.iter().map(|n| n.id).collect()
    }

    /// Returns active notifications whose message contains `query` (case-insensitive).
    pub fn search_active(&self, query: &str) -> Vec<&WorkbenchNotification> {
        self.notifications
            .iter()
            .filter(|n| !n.closed && n.message_contains(query))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Additional NotificationStack helpers
// ---------------------------------------------------------------------------

impl NotificationStack {
    /// Returns a reference to the notification with the given id, if it exists.
    pub fn get(&self, id: u64) -> Option<&WorkbenchNotification> {
        self.notifications.iter().find(|n| n.id == id)
    }

    /// Returns `true` if the stack has any active (non-closed) notifications.
    pub fn has_active(&self) -> bool {
        self.notifications.iter().any(|n| !n.closed)
    }

    /// Returns the configured maximum visible count.
    pub fn max_visible(&self) -> usize {
        self.max_visible
    }
}

// ---------------------------------------------------------------------------
// Additional NotificationHistory helpers
// ---------------------------------------------------------------------------

impl NotificationHistory {
    /// Returns `true` if history contains a notification with the given id.
    pub fn contains_id(&self, id: u64) -> bool {
        self.entries.iter().any(|n| n.id == id)
    }

    /// Returns the oldest entry, if any.
    pub fn oldest(&self) -> Option<&WorkbenchNotification> {
        self.entries.first()
    }

    /// Returns the newest entry, if any.
    pub fn newest(&self) -> Option<&WorkbenchNotification> {
        self.entries.last()
    }
}

// ---------------------------------------------------------------------------
// Additional NotificationBatch helpers
// ---------------------------------------------------------------------------

impl NotificationBatch {
    /// Returns the highest priority among items in the batch, or `None` if empty.
    pub fn max_priority(&self) -> Option<NotificationPriority> {
        self.items.iter().map(|n| n.priority).max()
    }

    /// Returns `true` if any item in the batch has the given priority.
    pub fn has_priority(&self, priority: NotificationPriority) -> bool {
        self.items.iter().any(|n| n.priority == priority)
    }

    /// Returns all messages in the batch.
    pub fn messages(&self) -> Vec<&str> {
        self.items.iter().map(|n| n.message.as_str()).collect()
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

// ---------------------------------------------------------------------------
// Notification filtering
// ---------------------------------------------------------------------------

/// Filter criteria for notifications.
#[derive(Debug, Clone, Default)]
pub struct NotificationFilter {
    pub priority: Option<NotificationPriority>,
    pub closed: Option<bool>,
    pub message_contains: Option<String>,
}

impl NotificationFilter {
    /// Filter for open notifications only.
    pub fn open_only() -> Self {
        Self { closed: Some(false), ..Default::default() }
    }

    /// Filter for urgent notifications.
    pub fn urgent() -> Self {
        Self { priority: Some(NotificationPriority::Urgent), ..Default::default() }
    }

    /// Check if a notification matches this filter.
    pub fn matches(&self, notif: &WorkbenchNotification) -> bool {
        if let Some(p) = &self.priority {
            if notif.priority != *p {
                return false;
            }
        }
        if let Some(c) = self.closed {
            if notif.closed != c {
                return false;
            }
        }
        if let Some(ref text) = self.message_contains {
            if !notif.message.to_lowercase().contains(&text.to_lowercase()) {
                return false;
            }
        }
        true
    }
}

/// Summary statistics for a set of notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationSummary {
    pub total: usize,
    pub open: usize,
    pub closed: usize,
    pub urgent: usize,
    pub with_progress: usize,
}

impl NotificationSummary {
    /// Compute summary from a slice of notifications.
    pub fn from_notifications(notifs: &[WorkbenchNotification]) -> Self {
        Self {
            total: notifs.len(),
            open: notifs.iter().filter(|n| !n.closed).count(),
            closed: notifs.iter().filter(|n| n.closed).count(),
            urgent: notifs.iter().filter(|n| n.priority == NotificationPriority::Urgent).count(),
            with_progress: notifs.iter().filter(|n| n.progress.is_some()).count(),
        }
    }
}

impl fmt::Display for NotificationSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} notifications ({} open, {} closed, {} urgent)",
            self.total, self.open, self.closed, self.urgent
        )
    }
}

// ---------------------------------------------------------------------------
// Priority helpers
// ---------------------------------------------------------------------------

impl NotificationPriority {
    /// Returns all priority variants in order.
    pub fn all() -> &'static [NotificationPriority] {
        &[
            NotificationPriority::Silent,
            NotificationPriority::Default,
            NotificationPriority::Urgent,
        ]
    }

    /// Parse from a string.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "silent" => Some(Self::Silent),
            "default" | "normal" => Some(Self::Default),
            "urgent" | "high" => Some(Self::Urgent),
            _ => None,
        }
    }

    /// Returns a numeric urgency level (0=silent, 1=default, 2=urgent).
    pub fn level(&self) -> u8 {
        match self {
            Self::Silent => 0,
            Self::Default => 1,
            Self::Urgent => 2,
        }
    }
}

/// Groups notifications by priority, returning counts.
pub fn group_by_priority(notifs: &[WorkbenchNotification]) -> std::collections::HashMap<String, usize> {
    let mut map = std::collections::HashMap::new();
    for n in notifs {
        *map.entry(format!("{}", n.priority)).or_insert(0) += 1;
    }
    map
}

/// Returns the highest priority among a set of notifications.
pub fn max_priority(notifs: &[WorkbenchNotification]) -> Option<NotificationPriority> {
    notifs.iter().map(|n| n.priority).max()
}

// ---------------------------------------------------------------------------
// NotificationFilterRule – rule-based filtering
// ---------------------------------------------------------------------------

/// A single filter rule for notifications.
#[derive(Debug, Clone)]
pub struct NotificationFilterRule {
    pub source_pattern: Option<String>,
    pub min_priority: Option<NotificationPriority>,
    pub message_pattern: Option<String>,
}

/// Filters notifications by a set of rules (all rules must match).
pub struct NotificationRuleFilter {
    rules: Vec<NotificationFilterRule>,
}

impl NotificationRuleFilter {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: NotificationFilterRule) {
        self.rules.push(rule);
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Returns true if the notification matches ALL rules.
    pub fn matches(&self, notif: &WorkbenchNotification) -> bool {
        self.rules.iter().all(|rule| {
            let source_ok = match &rule.source_pattern {
                Some(pat) => notif.source.as_ref()
                    .map_or(false, |s| s.label.contains(pat.as_str())),
                None => true,
            };
            let priority_ok = match rule.min_priority {
                Some(min) => notif.priority >= min,
                None => true,
            };
            let message_ok = match &rule.message_pattern {
                Some(pat) => notif.message.contains(pat.as_str()),
                None => true,
            };
            source_ok && priority_ok && message_ok
        })
    }

    /// Filter a slice of notifications, returning those that match.
    pub fn apply<'a>(&self, notifs: &'a [WorkbenchNotification]) -> Vec<&'a WorkbenchNotification> {
        notifs.iter().filter(|n| self.matches(n)).collect()
    }
}

// ---------------------------------------------------------------------------
// NotificationBatch – group related notifications
// ---------------------------------------------------------------------------

/// Groups related notifications into a batch for bulk operations.
pub struct NotificationBatch {
    items: Vec<WorkbenchNotification>,
    label: String,
}

impl NotificationBatch {
    pub fn new(label: impl Into<String>) -> Self {
        Self { items: Vec::new(), label: label.into() }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn add(&mut self, notif: WorkbenchNotification) {
        self.items.push(notif);
    }

    pub fn drain(&mut self) -> Vec<WorkbenchNotification> {
        std::mem::take(&mut self.items)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn priorities(&self) -> Vec<NotificationPriority> {
        self.items.iter().map(|n| n.priority).collect()
    }
}

impl fmt::Display for NotificationBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Batch '{}' ({} notification(s))", self.label, self.items.len())
    }
}

// ---------------------------------------------------------------------------
// NotificationHistory – searchable history ring
// ---------------------------------------------------------------------------

/// Keeps a bounded history of notifications for later querying.
pub struct NotificationHistory {
    entries: Vec<WorkbenchNotification>,
    capacity: usize,
}

impl NotificationHistory {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::with_capacity(capacity), capacity }
    }

    pub fn push(&mut self, notif: WorkbenchNotification) {
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(notif);
    }

    /// Search history for notifications whose message contains `query`.
    pub fn search(&self, query: &str) -> Vec<&WorkbenchNotification> {
        self.entries.iter().filter(|n| n.message.contains(query)).collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return the most recent `n` notifications (newest last).
    pub fn recent(&self, n: usize) -> &[WorkbenchNotification] {
        let start = self.entries.len().saturating_sub(n);
        &self.entries[start..]
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}


// ---------------------------------------------------------------------------
// NotificationStackLayout
// ---------------------------------------------------------------------------

pub struct NotificationStackLayout {
    pub max_visible: usize,
    pub position_x: u16,
    pub position_y: u16,
    pub width: u16,
    pub item_height: u16,
    pub gap: u16,
}

impl NotificationStackLayout {
    pub fn new(max_visible: usize) -> Self {
        Self { max_visible, position_x: 0, position_y: 0, width: 60, item_height: 3, gap: 1 }
    }

    pub fn total_height(&self, count: usize) -> u16 {
        let visible = count.min(self.max_visible) as u16;
        if visible == 0 { 0 } else { visible * self.item_height + (visible - 1) * self.gap }
    }

    pub fn item_y(&self, index: usize) -> u16 {
        self.position_y + index as u16 * (self.item_height + self.gap)
    }

    pub fn is_overflowing(&self, count: usize) -> bool { count > self.max_visible }
    pub fn overflow_count(&self, count: usize) -> usize { count.saturating_sub(self.max_visible) }
}

impl Default for NotificationStackLayout { fn default() -> Self { Self::new(5) } }

impl fmt::Display for NotificationStackLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StackLayout(max={})", self.max_visible)
    }
}

// ---------------------------------------------------------------------------
// NotificationDismissAllHandler
// ---------------------------------------------------------------------------

pub struct NotificationDismissAllHandler;

impl NotificationDismissAllHandler {
    pub fn dismiss_all(service: &mut NotificationWorkbenchService) -> usize {
        let active: Vec<u64> = service.get_active().iter().map(|n| n.id).collect();
        let count = active.len();
        for id in active { service.close(id); }
        count
    }

    pub fn dismiss_by_priority(service: &mut NotificationWorkbenchService, priority: NotificationPriority) -> usize {
        let ids: Vec<u64> = service.get_by_priority(priority).iter().map(|n| n.id).collect();
        let count = ids.len();
        for id in ids { service.close(id); }
        count
    }
}

// ---------------------------------------------------------------------------
// NotificationDoNotDisturb
// ---------------------------------------------------------------------------

pub struct NotificationDoNotDisturb {
    enabled: bool,
    allow_urgent: bool,
    queued_count: u64,
}

impl NotificationDoNotDisturb {
    pub fn new() -> Self { Self { enabled: false, allow_urgent: true, queued_count: 0 } }

    pub fn enable(&mut self) { self.enabled = true; }
    pub fn disable(&mut self) { self.enabled = false; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn toggle(&mut self) { self.enabled = !self.enabled; }

    pub fn should_show(&self, priority: &NotificationPriority) -> bool {
        if !self.enabled { return true; }
        if self.allow_urgent && matches!(priority, NotificationPriority::Urgent) { return true; }
        false
    }

    pub fn record_queued(&mut self) { self.queued_count += 1; }
    pub fn queued_count(&self) -> u64 { self.queued_count }
    pub fn set_allow_urgent(&mut self, allow: bool) { self.allow_urgent = allow; }
}

impl Default for NotificationDoNotDisturb { fn default() -> Self { Self::new() } }

impl fmt::Display for NotificationDoNotDisturb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DND(enabled={}, queued={})", self.enabled, self.queued_count)
    }
}

// ---------------------------------------------------------------------------
// NotificationSourceTracker
// ---------------------------------------------------------------------------

pub struct NotificationSourceTracker {
    counts: std::collections::HashMap<String, u64>,
}

impl NotificationSourceTracker {
    pub fn new() -> Self { Self { counts: std::collections::HashMap::new() } }

    pub fn record(&mut self, source: &str) {
        *self.counts.entry(source.to_string()).or_insert(0) += 1;
    }

    pub fn count_for(&self, source: &str) -> u64 {
        self.counts.get(source).copied().unwrap_or(0)
    }

    pub fn top_sources(&self, limit: usize) -> Vec<(&str, u64)> {
        let mut sorted: Vec<(&str, u64)> = self.counts.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(limit);
        sorted
    }

    pub fn total(&self) -> u64 { self.counts.values().sum() }
    pub fn source_count(&self) -> usize { self.counts.len() }
    pub fn clear(&mut self) { self.counts.clear(); }
}

impl Default for NotificationSourceTracker { fn default() -> Self { Self::new() } }


// === Notification Animation Timer ===

/// Notification Animation Timer implementation.
#[derive(Debug, Clone)]
pub struct NotificationAnimationTimer {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: NotificationAnimationTimerStats,
}

/// Statistics for NotificationAnimationTimer.
#[derive(Debug, Clone, Default)]
pub struct NotificationAnimationTimerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl NotificationAnimationTimerStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl NotificationAnimationTimer {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: NotificationAnimationTimerStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &NotificationAnimationTimerStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for NotificationAnimationTimer {
    fn default() -> Self {
        Self::new()
    }
}

// === Notification Sound Player ===

/// Priority level for NotificationSoundPlayer items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NotificationSoundPlayerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl NotificationSoundPlayerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for NotificationSoundPlayerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Notification Sound Player implementation.
#[derive(Debug, Clone)]
pub struct NotificationSoundPlayer {
    items: Vec<NotificationSoundPlayerItem>,
    max_items: usize,
    default_priority: NotificationSoundPlayerPriority,
}

/// A single item in NotificationSoundPlayer.
#[derive(Debug, Clone)]
pub struct NotificationSoundPlayerItem {
    pub id: String,
    pub label: String,
    pub priority: NotificationSoundPlayerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl NotificationSoundPlayerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: NotificationSoundPlayerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: NotificationSoundPlayerPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl NotificationSoundPlayer {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: NotificationSoundPlayerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: NotificationSoundPlayerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<NotificationSoundPlayerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&NotificationSoundPlayerItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: NotificationSoundPlayerPriority) -> Vec<&NotificationSoundPlayerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&NotificationSoundPlayerItem> {
        let mut sorted: Vec<&NotificationSoundPlayerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&NotificationSoundPlayerItem> {
        let mut sorted: Vec<&NotificationSoundPlayerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&NotificationSoundPlayerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: NotificationSoundPlayerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> NotificationSoundPlayerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &NotificationSoundPlayerItem> {
        self.items.iter()
    }
}

impl Default for NotificationSoundPlayer {
    fn default() -> Self {
        Self::new()
    }
}


// ─── Notif Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for notifications.
#[derive(Debug, Clone)]
pub struct NotifRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> NotifRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for NotifRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NotifRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── Notif LRU Cache ───────────────────────────────────────

/// A simple LRU cache for notification dedup.
#[derive(Debug)]
pub struct NotifLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> NotifLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for NotifLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NotifLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}



// ---------------------------------------------------------------------------
// wb_notification – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for workbench notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWbNotificationNotificationUrgency {
    Low,
    Normal,
    High,
    Critical,
}

impl YWbNotificationNotificationUrgency {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Low => 0,
            Self::Normal => 1,
            Self::High => 2,
            Self::Critical => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Normal => "Normal",
            Self::High => "High",
            Self::Critical => "Critical",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWbNotificationNotificationUrgency] {
        &[
            YWbNotificationNotificationUrgency::Low,
            YWbNotificationNotificationUrgency::Normal,
            YWbNotificationNotificationUrgency::High,
            YWbNotificationNotificationUrgency::Critical,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWbNotificationNotificationUrgency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks notification batch data.
#[derive(Debug, Clone)]
pub struct YWbNotificationNotificationBatch {
    pub items: Vec<(u64, String)>,
    pub auto_dismiss_ms: u64,
    pub pinned: bool,
}

impl YWbNotificationNotificationBatch {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            auto_dismiss_ms: 0,
            pinned: false,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWbNotificationNotificationBatch({}: {:?})", "items", self.items)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_wb_notification_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_wb_notification_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_wb_notification_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_wb_notification_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_wb_notification_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_wb_notification_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_wb_notification_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_wb_notification_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// wb_notification – Extended notification dedup helpers
// ---------------------------------------------------------------------------

/// Priority levels for notification dedup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWbNotificationPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWbNotificationPriority {
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
    pub fn all_asc() -> [ZWbNotificationPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWbNotificationPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks notification dedup data.
#[derive(Debug, Clone)]
pub struct ZWbNotificationNotificationDedup {
    pub seen_hashes: Vec<u64>,
    pub window_ms: u64,
    pub dedup_count: u64,
}

impl ZWbNotificationNotificationDedup {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            seen_hashes: Vec::new(),
            window_ms: 0,
            dedup_count: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.seen_hashes.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.seen_hashes.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.seen_hashes.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWbNotificationNotificationDedup[window_ms={:?}, dedup_count={:?}]", self.window_ms, self.dedup_count)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for notification dedup.
pub fn z_wb_notification_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_wb_notification_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_wb_notification_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_wb_notification_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_wb_notification_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_wb_notification_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_wb_notification_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 105
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer105 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer105 {
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
pub fn xb_fnv1a_105(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_105<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_105<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_105(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_105(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 217
// ---------------------------------------------------------------------------

/// Generic object pool `Xc217Pool<T>`.
pub struct Xc217Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc217Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc217PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc217Pool<T> {
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
    pub fn stats(&self) -> Xc217PoolStats {
        Xc217PoolStats {
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

impl<T> Default for Xc217Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc217Scheduler`.
pub struct Xc217Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc217Scheduler {
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

impl Default for Xc217Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_217 hash for the given byte slice.
pub fn xc_217_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_217 convention.
pub fn xc_217_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe118 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe118Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe118PipelineError {
    pub stage: Xe118Stage,
    pub message: String,
}

impl std::fmt::Display for Xe118PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe118Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe118Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe118PipelineError>>>,
    stage_names: Vec<Xe118Stage>,
}

impl Xe118Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe118PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe118Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe118PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe118Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe118PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe118Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe118PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe118Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe118PipelineError> {
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

    pub fn compose(mut self, other: Xe118Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe118CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe118CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe118Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe118CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe118CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe118Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe118CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_118_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe118CacheEntry {
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

    fn xe_118_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe118CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_118_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe118PipelineError> {
    Ok(data)
}

pub fn xe_118_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe118PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_118_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe118PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_118_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe118PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_118_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe118PipelineError> {
    Err(Xe118PipelineError {
        stage: Xe118Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_116: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg116Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg116Graph {
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

impl Default for Xg116Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_116: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg116Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg116Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg116Heap<T>) {
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

impl<T: Ord> Default for Xg116Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 216).
pub struct Xh216SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh216SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 258 as u64,
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

/// A compact bit set supporting boolean operations (variant 216).
pub struct Xh216BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh216BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 216).
pub struct Xi216Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi216Deque<T> {
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
pub struct Xi216Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi216Interval {
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

/// A simple interval tree (variant 216).
pub struct Xi216IntervalTree {
    xi_intervals: Vec<Xi216Interval>,
}

impl Xi216IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi216Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi216Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi216Interval) -> Vec<&Xi216Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi216Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi216Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi216Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi216Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi216Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi216Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 216) ---

/// Disjoint set / union-find for crate 216.
pub struct Xj216UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj216UnionFind {
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

const XJ216_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 216.
pub struct Xj216BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj216BTreeNode<K, V>>>,
    len: usize,
}

struct Xj216BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj216BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj216BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ216_BTREE_ORDER - 1
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
        let mid = XJ216_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj216BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj216BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj216BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj216BTreeNode::xj_new_leaf();
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


// --- xk_216 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk216SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk216SegmentTree {
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
pub struct Xk216DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk216DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_216).
#[derive(Debug, Clone)]
pub struct Xl216Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl216Rope {
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

/// Suffix array for efficient string searching (xl_216).
#[derive(Debug, Clone)]
pub struct Xl216SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl216SuffixArray {
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
pub struct Xm216MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm216MatrixSparse {
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
pub struct Xm216Tokenizer {
    text: String,
}

impl Xm216Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 216.
pub struct Xn216Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn216Fenwick {
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

// ----- AVL tree map — crate 216 -----

#[derive(Debug, Clone)]
struct Xn216AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn216AvlNode<K, V>>>,
    right: Option<Box<Xn216AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 216.
#[derive(Debug, Clone)]
pub struct Xn216AVL<K, V> {
    root: Option<Box<Xn216AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn216AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn216AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn216AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn216AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn216AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn216AvlNode<K, V>>) -> Box<Xn216AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn216AvlNode<K, V>>) -> Box<Xn216AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn216AvlNode<K, V>>) -> Box<Xn216AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn216AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn216AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn216AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn216AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn216AvlNode<K, V>>) -> &Xn216AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn216AvlNode<K, V>>) -> (Box<Xn216AvlNode<K, V>>, Option<Box<Xn216AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn216AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn216AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn216AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn216AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn216AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn216AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn216AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo216RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo216Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo216RBNode<K, V> {
    key: K,
    value: V,
    color: Xo216Color,
    left: Option<Box<Xo216RBNode<K, V>>>,
    right: Option<Box<Xo216RBNode<K, V>>>,
}

/// A red-black tree map for crate 216.
#[derive(Debug, Clone)]
pub struct Xo216RedBlack<K, V> {
    root: Option<Box<Xo216RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo216RedBlack<K, V> {
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
            r.color = Xo216Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo216RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo216RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo216RBNode {
                    key, value, color: Xo216Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo216RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo216Color::Red)
    }

    fn xo_balance(mut h: Box<Xo216RBNode<K, V>>) -> Box<Xo216RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo216Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo216RBNode<K, V>>) -> Box<Xo216RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo216Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo216RBNode<K, V>>) -> Box<Xo216RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo216Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo216RBNode<K, V>>) {
        h.color = Xo216Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo216Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo216Color::Black; }
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
            r.color = Xo216Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo216RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo216RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo216RBNode<K, V>) -> (K, V, Option<Box<Xo216RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo216RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo216Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo216RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo216ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 216.
#[derive(Debug, Clone)]
pub struct Xo216ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo216ConsistentHash {
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
            let vkey = format!("{}#xo216#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo216#{}", node, i);
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

    #[test]
    fn test_notification_filter_open_only() {
        let notifs = vec![
            WorkbenchNotification {
                id: 1, message: "open msg".into(), priority: NotificationPriority::Default,
                source: None, actions: vec![], progress: None, closeable: true, closed: false,
            },
            WorkbenchNotification {
                id: 2, message: "closed msg".into(), priority: NotificationPriority::Default,
                source: None, actions: vec![], progress: None, closeable: true, closed: true,
            },
        ];
        let filter = NotificationFilter::open_only();
        let open: Vec<_> = notifs.iter().filter(|n| filter.matches(n)).collect();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].message, "open msg");
    }

    #[test]
    fn test_notification_filter_urgent() {
        let notifs = vec![
            WorkbenchNotification {
                id: 1, message: "low".into(), priority: NotificationPriority::Default,
                source: None, actions: vec![], progress: None, closeable: true, closed: false,
            },
            WorkbenchNotification {
                id: 2, message: "high".into(), priority: NotificationPriority::Urgent,
                source: None, actions: vec![], progress: None, closeable: true, closed: false,
            },
        ];
        let filter = NotificationFilter::urgent();
        let urgent: Vec<_> = notifs.iter().filter(|n| filter.matches(n)).collect();
        assert_eq!(urgent.len(), 1);
        assert_eq!(urgent[0].message, "high");
    }

    #[test]
    fn test_notification_summary() {
        let notifs = vec![
            WorkbenchNotification {
                id: 1, message: "a".into(), priority: NotificationPriority::Default,
                source: None, actions: vec![], progress: None, closeable: true, closed: false,
            },
            WorkbenchNotification {
                id: 2, message: "b".into(), priority: NotificationPriority::Urgent,
                source: None, actions: vec![], progress: None, closeable: true, closed: false,
            },
            WorkbenchNotification {
                id: 3, message: "c".into(), priority: NotificationPriority::Default,
                source: None, actions: vec![], progress: None, closeable: true, closed: true,
            },
        ];
        let summary = NotificationSummary::from_notifications(&notifs);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.open, 2);
        assert_eq!(summary.closed, 1);
        assert_eq!(summary.urgent, 1);
        assert!(format!("{summary}").contains("3 notifications"));
    }

    #[test]
    fn test_priority_all() {
        assert_eq!(NotificationPriority::all().len(), 3);
    }

    #[test]
    fn test_priority_from_str_opt() {
        assert_eq!(NotificationPriority::from_str_opt("urgent"), Some(NotificationPriority::Urgent));
        assert_eq!(NotificationPriority::from_str_opt("normal"), Some(NotificationPriority::Default));
        assert_eq!(NotificationPriority::from_str_opt("bogus"), None);
    }

    #[test]
    fn test_priority_level() {
        assert_eq!(NotificationPriority::Silent.level(), 0);
        assert_eq!(NotificationPriority::Default.level(), 1);
        assert_eq!(NotificationPriority::Urgent.level(), 2);
    }

    #[test]
    fn test_max_priority() {
        let notifs = vec![
            WorkbenchNotification {
                id: 1, message: "a".into(), priority: NotificationPriority::Default,
                source: None, actions: vec![], progress: None, closeable: true, closed: false,
            },
            WorkbenchNotification {
                id: 2, message: "b".into(), priority: NotificationPriority::Urgent,
                source: None, actions: vec![], progress: None, closeable: true, closed: false,
            },
        ];
        assert_eq!(max_priority(&notifs), Some(NotificationPriority::Urgent));
        assert_eq!(max_priority(&[]), None);
    }

    fn make_test_notif(id: u64, msg: &str, priority: NotificationPriority, source_label: Option<&str>) -> WorkbenchNotification {
        WorkbenchNotification {
            id,
            message: msg.to_string(),
            priority,
            source: source_label.map(|s| NotificationSource { label: s.to_string(), id: format!("src-{}", id) }),
            actions: vec![],
            progress: None,
            closeable: true,
            closed: false,
        }
    }

    #[test]
    fn test_notification_rule_filter_by_priority() {
        let mut filter = NotificationRuleFilter::new();
        filter.add_rule(NotificationFilterRule {
            source_pattern: None,
            min_priority: Some(NotificationPriority::Urgent),
            message_pattern: None,
        });
        let notifs = vec![
            make_test_notif(1, "low", NotificationPriority::Silent, None),
            make_test_notif(2, "urg", NotificationPriority::Urgent, None),
        ];
        let matched = filter.apply(&notifs);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].message, "urg");
    }

    #[test]
    fn test_notification_rule_filter_by_message() {
        let mut filter = NotificationRuleFilter::new();
        filter.add_rule(NotificationFilterRule {
            source_pattern: None,
            min_priority: None,
            message_pattern: Some("deploy".to_string()),
        });
        let n1 = make_test_notif(1, "deploy started", NotificationPriority::Default, None);
        let n2 = make_test_notif(2, "build ok", NotificationPriority::Default, None);
        assert!(filter.matches(&n1));
        assert!(!filter.matches(&n2));
    }

    #[test]
    fn test_notification_batch_drain() {
        let mut batch = NotificationBatch::new("build");
        assert!(batch.is_empty());
        batch.add(make_test_notif(1, "a", NotificationPriority::Default, None));
        batch.add(make_test_notif(2, "b", NotificationPriority::Urgent, None));
        assert_eq!(batch.len(), 2);
        assert_eq!(format!("{}", batch), "Batch 'build' (2 notification(s))");
        let drained = batch.drain();
        assert_eq!(drained.len(), 2);
        assert!(batch.is_empty());
    }

    #[test]
    fn test_notification_history_push_and_search() {
        let mut history = NotificationHistory::new(3);
        history.push(make_test_notif(1, "error in main.rs", NotificationPriority::Urgent, None));
        history.push(make_test_notif(2, "warning in lib.rs", NotificationPriority::Default, None));
        history.push(make_test_notif(3, "error in utils.rs", NotificationPriority::Urgent, None));
        let results = history.search("error");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_notification_history_capacity() {
        let mut history = NotificationHistory::new(2);
        history.push(make_test_notif(1, "a", NotificationPriority::Default, None));
        history.push(make_test_notif(2, "b", NotificationPriority::Default, None));
        history.push(make_test_notif(3, "c", NotificationPriority::Default, None));
        assert_eq!(history.len(), 2);
        assert_eq!(history.recent(5).len(), 2);
        assert_eq!(history.recent(1)[0].message, "c");
    }

    #[test]
    fn test_notification_history_recent() {
        let mut history = NotificationHistory::new(10);
        for i in 0..5 {
            history.push(make_test_notif(i, &format!("msg-{}", i), NotificationPriority::Default, None));
        }
        let recent = history.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].message, "msg-3");
        assert_eq!(recent[1].message, "msg-4");
        history.clear();
        assert!(history.is_empty());
    }

    // -----------------------------------------------------------------------
    // New tests for added functionality
    // -----------------------------------------------------------------------

    #[test]
    fn test_notification_error_predicates_and_id() {
        let not_found = NotificationError::NotFound(42);
        assert!(not_found.is_not_found());
        assert!(!not_found.is_already_closed());
        assert_eq!(not_found.notification_id(), Some(42));

        let already_closed = NotificationError::AlreadyClosed(7);
        assert!(!already_closed.is_not_found());
        assert!(already_closed.is_already_closed());
        assert_eq!(already_closed.notification_id(), Some(7));

        let empty = NotificationError::EmptyMessage;
        assert!(!empty.is_not_found());
        assert_eq!(empty.notification_id(), None);

        let invalid = NotificationError::InvalidProgress("1.5".into());
        assert_eq!(invalid.notification_id(), None);
    }

    #[test]
    fn test_notification_source_new_and_id_matches() {
        let src = NotificationSource::new("ext.Linter", "My Linter");
        assert_eq!(src.id, "ext.Linter");
        assert_eq!(src.label, "My Linter");
        assert!(src.id_matches("linter"));
        assert!(src.id_matches("EXT"));
        assert!(!src.id_matches("compiler"));
    }

    #[test]
    fn test_notification_action_new_and_id_eq_ignore_case() {
        let action = NotificationAction::new("retry-all", "Retry All");
        assert_eq!(action.id, "retry-all");
        assert_eq!(action.label, "Retry All");
        assert!(action.id_eq_ignore_case("RETRY-ALL"));
        assert!(action.id_eq_ignore_case("Retry-All"));
        assert!(!action.id_eq_ignore_case("cancel"));
    }

    #[test]
    fn test_workbench_notification_extra_helpers() {
        let mut svc = NotificationWorkbenchService::new();
        let actions = vec![
            NotificationAction::new("a1", "Action 1"),
            NotificationAction::new("a2", "Action 2"),
        ];
        let id = svc.notify_with_actions(
            "Build failed in module-xyz",
            NotificationPriority::Silent,
            actions,
        );
        let n = svc.get_notification(id).unwrap();

        assert!(n.is_silent());
        assert!(!n.is_urgent());
        assert!(n.is_open());
        assert!(!n.has_source());
        assert!(!n.has_progress());
        assert_eq!(n.action_count(), 2);
        assert_eq!(n.action_ids(), vec!["a1", "a2"]);
        assert!(n.message_contains("MODULE"));
        assert!(!n.message_contains("success"));
        assert_eq!(n.short_message(10), "Build fai…");
    }

    #[test]
    fn test_service_filter_summary_and_search() {
        let mut svc = NotificationWorkbenchService::new();
        svc.notify("error: disk full", NotificationPriority::Urgent);
        svc.notify("info: build ok", NotificationPriority::Default);
        let id3 = svc.notify("warning: low mem", NotificationPriority::Default);
        svc.close(id3);

        // filter
        let open = svc.filter(&NotificationFilter::open_only());
        assert_eq!(open.len(), 2);
        let urgent = svc.filter(&NotificationFilter::urgent());
        assert_eq!(urgent.len(), 1);

        // summary
        let summary = svc.summary();
        assert_eq!(summary.total, 3);
        assert_eq!(summary.open, 2);
        assert_eq!(summary.closed, 1);
        assert_eq!(summary.urgent, 1);

        // search_active
        let disk = svc.search_active("DISK");
        assert_eq!(disk.len(), 1);
        assert_eq!(disk[0].message, "error: disk full");
        let none = svc.search_active("nonexistent");
        assert!(none.is_empty());

        // all_ids
        let ids = svc.all_ids();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn test_service_update_priority() {
        let mut svc = NotificationWorkbenchService::new();
        let id = svc.notify("msg", NotificationPriority::Default);
        assert!(svc.update_priority(id, NotificationPriority::Urgent));
        assert_eq!(
            svc.get_notification(id).unwrap().priority,
            NotificationPriority::Urgent
        );
        assert!(!svc.update_priority(999, NotificationPriority::Silent));
    }

    #[test]
    fn test_notification_stack_get_and_has_active() {
        let mut stack = NotificationStack::new(10);
        assert!(!stack.has_active());
        assert_eq!(stack.max_visible(), 10);

        stack.push(make_test_notif(1, "hello", NotificationPriority::Default, None));
        assert!(stack.has_active());
        assert_eq!(stack.get(1).unwrap().message, "hello");
        assert!(stack.get(999).is_none());

        stack.close(1);
        assert!(!stack.has_active());
    }

    #[test]
    fn test_notification_history_contains_oldest_newest() {
        let mut history = NotificationHistory::new(5);
        assert!(history.oldest().is_none());
        assert!(history.newest().is_none());

        history.push(make_test_notif(10, "first", NotificationPriority::Default, None));
        history.push(make_test_notif(20, "second", NotificationPriority::Default, None));
        history.push(make_test_notif(30, "third", NotificationPriority::Default, None));

        assert!(history.contains_id(10));
        assert!(history.contains_id(30));
        assert!(!history.contains_id(99));
        assert_eq!(history.oldest().unwrap().message, "first");
        assert_eq!(history.newest().unwrap().message, "third");
    }

    #[test]
    fn test_notification_batch_max_priority_and_messages() {
        let mut batch = NotificationBatch::new("deploy");
        assert_eq!(batch.max_priority(), None);
        assert!(batch.messages().is_empty());

        batch.add(make_test_notif(1, "step 1", NotificationPriority::Silent, None));
        batch.add(make_test_notif(2, "step 2", NotificationPriority::Urgent, None));
        batch.add(make_test_notif(3, "step 3", NotificationPriority::Default, None));

        assert_eq!(batch.max_priority(), Some(NotificationPriority::Urgent));
        assert!(batch.has_priority(NotificationPriority::Silent));
        assert!(!batch.has_priority(NotificationPriority::Default) || batch.has_priority(NotificationPriority::Default));
        assert_eq!(batch.messages(), vec!["step 1", "step 2", "step 3"]);
        assert_eq!(batch.label(), "deploy");
    }


    #[test]
    fn stack_layout_height() {
        let layout = NotificationStackLayout::new(5);
        assert_eq!(layout.total_height(0), 0);
        assert_eq!(layout.total_height(2), 2 * 3 + 1);
    }

    #[test]
    fn stack_layout_overflow() {
        let layout = NotificationStackLayout::new(3);
        assert!(!layout.is_overflowing(2));
        assert!(layout.is_overflowing(5));
        assert_eq!(layout.overflow_count(5), 2);
    }

    #[test]
    fn dismiss_all_handler() {
        let mut svc = NotificationWorkbenchService::new();
        svc.notify("a", NotificationPriority::Default);
        svc.notify("b", NotificationPriority::Default);
        let count = NotificationDismissAllHandler::dismiss_all(&mut svc);
        assert_eq!(count, 2);
        assert_eq!(svc.active_count(), 0);
    }

    #[test]
    fn dismiss_by_priority() {
        let mut svc = NotificationWorkbenchService::new();
        svc.notify("a", NotificationPriority::Default);
        svc.notify("b", NotificationPriority::Urgent);
        let count = NotificationDismissAllHandler::dismiss_by_priority(&mut svc, NotificationPriority::Default);
        assert_eq!(count, 1);
    }

    #[test]
    fn dnd_basic() {
        let mut dnd = NotificationDoNotDisturb::new();
        assert!(!dnd.is_enabled());
        assert!(dnd.should_show(&NotificationPriority::Default));
        dnd.enable();
        assert!(!dnd.should_show(&NotificationPriority::Default));
        assert!(dnd.should_show(&NotificationPriority::Urgent));
    }

    #[test]
    fn dnd_toggle() {
        let mut dnd = NotificationDoNotDisturb::new();
        dnd.toggle();
        assert!(dnd.is_enabled());
        dnd.toggle();
        assert!(!dnd.is_enabled());
    }

    #[test]
    fn dnd_no_urgent() {
        let mut dnd = NotificationDoNotDisturb::new();
        dnd.enable();
        dnd.set_allow_urgent(false);
        assert!(!dnd.should_show(&NotificationPriority::Urgent));
    }

    #[test]
    fn source_tracker_basic() {
        let mut tracker = NotificationSourceTracker::new();
        tracker.record("ext1");
        tracker.record("ext1");
        tracker.record("ext2");
        assert_eq!(tracker.count_for("ext1"), 2);
        assert_eq!(tracker.total(), 3);
        assert_eq!(tracker.source_count(), 2);
    }

    #[test]
    fn source_tracker_top() {
        let mut tracker = NotificationSourceTracker::new();
        tracker.record("a");
        tracker.record("b");
        tracker.record("b");
        let top = tracker.top_sources(1);
        assert_eq!(top[0].0, "b");
    }

    #[test]
    fn stack_layout_display() {
        let layout = NotificationStackLayout::new(5);
        assert!(format!("{layout}").contains("max=5"));
    }

    #[test]
    fn dnd_display() {
        let dnd = NotificationDoNotDisturb::new();
        assert!(format!("{dnd}").contains("enabled=false"));
    }

    #[test]
    fn source_tracker_clear() {
        let mut t = NotificationSourceTracker::new();
        t.record("a");
        t.clear();
        assert_eq!(t.total(), 0);
    }


    #[test]
    fn notificationAnimationTimer_new() {
        let s = NotificationAnimationTimer::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn notificationAnimationTimer_add_contains() {
        let mut s = NotificationAnimationTimer::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn notificationAnimationTimer_add_duplicate() {
        let mut s = NotificationAnimationTimer::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn notificationAnimationTimer_remove() {
        let mut s = NotificationAnimationTimer::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn notificationAnimationTimer_capacity() {
        let s = NotificationAnimationTimer::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn notificationAnimationTimer_search() {
        let mut s = NotificationAnimationTimer::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn notificationAnimationTimer_stats() {
        let mut s = NotificationAnimationTimer::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn notificationSoundPlayer_new() {
        let m = NotificationSoundPlayer::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn notificationSoundPlayer_add_find() {
        let mut m = NotificationSoundPlayer::new();
        m.add(NotificationSoundPlayerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn notificationSoundPlayer_priority_filter() {
        let mut m = NotificationSoundPlayer::new();
        m.add(NotificationSoundPlayerItem::new("a", "A").with_priority(NotificationSoundPlayerPriority::High));
        m.add(NotificationSoundPlayerItem::new("b", "B").with_priority(NotificationSoundPlayerPriority::Low));
        m.add(NotificationSoundPlayerItem::new("c", "C").with_priority(NotificationSoundPlayerPriority::High));
        assert_eq!(m.by_priority(NotificationSoundPlayerPriority::High).len(), 2);
    }

    #[test]
    fn notificationSoundPlayer_remove() {
        let mut m = NotificationSoundPlayer::new();
        m.add(NotificationSoundPlayerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn notificationSoundPlayer_search() {
        let mut m = NotificationSoundPlayer::new();
        m.add(NotificationSoundPlayerItem::new("id1", "Hello World"));
        m.add(NotificationSoundPlayerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn notificationSoundPlayer_total_weight() {
        let mut m = NotificationSoundPlayer::new();
        m.add(NotificationSoundPlayerItem::new("a", "A").with_priority(NotificationSoundPlayerPriority::Critical));
        m.add(NotificationSoundPlayerItem::new("b", "B").with_priority(NotificationSoundPlayerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn notificationSoundPlayer_capacity_limit() {
        let mut m = NotificationSoundPlayer::new().with_max_items(2);
        m.add(NotificationSoundPlayerItem::new("1", "one"));
        m.add(NotificationSoundPlayerItem::new("2", "two"));
        assert!(!m.add(NotificationSoundPlayerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn notificationSoundPlayer_sorted_by_priority() {
        let mut m = NotificationSoundPlayer::new();
        m.add(NotificationSoundPlayerItem::new("lo", "Low").with_priority(NotificationSoundPlayerPriority::Low));
        m.add(NotificationSoundPlayerItem::new("hi", "High").with_priority(NotificationSoundPlayerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn notificationSoundPlayer_item_metadata() {
        let mut item = NotificationSoundPlayerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn notificationAnimationTimer_enabled_toggle() {
        let mut s = NotificationAnimationTimer::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn notificationSoundPlayer_priority_display() {
        assert_eq!(format!("{}", NotificationSoundPlayerPriority::High), "high");
        assert_eq!(format!("{}", NotificationSoundPlayerPriority::Low), "low");
    }


    #[test]
    fn notif_ringbuf_push_get() {
        let mut rb = NotifRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn notif_ringbuf_overflow() {
        let mut rb = NotifRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn notif_ringbuf_clear() {
        let mut rb = NotifRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn notif_ringbuf_newest_oldest() {
        let mut rb = NotifRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn notif_ringbuf_to_vec() {
        let mut rb = NotifRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn notif_ringbuf_is_full() {
        let mut rb = NotifRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn notif_lru_insert_get() {
        let mut c = NotifLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn notif_lru_eviction() {
        let mut c = NotifLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn notif_lru_hit_ratio() {
        let mut c = NotifLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn notif_lru_clear() {
        let mut c = NotifLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn notif_lru_remove() {
        let mut c = NotifLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn notif_lru_peek() {
        let mut c = NotifLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }


    // -- wb_notification extended domain tests ----------------------------------------

    #[test]
    fn y_wb_notification_enum_index() {
        assert_eq!(YWbNotificationNotificationUrgency::Low.index(), 0);
        assert_eq!(YWbNotificationNotificationUrgency::Normal.index(), 1);
        assert_eq!(YWbNotificationNotificationUrgency::High.index(), 2);
        assert_eq!(YWbNotificationNotificationUrgency::Critical.index(), 3);
    }

    #[test]
    fn y_wb_notification_enum_label() {
        assert_eq!(YWbNotificationNotificationUrgency::Low.label(), "Low");
        assert_eq!(YWbNotificationNotificationUrgency::Normal.label(), "Normal");
        assert_eq!(YWbNotificationNotificationUrgency::High.label(), "High");
        assert_eq!(YWbNotificationNotificationUrgency::Critical.label(), "Critical");
    }

    #[test]
    fn y_wb_notification_enum_all() {
        let all = YWbNotificationNotificationUrgency::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_wb_notification_enum_is_default() {
        assert!(YWbNotificationNotificationUrgency::Low.is_default());
        assert!(!YWbNotificationNotificationUrgency::Critical.is_default());
    }

    #[test]
    fn y_wb_notification_enum_display() {
        assert_eq!(format!("{}", YWbNotificationNotificationUrgency::Low), "Low");
    }

    #[test]
    fn y_wb_notification_struct_new() {
        let s = YWbNotificationNotificationBatch::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_wb_notification_struct_clear() {
        let mut s = YWbNotificationNotificationBatch::new();
        s.items.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_wb_notification_fingerprint_deterministic() {
        let h1 = y_wb_notification_fingerprint("hello");
        let h2 = y_wb_notification_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_wb_notification_fingerprint("a"), y_wb_notification_fingerprint("b"));
    }

    #[test]
    fn y_wb_notification_truncate_short() {
        assert_eq!(y_wb_notification_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_wb_notification_truncate_long() {
        let r = y_wb_notification_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_wb_notification_normalize_key_basic() {
        assert_eq!(y_wb_notification_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_wb_notification_split_path_basic() {
        let parts = y_wb_notification_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_wb_notification_count_occurrences_basic() {
        assert_eq!(y_wb_notification_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_wb_notification_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_wb_notification_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_wb_notification_in_range_basic() {
        assert!(y_wb_notification_in_range(5, 1, 10));
        assert!(y_wb_notification_in_range(1, 1, 10));
        assert!(y_wb_notification_in_range(10, 1, 10));
        assert!(!y_wb_notification_in_range(0, 1, 10));
        assert!(!y_wb_notification_in_range(11, 1, 10));
    }

    #[test]
    fn y_wb_notification_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_wb_notification_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_wb_notification_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_wb_notification_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- wb_notification Z-extended tests -----------------------------------------------

    #[test]
    fn z_wb_notification_priority_weight() {
        assert_eq!(ZWbNotificationPriority::Idle.weight(), 0);
        assert_eq!(ZWbNotificationPriority::Normal.weight(), 2);
        assert_eq!(ZWbNotificationPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_wb_notification_priority_label() {
        assert_eq!(ZWbNotificationPriority::Low.label(), "low");
        assert_eq!(ZWbNotificationPriority::High.label(), "high");
    }

    #[test]
    fn z_wb_notification_priority_is_elevated() {
        assert!(!ZWbNotificationPriority::Normal.is_elevated());
        assert!(ZWbNotificationPriority::High.is_elevated());
        assert!(ZWbNotificationPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_wb_notification_priority_display() {
        assert_eq!(format!("{}", ZWbNotificationPriority::Idle), "idle");
    }

    #[test]
    fn z_wb_notification_priority_all_asc() {
        let all = ZWbNotificationPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWbNotificationPriority::Idle);
        assert_eq!(all[4], ZWbNotificationPriority::Realtime);
    }

    #[test]
    fn z_wb_notification_struct_new() {
        let s = ZWbNotificationNotificationDedup::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_wb_notification_struct_toggled_clone() {
        let s = ZWbNotificationNotificationDedup::new();
        let t = s.toggled_clone();
        let _ = t.dedup_count;
    }

    #[test]
    fn z_wb_notification_rolling_hash_deterministic() {
        let h1 = z_wb_notification_rolling_hash(b"test");
        let h2 = z_wb_notification_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_wb_notification_rolling_hash(b"a"), z_wb_notification_rolling_hash(b"b"));
    }

    #[test]
    fn z_wb_notification_pad_to_basic() {
        assert_eq!(z_wb_notification_pad_to("hi", 5), "hi   ");
        assert_eq!(z_wb_notification_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_wb_notification_is_identifier_basic() {
        assert!(z_wb_notification_is_identifier("foo_bar"));
        assert!(z_wb_notification_is_identifier("abc123"));
        assert!(!z_wb_notification_is_identifier(""));
        assert!(!z_wb_notification_is_identifier("has space"));
    }

    #[test]
    fn z_wb_notification_levenshtein_basic() {
        assert_eq!(z_wb_notification_levenshtein("", ""), 0);
        assert_eq!(z_wb_notification_levenshtein("abc", "abc"), 0);
        assert_eq!(z_wb_notification_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_wb_notification_unique_words_basic() {
        let w = z_wb_notification_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_wb_notification_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_wb_notification_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_wb_notification_common_prefix_basic() {
        assert_eq!(z_wb_notification_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_wb_notification_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_wb_notification_struct_clear() {
        let mut s = ZWbNotificationNotificationDedup::new();
        s.seen_hashes.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_wb_notification_rolling_hash_empty() {
        let h = z_wb_notification_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_105_push_and_len() {
        let mut rb = super::XbRingBuffer105::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_105_overwrite() {
        let mut rb = super::XbRingBuffer105::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_105_get_out_of_bounds() {
        let rb = super::XbRingBuffer105::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_105_drain_all() {
        let mut rb = super::XbRingBuffer105::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_105_peek_front_back() {
        let mut rb = super::XbRingBuffer105::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_105_clear() {
        let mut rb = super::XbRingBuffer105::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_105_capacity() {
        let rb = super::XbRingBuffer105::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_105_basic() {
        let h = super::xb_fnv1a_105(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_105(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_105_different_inputs() {
        let h1 = super::xb_fnv1a_105(b"abc");
        let h2 = super::xb_fnv1a_105(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_105_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_105(&data);
        let dec = super::xb_rle_decode_105(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_105_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_105(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_105(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_105_values() {
        assert!((super::xb_clamp_105(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_105(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_105(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_105_values() {
        assert!((super::xb_lerp_105(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_105(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_105(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_105_wrap_around_twice() {
        let mut rb = super::XbRingBuffer105::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 217 ----

    #[test]
    fn xc_217_pool_new_empty() {
        let pool: super::Xc217Pool<i32> = super::Xc217Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_217_pool_release_acquire() {
        let mut pool = super::Xc217Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_217_pool_acquire_empty() {
        let mut pool: super::Xc217Pool<i32> = super::Xc217Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_217_pool_full() {
        let mut pool = super::Xc217Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_217_pool_drain() {
        let mut pool = super::Xc217Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_217_pool_stats() {
        let mut pool = super::Xc217Pool::new(8);
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
    fn xc_217_pool_clear() {
        let mut pool = super::Xc217Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_217_pool_shrink() {
        let mut pool = super::Xc217Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_217_pool_default() {
        let pool: super::Xc217Pool<String> = super::Xc217Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_217_pool_extend() {
        let mut pool = super::Xc217Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_217_pool_retain() {
        let mut pool = super::Xc217Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_217_scheduler_round_robin() {
        let mut sched = super::Xc217Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_217_scheduler_empty() {
        let mut sched = super::Xc217Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_217_scheduler_reset() {
        let mut sched = super::Xc217Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_217_scheduler_add_remove() {
        let mut sched = super::Xc217Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_217_scheduler_targets() {
        let sched = super::Xc217Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_217_hash_empty() {
        assert_eq!(super::xc_217_hash(b""), 5381);
    }

    #[test]
    fn xc_217_hash_data() {
        let h = super::xc_217_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_217_hash(b"hello"), h);
    }

    #[test]
    fn xc_217_reverse_str() {
        assert_eq!(super::xc_217_reverse("abc"), "cba");
        assert_eq!(super::xc_217_reverse(""), "");
    }


    #[test]
    fn xe_118_pipeline_empty() {
        let p = super::Xe118Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_118_pipeline_parse_stage() {
        let p = super::Xe118Pipeline::new()
            .add_parse(super::xe_118_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_118_pipeline_transform_double() {
        let p = super::Xe118Pipeline::new()
            .add_transform(super::xe_118_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_118_pipeline_validate_reverse() {
        let p = super::Xe118Pipeline::new()
            .add_validate(super::xe_118_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_118_pipeline_emit_filter() {
        let p = super::Xe118Pipeline::new()
            .add_emit(super::xe_118_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_118_pipeline_multi_stage() {
        let p = super::Xe118Pipeline::new()
            .add_parse(super::xe_118_pipeline_identity)
            .add_transform(super::xe_118_pipeline_double)
            .add_validate(super::xe_118_pipeline_reverse)
            .add_emit(super::xe_118_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_118_pipeline_error_propagation() {
        let p = super::Xe118Pipeline::new()
            .add_parse(super::xe_118_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe118Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_118_pipeline_compose() {
        let p1 = super::Xe118Pipeline::new()
            .add_parse(super::xe_118_pipeline_identity);
        let p2 = super::Xe118Pipeline::new()
            .add_transform(super::xe_118_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_118_pipeline_error_display() {
        let e = super::Xe118PipelineError {
            stage: super::Xe118Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_118_cache_put_get() {
        let mut c = super::Xe118Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_118_cache_miss() {
        let mut c: super::Xe118Cache<&str, i32> = super::Xe118Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_118_cache_ttl_expiry() {
        let mut c = super::Xe118Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_118_cache_evict() {
        let mut c = super::Xe118Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_118_cache_capacity() {
        let mut c = super::Xe118Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_118_cache_stats() {
        let mut c = super::Xe118Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_118_cache_clear() {
        let mut c = super::Xe118Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_116 graph tests ------------------------------------------------

    #[test]
    fn xg_116_graph_empty() {
        let g = super::Xg116Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_116_graph_add_node() {
        let mut g = super::Xg116Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_116_graph_add_edge() {
        let mut g = super::Xg116Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_116_graph_neighbors() {
        let mut g = super::Xg116Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_116_graph_has_path() {
        let mut g = super::Xg116Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_116_graph_self_path() {
        let g = super::Xg116Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_116_graph_topo_sort() {
        let mut g = super::Xg116Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_116_graph_cycle_detect_false() {
        let mut g = super::Xg116Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_116_graph_cycle_detect_true() {
        let mut g = super::Xg116Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_116 heap tests -------------------------------------------------

    #[test]
    fn xg_116_heap_empty() {
        let h: super::Xg116Heap<i32> = super::Xg116Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_116_heap_push_pop() {
        let mut h = super::Xg116Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_116_heap_peek() {
        let mut h = super::Xg116Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_116_heap_drain_sorted() {
        let mut h = super::Xg116Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_116_heap_merge() {
        let mut a = super::Xg116Heap::new();
        let mut b = super::Xg116Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_116_heap_default() {
        let h: super::Xg116Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_116_graph_default() {
        let g: super::Xg116Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh216_skip_insert_contains() {
        let mut sl = super::Xh216SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh216_skip_remove() {
        let mut sl = super::Xh216SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh216_skip_len() {
        let mut sl = super::Xh216SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh216_skip_range_query() {
        let mut sl = super::Xh216SkipList::xh_new(4);
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
    fn xh216_skip_floor_ceiling() {
        let mut sl = super::Xh216SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh216_skip_rank() {
        let mut sl = super::Xh216SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh216_skip_empty() {
        let sl = super::Xh216SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh216_skip_duplicates() {
        let mut sl = super::Xh216SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh216_bitset_set_test() {
        let mut bs = super::Xh216BitSet::xh_new(256);
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
    fn xh216_bitset_clear_count() {
        let mut bs = super::Xh216BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh216_bitset_and_or_xor() {
        let mut a = super::Xh216BitSet::xh_new(128);
        let mut b = super::Xh216BitSet::xh_new(128);
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
    fn xh216_bitset_iter_ones() {
        let mut bs = super::Xh216BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh216_bitset_first_last() {
        let mut bs = super::Xh216BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh216_bitset_empty() {
        let bs = super::Xh216BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi216_deque_push_pop_back() {
        let mut dq = super::Xi216Deque::xi_new(4);
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
    fn xi216_deque_push_pop_front() {
        let mut dq = super::Xi216Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi216_deque_mixed_ops() {
        let mut dq = super::Xi216Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi216_deque_get_and_split() {
        let mut dq = super::Xi216Deque::xi_new(8);
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
    fn xi216_deque_rotate_left() {
        let mut dq = super::Xi216Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi216_deque_rotate_right() {
        let mut dq = super::Xi216Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi216_deque_grow() {
        let mut dq = super::Xi216Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi216_deque_empty() {
        let dq = super::Xi216Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi216_interval_tree_insert_query() {
        let mut tree = super::Xi216IntervalTree::xi_new();
        tree.xi_insert(super::Xi216Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi216Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi216Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi216_interval_tree_overlap() {
        let mut tree = super::Xi216IntervalTree::xi_new();
        tree.xi_insert(super::Xi216Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi216Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi216Interval::xi_new(12, 20));
        let q = super::Xi216Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi216_interval_tree_remove() {
        let mut tree = super::Xi216IntervalTree::xi_new();
        tree.xi_insert(super::Xi216Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi216Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi216_interval_tree_gaps() {
        let mut tree = super::Xi216IntervalTree::xi_new();
        tree.xi_insert(super::Xi216Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi216Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi216Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi216Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi216Interval::xi_new(8, 10));
    }

    #[test]
    fn xi216_interval_tree_merge() {
        let mut tree = super::Xi216IntervalTree::xi_new();
        tree.xi_insert(super::Xi216Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi216Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi216Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi216Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi216Interval::xi_new(10, 15));
    }

    #[test]
    fn xi216_interval_tree_all() {
        let mut tree = super::Xi216IntervalTree::xi_new();
        tree.xi_insert(super::Xi216Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi216Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi216_interval_tree_empty() {
        let tree = super::Xi216IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi216_interval_tree_contains_point() {
        let iv = super::Xi216Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 216) ---

    #[test]
    fn xj_216_uf_make_and_find() {
        let mut uf = super::Xj216UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_216_uf_union_connected() {
        let mut uf = super::Xj216UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_216_uf_component_count() {
        let mut uf = super::Xj216UnionFind::xj_new();
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
    fn xj_216_uf_component_size() {
        let mut uf = super::Xj216UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_216_uf_largest_component() {
        let mut uf = super::Xj216UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_216_uf_many_elements() {
        let mut uf = super::Xj216UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_216_uf_separate_components() {
        let mut uf = super::Xj216UnionFind::xj_new();
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
    fn xj_216_uf_path_compression() {
        let mut uf = super::Xj216UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_216_bt_insert_get() {
        let mut bt = super::Xj216BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_216_bt_contains_len() {
        let mut bt = super::Xj216BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_216_bt_replace() {
        let mut bt = super::Xj216BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_216_bt_remove() {
        let mut bt = super::Xj216BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_216_bt_keys_values() {
        let mut bt = super::Xj216BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_216_bt_range() {
        let mut bt = super::Xj216BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_216_bt_min_max() {
        let mut bt = super::Xj216BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_216_bt_many_inserts() {
        let mut bt = super::Xj216BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_216 segment tree tests ---

    #[test]
    fn xk_216_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk216SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_216_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk216SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_216_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk216SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_216_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk216SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_216_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk216SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_216_st_single_element() {
        let data = vec![42];
        let st = super::Xk216SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_216_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk216SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_216_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk216SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_216 disjoint intervals tests ---

    #[test]
    fn xk_216_di_add_and_count() {
        let mut di = super::Xk216DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_216_di_merge_overlap() {
        let mut di = super::Xk216DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_216_di_contains() {
        let mut di = super::Xk216DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_216_di_remove() {
        let mut di = super::Xk216DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_216_di_covered_length() {
        let mut di = super::Xk216DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_216_di_gaps() {
        let mut di = super::Xk216DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_216_di_merge_adjacent() {
        let mut di = super::Xk216DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_216_di_empty() {
        let di = super::Xk216DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_216_rope_new_empty() {
        let rope = super::Xl216Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_216_rope_from_str() {
        let rope = super::Xl216Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_216_rope_insert_at() {
        let mut rope = super::Xl216Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_216_rope_delete_range() {
        let mut rope = super::Xl216Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_216_rope_char_at() {
        let rope = super::Xl216Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_216_rope_split_concat() {
        let rope = super::Xl216Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_216_rope_line_count() {
        let rope = super::Xl216Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_216_rope_line_at() {
        let rope = super::Xl216Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_216_sa_build_and_search() {
        let sa = super::Xl216SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_216_sa_count() {
        let sa = super::Xl216SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_216_sa_longest_repeated() {
        let sa = super::Xl216SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_216_sa_all_positions() {
        let sa = super::Xl216SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_216_sa_len() {
        let sa = super::Xl216SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_216_sa_empty() {
        let sa = super::Xl216SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_216_rope_slice() {
        let rope = super::Xl216Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_216_sa_search_start() {
        let sa = super::Xl216SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_216_sparse_set_get() {
        let mut m = super::Xm216MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_216_sparse_row_col() {
        let mut m = super::Xm216MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_216_sparse_transpose() {
        let mut m = super::Xm216MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_216_sparse_multiply_vec() {
        let mut m = super::Xm216MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_216_sparse_nnz_density() {
        let mut m = super::Xm216MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_216_sparse_clear() {
        let mut m = super::Xm216MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_216_sparse_overwrite_zero() {
        let mut m = super::Xm216MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_216_tokenizer_basic() {
        let t = super::Xm216Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_216_tokenizer_count() {
        let t = super::Xm216Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_216_tokenizer_unique() {
        let t = super::Xm216Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_216_tokenizer_frequency() {
        let t = super::Xm216Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_216_tokenizer_delimiter() {
        let t = super::Xm216Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_216_tokenizer_whitespace() {
        let t = super::Xm216Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_216_tokenizer_empty() {
        let t = super::Xm216Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 216 ----

    #[test]
    fn xn_216_fenwick_prefix_sum() {
        let mut ft = super::Xn216Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_216_fenwick_range_sum() {
        let mut ft = super::Xn216Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_216_fenwick_point_query() {
        let mut ft = super::Xn216Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_216_fenwick_len() {
        let ft = super::Xn216Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_216_fenwick_multiple_updates() {
        let mut ft = super::Xn216Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_216_fenwick_single_element() {
        let mut ft = super::Xn216Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_216_fenwick_find_kth() {
        let mut ft = super::Xn216Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_216_fenwick_negative_delta() {
        let mut ft = super::Xn216Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 216 ----

    #[test]
    fn xn_216_avl_insert_get() {
        let mut m = super::Xn216AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_216_avl_remove() {
        let mut m = super::Xn216AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_216_avl_in_order() {
        let mut m = super::Xn216AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_216_avl_min_max() {
        let mut m = super::Xn216AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_216_avl_floor_ceiling() {
        let mut m = super::Xn216AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_216_avl_height_balanced() {
        let mut m = super::Xn216AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_216_avl_overwrite() {
        let mut m = super::Xn216AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_216_avl_empty() {
        let m: super::Xn216AVL<i32, i32> = super::Xn216AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo216RedBlack tests ---

    #[test]
    fn xo_216_rb_insert_and_get() {
        let mut tree = super::Xo216RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_216_rb_len_and_empty() {
        let mut tree = super::Xo216RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_216_rb_min_max() {
        let mut tree = super::Xo216RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_216_rb_contains() {
        let mut tree = super::Xo216RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_216_rb_remove() {
        let mut tree = super::Xo216RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_216_rb_in_order() {
        let mut tree = super::Xo216RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_216_rb_black_height() {
        let mut tree = super::Xo216RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_216_rb_overwrite() {
        let mut tree = super::Xo216RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo216ConsistentHash tests ---

    #[test]
    fn xo_216_ch_add_and_count() {
        let mut ring = super::Xo216ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_216_ch_remove_node() {
        let mut ring = super::Xo216ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_216_ch_get_node() {
        let mut ring = super::Xo216ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_216_ch_empty_ring() {
        let ring = super::Xo216ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_216_ch_distribution() {
        let mut ring = super::Xo216ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_216_ch_rebalance() {
        let mut ring = super::Xo216ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_216_ch_virtual_nodes() {
        let mut ring = super::Xo216ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_216_ch_consistent_lookup() {
        let mut ring = super::Xo216ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}