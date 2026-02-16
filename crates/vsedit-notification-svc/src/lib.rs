//! Notification model service.

use std::fmt;
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

/// Priority level for notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl NotificationPriority {
    /// Returns `true` if this priority is `Urgent`.
    pub fn is_urgent(&self) -> bool {
        matches!(self, NotificationPriority::Urgent)
    }
}

impl fmt::Display for NotificationPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            NotificationPriority::Low => "Low",
            NotificationPriority::Normal => "Normal",
            NotificationPriority::High => "High",
            NotificationPriority::Urgent => "Urgent",
        };
        write!(f, "{s}")
    }
}

/// A typed action callback with optional tooltip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationActionCallback {
    label: String,
    action_id: String,
    tooltip: Option<String>,
}

impl NotificationActionCallback {
    /// Create a new action callback.
    pub fn new(label: &str, action_id: &str) -> Self {
        Self {
            label: label.to_string(),
            action_id: action_id.to_string(),
            tooltip: None,
        }
    }

    /// Set a tooltip for this action.
    pub fn with_tooltip(mut self, tooltip: &str) -> Self {
        self.tooltip = Some(tooltip.to_string());
        self
    }

    /// Returns the label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the action id.
    pub fn action_id(&self) -> &str {
        &self.action_id
    }

    /// Returns the tooltip, if set.
    pub fn tooltip(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }
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
    pub priority: Option<NotificationPriority>,
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
            priority: None,
        });
        id
    }

    /// Returns true if notifications is empty.
    pub fn is_notifications_empty(&self) -> bool {
        self.notifications.is_empty()
    }

    /// Get the first notification, if any.
    pub fn first_notification(&self) -> Option<&Notification> {
        self.notifications.first()
    }

    /// Get the last notification, if any.
    pub fn last_notification(&self) -> Option<&Notification> {
        self.notifications.last()
    }

    /// Retain only notifications matching the predicate.
    pub fn retain_notifications(&mut self, f: impl Fn(&Notification) -> bool) {
        self.notifications.retain(|item| f(item));
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

/// Accumulated statistics for notification-svc operations.
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationSvcStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl NotificationSvcStats {
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
    pub fn merge(&mut self, other: &NotificationSvcStats) {
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

impl Default for NotificationSvcStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NotificationSvcStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NotificationSvcStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for notification-svc.
#[derive(Debug, Clone)]
pub struct NotificationSvcValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl NotificationSvcValidator {
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

impl Default for NotificationSvcValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Dedup & priority extensions
// ---------------------------------------------------------------------------

impl NotificationService {
    /// Remove duplicate notifications by message, keeping the first occurrence.
    /// Only considers non-dismissed notifications. Returns the number removed.
    pub fn dedup_by_message(&mut self) -> usize {
        let mut seen = std::collections::HashSet::new();
        let before = self.notifications.len();
        self.notifications.retain(|n| {
            if n.dismissed {
                return true; // keep dismissed as-is
            }
            seen.insert(n.message.clone())
        });
        before - self.notifications.len()
    }

    /// Check if a non-dismissed notification with the given message already exists.
    pub fn has_duplicate(&self, message: &str) -> bool {
        self.notifications
            .iter()
            .any(|n| !n.dismissed && n.message == message)
    }

    /// Create a notification with a specific priority.
    pub fn add_with_priority(
        &mut self,
        msg: &str,
        severity: NotificationSeverity,
        priority: NotificationPriority,
    ) -> u64 {
        let id = self.add(msg.to_string(), severity);
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.priority = Some(priority);
        }
        id
    }

    /// Get all non-dismissed notifications matching the given priority.
    pub fn get_by_priority(&self, priority: NotificationPriority) -> Vec<&Notification> {
        self.notifications
            .iter()
            .filter(|n| !n.dismissed && n.priority == Some(priority))
            .collect()
    }

    /// Returns the highest priority among active (non-dismissed) notifications.
    pub fn highest_priority_active(&self) -> Option<NotificationPriority> {
        self.notifications
            .iter()
            .filter(|n| !n.dismissed)
            .filter_map(|n| n.priority)
            .max()
    }
}

// ---------------------------------------------------------------------------
// Notification grouping
// ---------------------------------------------------------------------------

/// A group of similar notifications collapsed into a single display item.
#[derive(Debug, Clone)]
pub struct NotificationGroup {
    /// Representative notification (first in the group).
    pub representative: String,
    /// Severity (highest severity in the group).
    pub severity: NotificationSeverity,
    /// Count of collapsed notifications.
    pub count: usize,
    /// Source, if all notifications share the same source.
    pub source: Option<String>,
}

impl NotificationGroup {
    pub fn new(message: impl Into<String>, severity: NotificationSeverity) -> Self {
        Self {
            representative: message.into(),
            severity,
            count: 1,
            source: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Summary string like "File not found (x3)".
    pub fn summary(&self) -> String {
        if self.count > 1 {
            format!("{} (x{})", self.representative, self.count)
        } else {
            self.representative.clone()
        }
    }
}

impl fmt::Display for NotificationGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

/// Group notifications by their message text, collapsing duplicates.
///
/// Returns groups sorted by count (most frequent first).
pub fn notification_group(notifications: &[Notification]) -> Vec<NotificationGroup> {
    use std::collections::HashMap;
    let mut map: HashMap<&str, NotificationGroup> = HashMap::new();

    for n in notifications {
        let entry = map.entry(n.message.as_str()).or_insert_with(|| {
            let mut g = NotificationGroup::new(&n.message, n.severity);
            g.source = n.source.clone();
            g.count = 0;
            g
        });
        entry.count += 1;
        // Escalate severity: Error > Warning > Info
        if n.severity as u8 > entry.severity as u8 {
            entry.severity = n.severity;
        }
    }

    let mut groups: Vec<NotificationGroup> = map.into_values().collect();
    groups.sort_by(|a, b| b.count.cmp(&a.count));
    groups
}

/// Group notifications by source instead of message.
pub fn notification_group_by_source(notifications: &[Notification]) -> Vec<NotificationGroup> {
    use std::collections::HashMap;
    let mut map: HashMap<String, NotificationGroup> = HashMap::new();

    for n in notifications {
        let key = n.source.as_deref().unwrap_or("(unknown)").to_string();
        let entry = map.entry(key.clone()).or_insert_with(|| {
            NotificationGroup::new(&key, n.severity).with_source(key.clone())
        });
        entry.count += 1;
        if n.severity as u8 > entry.severity as u8 {
            entry.severity = n.severity;
        }
    }

    let mut groups: Vec<NotificationGroup> = map.into_values().collect();
    groups.sort_by(|a, b| b.count.cmp(&a.count));
    groups
}

// ---------------------------------------------------------------------------
// Additional NotificationService methods
// ---------------------------------------------------------------------------

impl NotificationService {
    /// Returns the number of error notifications (including dismissed).
    pub fn error_count(&self) -> usize {
        self.notifications
            .iter()
            .filter(|n| n.severity == NotificationSeverity::Error)
            .count()
    }

    /// Returns the number of warning notifications (including dismissed).
    pub fn warning_count(&self) -> usize {
        self.notifications
            .iter()
            .filter(|n| n.severity == NotificationSeverity::Warning)
            .count()
    }

    /// Returns the number of info notifications (including dismissed).
    pub fn info_count(&self) -> usize {
        self.notifications
            .iter()
            .filter(|n| n.severity == NotificationSeverity::Info)
            .count()
    }

    /// Find the first notification whose message contains the given substring.
    pub fn find_by_message(&self, msg: &str) -> Option<&Notification> {
        self.notifications.iter().find(|n| n.message.contains(msg))
    }
}

// ---------------------------------------------------------------------------
// Display for NotificationService
// ---------------------------------------------------------------------------

impl fmt::Display for NotificationService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let active = self.notifications.iter().filter(|n| !n.dismissed).count();
        write!(
            f,
            "NotificationService(total={}, active={})",
            self.notifications.len(),
            active
        )
    }
}

// ---------------------------------------------------------------------------
// Display for Notification
// ---------------------------------------------------------------------------

impl fmt::Display for Notification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            NotificationSeverity::Info => "INFO",
            NotificationSeverity::Warning => "WARN",
            NotificationSeverity::Error => "ERROR",
        };
        write!(f, "[{}] {}", sev, self.message)
    }
}

// ---------------------------------------------------------------------------
// Additional NotificationProgress methods
// ---------------------------------------------------------------------------

impl NotificationProgress {
    /// Returns the completion percentage as a value between 0.0 and 100.0.
    /// Returns 100.0 when `total` is 0 to avoid division by zero.
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            return 100.0;
        }
        (self.worked as f64 / self.total as f64) * 100.0
    }
}

// ---------------------------------------------------------------------------
// Notification age helper
// ---------------------------------------------------------------------------

impl Notification {
    /// Compute the age in seconds given the current time and the notification id
    /// as a proxy timestamp. Returns `now - id` (saturating).
    pub fn age_seconds(&self, now: u64) -> u64 {
        now.saturating_sub(self.id)
    }
}

// ---------------------------------------------------------------------------
// NotificationFilter – builder-pattern predicate combinator
// ---------------------------------------------------------------------------

/// A composable filter for selecting notifications matching multiple criteria.
///
/// Criteria are combined with logical AND: a notification must satisfy every
/// configured predicate to pass the filter.
#[derive(Debug, Clone, Default)]
pub struct NotificationFilter {
    severities: Option<Vec<NotificationSeverity>>,
    priorities: Option<Vec<NotificationPriority>>,
    source: Option<String>,
    dismissed: Option<bool>,
    sticky: Option<bool>,
    message_contains: Option<String>,
}

impl NotificationFilter {
    /// Create an empty filter that matches everything.
    pub fn new() -> Self {
        Self::default()
    }

    /// Only match notifications with one of the given severities.
    pub fn severity(mut self, severities: &[NotificationSeverity]) -> Self {
        self.severities = Some(severities.to_vec());
        self
    }

    /// Only match notifications with one of the given priorities.
    pub fn priority(mut self, priorities: &[NotificationPriority]) -> Self {
        self.priorities = Some(priorities.to_vec());
        self
    }

    /// Only match notifications whose source equals `source`.
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Only match notifications with the given dismissed state.
    pub fn dismissed(mut self, dismissed: bool) -> Self {
        self.dismissed = Some(dismissed);
        self
    }

    /// Only match notifications with the given sticky state.
    pub fn sticky(mut self, sticky: bool) -> Self {
        self.sticky = Some(sticky);
        self
    }

    /// Only match notifications whose message contains `substring`.
    pub fn message_contains(mut self, substring: impl Into<String>) -> Self {
        self.message_contains = Some(substring.into());
        self
    }

    /// Test whether a single notification matches this filter.
    pub fn matches(&self, n: &Notification) -> bool {
        if let Some(ref sevs) = self.severities {
            if !sevs.contains(&n.severity) {
                return false;
            }
        }
        if let Some(ref pris) = self.priorities {
            let matched = match n.priority {
                Some(p) => pris.contains(&p),
                None => false,
            };
            if !matched {
                return false;
            }
        }
        if let Some(ref src) = self.source {
            if n.source.as_deref() != Some(src.as_str()) {
                return false;
            }
        }
        if let Some(d) = self.dismissed {
            if n.dismissed != d {
                return false;
            }
        }
        if let Some(s) = self.sticky {
            if n.sticky != s {
                return false;
            }
        }
        if let Some(ref sub) = self.message_contains {
            if !n.message.contains(sub.as_str()) {
                return false;
            }
        }
        true
    }

    /// Apply this filter to a slice and return matching notifications.
    pub fn apply<'a>(&self, notifications: &'a [Notification]) -> Vec<&'a Notification> {
        notifications.iter().filter(|n| self.matches(n)).collect()
    }
}

impl fmt::Display for NotificationFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(ref sevs) = self.severities {
            parts.push(format!("severity={sevs:?}"));
        }
        if let Some(ref pris) = self.priorities {
            parts.push(format!("priority={pris:?}"));
        }
        if let Some(ref src) = self.source {
            parts.push(format!("source={src}"));
        }
        if let Some(d) = self.dismissed {
            parts.push(format!("dismissed={d}"));
        }
        if let Some(s) = self.sticky {
            parts.push(format!("sticky={s}"));
        }
        if let Some(ref sub) = self.message_contains {
            parts.push(format!("message_contains={sub}"));
        }
        if parts.is_empty() {
            write!(f, "NotificationFilter(match_all)")
        } else {
            write!(f, "NotificationFilter({})", parts.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------
// NotificationSorter – configurable sorting
// ---------------------------------------------------------------------------

/// Sort criterion for notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSortKey {
    /// Sort by priority (notifications without a priority sort last).
    Priority,
    /// Sort by age (using id as a proxy timestamp – lower id = older).
    Age,
    /// Sort by severity (Error > Warning > Info).
    Severity,
}

/// Configurable sorter that applies one or more sort keys in sequence.
///
/// When multiple keys are specified the first key is the primary sort;
/// ties are broken by subsequent keys.
#[derive(Debug, Clone)]
pub struct NotificationSorter {
    keys: Vec<(NotificationSortKey, bool)>, // (key, ascending)
}

impl NotificationSorter {
    /// Create a sorter with no keys (preserves original order).
    pub fn new() -> Self {
        Self { keys: Vec::new() }
    }

    /// Append a sort key in ascending order.
    pub fn asc(mut self, key: NotificationSortKey) -> Self {
        self.keys.push((key, true));
        self
    }

    /// Append a sort key in descending order.
    pub fn desc(mut self, key: NotificationSortKey) -> Self {
        self.keys.push((key, false));
        self
    }

    /// Sort a mutable slice of notifications in place.
    pub fn sort(&self, notifications: &mut [Notification]) {
        notifications.sort_by(|a, b| {
            for &(key, ascending) in &self.keys {
                let ord = match key {
                    NotificationSortKey::Priority => {
                        let pa = a.priority.unwrap_or(NotificationPriority::Low);
                        let pb = b.priority.unwrap_or(NotificationPriority::Low);
                        pa.cmp(&pb)
                    }
                    NotificationSortKey::Age => {
                        // Lower id = older notification.
                        a.id.cmp(&b.id)
                    }
                    NotificationSortKey::Severity => {
                        (a.severity as u8).cmp(&(b.severity as u8))
                    }
                };
                let ord = if ascending { ord } else { ord.reverse() };
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            std::cmp::Ordering::Equal
        });
    }

    /// Return a sorted copy, leaving the original untouched.
    pub fn sorted(&self, notifications: &[Notification]) -> Vec<Notification> {
        let mut v = notifications.to_vec();
        self.sort(&mut v);
        v
    }
}

impl Default for NotificationSorter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// NotificationBatch – group related notifications
// ---------------------------------------------------------------------------

/// A batch of related notifications that can be operated on as a unit.
#[derive(Debug, Clone)]
pub struct NotificationBatch {
    label: String,
    items: Vec<Notification>,
}

impl NotificationBatch {
    /// Create a new empty batch with the given label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            items: Vec::new(),
        }
    }

    /// Add a notification to the batch.
    pub fn push(&mut self, notification: Notification) {
        self.items.push(notification);
    }

    /// Number of notifications in the batch.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return the batch label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Return a reference to all items.
    pub fn items(&self) -> &[Notification] {
        &self.items
    }

    /// Merge all notification messages into a single newline-separated string.
    pub fn merge_messages(&self) -> String {
        self.items
            .iter()
            .map(|n| n.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Return the highest severity present in the batch, or `None` if empty.
    pub fn highest_severity(&self) -> Option<NotificationSeverity> {
        self.items
            .iter()
            .map(|n| n.severity as u8)
            .max()
            .map(|v| match v {
                0 => NotificationSeverity::Info,
                1 => NotificationSeverity::Warning,
                _ => NotificationSeverity::Error,
            })
    }

    /// Mark every notification in the batch as dismissed.
    pub fn dismiss_all(&mut self) {
        for n in &mut self.items {
            n.dismissed = true;
        }
    }

    /// Return only the non-dismissed notifications.
    pub fn active(&self) -> Vec<&Notification> {
        self.items.iter().filter(|n| !n.dismissed).collect()
    }

    /// Return the highest priority in the batch, or `None` if empty / no priorities set.
    pub fn highest_priority(&self) -> Option<NotificationPriority> {
        self.items.iter().filter_map(|n| n.priority).max()
    }

    /// Iterate over the notifications in the batch.
    pub fn iter(&self) -> std::slice::Iter<'_, Notification> {
        self.items.iter()
    }
}

impl fmt::Display for NotificationBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NotificationBatch(\"{}\", count={})",
            self.label,
            self.items.len()
        )
    }
}

impl<'a> IntoIterator for &'a NotificationBatch {
    type Item = &'a Notification;
    type IntoIter = std::slice::Iter<'a, Notification>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl IntoIterator for NotificationBatch {
    type Item = Notification;
    type IntoIter = std::vec::IntoIter<Notification>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl FromIterator<Notification> for NotificationBatch {
    fn from_iter<T: IntoIterator<Item = Notification>>(iter: T) -> Self {
        let mut batch = NotificationBatch::new("collected");
        for n in iter {
            batch.push(n);
        }
        batch
    }
}

// ---------------------------------------------------------------------------
// Display & From impls for NotificationSeverity
// ---------------------------------------------------------------------------

impl fmt::Display for NotificationSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            NotificationSeverity::Info => "Info",
            NotificationSeverity::Warning => "Warning",
            NotificationSeverity::Error => "Error",
        };
        write!(f, "{s}")
    }
}

impl From<NotificationSeverity> for u8 {
    fn from(s: NotificationSeverity) -> Self {
        match s {
            NotificationSeverity::Info => 0,
            NotificationSeverity::Warning => 1,
            NotificationSeverity::Error => 2,
        }
    }
}

// ---------------------------------------------------------------------------
// NotificationService integration with filter/sorter
// ---------------------------------------------------------------------------

impl NotificationService {
    /// Return notifications matching `filter`.
    pub fn query(&self, filter: &NotificationFilter) -> Vec<&Notification> {
        filter.apply(&self.notifications)
    }
}

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
            priority: None,
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

    #[test]
    fn eq_notificationseverity_same() {
        assert_eq!(NotificationSeverity::Info, NotificationSeverity::Info);
    }

    #[test]
    fn ne_notificationseverity_diff() {
        assert_ne!(NotificationSeverity::Info, NotificationSeverity::Warning);
    }

    #[test]
    fn priority_ordering() {
        assert!(NotificationPriority::Urgent > NotificationPriority::High);
        assert!(NotificationPriority::High > NotificationPriority::Normal);
        assert!(NotificationPriority::Normal > NotificationPriority::Low);
    }

    #[test]
    fn priority_is_urgent() {
        assert!(NotificationPriority::Urgent.is_urgent());
        assert!(!NotificationPriority::High.is_urgent());
        assert!(!NotificationPriority::Normal.is_urgent());
        assert!(!NotificationPriority::Low.is_urgent());
    }

    #[test]
    fn priority_display() {
        assert_eq!(NotificationPriority::Low.to_string(), "Low");
        assert_eq!(NotificationPriority::Normal.to_string(), "Normal");
        assert_eq!(NotificationPriority::High.to_string(), "High");
        assert_eq!(NotificationPriority::Urgent.to_string(), "Urgent");
    }

    #[test]
    fn dedup_by_message() {
        let mut svc = NotificationService::new();
        svc.info("hello");
        svc.info("hello");
        svc.info("world");
        svc.info("hello");
        assert_eq!(svc.notification_count(), 4);
        let removed = svc.dedup_by_message();
        assert_eq!(removed, 2);
        assert_eq!(svc.notification_count(), 2);
        assert_eq!(svc.get_active()[0].message, "hello");
        assert_eq!(svc.get_active()[1].message, "world");
    }

    #[test]
    fn has_duplicate() {
        let mut svc = NotificationService::new();
        svc.info("test msg");
        assert!(svc.has_duplicate("test msg"));
        assert!(!svc.has_duplicate("other msg"));
        let id = svc.get_active()[0].id;
        svc.dismiss(id);
        assert!(!svc.has_duplicate("test msg")); // dismissed doesn't count
    }

    #[test]
    fn action_callback_builder() {
        let cb = NotificationActionCallback::new("Install", "install_action")
            .with_tooltip("Click to install");
        assert_eq!(cb.label(), "Install");
        assert_eq!(cb.action_id(), "install_action");
        assert_eq!(cb.tooltip(), Some("Click to install"));

        let cb2 = NotificationActionCallback::new("Cancel", "cancel");
        assert!(cb2.tooltip().is_none());
    }

    #[test]
    fn add_with_priority() {
        let mut svc = NotificationService::new();
        svc.add_with_priority("urgent!", NotificationSeverity::Error, NotificationPriority::Urgent);
        svc.add_with_priority("low priority", NotificationSeverity::Info, NotificationPriority::Low);
        svc.info("no priority");
        let urgent = svc.get_by_priority(NotificationPriority::Urgent);
        assert_eq!(urgent.len(), 1);
        assert_eq!(urgent[0].message, "urgent!");
    }

    #[test]
    fn get_by_priority_filters() {
        let mut svc = NotificationService::new();
        svc.add_with_priority("a", NotificationSeverity::Info, NotificationPriority::High);
        svc.add_with_priority("b", NotificationSeverity::Info, NotificationPriority::Low);
        svc.add_with_priority("c", NotificationSeverity::Info, NotificationPriority::High);
        assert_eq!(svc.get_by_priority(NotificationPriority::High).len(), 2);
        assert_eq!(svc.get_by_priority(NotificationPriority::Urgent).len(), 0);
    }

    #[test]
    fn highest_priority_active() {
        let mut svc = NotificationService::new();
        assert!(svc.highest_priority_active().is_none());

        svc.add_with_priority("low", NotificationSeverity::Info, NotificationPriority::Low);
        assert_eq!(svc.highest_priority_active(), Some(NotificationPriority::Low));

        svc.add_with_priority("high", NotificationSeverity::Warning, NotificationPriority::High);
        assert_eq!(svc.highest_priority_active(), Some(NotificationPriority::High));

        svc.add_with_priority("urgent", NotificationSeverity::Error, NotificationPriority::Urgent);
        assert_eq!(svc.highest_priority_active(), Some(NotificationPriority::Urgent));

        // Dismiss the urgent one
        let urgent_id = svc.get_by_priority(NotificationPriority::Urgent)[0].id;
        svc.dismiss(urgent_id);
        assert_eq!(svc.highest_priority_active(), Some(NotificationPriority::High));
    }

    #[test]
    fn notification_has_priority_field() {
        let mut svc = NotificationService::new();
        svc.info("plain");
        let n = svc.first_notification().unwrap();
        assert_eq!(n.priority, None);
    }

    #[test]
    fn notification_svc_stats_new_defaults() {
        let stats = NotificationSvcStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn notification_svc_stats_record_success() {
        let mut stats = NotificationSvcStats::new();
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
    fn notification_svc_stats_record_failure() {
        let mut stats = NotificationSvcStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn notification_svc_stats_reset() {
        let mut stats = NotificationSvcStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn notification_svc_stats_merge() {
        let mut a = NotificationSvcStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = NotificationSvcStats::new();
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
    fn notification_svc_stats_display() {
        let mut stats = NotificationSvcStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn notification_svc_stats_default() {
        let stats = NotificationSvcStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn notification_svc_validator_accepts_valid_name() {
        let v = NotificationSvcValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn notification_svc_validator_rejects_empty() {
        let v = NotificationSvcValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn notification_svc_validator_rejects_too_long() {
        let v = NotificationSvcValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn notification_svc_validator_forbidden_prefix() {
        let v = NotificationSvcValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn notification_svc_validator_allowed_chars() {
        let v = NotificationSvcValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn notification_svc_validator_range() {
        let v = NotificationSvcValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn notification_svc_sanitize_removes_control() {
        let result = NotificationSvcValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn notification_svc_truncate_short_string() {
        assert_eq!(NotificationSvcValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn notification_svc_truncate_long_string() {
        let result = NotificationSvcValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn notification_svc_is_ascii_printable() {
        assert!(NotificationSvcValidator::is_ascii_printable("Hello World 123"));
        assert!(!NotificationSvcValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- notification_group --

    #[test]
    fn group_collapses_duplicates() {
        let notifications = vec![
            Notification { id: 1, message: "File not found".into(), severity: NotificationSeverity::Error, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 2, message: "File not found".into(), severity: NotificationSeverity::Error, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 3, message: "Save complete".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
        ];
        let groups = notification_group(&notifications);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].count, 2);
        assert_eq!(groups[0].representative, "File not found");
    }

    #[test]
    fn group_summary_single() {
        let g = NotificationGroup::new("hello", NotificationSeverity::Info);
        assert_eq!(g.summary(), "hello");
    }

    #[test]
    fn group_summary_multiple() {
        let mut g = NotificationGroup::new("err", NotificationSeverity::Error);
        g.count = 5;
        assert_eq!(g.summary(), "err (x5)");
    }

    #[test]
    fn group_escalates_severity() {
        let notifications = vec![
            Notification { id: 1, message: "problem".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 2, message: "problem".into(), severity: NotificationSeverity::Error, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
        ];
        let groups = notification_group(&notifications);
        assert_eq!(groups[0].severity, NotificationSeverity::Error);
    }

    #[test]
    fn group_empty_input() {
        let groups = notification_group(&[]);
        assert!(groups.is_empty());
    }

    #[test]
    fn group_display() {
        let mut g = NotificationGroup::new("test", NotificationSeverity::Warning);
        g.count = 3;
        assert_eq!(format!("{g}"), "test (x3)");
    }

    #[test]
    fn error_warning_info_counts() {
        let mut svc = NotificationService::new();
        svc.info("i1");
        svc.info("i2");
        svc.warn("w1");
        svc.error("e1");
        svc.error("e2");
        svc.error("e3");
        assert_eq!(svc.info_count(), 2);
        assert_eq!(svc.warning_count(), 1);
        assert_eq!(svc.error_count(), 3);
    }

    #[test]
    fn find_by_message_found() {
        let mut svc = NotificationService::new();
        svc.info("file not found");
        svc.warn("disk full");
        let n = svc.find_by_message("not found");
        assert!(n.is_some());
        assert_eq!(n.unwrap().severity, NotificationSeverity::Info);
    }

    #[test]
    fn find_by_message_not_found() {
        let mut svc = NotificationService::new();
        svc.info("hello");
        assert!(svc.find_by_message("missing").is_none());
    }

    #[test]
    fn notification_service_display() {
        let mut svc = NotificationService::new();
        svc.info("a");
        svc.warn("b");
        let id = svc.error("c");
        svc.dismiss(id);
        let s = format!("{svc}");
        assert!(s.contains("total=3"));
        assert!(s.contains("active=2"));
    }

    #[test]
    fn notification_display() {
        let n = Notification {
            id: 1, message: "boom".into(),
            severity: NotificationSeverity::Error,
            source: None, actions: Vec::new(),
            sticky: false, dismissed: false, priority: None,
        };
        assert_eq!(format!("{n}"), "[ERROR] boom");
    }

    #[test]
    fn progress_percentage() {
        let mut p = NotificationProgress::new(200);
        assert!((p.percentage() - 0.0).abs() < f64::EPSILON);
        p.worked = 50;
        assert!((p.percentage() - 25.0).abs() < f64::EPSILON);
        p.worked = 200;
        assert!((p.percentage() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_percentage_zero_total() {
        let p = NotificationProgress::new(0);
        assert!((p.percentage() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn notification_age_seconds() {
        let n = Notification {
            id: 10, message: "x".into(),
            severity: NotificationSeverity::Info,
            source: None, actions: Vec::new(),
            sticky: false, dismissed: false, priority: None,
        };
        assert_eq!(n.age_seconds(15), 5);
        assert_eq!(n.age_seconds(5), 0);
    }

    // -- NotificationFilter tests --

    #[test]
    fn filter_by_severity_and_dismissed() {
        let notifications = vec![
            Notification { id: 1, message: "a".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 2, message: "b".into(), severity: NotificationSeverity::Error, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 3, message: "c".into(), severity: NotificationSeverity::Error, source: None, actions: vec![], sticky: false, dismissed: true, priority: None },
        ];

        let filter = NotificationFilter::new()
            .severity(&[NotificationSeverity::Error])
            .dismissed(false);

        let result = filter.apply(&notifications);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message, "b");
    }

    #[test]
    fn filter_by_source_and_message() {
        let notifications = vec![
            Notification { id: 1, message: "lint: unused var".into(), severity: NotificationSeverity::Warning, source: Some("linter".into()), actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 2, message: "lint: missing semi".into(), severity: NotificationSeverity::Warning, source: Some("linter".into()), actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 3, message: "build failed".into(), severity: NotificationSeverity::Error, source: Some("builder".into()), actions: vec![], sticky: false, dismissed: false, priority: None },
        ];

        let filter = NotificationFilter::new()
            .source("linter")
            .message_contains("unused");

        let result = filter.apply(&notifications);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 1);
    }

    #[test]
    fn filter_empty_matches_all() {
        let notifications = vec![
            Notification { id: 1, message: "a".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 2, message: "b".into(), severity: NotificationSeverity::Error, source: None, actions: vec![], sticky: false, dismissed: true, priority: None },
        ];
        let filter = NotificationFilter::new();
        assert_eq!(filter.apply(&notifications).len(), 2);
    }

    #[test]
    fn filter_display() {
        let f = NotificationFilter::new()
            .severity(&[NotificationSeverity::Error])
            .dismissed(false);
        let s = format!("{f}");
        assert!(s.contains("severity="));
        assert!(s.contains("dismissed=false"));

        let empty = NotificationFilter::new();
        assert_eq!(format!("{empty}"), "NotificationFilter(match_all)");
    }

    // -- NotificationSorter tests --

    #[test]
    fn sorter_by_severity_desc() {
        let mut notifications = vec![
            Notification { id: 1, message: "info".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 2, message: "error".into(), severity: NotificationSeverity::Error, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 3, message: "warn".into(), severity: NotificationSeverity::Warning, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
        ];
        let sorter = NotificationSorter::new().desc(NotificationSortKey::Severity);
        sorter.sort(&mut notifications);
        assert_eq!(notifications[0].severity, NotificationSeverity::Error);
        assert_eq!(notifications[1].severity, NotificationSeverity::Warning);
        assert_eq!(notifications[2].severity, NotificationSeverity::Info);
    }

    #[test]
    fn sorter_by_priority_asc_then_age() {
        let notifications = vec![
            Notification { id: 3, message: "c".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: Some(NotificationPriority::High) },
            Notification { id: 1, message: "a".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: Some(NotificationPriority::High) },
            Notification { id: 2, message: "b".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: Some(NotificationPriority::Low) },
        ];
        let sorter = NotificationSorter::new()
            .asc(NotificationSortKey::Priority)
            .asc(NotificationSortKey::Age);
        let sorted = sorter.sorted(&notifications);
        // Low priority first, then High sorted by id (age)
        assert_eq!(sorted[0].message, "b"); // Low, id=2
        assert_eq!(sorted[1].message, "a"); // High, id=1
        assert_eq!(sorted[2].message, "c"); // High, id=3
    }

    // -- NotificationBatch tests --

    #[test]
    fn batch_merge_and_severity() {
        let mut batch = NotificationBatch::new("build errors");
        batch.push(Notification { id: 1, message: "error in foo.rs".into(), severity: NotificationSeverity::Warning, source: None, actions: vec![], sticky: false, dismissed: false, priority: None });
        batch.push(Notification { id: 2, message: "error in bar.rs".into(), severity: NotificationSeverity::Error, source: None, actions: vec![], sticky: false, dismissed: false, priority: None });

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
        assert_eq!(batch.label(), "build errors");
        assert_eq!(batch.merge_messages(), "error in foo.rs\nerror in bar.rs");
        assert_eq!(batch.highest_severity(), Some(NotificationSeverity::Error));
    }

    #[test]
    fn batch_dismiss_all_and_active() {
        let mut batch = NotificationBatch::new("test");
        batch.push(Notification { id: 1, message: "a".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: None });
        batch.push(Notification { id: 2, message: "b".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: None });

        assert_eq!(batch.active().len(), 2);
        batch.dismiss_all();
        assert_eq!(batch.active().len(), 0);
        assert_eq!(batch.len(), 2); // still present, just dismissed
    }

    #[test]
    fn batch_from_iterator_and_into_iter() {
        let items = vec![
            Notification { id: 1, message: "x".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 2, message: "y".into(), severity: NotificationSeverity::Warning, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
        ];

        let batch: NotificationBatch = items.into_iter().collect();
        assert_eq!(batch.len(), 2);

        let messages: Vec<String> = batch.into_iter().map(|n| n.message).collect();
        assert_eq!(messages, vec!["x", "y"]);
    }

    #[test]
    fn batch_display() {
        let mut batch = NotificationBatch::new("deploys");
        batch.push(Notification { id: 1, message: "a".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: None });
        let s = format!("{batch}");
        assert!(s.contains("deploys"));
        assert!(s.contains("count=1"));
    }

    #[test]
    fn batch_empty() {
        let batch = NotificationBatch::new("empty");
        assert!(batch.is_empty());
        assert_eq!(batch.highest_severity(), None);
        assert_eq!(batch.highest_priority(), None);
        assert_eq!(batch.merge_messages(), "");
    }

    // -- Severity Display & From --

    #[test]
    fn severity_display() {
        assert_eq!(NotificationSeverity::Info.to_string(), "Info");
        assert_eq!(NotificationSeverity::Warning.to_string(), "Warning");
        assert_eq!(NotificationSeverity::Error.to_string(), "Error");
    }

    #[test]
    fn severity_into_u8() {
        let v: u8 = NotificationSeverity::Info.into();
        assert_eq!(v, 0);
        let v: u8 = NotificationSeverity::Warning.into();
        assert_eq!(v, 1);
        let v: u8 = NotificationSeverity::Error.into();
        assert_eq!(v, 2);
    }

    // -- NotificationService::query --

    #[test]
    fn service_query_integration() {
        let mut svc = NotificationService::new();
        svc.info("all good");
        svc.warn("watch out");
        svc.error("boom");
        let id = svc.info("dismissed info");
        svc.dismiss(id);

        let filter = NotificationFilter::new()
            .severity(&[NotificationSeverity::Info])
            .dismissed(false);
        let result = svc.query(&filter);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message, "all good");
    }
}
