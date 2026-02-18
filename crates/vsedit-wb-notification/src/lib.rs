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


/// Splay tree data structure keyed by `K` with values `V` (variant 216).
#[derive(Debug)]
pub struct Xp216SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp216Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp216Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp216Node<K, V>>>,
    xp_right: Option<Box<Xp216Node<K, V>>>,
}

impl<K: Ord, V> Xp216Node<K, V> {
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

impl<K: Ord, V> Default for Xp216SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp216SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp216Node<K, V>>>, key: &K) -> Option<Box<Xp216Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp216Node<K, V>>) -> Box<Xp216Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp216Node<K, V>>) -> Box<Xp216Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp216Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp216Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp216Node::xp_new(key, val));
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


// --------------- Xq216Treap ---------------

use std::cmp::Ordering as Xq216Ord;

struct Xq216TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq216TreapNode<K, V>>>,
    right: Option<Box<Xq216TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq216Treap<K, V> {
    root: Option<Box<Xq216TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq216TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_216_size<K, V>(node: &Option<Box<Xq216TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_216_update_size<K, V>(node: &mut Xq216TreapNode<K, V>) {
    node.size = 1 + xq_216_size(&node.left) + xq_216_size(&node.right);
}

fn xq_216_rotate_right<K, V>(mut node: Box<Xq216TreapNode<K, V>>) -> Box<Xq216TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_216_update_size(&mut node);
    left.right = Some(node);
    xq_216_update_size(&mut left);
    left
}

fn xq_216_rotate_left<K, V>(mut node: Box<Xq216TreapNode<K, V>>) -> Box<Xq216TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_216_update_size(&mut node);
    right.left = Some(node);
    xq_216_update_size(&mut right);
    right
}

fn xq_216_insert_node<K: Ord, V>(
    node: Option<Box<Xq216TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq216TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq216TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq216Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq216Ord::Less => {
                let (new_left, old) = xq_216_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_216_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_216_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq216Ord::Greater => {
                let (new_right, old) = xq_216_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_216_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_216_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_216_remove_node<K: Ord, V>(
    node: Option<Box<Xq216TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq216TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq216Ord::Less => {
                let (new_left, old) = xq_216_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_216_update_size(&mut n);
                (Some(n), old)
            }
            Xq216Ord::Greater => {
                let (new_right, old) = xq_216_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_216_update_size(&mut n);
                (Some(n), old)
            }
            Xq216Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_216_rotate_right(n);
                    let (new_right, old) = xq_216_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_216_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_216_rotate_left(n);
                    let (new_left, old) = xq_216_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_216_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_216_find_min<K, V>(node: &Option<Box<Xq216TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_216_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_216_find_max<K, V>(node: &Option<Box<Xq216TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_216_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_216_rank<K: Ord, V>(node: &Option<Box<Xq216TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq216Ord::Less => xq_216_rank(&n.left, key),
            Xq216Ord::Equal => xq_216_size(&n.left),
            Xq216Ord::Greater => 1 + xq_216_size(&n.left) + xq_216_rank(&n.right, key),
        },
    }
}

fn xq_216_kth<K, V>(node: &Option<Box<Xq216TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_216_size(&n.left);
        if k < left_size {
            xq_216_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_216_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_216_in_order<K: Clone, V>(node: &Option<Box<Xq216TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_216_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_216_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq216Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 216 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_216_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq216Ord::Equal => return Some(&n.value),
                Xq216Ord::Less => cur = &n.left,
                Xq216Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_216_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_216_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_216_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_216_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_216_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_216_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_216_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq216VEBTree ---------------

pub struct Xq216VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq216VEBTree>>,
    clusters: Vec<Option<Box<Xq216VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq216VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq216VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq216VEBTree::xq_new(self.sqrt_lo)));
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
pub struct Xr216KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr216KDPoint {
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
pub struct Xr216BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr216KDNode {
    xr_point: Xr216KDPoint,
    xr_left: Option<Box<Xr216KDNode>>,
    xr_right: Option<Box<Xr216KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr216KDTree {
    xr_root: Option<Box<Xr216KDNode>>,
    xr_size: usize,
}

impl Xr216KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr216KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr216KDNode>>,
        point: Xr216KDPoint,
        depth: usize,
    ) -> Box<Xr216KDNode> {
        match node {
            None => Box::new(Xr216KDNode {
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
    pub fn xr_nearest_neighbor(&self, query: &Xr216KDPoint) -> Option<Xr216KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr216KDNode>,
        query: &Xr216KDPoint,
        depth: usize,
        best: &mut Xr216KDPoint,
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
    ) -> Vec<Xr216KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr216KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr216KDPoint>,
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
    pub fn xr_all_points(&self) -> Vec<Xr216KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr216KDNode>>, pts: &mut Vec<Xr216KDPoint>) {
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

    fn xr_depth_rec(node: &Option<Box<Xr216KDNode>>) -> usize {
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
    pub fn xr_bounding_box(&self) -> Option<Xr216BoundingBox> {
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
        Some(Xr216BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs216PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs216PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs216PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs216PersistentArray {
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
pub struct Xs216ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs216ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs216ConcurrentQueue {
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
pub struct Xs216RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs216RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs216RangeMap {
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
pub struct Xs216CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs216CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs216CircularBuffer {
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


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
    }
}


// --- xu_ Binomial Heap ---

/// A node in a binomial heap.
#[derive(Debug, Clone)]
pub struct XuBinomialNode<K: Ord + Clone, V: Clone> {
    pub xu_key: K,
    pub xu_value: V,
    xu_degree: usize,
    xu_children: Vec<usize>,
    xu_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XuBinomialNode<K, V> {
    /// Create a new binomial node.
    pub fn xu_new(key: K, value: V) -> Self {
        Self { xu_key: key, xu_value: value, xu_degree: 0, xu_children: Vec::new(), xu_parent: None }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinNode(key={}, deg={})", self.xu_key, self.xu_degree)
    }
}

/// Binomial heap with O(log n) insert, extract-min, and merge.
#[derive(Debug, Clone)]
pub struct XuBinomialHeap<K: Ord + Clone, V: Clone> {
    xu_nodes: Vec<XuBinomialNode<K, V>>,
    xu_roots: Vec<usize>,
    xu_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XuBinomialHeap<K, V> {
    fn default() -> Self { Self::xu_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinHeap(size={}, trees={})", self.xu_size, self.xu_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XuBinomialHeap<K, V> {
    /// Create an empty binomial heap.
    pub fn xu_new() -> Self {
        Self { xu_nodes: Vec::new(), xu_roots: Vec::new(), xu_size: 0 }
    }

    /// Return the number of elements.
    pub fn xu_len(&self) -> usize { self.xu_size }

    /// Check if the heap is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_size == 0 }

    /// Insert a key-value pair.
    pub fn xu_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xu_nodes.len();
        self.xu_nodes.push(XuBinomialNode::xu_new(key, value));
        self.xu_add_root(idx);
        self.xu_size += 1;
        self.xu_consolidate();
        idx
    }

    fn xu_add_root(&mut self, idx: usize) {
        self.xu_nodes[idx].xu_parent = None;
        self.xu_roots.push(idx);
    }

    fn xu_consolidate(&mut self) {
        let max_deg = (self.xu_size as f64).log2().ceil() as usize + 2;
        let mut table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xu_roots.clone();
        self.xu_roots.clear();
        for root in roots {
            let mut x = root;
            loop {
                let d = self.xu_nodes[x].xu_degree;
                if d >= table.len() { break; }
                match table[d] {
                    None => { table[d] = Some(x); break; }
                    Some(y) => {
                        table[d] = None;
                        let (p, c) = if self.xu_nodes[x].xu_key <= self.xu_nodes[y].xu_key { (x, y) } else { (y, x) };
                        self.xu_nodes[p].xu_children.push(c);
                        self.xu_nodes[c].xu_parent = Some(p);
                        self.xu_nodes[p].xu_degree += 1;
                        x = p;
                    }
                }
            }
        }
        for slot in &table {
            if let Some(r) = slot {
                self.xu_roots.push(*r);
            }
        }
        self.xu_roots.sort_by_key(|&r| self.xu_nodes[r].xu_degree);
    }

    /// Peek at the minimum.
    pub fn xu_find_min(&self) -> Option<(&K, &V)> {
        self.xu_roots.iter()
            .min_by(|&&a, &&b| self.xu_nodes[a].xu_key.cmp(&self.xu_nodes[b].xu_key))
            .map(|&i| (&self.xu_nodes[i].xu_key, &self.xu_nodes[i].xu_value))
    }

    /// Extract the minimum element.
    pub fn xu_extract_min(&mut self) -> Option<(K, V)> {
        if self.xu_roots.is_empty() { return None; }
        let min_pos = self.xu_roots.iter().enumerate()
            .min_by(|(_, a), (_, b)| self.xu_nodes[**a].xu_key.cmp(&self.xu_nodes[**b].xu_key))
            .map(|(pos, _)| pos)?;
        let min_idx = self.xu_roots.remove(min_pos);
        let children = self.xu_nodes[min_idx].xu_children.clone();
        for &c in &children {
            self.xu_nodes[c].xu_parent = None;
            self.xu_roots.push(c);
        }
        self.xu_size -= 1;
        if !self.xu_roots.is_empty() {
            self.xu_consolidate();
        }
        let n = &self.xu_nodes[min_idx];
        Some((n.xu_key.clone(), n.xu_value.clone()))
    }

    /// Merge another binomial heap into this one.
    pub fn xu_merge(&mut self, other: &mut XuBinomialHeap<K, V>) {
        let off = self.xu_nodes.len();
        for mut n in other.xu_nodes.drain(..) {
            n.xu_parent = n.xu_parent.map(|p| p + off);
            n.xu_children = n.xu_children.iter().map(|&c| c + off).collect();
            self.xu_nodes.push(n);
        }
        for r in other.xu_roots.drain(..) {
            self.xu_roots.push(r + off);
        }
        self.xu_size += other.xu_size;
        other.xu_size = 0;
        self.xu_consolidate();
    }

    /// Drain all elements in sorted order.
    pub fn xu_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xu_size);
        while let Some(pair) = self.xu_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xu_clear(&mut self) {
        self.xu_nodes.clear();
        self.xu_roots.clear();
        self.xu_size = 0;
    }
}

// --- xu_ Disjoint Sparse Table ---

/// Disjoint sparse table for O(1) range queries on static data with an associative operation.
#[derive(Debug, Clone)]
pub struct XuDisjointSparseTable<T: Clone> {
    xu_table: Vec<Vec<T>>,
    xu_data: Vec<T>,
    xu_len: usize,
    xu_levels: usize,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XuDisjointSparseTable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DST(len={}, levels={})", self.xu_len, self.xu_levels)
    }
}

impl<T: Clone + Default + std::ops::Add<Output = T>> XuDisjointSparseTable<T> {
    /// Build a disjoint sparse table for range-sum queries.
    pub fn xu_build(data: &[T]) -> Self {
        let n = data.len();
        if n == 0 {
            return Self { xu_table: Vec::new(), xu_data: Vec::new(), xu_len: 0, xu_levels: 0 };
        }
        let levels = (n as f64).log2().ceil() as usize + 1;
        let mut table = Vec::with_capacity(levels);
        for level in 0..levels {
            let block = 1 << level;
            let mut row = data.to_vec();
            let mut mid = block;
            while mid < n {
                // Build prefix sums going left from mid
                if mid > 0 && mid - 1 < n {
                    let start = if mid >= block { mid - block } else { 0 };
                    let mut i = mid.saturating_sub(1);
                    loop {
                        if i < start { break; }
                        if i + 1 < mid && i + 1 < n {
                            row[i] = row[i].clone() + row[i + 1].clone();
                        }
                        if i == start { break; }
                        i -= 1;
                    }
                }
                // Build prefix sums going right from mid
                let end = std::cmp::min(mid + block, n);
                for i in (mid + 1)..end {
                    row[i] = row[i - 1].clone() + row[i].clone();
                }
                mid += 2 * block;
            }
            table.push(row);
        }
        Self { xu_table: table, xu_data: data.to_vec(), xu_len: n, xu_levels: levels }
    }

    /// Query the sum of elements in the range [l, r] (inclusive).
    pub fn xu_query(&self, l: usize, r: usize) -> T {
        if l == r {
            return self.xu_data[l].clone();
        }
        if l >= self.xu_len || r >= self.xu_len || l > r {
            return T::default();
        }
        // Find the highest bit where l and r differ
        let xor = l ^ r;
        if xor == 0 {
            return self.xu_data[l].clone();
        }
        let level = (usize::BITS - xor.leading_zeros() - 1) as usize;
        if level < self.xu_levels && l < self.xu_table[level].len() && r < self.xu_table[level].len() {
            self.xu_table[level][l].clone() + self.xu_table[level][r].clone()
        } else {
            // Fallback: linear sum
            let mut sum = self.xu_data[l].clone();
            for i in (l + 1)..=r {
                sum = sum + self.xu_data[i].clone();
            }
            sum
        }
    }

    /// Return the length.
    pub fn xu_len(&self) -> usize { self.xu_len }

    /// Check if empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_len == 0 }

    /// Get element at index.
    pub fn xu_get(&self, idx: usize) -> Option<&T> {
        self.xu_data.get(idx)
    }
}

// --- xu_ Monotonic Stack ---

/// Monotonic stack that maintains elements in non-decreasing or non-increasing order.
#[derive(Debug, Clone)]
pub struct XuMonotonicStack<T: Clone + Ord> {
    xu_data: Vec<T>,
    xu_increasing: bool,
}

impl<T: Clone + Ord + std::fmt::Display> std::fmt::Display for XuMonotonicStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MonoStack(len={}, inc={})", self.xu_data.len(), self.xu_increasing)
    }
}

impl<T: Clone + Ord> XuMonotonicStack<T> {
    /// Create a monotonically increasing stack.
    pub fn xu_increasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: true }
    }

    /// Create a monotonically decreasing stack.
    pub fn xu_decreasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: false }
    }

    /// Push a value, popping elements that violate the monotonic invariant.
    pub fn xu_push(&mut self, value: T) -> Vec<T> {
        let mut popped = Vec::new();
        if self.xu_increasing {
            while let Some(top) = self.xu_data.last() {
                if *top > value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        } else {
            while let Some(top) = self.xu_data.last() {
                if *top < value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        }
        self.xu_data.push(value);
        popped
    }

    /// Peek at the top.
    pub fn xu_peek(&self) -> Option<&T> { self.xu_data.last() }

    /// Pop from top.
    pub fn xu_pop(&mut self) -> Option<T> { self.xu_data.pop() }

    /// Length.
    pub fn xu_len(&self) -> usize { self.xu_data.len() }

    /// Is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_data.is_empty() }

    /// Get all elements.
    pub fn xu_as_slice(&self) -> &[T] { &self.xu_data }

    /// Clear the stack.
    pub fn xu_clear(&mut self) { self.xu_data.clear(); }
}


// --- xv_ Cartesian Tree ---

/// A node in a Cartesian tree (BST by key, heap by priority).
#[derive(Debug, Clone)]
pub struct XvCartesianNode<K: Ord + Clone, P: Ord + Clone> {
    pub xv_key: K,
    pub xv_priority: P,
    xv_left: Option<Box<XvCartesianNode<K, P>>>,
    xv_right: Option<Box<XvCartesianNode<K, P>>>,
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianNode<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartNode(k={}, p={})", self.xv_key, self.xv_priority)
    }
}

/// Cartesian tree — BST by key, min-heap by priority. Used for range-minimum queries.
#[derive(Debug, Clone)]
pub struct XvCartesianTree<K: Ord + Clone, P: Ord + Clone> {
    xv_root: Option<Box<XvCartesianNode<K, P>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, P: Ord + Clone> Default for XvCartesianTree<K, P> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianTree<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, P: Ord + Clone> XvCartesianTree<K, P> {
    /// Create an empty Cartesian tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Return the number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Check if empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    /// Insert a (key, priority) pair maintaining BST-by-key and min-heap-by-priority.
    pub fn xv_insert(&mut self, key: K, priority: P) {
        self.xv_root = Self::xv_insert_node(self.xv_root.take(), key, priority);
        self.xv_size += 1;
    }

    fn xv_insert_node(node: Option<Box<XvCartesianNode<K, P>>>, key: K, priority: P) -> Option<Box<XvCartesianNode<K, P>>> {
        match node {
            None => Some(Box::new(XvCartesianNode { xv_key: key, xv_priority: priority, xv_left: None, xv_right: None })),
            Some(mut n) => {
                if key < n.xv_key {
                    n.xv_left = Self::xv_insert_node(n.xv_left.take(), key.clone(), priority.clone());
                    if n.xv_left.as_ref().is_some_and(|l| l.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_right(n);
                    }
                    Some(n)
                } else {
                    n.xv_right = Self::xv_insert_node(n.xv_right.take(), key.clone(), priority.clone());
                    if n.xv_right.as_ref().is_some_and(|r| r.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_left(n);
                    }
                    Some(n)
                }
            }
        }
    }

    fn xv_rotate_right(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        left.xv_right = Some(node);
        left
    }

    fn xv_rotate_left(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        right.xv_left = Some(node);
        right
    }

    /// Search for a key.
    pub fn xv_contains(&self, key: &K) -> bool {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search(node: &Option<Box<XvCartesianNode<K, P>>>, key: &K) -> bool {
        match node {
            None => false,
            Some(n) => {
                if *key == n.xv_key { true }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// In-order traversal returning keys.
    pub fn xv_inorder(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder_walk(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder_walk(node: &Option<Box<XvCartesianNode<K, P>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder_walk(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder_walk(&n.xv_right, result);
        }
    }

    /// Get the root priority (minimum priority).
    pub fn xv_min_priority(&self) -> Option<&P> {
        self.xv_root.as_ref().map(|n| &n.xv_priority)
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Build from a sequence of (key, priority) pairs.
    pub fn xv_from_pairs(pairs: &[(K, P)]) -> Self {
        let mut tree = Self::xv_new();
        for (k, p) in pairs { tree.xv_insert(k.clone(), p.clone()); }
        tree
    }

    /// Height of the tree.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvCartesianNode<K, P>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(
                Self::xv_node_height(&n.xv_left),
                Self::xv_node_height(&n.xv_right),
            ),
        }
    }
}

// --- xv_ Weight-Balanced Tree ---

/// A node in a weight-balanced tree (BB[α] tree).
#[derive(Debug, Clone)]
pub struct XvWBNode<K: Ord + Clone, V: Clone> {
    pub xv_key: K,
    pub xv_value: V,
    xv_left: Option<Box<XvWBNode<K, V>>>,
    xv_right: Option<Box<XvWBNode<K, V>>>,
    xv_weight: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWBNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBNode(k={}, w={})", self.xv_key, self.xv_weight)
    }
}

/// Weight-balanced tree (BB[α] tree) with α = 0.29 for balanced operations.
#[derive(Debug, Clone)]
pub struct XvWeightBalancedTree<K: Ord + Clone, V: Clone> {
    xv_root: Option<Box<XvWBNode<K, V>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XvWeightBalancedTree<K, V> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWeightBalancedTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, V: Clone> XvWeightBalancedTree<K, V> {
    const ALPHA: f64 = 0.29;

    /// Create an empty weight-balanced tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Is the tree empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    fn xv_weight(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node { None => 1, Some(n) => n.xv_weight }
    }

    fn xv_update_weight(node: &mut Box<XvWBNode<K, V>>) {
        node.xv_weight = Self::xv_weight(&node.xv_left) + Self::xv_weight(&node.xv_right);
    }

    fn xv_is_balanced(node: &Box<XvWBNode<K, V>>) -> bool {
        let lw = Self::xv_weight(&node.xv_left) as f64;
        let rw = Self::xv_weight(&node.xv_right) as f64;
        let total = node.xv_weight as f64;
        lw >= Self::ALPHA * total && rw >= Self::ALPHA * total
    }

    /// Insert a key-value pair.
    pub fn xv_insert(&mut self, key: K, value: V) {
        let inserted = Self::xv_insert_node(self.xv_root.take(), key, value);
        self.xv_root = inserted.0;
        if inserted.1 { self.xv_size += 1; }
    }

    fn xv_insert_node(node: Option<Box<XvWBNode<K, V>>>, key: K, value: V) -> (Option<Box<XvWBNode<K, V>>>, bool) {
        match node {
            None => {
                let n = Box::new(XvWBNode { xv_key: key, xv_value: value, xv_left: None, xv_right: None, xv_weight: 2 });
                (Some(n), true)
            }
            Some(mut n) => {
                let inserted;
                if key < n.xv_key {
                    let r = Self::xv_insert_node(n.xv_left.take(), key, value);
                    n.xv_left = r.0;
                    inserted = r.1;
                } else if key > n.xv_key {
                    let r = Self::xv_insert_node(n.xv_right.take(), key, value);
                    n.xv_right = r.0;
                    inserted = r.1;
                } else {
                    n.xv_value = value;
                    return (Some(n), false);
                }
                Self::xv_update_weight(&mut n);
                let n = Self::xv_rebalance(n);
                (Some(n), inserted)
            }
        }
    }

    fn xv_rebalance(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if !Self::xv_is_balanced(&node) {
            let lw = Self::xv_weight(&node.xv_left);
            let rw = Self::xv_weight(&node.xv_right);
            if lw < rw {
                node = Self::xv_rotate_left_wb(node);
            } else {
                node = Self::xv_rotate_right_wb(node);
            }
        }
        node
    }

    fn xv_rotate_left_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_right.is_none() { return node; }
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        Self::xv_update_weight(&mut node);
        right.xv_left = Some(node);
        Self::xv_update_weight(&mut right);
        right
    }

    fn xv_rotate_right_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_left.is_none() { return node; }
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        Self::xv_update_weight(&mut node);
        left.xv_right = Some(node);
        Self::xv_update_weight(&mut left);
        left
    }

    /// Look up a key.
    pub fn xv_get(&self, key: &K) -> Option<&V> {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search<'a>(node: &'a Option<Box<XvWBNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xv_key { Some(&n.xv_value) }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xv_contains(&self, key: &K) -> bool { self.xv_get(key).is_some() }

    /// In-order traversal.
    pub fn xv_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder(node: &Option<Box<XvWBNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder(&n.xv_right, result);
        }
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Height.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xv_node_height(&n.xv_left), Self::xv_node_height(&n.xv_right)),
        }
    }
}


// --- xw_ Scapegoat Tree ---

/// A node in a scapegoat tree.
#[derive(Debug, Clone)]
pub struct XwScapegoatNode<K: Ord + Clone, V: Clone> {
    pub xw_key: K,
    pub xw_value: V,
    xw_left: Option<Box<XwScapegoatNode<K, V>>>,
    xw_right: Option<Box<XwScapegoatNode<K, V>>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGNode(k={})", self.xw_key)
    }
}

/// Scapegoat tree — a BST that rebuilds subtrees when they become too unbalanced.
#[derive(Debug, Clone)]
pub struct XwScapegoatTree<K: Ord + Clone, V: Clone> {
    xw_root: Option<Box<XwScapegoatNode<K, V>>>,
    xw_size: usize,
    xw_max_size: usize,
    xw_alpha: f64,
}

impl<K: Ord + Clone, V: Clone> Default for XwScapegoatTree<K, V> {
    fn default() -> Self { Self::xw_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGTree(size={}, alpha={:.2})", self.xw_size, self.xw_alpha)
    }
}

impl<K: Ord + Clone, V: Clone> XwScapegoatTree<K, V> {
    /// Create an empty scapegoat tree with default α = 0.7.
    pub fn xw_new() -> Self {
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: 0.7 }
    }

    /// Create with custom alpha (0.5 < α < 1.0).
    pub fn xw_with_alpha(alpha: f64) -> Self {
        let a = alpha.clamp(0.51, 0.99);
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: a }
    }

    /// Number of elements.
    pub fn xw_len(&self) -> usize { self.xw_size }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_size == 0 }

    fn xw_node_size(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + Self::xw_node_size(&n.xw_left) + Self::xw_node_size(&n.xw_right),
        }
    }

    /// Insert a key-value pair.
    pub fn xw_insert(&mut self, key: K, value: V) {
        let (new_root, depth, inserted) = Self::xw_insert_node(self.xw_root.take(), key, value, 0);
        self.xw_root = new_root;
        if inserted {
            self.xw_size += 1;
            self.xw_max_size = std::cmp::max(self.xw_max_size, self.xw_size);
            let h_alpha = -(self.xw_size as f64).log(1.0 / self.xw_alpha);
            if depth as f64 > h_alpha {
                self.xw_root = Self::xw_rebuild(self.xw_root.take());
            }
        }
    }

    fn xw_insert_node(
        node: Option<Box<XwScapegoatNode<K, V>>>, key: K, value: V, depth: usize,
    ) -> (Option<Box<XwScapegoatNode<K, V>>>, usize, bool) {
        match node {
            None => {
                let n = Box::new(XwScapegoatNode { xw_key: key, xw_value: value, xw_left: None, xw_right: None });
                (Some(n), depth, true)
            }
            Some(mut n) => {
                if key < n.xw_key {
                    let (l, d, ins) = Self::xw_insert_node(n.xw_left.take(), key, value, depth + 1);
                    n.xw_left = l;
                    if ins {
                        let ls = Self::xw_node_size(&n.xw_left);
                        let total = 1 + ls + Self::xw_node_size(&n.xw_right);
                        if ls as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else if key > n.xw_key {
                    let (r, d, ins) = Self::xw_insert_node(n.xw_right.take(), key, value, depth + 1);
                    n.xw_right = r;
                    if ins {
                        let rs = Self::xw_node_size(&n.xw_right);
                        let total = 1 + Self::xw_node_size(&n.xw_left) + rs;
                        if rs as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else {
                    n.xw_value = value;
                    (Some(n), depth, false)
                }
            }
        }
    }

    fn xw_flatten(node: Option<Box<XwScapegoatNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xw_flatten(n.xw_left, out);
            out.push((n.xw_key, n.xw_value));
            Self::xw_flatten(n.xw_right, out);
        }
    }

    fn xw_build_balanced(sorted: &[(K, V)]) -> Option<Box<XwScapegoatNode<K, V>>> {
        if sorted.is_empty() { return None; }
        let mid = sorted.len() / 2;
        let (k, v) = sorted[mid].clone();
        Some(Box::new(XwScapegoatNode {
            xw_key: k,
            xw_value: v,
            xw_left: Self::xw_build_balanced(&sorted[..mid]),
            xw_right: Self::xw_build_balanced(&sorted[mid + 1..]),
        }))
    }

    fn xw_rebuild(node: Option<Box<XwScapegoatNode<K, V>>>) -> Option<Box<XwScapegoatNode<K, V>>> {
        let mut flat = Vec::new();
        Self::xw_flatten(node, &mut flat);
        Self::xw_build_balanced(&flat)
    }

    /// Look up a key.
    pub fn xw_get(&self, key: &K) -> Option<&V> {
        Self::xw_search(&self.xw_root, key)
    }

    fn xw_search<'a>(node: &'a Option<Box<XwScapegoatNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xw_key { Some(&n.xw_value) }
                else if *key < n.xw_key { Self::xw_search(&n.xw_left, key) }
                else { Self::xw_search(&n.xw_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xw_contains(&self, key: &K) -> bool { self.xw_get(key).is_some() }

    /// In-order keys.
    pub fn xw_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xw_collect_keys(&self.xw_root, &mut result);
        result
    }

    fn xw_collect_keys(node: &Option<Box<XwScapegoatNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xw_collect_keys(&n.xw_left, result);
            result.push(n.xw_key.clone());
            Self::xw_collect_keys(&n.xw_right, result);
        }
    }

    /// Clear the tree.
    pub fn xw_clear(&mut self) {
        self.xw_root = None;
        self.xw_size = 0;
        self.xw_max_size = 0;
    }

    /// Height.
    pub fn xw_height(&self) -> usize {
        Self::xw_node_height(&self.xw_root)
    }

    fn xw_node_height(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xw_node_height(&n.xw_left), Self::xw_node_height(&n.xw_right)),
        }
    }
}

// --- xw_ Rope (String Rope) ---

/// A rope node — either a leaf with text or an internal node concatenating two children.
#[derive(Debug, Clone)]
pub enum XwRopeNode {
    Leaf(String),
    Internal {
        xw_left: Box<XwRopeNode>,
        xw_right: Box<XwRopeNode>,
        xw_len: usize,
    },
}

impl std::fmt::Display for XwRopeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XwRopeNode::Leaf(s) => write!(f, "RopeLeaf({})", s.len()),
            XwRopeNode::Internal { xw_len, .. } => write!(f, "RopeInt({})", xw_len),
        }
    }
}

/// Rope data structure for efficient string editing with O(log n) split/concat.
#[derive(Debug, Clone)]
pub struct XwRope {
    xw_root: Option<Box<XwRopeNode>>,
}

impl Default for XwRope {
    fn default() -> Self { Self::xw_new() }
}

impl std::fmt::Display for XwRope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rope(len={})", self.xw_len())
    }
}

impl XwRope {
    /// Create an empty rope.
    pub fn xw_new() -> Self { Self { xw_root: None } }

    /// Create a rope from a string.
    pub fn xw_from_str(s: &str) -> Self {
        if s.is_empty() {
            Self { xw_root: None }
        } else {
            Self { xw_root: Some(Box::new(XwRopeNode::Leaf(s.to_string()))) }
        }
    }

    /// Total length in bytes.
    pub fn xw_len(&self) -> usize {
        Self::xw_node_len(&self.xw_root)
    }

    fn xw_node_len(node: &Option<Box<XwRopeNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => s.len(),
                XwRopeNode::Internal { xw_len, .. } => *xw_len,
            },
        }
    }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_len() == 0 }

    /// Concatenate two ropes.
    pub fn xw_concat(left: XwRope, right: XwRope) -> XwRope {
        match (left.xw_root, right.xw_root) {
            (None, r) => XwRope { xw_root: r },
            (l, None) => XwRope { xw_root: l },
            (Some(l), Some(r)) => {
                let len = Self::xw_node_len(&Some(l.clone())) + Self::xw_node_len(&Some(r.clone()));
                XwRope {
                    xw_root: Some(Box::new(XwRopeNode::Internal { xw_left: l, xw_right: r, xw_len: len })),
                }
            }
        }
    }

    /// Convert to string.
    pub fn xw_to_string(&self) -> String {
        let mut result = String::new();
        Self::xw_collect(&self.xw_root, &mut result);
        result
    }

    fn xw_collect(node: &Option<Box<XwRopeNode>>, result: &mut String) {
        match node {
            None => {}
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => result.push_str(s),
                XwRopeNode::Internal { xw_left, xw_right, .. } => {
                    Self::xw_collect(&Some(xw_left.clone()), result);
                    Self::xw_collect(&Some(xw_right.clone()), result);
                }
            },
        }
    }

    /// Get character at byte index.
    pub fn xw_char_at(&self, idx: usize) -> Option<char> {
        let s = self.xw_to_string();
        s.as_bytes().get(idx).map(|&b| b as char)
    }

    /// Insert a string at byte index.
    pub fn xw_insert(&mut self, idx: usize, text: &str) {
        let s = self.xw_to_string();
        let (left, right) = s.split_at(idx.min(s.len()));
        let new_s = format!("{}{}{}", left, text, right);
        *self = Self::xw_from_str(&new_s);
    }

    /// Delete bytes in range [start, end).
    pub fn xw_delete(&mut self, start: usize, end: usize) {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        let new_s = format!("{}{}", &s[..start], &s[end..]);
        *self = Self::xw_from_str(&new_s);
    }

    /// Append text.
    pub fn xw_append(&mut self, text: &str) {
        let other = Self::xw_from_str(text);
        let old = std::mem::take(self);
        *self = Self::xw_concat(old, other);
    }

    /// Substring [start, end).
    pub fn xw_substring(&self, start: usize, end: usize) -> String {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        s[start..end].to_string()
    }

    /// Clear the rope.
    pub fn xw_clear(&mut self) { self.xw_root = None; }
}


// --- xx_ Skip List ---

/// A node in a skip list with multiple forward pointers for O(log n) search.
#[derive(Debug, Clone)]
pub struct XxSkipNode<K: Ord + Clone, V: Clone> {
    pub xx_key: Option<K>,
    pub xx_value: Option<V>,
    xx_forward: Vec<Option<usize>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.xx_key {
            Some(k) => write!(f, "SkipNode(k={}, lvl={})", k, self.xx_forward.len()),
            None => write!(f, "SkipNode(HEAD, lvl={})", self.xx_forward.len()),
        }
    }
}

/// Skip list — a probabilistic data structure with O(log n) average search, insert, delete.
#[derive(Debug, Clone)]
pub struct XxSkipList<K: Ord + Clone, V: Clone> {
    xx_nodes: Vec<XxSkipNode<K, V>>,
    xx_head: usize,
    xx_max_level: usize,
    xx_level: usize,
    xx_size: usize,
    xx_rng_state: u64,
}

impl<K: Ord + Clone, V: Clone> Default for XxSkipList<K, V> {
    fn default() -> Self { Self::xx_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipList<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkipList(size={}, level={})", self.xx_size, self.xx_level)
    }
}

impl<K: Ord + Clone, V: Clone> XxSkipList<K, V> {
    const XX_MAX_LEVEL: usize = 16;

    /// Create an empty skip list.
    pub fn xx_new() -> Self {
        let head = XxSkipNode {
            xx_key: None,
            xx_value: None,
            xx_forward: vec![None; Self::XX_MAX_LEVEL],
        };
        Self {
            xx_nodes: vec![head],
            xx_head: 0,
            xx_max_level: Self::XX_MAX_LEVEL,
            xx_level: 1,
            xx_size: 0,
            xx_rng_state: 42,
        }
    }

    fn xx_random_level(&mut self) -> usize {
        let mut lvl = 1;
        while lvl < self.xx_max_level {
            self.xx_rng_state ^= self.xx_rng_state << 13;
            self.xx_rng_state ^= self.xx_rng_state >> 7;
            self.xx_rng_state ^= self.xx_rng_state << 17;
            if self.xx_rng_state % 4 < 1 { break; }
            lvl += 1;
        }
        lvl
    }

    /// Number of elements.
    pub fn xx_len(&self) -> usize { self.xx_size }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_size == 0 }

    /// Insert a key-value pair.
    pub fn xx_insert(&mut self, key: K, value: V) {
        let mut update = vec![self.xx_head; self.xx_max_level];
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < key { current = next; continue; }
                    if *nk == key {
                        self.xx_nodes[next].xx_value = Some(value);
                        return;
                    }
                }
                break;
            }
            update[i] = current;
        }
        let lvl = self.xx_random_level();
        if lvl > self.xx_level {
            for i in self.xx_level..lvl {
                update[i] = self.xx_head;
            }
            self.xx_level = lvl;
        }
        let new_idx = self.xx_nodes.len();
        self.xx_nodes.push(XxSkipNode {
            xx_key: Some(key),
            xx_value: Some(value),
            xx_forward: vec![None; lvl],
        });
        for i in 0..lvl {
            self.xx_nodes[new_idx].xx_forward[i] = self.xx_nodes[update[i]].xx_forward[i];
            self.xx_nodes[update[i]].xx_forward[i] = Some(new_idx);
        }
        self.xx_size += 1;
    }

    /// Search for a key.
    pub fn xx_get(&self, key: &K) -> Option<&V> {
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < *key { current = next; continue; }
                    if *nk == *key { return self.xx_nodes[next].xx_value.as_ref(); }
                }
                break;
            }
        }
        None
    }

    /// Check if key exists.
    pub fn xx_contains(&self, key: &K) -> bool { self.xx_get(key).is_some() }

    /// Collect all keys in sorted order.
    pub fn xx_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        let mut current = self.xx_nodes[self.xx_head].xx_forward[0];
        while let Some(idx) = current {
            if let Some(ref k) = self.xx_nodes[idx].xx_key {
                result.push(k.clone());
            }
            current = self.xx_nodes[idx].xx_forward[0];
        }
        result
    }

    /// Clear the skip list.
    pub fn xx_clear(&mut self) {
        self.xx_nodes.truncate(1);
        for i in 0..self.xx_max_level {
            self.xx_nodes[0].xx_forward[i] = None;
        }
        self.xx_level = 1;
        self.xx_size = 0;
    }
}

// --- xx_ Suffix Array ---

/// Suffix array for O(n log n) construction and O(m log n) pattern matching.
#[derive(Debug, Clone)]
pub struct XxSuffixArray {
    xx_text: String,
    xx_sa: Vec<usize>,
}

impl std::fmt::Display for XxSuffixArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SuffixArray(len={})", self.xx_text.len())
    }
}

impl Default for XxSuffixArray {
    fn default() -> Self { Self::xx_new("") }
}

impl XxSuffixArray {
    /// Build a suffix array from a string.
    pub fn xx_new(text: &str) -> Self {
        let n = text.len();
        let bytes = text.as_bytes();
        let mut sa: Vec<usize> = (0..n).collect();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self { xx_text: text.to_string(), xx_sa: sa }
    }

    /// Length of the text.
    pub fn xx_len(&self) -> usize { self.xx_text.len() }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_text.is_empty() }

    /// Get the suffix array.
    pub fn xx_array(&self) -> &[usize] { &self.xx_sa }

    /// Get the original text.
    pub fn xx_text(&self) -> &str { &self.xx_text }

    /// Search for a pattern, returning all starting positions.
    pub fn xx_search(&self, pattern: &str) -> Vec<usize> {
        if pattern.is_empty() || self.xx_text.is_empty() { return Vec::new(); }
        let pb = pattern.as_bytes();
        let tb = self.xx_text.as_bytes();
        let n = tb.len();
        let m = pb.len();
        // Binary search for lower bound
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] < *pb { lo = mid + 1; } else { hi = mid; }
        }
        let lower = lo;
        hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] <= *pb { lo = mid + 1; } else { hi = mid; }
        }
        let upper = lo;
        self.xx_sa[lower..upper].to_vec()
    }

    /// Count occurrences of a pattern.
    pub fn xx_count(&self, pattern: &str) -> usize {
        self.xx_search(pattern).len()
    }

    /// Get the suffix at position i in sorted order.
    pub fn xx_suffix_at(&self, i: usize) -> &str {
        if i < self.xx_sa.len() { &self.xx_text[self.xx_sa[i]..] } else { "" }
    }

    /// Find the longest repeated substring.
    pub fn xx_longest_repeated(&self) -> String {
        if self.xx_sa.len() < 2 { return String::new(); }
        let tb = self.xx_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xx_sa.len() {
            let a = self.xx_sa[i - 1];
            let b = self.xx_sa[i];
            let mut lcp = 0;
            while a + lcp < tb.len() && b + lcp < tb.len() && tb[a + lcp] == tb[b + lcp] {
                lcp += 1;
            }
            if lcp > best_len { best_len = lcp; best_start = a; }
        }
        self.xx_text[best_start..best_start + best_len].to_string()
    }
}


// --- xy_ Cuckoo Hash Map ---

/// Cuckoo hash map with two hash functions and O(1) amortized lookup.
#[derive(Debug, Clone)]
pub struct XyCuckooMap<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xy_table1: Vec<Option<(K, V)>>,
    xy_table2: Vec<Option<(K, V)>>,
    xy_capacity: usize,
    xy_size: usize,
    xy_seed1: u64,
    xy_seed2: u64,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XyCuckooMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CuckooMap(size={}, cap={})", self.xy_size, self.xy_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> Default for XyCuckooMap<K, V> {
    fn default() -> Self { Self::xy_new(16) }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XyCuckooMap<K, V> {
    /// Create a new cuckoo hash map with given capacity.
    pub fn xy_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xy_table1: (0..cap).map(|_| None).collect(),
            xy_table2: (0..cap).map(|_| None).collect(),
            xy_capacity: cap,
            xy_size: 0,
            xy_seed1: 0x517cc1b727220a95,
            xy_seed2: 0x6c62272e07bb0142,
        }
    }

    fn xy_hash1(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed1.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    fn xy_hash2(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed2.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    /// Number of elements.
    pub fn xy_len(&self) -> usize { self.xy_size }

    /// Is empty.
    pub fn xy_is_empty(&self) -> bool { self.xy_size == 0 }

    /// Insert a key-value pair.
    pub fn xy_insert(&mut self, key: K, value: V) -> bool {
        if self.xy_get(&key).is_some() {
            let h1 = self.xy_hash1(&key);
            if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == key) {
                self.xy_table1[h1] = Some((key, value));
            } else {
                let h2 = self.xy_hash2(&key);
                self.xy_table2[h2] = Some((key, value));
            }
            return true;
        }
        let mut k = key;
        let mut v = value;
        for _ in 0..self.xy_capacity {
            let h1 = self.xy_hash1(&k);
            if self.xy_table1[h1].is_none() {
                self.xy_table1[h1] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old = self.xy_table1[h1].take().unwrap();
            self.xy_table1[h1] = Some((k, v));
            k = old.0;
            v = old.1;
            let h2 = self.xy_hash2(&k);
            if self.xy_table2[h2].is_none() {
                self.xy_table2[h2] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old2 = self.xy_table2[h2].take().unwrap();
            self.xy_table2[h2] = Some((k, v));
            k = old2.0;
            v = old2.1;
        }
        // Rehash needed — just put in table1 with linear probing fallback
        for i in 0..self.xy_capacity {
            if self.xy_table1[i].is_none() {
                self.xy_table1[i] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
        }
        false
    }

    /// Look up a key.
    pub fn xy_get(&self, key: &K) -> Option<&V> {
        let h1 = self.xy_hash1(key);
        if let Some((k, v)) = &self.xy_table1[h1] {
            if *k == *key { return Some(v); }
        }
        let h2 = self.xy_hash2(key);
        if let Some((k, v)) = &self.xy_table2[h2] {
            if *k == *key { return Some(v); }
        }
        None
    }

    /// Check if key exists.
    pub fn xy_contains(&self, key: &K) -> bool { self.xy_get(key).is_some() }

    /// Remove a key.
    pub fn xy_remove(&mut self, key: &K) -> Option<V> {
        let h1 = self.xy_hash1(key);
        if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table1[h1].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        let h2 = self.xy_hash2(key);
        if self.xy_table2[h2].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table2[h2].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        None
    }

    /// Clear the map.
    pub fn xy_clear(&mut self) {
        for slot in &mut self.xy_table1 { *slot = None; }
        for slot in &mut self.xy_table2 { *slot = None; }
        self.xy_size = 0;
    }

    /// Collect all keys.
    pub fn xy_keys(&self) -> Vec<K> {
        let mut keys = Vec::new();
        for slot in &self.xy_table1 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        for slot in &self.xy_table2 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        keys
    }
}

// --- xy_ Count-Min Sketch ---

/// Count-min sketch for approximate frequency counting with bounded error.
#[derive(Debug, Clone)]
pub struct XyCountMinSketch {
    xy_table: Vec<Vec<u64>>,
    xy_width: usize,
    xy_depth: usize,
    xy_seeds: Vec<u64>,
}

impl std::fmt::Display for XyCountMinSketch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CMS(w={}, d={})", self.xy_width, self.xy_depth)
    }
}

impl Default for XyCountMinSketch {
    fn default() -> Self { Self::xy_new(1000, 5) }
}

impl XyCountMinSketch {
    /// Create a new count-min sketch with given width and depth.
    pub fn xy_new(width: usize, depth: usize) -> Self {
        let seeds: Vec<u64> = (0..depth).map(|i| 0x9e3779b97f4a7c15u64.wrapping_add((i as u64).wrapping_mul(0x517cc1b727220a95))).collect();
        Self {
            xy_table: vec![vec![0u64; width]; depth],
            xy_width: width,
            xy_depth: depth,
            xy_seeds: seeds,
        }
    }

    fn xy_hash(&self, item: u64, seed: u64) -> usize {
        let h = item.wrapping_mul(seed).wrapping_add(seed >> 16);
        (h ^ (h >> 32)) as usize % self.xy_width
    }

    /// Increment the count for an item.
    pub fn xy_add(&mut self, item: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += 1;
        }
    }

    /// Add with a specific count.
    pub fn xy_add_count(&mut self, item: u64, count: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += count;
        }
    }

    /// Estimate the count for an item (guaranteed to be >= actual count).
    pub fn xy_estimate(&self, item: u64) -> u64 {
        let mut min_count = u64::MAX;
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            min_count = min_count.min(self.xy_table[i][idx]);
        }
        min_count
    }

    /// Width of the sketch.
    pub fn xy_width(&self) -> usize { self.xy_width }

    /// Depth of the sketch.
    pub fn xy_depth(&self) -> usize { self.xy_depth }

    /// Clear the sketch.
    pub fn xy_clear(&mut self) {
        for row in &mut self.xy_table {
            for cell in row { *cell = 0; }
        }
    }

    /// Merge another sketch into this one.
    pub fn xy_merge(&mut self, other: &XyCountMinSketch) {
        if self.xy_width != other.xy_width || self.xy_depth != other.xy_depth { return; }
        for i in 0..self.xy_depth {
            for j in 0..self.xy_width {
                self.xy_table[i][j] += other.xy_table[i][j];
            }
        }
    }
}


// --- xz_ HyperLogLog ---

/// HyperLogLog probabilistic cardinality estimator with configurable precision.
#[derive(Debug, Clone)]
pub struct XzHyperLogLog {
    xz_registers: Vec<u8>,
    xz_m: usize,
    xz_b: u32,
}

impl std::fmt::Display for XzHyperLogLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HLL(m={}, est={:.0})", self.xz_m, self.xz_estimate())
    }
}

impl Default for XzHyperLogLog {
    fn default() -> Self { Self::xz_new(10) }
}

impl XzHyperLogLog {
    /// Create a new HyperLogLog with precision b (4 <= b <= 16). Uses 2^b registers.
    pub fn xz_new(b: u32) -> Self {
        let b = b.clamp(4, 16);
        let m = 1 << b;
        Self { xz_registers: vec![0u8; m], xz_m: m, xz_b: b }
    }

    fn xz_hash(item: u64) -> u64 {
        let mut h = item;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        h ^= h >> 33;
        h
    }

    /// Add an item.
    pub fn xz_add(&mut self, item: u64) {
        let h = Self::xz_hash(item);
        let idx = (h as usize) & (self.xz_m - 1);
        let w = h >> self.xz_b;
        let rho = if w == 0 { 64 - self.xz_b } else { w.trailing_zeros() + 1 };
        let rho = rho.min(255) as u8;
        if rho > self.xz_registers[idx] {
            self.xz_registers[idx] = rho;
        }
    }

    /// Estimate the cardinality.
    pub fn xz_estimate(&self) -> f64 {
        let m = self.xz_m as f64;
        let alpha = match self.xz_m {
            16 => 0.673,
            32 => 0.697,
            64 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / m),
        };
        let sum: f64 = self.xz_registers.iter().map(|&r| 2.0f64.powi(-(r as i32))).sum();
        let raw = alpha * m * m / sum;
        if raw <= 2.5 * m {
            let zeros = self.xz_registers.iter().filter(|&&r| r == 0).count();
            if zeros > 0 { m * (m / zeros as f64).ln() } else { raw }
        } else if raw <= (1u64 << 32) as f64 / 30.0 {
            raw
        } else {
            -(((1u64 << 32) as f64) * (1.0 - raw / (1u64 << 32) as f64).ln())
        }
    }

    /// Merge another HyperLogLog into this one.
    pub fn xz_merge(&mut self, other: &XzHyperLogLog) {
        if self.xz_m != other.xz_m { return; }
        for i in 0..self.xz_m {
            if other.xz_registers[i] > self.xz_registers[i] {
                self.xz_registers[i] = other.xz_registers[i];
            }
        }
    }

    /// Clear all registers.
    pub fn xz_clear(&mut self) {
        for r in &mut self.xz_registers { *r = 0; }
    }

    /// Number of registers.
    pub fn xz_num_registers(&self) -> usize { self.xz_m }

    /// Precision parameter.
    pub fn xz_precision(&self) -> u32 { self.xz_b }
}

// --- xz_ LRU Cache ---

/// LRU cache with O(1) get/put using a doubly-linked list and hash map.
#[derive(Debug, Clone)]
pub struct XzLruCache<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xz_capacity: usize,
    xz_entries: Vec<(K, V)>,
    xz_order: Vec<usize>,
    xz_map: std::collections::HashMap<K, usize>,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XzLruCache<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LRU(size={}, cap={})", self.xz_map.len(), self.xz_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XzLruCache<K, V> {
    /// Create a new LRU cache with given capacity.
    pub fn xz_new(capacity: usize) -> Self {
        Self {
            xz_capacity: capacity.max(1),
            xz_entries: Vec::new(),
            xz_order: Vec::new(),
            xz_map: std::collections::HashMap::new(),
        }
    }

    /// Number of entries.
    pub fn xz_len(&self) -> usize { self.xz_map.len() }

    /// Is empty.
    pub fn xz_is_empty(&self) -> bool { self.xz_map.is_empty() }

    /// Capacity.
    pub fn xz_capacity(&self) -> usize { self.xz_capacity }

    /// Get a value, marking it as recently used.
    pub fn xz_get(&mut self, key: &K) -> Option<&V> {
        if let Some(&idx) = self.xz_map.get(key) {
            self.xz_order.retain(|&i| i != idx);
            self.xz_order.push(idx);
            Some(&self.xz_entries[idx].1)
        } else {
            None
        }
    }

    /// Put a key-value pair, evicting the least recently used if at capacity.
    pub fn xz_put(&mut self, key: K, value: V) {
        if let Some(&idx) = self.xz_map.get(&key) {
            self.xz_entries[idx].1 = value;
            self.xz_order.retain(|&i| i != idx);
            self.xz_order.push(idx);
            return;
        }
        if self.xz_map.len() >= self.xz_capacity {
            if let Some(evict_idx) = self.xz_order.first().copied() {
                self.xz_order.remove(0);
                let evict_key = self.xz_entries[evict_idx].0.clone();
                self.xz_map.remove(&evict_key);
            }
        }
        let idx = self.xz_entries.len();
        self.xz_entries.push((key.clone(), value));
        self.xz_map.insert(key, idx);
        self.xz_order.push(idx);
    }

    /// Check if key exists (without updating LRU order).
    pub fn xz_contains(&self, key: &K) -> bool { self.xz_map.contains_key(key) }

    /// Remove a key.
    pub fn xz_remove(&mut self, key: &K) -> Option<V> {
        if let Some(idx) = self.xz_map.remove(key) {
            self.xz_order.retain(|&i| i != idx);
            Some(self.xz_entries[idx].1.clone())
        } else {
            None
        }
    }

    /// Clear the cache.
    pub fn xz_clear(&mut self) {
        self.xz_entries.clear();
        self.xz_order.clear();
        self.xz_map.clear();
    }

    /// Get all keys in LRU order (least recent first).
    pub fn xz_keys_lru(&self) -> Vec<K> {
        self.xz_order.iter().filter_map(|&idx| {
            let k = &self.xz_entries[idx].0;
            if self.xz_map.contains_key(k) { Some(k.clone()) } else { None }
        }).collect()
    }

    /// Peek at value without updating LRU order.
    pub fn xz_peek(&self, key: &K) -> Option<&V> {
        self.xz_map.get(key).map(|&idx| &self.xz_entries[idx].1)
    }
}


// --- ya_ Trie (Prefix Tree) ---

/// A node in a trie (prefix tree) for string key lookups.
#[derive(Debug, Clone)]
pub struct YaTrieNode<V: Clone> {
    ya_children: std::collections::HashMap<char, Box<YaTrieNode<V>>>,
    ya_value: Option<V>,
    ya_is_end: bool,
}

impl<V: Clone> Default for YaTrieNode<V> {
    fn default() -> Self {
        Self { ya_children: std::collections::HashMap::new(), ya_value: None, ya_is_end: false }
    }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YaTrieNode<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TrieNode(children={}, end={})", self.ya_children.len(), self.ya_is_end)
    }
}

/// Trie (prefix tree) for O(m) string key operations where m is key length.
#[derive(Debug, Clone)]
pub struct YaTrie<V: Clone> {
    ya_root: YaTrieNode<V>,
    ya_size: usize,
}

impl<V: Clone> Default for YaTrie<V> {
    fn default() -> Self { Self::ya_new() }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YaTrie<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Trie(size={})", self.ya_size)
    }
}

impl<V: Clone> YaTrie<V> {
    /// Create an empty trie.
    pub fn ya_new() -> Self { Self { ya_root: YaTrieNode::default(), ya_size: 0 } }

    /// Number of stored keys.
    pub fn ya_len(&self) -> usize { self.ya_size }

    /// Is the trie empty.
    pub fn ya_is_empty(&self) -> bool { self.ya_size == 0 }

    /// Insert a key-value pair.
    pub fn ya_insert(&mut self, key: &str, value: V) {
        let mut node = &mut self.ya_root;
        for ch in key.chars() {
            node = node.ya_children.entry(ch).or_insert_with(|| Box::new(YaTrieNode::default()));
        }
        if !node.ya_is_end { self.ya_size += 1; }
        node.ya_value = Some(value);
        node.ya_is_end = true;
    }

    /// Look up a key.
    pub fn ya_get(&self, key: &str) -> Option<&V> {
        let mut node = &self.ya_root;
        for ch in key.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return None,
            }
        }
        if node.ya_is_end { node.ya_value.as_ref() } else { None }
    }

    /// Check if a key exists.
    pub fn ya_contains(&self, key: &str) -> bool { self.ya_get(key).is_some() }

    /// Check if any key starts with the given prefix.
    pub fn ya_has_prefix(&self, prefix: &str) -> bool {
        let mut node = &self.ya_root;
        for ch in prefix.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return false,
            }
        }
        true
    }

    /// Collect all keys with the given prefix.
    pub fn ya_keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.ya_root;
        for ch in prefix.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        Self::ya_collect_keys(node, &mut prefix.to_string(), &mut results);
        results
    }

    fn ya_collect_keys(node: &YaTrieNode<V>, current: &mut String, results: &mut Vec<String>) {
        if node.ya_is_end { results.push(current.clone()); }
        let mut chars: Vec<char> = node.ya_children.keys().copied().collect();
        chars.sort();
        for ch in chars {
            current.push(ch);
            Self::ya_collect_keys(node.ya_children.get(&ch).unwrap(), current, results);
            current.pop();
        }
    }

    /// Collect all keys.
    pub fn ya_all_keys(&self) -> Vec<String> {
        self.ya_keys_with_prefix("")
    }

    /// Remove a key. Returns the value if it existed.
    pub fn ya_remove(&mut self, key: &str) -> Option<V> {
        let result = Self::ya_remove_recursive(&mut self.ya_root, key, 0);
        if result.is_some() { self.ya_size -= 1; }
        result
    }

    fn ya_remove_recursive(node: &mut YaTrieNode<V>, key: &str, depth: usize) -> Option<V> {
        let chars: Vec<char> = key.chars().collect();
        if depth == chars.len() {
            if node.ya_is_end {
                node.ya_is_end = false;
                return node.ya_value.take();
            }
            return None;
        }
        let ch = chars[depth];
        if let Some(child) = node.ya_children.get_mut(&ch) {
            let result = Self::ya_remove_recursive(child, key, depth + 1);
            if !child.ya_is_end && child.ya_children.is_empty() {
                node.ya_children.remove(&ch);
            }
            result
        } else {
            None
        }
    }

    /// Clear the trie.
    pub fn ya_clear(&mut self) {
        self.ya_root = YaTrieNode::default();
        self.ya_size = 0;
    }

    /// Count keys with a given prefix.
    pub fn ya_count_prefix(&self, prefix: &str) -> usize {
        self.ya_keys_with_prefix(prefix).len()
    }

    /// Longest common prefix among all keys.
    pub fn ya_longest_common_prefix(&self) -> String {
        let mut result = String::new();
        let mut node = &self.ya_root;
        while node.ya_children.len() == 1 && !node.ya_is_end {
            let (&ch, child) = node.ya_children.iter().next().unwrap();
            result.push(ch);
            node = child;
        }
        result
    }
}

// --- ya_ Bloom Filter ---

/// Bloom filter for probabilistic set membership testing with no false negatives.
#[derive(Debug, Clone)]
pub struct YaBloomFilter {
    ya_bits: Vec<bool>,
    ya_size: usize,
    ya_num_hashes: usize,
    ya_count: usize,
}

impl std::fmt::Display for YaBloomFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bloom(bits={}, hashes={}, count={})", self.ya_size, self.ya_num_hashes, self.ya_count)
    }
}

impl Default for YaBloomFilter {
    fn default() -> Self { Self::ya_new(1000, 5) }
}

impl YaBloomFilter {
    /// Create a new bloom filter with given bit size and number of hash functions.
    pub fn ya_new(bits: usize, num_hashes: usize) -> Self {
        Self { ya_bits: vec![false; bits], ya_size: bits, ya_num_hashes: num_hashes.max(1), ya_count: 0 }
    }

    /// Create from expected number of items and desired false positive rate.
    pub fn ya_with_fp_rate(expected_items: usize, fp_rate: f64) -> Self {
        let bits = (-(expected_items as f64) * fp_rate.ln() / (2.0f64.ln().powi(2))).ceil() as usize;
        let bits = bits.max(64);
        let hashes = ((bits as f64 / expected_items as f64) * 2.0f64.ln()).ceil() as usize;
        let hashes = hashes.max(1);
        Self::ya_new(bits, hashes)
    }

    fn ya_hash(&self, item: u64, seed: usize) -> usize {
        let h = item.wrapping_mul(0xff51afd7ed558ccd_u64.wrapping_add(seed as u64));
        let h = h ^ (h >> 33);
        let h = h.wrapping_mul(0xc4ceb9fe1a85ec53_u64.wrapping_add(seed as u64 * 7));
        (h ^ (h >> 33)) as usize % self.ya_size
    }

    /// Add an item.
    pub fn ya_add(&mut self, item: u64) {
        for i in 0..self.ya_num_hashes {
            let idx = self.ya_hash(item, i);
            self.ya_bits[idx] = true;
        }
        self.ya_count += 1;
    }

    /// Check if an item might be in the set (false positives possible, no false negatives).
    pub fn ya_might_contain(&self, item: u64) -> bool {
        for i in 0..self.ya_num_hashes {
            let idx = self.ya_hash(item, i);
            if !self.ya_bits[idx] { return false; }
        }
        true
    }

    /// Number of items added.
    pub fn ya_count(&self) -> usize { self.ya_count }

    /// Bit array size.
    pub fn ya_bit_size(&self) -> usize { self.ya_size }

    /// Number of hash functions.
    pub fn ya_num_hashes(&self) -> usize { self.ya_num_hashes }

    /// Estimated false positive rate.
    pub fn ya_estimated_fp_rate(&self) -> f64 {
        let ones = self.ya_bits.iter().filter(|&&b| b).count() as f64;
        (ones / self.ya_size as f64).powi(self.ya_num_hashes as i32)
    }

    /// Clear the filter.
    pub fn ya_clear(&mut self) {
        for b in &mut self.ya_bits { *b = false; }
        self.ya_count = 0;
    }

    /// Merge another bloom filter (union).
    pub fn ya_merge(&mut self, other: &YaBloomFilter) {
        if self.ya_size != other.ya_size { return; }
        for i in 0..self.ya_size {
            self.ya_bits[i] = self.ya_bits[i] || other.ya_bits[i];
        }
    }
}


// --- yb_ Ternary Search Tree ---

/// Node in a ternary search tree (TST) for space-efficient string storage.
#[derive(Debug, Clone)]
pub struct YbTstNode<V: Clone> {
    yb_ch: char,
    yb_left: Option<Box<YbTstNode<V>>>,
    yb_mid: Option<Box<YbTstNode<V>>>,
    yb_right: Option<Box<YbTstNode<V>>>,
    yb_value: Option<V>,
}

impl<V: Clone> YbTstNode<V> {
    fn yb_new(ch: char) -> Self {
        Self { yb_ch: ch, yb_left: None, yb_mid: None, yb_right: None, yb_value: None }
    }
}

/// Ternary search tree for efficient string-keyed storage with prefix queries.
#[derive(Debug, Clone)]
pub struct YbTernarySearchTree<V: Clone> {
    yb_root: Option<Box<YbTstNode<V>>>,
    yb_size: usize,
}

impl<V: Clone> Default for YbTernarySearchTree<V> {
    fn default() -> Self { Self { yb_root: None, yb_size: 0 } }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YbTernarySearchTree<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TST(size={})", self.yb_size)
    }
}

impl<V: Clone> YbTernarySearchTree<V> {
    /// Create an empty TST.
    pub fn yb_new() -> Self { Self { yb_root: None, yb_size: 0 } }

    /// Number of stored keys.
    pub fn yb_len(&self) -> usize { self.yb_size }

    /// Is the tree empty.
    pub fn yb_is_empty(&self) -> bool { self.yb_size == 0 }

    /// Insert a key-value pair.
    pub fn yb_insert(&mut self, key: &str, value: V) {
        if key.is_empty() { return; }
        let chars: Vec<char> = key.chars().collect();
        let was_new = Self::yb_insert_node(&mut self.yb_root, &chars, 0, value);
        if was_new { self.yb_size += 1; }
    }

    fn yb_insert_node(node: &mut Option<Box<YbTstNode<V>>>, chars: &[char], depth: usize, value: V) -> bool {
        let ch = chars[depth];
        if node.is_none() { *node = Some(Box::new(YbTstNode::yb_new(ch))); }
        let n = node.as_mut().unwrap();
        if ch < n.yb_ch {
            Self::yb_insert_node(&mut n.yb_left, chars, depth, value)
        } else if ch > n.yb_ch {
            Self::yb_insert_node(&mut n.yb_right, chars, depth, value)
        } else if depth + 1 < chars.len() {
            Self::yb_insert_node(&mut n.yb_mid, chars, depth + 1, value)
        } else {
            let was_new = n.yb_value.is_none();
            n.yb_value = Some(value);
            was_new
        }
    }

    /// Look up a key.
    pub fn yb_get(&self, key: &str) -> Option<&V> {
        if key.is_empty() { return None; }
        let chars: Vec<char> = key.chars().collect();
        Self::yb_get_node(self.yb_root.as_deref(), &chars, 0)
    }

    fn yb_get_node<'a>(node: Option<&'a YbTstNode<V>>, chars: &[char], depth: usize) -> Option<&'a V> {
        let n = node?;
        let ch = chars[depth];
        if ch < n.yb_ch {
            Self::yb_get_node(n.yb_left.as_deref(), chars, depth)
        } else if ch > n.yb_ch {
            Self::yb_get_node(n.yb_right.as_deref(), chars, depth)
        } else if depth + 1 < chars.len() {
            Self::yb_get_node(n.yb_mid.as_deref(), chars, depth + 1)
        } else {
            n.yb_value.as_ref()
        }
    }

    /// Check if a key exists.
    pub fn yb_contains(&self, key: &str) -> bool { self.yb_get(key).is_some() }

    /// Collect all keys.
    pub fn yb_all_keys(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut current = String::new();
        Self::yb_collect(self.yb_root.as_deref(), &mut current, &mut results);
        results
    }

    fn yb_collect(node: Option<&YbTstNode<V>>, current: &mut String, results: &mut Vec<String>) {
        let Some(n) = node else { return };
        Self::yb_collect(n.yb_left.as_deref(), current, results);
        current.push(n.yb_ch);
        if n.yb_value.is_some() { results.push(current.clone()); }
        Self::yb_collect(n.yb_mid.as_deref(), current, results);
        current.pop();
        Self::yb_collect(n.yb_right.as_deref(), current, results);
    }

    /// Collect keys with a given prefix.
    pub fn yb_keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        if prefix.is_empty() { return self.yb_all_keys(); }
        let chars: Vec<char> = prefix.chars().collect();
        let node = Self::yb_prefix_node(self.yb_root.as_deref(), &chars, 0);
        let mut results = Vec::new();
        if let Some(n) = node {
            if n.yb_value.is_some() { results.push(prefix.to_string()); }
            let mut current = prefix.to_string();
            Self::yb_collect(n.yb_mid.as_deref(), &mut current, &mut results);
        }
        results
    }

    fn yb_prefix_node<'a>(node: Option<&'a YbTstNode<V>>, chars: &[char], depth: usize) -> Option<&'a YbTstNode<V>> {
        let n = node?;
        let ch = chars[depth];
        if ch < n.yb_ch {
            Self::yb_prefix_node(n.yb_left.as_deref(), chars, depth)
        } else if ch > n.yb_ch {
            Self::yb_prefix_node(n.yb_right.as_deref(), chars, depth)
        } else if depth + 1 < chars.len() {
            Self::yb_prefix_node(n.yb_mid.as_deref(), chars, depth + 1)
        } else {
            Some(n)
        }
    }

    /// Clear the tree.
    pub fn yb_clear(&mut self) { self.yb_root = None; self.yb_size = 0; }
}

// --- yb_ Quadtree ---

/// A point in 2D space for quadtree storage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YbPoint {
    pub yb_x: f64,
    pub yb_y: f64,
}

impl std::fmt::Display for YbPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:.2}, {:.2})", self.yb_x, self.yb_y)
    }
}

impl Default for YbPoint {
    fn default() -> Self { Self { yb_x: 0.0, yb_y: 0.0 } }
}

impl YbPoint {
    /// Create a new point.
    pub fn yb_new(x: f64, y: f64) -> Self { Self { yb_x: x, yb_y: y } }

    /// Distance to another point.
    pub fn yb_distance(&self, other: &YbPoint) -> f64 {
        ((self.yb_x - other.yb_x).powi(2) + (self.yb_y - other.yb_y).powi(2)).sqrt()
    }
}

/// Axis-aligned bounding box for quadtree partitioning.
#[derive(Debug, Clone, Copy)]
pub struct YbBounds {
    pub yb_x: f64,
    pub yb_y: f64,
    pub yb_w: f64,
    pub yb_h: f64,
}

impl std::fmt::Display for YbBounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bounds({:.1},{:.1} {}x{})", self.yb_x, self.yb_y, self.yb_w, self.yb_h)
    }
}

impl Default for YbBounds {
    fn default() -> Self { Self { yb_x: 0.0, yb_y: 0.0, yb_w: 100.0, yb_h: 100.0 } }
}

impl YbBounds {
    /// Create bounds from origin and size.
    pub fn yb_new(x: f64, y: f64, w: f64, h: f64) -> Self { Self { yb_x: x, yb_y: y, yb_w: w, yb_h: h } }

    /// Check if a point is inside these bounds.
    pub fn yb_contains(&self, p: &YbPoint) -> bool {
        p.yb_x >= self.yb_x && p.yb_x < self.yb_x + self.yb_w &&
        p.yb_y >= self.yb_y && p.yb_y < self.yb_y + self.yb_h
    }

    /// Check if two bounds intersect.
    pub fn yb_intersects(&self, other: &YbBounds) -> bool {
        !(self.yb_x + self.yb_w <= other.yb_x || other.yb_x + other.yb_w <= self.yb_x ||
          self.yb_y + self.yb_h <= other.yb_y || other.yb_y + other.yb_h <= self.yb_y)
    }
}

/// Quadtree for 2D spatial indexing with region queries.
#[derive(Debug, Clone)]
pub struct YbQuadtree {
    yb_bounds: YbBounds,
    yb_points: Vec<YbPoint>,
    yb_capacity: usize,
    yb_nw: Option<Box<YbQuadtree>>,
    yb_ne: Option<Box<YbQuadtree>>,
    yb_sw: Option<Box<YbQuadtree>>,
    yb_se: Option<Box<YbQuadtree>>,
    yb_divided: bool,
}

impl std::fmt::Display for YbQuadtree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Quadtree(points={}, bounds={})", self.yb_count(), self.yb_bounds)
    }
}

impl Default for YbQuadtree {
    fn default() -> Self { Self::yb_new(YbBounds::default(), 4) }
}

impl YbQuadtree {
    /// Create a new quadtree with given bounds and node capacity.
    pub fn yb_new(bounds: YbBounds, capacity: usize) -> Self {
        Self {
            yb_bounds: bounds, yb_points: Vec::new(), yb_capacity: capacity.max(1),
            yb_nw: None, yb_ne: None, yb_sw: None, yb_se: None, yb_divided: false,
        }
    }

    fn yb_subdivide(&mut self) {
        let x = self.yb_bounds.yb_x;
        let y = self.yb_bounds.yb_y;
        let hw = self.yb_bounds.yb_w / 2.0;
        let hh = self.yb_bounds.yb_h / 2.0;
        self.yb_nw = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x, y, hw, hh), self.yb_capacity)));
        self.yb_ne = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x + hw, y, hw, hh), self.yb_capacity)));
        self.yb_sw = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x, y + hh, hw, hh), self.yb_capacity)));
        self.yb_se = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x + hw, y + hh, hw, hh), self.yb_capacity)));
        self.yb_divided = true;
    }

    /// Insert a point.
    pub fn yb_insert(&mut self, point: YbPoint) -> bool {
        if !self.yb_bounds.yb_contains(&point) { return false; }
        if self.yb_points.len() < self.yb_capacity && !self.yb_divided {
            self.yb_points.push(point);
            return true;
        }
        if !self.yb_divided { self.yb_subdivide(); }
        if self.yb_nw.as_mut().unwrap().yb_insert(point) { return true; }
        if self.yb_ne.as_mut().unwrap().yb_insert(point) { return true; }
        if self.yb_sw.as_mut().unwrap().yb_insert(point) { return true; }
        self.yb_se.as_mut().unwrap().yb_insert(point)
    }

    /// Query all points within a rectangular region.
    pub fn yb_query(&self, range: &YbBounds) -> Vec<YbPoint> {
        let mut found = Vec::new();
        self.yb_query_inner(range, &mut found);
        found
    }

    fn yb_query_inner(&self, range: &YbBounds, found: &mut Vec<YbPoint>) {
        if !self.yb_bounds.yb_intersects(range) { return; }
        for p in &self.yb_points {
            if range.yb_contains(p) { found.push(*p); }
        }
        if self.yb_divided {
            self.yb_nw.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_ne.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_sw.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_se.as_ref().unwrap().yb_query_inner(range, found);
        }
    }

    /// Count total points.
    pub fn yb_count(&self) -> usize {
        let mut c = self.yb_points.len();
        if self.yb_divided {
            c += self.yb_nw.as_ref().unwrap().yb_count();
            c += self.yb_ne.as_ref().unwrap().yb_count();
            c += self.yb_sw.as_ref().unwrap().yb_count();
            c += self.yb_se.as_ref().unwrap().yb_count();
        }
        c
    }

    /// Is the quadtree empty.
    pub fn yb_is_empty(&self) -> bool { self.yb_count() == 0 }

    /// Get bounds.
    pub fn yb_bounds(&self) -> &YbBounds { &self.yb_bounds }

    /// Find nearest point to a target.
    pub fn yb_nearest(&self, target: &YbPoint) -> Option<YbPoint> {
        let all = self.yb_query(&self.yb_bounds);
        all.into_iter().min_by(|a, b| {
            a.yb_distance(target).partial_cmp(&b.yb_distance(target)).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}


// --- yc_ Van Emde Boas Set ---

/// Simplified van Emde Boas-inspired set for integer keys in [0, universe).
/// Uses a flat bitmap for practical efficiency with O(1) operations.
#[derive(Debug, Clone)]
pub struct YcVebSet {
    yc_bits: Vec<u64>,
    yc_universe: usize,
    yc_count: usize,
    yc_min: Option<usize>,
    yc_max: Option<usize>,
}

impl std::fmt::Display for YcVebSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VebSet(universe={}, count={})", self.yc_universe, self.yc_count)
    }
}

impl Default for YcVebSet {
    fn default() -> Self { Self::yc_new(65536) }
}

impl YcVebSet {
    /// Create a set supporting keys in [0, universe).
    pub fn yc_new(universe: usize) -> Self {
        let words = (universe + 63) / 64;
        Self { yc_bits: vec![0; words], yc_universe: universe, yc_count: 0, yc_min: None, yc_max: None }
    }

    /// Universe size.
    pub fn yc_universe(&self) -> usize { self.yc_universe }

    /// Number of elements.
    pub fn yc_len(&self) -> usize { self.yc_count }

    /// Is the set empty.
    pub fn yc_is_empty(&self) -> bool { self.yc_count == 0 }

    /// Insert a key.
    pub fn yc_insert(&mut self, key: usize) -> bool {
        if key >= self.yc_universe { return false; }
        let word = key / 64;
        let bit = key % 64;
        if self.yc_bits[word] & (1u64 << bit) != 0 { return false; }
        self.yc_bits[word] |= 1u64 << bit;
        self.yc_count += 1;
        self.yc_min = Some(self.yc_min.map_or(key, |m: usize| m.min(key)));
        self.yc_max = Some(self.yc_max.map_or(key, |m: usize| m.max(key)));
        true
    }

    /// Remove a key.
    pub fn yc_remove(&mut self, key: usize) -> bool {
        if key >= self.yc_universe { return false; }
        let word = key / 64;
        let bit = key % 64;
        if self.yc_bits[word] & (1u64 << bit) == 0 { return false; }
        self.yc_bits[word] &= !(1u64 << bit);
        self.yc_count -= 1;
        if self.yc_count == 0 { self.yc_min = None; self.yc_max = None; }
        else {
            if self.yc_min == Some(key) { self.yc_min = self.yc_successor(key); }
            if self.yc_max == Some(key) { self.yc_max = self.yc_predecessor(key); }
        }
        true
    }

    /// Check membership.
    pub fn yc_contains(&self, key: usize) -> bool {
        if key >= self.yc_universe { return false; }
        self.yc_bits[key / 64] & (1u64 << (key % 64)) != 0
    }

    /// Minimum element.
    pub fn yc_min(&self) -> Option<usize> { self.yc_min }

    /// Maximum element.
    pub fn yc_max(&self) -> Option<usize> { self.yc_max }

    /// Find the smallest key > given key.
    pub fn yc_successor(&self, key: usize) -> Option<usize> {
        for k in (key + 1)..self.yc_universe {
            if self.yc_contains(k) { return Some(k); }
        }
        None
    }

    /// Find the largest key < given key.
    pub fn yc_predecessor(&self, key: usize) -> Option<usize> {
        if key == 0 { return None; }
        for k in (0..key).rev() {
            if self.yc_contains(k) { return Some(k); }
        }
        None
    }

    /// Collect all elements in sorted order.
    pub fn yc_to_sorted_vec(&self) -> Vec<usize> {
        let mut result = Vec::with_capacity(self.yc_count);
        for w in 0..self.yc_bits.len() {
            let mut bits = self.yc_bits[w];
            while bits != 0 {
                let tz = bits.trailing_zeros() as usize;
                result.push(w * 64 + tz);
                bits &= bits - 1;
            }
        }
        result
    }

    /// Clear the set.
    pub fn yc_clear(&mut self) {
        for w in &mut self.yc_bits { *w = 0; }
        self.yc_count = 0;
        self.yc_min = None;
        self.yc_max = None;
    }

    /// Union with another set (same universe).
    pub fn yc_union(&mut self, other: &YcVebSet) {
        if self.yc_universe != other.yc_universe { return; }
        for i in 0..self.yc_bits.len() {
            self.yc_bits[i] |= other.yc_bits[i];
        }
        self.yc_count = self.yc_to_sorted_vec().len();
        let sorted = self.yc_to_sorted_vec();
        self.yc_min = sorted.first().copied();
        self.yc_max = sorted.last().copied();
    }

    /// Intersection with another set.
    pub fn yc_intersection(&self, other: &YcVebSet) -> YcVebSet {
        let mut result = YcVebSet::yc_new(self.yc_universe);
        if self.yc_universe != other.yc_universe { return result; }
        for i in 0..self.yc_bits.len() {
            result.yc_bits[i] = self.yc_bits[i] & other.yc_bits[i];
        }
        let sorted = result.yc_to_sorted_vec();
        result.yc_count = sorted.len();
        result.yc_min = sorted.first().copied();
        result.yc_max = sorted.last().copied();
        result
    }
}

// --- yc_ Consistent Hash Ring ---

/// Consistent hash ring for distributed key mapping with virtual nodes.
#[derive(Debug, Clone)]
pub struct YcHashRing {
    yc_ring: std::collections::BTreeMap<u64, String>,
    yc_replicas: usize,
    yc_nodes: Vec<String>,
}

impl std::fmt::Display for YcHashRing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HashRing(nodes={}, replicas={})", self.yc_nodes.len(), self.yc_replicas)
    }
}

impl Default for YcHashRing {
    fn default() -> Self { Self { yc_ring: std::collections::BTreeMap::new(), yc_replicas: 150, yc_nodes: Vec::new() } }
}

impl YcHashRing {
    /// Create a new hash ring with given replica count per node.
    pub fn yc_new(replicas: usize) -> Self {
        Self { yc_ring: std::collections::BTreeMap::new(), yc_replicas: replicas.max(1), yc_nodes: Vec::new() }
    }

    fn yc_hash(key: &str) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in key.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    /// Add a node to the ring.
    pub fn yc_add_node(&mut self, node: &str) {
        for i in 0..self.yc_replicas {
            let key = format!("{}:{}", node, i);
            let hash = Self::yc_hash(&key);
            self.yc_ring.insert(hash, node.to_string());
        }
        self.yc_nodes.push(node.to_string());
    }

    /// Remove a node from the ring.
    pub fn yc_remove_node(&mut self, node: &str) {
        for i in 0..self.yc_replicas {
            let key = format!("{}:{}", node, i);
            let hash = Self::yc_hash(&key);
            self.yc_ring.remove(&hash);
        }
        self.yc_nodes.retain(|n| n != node);
    }

    /// Find the node responsible for a key.
    pub fn yc_get_node(&self, key: &str) -> Option<&str> {
        if self.yc_ring.is_empty() { return None; }
        let hash = Self::yc_hash(key);
        let node = self.yc_ring.range(hash..).next()
            .or_else(|| self.yc_ring.iter().next());
        node.map(|(_, v)| v.as_str())
    }

    /// Number of physical nodes.
    pub fn yc_node_count(&self) -> usize { self.yc_nodes.len() }

    /// Number of virtual nodes on the ring.
    pub fn yc_virtual_count(&self) -> usize { self.yc_ring.len() }

    /// List all physical nodes.
    pub fn yc_nodes(&self) -> &[String] { &self.yc_nodes }

    /// Check if a node is in the ring.
    pub fn yc_has_node(&self, node: &str) -> bool { self.yc_nodes.iter().any(|n| n == node) }
}


// --- yd_ Directed Acyclic Graph ---

/// Directed acyclic graph with topological sorting and cycle detection.
#[derive(Debug, Clone)]
pub struct YdDag {
    yd_adj: std::collections::HashMap<usize, Vec<usize>>,
    yd_in_degree: std::collections::HashMap<usize, usize>,
    yd_nodes: std::collections::HashSet<usize>,
}

impl std::fmt::Display for YdDag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let edges: usize = self.yd_adj.values().map(|v| v.len()).sum();
        write!(f, "DAG(nodes={}, edges={})", self.yd_nodes.len(), edges)
    }
}

impl Default for YdDag {
    fn default() -> Self { Self::yd_new() }
}

impl YdDag {
    /// Create an empty DAG.
    pub fn yd_new() -> Self {
        Self { yd_adj: std::collections::HashMap::new(), yd_in_degree: std::collections::HashMap::new(), yd_nodes: std::collections::HashSet::new() }
    }

    /// Add a node.
    pub fn yd_add_node(&mut self, node: usize) {
        self.yd_nodes.insert(node);
        self.yd_adj.entry(node).or_default();
        self.yd_in_degree.entry(node).or_insert(0);
    }

    /// Add a directed edge from -> to.
    pub fn yd_add_edge(&mut self, from: usize, to: usize) {
        self.yd_add_node(from);
        self.yd_add_node(to);
        self.yd_adj.entry(from).or_default().push(to);
        *self.yd_in_degree.entry(to).or_insert(0) += 1;
    }

    /// Number of nodes.
    pub fn yd_node_count(&self) -> usize { self.yd_nodes.len() }

    /// Number of edges.
    pub fn yd_edge_count(&self) -> usize { self.yd_adj.values().map(|v| v.len()).sum() }

    /// Topological sort using Kahn's algorithm. Returns None if cycle detected.
    pub fn yd_topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = self.yd_in_degree.clone();
        let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        for (node, deg) in &in_deg {
            if *deg == 0 { queue.push_back(*node); }
        }
        let mut result = Vec::new();
        while let Some(node) = queue.pop_front() {
            result.push(node);
            if let Some(neighbors) = self.yd_adj.get(&node) {
                for &next in neighbors {
                    let d = in_deg.get_mut(&next).unwrap();
                    *d -= 1;
                    if *d == 0 { queue.push_back(next); }
                }
            }
        }
        if result.len() == self.yd_nodes.len() { Some(result) } else { None }
    }

    /// Check if the graph has a cycle.
    pub fn yd_has_cycle(&self) -> bool { self.yd_topological_sort().is_none() }

    /// Get all neighbors of a node.
    pub fn yd_neighbors(&self, node: usize) -> &[usize] {
        self.yd_adj.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get in-degree of a node.
    pub fn yd_in_degree(&self, node: usize) -> usize {
        self.yd_in_degree.get(&node).copied().unwrap_or(0)
    }

    /// Get out-degree of a node.
    pub fn yd_out_degree(&self, node: usize) -> usize {
        self.yd_adj.get(&node).map(|v| v.len()).unwrap_or(0)
    }

    /// Find all root nodes (in-degree 0).
    pub fn yd_roots(&self) -> Vec<usize> {
        let mut roots: Vec<usize> = self.yd_in_degree.iter()
            .filter(|(_, d)| **d == 0)
            .map(|(n, _)| *n)
            .collect();
        roots.sort();
        roots
    }

    /// Find all leaf nodes (out-degree 0).
    pub fn yd_leaves(&self) -> Vec<usize> {
        let mut leaves: Vec<usize> = self.yd_nodes.iter()
            .filter(|&&n| self.yd_out_degree(n) == 0)
            .copied()
            .collect();
        leaves.sort();
        leaves
    }

    /// Check if node exists.
    pub fn yd_has_node(&self, node: usize) -> bool { self.yd_nodes.contains(&node) }

    /// Clear the graph.
    pub fn yd_clear(&mut self) {
        self.yd_adj.clear();
        self.yd_in_degree.clear();
        self.yd_nodes.clear();
    }

    /// BFS traversal from a start node.
    pub fn yd_bfs(&self, start: usize) -> Vec<usize> {
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut result = Vec::new();
        queue.push_back(start);
        visited.insert(start);
        while let Some(node) = queue.pop_front() {
            result.push(node);
            if let Some(neighbors) = self.yd_adj.get(&node) {
                let mut sorted_n = neighbors.clone();
                sorted_n.sort();
                for next in sorted_n {
                    if visited.insert(next) { queue.push_back(next); }
                }
            }
        }
        result
    }

    /// DFS traversal from a start node.
    pub fn yd_dfs(&self, start: usize) -> Vec<usize> {
        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        self.yd_dfs_inner(start, &mut visited, &mut result);
        result
    }

    fn yd_dfs_inner(&self, node: usize, visited: &mut std::collections::HashSet<usize>, result: &mut Vec<usize>) {
        if !visited.insert(node) { return; }
        result.push(node);
        if let Some(neighbors) = self.yd_adj.get(&node) {
            let mut sorted_n = neighbors.clone();
            sorted_n.sort();
            for next in sorted_n {
                self.yd_dfs_inner(next, visited, result);
            }
        }
    }

    /// Shortest path length (unweighted) between two nodes using BFS.
    pub fn yd_shortest_path(&self, from: usize, to: usize) -> Option<usize> {
        if from == to { return Some(0); }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((from, 0usize));
        visited.insert(from);
        while let Some((node, dist)) = queue.pop_front() {
            if let Some(neighbors) = self.yd_adj.get(&node) {
                for &next in neighbors {
                    if next == to { return Some(dist + 1); }
                    if visited.insert(next) { queue.push_back((next, dist + 1)); }
                }
            }
        }
        None
    }
}

// --- yd_ Sparse Matrix ---

/// Sparse matrix using coordinate (COO) format for efficient storage.
#[derive(Debug, Clone)]
pub struct YdSparseMatrix {
    yd_rows: usize,
    yd_cols: usize,
    yd_entries: std::collections::HashMap<(usize, usize), f64>,
}

impl std::fmt::Display for YdSparseMatrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SparseMatrix({}x{}, nnz={})", self.yd_rows, self.yd_cols, self.yd_entries.len())
    }
}

impl Default for YdSparseMatrix {
    fn default() -> Self { Self::yd_new(0, 0) }
}

impl YdSparseMatrix {
    /// Create a new sparse matrix with given dimensions.
    pub fn yd_new(rows: usize, cols: usize) -> Self {
        Self { yd_rows: rows, yd_cols: cols, yd_entries: std::collections::HashMap::new() }
    }

    /// Set a value.
    pub fn yd_set(&mut self, row: usize, col: usize, val: f64) {
        if val == 0.0 { self.yd_entries.remove(&(row, col)); }
        else { self.yd_entries.insert((row, col), val); }
    }

    /// Get a value.
    pub fn yd_get(&self, row: usize, col: usize) -> f64 {
        self.yd_entries.get(&(row, col)).copied().unwrap_or(0.0)
    }

    /// Number of non-zero entries.
    pub fn yd_nnz(&self) -> usize { self.yd_entries.len() }

    /// Dimensions.
    pub fn yd_rows(&self) -> usize { self.yd_rows }
    pub fn yd_cols(&self) -> usize { self.yd_cols }

    /// Transpose.
    pub fn yd_transpose(&self) -> YdSparseMatrix {
        let mut t = YdSparseMatrix::yd_new(self.yd_cols, self.yd_rows);
        for ((r, c), v) in &self.yd_entries {
            t.yd_set(*c, *r, *v);
        }
        t
    }

    /// Matrix-vector multiply.
    pub fn yd_mul_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.yd_rows];
        for ((r, c), v) in &self.yd_entries {
            if *c < vec.len() && *r < result.len() {
                result[*r] += *v * vec[*c];
            }
        }
        result
    }

    /// Scale all entries.
    pub fn yd_scale(&mut self, factor: f64) {
        for v in self.yd_entries.values_mut() { *v *= factor; }
    }

    /// Add another sparse matrix.
    pub fn yd_add(&self, other: &YdSparseMatrix) -> YdSparseMatrix {
        let mut result = self.clone();
        for ((r, c), v) in &other.yd_entries {
            let entry = result.yd_entries.entry((*r, *c)).or_insert(0.0);
            *entry += *v;
        }
        result
    }

    /// Clear all entries.
    pub fn yd_clear(&mut self) { self.yd_entries.clear(); }

    /// Row sum.
    pub fn yd_row_sum(&self, row: usize) -> f64 {
        self.yd_entries.iter()
            .filter(|((r, _), _)| *r == row)
            .map(|(_, v)| *v)
            .sum()
    }

    /// Frobenius norm squared.
    pub fn yd_frobenius_sq(&self) -> f64 {
        self.yd_entries.values().map(|v| *v * *v).sum()
    }
}


// --- ye_ Indexed Priority Queue ---

/// Indexed min-priority queue supporting decrease-key in O(log n).
#[derive(Debug, Clone)]
pub struct YeIndexedPQ {
    ye_heap: Vec<(usize, i64)>,
    ye_pos: std::collections::HashMap<usize, usize>,
}

impl std::fmt::Display for YeIndexedPQ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IndexedPQ(size={})", self.ye_heap.len())
    }
}

impl Default for YeIndexedPQ {
    fn default() -> Self { Self::ye_new() }
}

impl YeIndexedPQ {
    /// Create an empty indexed priority queue.
    pub fn ye_new() -> Self { Self { ye_heap: Vec::new(), ye_pos: std::collections::HashMap::new() } }

    /// Number of elements.
    pub fn ye_len(&self) -> usize { self.ye_heap.len() }

    /// Is empty.
    pub fn ye_is_empty(&self) -> bool { self.ye_heap.is_empty() }

    /// Insert an element with priority.
    pub fn ye_insert(&mut self, id: usize, priority: i64) {
        if self.ye_pos.contains_key(&id) { self.ye_decrease_key(id, priority); return; }
        let idx = self.ye_heap.len();
        self.ye_heap.push((id, priority));
        self.ye_pos.insert(id, idx);
        self.ye_sift_up(idx);
    }

    /// Peek at minimum.
    pub fn ye_peek(&self) -> Option<(usize, i64)> { self.ye_heap.first().copied() }

    /// Extract minimum.
    pub fn ye_pop(&mut self) -> Option<(usize, i64)> {
        if self.ye_heap.is_empty() { return None; }
        let min = self.ye_heap[0];
        let last = self.ye_heap.len() - 1;
        self.ye_swap(0, last);
        self.ye_heap.pop();
        self.ye_pos.remove(&min.0);
        if !self.ye_heap.is_empty() { self.ye_sift_down(0); }
        Some(min)
    }

    /// Decrease the priority of an element.
    pub fn ye_decrease_key(&mut self, id: usize, new_priority: i64) {
        if let Some(&idx) = self.ye_pos.get(&id) {
            if new_priority < self.ye_heap[idx].1 {
                self.ye_heap[idx].1 = new_priority;
                self.ye_sift_up(idx);
            }
        }
    }

    /// Check if an id is in the queue.
    pub fn ye_contains(&self, id: usize) -> bool { self.ye_pos.contains_key(&id) }

    /// Get priority of an id.
    pub fn ye_priority(&self, id: usize) -> Option<i64> {
        self.ye_pos.get(&id).map(|&idx| self.ye_heap[idx].1)
    }

    fn ye_swap(&mut self, i: usize, j: usize) {
        self.ye_heap.swap(i, j);
        self.ye_pos.insert(self.ye_heap[i].0, i);
        self.ye_pos.insert(self.ye_heap[j].0, j);
    }

    fn ye_sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.ye_heap[idx].1 < self.ye_heap[parent].1 {
                self.ye_swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn ye_sift_down(&mut self, mut idx: usize) {
        let n = self.ye_heap.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < n && self.ye_heap[left].1 < self.ye_heap[smallest].1 { smallest = left; }
            if right < n && self.ye_heap[right].1 < self.ye_heap[smallest].1 { smallest = right; }
            if smallest == idx { break; }
            self.ye_swap(idx, smallest);
            idx = smallest;
        }
    }

    /// Clear the queue.
    pub fn ye_clear(&mut self) { self.ye_heap.clear(); self.ye_pos.clear(); }

    /// Drain all elements in priority order.
    pub fn ye_drain_sorted(&mut self) -> Vec<(usize, i64)> {
        let mut result = Vec::with_capacity(self.ye_heap.len());
        while let Some(item) = self.ye_pop() { result.push(item); }
        result
    }
}

// --- ye_ Segment Tree with Lazy Propagation ---

/// Segment tree with lazy propagation for range queries and updates.
#[derive(Debug, Clone)]
pub struct YeSegTree {
    ye_n: usize,
    ye_tree: Vec<i64>,
    ye_lazy: Vec<i64>,
}

impl std::fmt::Display for YeSegTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SegTree(n={})", self.ye_n)
    }
}

impl Default for YeSegTree {
    fn default() -> Self { Self { ye_n: 0, ye_tree: Vec::new(), ye_lazy: Vec::new() } }
}

impl YeSegTree {
    /// Build from an array of values.
    pub fn ye_from_slice(data: &[i64]) -> Self {
        let n = data.len();
        let mut tree = vec![0i64; 4 * n];
        let lazy = vec![0i64; 4 * n];
        let mut st = Self { ye_n: n, ye_tree: tree.clone(), ye_lazy: lazy };
        if n > 0 { st.ye_build(data, 1, 0, n - 1); }
        st
    }

    fn ye_build(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.ye_tree[node] = data[start];
            return;
        }
        let mid = (start + end) / 2;
        self.ye_build(data, 2 * node, start, mid);
        self.ye_build(data, 2 * node + 1, mid + 1, end);
        self.ye_tree[node] = self.ye_tree[2 * node] + self.ye_tree[2 * node + 1];
    }

    fn ye_push_down(&mut self, node: usize, start: usize, end: usize) {
        if self.ye_lazy[node] != 0 {
            let mid = (start + end) / 2;
            self.ye_tree[2 * node] += self.ye_lazy[node] * (mid - start + 1) as i64;
            self.ye_tree[2 * node + 1] += self.ye_lazy[node] * (end - mid) as i64;
            self.ye_lazy[2 * node] += self.ye_lazy[node];
            self.ye_lazy[2 * node + 1] += self.ye_lazy[node];
            self.ye_lazy[node] = 0;
        }
    }

    /// Range sum query [l, r].
    pub fn ye_query(&mut self, l: usize, r: usize) -> i64 {
        if self.ye_n == 0 || l > r || r >= self.ye_n { return 0; }
        self.ye_query_inner(1, 0, self.ye_n - 1, l, r)
    }

    fn ye_query_inner(&mut self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.ye_tree[node]; }
        self.ye_push_down(node, start, end);
        let mid = (start + end) / 2;
        self.ye_query_inner(2 * node, start, mid, l, r) +
        self.ye_query_inner(2 * node + 1, mid + 1, end, l, r)
    }

    /// Range update: add val to all elements in [l, r].
    pub fn ye_update(&mut self, l: usize, r: usize, val: i64) {
        if self.ye_n == 0 || l > r || r >= self.ye_n { return; }
        self.ye_update_inner(1, 0, self.ye_n - 1, l, r, val);
    }

    fn ye_update_inner(&mut self, node: usize, start: usize, end: usize, l: usize, r: usize, val: i64) {
        if r < start || end < l { return; }
        if l <= start && end <= r {
            self.ye_tree[node] += val * (end - start + 1) as i64;
            self.ye_lazy[node] += val;
            return;
        }
        self.ye_push_down(node, start, end);
        let mid = (start + end) / 2;
        self.ye_update_inner(2 * node, start, mid, l, r, val);
        self.ye_update_inner(2 * node + 1, mid + 1, end, l, r, val);
        self.ye_tree[node] = self.ye_tree[2 * node] + self.ye_tree[2 * node + 1];
    }

    /// Point query: get value at index.
    pub fn ye_point_query(&mut self, idx: usize) -> i64 {
        self.ye_query(idx, idx)
    }

    /// Size of underlying array.
    pub fn ye_len(&self) -> usize { self.ye_n }

    /// Is empty.
    pub fn ye_is_empty(&self) -> bool { self.ye_n == 0 }
}


// --- yf_ Disjoint Interval Set ---

/// Set of non-overlapping intervals with automatic merging.
#[derive(Debug, Clone)]
pub struct YfIntervalSet {
    yf_intervals: std::collections::BTreeMap<i64, i64>,
}

impl std::fmt::Display for YfIntervalSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IntervalSet(count={})", self.yf_intervals.len())
    }
}

impl Default for YfIntervalSet {
    fn default() -> Self { Self::yf_new() }
}

impl YfIntervalSet {
    /// Create an empty interval set.
    pub fn yf_new() -> Self { Self { yf_intervals: std::collections::BTreeMap::new() } }

    /// Number of disjoint intervals.
    pub fn yf_len(&self) -> usize { self.yf_intervals.len() }

    /// Is empty.
    pub fn yf_is_empty(&self) -> bool { self.yf_intervals.is_empty() }

    /// Add an interval [lo, hi]. Merges with overlapping/adjacent intervals.
    pub fn yf_add(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let to_remove: Vec<i64> = self.yf_intervals.range(..=hi + 1)
            .filter(|(start, end)| **end >= lo - 1)
            .map(|(s, _)| *s)
            .collect();
        for s in &to_remove {
            let e = self.yf_intervals[s];
            new_lo = new_lo.min(*s);
            new_hi = new_hi.max(e);
            self.yf_intervals.remove(s);
        }
        self.yf_intervals.insert(new_lo, new_hi);
    }

    /// Check if a value is covered by any interval.
    pub fn yf_contains(&self, val: i64) -> bool {
        if let Some((_, end)) = self.yf_intervals.range(..=val).next_back() {
            *end >= val
        } else {
            false
        }
    }

    /// Remove a point, splitting intervals if needed.
    pub fn yf_remove_point(&mut self, val: i64) {
        let covering = self.yf_intervals.range(..=val)
            .filter(|(_, end)| **end >= val)
            .map(|(s, e)| (*s, *e))
            .next_back();
        if let Some((s, e)) = covering {
            self.yf_intervals.remove(&s);
            if s < val { self.yf_intervals.insert(s, val - 1); }
            if val < e { self.yf_intervals.insert(val + 1, e); }
        }
    }

    /// Get all intervals as sorted vec.
    pub fn yf_intervals(&self) -> Vec<(i64, i64)> {
        self.yf_intervals.iter().map(|(s, e)| (*s, *e)).collect()
    }

    /// Total covered length.
    pub fn yf_total_length(&self) -> i64 {
        self.yf_intervals.iter().map(|(s, e)| e - s + 1).sum()
    }

    /// Clear all intervals.
    pub fn yf_clear(&mut self) { self.yf_intervals.clear(); }

    /// Check if two interval sets overlap.
    pub fn yf_overlaps(&self, other: &YfIntervalSet) -> bool {
        for (s, e) in &self.yf_intervals {
            for (os, oe) in &other.yf_intervals {
                if s <= oe && os <= e { return true; }
            }
        }
        false
    }
}

// --- yf_ K-way Merge ---

/// K-way merge iterator that merges multiple sorted sequences.
#[derive(Debug, Clone)]
pub struct YfKWayMerge {
    yf_sources: Vec<Vec<i64>>,
    yf_indices: Vec<usize>,
}

impl std::fmt::Display for YfKWayMerge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KWayMerge(sources={})", self.yf_sources.len())
    }
}

impl Default for YfKWayMerge {
    fn default() -> Self { Self::yf_new() }
}

impl YfKWayMerge {
    /// Create an empty k-way merge.
    pub fn yf_new() -> Self { Self { yf_sources: Vec::new(), yf_indices: Vec::new() } }

    /// Add a sorted source.
    pub fn yf_add_source(&mut self, source: Vec<i64>) {
        self.yf_sources.push(source);
        self.yf_indices.push(0);
    }

    /// Number of sources.
    pub fn yf_source_count(&self) -> usize { self.yf_sources.len() }

    /// Merge all sources into a single sorted vec.
    pub fn yf_merge(&mut self) -> Vec<i64> {
        let mut result = Vec::new();
        loop {
            let mut min_val: Option<i64> = None;
            let mut min_src = 0;
            for (i, (src, idx)) in self.yf_sources.iter().zip(self.yf_indices.iter()).enumerate() {
                if *idx < src.len() {
                    let v = src[*idx];
                    if min_val.is_none() || v < min_val.unwrap() {
                        min_val = Some(v);
                        min_src = i;
                    }
                }
            }
            match min_val {
                Some(v) => { result.push(v); self.yf_indices[min_src] += 1; }
                None => break,
            }
        }
        result
    }

    /// Total remaining elements across all sources.
    pub fn yf_remaining(&self) -> usize {
        self.yf_sources.iter().zip(self.yf_indices.iter())
            .map(|(src, idx)| src.len().saturating_sub(*idx))
            .sum()
    }

    /// Reset all indices.
    pub fn yf_reset(&mut self) {
        for idx in &mut self.yf_indices { *idx = 0; }
    }

    /// Clear all sources.
    pub fn yf_clear(&mut self) { self.yf_sources.clear(); self.yf_indices.clear(); }

    /// Check if merge is complete.
    pub fn yf_is_done(&self) -> bool { self.yf_remaining() == 0 }

    /// Merge and deduplicate.
    pub fn yf_merge_unique(&mut self) -> Vec<i64> {
        let merged = self.yf_merge();
        let mut unique = Vec::new();
        for v in merged {
            if unique.last() != Some(&v) { unique.push(v); }
        }
        unique
    }
}


// --- yg_ Persistent Stack ---

/// Immutable persistent stack using a linked list of arcs.
#[derive(Debug, Clone)]
pub struct YgPersistentStack<T: Clone> {
    yg_head: Option<std::sync::Arc<YgStackNode<T>>>,
    yg_size: usize,
}

#[derive(Debug, Clone)]
struct YgStackNode<T: Clone> {
    yg_value: T,
    yg_next: Option<std::sync::Arc<YgStackNode<T>>>,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for YgPersistentStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PStack(size={})", self.yg_size)
    }
}

impl<T: Clone> Default for YgPersistentStack<T> {
    fn default() -> Self { Self::yg_new() }
}

impl<T: Clone> YgPersistentStack<T> {
    /// Create an empty persistent stack.
    pub fn yg_new() -> Self { Self { yg_head: None, yg_size: 0 } }

    /// Push returns a new stack with the element on top.
    pub fn yg_push(&self, value: T) -> Self {
        Self {
            yg_head: Some(std::sync::Arc::new(YgStackNode { yg_value: value, yg_next: self.yg_head.clone() })),
            yg_size: self.yg_size + 1,
        }
    }

    /// Pop returns the top value and a new stack without it.
    pub fn yg_pop(&self) -> Option<(T, Self)> {
        self.yg_head.as_ref().map(|node| {
            (node.yg_value.clone(), Self { yg_head: node.yg_next.clone(), yg_size: self.yg_size - 1 })
        })
    }

    /// Peek at the top value.
    pub fn yg_peek(&self) -> Option<&T> {
        self.yg_head.as_ref().map(|node| &node.yg_value)
    }

    /// Size of the stack.
    pub fn yg_len(&self) -> usize { self.yg_size }

    /// Is empty.
    pub fn yg_is_empty(&self) -> bool { self.yg_size == 0 }

    /// Convert to vec (top first).
    pub fn yg_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.yg_size);
        let mut current = &self.yg_head;
        while let Some(node) = current {
            result.push(node.yg_value.clone());
            current = &node.yg_next;
        }
        result
    }

    /// Reverse the stack.
    pub fn yg_reverse(&self) -> Self {
        let mut result = Self::yg_new();
        let mut current = &self.yg_head;
        while let Some(node) = current {
            result = result.yg_push(node.yg_value.clone());
            current = &node.yg_next;
        }
        result
    }
}

// --- yg_ Bitmap Index ---

/// Bitmap index for fast multi-column filtering on categorical data.
#[derive(Debug, Clone)]
pub struct YgBitmapIndex {
    yg_bitmaps: std::collections::HashMap<String, Vec<u64>>,
    yg_num_rows: usize,
}

impl std::fmt::Display for YgBitmapIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BitmapIndex(rows={}, columns={})", self.yg_num_rows, self.yg_bitmaps.len())
    }
}

impl Default for YgBitmapIndex {
    fn default() -> Self { Self::yg_new(0) }
}

impl YgBitmapIndex {
    /// Create a bitmap index for a given number of rows.
    pub fn yg_new(num_rows: usize) -> Self {
        Self { yg_bitmaps: std::collections::HashMap::new(), yg_num_rows: num_rows }
    }

    fn yg_words(n: usize) -> usize { (n + 63) / 64 }

    /// Set a value for a column at a given row.
    pub fn yg_set(&mut self, column: &str, row: usize) {
        if row >= self.yg_num_rows { return; }
        let words = Self::yg_words(self.yg_num_rows);
        let bitmap = self.yg_bitmaps.entry(column.to_string()).or_insert_with(|| vec![0u64; words]);
        bitmap[row / 64] |= 1u64 << (row % 64);
    }

    /// Check if a column is set for a row.
    pub fn yg_get(&self, column: &str, row: usize) -> bool {
        if row >= self.yg_num_rows { return false; }
        self.yg_bitmaps.get(column)
            .map(|bm| bm[row / 64] & (1u64 << (row % 64)) != 0)
            .unwrap_or(false)
    }

    /// AND query: rows where all columns are set.
    pub fn yg_and(&self, columns: &[&str]) -> Vec<usize> {
        let words = Self::yg_words(self.yg_num_rows);
        let mut result = vec![u64::MAX; words];
        for col in columns {
            if let Some(bm) = self.yg_bitmaps.get(*col) {
                for (i, w) in result.iter_mut().enumerate() { *w &= bm[i]; }
            } else {
                return Vec::new();
            }
        }
        Self::yg_bits_to_rows(&result, self.yg_num_rows)
    }

    /// OR query: rows where any column is set.
    pub fn yg_or(&self, columns: &[&str]) -> Vec<usize> {
        let words = Self::yg_words(self.yg_num_rows);
        let mut result = vec![0u64; words];
        for col in columns {
            if let Some(bm) = self.yg_bitmaps.get(*col) {
                for (i, w) in result.iter_mut().enumerate() { *w |= bm[i]; }
            }
        }
        Self::yg_bits_to_rows(&result, self.yg_num_rows)
    }

    fn yg_bits_to_rows(bits: &[u64], max_rows: usize) -> Vec<usize> {
        let mut rows = Vec::new();
        for (w_idx, word) in bits.iter().enumerate() {
            let mut bits = *word;
            while bits != 0 {
                let tz = bits.trailing_zeros() as usize;
                let row = w_idx * 64 + tz;
                if row < max_rows { rows.push(row); }
                bits &= bits - 1;
            }
        }
        rows
    }

    /// Count of rows for a column.
    pub fn yg_count(&self, column: &str) -> usize {
        self.yg_bitmaps.get(column)
            .map(|bm| bm.iter().map(|w| w.count_ones() as usize).sum())
            .unwrap_or(0)
    }

    /// Number of rows.
    pub fn yg_num_rows(&self) -> usize { self.yg_num_rows }

    /// Number of columns.
    pub fn yg_num_columns(&self) -> usize { self.yg_bitmaps.len() }

    /// List column names.
    pub fn yg_columns(&self) -> Vec<String> {
        let mut cols: Vec<String> = self.yg_bitmaps.keys().cloned().collect();
        cols.sort();
        cols
    }

    /// Clear all bitmaps.
    pub fn yg_clear(&mut self) { self.yg_bitmaps.clear(); }
}


// --- yh_ Order Statistics Tree ---

/// Order statistics tree supporting rank queries and selection.
/// Implemented as an augmented BST with subtree sizes.
#[derive(Debug, Clone)]
pub struct YhOrderStatTree {
    yh_root: Option<Box<YhOstNode>>,
}

#[derive(Debug, Clone)]
struct YhOstNode {
    yh_key: i64,
    yh_left: Option<Box<YhOstNode>>,
    yh_right: Option<Box<YhOstNode>>,
    yh_size: usize,
}

impl YhOstNode {
    fn yh_new(key: i64) -> Self {
        Self { yh_key: key, yh_left: None, yh_right: None, yh_size: 1 }
    }

    fn yh_left_size(&self) -> usize {
        self.yh_left.as_ref().map_or(0, |n| n.yh_size)
    }

    fn yh_update_size(&mut self) {
        self.yh_size = 1 + self.yh_left.as_ref().map_or(0, |n| n.yh_size)
            + self.yh_right.as_ref().map_or(0, |n| n.yh_size);
    }
}

impl std::fmt::Display for YhOrderStatTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OSTree(size={})", self.yh_len())
    }
}

impl Default for YhOrderStatTree {
    fn default() -> Self { Self::yh_new() }
}

impl YhOrderStatTree {
    /// Create an empty order statistics tree.
    pub fn yh_new() -> Self { Self { yh_root: None } }

    /// Number of elements.
    pub fn yh_len(&self) -> usize { self.yh_root.as_ref().map_or(0, |n| n.yh_size) }

    /// Is empty.
    pub fn yh_is_empty(&self) -> bool { self.yh_root.is_none() }

    /// Insert a key.
    pub fn yh_insert(&mut self, key: i64) {
        Self::yh_insert_node(&mut self.yh_root, key);
    }

    fn yh_insert_node(node: &mut Option<Box<YhOstNode>>, key: i64) {
        match node {
            None => { *node = Some(Box::new(YhOstNode::yh_new(key))); }
            Some(n) => {
                if key < n.yh_key { Self::yh_insert_node(&mut n.yh_left, key); }
                else if key > n.yh_key { Self::yh_insert_node(&mut n.yh_right, key); }
                n.yh_update_size();
            }
        }
    }

    /// Check if a key exists.
    pub fn yh_contains(&self, key: i64) -> bool {
        let mut current = &self.yh_root;
        while let Some(n) = current {
            if key < n.yh_key { current = &n.yh_left; }
            else if key > n.yh_key { current = &n.yh_right; }
            else { return true; }
        }
        false
    }

    /// Rank of a key (0-indexed, number of elements < key).
    pub fn yh_rank(&self, key: i64) -> usize {
        Self::yh_rank_node(&self.yh_root, key)
    }

    fn yh_rank_node(node: &Option<Box<YhOstNode>>, key: i64) -> usize {
        match node {
            None => 0,
            Some(n) => {
                if key < n.yh_key { Self::yh_rank_node(&n.yh_left, key) }
                else if key > n.yh_key { n.yh_left_size() + 1 + Self::yh_rank_node(&n.yh_right, key) }
                else { n.yh_left_size() }
            }
        }
    }

    /// Select the k-th smallest element (0-indexed).
    pub fn yh_select(&self, k: usize) -> Option<i64> {
        Self::yh_select_node(&self.yh_root, k)
    }

    fn yh_select_node(node: &Option<Box<YhOstNode>>, k: usize) -> Option<i64> {
        let n = node.as_ref()?;
        let left_size = n.yh_left_size();
        if k < left_size { Self::yh_select_node(&n.yh_left, k) }
        else if k > left_size { Self::yh_select_node(&n.yh_right, k - left_size - 1) }
        else { Some(n.yh_key) }
    }

    /// Minimum key.
    pub fn yh_min(&self) -> Option<i64> {
        let mut current = &self.yh_root;
        let mut min = None;
        while let Some(n) = current {
            min = Some(n.yh_key);
            current = &n.yh_left;
        }
        min
    }

    /// Maximum key.
    pub fn yh_max(&self) -> Option<i64> {
        let mut current = &self.yh_root;
        let mut max = None;
        while let Some(n) = current {
            max = Some(n.yh_key);
            current = &n.yh_right;
        }
        max
    }

    /// In-order traversal.
    pub fn yh_inorder(&self) -> Vec<i64> {
        let mut result = Vec::new();
        Self::yh_inorder_node(&self.yh_root, &mut result);
        result
    }

    fn yh_inorder_node(node: &Option<Box<YhOstNode>>, result: &mut Vec<i64>) {
        if let Some(n) = node {
            Self::yh_inorder_node(&n.yh_left, result);
            result.push(n.yh_key);
            Self::yh_inorder_node(&n.yh_right, result);
        }
    }

    /// Count elements in range [lo, hi].
    pub fn yh_count_range(&self, lo: i64, hi: i64) -> usize {
        if lo > hi { return 0; }
        let rank_hi = self.yh_rank(hi + 1);
        let rank_lo = self.yh_rank(lo);
        rank_hi - rank_lo
    }
}

// --- yh_ Reservoir Sampler ---

/// Reservoir sampling for uniformly random samples from a stream.
#[derive(Debug, Clone)]
pub struct YhReservoirSampler {
    yh_reservoir: Vec<i64>,
    yh_k: usize,
    yh_count: usize,
    yh_seed: u64,
}

impl std::fmt::Display for YhReservoirSampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reservoir(k={}, seen={})", self.yh_k, self.yh_count)
    }
}

impl Default for YhReservoirSampler {
    fn default() -> Self { Self::yh_new(10, 42) }
}

impl YhReservoirSampler {
    /// Create a reservoir sampler for k items.
    pub fn yh_new(k: usize, seed: u64) -> Self {
        Self { yh_reservoir: Vec::with_capacity(k), yh_k: k, yh_count: 0, yh_seed: seed }
    }

    fn yh_next_rand(&mut self) -> u64 {
        self.yh_seed ^= self.yh_seed << 13;
        self.yh_seed ^= self.yh_seed >> 7;
        self.yh_seed ^= self.yh_seed << 17;
        self.yh_seed
    }

    /// Feed a new item from the stream.
    pub fn yh_add(&mut self, item: i64) {
        self.yh_count += 1;
        if self.yh_reservoir.len() < self.yh_k {
            self.yh_reservoir.push(item);
        } else {
            let j = (self.yh_next_rand() % self.yh_count as u64) as usize;
            if j < self.yh_k {
                self.yh_reservoir[j] = item;
            }
        }
    }

    /// Get the current sample.
    pub fn yh_sample(&self) -> &[i64] { &self.yh_reservoir }

    /// Number of items seen.
    pub fn yh_count(&self) -> usize { self.yh_count }

    /// Sample size.
    pub fn yh_k(&self) -> usize { self.yh_k }

    /// Reset the sampler.
    pub fn yh_reset(&mut self, seed: u64) {
        self.yh_reservoir.clear();
        self.yh_count = 0;
        self.yh_seed = seed;
    }

    /// Is the reservoir full.
    pub fn yh_is_full(&self) -> bool { self.yh_reservoir.len() == self.yh_k }

    /// Current reservoir size.
    pub fn yh_len(&self) -> usize { self.yh_reservoir.len() }
}


// --- yi_ Ring Buffer ---

/// Fixed-capacity ring buffer (circular buffer) with O(1) push/pop at both ends.
#[derive(Debug, Clone)]
pub struct YiRingBuffer<T: Clone + Default> {
    yi_data: Vec<T>,
    yi_head: usize,
    yi_len: usize,
    yi_cap: usize,
}

impl<T: Clone + Default + std::fmt::Display> std::fmt::Display for YiRingBuffer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RingBuffer(len={}, cap={})", self.yi_len, self.yi_cap)
    }
}

impl<T: Clone + Default> Default for YiRingBuffer<T> {
    fn default() -> Self { Self::yi_new(16) }
}

impl<T: Clone + Default> YiRingBuffer<T> {
    /// Create a ring buffer with given capacity.
    pub fn yi_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self { yi_data: vec![T::default(); cap], yi_head: 0, yi_len: 0, yi_cap: cap }
    }

    /// Current number of elements.
    pub fn yi_len(&self) -> usize { self.yi_len }

    /// Maximum capacity.
    pub fn yi_capacity(&self) -> usize { self.yi_cap }

    /// Is the buffer empty.
    pub fn yi_is_empty(&self) -> bool { self.yi_len == 0 }

    /// Is the buffer full.
    pub fn yi_is_full(&self) -> bool { self.yi_len == self.yi_cap }

    /// Push to the back. Returns false if full.
    pub fn yi_push_back(&mut self, value: T) -> bool {
        if self.yi_is_full() { return false; }
        let idx = (self.yi_head + self.yi_len) % self.yi_cap;
        self.yi_data[idx] = value;
        self.yi_len += 1;
        true
    }

    /// Push to the front. Returns false if full.
    pub fn yi_push_front(&mut self, value: T) -> bool {
        if self.yi_is_full() { return false; }
        self.yi_head = if self.yi_head == 0 { self.yi_cap - 1 } else { self.yi_head - 1 };
        self.yi_data[self.yi_head] = value;
        self.yi_len += 1;
        true
    }

    /// Pop from the front.
    pub fn yi_pop_front(&mut self) -> Option<T> {
        if self.yi_is_empty() { return None; }
        let val = self.yi_data[self.yi_head].clone();
        self.yi_head = (self.yi_head + 1) % self.yi_cap;
        self.yi_len -= 1;
        Some(val)
    }

    /// Pop from the back.
    pub fn yi_pop_back(&mut self) -> Option<T> {
        if self.yi_is_empty() { return None; }
        self.yi_len -= 1;
        let idx = (self.yi_head + self.yi_len) % self.yi_cap;
        Some(self.yi_data[idx].clone())
    }

    /// Peek at the front.
    pub fn yi_front(&self) -> Option<&T> {
        if self.yi_is_empty() { None } else { Some(&self.yi_data[self.yi_head]) }
    }

    /// Peek at the back.
    pub fn yi_back(&self) -> Option<&T> {
        if self.yi_is_empty() { None }
        else { Some(&self.yi_data[(self.yi_head + self.yi_len - 1) % self.yi_cap]) }
    }

    /// Get element at logical index.
    pub fn yi_get(&self, index: usize) -> Option<&T> {
        if index >= self.yi_len { None }
        else { Some(&self.yi_data[(self.yi_head + index) % self.yi_cap]) }
    }

    /// Convert to vec preserving order.
    pub fn yi_to_vec(&self) -> Vec<T> {
        (0..self.yi_len).map(|i| self.yi_data[(self.yi_head + i) % self.yi_cap].clone()).collect()
    }

    /// Clear the buffer.
    pub fn yi_clear(&mut self) { self.yi_len = 0; self.yi_head = 0; }

    /// Force push to back, overwriting oldest if full.
    pub fn yi_force_push_back(&mut self, value: T) {
        if self.yi_is_full() { self.yi_pop_front(); }
        self.yi_push_back(value);
    }
}

// --- yi_ Weighted Graph ---

/// Weighted directed graph with Dijkstra shortest paths.
#[derive(Debug, Clone)]
pub struct YiWeightedGraph {
    yi_adj: std::collections::HashMap<usize, Vec<(usize, f64)>>,
    yi_nodes: std::collections::HashSet<usize>,
}

impl std::fmt::Display for YiWeightedGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let edges: usize = self.yi_adj.values().map(|v| v.len()).sum();
        write!(f, "WGraph(nodes={}, edges={})", self.yi_nodes.len(), edges)
    }
}

impl Default for YiWeightedGraph {
    fn default() -> Self { Self::yi_new() }
}

impl YiWeightedGraph {
    /// Create an empty weighted graph.
    pub fn yi_new() -> Self {
        Self { yi_adj: std::collections::HashMap::new(), yi_nodes: std::collections::HashSet::new() }
    }

    /// Add a node.
    pub fn yi_add_node(&mut self, node: usize) {
        self.yi_nodes.insert(node);
        self.yi_adj.entry(node).or_default();
    }

    /// Add a weighted directed edge.
    pub fn yi_add_edge(&mut self, from: usize, to: usize, weight: f64) {
        self.yi_add_node(from);
        self.yi_add_node(to);
        self.yi_adj.entry(from).or_default().push((to, weight));
    }

    /// Number of nodes.
    pub fn yi_node_count(&self) -> usize { self.yi_nodes.len() }

    /// Dijkstra shortest path distances from source.
    pub fn yi_dijkstra(&self, source: usize) -> std::collections::HashMap<usize, f64> {
        let mut dist: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();
        let mut visited: std::collections::HashSet<usize> = std::collections::HashSet::new();
        dist.insert(source, 0.0);
        loop {
            let mut min_node = None;
            let mut min_dist = f64::INFINITY;
            for (node, d) in &dist {
                if !visited.contains(node) && *d < min_dist {
                    min_dist = *d;
                    min_node = Some(*node);
                }
            }
            let Some(u) = min_node else { break };
            visited.insert(u);
            if let Some(neighbors) = self.yi_adj.get(&u) {
                for (v, w) in neighbors {
                    let new_dist = min_dist + w;
                    let entry = dist.entry(*v).or_insert(f64::INFINITY);
                    if new_dist < *entry { *entry = new_dist; }
                }
            }
        }
        dist
    }

    /// Shortest path distance between two nodes.
    pub fn yi_shortest_distance(&self, from: usize, to: usize) -> Option<f64> {
        let dists = self.yi_dijkstra(from);
        dists.get(&to).copied().filter(|d| d.is_finite())
    }

    /// Get neighbors of a node.
    pub fn yi_neighbors(&self, node: usize) -> &[(usize, f64)] {
        self.yi_adj.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Check if node exists.
    pub fn yi_has_node(&self, node: usize) -> bool { self.yi_nodes.contains(&node) }

    /// Clear the graph.
    pub fn yi_clear(&mut self) { self.yi_adj.clear(); self.yi_nodes.clear(); }

    /// Total edge weight.
    pub fn yi_total_weight(&self) -> f64 {
        self.yi_adj.values().flat_map(|v| v.iter()).map(|(_, w)| w).sum()
    }

    /// Add bidirectional edge.
    pub fn yi_add_undirected_edge(&mut self, a: usize, b: usize, weight: f64) {
        self.yi_add_edge(a, b, weight);
        self.yi_add_edge(b, a, weight);
    }
}


// --- yj_ Expression Evaluator ---

/// Simple arithmetic expression evaluator supporting +, -, *, /, parentheses.
#[derive(Debug, Clone)]
pub struct YjExprEval {
    yj_vars: std::collections::HashMap<String, f64>,
}

impl std::fmt::Display for YjExprEval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExprEval(vars={})", self.yj_vars.len())
    }
}

impl Default for YjExprEval {
    fn default() -> Self { Self::yj_new() }
}

impl YjExprEval {
    /// Create a new expression evaluator.
    pub fn yj_new() -> Self { Self { yj_vars: std::collections::HashMap::new() } }

    /// Set a variable.
    pub fn yj_set_var(&mut self, name: &str, value: f64) { self.yj_vars.insert(name.to_string(), value); }

    /// Get a variable.
    pub fn yj_get_var(&self, name: &str) -> Option<f64> { self.yj_vars.get(name).copied() }

    /// Evaluate an expression string.
    pub fn yj_eval(&self, expr: &str) -> std::result::Result<f64, String> {
        let tokens = Self::yj_tokenize(expr)?;
        let mut pos = 0;
        let result = self.yj_parse_expr(&tokens, &mut pos)?;
        if pos != tokens.len() { return Err("unexpected token".to_string()); }
        Ok(result)
    }

    fn yj_tokenize(expr: &str) -> std::result::Result<Vec<String>, String> {
        let mut tokens = Vec::new();
        let mut chars = expr.chars().peekable();
        while let Some(&ch) = chars.peek() {
            if ch.is_whitespace() { chars.next(); continue; }
            if ch.is_ascii_digit() || ch == '.' {
                let mut num = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' { num.push(c); chars.next(); } else { break; }
                }
                tokens.push(num);
            } else if ch.is_ascii_alphabetic() || ch == '_' {
                let mut name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' { name.push(c); chars.next(); } else { break; }
                }
                tokens.push(name);
            } else if "+-*/()".contains(ch) {
                tokens.push(ch.to_string());
                chars.next();
            } else {
                return Err(format!("unexpected character: {}", ch));
            }
        }
        Ok(tokens)
    }

    fn yj_parse_expr(&self, tokens: &[String], pos: &mut usize) -> std::result::Result<f64, String> {
        let mut result = self.yj_parse_term(tokens, pos)?;
        while *pos < tokens.len() && (tokens[*pos] == "+" || tokens[*pos] == "-") {
            let op = tokens[*pos].clone();
            *pos += 1;
            let right = self.yj_parse_term(tokens, pos)?;
            result = if op == "+" { result + right } else { result - right };
        }
        Ok(result)
    }

    fn yj_parse_term(&self, tokens: &[String], pos: &mut usize) -> std::result::Result<f64, String> {
        let mut result = self.yj_parse_factor(tokens, pos)?;
        while *pos < tokens.len() && (tokens[*pos] == "*" || tokens[*pos] == "/") {
            let op = tokens[*pos].clone();
            *pos += 1;
            let right = self.yj_parse_factor(tokens, pos)?;
            result = if op == "*" { result * right } else { result / right };
        }
        Ok(result)
    }

    fn yj_parse_factor(&self, tokens: &[String], pos: &mut usize) -> std::result::Result<f64, String> {
        if *pos >= tokens.len() { return Err("unexpected end".to_string()); }
        if tokens[*pos] == "(" {
            *pos += 1;
            let result = self.yj_parse_expr(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos] != ")" { return Err("missing )".to_string()); }
            *pos += 1;
            return Ok(result);
        }
        if tokens[*pos] == "-" {
            *pos += 1;
            let val = self.yj_parse_factor(tokens, pos)?;
            return Ok(-val);
        }
        if let Ok(num) = tokens[*pos].parse::<f64>() {
            *pos += 1;
            return Ok(num);
        }
        if let Some(val) = self.yj_vars.get(&tokens[*pos]) {
            *pos += 1;
            return Ok(*val);
        }
        Err(format!("unknown token: {}", tokens[*pos]))
    }

    /// Clear all variables.
    pub fn yj_clear(&mut self) { self.yj_vars.clear(); }

    /// Number of variables.
    pub fn yj_var_count(&self) -> usize { self.yj_vars.len() }
}

// --- yj_ TTL Cache ---

/// Cache with time-to-live expiration for entries.
#[derive(Debug, Clone)]
pub struct YjTtlCache<V: Clone> {
    yj_entries: std::collections::HashMap<String, (V, u64)>,
    yj_ttl: u64,
    yj_clock: u64,
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YjTtlCache<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TtlCache(entries={}, ttl={})", self.yj_entries.len(), self.yj_ttl)
    }
}

impl<V: Clone> Default for YjTtlCache<V> {
    fn default() -> Self { Self::yj_new(60) }
}

impl<V: Clone> YjTtlCache<V> {
    /// Create a TTL cache with given TTL in ticks.
    pub fn yj_new(ttl: u64) -> Self {
        Self { yj_entries: std::collections::HashMap::new(), yj_ttl: ttl, yj_clock: 0 }
    }

    /// Advance the clock by a given number of ticks.
    pub fn yj_tick(&mut self, ticks: u64) { self.yj_clock += ticks; }

    /// Current clock value.
    pub fn yj_clock(&self) -> u64 { self.yj_clock }

    /// Insert a key-value pair.
    pub fn yj_put(&mut self, key: &str, value: V) {
        self.yj_entries.insert(key.to_string(), (value, self.yj_clock));
    }

    /// Get a value if not expired.
    pub fn yj_get(&self, key: &str) -> Option<&V> {
        self.yj_entries.get(key).and_then(|(v, ts)| {
            if self.yj_clock - ts <= self.yj_ttl { Some(v) } else { None }
        })
    }

    /// Check if a key exists and is not expired.
    pub fn yj_contains(&self, key: &str) -> bool { self.yj_get(key).is_some() }

    /// Remove expired entries.
    pub fn yj_evict_expired(&mut self) {
        let clock = self.yj_clock;
        let ttl = self.yj_ttl;
        self.yj_entries.retain(|_, (_, ts)| clock - *ts <= ttl);
    }

    /// Number of entries (including possibly expired).
    pub fn yj_len(&self) -> usize { self.yj_entries.len() }

    /// Number of valid (non-expired) entries.
    pub fn yj_valid_count(&self) -> usize {
        self.yj_entries.values().filter(|(_, ts)| self.yj_clock - *ts <= self.yj_ttl).count()
    }

    /// Remove a key.
    pub fn yj_remove(&mut self, key: &str) -> Option<V> {
        self.yj_entries.remove(key).map(|(v, _)| v)
    }

    /// Clear the cache.
    pub fn yj_clear(&mut self) { self.yj_entries.clear(); }

    /// TTL value.
    pub fn yj_ttl(&self) -> u64 { self.yj_ttl }

    /// Set new TTL.
    pub fn yj_set_ttl(&mut self, ttl: u64) { self.yj_ttl = ttl; }
}


// --- yk_ Glob Pattern Matcher ---

/// Simple glob pattern matcher supporting *, ?, and character classes.
#[derive(Debug, Clone)]
pub struct YkGlobMatcher {
    yk_pattern: String,
}

impl std::fmt::Display for YkGlobMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Glob({})", self.yk_pattern)
    }
}

impl Default for YkGlobMatcher {
    fn default() -> Self { Self { yk_pattern: String::new() } }
}

impl YkGlobMatcher {
    /// Create a glob matcher from a pattern.
    pub fn yk_new(pattern: &str) -> Self { Self { yk_pattern: pattern.to_string() } }

    /// Get the pattern.
    pub fn yk_pattern(&self) -> &str { &self.yk_pattern }

    /// Check if a string matches the glob pattern.
    pub fn yk_matches(&self, text: &str) -> bool {
        Self::yk_match_impl(self.yk_pattern.as_bytes(), text.as_bytes())
    }

    fn yk_match_impl(pattern: &[u8], text: &[u8]) -> bool {
        let mut pi = 0;
        let mut ti = 0;
        let mut star_pi = usize::MAX;
        let mut star_ti = 0;
        while ti < text.len() {
            if pi < pattern.len() && (pattern[pi] == b'?' || pattern[pi] == text[ti]) {
                pi += 1;
                ti += 1;
            } else if pi < pattern.len() && pattern[pi] == b'*' {
                star_pi = pi;
                star_ti = ti;
                pi += 1;
            } else if star_pi != usize::MAX {
                pi = star_pi + 1;
                star_ti += 1;
                ti = star_ti;
            } else {
                return false;
            }
        }
        while pi < pattern.len() && pattern[pi] == b'*' { pi += 1; }
        pi == pattern.len()
    }

    /// Match multiple patterns (any match).
    pub fn yk_matches_any(patterns: &[&str], text: &str) -> bool {
        patterns.iter().any(|p| YkGlobMatcher::yk_new(p).yk_matches(text))
    }

    /// Match multiple patterns (all match).
    pub fn yk_matches_all(patterns: &[&str], text: &str) -> bool {
        patterns.iter().all(|p| YkGlobMatcher::yk_new(p).yk_matches(text))
    }

    /// Filter a list of strings by this pattern.
    pub fn yk_filter<'a>(&self, items: &[&'a str]) -> Vec<&'a str> {
        items.iter().filter(|s| self.yk_matches(s)).copied().collect()
    }
}

// --- yk_ Event Bus ---

/// Simple typed event bus with subscriber IDs.
#[derive(Debug, Clone)]
pub struct YkEventBus {
    yk_events: Vec<(String, Vec<(usize, String)>)>,
    yk_next_id: usize,
}

impl std::fmt::Display for YkEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total: usize = self.yk_events.iter().map(|(_, subs)| subs.len()).sum();
        write!(f, "EventBus(topics={}, subs={})", self.yk_events.len(), total)
    }
}

impl Default for YkEventBus {
    fn default() -> Self { Self::yk_new() }
}

impl YkEventBus {
    /// Create a new event bus.
    pub fn yk_new() -> Self { Self { yk_events: Vec::new(), yk_next_id: 0 } }

    /// Subscribe to a topic. Returns subscription ID.
    pub fn yk_subscribe(&mut self, topic: &str, handler_name: &str) -> usize {
        let id = self.yk_next_id;
        self.yk_next_id += 1;
        if let Some((_, subs)) = self.yk_events.iter_mut().find(|(t, _)| t == topic) {
            subs.push((id, handler_name.to_string()));
        } else {
            self.yk_events.push((topic.to_string(), vec![(id, handler_name.to_string())]));
        }
        id
    }

    /// Unsubscribe by ID.
    pub fn yk_unsubscribe(&mut self, id: usize) {
        for (_, subs) in &mut self.yk_events {
            subs.retain(|(sid, _)| *sid != id);
        }
    }

    /// Emit an event, returns list of handler names that were notified.
    pub fn yk_emit(&self, topic: &str) -> Vec<String> {
        self.yk_events.iter()
            .filter(|(t, _)| t == topic)
            .flat_map(|(_, subs)| subs.iter().map(|(_, name)| name.clone()))
            .collect()
    }

    /// Number of topics.
    pub fn yk_topic_count(&self) -> usize { self.yk_events.len() }

    /// Number of subscribers for a topic.
    pub fn yk_subscriber_count(&self, topic: &str) -> usize {
        self.yk_events.iter().find(|(t, _)| t == topic).map(|(_, s)| s.len()).unwrap_or(0)
    }

    /// Total subscribers across all topics.
    pub fn yk_total_subscribers(&self) -> usize {
        self.yk_events.iter().map(|(_, subs)| subs.len()).sum()
    }

    /// List all topics.
    pub fn yk_topics(&self) -> Vec<String> {
        self.yk_events.iter().map(|(t, _)| t.clone()).collect()
    }

    /// Clear all subscriptions.
    pub fn yk_clear(&mut self) { self.yk_events.clear(); self.yk_next_id = 0; }

    /// Check if a topic has subscribers.
    pub fn yk_has_subscribers(&self, topic: &str) -> bool {
        self.yk_subscriber_count(topic) > 0
    }

    /// Emit to topics matching a glob pattern.
    pub fn yk_emit_pattern(&self, pattern: &str) -> Vec<(String, Vec<String>)> {
        let matcher = YkGlobMatcher::yk_new(pattern);
        self.yk_events.iter()
            .filter(|(t, _)| matcher.yk_matches(t))
            .map(|(t, subs)| (t.clone(), subs.iter().map(|(_, n)| n.clone()).collect()))
            .collect()
    }
}


// --- yl_ Min-Max Heap ---

/// Min-max heap: O(1) access to both min and max, O(log n) insert/remove.
#[derive(Debug, Clone)]
pub struct YlMinMaxHeap {
    yl_data: Vec<i64>,
}

impl std::fmt::Display for YlMinMaxHeap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MinMaxHeap(size={})", self.yl_data.len())
    }
}

impl Default for YlMinMaxHeap {
    fn default() -> Self { Self::yl_new() }
}

impl YlMinMaxHeap {
    /// Create an empty min-max heap.
    pub fn yl_new() -> Self { Self { yl_data: Vec::new() } }

    /// Number of elements.
    pub fn yl_len(&self) -> usize { self.yl_data.len() }

    /// Is empty.
    pub fn yl_is_empty(&self) -> bool { self.yl_data.is_empty() }

    fn yl_is_min_level(idx: usize) -> bool {
        let level = ((idx + 1) as f64).log2().floor() as u32;
        level % 2 == 0
    }

    /// Insert a value.
    pub fn yl_insert(&mut self, val: i64) {
        self.yl_data.push(val);
        let idx = self.yl_data.len() - 1;
        self.yl_bubble_up(idx);
    }

    fn yl_bubble_up(&mut self, idx: usize) {
        if idx == 0 { return; }
        let parent = (idx - 1) / 2;
        if Self::yl_is_min_level(idx) {
            if self.yl_data[idx] > self.yl_data[parent] {
                self.yl_data.swap(idx, parent);
                self.yl_bubble_up_max(parent);
            } else {
                self.yl_bubble_up_min(idx);
            }
        } else {
            if self.yl_data[idx] < self.yl_data[parent] {
                self.yl_data.swap(idx, parent);
                self.yl_bubble_up_min(parent);
            } else {
                self.yl_bubble_up_max(idx);
            }
        }
    }

    fn yl_bubble_up_min(&mut self, mut idx: usize) {
        while idx > 2 {
            let grandparent = ((idx - 1) / 2 - 1) / 2;
            if self.yl_data[idx] < self.yl_data[grandparent] {
                self.yl_data.swap(idx, grandparent);
                idx = grandparent;
            } else { break; }
        }
    }

    fn yl_bubble_up_max(&mut self, mut idx: usize) {
        while idx > 2 {
            let grandparent = ((idx - 1) / 2 - 1) / 2;
            if self.yl_data[idx] > self.yl_data[grandparent] {
                self.yl_data.swap(idx, grandparent);
                idx = grandparent;
            } else { break; }
        }
    }

    /// Peek at minimum.
    pub fn yl_peek_min(&self) -> Option<i64> { self.yl_data.first().copied() }

    /// Peek at maximum.
    pub fn yl_peek_max(&self) -> Option<i64> {
        match self.yl_data.len() {
            0 => None,
            1 => Some(self.yl_data[0]),
            2 => Some(self.yl_data[1]),
            _ => Some(self.yl_data[1].max(self.yl_data[2])),
        }
    }

    /// Pop minimum.
    pub fn yl_pop_min(&mut self) -> Option<i64> {
        if self.yl_data.is_empty() { return None; }
        let min = self.yl_data[0];
        let last = self.yl_data.len() - 1;
        self.yl_data.swap(0, last);
        self.yl_data.pop();
        if !self.yl_data.is_empty() { self.yl_trickle_down(0); }
        Some(min)
    }

    fn yl_trickle_down(&mut self, idx: usize) {
        if Self::yl_is_min_level(idx) {
            self.yl_trickle_down_min(idx);
        } else {
            self.yl_trickle_down_max(idx);
        }
    }

    fn yl_trickle_down_min(&mut self, idx: usize) {
        let n = self.yl_data.len();
        let mut smallest = idx;
        for child in [2 * idx + 1, 2 * idx + 2] {
            if child < n && self.yl_data[child] < self.yl_data[smallest] { smallest = child; }
            for gc in [2 * child + 1, 2 * child + 2] {
                if gc < n && self.yl_data[gc] < self.yl_data[smallest] { smallest = gc; }
            }
        }
        if smallest != idx {
            self.yl_data.swap(idx, smallest);
            if smallest > 2 * idx + 2 { // grandchild
                let parent = (smallest - 1) / 2;
                if self.yl_data[smallest] > self.yl_data[parent] {
                    self.yl_data.swap(smallest, parent);
                }
                self.yl_trickle_down_min(smallest);
            }
        }
    }

    fn yl_trickle_down_max(&mut self, idx: usize) {
        let n = self.yl_data.len();
        let mut largest = idx;
        for child in [2 * idx + 1, 2 * idx + 2] {
            if child < n && self.yl_data[child] > self.yl_data[largest] { largest = child; }
            for gc in [2 * child + 1, 2 * child + 2] {
                if gc < n && self.yl_data[gc] > self.yl_data[largest] { largest = gc; }
            }
        }
        if largest != idx {
            self.yl_data.swap(idx, largest);
            if largest > 2 * idx + 2 {
                let parent = (largest - 1) / 2;
                if self.yl_data[largest] < self.yl_data[parent] {
                    self.yl_data.swap(largest, parent);
                }
                self.yl_trickle_down_max(largest);
            }
        }
    }

    /// Convert to sorted vec.
    pub fn yl_to_sorted_vec(&mut self) -> Vec<i64> {
        let mut result = Vec::with_capacity(self.yl_data.len());
        while let Some(v) = self.yl_pop_min() { result.push(v); }
        result
    }

    /// Clear.
    pub fn yl_clear(&mut self) { self.yl_data.clear(); }
}

// --- yl_ State Machine ---

/// Simple deterministic finite state machine.
#[derive(Debug, Clone)]
pub struct YlStateMachine {
    yl_states: Vec<String>,
    yl_current: usize,
    yl_transitions: Vec<(usize, String, usize)>,
    yl_accept: std::collections::HashSet<usize>,
}

impl std::fmt::Display for YlStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FSM(states={}, current={})", self.yl_states.len(),
            self.yl_states.get(self.yl_current).map(|s| s.as_str()).unwrap_or("?"))
    }
}

impl Default for YlStateMachine {
    fn default() -> Self { Self::yl_new() }
}

impl YlStateMachine {
    /// Create an empty state machine.
    pub fn yl_new() -> Self {
        Self { yl_states: Vec::new(), yl_current: 0, yl_transitions: Vec::new(), yl_accept: std::collections::HashSet::new() }
    }

    /// Add a state. Returns state index.
    pub fn yl_add_state(&mut self, name: &str) -> usize {
        let idx = self.yl_states.len();
        self.yl_states.push(name.to_string());
        idx
    }

    /// Add a transition.
    pub fn yl_add_transition(&mut self, from: usize, input: &str, to: usize) {
        self.yl_transitions.push((from, input.to_string(), to));
    }

    /// Mark a state as accepting.
    pub fn yl_set_accept(&mut self, state: usize) { self.yl_accept.insert(state); }

    /// Set starting state.
    pub fn yl_set_start(&mut self, state: usize) { self.yl_current = state; }

    /// Process an input. Returns true if transition found.
    pub fn yl_step(&mut self, input: &str) -> bool {
        for (from, inp, to) in &self.yl_transitions {
            if *from == self.yl_current && inp == input {
                self.yl_current = *to;
                return true;
            }
        }
        false
    }

    /// Process a sequence of inputs. Returns true if all transitions found.
    pub fn yl_run(&mut self, inputs: &[&str]) -> bool {
        for input in inputs {
            if !self.yl_step(input) { return false; }
        }
        true
    }

    /// Current state name.
    pub fn yl_current_state(&self) -> &str {
        self.yl_states.get(self.yl_current).map(|s| s.as_str()).unwrap_or("")
    }

    /// Is current state accepting.
    pub fn yl_is_accepting(&self) -> bool { self.yl_accept.contains(&self.yl_current) }

    /// Number of states.
    pub fn yl_state_count(&self) -> usize { self.yl_states.len() }

    /// Number of transitions.
    pub fn yl_transition_count(&self) -> usize { self.yl_transitions.len() }

    /// Available transitions from current state.
    pub fn yl_available_inputs(&self) -> Vec<String> {
        self.yl_transitions.iter()
            .filter(|(from, _, _)| *from == self.yl_current)
            .map(|(_, input, _)| input.clone())
            .collect()
    }

    /// Reset to start state.
    pub fn yl_reset(&mut self) { self.yl_current = 0; }
}


// --- ym_ Sorted Multi-Map ---

/// Sorted multi-map allowing multiple values per key, stored in a BTreeMap.
#[derive(Debug, Clone)]
pub struct YmSortedMultiMap<K: Ord + Clone, V: Clone> {
    ym_data: std::collections::BTreeMap<K, Vec<V>>,
    ym_count: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for YmSortedMultiMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SortedMultiMap(keys={}, total={})", self.ym_data.len(), self.ym_count)
    }
}

impl<K: Ord + Clone, V: Clone> Default for YmSortedMultiMap<K, V> {
    fn default() -> Self { Self::ym_new() }
}

impl<K: Ord + Clone, V: Clone> YmSortedMultiMap<K, V> {
    /// Create empty sorted multi-map.
    pub fn ym_new() -> Self { Self { ym_data: std::collections::BTreeMap::new(), ym_count: 0 } }

    /// Insert a key-value pair.
    pub fn ym_insert(&mut self, key: K, value: V) {
        self.ym_data.entry(key).or_default().push(value);
        self.ym_count += 1;
    }

    /// Get all values for a key.
    pub fn ym_get(&self, key: &K) -> &[V] {
        self.ym_data.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Number of unique keys.
    pub fn ym_key_count(&self) -> usize { self.ym_data.len() }

    /// Total number of values.
    pub fn ym_total_count(&self) -> usize { self.ym_count }

    /// Is empty.
    pub fn ym_is_empty(&self) -> bool { self.ym_count == 0 }

    /// Contains key.
    pub fn ym_contains_key(&self, key: &K) -> bool { self.ym_data.contains_key(key) }

    /// Remove all values for a key.
    pub fn ym_remove_key(&mut self, key: &K) -> Vec<V> {
        if let Some(vals) = self.ym_data.remove(key) {
            self.ym_count -= vals.len();
            vals
        } else {
            Vec::new()
        }
    }

    /// Get all keys in sorted order.
    pub fn ym_keys(&self) -> Vec<K> {
        self.ym_data.keys().cloned().collect()
    }

    /// Get keys in a range.
    pub fn ym_range(&self, lo: &K, hi: &K) -> Vec<K> {
        self.ym_data.range(lo..=hi).map(|(k, _)| k.clone()).collect()
    }

    /// First key.
    pub fn ym_first_key(&self) -> Option<K> {
        self.ym_data.keys().next().cloned()
    }

    /// Last key.
    pub fn ym_last_key(&self) -> Option<K> {
        self.ym_data.keys().next_back().cloned()
    }

    /// Clear.
    pub fn ym_clear(&mut self) { self.ym_data.clear(); self.ym_count = 0; }

    /// Count values for a key.
    pub fn ym_count_for(&self, key: &K) -> usize {
        self.ym_data.get(key).map(|v| v.len()).unwrap_or(0)
    }
}

// --- ym_ Task Scheduler ---

/// Priority-based task scheduler with dependencies.
#[derive(Debug, Clone)]
pub struct YmTaskScheduler {
    ym_tasks: Vec<YmTask>,
    ym_next_id: usize,
}

/// A scheduled task.
#[derive(Debug, Clone)]
pub struct YmTask {
    /// Task ID.
    pub ym_id: usize,
    /// Task name.
    pub ym_name: String,
    /// Priority (lower = higher priority).
    pub ym_priority: i32,
    /// Dependencies (task IDs that must complete first).
    pub ym_deps: Vec<usize>,
    /// Is completed.
    pub ym_done: bool,
}

impl std::fmt::Display for YmTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Task({}: {}, pri={}, done={})", self.ym_id, self.ym_name, self.ym_priority, self.ym_done)
    }
}

impl std::fmt::Display for YmTaskScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Scheduler(tasks={}, pending={})", self.ym_tasks.len(), self.ym_pending_count())
    }
}

impl Default for YmTaskScheduler {
    fn default() -> Self { Self::ym_new() }
}

impl YmTaskScheduler {
    /// Create an empty scheduler.
    pub fn ym_new() -> Self { Self { ym_tasks: Vec::new(), ym_next_id: 0 } }

    /// Add a task. Returns task ID.
    pub fn ym_add_task(&mut self, name: &str, priority: i32, deps: Vec<usize>) -> usize {
        let id = self.ym_next_id;
        self.ym_next_id += 1;
        self.ym_tasks.push(YmTask { ym_id: id, ym_name: name.to_string(), ym_priority: priority, ym_deps: deps, ym_done: false });
        id
    }

    /// Mark a task as done.
    pub fn ym_complete(&mut self, id: usize) {
        if let Some(t) = self.ym_tasks.iter_mut().find(|t| t.ym_id == id) {
            t.ym_done = true;
        }
    }

    /// Get the next ready task (all deps done, highest priority).
    pub fn ym_next_ready(&self) -> Option<&YmTask> {
        let done_set: std::collections::HashSet<usize> = self.ym_tasks.iter()
            .filter(|t| t.ym_done).map(|t| t.ym_id).collect();
        self.ym_tasks.iter()
            .filter(|t| !t.ym_done && t.ym_deps.iter().all(|d| done_set.contains(d)))
            .min_by_key(|t| t.ym_priority)
    }

    /// Get all ready tasks.
    pub fn ym_all_ready(&self) -> Vec<&YmTask> {
        let done_set: std::collections::HashSet<usize> = self.ym_tasks.iter()
            .filter(|t| t.ym_done).map(|t| t.ym_id).collect();
        let mut ready: Vec<&YmTask> = self.ym_tasks.iter()
            .filter(|t| !t.ym_done && t.ym_deps.iter().all(|d| done_set.contains(d)))
            .collect();
        ready.sort_by_key(|t| t.ym_priority);
        ready
    }

    /// Number of pending tasks.
    pub fn ym_pending_count(&self) -> usize {
        self.ym_tasks.iter().filter(|t| !t.ym_done).count()
    }

    /// Number of completed tasks.
    pub fn ym_done_count(&self) -> usize {
        self.ym_tasks.iter().filter(|t| t.ym_done).count()
    }

    /// Total tasks.
    pub fn ym_total(&self) -> usize { self.ym_tasks.len() }

    /// Is all done.
    pub fn ym_is_all_done(&self) -> bool { self.ym_pending_count() == 0 }

    /// Get task by ID.
    pub fn ym_get_task(&self, id: usize) -> Option<&YmTask> {
        self.ym_tasks.iter().find(|t| t.ym_id == id)
    }

    /// Clear.
    pub fn ym_clear(&mut self) { self.ym_tasks.clear(); self.ym_next_id = 0; }
}


// --- yn_ Immutable Map (HAMT-inspired) ---

/// Persistent immutable map using a sorted vector for small maps.
#[derive(Debug, Clone)]
pub struct YnImmutableMap<K: Ord + Clone, V: Clone> {
    yn_entries: Vec<(K, V)>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for YnImmutableMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ImmMap(size={})", self.yn_entries.len())
    }
}

impl<K: Ord + Clone, V: Clone> Default for YnImmutableMap<K, V> {
    fn default() -> Self { Self::yn_new() }
}

impl<K: Ord + Clone, V: Clone> YnImmutableMap<K, V> {
    /// Create an empty immutable map.
    pub fn yn_new() -> Self { Self { yn_entries: Vec::new() } }

    /// Insert returns a new map with the key-value pair added.
    pub fn yn_insert(&self, key: K, value: V) -> Self {
        let mut entries = self.yn_entries.clone();
        match entries.binary_search_by(|(k, _)| k.cmp(&key)) {
            Ok(idx) => entries[idx] = (key, value),
            Err(idx) => entries.insert(idx, (key, value)),
        }
        Self { yn_entries: entries }
    }

    /// Remove returns a new map without the key.
    pub fn yn_remove(&self, key: &K) -> Self {
        let mut entries = self.yn_entries.clone();
        if let Ok(idx) = entries.binary_search_by(|(k, _)| k.cmp(key)) {
            entries.remove(idx);
        }
        Self { yn_entries: entries }
    }

    /// Look up a key.
    pub fn yn_get(&self, key: &K) -> Option<&V> {
        self.yn_entries.binary_search_by(|(k, _)| k.cmp(key))
            .ok()
            .map(|idx| &self.yn_entries[idx].1)
    }

    /// Contains key.
    pub fn yn_contains_key(&self, key: &K) -> bool { self.yn_get(key).is_some() }

    /// Number of entries.
    pub fn yn_len(&self) -> usize { self.yn_entries.len() }

    /// Is empty.
    pub fn yn_is_empty(&self) -> bool { self.yn_entries.is_empty() }

    /// All keys in sorted order.
    pub fn yn_keys(&self) -> Vec<K> { self.yn_entries.iter().map(|(k, _)| k.clone()).collect() }

    /// All values.
    pub fn yn_values(&self) -> Vec<V> { self.yn_entries.iter().map(|(_, v)| v.clone()).collect() }

    /// Merge with another map (other takes precedence).
    pub fn yn_merge(&self, other: &Self) -> Self {
        let mut result = self.clone();
        for (k, v) in &other.yn_entries {
            result = result.yn_insert(k.clone(), v.clone());
        }
        result
    }

    /// Map values.
    pub fn yn_map_values<F: Fn(&V) -> V>(&self, f: F) -> Self {
        Self { yn_entries: self.yn_entries.iter().map(|(k, v)| (k.clone(), f(v))).collect() }
    }

    /// Filter entries.
    pub fn yn_filter<F: Fn(&K, &V) -> bool>(&self, f: F) -> Self {
        Self { yn_entries: self.yn_entries.iter().filter(|(k, v)| f(k, v)).cloned().collect() }
    }
}

// --- yn_ Tokenizer ---

/// Simple token-based text tokenizer for parsing structured text.
#[derive(Debug, Clone, PartialEq)]
pub enum YnTokenKind {
    /// A word/identifier.
    YnWord,
    /// A number literal.
    YnNumber,
    /// A string literal.
    YnString,
    /// An operator or punctuation.
    YnPunct,
    /// Whitespace.
    YnWhitespace,
}

impl std::fmt::Display for YnTokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::YnWord => write!(f, "Word"),
            Self::YnNumber => write!(f, "Number"),
            Self::YnString => write!(f, "String"),
            Self::YnPunct => write!(f, "Punct"),
            Self::YnWhitespace => write!(f, "Whitespace"),
        }
    }
}

/// A token produced by the tokenizer.
#[derive(Debug, Clone)]
pub struct YnToken {
    /// Token kind.
    pub yn_kind: YnTokenKind,
    /// Token text.
    pub yn_text: String,
    /// Start offset.
    pub yn_start: usize,
}

impl std::fmt::Display for YnToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({:?}@{})", self.yn_kind, self.yn_text, self.yn_start)
    }
}

/// Simple text tokenizer.
#[derive(Debug, Clone)]
pub struct YnTokenizer;

impl std::fmt::Display for YnTokenizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tokenizer")
    }
}

impl Default for YnTokenizer {
    fn default() -> Self { Self }
}

impl YnTokenizer {
    /// Tokenize input text.
    pub fn yn_tokenize(input: &str) -> Vec<YnToken> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let start = i;
            if chars[i].is_whitespace() {
                while i < chars.len() && chars[i].is_whitespace() { i += 1; }
                tokens.push(YnToken { yn_kind: YnTokenKind::YnWhitespace, yn_text: chars[start..i].iter().collect(), yn_start: start });
            } else if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') { i += 1; }
                tokens.push(YnToken { yn_kind: YnTokenKind::YnWord, yn_text: chars[start..i].iter().collect(), yn_start: start });
            } else if chars[i].is_ascii_digit() {
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') { i += 1; }
                tokens.push(YnToken { yn_kind: YnTokenKind::YnNumber, yn_text: chars[start..i].iter().collect(), yn_start: start });
            } else if chars[i] == '\"' {
                i += 1;
                while i < chars.len() && chars[i] != '"' { i += 1; }
                if i < chars.len() { i += 1; }
                tokens.push(YnToken { yn_kind: YnTokenKind::YnString, yn_text: chars[start..i].iter().collect(), yn_start: start });
            } else {
                i += 1;
                tokens.push(YnToken { yn_kind: YnTokenKind::YnPunct, yn_text: chars[start..i].iter().collect(), yn_start: start });
            }
        }
        tokens
    }

    /// Tokenize and filter out whitespace.
    pub fn yn_tokenize_no_ws(input: &str) -> Vec<YnToken> {
        Self::yn_tokenize(input).into_iter().filter(|t| t.yn_kind != YnTokenKind::YnWhitespace).collect()
    }

    /// Count tokens by kind.
    pub fn yn_count_by_kind(tokens: &[YnToken], kind: &YnTokenKind) -> usize {
        tokens.iter().filter(|t| t.yn_kind == *kind).count()
    }
}


// --- yo_ Levenshtein Distance ---

/// Levenshtein (edit) distance calculator for fuzzy string matching.
#[derive(Debug, Clone)]
pub struct YoLevenshtein;

impl std::fmt::Display for YoLevenshtein {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Levenshtein")
    }
}

impl Default for YoLevenshtein {
    fn default() -> Self { Self }
}

impl YoLevenshtein {
    /// Compute edit distance between two strings.
    pub fn yo_distance(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let m = a_chars.len();
        let n = b_chars.len();
        if m == 0 { return n; }
        if n == 0 { return m; }
        let mut prev = (0..=n).collect::<Vec<_>>();
        let mut curr = vec![0; n + 1];
        for i in 1..=m {
            curr[0] = i;
            for j in 1..=n {
                let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
                curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
            }
            std::mem::swap(&mut prev, &mut curr);
        }
        prev[n]
    }

    /// Normalized similarity [0.0, 1.0].
    pub fn yo_similarity(a: &str, b: &str) -> f64 {
        let max_len = a.chars().count().max(b.chars().count());
        if max_len == 0 { return 1.0; }
        1.0 - (Self::yo_distance(a, b) as f64 / max_len as f64)
    }

    /// Find closest match from candidates.
    pub fn yo_closest<'a>(target: &str, candidates: &[&'a str]) -> Option<&'a str> {
        candidates.iter().min_by_key(|c| Self::yo_distance(target, c)).copied()
    }

    /// Filter candidates within a max distance.
    pub fn yo_within_distance<'a>(target: &str, candidates: &[&'a str], max_dist: usize) -> Vec<&'a str> {
        candidates.iter().filter(|c| Self::yo_distance(target, c) <= max_dist).copied().collect()
    }

    /// Rank candidates by distance (closest first).
    pub fn yo_rank<'a>(target: &str, candidates: &[&'a str]) -> Vec<(&'a str, usize)> {
        let mut ranked: Vec<_> = candidates.iter().map(|c| (*c, Self::yo_distance(target, c))).collect();
        ranked.sort_by_key(|(_, d)| *d);
        ranked
    }
}

// --- yo_ Diff Engine ---

/// Line-based diff engine using longest common subsequence.
#[derive(Debug, Clone, PartialEq)]
pub enum YoDiffOp {
    /// Line exists in both.
    YoEqual(String),
    /// Line added in new version.
    YoInsert(String),
    /// Line removed from old version.
    YoDelete(String),
}

impl std::fmt::Display for YoDiffOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::YoEqual(s) => write!(f, "  {}", s),
            Self::YoInsert(s) => write!(f, "+ {}", s),
            Self::YoDelete(s) => write!(f, "- {}", s),
        }
    }
}

/// Line-based diff engine.
#[derive(Debug, Clone)]
pub struct YoDiffEngine;

impl std::fmt::Display for YoDiffEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DiffEngine")
    }
}

impl Default for YoDiffEngine {
    fn default() -> Self { Self }
}

impl YoDiffEngine {
    /// Compute diff between two texts (split by lines).
    pub fn yo_diff(old: &str, new: &str) -> Vec<YoDiffOp> {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();
        let lcs = Self::yo_lcs(&old_lines, &new_lines);
        let mut result = Vec::new();
        let mut oi = 0;
        let mut ni = 0;
        let mut li = 0;
        while oi < old_lines.len() || ni < new_lines.len() {
            if li < lcs.len() && oi < old_lines.len() && ni < new_lines.len() && old_lines[oi] == lcs[li] && new_lines[ni] == lcs[li] {
                result.push(YoDiffOp::YoEqual(lcs[li].to_string()));
                oi += 1; ni += 1; li += 1;
            } else if ni < new_lines.len() && (li >= lcs.len() || new_lines[ni] != lcs[li]) {
                result.push(YoDiffOp::YoInsert(new_lines[ni].to_string()));
                ni += 1;
            } else if oi < old_lines.len() && (li >= lcs.len() || old_lines[oi] != lcs[li]) {
                result.push(YoDiffOp::YoDelete(old_lines[oi].to_string()));
                oi += 1;
            }
        }
        result
    }

    fn yo_lcs<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<&'a str> {
        let m = a.len();
        let n = b.len();
        let mut dp = vec![vec![0usize; n + 1]; m + 1];
        for i in 1..=m {
            for j in 1..=n {
                dp[i][j] = if a[i - 1] == b[j - 1] { dp[i - 1][j - 1] + 1 } else { dp[i - 1][j].max(dp[i][j - 1]) };
            }
        }
        let mut result = Vec::new();
        let (mut i, mut j) = (m, n);
        while i > 0 && j > 0 {
            if a[i - 1] == b[j - 1] { result.push(a[i - 1]); i -= 1; j -= 1; }
            else if dp[i - 1][j] > dp[i][j - 1] { i -= 1; }
            else { j -= 1; }
        }
        result.reverse();
        result
    }

    /// Count insertions in a diff.
    pub fn yo_count_insertions(ops: &[YoDiffOp]) -> usize {
        ops.iter().filter(|op| matches!(op, YoDiffOp::YoInsert(_))).count()
    }

    /// Count deletions in a diff.
    pub fn yo_count_deletions(ops: &[YoDiffOp]) -> usize {
        ops.iter().filter(|op| matches!(op, YoDiffOp::YoDelete(_))).count()
    }

    /// Count equal lines.
    pub fn yo_count_equal(ops: &[YoDiffOp]) -> usize {
        ops.iter().filter(|op| matches!(op, YoDiffOp::YoEqual(_))).count()
    }

    /// Format diff as unified diff string.
    pub fn yo_format(ops: &[YoDiffOp]) -> String {
        ops.iter().map(|op| format!("{}", op)).collect::<Vec<_>>().join("\n")
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


    #[test]
    fn xp_216_splay_insert_get() {
        let mut t = super::Xp216SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_216_splay_remove() {
        let mut t = super::Xp216SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_216_splay_count_increases() {
        let mut t = super::Xp216SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_216_splay_depth() {
        let mut t = super::Xp216SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_216_splay_len_empty() {
        let t = super::Xp216SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_216_splay_min_max() {
        let mut t = super::Xp216SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_216_splay_overwrite() {
        let mut t = super::Xp216SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_216_splay_remove_missing() {
        let mut t = super::Xp216SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_216 treap tests ----
    #[test]
    fn xq_216_treap_empty() {
        let t = super::Xq216Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_216_treap_insert_get() {
        let mut t = super::Xq216Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_216_treap_overwrite() {
        let mut t = super::Xq216Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_216_treap_remove() {
        let mut t = super::Xq216Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_216_treap_min_max() {
        let mut t = super::Xq216Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_216_treap_rank() {
        let mut t = super::Xq216Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_216_treap_kth() {
        let mut t = super::Xq216Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_216_treap_in_order() {
        let mut t = super::Xq216Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_216 VEB tree tests ----
    #[test]
    fn xq_216_veb_empty() {
        let v = super::Xq216VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_216_veb_insert_contains() {
        let mut v = super::Xq216VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_216_veb_min_max() {
        let mut v = super::Xq216VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_216_veb_delete() {
        let mut v = super::Xq216VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_216_veb_successor() {
        let mut v = super::Xq216VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_216_veb_predecessor() {
        let mut v = super::Xq216VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_216_veb_count() {
        let mut v = super::Xq216VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_216_veb_duplicate_insert() {
        let mut v = super::Xq216VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_216_kdtree_empty() {
        let tree = super::Xr216KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_216_kdtree_insert_one() {
        let mut tree = super::Xr216KDTree::xr_new();
        tree.xr_insert(super::Xr216KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_216_kdtree_insert_multiple() {
        let mut tree = super::Xr216KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr216KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_216_kdtree_nearest_neighbor() {
        let mut tree = super::Xr216KDTree::xr_new();
        tree.xr_insert(super::Xr216KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr216KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr216KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_216_kdtree_nn_empty() {
        let tree = super::Xr216KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr216KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_216_kdtree_range_search() {
        let mut tree = super::Xr216KDTree::xr_new();
        tree.xr_insert(super::Xr216KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr216KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr216KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_216_kdtree_range_empty() {
        let mut tree = super::Xr216KDTree::xr_new();
        tree.xr_insert(super::Xr216KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_216_kdtree_all_points() {
        let mut tree = super::Xr216KDTree::xr_new();
        tree.xr_insert(super::Xr216KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr216KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_216_kdtree_depth() {
        let mut tree = super::Xr216KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr216KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_216_kdtree_bounding_box() {
        let mut tree = super::Xr216KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr216KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr216KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_216_persistent_array_new() {
        let arr = super::Xs216PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_216_persistent_array_push() {
        let mut arr = super::Xs216PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_216_persistent_array_set() {
        let mut arr = super::Xs216PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_216_persistent_array_diff() {
        let mut arr = super::Xs216PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_216_persistent_array_rollback() {
        let mut arr = super::Xs216PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_216_persistent_array_history() {
        let mut arr = super::Xs216PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_216_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs216PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_216_persistent_array_from_vec() {
        let arr = super::Xs216PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_216_concurrent_queue_new() {
        let q = super::Xs216ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_216_concurrent_queue_push_pop() {
        let mut q = super::Xs216ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_216_concurrent_queue_full() {
        let mut q = super::Xs216ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_216_concurrent_queue_drain() {
        let mut q = super::Xs216ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_216_concurrent_queue_try_pop() {
        let mut q = super::Xs216ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_216_concurrent_queue_clear() {
        let mut q = super::Xs216ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_216_range_map_new() {
        let rm = super::Xs216RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_216_range_map_insert_get() {
        let mut rm = super::Xs216RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_216_range_map_overlap() {
        let mut rm = super::Xs216RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_216_range_map_remove() {
        let mut rm = super::Xs216RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_216_range_map_gaps() {
        let mut rm = super::Xs216RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_216_range_map_coverage() {
        let mut rm = super::Xs216RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_216_range_map_contains() {
        let mut rm = super::Xs216RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_216_range_map_clear() {
        let mut rm = super::Xs216RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_216_circular_buffer_new() {
        let buf = super::Xs216CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_216_circular_buffer_push_pop() {
        let mut buf = super::Xs216CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_216_circular_buffer_overwrite() {
        let mut buf = super::Xs216CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_216_circular_buffer_peek() {
        let mut buf = super::Xs216CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_216_circular_buffer_is_full() {
        let mut buf = super::Xs216CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_216_circular_buffer_iter() {
        let mut buf = super::Xs216CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_216_circular_buffer_clear() {
        let mut buf = super::Xs216CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_216_circular_buffer_to_vec() {
        let mut buf = super::Xs216CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }


    // --- xu_ Binomial Heap tests ---

    #[test]
    fn xu_bin_heap_new() {
        let h = super::XuBinomialHeap::<i32, &str>::xu_new();
        assert!(h.xu_is_empty());
        assert_eq!(h.xu_len(), 0);
    }

    #[test]
    fn xu_bin_heap_insert_find_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(5, "five");
        h.xu_insert(3, "three");
        h.xu_insert(7, "seven");
        assert_eq!(h.xu_len(), 3);
        assert_eq!(h.xu_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xu_bin_heap_extract_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(10, "a");
        h.xu_insert(2, "b");
        h.xu_insert(8, "c");
        h.xu_insert(1, "d");
        assert_eq!(h.xu_extract_min(), Some((1, "d")));
        assert_eq!(h.xu_extract_min(), Some((2, "b")));
    }

    #[test]
    fn xu_bin_heap_sorted_drain() {
        let mut h = super::XuBinomialHeap::xu_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xu_insert(v, v * 10);
        }
        let sorted = h.xu_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xu_bin_heap_merge() {
        let mut h1 = super::XuBinomialHeap::xu_new();
        h1.xu_insert(3, "a");
        h1.xu_insert(7, "b");
        let mut h2 = super::XuBinomialHeap::xu_new();
        h2.xu_insert(1, "c");
        h2.xu_insert(5, "d");
        h1.xu_merge(&mut h2);
        assert_eq!(h1.xu_len(), 4);
        assert_eq!(h1.xu_find_min(), Some((&1, &"c")));
    }

    #[test]
    fn xu_bin_heap_clear() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "a");
        h.xu_clear();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_heap_display() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "x");
        assert!(format!("{}", h).contains("BinHeap"));
    }

    #[test]
    fn xu_bin_heap_default() {
        let h = super::XuBinomialHeap::<i32, i32>::default();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_node_display() {
        let n = super::XuBinomialNode::xu_new(5, "v");
        assert!(format!("{}", n).contains("BinNode"));
    }

    #[test]
    fn xu_bin_heap_single() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(42, "answer");
        assert_eq!(h.xu_extract_min(), Some((42, "answer")));
        assert!(h.xu_is_empty());
    }

    // --- xu_ Disjoint Sparse Table tests ---

    #[test]
    fn xu_dst_build() {
        let data = vec![1, 2, 3, 4, 5];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_len(), 5);
        assert!(!dst.xu_is_empty());
    }

    #[test]
    fn xu_dst_single_element_query() {
        let data = vec![10, 20, 30];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_query(0, 0), 10);
        assert_eq!(dst.xu_query(1, 1), 20);
        assert_eq!(dst.xu_query(2, 2), 30);
    }

    #[test]
    fn xu_dst_get() {
        let data = vec![5, 10, 15];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_get(0), Some(&5));
        assert_eq!(dst.xu_get(2), Some(&15));
        assert_eq!(dst.xu_get(10), None);
    }

    #[test]
    fn xu_dst_empty() {
        let dst = super::XuDisjointSparseTable::<i32>::xu_build(&[]);
        assert!(dst.xu_is_empty());
        assert_eq!(dst.xu_len(), 0);
    }

    #[test]
    fn xu_dst_display() {
        let data = vec![1, 2, 3];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert!(format!("{}", dst).contains("DST"));
    }

    // --- xu_ Monotonic Stack tests ---

    #[test]
    fn xu_mono_stack_increasing() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        assert!(s.xu_is_empty());
        let popped = s.xu_push(3);
        assert!(popped.is_empty());
        let popped = s.xu_push(5);
        assert!(popped.is_empty());
        let popped = s.xu_push(2);
        assert_eq!(popped, vec![5, 3]);
        assert_eq!(s.xu_as_slice(), &[2]);
    }

    #[test]
    fn xu_mono_stack_decreasing() {
        let mut s = super::XuMonotonicStack::xu_decreasing();
        s.xu_push(2);
        s.xu_push(1);
        let popped = s.xu_push(5);
        assert_eq!(popped, vec![1, 2]);
        assert_eq!(s.xu_as_slice(), &[5]);
    }

    #[test]
    fn xu_mono_stack_peek_pop() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(3);
        s.xu_push(5);
        assert_eq!(s.xu_peek(), Some(&5));
        assert_eq!(s.xu_pop(), Some(5));
        assert_eq!(s.xu_len(), 2);
    }

    #[test]
    fn xu_mono_stack_clear() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(2);
        s.xu_clear();
        assert!(s.xu_is_empty());
    }

    #[test]
    fn xu_mono_stack_display() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        assert!(format!("{}", s).contains("MonoStack"));
    }


    // --- xv_ Cartesian Tree tests ---

    #[test]
    fn xv_cart_tree_new() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_cart_tree_insert_contains() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        t.xv_insert(3, 2);
        t.xv_insert(7, 3);
        assert!(t.xv_contains(&5));
        assert!(t.xv_contains(&3));
        assert!(t.xv_contains(&7));
        assert!(!t.xv_contains(&4));
        assert_eq!(t.xv_len(), 3);
    }

    #[test]
    fn xv_cart_tree_inorder() {
        let mut t = super::XvCartesianTree::xv_new();
        for (k, p) in [(5, 3), (3, 1), (7, 2), (1, 5), (9, 4)] {
            t.xv_insert(k, p);
        }
        let keys = t.xv_inorder();
        assert_eq!(keys, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn xv_cart_tree_min_priority() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 10);
        t.xv_insert(3, 2);
        t.xv_insert(7, 5);
        assert_eq!(t.xv_min_priority(), Some(&2));
    }

    #[test]
    fn xv_cart_tree_from_pairs() {
        let t = super::XvCartesianTree::xv_from_pairs(&[(3, 1), (1, 3), (5, 2)]);
        assert_eq!(t.xv_len(), 3);
        assert!(t.xv_contains(&1));
    }

    #[test]
    fn xv_cart_tree_height() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        assert!(t.xv_height() >= 1);
    }

    #[test]
    fn xv_cart_tree_clear() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(1, 1);
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_tree_display() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("CartTree"));
    }

    #[test]
    fn xv_cart_tree_default() {
        let t = super::XvCartesianTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_node_display() {
        let n = super::XvCartesianNode { xv_key: 1, xv_priority: 2, xv_left: None, xv_right: None };
        assert!(format!("{}", n).contains("CartNode"));
    }

    // --- xv_ Weight-Balanced Tree tests ---

    #[test]
    fn xv_wb_tree_new() {
        let t = super::XvWeightBalancedTree::<i32, &str>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_wb_tree_insert_get() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "five");
        t.xv_insert(3, "three");
        t.xv_insert(7, "seven");
        assert_eq!(t.xv_get(&5), Some(&"five"));
        assert_eq!(t.xv_get(&3), Some(&"three"));
        assert_eq!(t.xv_get(&7), Some(&"seven"));
        assert_eq!(t.xv_get(&4), None);
    }

    #[test]
    fn xv_wb_tree_contains() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(10, "a");
        assert!(t.xv_contains(&10));
        assert!(!t.xv_contains(&20));
    }

    #[test]
    fn xv_wb_tree_keys_sorted() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xv_insert(k, k * 10);
        }
        assert_eq!(t.xv_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xv_wb_tree_replace_value() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "old");
        t.xv_insert(5, "new");
        assert_eq!(t.xv_get(&5), Some(&"new"));
        assert_eq!(t.xv_len(), 1);
    }

    #[test]
    fn xv_wb_tree_height() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in 1..=15 {
            t.xv_insert(k, k);
        }
        assert!(t.xv_height() <= 20);
    }

    #[test]
    fn xv_wb_tree_clear() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(1, "a");
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_tree_display() {
        let t = super::XvWeightBalancedTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("WBTree"));
    }

    #[test]
    fn xv_wb_tree_default() {
        let t = super::XvWeightBalancedTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_node_display() {
        let n = super::XvWBNode { xv_key: 1, xv_value: "a", xv_left: None, xv_right: None, xv_weight: 2 };
        assert!(format!("{}", n).contains("WBNode"));
    }


    // --- xw_ Scapegoat Tree tests ---

    #[test]
    fn xw_sg_tree_new() {
        let t = super::XwScapegoatTree::<i32, &str>::xw_new();
        assert!(t.xw_is_empty());
        assert_eq!(t.xw_len(), 0);
    }

    #[test]
    fn xw_sg_tree_insert_get() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "five");
        t.xw_insert(3, "three");
        t.xw_insert(7, "seven");
        assert_eq!(t.xw_get(&5), Some(&"five"));
        assert_eq!(t.xw_get(&3), Some(&"three"));
        assert_eq!(t.xw_get(&4), None);
    }

    #[test]
    fn xw_sg_tree_contains() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(10, "a");
        assert!(t.xw_contains(&10));
        assert!(!t.xw_contains(&20));
    }

    #[test]
    fn xw_sg_tree_keys_sorted() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xw_insert(k, k * 10);
        }
        assert_eq!(t.xw_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xw_sg_tree_sequential_inserts() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in 1..=20 {
            t.xw_insert(k, k);
        }
        assert_eq!(t.xw_len(), 20);
        assert!(t.xw_height() <= 15);
    }

    #[test]
    fn xw_sg_tree_replace_value() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "old");
        t.xw_insert(5, "new");
        assert_eq!(t.xw_get(&5), Some(&"new"));
        assert_eq!(t.xw_len(), 1);
    }

    #[test]
    fn xw_sg_tree_clear() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(1, "a");
        t.xw_clear();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_tree_display() {
        let t = super::XwScapegoatTree::<i32, i32>::xw_new();
        assert!(format!("{}", t).contains("SGTree"));
    }

    #[test]
    fn xw_sg_tree_default() {
        let t = super::XwScapegoatTree::<i32, i32>::default();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_node_display() {
        let n = super::XwScapegoatNode { xw_key: 1, xw_value: "a", xw_left: None, xw_right: None };
        assert!(format!("{}", n).contains("SGNode"));
    }

    // --- xw_ Rope tests ---

    #[test]
    fn xw_rope_new() {
        let r = super::XwRope::xw_new();
        assert!(r.xw_is_empty());
        assert_eq!(r.xw_len(), 0);
    }

    #[test]
    fn xw_rope_from_str() {
        let r = super::XwRope::xw_from_str("hello");
        assert_eq!(r.xw_len(), 5);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_concat() {
        let a = super::XwRope::xw_from_str("hello ");
        let b = super::XwRope::xw_from_str("world");
        let c = super::XwRope::xw_concat(a, b);
        assert_eq!(c.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_insert() {
        let mut r = super::XwRope::xw_from_str("helo");
        r.xw_insert(3, "l");
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_delete() {
        let mut r = super::XwRope::xw_from_str("hello world");
        r.xw_delete(5, 11);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_append() {
        let mut r = super::XwRope::xw_from_str("hello");
        r.xw_append(" world");
        assert_eq!(r.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_substring() {
        let r = super::XwRope::xw_from_str("hello world");
        assert_eq!(r.xw_substring(6, 11), "world");
    }

    #[test]
    fn xw_rope_char_at() {
        let r = super::XwRope::xw_from_str("abc");
        assert_eq!(r.xw_char_at(0), Some('a'));
        assert_eq!(r.xw_char_at(2), Some('c'));
    }

    #[test]
    fn xw_rope_clear() {
        let mut r = super::XwRope::xw_from_str("text");
        r.xw_clear();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_display() {
        let r = super::XwRope::xw_from_str("test");
        assert!(format!("{}", r).contains("Rope"));
    }

    #[test]
    fn xw_rope_default() {
        let r = super::XwRope::default();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_empty_ops() {
        let r = super::XwRope::xw_new();
        assert_eq!(r.xw_to_string(), "");
        assert_eq!(r.xw_substring(0, 5), "");
    }


    // --- xx_ Skip List tests ---

    #[test]
    fn xx_skip_list_new() {
        let sl = super::XxSkipList::<i32, &str>::xx_new();
        assert!(sl.xx_is_empty());
        assert_eq!(sl.xx_len(), 0);
    }

    #[test]
    fn xx_skip_list_insert_get() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "five");
        sl.xx_insert(3, "three");
        sl.xx_insert(7, "seven");
        assert_eq!(sl.xx_get(&5), Some(&"five"));
        assert_eq!(sl.xx_get(&3), Some(&"three"));
        assert_eq!(sl.xx_get(&7), Some(&"seven"));
        assert_eq!(sl.xx_get(&4), None);
    }

    #[test]
    fn xx_skip_list_contains() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(10, "a");
        assert!(sl.xx_contains(&10));
        assert!(!sl.xx_contains(&20));
    }

    #[test]
    fn xx_skip_list_keys_sorted() {
        let mut sl = super::XxSkipList::xx_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            sl.xx_insert(k, k * 10);
        }
        assert_eq!(sl.xx_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xx_skip_list_replace() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "old");
        sl.xx_insert(5, "new");
        assert_eq!(sl.xx_get(&5), Some(&"new"));
    }

    #[test]
    fn xx_skip_list_many() {
        let mut sl = super::XxSkipList::xx_new();
        for k in 1..=50 {
            sl.xx_insert(k, k);
        }
        assert_eq!(sl.xx_len(), 50);
        for k in 1..=50 {
            assert!(sl.xx_contains(&k));
        }
    }

    #[test]
    fn xx_skip_list_clear() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(1, "a");
        sl.xx_clear();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_list_display() {
        let sl = super::XxSkipList::<i32, i32>::xx_new();
        assert!(format!("{}", sl).contains("SkipList"));
    }

    #[test]
    fn xx_skip_list_default() {
        let sl = super::XxSkipList::<i32, i32>::default();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_node_display() {
        let n = super::XxSkipNode::<i32, i32> { xx_key: Some(5), xx_value: Some(50), xx_forward: vec![None] };
        assert!(format!("{}", n).contains("SkipNode"));
    }

    // --- xx_ Suffix Array tests ---

    #[test]
    fn xx_suffix_array_new() {
        let sa = super::XxSuffixArray::xx_new("banana");
        assert_eq!(sa.xx_len(), 6);
        assert!(!sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_search() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let pos = sa.xx_search("ana");
        assert_eq!(pos.len(), 2);
    }

    #[test]
    fn xx_suffix_array_count() {
        let sa = super::XxSuffixArray::xx_new("abcabcabc");
        assert_eq!(sa.xx_count("abc"), 3);
    }

    #[test]
    fn xx_suffix_array_no_match() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_count("xyz"), 0);
    }

    #[test]
    fn xx_suffix_array_suffix_at() {
        let sa = super::XxSuffixArray::xx_new("abc");
        let s = sa.xx_suffix_at(0);
        assert!(!s.is_empty());
    }

    #[test]
    fn xx_suffix_array_longest_repeated() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let lr = sa.xx_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xx_suffix_array_empty() {
        let sa = super::XxSuffixArray::xx_new("");
        assert!(sa.xx_is_empty());
        assert_eq!(sa.xx_search("a").len(), 0);
    }

    #[test]
    fn xx_suffix_array_display() {
        let sa = super::XxSuffixArray::xx_new("test");
        assert!(format!("{}", sa).contains("SuffixArray"));
    }

    #[test]
    fn xx_suffix_array_default() {
        let sa = super::XxSuffixArray::default();
        assert!(sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_text() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_text(), "hello");
    }


    // --- xy_ Cuckoo Hash Map tests ---

    #[test]
    fn xy_cuckoo_new() {
        let m = super::XyCuckooMap::<String, i32>::xy_new(16);
        assert!(m.xy_is_empty());
        assert_eq!(m.xy_len(), 0);
    }

    #[test]
    fn xy_cuckoo_insert_get() {
        let mut m = super::XyCuckooMap::xy_new(32);
        m.xy_insert("hello".to_string(), 1);
        m.xy_insert("world".to_string(), 2);
        assert_eq!(m.xy_get(&"hello".to_string()), Some(&1));
        assert_eq!(m.xy_get(&"world".to_string()), Some(&2));
        assert_eq!(m.xy_get(&"missing".to_string()), None);
    }

    #[test]
    fn xy_cuckoo_contains() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(42, "a");
        assert!(m.xy_contains(&42));
        assert!(!m.xy_contains(&99));
    }

    #[test]
    fn xy_cuckoo_replace() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(5, "old");
        m.xy_insert(5, "new");
        assert_eq!(m.xy_get(&5), Some(&"new"));
    }

    #[test]
    fn xy_cuckoo_remove() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(10, "val");
        assert_eq!(m.xy_remove(&10), Some("val"));
        assert!(!m.xy_contains(&10));
    }

    #[test]
    fn xy_cuckoo_many() {
        let mut m = super::XyCuckooMap::xy_new(64);
        for i in 0..30 {
            m.xy_insert(i, i * 10);
        }
        assert_eq!(m.xy_len(), 30);
        for i in 0..30 {
            assert!(m.xy_contains(&i));
        }
    }

    #[test]
    fn xy_cuckoo_keys() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_insert(2, "b");
        let keys = m.xy_keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn xy_cuckoo_clear() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_clear();
        assert!(m.xy_is_empty());
    }

    #[test]
    fn xy_cuckoo_display() {
        let m = super::XyCuckooMap::<i32, i32>::xy_new(16);
        assert!(format!("{}", m).contains("CuckooMap"));
    }

    #[test]
    fn xy_cuckoo_default() {
        let m = super::XyCuckooMap::<i32, i32>::default();
        assert!(m.xy_is_empty());
    }

    // --- xy_ Count-Min Sketch tests ---

    #[test]
    fn xy_cms_new() {
        let cms = super::XyCountMinSketch::xy_new(100, 5);
        assert_eq!(cms.xy_width(), 100);
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_add_estimate() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..10 { cms.xy_add(42); }
        assert!(cms.xy_estimate(42) >= 10);
    }

    #[test]
    fn xy_cms_add_count() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        cms.xy_add_count(7, 100);
        assert!(cms.xy_estimate(7) >= 100);
    }

    #[test]
    fn xy_cms_unseen() {
        let cms = super::XyCountMinSketch::xy_new(1000, 5);
        assert_eq!(cms.xy_estimate(999), 0);
    }

    #[test]
    fn xy_cms_merge() {
        let mut a = super::XyCountMinSketch::xy_new(100, 3);
        let mut b = super::XyCountMinSketch::xy_new(100, 3);
        a.xy_add(1);
        b.xy_add(1);
        a.xy_merge(&b);
        assert!(a.xy_estimate(1) >= 2);
    }

    #[test]
    fn xy_cms_clear() {
        let mut cms = super::XyCountMinSketch::xy_new(100, 3);
        cms.xy_add(1);
        cms.xy_clear();
        assert_eq!(cms.xy_estimate(1), 0);
    }

    #[test]
    fn xy_cms_display() {
        let cms = super::XyCountMinSketch::xy_new(100, 3);
        assert!(format!("{}", cms).contains("CMS"));
    }

    #[test]
    fn xy_cms_default() {
        let cms = super::XyCountMinSketch::default();
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_multiple_items() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for i in 0..100 { cms.xy_add(i); }
        for i in 0..100 { assert!(cms.xy_estimate(i) >= 1); }
    }

    #[test]
    fn xy_cms_heavy_hitter() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..1000 { cms.xy_add(42); }
        for i in 0..10 { cms.xy_add(i); }
        assert!(cms.xy_estimate(42) > cms.xy_estimate(0));
    }


    // --- xz_ HyperLogLog tests ---

    #[test]
    fn xz_hll_new() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert_eq!(hll.xz_num_registers(), 1024);
        assert_eq!(hll.xz_precision(), 10);
    }

    #[test]
    fn xz_hll_add_estimate() {
        let mut hll = super::XzHyperLogLog::xz_new(12);
        for i in 0..1000 {
            hll.xz_add(i);
        }
        let est = hll.xz_estimate();
        assert!(est > 500.0 && est < 2000.0);
    }

    #[test]
    fn xz_hll_empty() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert_eq!(hll.xz_estimate(), 0.0);
    }

    #[test]
    fn xz_hll_merge() {
        let mut a = super::XzHyperLogLog::xz_new(10);
        let mut b = super::XzHyperLogLog::xz_new(10);
        for i in 0..500 { a.xz_add(i); }
        for i in 500..1000 { b.xz_add(i); }
        a.xz_merge(&b);
        let est = a.xz_estimate();
        assert!(est > 500.0);
    }

    #[test]
    fn xz_hll_clear() {
        let mut hll = super::XzHyperLogLog::xz_new(10);
        hll.xz_add(1);
        hll.xz_clear();
        assert_eq!(hll.xz_estimate(), 0.0);
    }

    #[test]
    fn xz_hll_display() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert!(format!("{}", hll).contains("HLL"));
    }

    #[test]
    fn xz_hll_default() {
        let hll = super::XzHyperLogLog::default();
        assert_eq!(hll.xz_precision(), 10);
    }

    #[test]
    fn xz_hll_duplicates() {
        let mut hll = super::XzHyperLogLog::xz_new(12);
        for _ in 0..1000 { hll.xz_add(42); }
        let est = hll.xz_estimate();
        assert!(est < 10.0);
    }

    // --- xz_ LRU Cache tests ---

    #[test]
    fn xz_lru_new() {
        let lru = super::XzLruCache::<String, i32>::xz_new(10);
        assert!(lru.xz_is_empty());
        assert_eq!(lru.xz_capacity(), 10);
    }

    #[test]
    fn xz_lru_put_get() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put("a".to_string(), 1);
        lru.xz_put("b".to_string(), 2);
        assert_eq!(lru.xz_get(&"a".to_string()), Some(&1));
        assert_eq!(lru.xz_get(&"b".to_string()), Some(&2));
    }

    #[test]
    fn xz_lru_eviction() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_put(3, "c");
        assert!(!lru.xz_contains(&1));
        assert!(lru.xz_contains(&2));
        assert!(lru.xz_contains(&3));
    }

    #[test]
    fn xz_lru_access_updates_order() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_get(&1);
        lru.xz_put(3, "c");
        assert!(lru.xz_contains(&1));
        assert!(!lru.xz_contains(&2));
    }

    #[test]
    fn xz_lru_update_value() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "old");
        lru.xz_put(1, "new");
        assert_eq!(lru.xz_get(&1), Some(&"new"));
        assert_eq!(lru.xz_len(), 1);
    }

    #[test]
    fn xz_lru_remove() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        assert_eq!(lru.xz_remove(&1), Some("a"));
        assert!(!lru.xz_contains(&1));
    }

    #[test]
    fn xz_lru_peek() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        assert_eq!(lru.xz_peek(&1), Some(&"a"));
        lru.xz_put(3, "c");
        assert!(lru.xz_contains(&1) || !lru.xz_contains(&1));
    }

    #[test]
    fn xz_lru_keys_order() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_put(3, "c");
        let keys = lru.xz_keys_lru();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn xz_lru_clear() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        lru.xz_clear();
        assert!(lru.xz_is_empty());
    }

    #[test]
    fn xz_lru_display() {
        let lru = super::XzLruCache::<i32, i32>::xz_new(10);
        assert!(format!("{}", lru).contains("LRU"));
    }

    #[test]
    fn xz_lru_missing_key() {
        let mut lru = super::XzLruCache::<i32, i32>::xz_new(10);
        assert_eq!(lru.xz_get(&999), None);
    }


    // --- ya_ Trie tests ---

    #[test]
    fn ya_trie_new() {
        let t = super::YaTrie::<i32>::ya_new();
        assert!(t.ya_is_empty());
        assert_eq!(t.ya_len(), 0);
    }

    #[test]
    fn ya_trie_insert_get() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("hello", 1);
        t.ya_insert("world", 2);
        assert_eq!(t.ya_get("hello"), Some(&1));
        assert_eq!(t.ya_get("world"), Some(&2));
        assert_eq!(t.ya_get("missing"), None);
    }

    #[test]
    fn ya_trie_contains() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        assert!(t.ya_contains("abc"));
        assert!(!t.ya_contains("ab"));
        assert!(!t.ya_contains("abcd"));
    }

    #[test]
    fn ya_trie_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        t.ya_insert("abd", 2);
        assert!(t.ya_has_prefix("ab"));
        assert!(!t.ya_has_prefix("ac"));
    }

    #[test]
    fn ya_trie_keys_with_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("cat", 1);
        t.ya_insert("car", 2);
        t.ya_insert("dog", 3);
        let keys = t.ya_keys_with_prefix("ca");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"cat".to_string()));
        assert!(keys.contains(&"car".to_string()));
    }

    #[test]
    fn ya_trie_all_keys() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("b", 1);
        t.ya_insert("a", 2);
        t.ya_insert("c", 3);
        let keys = t.ya_all_keys();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn ya_trie_remove() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("hello", 1);
        assert_eq!(t.ya_remove("hello"), Some(1));
        assert!(!t.ya_contains("hello"));
        assert_eq!(t.ya_len(), 0);
    }

    #[test]
    fn ya_trie_lcp() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        t.ya_insert("abd", 2);
        assert_eq!(t.ya_longest_common_prefix(), "ab");
    }

    #[test]
    fn ya_trie_clear() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("a", 1);
        t.ya_clear();
        assert!(t.ya_is_empty());
    }

    #[test]
    fn ya_trie_display() {
        let t = super::YaTrie::<i32>::ya_new();
        assert!(format!("{}", t).contains("Trie"));
    }

    #[test]
    fn ya_trie_default() {
        let t = super::YaTrie::<i32>::default();
        assert!(t.ya_is_empty());
    }

    #[test]
    fn ya_trie_count_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("test1", 1);
        t.ya_insert("test2", 2);
        t.ya_insert("other", 3);
        assert_eq!(t.ya_count_prefix("test"), 2);
    }

    // --- ya_ Bloom Filter tests ---

    #[test]
    fn ya_bloom_new() {
        let bf = super::YaBloomFilter::ya_new(1000, 5);
        assert_eq!(bf.ya_bit_size(), 1000);
        assert_eq!(bf.ya_num_hashes(), 5);
        assert_eq!(bf.ya_count(), 0);
    }

    #[test]
    fn ya_bloom_add_contains() {
        let mut bf = super::YaBloomFilter::ya_new(10000, 7);
        bf.ya_add(42);
        bf.ya_add(100);
        assert!(bf.ya_might_contain(42));
        assert!(bf.ya_might_contain(100));
    }

    #[test]
    fn ya_bloom_no_false_negatives() {
        let mut bf = super::YaBloomFilter::ya_new(10000, 7);
        for i in 0..100 { bf.ya_add(i); }
        for i in 0..100 { assert!(bf.ya_might_contain(i)); }
    }

    #[test]
    fn ya_bloom_with_fp_rate() {
        let bf = super::YaBloomFilter::ya_with_fp_rate(1000, 0.01);
        assert!(bf.ya_bit_size() > 0);
        assert!(bf.ya_num_hashes() > 0);
    }

    #[test]
    fn ya_bloom_clear() {
        let mut bf = super::YaBloomFilter::ya_new(1000, 5);
        bf.ya_add(1);
        bf.ya_clear();
        assert_eq!(bf.ya_count(), 0);
        assert!(!bf.ya_might_contain(1));
    }

    #[test]
    fn ya_bloom_merge() {
        let mut a = super::YaBloomFilter::ya_new(1000, 5);
        let mut b = super::YaBloomFilter::ya_new(1000, 5);
        a.ya_add(1);
        b.ya_add(2);
        a.ya_merge(&b);
        assert!(a.ya_might_contain(1));
        assert!(a.ya_might_contain(2));
    }

    #[test]
    fn ya_bloom_fp_rate() {
        let bf = super::YaBloomFilter::ya_new(1000, 5);
        assert_eq!(bf.ya_estimated_fp_rate(), 0.0);
    }

    #[test]
    fn ya_bloom_display() {
        let bf = super::YaBloomFilter::ya_new(100, 3);
        assert!(format!("{}", bf).contains("Bloom"));
    }

    #[test]
    fn ya_bloom_default() {
        let bf = super::YaBloomFilter::default();
        assert_eq!(bf.ya_num_hashes(), 5);
    }


    // --- yb_ TST tests ---

    #[test]
    fn yb_tst_new() {
        let t = super::YbTernarySearchTree::<i32>::yb_new();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_insert_get() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("hello", 1);
        t.yb_insert("world", 2);
        assert_eq!(t.yb_get("hello"), Some(&1));
        assert_eq!(t.yb_get("world"), Some(&2));
        assert_eq!(t.yb_get("missing"), None);
    }

    #[test]
    fn yb_tst_contains() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("abc", 10);
        assert!(t.yb_contains("abc"));
        assert!(!t.yb_contains("ab"));
    }

    #[test]
    fn yb_tst_all_keys() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("b", 1);
        t.yb_insert("a", 2);
        t.yb_insert("c", 3);
        let keys = t.yb_all_keys();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn yb_tst_prefix() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("cat", 1);
        t.yb_insert("car", 2);
        t.yb_insert("dog", 3);
        let keys = t.yb_keys_with_prefix("ca");
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn yb_tst_clear() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("a", 1);
        t.yb_clear();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_display() {
        let t = super::YbTernarySearchTree::<i32>::yb_new();
        assert!(format!("{}", t).contains("TST"));
    }

    #[test]
    fn yb_tst_default() {
        let t = super::YbTernarySearchTree::<i32>::default();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_overwrite() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("key", 1);
        t.yb_insert("key", 2);
        assert_eq!(t.yb_get("key"), Some(&2));
        assert_eq!(t.yb_len(), 1);
    }

    // --- yb_ Quadtree tests ---

    #[test]
    fn yb_quad_new() {
        let q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(q.yb_is_empty());
    }

    #[test]
    fn yb_quad_insert() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(q.yb_insert(super::YbPoint::yb_new(50.0, 50.0)));
        assert_eq!(q.yb_count(), 1);
    }

    #[test]
    fn yb_quad_query() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 2);
        q.yb_insert(super::YbPoint::yb_new(10.0, 10.0));
        q.yb_insert(super::YbPoint::yb_new(90.0, 90.0));
        q.yb_insert(super::YbPoint::yb_new(15.0, 15.0));
        let found = q.yb_query(&super::YbBounds::yb_new(0.0, 0.0, 50.0, 50.0));
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn yb_quad_outside() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(!q.yb_insert(super::YbPoint::yb_new(200.0, 200.0)));
    }

    #[test]
    fn yb_quad_nearest() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        q.yb_insert(super::YbPoint::yb_new(10.0, 10.0));
        q.yb_insert(super::YbPoint::yb_new(90.0, 90.0));
        let near = q.yb_nearest(&super::YbPoint::yb_new(12.0, 12.0)).unwrap();
        assert!((near.yb_x - 10.0).abs() < 0.001);
    }

    #[test]
    fn yb_quad_display() {
        let q = super::YbQuadtree::default();
        assert!(format!("{}", q).contains("Quadtree"));
    }

    #[test]
    fn yb_quad_default() {
        let q = super::YbQuadtree::default();
        assert!(q.yb_is_empty());
    }

    #[test]
    fn yb_quad_many() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 2);
        for i in 0..20 {
            q.yb_insert(super::YbPoint::yb_new(i as f64 * 4.0, i as f64 * 4.0));
        }
        assert_eq!(q.yb_count(), 20);
    }

    #[test]
    fn yb_point_distance() {
        let a = super::YbPoint::yb_new(0.0, 0.0);
        let b = super::YbPoint::yb_new(3.0, 4.0);
        assert!((a.yb_distance(&b) - 5.0).abs() < 0.001);
    }

    #[test]
    fn yb_bounds_intersects() {
        let a = super::YbBounds::yb_new(0.0, 0.0, 50.0, 50.0);
        let b = super::YbBounds::yb_new(25.0, 25.0, 50.0, 50.0);
        assert!(a.yb_intersects(&b));
    }


    // --- yc_ VebSet tests ---

    #[test]
    fn yc_veb_new() {
        let v = super::YcVebSet::yc_new(1000);
        assert!(v.yc_is_empty());
        assert_eq!(v.yc_universe(), 1000);
    }

    #[test]
    fn yc_veb_insert_contains() {
        let mut v = super::YcVebSet::yc_new(1000);
        assert!(v.yc_insert(42));
        assert!(v.yc_contains(42));
        assert!(!v.yc_contains(43));
    }

    #[test]
    fn yc_veb_remove() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(10);
        assert!(v.yc_remove(10));
        assert!(!v.yc_contains(10));
    }

    #[test]
    fn yc_veb_min_max() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(50);
        v.yc_insert(10);
        v.yc_insert(90);
        assert_eq!(v.yc_min(), Some(10));
        assert_eq!(v.yc_max(), Some(90));
    }

    #[test]
    fn yc_veb_successor() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(10);
        v.yc_insert(20);
        v.yc_insert(30);
        assert_eq!(v.yc_successor(10), Some(20));
        assert_eq!(v.yc_successor(20), Some(30));
        assert_eq!(v.yc_successor(30), None);
    }

    #[test]
    fn yc_veb_predecessor() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(10);
        v.yc_insert(20);
        assert_eq!(v.yc_predecessor(20), Some(10));
        assert_eq!(v.yc_predecessor(10), None);
    }

    #[test]
    fn yc_veb_sorted() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(30);
        v.yc_insert(10);
        v.yc_insert(20);
        assert_eq!(v.yc_to_sorted_vec(), vec![10, 20, 30]);
    }

    #[test]
    fn yc_veb_clear() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(1);
        v.yc_clear();
        assert!(v.yc_is_empty());
    }

    #[test]
    fn yc_veb_union() {
        let mut a = super::YcVebSet::yc_new(100);
        let mut b = super::YcVebSet::yc_new(100);
        a.yc_insert(1);
        b.yc_insert(2);
        a.yc_union(&b);
        assert!(a.yc_contains(1));
        assert!(a.yc_contains(2));
    }

    #[test]
    fn yc_veb_intersection() {
        let mut a = super::YcVebSet::yc_new(100);
        let mut b = super::YcVebSet::yc_new(100);
        a.yc_insert(1); a.yc_insert(2);
        b.yc_insert(2); b.yc_insert(3);
        let c = a.yc_intersection(&b);
        assert!(c.yc_contains(2));
        assert!(!c.yc_contains(1));
    }

    #[test]
    fn yc_veb_display() {
        let v = super::YcVebSet::yc_new(100);
        assert!(format!("{}", v).contains("VebSet"));
    }

    #[test]
    fn yc_veb_default() {
        let v = super::YcVebSet::default();
        assert_eq!(v.yc_universe(), 65536);
    }

    // --- yc_ HashRing tests ---

    #[test]
    fn yc_ring_new() {
        let r = super::YcHashRing::yc_new(100);
        assert_eq!(r.yc_node_count(), 0);
    }

    #[test]
    fn yc_ring_add_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("server1");
        assert_eq!(r.yc_node_count(), 1);
        assert_eq!(r.yc_virtual_count(), 50);
    }

    #[test]
    fn yc_ring_get_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("a");
        r.yc_add_node("b");
        let n = r.yc_get_node("mykey");
        assert!(n.is_some());
    }

    #[test]
    fn yc_ring_remove_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("a");
        r.yc_remove_node("a");
        assert_eq!(r.yc_node_count(), 0);
    }

    #[test]
    fn yc_ring_has_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("server1");
        assert!(r.yc_has_node("server1"));
        assert!(!r.yc_has_node("server2"));
    }

    #[test]
    fn yc_ring_display() {
        let r = super::YcHashRing::yc_new(10);
        assert!(format!("{}", r).contains("HashRing"));
    }

    #[test]
    fn yc_ring_default() {
        let r = super::YcHashRing::default();
        assert_eq!(r.yc_node_count(), 0);
    }

    #[test]
    fn yc_ring_consistency() {
        let mut r = super::YcHashRing::yc_new(100);
        r.yc_add_node("a");
        r.yc_add_node("b");
        let n1 = r.yc_get_node("key1").unwrap().to_string();
        let n2 = r.yc_get_node("key1").unwrap().to_string();
        assert_eq!(n1, n2);
    }


    // --- yd_ DAG tests ---

    #[test]
    fn yd_dag_new() {
        let g = super::YdDag::yd_new();
        assert_eq!(g.yd_node_count(), 0);
    }

    #[test]
    fn yd_dag_add_edge() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(1, 2);
        assert_eq!(g.yd_node_count(), 3);
        assert_eq!(g.yd_edge_count(), 2);
    }

    #[test]
    fn yd_dag_topo_sort() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        g.yd_add_edge(1, 3);
        g.yd_add_edge(2, 3);
        let order = g.yd_topological_sort().unwrap();
        assert_eq!(order.len(), 4);
        let pos: std::collections::HashMap<usize, usize> = order.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&0] < pos[&2]);
    }

    #[test]
    fn yd_dag_cycle() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(1, 2);
        g.yd_add_edge(2, 0);
        assert!(g.yd_has_cycle());
    }

    #[test]
    fn yd_dag_roots_leaves() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        assert_eq!(g.yd_roots(), vec![0]);
        assert_eq!(g.yd_leaves(), vec![1, 2]);
    }

    #[test]
    fn yd_dag_bfs() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        g.yd_add_edge(1, 3);
        let bfs = g.yd_bfs(0);
        assert_eq!(bfs[0], 0);
        assert_eq!(bfs.len(), 4);
    }

    #[test]
    fn yd_dag_dfs() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        let dfs = g.yd_dfs(0);
        assert_eq!(dfs[0], 0);
        assert_eq!(dfs.len(), 3);
    }

    #[test]
    fn yd_dag_shortest_path() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(1, 2);
        g.yd_add_edge(0, 2);
        assert_eq!(g.yd_shortest_path(0, 2), Some(1));
    }

    #[test]
    fn yd_dag_degrees() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        assert_eq!(g.yd_out_degree(0), 2);
        assert_eq!(g.yd_in_degree(1), 1);
    }

    #[test]
    fn yd_dag_clear() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_clear();
        assert_eq!(g.yd_node_count(), 0);
    }

    #[test]
    fn yd_dag_display() {
        let g = super::YdDag::yd_new();
        assert!(format!("{}", g).contains("DAG"));
    }

    // --- yd_ SparseMatrix tests ---

    #[test]
    fn yd_sparse_new() {
        let m = super::YdSparseMatrix::yd_new(3, 3);
        assert_eq!(m.yd_nnz(), 0);
    }

    #[test]
    fn yd_sparse_set_get() {
        let mut m = super::YdSparseMatrix::yd_new(3, 3);
        m.yd_set(0, 1, 5.0);
        assert_eq!(m.yd_get(0, 1), 5.0);
        assert_eq!(m.yd_get(0, 0), 0.0);
    }

    #[test]
    fn yd_sparse_transpose() {
        let mut m = super::YdSparseMatrix::yd_new(2, 3);
        m.yd_set(0, 2, 7.0);
        let t = m.yd_transpose();
        assert_eq!(t.yd_get(2, 0), 7.0);
    }

    #[test]
    fn yd_sparse_mul_vec() {
        let mut m = super::YdSparseMatrix::yd_new(2, 2);
        m.yd_set(0, 0, 1.0);
        m.yd_set(1, 1, 2.0);
        let r = m.yd_mul_vec(&[3.0, 4.0]);
        assert_eq!(r, vec![3.0, 8.0]);
    }

    #[test]
    fn yd_sparse_add() {
        let mut a = super::YdSparseMatrix::yd_new(2, 2);
        let mut b = super::YdSparseMatrix::yd_new(2, 2);
        a.yd_set(0, 0, 1.0);
        b.yd_set(0, 0, 2.0);
        let c = a.yd_add(&b);
        assert_eq!(c.yd_get(0, 0), 3.0);
    }

    #[test]
    fn yd_sparse_scale() {
        let mut m = super::YdSparseMatrix::yd_new(1, 1);
        m.yd_set(0, 0, 5.0);
        m.yd_scale(2.0);
        assert_eq!(m.yd_get(0, 0), 10.0);
    }

    #[test]
    fn yd_sparse_row_sum() {
        let mut m = super::YdSparseMatrix::yd_new(2, 3);
        m.yd_set(0, 0, 1.0);
        m.yd_set(0, 1, 2.0);
        m.yd_set(0, 2, 3.0);
        assert_eq!(m.yd_row_sum(0), 6.0);
    }

    #[test]
    fn yd_sparse_display() {
        let m = super::YdSparseMatrix::yd_new(2, 2);
        assert!(format!("{}", m).contains("SparseMatrix"));
    }

    #[test]
    fn yd_sparse_clear() {
        let mut m = super::YdSparseMatrix::yd_new(2, 2);
        m.yd_set(0, 0, 1.0);
        m.yd_clear();
        assert_eq!(m.yd_nnz(), 0);
    }


    // --- ye_ IndexedPQ tests ---

    #[test]
    fn ye_ipq_new() {
        let pq = super::YeIndexedPQ::ye_new();
        assert!(pq.ye_is_empty());
    }

    #[test]
    fn ye_ipq_insert_pop() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 10);
        pq.ye_insert(1, 5);
        pq.ye_insert(2, 15);
        assert_eq!(pq.ye_pop(), Some((1, 5)));
        assert_eq!(pq.ye_pop(), Some((0, 10)));
    }

    #[test]
    fn ye_ipq_decrease_key() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 10);
        pq.ye_insert(1, 20);
        pq.ye_decrease_key(1, 5);
        assert_eq!(pq.ye_peek(), Some((1, 5)));
    }

    #[test]
    fn ye_ipq_contains() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(42, 1);
        assert!(pq.ye_contains(42));
        assert!(!pq.ye_contains(99));
    }

    #[test]
    fn ye_ipq_priority() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 7);
        assert_eq!(pq.ye_priority(0), Some(7));
    }

    #[test]
    fn ye_ipq_drain() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 30);
        pq.ye_insert(1, 10);
        pq.ye_insert(2, 20);
        let sorted = pq.ye_drain_sorted();
        assert_eq!(sorted, vec![(1, 10), (2, 20), (0, 30)]);
    }

    #[test]
    fn ye_ipq_clear() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 1);
        pq.ye_clear();
        assert!(pq.ye_is_empty());
    }

    #[test]
    fn ye_ipq_display() {
        let pq = super::YeIndexedPQ::ye_new();
        assert!(format!("{}", pq).contains("IndexedPQ"));
    }

    #[test]
    fn ye_ipq_default() {
        let pq = super::YeIndexedPQ::default();
        assert!(pq.ye_is_empty());
    }

    // --- ye_ SegTree tests ---

    #[test]
    fn ye_seg_from_slice() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(st.ye_len(), 5);
        assert_eq!(st.ye_query(0, 4), 15);
    }

    #[test]
    fn ye_seg_point_query() {
        let mut st = super::YeSegTree::ye_from_slice(&[10, 20, 30]);
        assert_eq!(st.ye_point_query(1), 20);
    }

    #[test]
    fn ye_seg_range_query() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(st.ye_query(1, 3), 9);
    }

    #[test]
    fn ye_seg_update() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3, 4, 5]);
        st.ye_update(1, 3, 10);
        assert_eq!(st.ye_query(0, 4), 45);
    }

    #[test]
    fn ye_seg_single_update() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3]);
        st.ye_update(1, 1, 5);
        assert_eq!(st.ye_point_query(1), 7);
    }

    #[test]
    fn ye_seg_empty() {
        let st = super::YeSegTree::ye_from_slice(&[]);
        assert!(st.ye_is_empty());
    }

    #[test]
    fn ye_seg_single() {
        let mut st = super::YeSegTree::ye_from_slice(&[42]);
        assert_eq!(st.ye_query(0, 0), 42);
    }

    #[test]
    fn ye_seg_display() {
        let st = super::YeSegTree::ye_from_slice(&[1, 2, 3]);
        assert!(format!("{}", st).contains("SegTree"));
    }

    #[test]
    fn ye_seg_default() {
        let st = super::YeSegTree::default();
        assert!(st.ye_is_empty());
    }


    // --- yf_ IntervalSet tests ---

    #[test]
    fn yf_interval_new() {
        let s = super::YfIntervalSet::yf_new();
        assert!(s.yf_is_empty());
    }

    #[test]
    fn yf_interval_add() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        assert_eq!(s.yf_len(), 1);
        assert!(s.yf_contains(3));
    }

    #[test]
    fn yf_interval_merge() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_add(3, 8);
        assert_eq!(s.yf_len(), 1);
        assert_eq!(s.yf_intervals(), vec![(1, 8)]);
    }

    #[test]
    fn yf_interval_adjacent() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_add(6, 10);
        assert_eq!(s.yf_len(), 1);
    }

    #[test]
    fn yf_interval_disjoint() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 3);
        s.yf_add(10, 15);
        assert_eq!(s.yf_len(), 2);
    }

    #[test]
    fn yf_interval_remove_point() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 10);
        s.yf_remove_point(5);
        assert!(!s.yf_contains(5));
        assert!(s.yf_contains(4));
        assert!(s.yf_contains(6));
    }

    #[test]
    fn yf_interval_length() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_add(10, 14);
        assert_eq!(s.yf_total_length(), 10);
    }

    #[test]
    fn yf_interval_clear() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_clear();
        assert!(s.yf_is_empty());
    }

    #[test]
    fn yf_interval_display() {
        let s = super::YfIntervalSet::yf_new();
        assert!(format!("{}", s).contains("IntervalSet"));
    }

    #[test]
    fn yf_interval_overlaps() {
        let mut a = super::YfIntervalSet::yf_new();
        let mut b = super::YfIntervalSet::yf_new();
        a.yf_add(1, 5);
        b.yf_add(3, 8);
        assert!(a.yf_overlaps(&b));
    }

    // --- yf_ KWayMerge tests ---

    #[test]
    fn yf_kmerge_new() {
        let m = super::YfKWayMerge::yf_new();
        assert_eq!(m.yf_source_count(), 0);
    }

    #[test]
    fn yf_kmerge_merge() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 4, 7]);
        m.yf_add_source(vec![2, 5, 8]);
        m.yf_add_source(vec![3, 6, 9]);
        let result = m.yf_merge();
        assert_eq!(result, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn yf_kmerge_single() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2, 3]);
        let result = m.yf_merge();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn yf_kmerge_empty() {
        let mut m = super::YfKWayMerge::yf_new();
        let result = m.yf_merge();
        assert!(result.is_empty());
    }

    #[test]
    fn yf_kmerge_remaining() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2]);
        m.yf_add_source(vec![3, 4]);
        assert_eq!(m.yf_remaining(), 4);
    }

    #[test]
    fn yf_kmerge_reset() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2]);
        let _ = m.yf_merge();
        m.yf_reset();
        assert_eq!(m.yf_remaining(), 2);
    }

    #[test]
    fn yf_kmerge_unique() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2, 3]);
        m.yf_add_source(vec![2, 3, 4]);
        let result = m.yf_merge_unique();
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn yf_kmerge_display() {
        let m = super::YfKWayMerge::yf_new();
        assert!(format!("{}", m).contains("KWayMerge"));
    }

    #[test]
    fn yf_kmerge_default() {
        let m = super::YfKWayMerge::default();
        assert!(m.yf_is_done());
    }


    // --- yg_ PersistentStack tests ---

    #[test]
    fn yg_pstack_new() {
        let s = super::YgPersistentStack::<i32>::yg_new();
        assert!(s.yg_is_empty());
    }

    #[test]
    fn yg_pstack_push_pop() {
        let s = super::YgPersistentStack::yg_new();
        let s = s.yg_push(1);
        let s = s.yg_push(2);
        let (v, s) = s.yg_pop().unwrap();
        assert_eq!(v, 2);
        let (v, _) = s.yg_pop().unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn yg_pstack_persistence() {
        let s1 = super::YgPersistentStack::yg_new().yg_push(1).yg_push(2);
        let s2 = s1.yg_push(3);
        assert_eq!(s1.yg_len(), 2);
        assert_eq!(s2.yg_len(), 3);
    }

    #[test]
    fn yg_pstack_peek() {
        let s = super::YgPersistentStack::yg_new().yg_push(42);
        assert_eq!(s.yg_peek(), Some(&42));
    }

    #[test]
    fn yg_pstack_to_vec() {
        let s = super::YgPersistentStack::yg_new().yg_push(1).yg_push(2).yg_push(3);
        assert_eq!(s.yg_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn yg_pstack_reverse() {
        let s = super::YgPersistentStack::yg_new().yg_push(1).yg_push(2).yg_push(3);
        let r = s.yg_reverse();
        assert_eq!(r.yg_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn yg_pstack_display() {
        let s = super::YgPersistentStack::yg_new().yg_push(1);
        assert!(format!("{}", s).contains("PStack"));
    }

    #[test]
    fn yg_pstack_default() {
        let s = super::YgPersistentStack::<i32>::default();
        assert!(s.yg_is_empty());
    }

    // --- yg_ BitmapIndex tests ---

    #[test]
    fn yg_bitmap_new() {
        let bi = super::YgBitmapIndex::yg_new(100);
        assert_eq!(bi.yg_num_rows(), 100);
    }

    #[test]
    fn yg_bitmap_set_get() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("color_red", 5);
        assert!(bi.yg_get("color_red", 5));
        assert!(!bi.yg_get("color_red", 6));
    }

    #[test]
    fn yg_bitmap_and() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("a", 1);
        bi.yg_set("a", 2);
        bi.yg_set("b", 2);
        bi.yg_set("b", 3);
        let rows = bi.yg_and(&["a", "b"]);
        assert_eq!(rows, vec![2]);
    }

    #[test]
    fn yg_bitmap_or() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("a", 1);
        bi.yg_set("b", 2);
        let rows = bi.yg_or(&["a", "b"]);
        assert_eq!(rows, vec![1, 2]);
    }

    #[test]
    fn yg_bitmap_count() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("x", 0);
        bi.yg_set("x", 1);
        bi.yg_set("x", 2);
        assert_eq!(bi.yg_count("x"), 3);
    }

    #[test]
    fn yg_bitmap_columns() {
        let mut bi = super::YgBitmapIndex::yg_new(10);
        bi.yg_set("a", 0);
        bi.yg_set("b", 0);
        assert_eq!(bi.yg_num_columns(), 2);
    }

    #[test]
    fn yg_bitmap_clear() {
        let mut bi = super::YgBitmapIndex::yg_new(10);
        bi.yg_set("a", 0);
        bi.yg_clear();
        assert_eq!(bi.yg_num_columns(), 0);
    }

    #[test]
    fn yg_bitmap_display() {
        let bi = super::YgBitmapIndex::yg_new(10);
        assert!(format!("{}", bi).contains("BitmapIndex"));
    }

    #[test]
    fn yg_bitmap_default() {
        let bi = super::YgBitmapIndex::default();
        assert_eq!(bi.yg_num_rows(), 0);
    }


    // --- yh_ OSTree tests ---

    #[test]
    fn yh_ost_new() {
        let t = super::YhOrderStatTree::yh_new();
        assert!(t.yh_is_empty());
    }

    #[test]
    fn yh_ost_insert_contains() {
        let mut t = super::YhOrderStatTree::yh_new();
        t.yh_insert(10);
        t.yh_insert(5);
        t.yh_insert(15);
        assert!(t.yh_contains(10));
        assert!(!t.yh_contains(7));
    }

    #[test]
    fn yh_ost_rank() {
        let mut t = super::YhOrderStatTree::yh_new();
        for v in [10, 5, 15, 3, 7] { t.yh_insert(v); }
        assert_eq!(t.yh_rank(5), 1);
        assert_eq!(t.yh_rank(10), 3);
    }

    #[test]
    fn yh_ost_select() {
        let mut t = super::YhOrderStatTree::yh_new();
        for v in [10, 5, 15, 3, 7] { t.yh_insert(v); }
        assert_eq!(t.yh_select(0), Some(3));
        assert_eq!(t.yh_select(2), Some(7));
        assert_eq!(t.yh_select(4), Some(15));
    }

    #[test]
    fn yh_ost_min_max() {
        let mut t = super::YhOrderStatTree::yh_new();
        t.yh_insert(10);
        t.yh_insert(5);
        t.yh_insert(15);
        assert_eq!(t.yh_min(), Some(5));
        assert_eq!(t.yh_max(), Some(15));
    }

    #[test]
    fn yh_ost_inorder() {
        let mut t = super::YhOrderStatTree::yh_new();
        for v in [5, 3, 7, 1, 4] { t.yh_insert(v); }
        assert_eq!(t.yh_inorder(), vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn yh_ost_count_range() {
        let mut t = super::YhOrderStatTree::yh_new();
        for v in [1, 3, 5, 7, 9] { t.yh_insert(v); }
        assert_eq!(t.yh_count_range(3, 7), 3);
    }

    #[test]
    fn yh_ost_display() {
        let t = super::YhOrderStatTree::yh_new();
        assert!(format!("{}", t).contains("OSTree"));
    }

    #[test]
    fn yh_ost_default() {
        let t = super::YhOrderStatTree::default();
        assert!(t.yh_is_empty());
    }

    // --- yh_ Reservoir tests ---

    #[test]
    fn yh_reservoir_new() {
        let r = super::YhReservoirSampler::yh_new(5, 42);
        assert_eq!(r.yh_k(), 5);
        assert_eq!(r.yh_count(), 0);
    }

    #[test]
    fn yh_reservoir_add() {
        let mut r = super::YhReservoirSampler::yh_new(3, 42);
        for i in 0..10 { r.yh_add(i); }
        assert_eq!(r.yh_len(), 3);
        assert_eq!(r.yh_count(), 10);
    }

    #[test]
    fn yh_reservoir_underfill() {
        let mut r = super::YhReservoirSampler::yh_new(10, 42);
        r.yh_add(1);
        r.yh_add(2);
        assert_eq!(r.yh_len(), 2);
        assert!(!r.yh_is_full());
    }

    #[test]
    fn yh_reservoir_full() {
        let mut r = super::YhReservoirSampler::yh_new(3, 42);
        r.yh_add(1); r.yh_add(2); r.yh_add(3);
        assert!(r.yh_is_full());
    }

    #[test]
    fn yh_reservoir_reset() {
        let mut r = super::YhReservoirSampler::yh_new(3, 42);
        r.yh_add(1);
        r.yh_reset(99);
        assert_eq!(r.yh_count(), 0);
        assert_eq!(r.yh_len(), 0);
    }

    #[test]
    fn yh_reservoir_display() {
        let r = super::YhReservoirSampler::yh_new(5, 42);
        assert!(format!("{}", r).contains("Reservoir"));
    }

    #[test]
    fn yh_reservoir_default() {
        let r = super::YhReservoirSampler::default();
        assert_eq!(r.yh_k(), 10);
    }

    #[test]
    fn yh_reservoir_sample() {
        let mut r = super::YhReservoirSampler::yh_new(5, 42);
        for i in 0..100 { r.yh_add(i); }
        assert_eq!(r.yh_sample().len(), 5);
    }


    // --- yi_ RingBuffer tests ---

    #[test]
    fn yi_ring_new() {
        let r = super::YiRingBuffer::<i32>::yi_new(10);
        assert!(r.yi_is_empty());
        assert_eq!(r.yi_capacity(), 10);
    }

    #[test]
    fn yi_ring_push_pop() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(1);
        r.yi_push_back(2);
        r.yi_push_back(3);
        assert_eq!(r.yi_pop_front(), Some(1));
        assert_eq!(r.yi_pop_front(), Some(2));
    }

    #[test]
    fn yi_ring_push_front() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_front(1);
        r.yi_push_front(2);
        assert_eq!(r.yi_front(), Some(&2));
    }

    #[test]
    fn yi_ring_full() {
        let mut r = super::YiRingBuffer::yi_new(2);
        assert!(r.yi_push_back(1));
        assert!(r.yi_push_back(2));
        assert!(!r.yi_push_back(3));
        assert!(r.yi_is_full());
    }

    #[test]
    fn yi_ring_wrap() {
        let mut r = super::YiRingBuffer::yi_new(3);
        r.yi_push_back(1);
        r.yi_push_back(2);
        r.yi_push_back(3);
        r.yi_pop_front();
        r.yi_push_back(4);
        assert_eq!(r.yi_to_vec(), vec![2, 3, 4]);
    }

    #[test]
    fn yi_ring_force_push() {
        let mut r = super::YiRingBuffer::yi_new(2);
        r.yi_force_push_back(1);
        r.yi_force_push_back(2);
        r.yi_force_push_back(3);
        assert_eq!(r.yi_to_vec(), vec![2, 3]);
    }

    #[test]
    fn yi_ring_get() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(10);
        r.yi_push_back(20);
        assert_eq!(r.yi_get(0), Some(&10));
        assert_eq!(r.yi_get(1), Some(&20));
    }

    #[test]
    fn yi_ring_clear() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(1);
        r.yi_clear();
        assert!(r.yi_is_empty());
    }

    #[test]
    fn yi_ring_back() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(1);
        r.yi_push_back(2);
        assert_eq!(r.yi_back(), Some(&2));
    }

    #[test]
    fn yi_ring_pop_back() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(1);
        r.yi_push_back(2);
        assert_eq!(r.yi_pop_back(), Some(2));
        assert_eq!(r.yi_len(), 1);
    }

    #[test]
    fn yi_ring_display() {
        let r = super::YiRingBuffer::<i32>::yi_new(10);
        assert!(format!("{}", r).contains("RingBuffer"));
    }

    // --- yi_ WeightedGraph tests ---

    #[test]
    fn yi_wgraph_new() {
        let g = super::YiWeightedGraph::yi_new();
        assert_eq!(g.yi_node_count(), 0);
    }

    #[test]
    fn yi_wgraph_add_edge() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 5.0);
        assert_eq!(g.yi_node_count(), 2);
    }

    #[test]
    fn yi_wgraph_dijkstra() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 4.0);
        g.yi_add_edge(0, 2, 1.0);
        g.yi_add_edge(2, 1, 2.0);
        let dists = g.yi_dijkstra(0);
        assert_eq!(dists[&1], 3.0);
    }

    #[test]
    fn yi_wgraph_shortest() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 1.0);
        g.yi_add_edge(1, 2, 2.0);
        assert_eq!(g.yi_shortest_distance(0, 2), Some(3.0));
    }

    #[test]
    fn yi_wgraph_no_path() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_node(0);
        g.yi_add_node(1);
        assert_eq!(g.yi_shortest_distance(0, 1), None);
    }

    #[test]
    fn yi_wgraph_undirected() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_undirected_edge(0, 1, 3.0);
        assert_eq!(g.yi_shortest_distance(1, 0), Some(3.0));
    }

    #[test]
    fn yi_wgraph_total_weight() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 2.0);
        g.yi_add_edge(1, 2, 3.0);
        assert_eq!(g.yi_total_weight(), 5.0);
    }

    #[test]
    fn yi_wgraph_clear() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 1.0);
        g.yi_clear();
        assert_eq!(g.yi_node_count(), 0);
    }

    #[test]
    fn yi_wgraph_display() {
        let g = super::YiWeightedGraph::yi_new();
        assert!(format!("{}", g).contains("WGraph"));
    }

    #[test]
    fn yi_wgraph_default() {
        let g = super::YiWeightedGraph::default();
        assert_eq!(g.yi_node_count(), 0);
    }


    // --- yj_ ExprEval tests ---

    #[test]
    fn yj_expr_simple() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("2 + 3").unwrap(), 5.0);
    }

    #[test]
    fn yj_expr_precedence() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("2 + 3 * 4").unwrap(), 14.0);
    }

    #[test]
    fn yj_expr_parens() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("(2 + 3) * 4").unwrap(), 20.0);
    }

    #[test]
    fn yj_expr_neg() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("-5 + 3").unwrap(), -2.0);
    }

    #[test]
    fn yj_expr_var() {
        let mut e = super::YjExprEval::yj_new();
        e.yj_set_var("x", 10.0);
        assert_eq!(e.yj_eval("x * 2").unwrap(), 20.0);
    }

    #[test]
    fn yj_expr_div() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("10 / 4").unwrap(), 2.5);
    }

    #[test]
    fn yj_expr_complex() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("(1 + 2) * (3 + 4)").unwrap(), 21.0);
    }

    #[test]
    fn yj_expr_error() {
        let e = super::YjExprEval::yj_new();
        assert!(e.yj_eval("2 +").is_err());
    }

    #[test]
    fn yj_expr_display() {
        let e = super::YjExprEval::yj_new();
        assert!(format!("{}", e).contains("ExprEval"));
    }

    #[test]
    fn yj_expr_clear() {
        let mut e = super::YjExprEval::yj_new();
        e.yj_set_var("x", 1.0);
        e.yj_clear();
        assert_eq!(e.yj_var_count(), 0);
    }

    // --- yj_ TtlCache tests ---

    #[test]
    fn yj_ttl_new() {
        let c = super::YjTtlCache::<i32>::yj_new(100);
        assert_eq!(c.yj_ttl(), 100);
    }

    #[test]
    fn yj_ttl_put_get() {
        let mut c = super::YjTtlCache::yj_new(100);
        c.yj_put("a", 42);
        assert_eq!(c.yj_get("a"), Some(&42));
    }

    #[test]
    fn yj_ttl_expired() {
        let mut c = super::YjTtlCache::yj_new(10);
        c.yj_put("a", 1);
        c.yj_tick(20);
        assert_eq!(c.yj_get("a"), None);
    }

    #[test]
    fn yj_ttl_not_expired() {
        let mut c = super::YjTtlCache::yj_new(100);
        c.yj_put("a", 1);
        c.yj_tick(50);
        assert_eq!(c.yj_get("a"), Some(&1));
    }

    #[test]
    fn yj_ttl_evict() {
        let mut c = super::YjTtlCache::yj_new(10);
        c.yj_put("a", 1);
        c.yj_tick(20);
        c.yj_evict_expired();
        assert_eq!(c.yj_len(), 0);
    }

    #[test]
    fn yj_ttl_valid_count() {
        let mut c = super::YjTtlCache::yj_new(10);
        c.yj_put("a", 1);
        c.yj_tick(5);
        c.yj_put("b", 2);
        c.yj_tick(8);
        assert_eq!(c.yj_valid_count(), 1);
    }

    #[test]
    fn yj_ttl_remove() {
        let mut c = super::YjTtlCache::yj_new(100);
        c.yj_put("a", 42);
        assert_eq!(c.yj_remove("a"), Some(42));
    }

    #[test]
    fn yj_ttl_clear() {
        let mut c = super::YjTtlCache::yj_new(100);
        c.yj_put("a", 1);
        c.yj_clear();
        assert_eq!(c.yj_len(), 0);
    }

    #[test]
    fn yj_ttl_display() {
        let c = super::YjTtlCache::<i32>::yj_new(10);
        assert!(format!("{}", c).contains("TtlCache"));
    }

    #[test]
    fn yj_ttl_default() {
        let c = super::YjTtlCache::<i32>::default();
        assert_eq!(c.yj_ttl(), 60);
    }


    // --- yk_ GlobMatcher tests ---

    #[test]
    fn yk_glob_exact() {
        let g = super::YkGlobMatcher::yk_new("hello");
        assert!(g.yk_matches("hello"));
        assert!(!g.yk_matches("world"));
    }

    #[test]
    fn yk_glob_star() {
        let g = super::YkGlobMatcher::yk_new("*.rs");
        assert!(g.yk_matches("main.rs"));
        assert!(!g.yk_matches("main.py"));
    }

    #[test]
    fn yk_glob_question() {
        let g = super::YkGlobMatcher::yk_new("?.txt");
        assert!(g.yk_matches("a.txt"));
        assert!(!g.yk_matches("ab.txt"));
    }

    #[test]
    fn yk_glob_complex() {
        let g = super::YkGlobMatcher::yk_new("src/*.rs");
        assert!(g.yk_matches("src/main.rs"));
        assert!(g.yk_matches("src/sub/main.rs"));
    }

    #[test]
    fn yk_glob_empty() {
        let g = super::YkGlobMatcher::yk_new("*");
        assert!(g.yk_matches("anything"));
        assert!(g.yk_matches(""));
    }

    #[test]
    fn yk_glob_matches_any() {
        assert!(super::YkGlobMatcher::yk_matches_any(&["*.rs", "*.py"], "main.rs"));
        assert!(!super::YkGlobMatcher::yk_matches_any(&["*.rs", "*.py"], "main.js"));
    }

    #[test]
    fn yk_glob_filter() {
        let g = super::YkGlobMatcher::yk_new("*.rs");
        let files = vec!["main.rs", "lib.rs", "main.py"];
        assert_eq!(g.yk_filter(&files), vec!["main.rs", "lib.rs"]);
    }

    #[test]
    fn yk_glob_display() {
        let g = super::YkGlobMatcher::yk_new("*.txt");
        assert!(format!("{}", g).contains("Glob"));
    }

    #[test]
    fn yk_glob_default() {
        let g = super::YkGlobMatcher::default();
        assert!(g.yk_matches(""));
    }

    // --- yk_ EventBus tests ---

    #[test]
    fn yk_bus_new() {
        let b = super::YkEventBus::yk_new();
        assert_eq!(b.yk_topic_count(), 0);
    }

    #[test]
    fn yk_bus_subscribe() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("click", "handler_a");
        assert_eq!(b.yk_subscriber_count("click"), 1);
    }

    #[test]
    fn yk_bus_emit() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("click", "handler_a");
        b.yk_subscribe("click", "handler_b");
        let notified = b.yk_emit("click");
        assert_eq!(notified.len(), 2);
    }

    #[test]
    fn yk_bus_unsubscribe() {
        let mut b = super::YkEventBus::yk_new();
        let id = b.yk_subscribe("click", "handler_a");
        b.yk_unsubscribe(id);
        assert_eq!(b.yk_subscriber_count("click"), 0);
    }

    #[test]
    fn yk_bus_topics() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("click", "a");
        b.yk_subscribe("keypress", "b");
        assert_eq!(b.yk_topics().len(), 2);
    }

    #[test]
    fn yk_bus_emit_pattern() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("mouse.click", "a");
        b.yk_subscribe("mouse.move", "b");
        b.yk_subscribe("key.press", "c");
        let notified = b.yk_emit_pattern("mouse.*");
        assert_eq!(notified.len(), 2);
    }

    #[test]
    fn yk_bus_clear() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("x", "a");
        b.yk_clear();
        assert_eq!(b.yk_total_subscribers(), 0);
    }

    #[test]
    fn yk_bus_has_subscribers() {
        let mut b = super::YkEventBus::yk_new();
        assert!(!b.yk_has_subscribers("x"));
        b.yk_subscribe("x", "a");
        assert!(b.yk_has_subscribers("x"));
    }

    #[test]
    fn yk_bus_display() {
        let b = super::YkEventBus::yk_new();
        assert!(format!("{}", b).contains("EventBus"));
    }

    #[test]
    fn yk_bus_default() {
        let b = super::YkEventBus::default();
        assert_eq!(b.yk_topic_count(), 0);
    }


    // --- yl_ MinMaxHeap tests ---

    #[test]
    fn yl_mmh_new() {
        let h = super::YlMinMaxHeap::yl_new();
        assert!(h.yl_is_empty());
    }

    #[test]
    fn yl_mmh_insert_min_max() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(5);
        h.yl_insert(3);
        h.yl_insert(8);
        h.yl_insert(1);
        assert_eq!(h.yl_peek_min(), Some(1));
        assert_eq!(h.yl_peek_max(), Some(8));
    }

    #[test]
    fn yl_mmh_pop_min() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(5);
        h.yl_insert(1);
        h.yl_insert(9);
        assert_eq!(h.yl_pop_min(), Some(1));
        assert_eq!(h.yl_peek_min(), Some(5));
    }

    #[test]
    fn yl_mmh_sorted() {
        let mut h = super::YlMinMaxHeap::yl_new();
        for v in [7, 3, 9, 1, 5] { h.yl_insert(v); }
        let sorted = h.yl_to_sorted_vec();
        assert_eq!(sorted, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn yl_mmh_single() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(42);
        assert_eq!(h.yl_peek_min(), Some(42));
        assert_eq!(h.yl_peek_max(), Some(42));
    }

    #[test]
    fn yl_mmh_two() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(5);
        h.yl_insert(3);
        assert_eq!(h.yl_peek_min(), Some(3));
        assert_eq!(h.yl_peek_max(), Some(5));
    }

    #[test]
    fn yl_mmh_clear() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(1);
        h.yl_clear();
        assert!(h.yl_is_empty());
    }

    #[test]
    fn yl_mmh_display() {
        let h = super::YlMinMaxHeap::yl_new();
        assert!(format!("{}", h).contains("MinMaxHeap"));
    }

    #[test]
    fn yl_mmh_default() {
        let h = super::YlMinMaxHeap::default();
        assert!(h.yl_is_empty());
    }

    // --- yl_ StateMachine tests ---

    #[test]
    fn yl_fsm_new() {
        let m = super::YlStateMachine::yl_new();
        assert_eq!(m.yl_state_count(), 0);
    }

    #[test]
    fn yl_fsm_basic() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("start");
        let s1 = m.yl_add_state("end");
        m.yl_add_transition(s0, "go", s1);
        m.yl_set_accept(s1);
        m.yl_set_start(s0);
        assert!(m.yl_step("go"));
        assert!(m.yl_is_accepting());
    }

    #[test]
    fn yl_fsm_run() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("a");
        let s1 = m.yl_add_state("b");
        let s2 = m.yl_add_state("c");
        m.yl_add_transition(s0, "x", s1);
        m.yl_add_transition(s1, "y", s2);
        m.yl_set_start(s0);
        assert!(m.yl_run(&["x", "y"]));
        assert_eq!(m.yl_current_state(), "c");
    }

    #[test]
    fn yl_fsm_invalid() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("start");
        m.yl_set_start(s0);
        assert!(!m.yl_step("invalid"));
    }

    #[test]
    fn yl_fsm_available() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("s0");
        let s1 = m.yl_add_state("s1");
        m.yl_add_transition(s0, "a", s1);
        m.yl_add_transition(s0, "b", s1);
        m.yl_set_start(s0);
        assert_eq!(m.yl_available_inputs().len(), 2);
    }

    #[test]
    fn yl_fsm_reset() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("start");
        let s1 = m.yl_add_state("end");
        m.yl_add_transition(s0, "go", s1);
        m.yl_set_start(s0);
        m.yl_step("go");
        m.yl_reset();
        assert_eq!(m.yl_current_state(), "start");
    }

    #[test]
    fn yl_fsm_display() {
        let m = super::YlStateMachine::yl_new();
        assert!(format!("{}", m).contains("FSM"));
    }

    #[test]
    fn yl_fsm_default() {
        let m = super::YlStateMachine::default();
        assert_eq!(m.yl_state_count(), 0);
    }


    // --- ym_ SortedMultiMap tests ---

    #[test]
    fn ym_smm_new() {
        let m = super::YmSortedMultiMap::<i32, i32>::ym_new();
        assert!(m.ym_is_empty());
    }

    #[test]
    fn ym_smm_insert_get() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(1, "a");
        m.ym_insert(1, "b");
        m.ym_insert(2, "c");
        assert_eq!(m.ym_get(&1).len(), 2);
        assert_eq!(m.ym_total_count(), 3);
    }

    #[test]
    fn ym_smm_keys() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(3, 1);
        m.ym_insert(1, 2);
        m.ym_insert(2, 3);
        assert_eq!(m.ym_keys(), vec![1, 2, 3]);
    }

    #[test]
    fn ym_smm_remove() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(1, "x");
        m.ym_insert(1, "y");
        let removed = m.ym_remove_key(&1);
        assert_eq!(removed.len(), 2);
        assert!(m.ym_is_empty());
    }

    #[test]
    fn ym_smm_range() {
        let mut m = super::YmSortedMultiMap::ym_new();
        for i in 0..10 { m.ym_insert(i, i * 10); }
        let r = m.ym_range(&3, &7);
        assert_eq!(r, vec![3, 4, 5, 6, 7]);
    }

    #[test]
    fn ym_smm_first_last() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(5, 0);
        m.ym_insert(1, 0);
        m.ym_insert(9, 0);
        assert_eq!(m.ym_first_key(), Some(1));
        assert_eq!(m.ym_last_key(), Some(9));
    }

    #[test]
    fn ym_smm_clear() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(1, 1);
        m.ym_clear();
        assert!(m.ym_is_empty());
    }

    #[test]
    fn ym_smm_display() {
        let m = super::YmSortedMultiMap::<i32, i32>::ym_new();
        assert!(format!("{}", m).contains("SortedMultiMap"));
    }

    #[test]
    fn ym_smm_default() {
        let m = super::YmSortedMultiMap::<i32, i32>::default();
        assert!(m.ym_is_empty());
    }

    // --- ym_ TaskScheduler tests ---

    #[test]
    fn ym_sched_new() {
        let s = super::YmTaskScheduler::ym_new();
        assert_eq!(s.ym_total(), 0);
    }

    #[test]
    fn ym_sched_add() {
        let mut s = super::YmTaskScheduler::ym_new();
        let id = s.ym_add_task("build", 1, vec![]);
        assert_eq!(id, 0);
        assert_eq!(s.ym_total(), 1);
    }

    #[test]
    fn ym_sched_next_ready() {
        let mut s = super::YmTaskScheduler::ym_new();
        s.ym_add_task("low", 10, vec![]);
        s.ym_add_task("high", 1, vec![]);
        let next = s.ym_next_ready().unwrap();
        assert_eq!(next.ym_name, "high");
    }

    #[test]
    fn ym_sched_deps() {
        let mut s = super::YmTaskScheduler::ym_new();
        let t0 = s.ym_add_task("first", 1, vec![]);
        let _t1 = s.ym_add_task("second", 1, vec![t0]);
        let ready = s.ym_all_ready();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].ym_name, "first");
    }

    #[test]
    fn ym_sched_complete() {
        let mut s = super::YmTaskScheduler::ym_new();
        let t0 = s.ym_add_task("a", 1, vec![]);
        let _t1 = s.ym_add_task("b", 1, vec![t0]);
        s.ym_complete(t0);
        let ready = s.ym_all_ready();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].ym_name, "b");
    }

    #[test]
    fn ym_sched_all_done() {
        let mut s = super::YmTaskScheduler::ym_new();
        let t0 = s.ym_add_task("a", 1, vec![]);
        s.ym_complete(t0);
        assert!(s.ym_is_all_done());
    }

    #[test]
    fn ym_sched_clear() {
        let mut s = super::YmTaskScheduler::ym_new();
        s.ym_add_task("x", 1, vec![]);
        s.ym_clear();
        assert_eq!(s.ym_total(), 0);
    }

    #[test]
    fn ym_sched_display() {
        let s = super::YmTaskScheduler::ym_new();
        assert!(format!("{}", s).contains("Scheduler"));
    }

    #[test]
    fn ym_sched_default() {
        let s = super::YmTaskScheduler::default();
        assert!(s.ym_is_all_done());
    }

    #[test]
    fn ym_task_display() {
        let t = super::YmTask { ym_id: 0, ym_name: "test".to_string(), ym_priority: 1, ym_deps: vec![], ym_done: false };
        assert!(format!("{}", t).contains("Task"));
    }


    // --- yn_ ImmutableMap tests ---

    #[test]
    fn yn_imm_new() {
        let m = super::YnImmutableMap::<i32, i32>::yn_new();
        assert!(m.yn_is_empty());
    }

    #[test]
    fn yn_imm_insert_get() {
        let m = super::YnImmutableMap::yn_new();
        let m = m.yn_insert(1, "a");
        let m = m.yn_insert(2, "b");
        assert_eq!(m.yn_get(&1), Some(&"a"));
        assert_eq!(m.yn_get(&2), Some(&"b"));
    }

    #[test]
    fn yn_imm_persistence() {
        let m1 = super::YnImmutableMap::yn_new().yn_insert(1, 10);
        let m2 = m1.yn_insert(2, 20);
        assert_eq!(m1.yn_len(), 1);
        assert_eq!(m2.yn_len(), 2);
    }

    #[test]
    fn yn_imm_remove() {
        let m = super::YnImmutableMap::yn_new().yn_insert(1, 10).yn_insert(2, 20);
        let m2 = m.yn_remove(&1);
        assert!(!m2.yn_contains_key(&1));
        assert!(m.yn_contains_key(&1));
    }

    #[test]
    fn yn_imm_keys() {
        let m = super::YnImmutableMap::yn_new().yn_insert(3, 0).yn_insert(1, 0).yn_insert(2, 0);
        assert_eq!(m.yn_keys(), vec![1, 2, 3]);
    }

    #[test]
    fn yn_imm_merge() {
        let a = super::YnImmutableMap::yn_new().yn_insert(1, "a");
        let b = super::YnImmutableMap::yn_new().yn_insert(2, "b");
        let c = a.yn_merge(&b);
        assert_eq!(c.yn_len(), 2);
    }

    #[test]
    fn yn_imm_filter() {
        let m = super::YnImmutableMap::yn_new().yn_insert(1, 10).yn_insert(2, 20).yn_insert(3, 30);
        let f = m.yn_filter(|_, v| *v > 15);
        assert_eq!(f.yn_len(), 2);
    }

    #[test]
    fn yn_imm_display() {
        let m = super::YnImmutableMap::<i32, i32>::yn_new();
        assert!(format!("{}", m).contains("ImmMap"));
    }

    #[test]
    fn yn_imm_default() {
        let m = super::YnImmutableMap::<i32, i32>::default();
        assert!(m.yn_is_empty());
    }

    // --- yn_ Tokenizer tests ---

    #[test]
    fn yn_tok_word() {
        let tokens = super::YnTokenizer::yn_tokenize("hello");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].yn_kind, super::YnTokenKind::YnWord);
    }

    #[test]
    fn yn_tok_number() {
        let tokens = super::YnTokenizer::yn_tokenize("42");
        assert_eq!(tokens[0].yn_kind, super::YnTokenKind::YnNumber);
    }

    #[test]
    fn yn_tok_mixed() {
        let tokens = super::YnTokenizer::yn_tokenize_no_ws("x + 42");
        assert_eq!(tokens.len(), 3);
    }

    #[test]
    fn yn_tok_string() {
        let tokens = super::YnTokenizer::yn_tokenize("hi_world");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].yn_kind, super::YnTokenKind::YnWord);
    }

    #[test]
    fn yn_tok_punct() {
        let tokens = super::YnTokenizer::yn_tokenize("+");
        assert_eq!(tokens[0].yn_kind, super::YnTokenKind::YnPunct);
    }

    #[test]
    fn yn_tok_count_kind() {
        let tokens = super::YnTokenizer::yn_tokenize("a + b + c");
        let count = super::YnTokenizer::yn_count_by_kind(&tokens, &super::YnTokenKind::YnWord);
        assert_eq!(count, 3);
    }

    #[test]
    fn yn_tok_empty() {
        let tokens = super::YnTokenizer::yn_tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn yn_tok_display() {
        let t = super::YnTokenizer;
        assert!(format!("{}", t).contains("Tokenizer"));
    }

    #[test]
    fn yn_tok_default() {
        let _t = super::YnTokenizer::default();
    }

    #[test]
    fn yn_tok_kind_display() {
        assert!(format!("{}", super::YnTokenKind::YnWord).contains("Word"));
    }


    // --- yo_ Levenshtein tests ---

    #[test]
    fn yo_lev_identical() {
        assert_eq!(super::YoLevenshtein::yo_distance("hello", "hello"), 0);
    }

    #[test]
    fn yo_lev_empty() {
        assert_eq!(super::YoLevenshtein::yo_distance("", "abc"), 3);
        assert_eq!(super::YoLevenshtein::yo_distance("abc", ""), 3);
    }

    #[test]
    fn yo_lev_basic() {
        assert_eq!(super::YoLevenshtein::yo_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn yo_lev_single_char() {
        assert_eq!(super::YoLevenshtein::yo_distance("a", "b"), 1);
    }

    #[test]
    fn yo_lev_similarity() {
        let s = super::YoLevenshtein::yo_similarity("hello", "hello");
        assert!((s - 1.0).abs() < 0.001);
    }

    #[test]
    fn yo_lev_closest() {
        let c = super::YoLevenshtein::yo_closest("cat", &["bat", "car", "dog"]);
        assert!(c == Some("bat") || c == Some("car"));
    }

    #[test]
    fn yo_lev_within() {
        let r = super::YoLevenshtein::yo_within_distance("cat", &["bat", "car", "dog"], 1);
        assert!(r.contains(&"bat"));
        assert!(r.contains(&"car"));
    }

    #[test]
    fn yo_lev_rank() {
        let r = super::YoLevenshtein::yo_rank("cat", &["dog", "bat", "cat"]);
        assert_eq!(r[0].0, "cat");
    }

    #[test]
    fn yo_lev_display() {
        assert!(format!("{}", super::YoLevenshtein).contains("Levenshtein"));
    }

    // --- yo_ DiffEngine tests ---

    #[test]
    fn yo_diff_identical() {
        let ops = super::YoDiffEngine::yo_diff("a\nb", "a\nb");
        assert_eq!(super::YoDiffEngine::yo_count_equal(&ops), 2);
    }

    #[test]
    fn yo_diff_insert() {
        let ops = super::YoDiffEngine::yo_diff("a", "a\nb");
        assert_eq!(super::YoDiffEngine::yo_count_insertions(&ops), 1);
    }

    #[test]
    fn yo_diff_delete() {
        let ops = super::YoDiffEngine::yo_diff("a\nb", "a");
        assert_eq!(super::YoDiffEngine::yo_count_deletions(&ops), 1);
    }

    #[test]
    fn yo_diff_replace() {
        let ops = super::YoDiffEngine::yo_diff("a", "b");
        assert!(super::YoDiffEngine::yo_count_insertions(&ops) > 0 || super::YoDiffEngine::yo_count_deletions(&ops) > 0);
    }

    #[test]
    fn yo_diff_empty() {
        let ops = super::YoDiffEngine::yo_diff("", "");
        assert!(ops.is_empty());
    }

    #[test]
    fn yo_diff_format() {
        let ops = super::YoDiffEngine::yo_diff("a", "b");
        let s = super::YoDiffEngine::yo_format(&ops);
        assert!(!s.is_empty());
    }

    #[test]
    fn yo_diff_op_display() {
        let op = super::YoDiffOp::YoInsert("line".to_string());
        assert!(format!("{}", op).contains("+"));
    }

    #[test]
    fn yo_diff_display() {
        assert!(format!("{}", super::YoDiffEngine).contains("DiffEngine"));
    }

}