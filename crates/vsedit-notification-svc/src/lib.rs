//! Notification model service.

use std::fmt;
use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// NotificationThrottle – prevent notification spam
// ---------------------------------------------------------------------------

/// Prevents notification spam by enforcing a cooldown between identical messages.
///
/// Tracks a hash of each message and the timestamp (as a `u64` tick counter)
/// when it was last emitted. A duplicate message is suppressed if the elapsed
/// time since the previous emission is less than `cooldown_ticks`.
#[derive(Debug, Clone)]
pub struct NotificationThrottle {
    cooldown_ticks: u64,
    last_seen: std::collections::HashMap<String, u64>,
}

impl NotificationThrottle {
    /// Create a throttle with the given cooldown period (in abstract ticks).
    pub fn new(cooldown_ticks: u64) -> Self {
        Self {
            cooldown_ticks,
            last_seen: std::collections::HashMap::new(),
        }
    }

    /// Returns `true` if the message is allowed (not throttled) at `now`.
    ///
    /// If allowed, the internal timestamp for the message is updated.
    pub fn allow(&mut self, message: &str, now: u64) -> bool {
        if let Some(&last) = self.last_seen.get(message) {
            if now.saturating_sub(last) < self.cooldown_ticks {
                return false;
            }
        }
        self.last_seen.insert(message.to_string(), now);
        true
    }

    /// Returns `true` if the message would be throttled at `now` (without
    /// updating internal state).
    pub fn is_throttled(&self, message: &str, now: u64) -> bool {
        if let Some(&last) = self.last_seen.get(message) {
            now.saturating_sub(last) < self.cooldown_ticks
        } else {
            false
        }
    }

    /// Remove all tracked messages, resetting the throttle.
    pub fn reset(&mut self) {
        self.last_seen.clear();
    }

    /// Number of distinct messages currently tracked.
    pub fn tracked_count(&self) -> usize {
        self.last_seen.len()
    }

    /// Evict entries older than `max_age` ticks relative to `now`.
    pub fn evict_older_than(&mut self, now: u64, max_age: u64) {
        self.last_seen
            .retain(|_, &mut ts| now.saturating_sub(ts) <= max_age);
    }
}

impl fmt::Display for NotificationThrottle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NotificationThrottle(cooldown={}, tracked={})",
            self.cooldown_ticks,
            self.last_seen.len()
        )
    }
}

// ---------------------------------------------------------------------------
// NotificationProgressTracker – step-based progress with elapsed time
// ---------------------------------------------------------------------------

/// Tracks step-based progress for a long-running operation, including elapsed
/// time bookkeeping. Unlike [`NotificationProgress`] (which stores raw
/// worked/total counters), this tracker records individual step completions
/// and the tick at which the operation started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationProgressTracker {
    total_steps: u64,
    completed_steps: u64,
    start_tick: u64,
    last_tick: u64,
    label: String,
}

impl NotificationProgressTracker {
    /// Create a tracker for an operation with `total_steps` steps, starting at
    /// `start_tick`.
    pub fn new(label: impl Into<String>, total_steps: u64, start_tick: u64) -> Self {
        Self {
            total_steps,
            completed_steps: 0,
            start_tick,
            last_tick: start_tick,
            label: label.into(),
        }
    }

    /// Mark one step as completed at `tick`.
    pub fn complete_step(&mut self, tick: u64) {
        self.completed_steps = self.completed_steps.saturating_add(1);
        self.last_tick = tick;
    }

    /// Mark `n` steps as completed at `tick`.
    pub fn complete_steps(&mut self, n: u64, tick: u64) {
        self.completed_steps = self.completed_steps.saturating_add(n);
        self.last_tick = tick;
    }

    /// Returns `true` when all steps have been completed.
    pub fn is_done(&self) -> bool {
        self.completed_steps >= self.total_steps
    }

    /// Remaining steps.
    pub fn remaining(&self) -> u64 {
        self.total_steps.saturating_sub(self.completed_steps)
    }

    /// Elapsed ticks since the operation started.
    pub fn elapsed(&self) -> u64 {
        self.last_tick.saturating_sub(self.start_tick)
    }

    /// Completion ratio in `[0.0, 1.0]`.
    pub fn ratio(&self) -> f64 {
        if self.total_steps == 0 {
            return 1.0;
        }
        (self.completed_steps as f64 / self.total_steps as f64).min(1.0)
    }

    /// Estimated remaining ticks based on average pace so far.
    pub fn eta(&self) -> Option<u64> {
        if self.completed_steps == 0 {
            return None;
        }
        let elapsed = self.elapsed();
        let per_step = elapsed / self.completed_steps;
        Some(per_step * self.remaining())
    }

    /// The tracker label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl fmt::Display for NotificationProgressTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}/{} ({:.0}%)",
            self.label,
            self.completed_steps,
            self.total_steps,
            self.ratio() * 100.0,
        )
    }
}

// ---------------------------------------------------------------------------
// NotificationActionPipeline – chained actions with retry
// ---------------------------------------------------------------------------

/// Outcome of a single action execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionOutcome {
    Success,
    Failed(String),
}

/// Records the result of executing one step in the pipeline.
#[derive(Debug, Clone)]
pub struct PipelineStepResult {
    pub action_id: String,
    pub outcome: ActionOutcome,
    pub attempts: u32,
}

/// Chains multiple [`NotificationAction`]s with per-step retry logic.
///
/// Actions are executed in order. Each action is retried up to `max_retries`
/// times on failure. The pipeline records the outcome of every step so callers
/// can inspect which actions succeeded and which failed.
#[derive(Debug, Clone)]
pub struct NotificationActionPipeline {
    actions: Vec<NotificationAction>,
    max_retries: u32,
    results: Vec<PipelineStepResult>,
}

impl NotificationActionPipeline {
    /// Create a new empty pipeline with the given retry limit per action.
    pub fn new(max_retries: u32) -> Self {
        Self {
            actions: Vec::new(),
            max_retries,
            results: Vec::new(),
        }
    }

    /// Append an action to the pipeline.
    pub fn push(&mut self, action: NotificationAction) {
        self.actions.push(action);
    }

    /// Number of actions in the pipeline.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Returns `true` if the pipeline has no actions.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Run every action through the provided executor function. The executor
    /// receives the action and returns `Ok(())` on success or `Err(msg)` on
    /// failure.
    pub fn execute<F>(&mut self, mut executor: F)
    where
        F: FnMut(&NotificationAction) -> Result<(), String>,
    {
        self.results.clear();
        for action in &self.actions {
            let mut last_err = String::new();
            let mut succeeded = false;
            for attempt in 0..=self.max_retries {
                match executor(action) {
                    Ok(()) => {
                        self.results.push(PipelineStepResult {
                            action_id: action.id.clone(),
                            outcome: ActionOutcome::Success,
                            attempts: attempt + 1,
                        });
                        succeeded = true;
                        break;
                    }
                    Err(e) => {
                        last_err = e;
                    }
                }
            }
            if !succeeded {
                self.results.push(PipelineStepResult {
                    action_id: action.id.clone(),
                    outcome: ActionOutcome::Failed(last_err),
                    attempts: self.max_retries + 1,
                });
            }
        }
    }

    /// Returns the recorded results from the last `execute` call.
    pub fn results(&self) -> &[PipelineStepResult] {
        &self.results
    }

    /// Returns `true` if every action succeeded in the last execution.
    pub fn all_succeeded(&self) -> bool {
        !self.results.is_empty()
            && self
                .results
                .iter()
                .all(|r| r.outcome == ActionOutcome::Success)
    }

    /// Count of actions that failed in the last execution.
    pub fn failure_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.outcome != ActionOutcome::Success)
            .count()
    }
}

impl fmt::Display for NotificationActionPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NotificationActionPipeline(actions={}, retries={})",
            self.actions.len(),
            self.max_retries
        )
    }
}

// ---------------------------------------------------------------------------
// NotificationPersistence – serialize / deserialize notifications
// ---------------------------------------------------------------------------

/// A serializable snapshot of a [`Notification`] suitable for persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationRecord {
    pub id: u64,
    pub message: String,
    pub severity: u8,
    pub source: Option<String>,
    pub dismissed: bool,
    pub sticky: bool,
}

impl NotificationRecord {
    /// Convert a [`Notification`] into a persistable record.
    pub fn from_notification(n: &Notification) -> Self {
        Self {
            id: n.id,
            message: n.message.clone(),
            severity: u8::from(n.severity),
            source: n.source.clone(),
            dismissed: n.dismissed,
            sticky: n.sticky,
        }
    }

    /// Reconstruct a [`Notification`] from this record.
    pub fn to_notification(&self) -> Notification {
        let severity = match self.severity {
            0 => NotificationSeverity::Info,
            1 => NotificationSeverity::Warning,
            _ => NotificationSeverity::Error,
        };
        Notification {
            id: self.id,
            message: self.message.clone(),
            severity,
            source: self.source.clone(),
            actions: Vec::new(),
            sticky: self.sticky,
            dismissed: self.dismissed,
            priority: None,
        }
    }
}

/// Stores a collection of [`NotificationRecord`]s for persistence across
/// application restarts.
#[derive(Debug, Clone)]
pub struct NotificationPersistence {
    records: Vec<NotificationRecord>,
}

impl NotificationPersistence {
    /// Create an empty persistence store.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Snapshot the current notifications from a service into records.
    pub fn save(&mut self, service: &NotificationService) {
        self.records = service
            .get_active()
            .iter()
            .map(|n| NotificationRecord::from_notification(n))
            .collect();
    }

    /// Restore saved records into a new `NotificationService`.
    pub fn restore(&self) -> NotificationService {
        let mut svc = NotificationService::new();
        for rec in &self.records {
            let n = rec.to_notification();
            let id = svc.add(n.message.clone(), n.severity);
            if n.sticky {
                svc.set_sticky(id, true);
            }
            if let Some(ref src) = n.source {
                if let Some(entry) = svc.notifications.iter_mut().find(|e| e.id == id) {
                    entry.source = Some(src.clone());
                }
            }
        }
        svc
    }

    /// Number of stored records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` if no records are stored.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Clear all stored records.
    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Access the raw records.
    pub fn records(&self) -> &[NotificationRecord] {
        &self.records
    }
}

impl Default for NotificationPersistence {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NotificationPersistence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NotificationPersistence(records={})",
            self.records.len()
        )
    }
}

// --- NotificationThrottlerV2: rate limit notifications per source ---

pub struct NotificationThrottlerV2 {
    cooldown_ms: u64,
    last_shown: HashMap<String, u64>,
    suppressed_count: usize,
}

impl NotificationThrottlerV2 {
    pub fn new(cooldown_ms: u64) -> Self {
        Self { cooldown_ms, last_shown: HashMap::new(), suppressed_count: 0 }
    }

    pub fn should_show(&self, source: &str, now_ms: u64) -> bool {
        match self.last_shown.get(source) {
            Some(&last) => now_ms.saturating_sub(last) >= self.cooldown_ms,
            None => true,
        }
    }

    pub fn record_shown(&mut self, source: &str, now_ms: u64) {
        self.last_shown.insert(source.to_string(), now_ms);
    }

    pub fn try_show(&mut self, source: &str, now_ms: u64) -> bool {
        if self.should_show(source, now_ms) {
            self.record_shown(source, now_ms);
            true
        } else {
            self.suppressed_count += 1;
            false
        }
    }

    pub fn suppressed_count(&self) -> usize { self.suppressed_count }
    pub fn cooldown_ms(&self) -> u64 { self.cooldown_ms }
}

// --- NotificationStack: compute y positions ---

pub struct NotificationStack {
    max_visible: usize,
    item_height: u16,
    from_bottom: bool,
    container_height: u16,
}

impl NotificationStack {
    pub fn new(max_visible: usize, item_height: u16, container_height: u16, from_bottom: bool) -> Self {
        Self { max_visible, item_height, from_bottom, container_height }
    }

    pub fn compute_y_offset(&self, index: usize) -> Option<u16> {
        if index >= self.max_visible { return None; }
        if self.from_bottom {
            Some(self.container_height.saturating_sub((index as u16 + 1) * self.item_height))
        } else {
            Some(index as u16 * self.item_height)
        }
    }

    pub fn max_visible(&self) -> usize { self.max_visible }

    pub fn animate_shift(&self, positions: &[u16]) -> Vec<u16> {
        positions.iter().enumerate().filter_map(|(i, _)| {
            if i + 1 < positions.len() { self.compute_y_offset(i) } else { None }
        }).collect()
    }
}

// --- NotificationHistory ---

pub struct NotificationHistoryEntry {
    pub message: String,
    pub severity: NotificationSeverity,
    pub timestamp_ms: u64,
}

pub struct NotificationHistory {
    entries: Vec<NotificationHistoryEntry>,
}

impl NotificationHistory {
    pub fn new() -> Self { Self { entries: Vec::new() } }

    pub fn add(&mut self, message: &str, severity: NotificationSeverity, timestamp_ms: u64) {
        self.entries.push(NotificationHistoryEntry {
            message: message.to_string(), severity, timestamp_ms,
        });
    }

    pub fn query_by_time_range(&self, from_ms: u64, to_ms: u64) -> Vec<&NotificationHistoryEntry> {
        self.entries.iter().filter(|e| e.timestamp_ms >= from_ms && e.timestamp_ms <= to_ms).collect()
    }

    pub fn query_by_severity(&self, severity: NotificationSeverity) -> Vec<&NotificationHistoryEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn clear_before(&mut self, timestamp_ms: u64) {
        self.entries.retain(|e| e.timestamp_ms >= timestamp_ms);
    }

    pub fn total_count(&self) -> usize { self.entries.len() }

    pub fn most_recent_n(&self, n: usize) -> Vec<&NotificationHistoryEntry> {
        self.entries.iter().rev().take(n).collect()
    }
}


// ---------------------------------------------------------------------------
// notification_svc – Platform service helpers
// ---------------------------------------------------------------------------

/// Capability flags for platform feature detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XNotificationSvcCapabilities {
    flags: std::collections::HashSet<String>,
}

impl XNotificationSvcCapabilities {
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

impl Default for XNotificationSvcCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple service registry keyed by name.
#[derive(Debug, Default)]
pub struct XNotificationSvcServiceRegistry {
    services: std::collections::HashMap<String, String>,
}

impl XNotificationSvcServiceRegistry {
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
pub fn x_notification_svc_sanitize_path(p: &str) -> String {
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
// notification_svc – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for notification service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YNotificationSvcNotificationLevel {
    Info,
    Warning,
    Error,
    Silent,
}

impl YNotificationSvcNotificationLevel {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Info => 0,
            Self::Warning => 1,
            Self::Error => 2,
            Self::Silent => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Silent => "Silent",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YNotificationSvcNotificationLevel] {
        &[
            YNotificationSvcNotificationLevel::Info,
            YNotificationSvcNotificationLevel::Warning,
            YNotificationSvcNotificationLevel::Error,
            YNotificationSvcNotificationLevel::Silent,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YNotificationSvcNotificationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks notification queue data.
#[derive(Debug, Clone)]
pub struct YNotificationSvcNotificationQueue {
    pub items: Vec<(String, u64)>,
    pub max_visible: usize,
    pub dismissed: u64,
}

impl YNotificationSvcNotificationQueue {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_visible: 0,
            dismissed: 0,
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
        format!("YNotificationSvcNotificationQueue({}: {:?})", "items", self.items)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_notification_svc_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_notification_svc_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_notification_svc_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_notification_svc_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_notification_svc_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_notification_svc_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_notification_svc_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_notification_svc_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// notification_svc – Extended notification throttle helpers
// ---------------------------------------------------------------------------

/// Priority levels for notification throttle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZNotificationSvcPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZNotificationSvcPriority {
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
    pub fn all_asc() -> [ZNotificationSvcPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZNotificationSvcPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks notification throttle data.
#[derive(Debug, Clone)]
pub struct ZNotificationSvcNotificationThrottle {
    pub recent_ids: Vec<(u64, u64)>,
    pub window_ms: u64,
    pub suppressed: u64,
}

impl ZNotificationSvcNotificationThrottle {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            recent_ids: Vec::new(),
            window_ms: 0,
            suppressed: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.recent_ids.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.recent_ids.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.recent_ids.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZNotificationSvcNotificationThrottle[window_ms={:?}, suppressed={:?}]", self.window_ms, self.suppressed)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for notification throttle.
pub fn z_notification_svc_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_notification_svc_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_notification_svc_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_notification_svc_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_notification_svc_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_notification_svc_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_notification_svc_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 86
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer86 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer86 {
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
pub fn xb_fnv1a_86(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_86<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_86<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_86(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_86(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 129
// ---------------------------------------------------------------------------

/// Generic object pool `Xc129Pool<T>`.
pub struct Xc129Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc129Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc129PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc129Pool<T> {
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
    pub fn stats(&self) -> Xc129PoolStats {
        Xc129PoolStats {
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

impl<T> Default for Xc129Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc129Scheduler`.
pub struct Xc129Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc129Scheduler {
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

impl Default for Xc129Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_129 hash for the given byte slice.
pub fn xc_129_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_129 convention.
pub fn xc_129_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe99 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe99Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe99PipelineError {
    pub stage: Xe99Stage,
    pub message: String,
}

impl std::fmt::Display for Xe99PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe99Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe99Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe99PipelineError>>>,
    stage_names: Vec<Xe99Stage>,
}

impl Xe99Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe99PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe99Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe99PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe99Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe99PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe99Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe99PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe99Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe99PipelineError> {
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

    pub fn compose(mut self, other: Xe99Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe99CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe99CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe99Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe99CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe99CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe99Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe99CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_99_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe99CacheEntry {
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

    fn xe_99_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe99CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_99_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe99PipelineError> {
    Ok(data)
}

pub fn xe_99_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe99PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_99_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe99PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_99_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe99PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_99_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe99PipelineError> {
    Err(Xe99PipelineError {
        stage: Xe99Stage::Parse,
        message: "intentional failure".to_string(),
    })
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
    fn set_sticky_works() {
        let mut svc = NotificationService::new();
        let id = svc.warn("important");
        svc.set_sticky(id, true);
        assert!(svc.get_active()[0].sticky);
        svc.set_sticky(id, false);
        assert!(!svc.get_active()[0].sticky);
    }

    #[test]
    fn get_by_severity_works() {
        let mut svc = NotificationService::new();
        svc.info("a");
        svc.warn("b");
        svc.error("c");
        svc.info("d");
        assert_eq!(svc.get_by_severity(NotificationSeverity::Info).len(), 2);
        assert_eq!(svc.get_by_severity(NotificationSeverity::Error).len(), 1);
    }

    #[test]
    fn get_by_source_works() {
        let mut svc = NotificationService::new();
        let id = svc.info("from linter");
        // Manually set source for testing.
        svc.notifications.iter_mut().find(|n| n.id == id).unwrap().source = Some("linter".into());
        svc.info("no source");
        assert_eq!(svc.get_by_source("linter").len(), 1);
        assert_eq!(svc.get_by_source("unknown").len(), 0);
    }

    #[test]
    fn remove_dismissed_works() {
        let mut svc = NotificationService::new();
        let id = svc.info("gone");
        svc.info("stay");
        svc.dismiss(id);
        svc.remove_dismissed();
        assert_eq!(svc.notification_count(), 1);
        assert_eq!(svc.get_active()[0].message, "stay");
    }

    #[test]
    fn get_stats_works() {
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
    fn dedup_by_message_works() {
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
    fn has_duplicate_works() {
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
    fn add_with_priority_works() {
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
    fn highest_priority_active_works() {
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

    // -- NotificationThrottle tests ----------------------------------------

    #[test]
    fn throttle_allows_first_message() {
        let mut t = NotificationThrottle::new(10);
        assert!(t.allow("hello", 0));
    }

    #[test]
    fn throttle_blocks_duplicate_within_cooldown() {
        let mut t = NotificationThrottle::new(10);
        assert!(t.allow("hello", 0));
        assert!(!t.allow("hello", 5));
    }

    #[test]
    fn throttle_allows_after_cooldown() {
        let mut t = NotificationThrottle::new(10);
        assert!(t.allow("hello", 0));
        assert!(t.allow("hello", 10));
    }

    #[test]
    fn throttle_is_throttled_does_not_mutate() {
        let mut t = NotificationThrottle::new(10);
        assert!(t.allow("msg", 0));
        assert!(t.is_throttled("msg", 5));
        assert!(!t.is_throttled("other", 5));
    }

    #[test]
    fn throttle_evict_and_reset() {
        let mut t = NotificationThrottle::new(10);
        t.allow("a", 0);
        t.allow("b", 5);
        assert_eq!(t.tracked_count(), 2);
        t.evict_older_than(20, 10);
        assert_eq!(t.tracked_count(), 0);
        t.allow("c", 30);
        t.reset();
        assert_eq!(t.tracked_count(), 0);
    }

    #[test]
    fn throttle_display() {
        let t = NotificationThrottle::new(42);
        assert!(format!("{t}").contains("cooldown=42"));
    }

    // -- NotificationProgressTracker tests ---------------------------------

    #[test]
    fn progress_tracker_basic_flow() {
        let mut pt = NotificationProgressTracker::new("build", 4, 100);
        assert_eq!(pt.remaining(), 4);
        assert!(!pt.is_done());
        pt.complete_step(110);
        pt.complete_steps(3, 140);
        assert!(pt.is_done());
        assert_eq!(pt.elapsed(), 40);
        assert_eq!(pt.label(), "build");
    }

    #[test]
    fn progress_tracker_ratio_and_eta() {
        let mut pt = NotificationProgressTracker::new("index", 10, 0);
        assert!((pt.ratio() - 0.0).abs() < f64::EPSILON);
        assert!(pt.eta().is_none());
        pt.complete_steps(5, 50);
        assert!((pt.ratio() - 0.5).abs() < f64::EPSILON);
        assert_eq!(pt.eta(), Some(50));
    }

    #[test]
    fn progress_tracker_display() {
        let mut pt = NotificationProgressTracker::new("scan", 2, 0);
        pt.complete_step(1);
        let s = format!("{pt}");
        assert!(s.contains("scan"));
        assert!(s.contains("1/2"));
    }

    // -- NotificationActionPipeline tests ----------------------------------

    #[test]
    fn pipeline_all_succeed() {
        let mut p = NotificationActionPipeline::new(2);
        p.push(NotificationAction {
            label: "Save".into(),
            id: "save".into(),
        });
        p.push(NotificationAction {
            label: "Close".into(),
            id: "close".into(),
        });
        p.execute(|_| Ok(()));
        assert!(p.all_succeeded());
        assert_eq!(p.failure_count(), 0);
        assert_eq!(p.results().len(), 2);
    }

    #[test]
    fn pipeline_retry_then_succeed() {
        let mut p = NotificationActionPipeline::new(3);
        p.push(NotificationAction {
            label: "Flaky".into(),
            id: "flaky".into(),
        });
        let mut call = 0u32;
        p.execute(|_| {
            call += 1;
            if call < 3 {
                Err("transient".into())
            } else {
                Ok(())
            }
        });
        assert!(p.all_succeeded());
        assert_eq!(p.results()[0].attempts, 3);
    }

    #[test]
    fn pipeline_permanent_failure() {
        let mut p = NotificationActionPipeline::new(1);
        p.push(NotificationAction {
            label: "Bad".into(),
            id: "bad".into(),
        });
        p.execute(|_| Err("nope".into()));
        assert!(!p.all_succeeded());
        assert_eq!(p.failure_count(), 1);
        assert_eq!(
            p.results()[0].outcome,
            ActionOutcome::Failed("nope".into())
        );
    }

    #[test]
    fn pipeline_display() {
        let p = NotificationActionPipeline::new(5);
        assert!(format!("{p}").contains("retries=5"));
    }

    // -- NotificationPersistence tests -------------------------------------

    #[test]
    fn persistence_save_and_restore() {
        let mut svc = NotificationService::new();
        svc.info("one");
        svc.warn("two");
        let id3 = svc.error("three");
        svc.dismiss(id3);

        let mut store = NotificationPersistence::new();
        store.save(&svc);
        // Only active (non-dismissed) are saved
        assert_eq!(store.len(), 2);

        let restored = store.restore();
        assert_eq!(restored.notification_count(), 2);
        assert!(restored.find_by_message("one").is_some());
        assert!(restored.find_by_message("two").is_some());
    }

    #[test]
    fn persistence_clear() {
        let mut store = NotificationPersistence::new();
        let mut svc = NotificationService::new();
        svc.info("x");
        store.save(&svc);
        assert!(!store.is_empty());
        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn persistence_display() {
        let store = NotificationPersistence::new();
        assert!(format!("{store}").contains("records=0"));
    }

    #[test]
    fn notification_record_roundtrip() {
        let n = Notification {
            id: 42,
            message: "test".into(),
            severity: NotificationSeverity::Warning,
            source: Some("lsp".into()),
            actions: vec![],
            sticky: true,
            dismissed: false,
            priority: None,
        };
        let rec = NotificationRecord::from_notification(&n);
        let back = rec.to_notification();
        assert_eq!(back.id, 42);
        assert_eq!(back.message, "test");
        assert_eq!(back.severity, NotificationSeverity::Warning);
        assert_eq!(back.source, Some("lsp".into()));
        assert!(back.sticky);
    }

    #[test]
    fn throttler_v2_should_show_first_time() {
        let t = NotificationThrottlerV2::new(1000);
        assert!(t.should_show("src", 100));
    }

    #[test]
    fn throttler_v2_suppresses_within_cooldown() {
        let mut t = NotificationThrottlerV2::new(1000);
        t.record_shown("src", 100);
        assert!(!t.should_show("src", 500));
    }

    #[test]
    fn throttler_v2_allows_after_cooldown() {
        let mut t = NotificationThrottlerV2::new(1000);
        t.record_shown("src", 100);
        assert!(t.should_show("src", 1200));
    }

    #[test]
    fn throttler_v2_try_show_suppresses() {
        let mut t = NotificationThrottlerV2::new(1000);
        assert!(t.try_show("s", 0));
        assert!(!t.try_show("s", 500));
        assert_eq!(t.suppressed_count(), 1);
    }

    #[test]
    fn notification_stack_y_from_top() {
        let s = NotificationStack::new(5, 30, 300, false);
        assert_eq!(s.compute_y_offset(0), Some(0));
        assert_eq!(s.compute_y_offset(1), Some(30));
    }

    #[test]
    fn notification_stack_y_from_bottom() {
        let s = NotificationStack::new(5, 30, 300, true);
        assert_eq!(s.compute_y_offset(0), Some(270));
        assert_eq!(s.compute_y_offset(1), Some(240));
    }

    #[test]
    fn notification_stack_exceeds_max() {
        let s = NotificationStack::new(2, 30, 300, false);
        assert!(s.compute_y_offset(5).is_none());
    }

    #[test]
    fn notification_history_add_and_count() {
        let mut h = NotificationHistory::new();
        h.add("msg1", NotificationSeverity::Info, 100);
        h.add("msg2", NotificationSeverity::Error, 200);
        assert_eq!(h.total_count(), 2);
    }

    #[test]
    fn notification_history_by_time_range() {
        let mut h = NotificationHistory::new();
        h.add("a", NotificationSeverity::Info, 100);
        h.add("b", NotificationSeverity::Info, 200);
        h.add("c", NotificationSeverity::Info, 300);
        assert_eq!(h.query_by_time_range(150, 250).len(), 1);
    }

    #[test]
    fn notification_history_by_severity() {
        let mut h = NotificationHistory::new();
        h.add("a", NotificationSeverity::Info, 100);
        h.add("b", NotificationSeverity::Error, 200);
        assert_eq!(h.query_by_severity(NotificationSeverity::Error).len(), 1);
    }

    #[test]
    fn notification_history_clear_before() {
        let mut h = NotificationHistory::new();
        h.add("a", NotificationSeverity::Info, 100);
        h.add("b", NotificationSeverity::Info, 200);
        h.clear_before(150);
        assert_eq!(h.total_count(), 1);
    }

    #[test]
    fn notification_history_most_recent() {
        let mut h = NotificationHistory::new();
        h.add("a", NotificationSeverity::Info, 100);
        h.add("b", NotificationSeverity::Info, 200);
        h.add("c", NotificationSeverity::Info, 300);
        let recent = h.most_recent_n(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].message, "c");
    }

    // -- notification_svc additional tests -------------------------------------------

    #[test]
    fn x_notification_svc_capabilities_register_and_has() {
        let mut caps = XNotificationSvcCapabilities::new();
        caps.register("clipboard");
        assert!(caps.has("clipboard"));
        assert!(!caps.has("fs"));
    }

    #[test]
    fn x_notification_svc_capabilities_len() {
        let mut caps = XNotificationSvcCapabilities::new();
        assert!(caps.is_empty());
        caps.register("a");
        caps.register("b");
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn x_notification_svc_capabilities_intersect() {
        let mut a = XNotificationSvcCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XNotificationSvcCapabilities::new();
        b.register("y");
        b.register("z");
        let inter = a.intersect(&b);
        assert_eq!(inter.len(), 1);
        assert!(inter.has("y"));
    }

    #[test]
    fn x_notification_svc_capabilities_diff() {
        let mut a = XNotificationSvcCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XNotificationSvcCapabilities::new();
        b.register("y");
        let d = a.diff(&b);
        assert_eq!(d.len(), 1);
        assert!(d.has("x"));
    }

    #[test]
    fn x_notification_svc_service_registry_basic() {
        let mut reg = XNotificationSvcServiceRegistry::new();
        assert!(reg.is_empty());
        reg.register("clipboard", "v1");
        assert_eq!(reg.get("clipboard"), Some("v1"));
        assert!(reg.contains("clipboard"));
    }

    #[test]
    fn x_notification_svc_service_registry_replace() {
        let mut reg = XNotificationSvcServiceRegistry::new();
        assert!(reg.register("svc", "old").is_none());
        assert_eq!(reg.register("svc", "new"), Some("old".into()));
        assert_eq!(reg.get("svc"), Some("new"));
    }

    #[test]
    fn x_notification_svc_service_registry_remove() {
        let mut reg = XNotificationSvcServiceRegistry::new();
        reg.register("svc", "v1");
        assert_eq!(reg.remove("svc"), Some("v1".into()));
        assert!(reg.is_empty());
    }

    #[test]
    fn x_notification_svc_service_registry_names() {
        let mut reg = XNotificationSvcServiceRegistry::new();
        reg.register("a", "1");
        reg.register("b", "2");
        let mut names = reg.names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn x_notification_svc_sanitize_path_basic() {
        assert_eq!(x_notification_svc_sanitize_path("/a//b///c/"), "/a/b/c");
    }

    #[test]
    fn x_notification_svc_sanitize_path_backslash() {
        assert_eq!(x_notification_svc_sanitize_path("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn x_notification_svc_sanitize_path_single() {
        assert_eq!(x_notification_svc_sanitize_path("/"), "/");
    }

    #[test]
    fn x_notification_svc_capabilities_default() {
        let caps = XNotificationSvcCapabilities::default();
        assert!(caps.is_empty());
    }

    #[test]
    fn x_notification_svc_capabilities_all() {
        let mut caps = XNotificationSvcCapabilities::new();
        caps.register("a");
        caps.register("b");
        let mut all = caps.all();
        all.sort();
        assert_eq!(all, vec!["a", "b"]);
    }


    // -- notification_svc extended domain tests ----------------------------------------

    #[test]
    fn y_notification_svc_enum_index() {
        assert_eq!(YNotificationSvcNotificationLevel::Info.index(), 0);
        assert_eq!(YNotificationSvcNotificationLevel::Warning.index(), 1);
        assert_eq!(YNotificationSvcNotificationLevel::Error.index(), 2);
        assert_eq!(YNotificationSvcNotificationLevel::Silent.index(), 3);
    }

    #[test]
    fn y_notification_svc_enum_label() {
        assert_eq!(YNotificationSvcNotificationLevel::Info.label(), "Info");
        assert_eq!(YNotificationSvcNotificationLevel::Warning.label(), "Warning");
        assert_eq!(YNotificationSvcNotificationLevel::Error.label(), "Error");
        assert_eq!(YNotificationSvcNotificationLevel::Silent.label(), "Silent");
    }

    #[test]
    fn y_notification_svc_enum_all() {
        let all = YNotificationSvcNotificationLevel::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_notification_svc_enum_is_default() {
        assert!(YNotificationSvcNotificationLevel::Info.is_default());
        assert!(!YNotificationSvcNotificationLevel::Silent.is_default());
    }

    #[test]
    fn y_notification_svc_enum_display() {
        assert_eq!(format!("{}", YNotificationSvcNotificationLevel::Info), "Info");
    }

    #[test]
    fn y_notification_svc_struct_new() {
        let s = YNotificationSvcNotificationQueue::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_notification_svc_struct_clear() {
        let mut s = YNotificationSvcNotificationQueue::new();
        s.items.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_notification_svc_fingerprint_deterministic() {
        let h1 = y_notification_svc_fingerprint("hello");
        let h2 = y_notification_svc_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_notification_svc_fingerprint("a"), y_notification_svc_fingerprint("b"));
    }

    #[test]
    fn y_notification_svc_truncate_short() {
        assert_eq!(y_notification_svc_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_notification_svc_truncate_long() {
        let r = y_notification_svc_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_notification_svc_normalize_key_basic() {
        assert_eq!(y_notification_svc_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_notification_svc_split_path_basic() {
        let parts = y_notification_svc_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_notification_svc_count_occurrences_basic() {
        assert_eq!(y_notification_svc_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_notification_svc_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_notification_svc_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_notification_svc_in_range_basic() {
        assert!(y_notification_svc_in_range(5, 1, 10));
        assert!(y_notification_svc_in_range(1, 1, 10));
        assert!(y_notification_svc_in_range(10, 1, 10));
        assert!(!y_notification_svc_in_range(0, 1, 10));
        assert!(!y_notification_svc_in_range(11, 1, 10));
    }

    #[test]
    fn y_notification_svc_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_notification_svc_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_notification_svc_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_notification_svc_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- notification_svc Z-extended tests -----------------------------------------------

    #[test]
    fn z_notification_svc_priority_weight() {
        assert_eq!(ZNotificationSvcPriority::Idle.weight(), 0);
        assert_eq!(ZNotificationSvcPriority::Normal.weight(), 2);
        assert_eq!(ZNotificationSvcPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_notification_svc_priority_label() {
        assert_eq!(ZNotificationSvcPriority::Low.label(), "low");
        assert_eq!(ZNotificationSvcPriority::High.label(), "high");
    }

    #[test]
    fn z_notification_svc_priority_is_elevated() {
        assert!(!ZNotificationSvcPriority::Normal.is_elevated());
        assert!(ZNotificationSvcPriority::High.is_elevated());
        assert!(ZNotificationSvcPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_notification_svc_priority_display() {
        assert_eq!(format!("{}", ZNotificationSvcPriority::Idle), "idle");
    }

    #[test]
    fn z_notification_svc_priority_all_asc() {
        let all = ZNotificationSvcPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZNotificationSvcPriority::Idle);
        assert_eq!(all[4], ZNotificationSvcPriority::Realtime);
    }

    #[test]
    fn z_notification_svc_struct_new() {
        let s = ZNotificationSvcNotificationThrottle::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_notification_svc_struct_toggled_clone() {
        let s = ZNotificationSvcNotificationThrottle::new();
        let t = s.toggled_clone();
        let _ = t.suppressed;
    }

    #[test]
    fn z_notification_svc_rolling_hash_deterministic() {
        let h1 = z_notification_svc_rolling_hash(b"test");
        let h2 = z_notification_svc_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_notification_svc_rolling_hash(b"a"), z_notification_svc_rolling_hash(b"b"));
    }

    #[test]
    fn z_notification_svc_pad_to_basic() {
        assert_eq!(z_notification_svc_pad_to("hi", 5), "hi   ");
        assert_eq!(z_notification_svc_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_notification_svc_is_identifier_basic() {
        assert!(z_notification_svc_is_identifier("foo_bar"));
        assert!(z_notification_svc_is_identifier("abc123"));
        assert!(!z_notification_svc_is_identifier(""));
        assert!(!z_notification_svc_is_identifier("has space"));
    }

    #[test]
    fn z_notification_svc_levenshtein_basic() {
        assert_eq!(z_notification_svc_levenshtein("", ""), 0);
        assert_eq!(z_notification_svc_levenshtein("abc", "abc"), 0);
        assert_eq!(z_notification_svc_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_notification_svc_unique_words_basic() {
        let w = z_notification_svc_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_notification_svc_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_notification_svc_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_notification_svc_common_prefix_basic() {
        assert_eq!(z_notification_svc_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_notification_svc_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_notification_svc_struct_clear() {
        let mut s = ZNotificationSvcNotificationThrottle::new();
        s.recent_ids.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_notification_svc_rolling_hash_empty() {
        let h = z_notification_svc_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_86_push_and_len() {
        let mut rb = super::XbRingBuffer86::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_86_overwrite() {
        let mut rb = super::XbRingBuffer86::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_86_get_out_of_bounds() {
        let rb = super::XbRingBuffer86::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_86_drain_all() {
        let mut rb = super::XbRingBuffer86::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_86_peek_front_back() {
        let mut rb = super::XbRingBuffer86::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_86_clear() {
        let mut rb = super::XbRingBuffer86::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_86_capacity() {
        let rb = super::XbRingBuffer86::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_86_basic() {
        let h = super::xb_fnv1a_86(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_86(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_86_different_inputs() {
        let h1 = super::xb_fnv1a_86(b"abc");
        let h2 = super::xb_fnv1a_86(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_86_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_86(&data);
        let dec = super::xb_rle_decode_86(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_86_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_86(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_86(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_86_values() {
        assert!((super::xb_clamp_86(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_86(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_86(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_86_values() {
        assert!((super::xb_lerp_86(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_86(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_86(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_86_wrap_around_twice() {
        let mut rb = super::XbRingBuffer86::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 129 ----

    #[test]
    fn xc_129_pool_new_empty() {
        let pool: super::Xc129Pool<i32> = super::Xc129Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_129_pool_release_acquire() {
        let mut pool = super::Xc129Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_129_pool_acquire_empty() {
        let mut pool: super::Xc129Pool<i32> = super::Xc129Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_129_pool_full() {
        let mut pool = super::Xc129Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_129_pool_drain() {
        let mut pool = super::Xc129Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_129_pool_stats() {
        let mut pool = super::Xc129Pool::new(8);
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
    fn xc_129_pool_clear() {
        let mut pool = super::Xc129Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_129_pool_shrink() {
        let mut pool = super::Xc129Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_129_pool_default() {
        let pool: super::Xc129Pool<String> = super::Xc129Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_129_pool_extend() {
        let mut pool = super::Xc129Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_129_pool_retain() {
        let mut pool = super::Xc129Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_129_scheduler_round_robin() {
        let mut sched = super::Xc129Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_129_scheduler_empty() {
        let mut sched = super::Xc129Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_129_scheduler_reset() {
        let mut sched = super::Xc129Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_129_scheduler_add_remove() {
        let mut sched = super::Xc129Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_129_scheduler_targets() {
        let sched = super::Xc129Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_129_hash_empty() {
        assert_eq!(super::xc_129_hash(b""), 5381);
    }

    #[test]
    fn xc_129_hash_data() {
        let h = super::xc_129_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_129_hash(b"hello"), h);
    }

    #[test]
    fn xc_129_reverse_str() {
        assert_eq!(super::xc_129_reverse("abc"), "cba");
        assert_eq!(super::xc_129_reverse(""), "");
    }


    #[test]
    fn xe_99_pipeline_empty() {
        let p = super::Xe99Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_99_pipeline_parse_stage() {
        let p = super::Xe99Pipeline::new()
            .add_parse(super::xe_99_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_99_pipeline_transform_double() {
        let p = super::Xe99Pipeline::new()
            .add_transform(super::xe_99_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_99_pipeline_validate_reverse() {
        let p = super::Xe99Pipeline::new()
            .add_validate(super::xe_99_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_99_pipeline_emit_filter() {
        let p = super::Xe99Pipeline::new()
            .add_emit(super::xe_99_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_99_pipeline_multi_stage() {
        let p = super::Xe99Pipeline::new()
            .add_parse(super::xe_99_pipeline_identity)
            .add_transform(super::xe_99_pipeline_double)
            .add_validate(super::xe_99_pipeline_reverse)
            .add_emit(super::xe_99_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_99_pipeline_error_propagation() {
        let p = super::Xe99Pipeline::new()
            .add_parse(super::xe_99_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe99Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_99_pipeline_compose() {
        let p1 = super::Xe99Pipeline::new()
            .add_parse(super::xe_99_pipeline_identity);
        let p2 = super::Xe99Pipeline::new()
            .add_transform(super::xe_99_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_99_pipeline_error_display() {
        let e = super::Xe99PipelineError {
            stage: super::Xe99Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_99_cache_put_get() {
        let mut c = super::Xe99Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_99_cache_miss() {
        let mut c: super::Xe99Cache<&str, i32> = super::Xe99Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_99_cache_ttl_expiry() {
        let mut c = super::Xe99Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_99_cache_evict() {
        let mut c = super::Xe99Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_99_cache_capacity() {
        let mut c = super::Xe99Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_99_cache_stats() {
        let mut c = super::Xe99Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_99_cache_clear() {
        let mut c = super::Xe99Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

}
