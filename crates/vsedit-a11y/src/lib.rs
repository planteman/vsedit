//! Accessibility service.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur when using the accessibility service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessibilityError {
    ServiceDisabled,
    InvalidAnnouncement,
    RoleNotSupported(AriaRole),
}

impl fmt::Display for AccessibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ServiceDisabled => write!(f, "accessibility service is disabled"),
            Self::InvalidAnnouncement => write!(f, "invalid announcement"),
            Self::RoleNotSupported(role) => write!(f, "role not supported: {role}"),
        }
    }
}

/// Whether accessibility support is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilitySupport {
    Unknown,
    Disabled,
    Enabled,
}

impl fmt::Display for AccessibilitySupport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => write!(f, "Unknown"),
            Self::Disabled => write!(f, "Disabled"),
            Self::Enabled => write!(f, "Enabled"),
        }
    }
}

/// Wrapper indicating screen-reader-optimized mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenReaderOptimized(pub bool);

/// Priority level for an announcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncementPriority {
    Polite,
    Assertive,
}

impl fmt::Display for AnnouncementPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Polite => write!(f, "Polite"),
            Self::Assertive => write!(f, "Assertive"),
        }
    }
}

/// A live-region announcement to be delivered to assistive technology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    pub message: String,
    pub priority: AnnouncementPriority,
}

impl Announcement {
    /// Create a polite announcement.
    pub fn polite(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            priority: AnnouncementPriority::Polite,
        }
    }

    /// Create an assertive announcement.
    pub fn assertive(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            priority: AnnouncementPriority::Assertive,
        }
    }
}

/// ARIA roles used for widget semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaRole {
    Button,
    Checkbox,
    Dialog,
    Grid,
    List,
    ListItem,
    Menu,
    MenuItem,
    Tab,
    TabPanel,
    Tree,
    TreeItem,
    TextBox,
    Status,
    Alert,
}

impl fmt::Display for AriaRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Button => "button",
            Self::Checkbox => "checkbox",
            Self::Dialog => "dialog",
            Self::Grid => "grid",
            Self::List => "list",
            Self::ListItem => "listitem",
            Self::Menu => "menu",
            Self::MenuItem => "menuitem",
            Self::Tab => "tab",
            Self::TabPanel => "tabpanel",
            Self::Tree => "tree",
            Self::TreeItem => "treeitem",
            Self::TextBox => "textbox",
            Self::Status => "status",
            Self::Alert => "alert",
        };
        write!(f, "{name}")
    }
}

/// Central accessibility service that tracks state and queues announcements.
pub struct AccessibilityService {
    support: AccessibilitySupport,
    screen_reader_optimized: ScreenReaderOptimized,
    announcements: Vec<Announcement>,
}

impl AccessibilityService {
    pub fn new() -> Self {
        Self {
            support: AccessibilitySupport::Unknown,
            screen_reader_optimized: ScreenReaderOptimized(false),
            announcements: Vec::new(),
        }
    }

    pub fn set_support(&mut self, support: AccessibilitySupport) {
        self.support = support;
        self.screen_reader_optimized =
            ScreenReaderOptimized(support == AccessibilitySupport::Enabled);
    }

    pub fn get_support(&self) -> AccessibilitySupport {
        self.support
    }

    pub fn announce(&mut self, message: impl Into<String>, priority: AnnouncementPriority) {
        self.announcements.push(Announcement {
            message: message.into(),
            priority,
        });
    }

    pub fn is_screen_reader_optimized(&self) -> bool {
        self.screen_reader_optimized.0
    }

    /// Drain all pending announcements.
    pub fn take_announcements(&mut self) -> Vec<Announcement> {
        std::mem::take(&mut self.announcements)
    }

    /// Convenience method to announce a status update (Polite priority).
    pub fn announce_status(&mut self, message: impl Into<String>) {
        self.announcements.push(Announcement::polite(message));
    }

    /// Convenience method to announce an alert (Assertive priority).
    pub fn announce_alert(&mut self, message: impl Into<String>) {
        self.announcements.push(Announcement::assertive(message));
    }

    /// Return the number of pending announcements.
    pub fn announcement_count(&self) -> usize {
        self.announcements.len()
    }

    /// Peek at the most recent announcement, if any.
    pub fn last_announcement(&self) -> Option<&Announcement> {
        self.announcements.last()
    }

    /// Remove all pending announcements without returning them.
    pub fn clear_announcements(&mut self) {
        self.announcements.clear();
    }
}

impl Default for AccessibilityService {
    fn default() -> Self {
        Self::new()
    }
}

/// Verbosity level for accessibility output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    Low,
    Medium,
    High,
}

/// Configuration for accessibility behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibilityConfig {
    pub enabled: bool,
    pub verbosity: Verbosity,
    pub reduce_motion: bool,
}

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            verbosity: Verbosity::Medium,
            reduce_motion: false,
        }
    }
}

/// Describes an accessible element with role, label, and description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AriaDescription {
    pub role: AriaRole,
    pub label: String,
    pub description: String,
}

impl fmt::Display for AriaDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} \"{}\" - {}", self.role, self.label, self.description)
    }
}

// --- FocusTracker ---

/// Tracks focused element IDs in a focus chain.
#[derive(Debug, Clone)]
pub struct FocusTracker {
    pub focus_chain: Vec<String>,
    pub current_index: Option<usize>,
}

impl FocusTracker {
    pub fn new() -> Self {
        Self {
            focus_chain: Vec::new(),
            current_index: None,
        }
    }

    pub fn push(&mut self, id: String) {
        self.focus_chain.push(id);
        if self.current_index.is_none() {
            self.current_index = Some(0);
        }
    }

    pub fn focus_next(&mut self) -> Option<&str> {
        let len = self.focus_chain.len();
        if len == 0 {
            return None;
        }
        let idx = match self.current_index {
            Some(i) if i + 1 < len => i + 1,
            Some(i) => i,
            None => 0,
        };
        self.current_index = Some(idx);
        Some(&self.focus_chain[idx])
    }

    pub fn focus_previous(&mut self) -> Option<&str> {
        let len = self.focus_chain.len();
        if len == 0 {
            return None;
        }
        let idx = match self.current_index {
            Some(i) if i > 0 => i - 1,
            Some(_) => 0,
            None => 0,
        };
        self.current_index = Some(idx);
        Some(&self.focus_chain[idx])
    }

    pub fn current_focus(&self) -> Option<&str> {
        self.current_index
            .and_then(|i| self.focus_chain.get(i))
            .map(|s| s.as_str())
    }

    pub fn clear(&mut self) {
        self.focus_chain.clear();
        self.current_index = None;
    }

    pub fn len(&self) -> usize {
        self.focus_chain.len()
    }

    pub fn is_empty(&self) -> bool {
        self.focus_chain.is_empty()
    }

    pub fn remove(&mut self, id: &str) -> bool {
        if let Some(pos) = self.focus_chain.iter().position(|s| s == id) {
            self.focus_chain.remove(pos);
            if self.focus_chain.is_empty() {
                self.current_index = None;
            } else if let Some(ci) = self.current_index {
                if ci >= self.focus_chain.len() {
                    self.current_index = Some(self.focus_chain.len() - 1);
                }
            }
            true
        } else {
            false
        }
    }
}

impl Default for FocusTracker {
    fn default() -> Self {
        Self::new()
    }
}

// --- KeyboardNavigation ---

/// Tracks keyboard navigation state.
#[derive(Debug, Clone)]
pub struct KeyboardNavigation {
    pub tab_index: i32,
    pub trap_focus: bool,
    pub skip_links: Vec<String>,
}

impl KeyboardNavigation {
    pub fn new() -> Self {
        Self {
            tab_index: 0,
            trap_focus: false,
            skip_links: Vec::new(),
        }
    }

    pub fn set_tab_index(&mut self, index: i32) {
        self.tab_index = index;
    }

    pub fn add_skip_link(&mut self, link: String) {
        self.skip_links.push(link);
    }

    pub fn is_focus_trapped(&self) -> bool {
        self.trap_focus
    }

    pub fn set_trap_focus(&mut self, trapped: bool) {
        self.trap_focus = trapped;
    }

    pub fn skip_link_count(&self) -> usize {
        self.skip_links.len()
    }
}

impl Default for KeyboardNavigation {
    fn default() -> Self {
        Self::new()
    }
}

// --- Display impls ---

impl fmt::Display for Verbosity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
        }
    }
}

impl fmt::Display for AccessibilityConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AccessibilityConfig(enabled={}, verbosity={}, reduce_motion={})",
            self.enabled, self.verbosity, self.reduce_motion
        )
    }
}

// --- Extra methods ---

impl Announcement {
    /// Validate that the announcement message is non-empty.
    pub fn validate(&self) -> Result<(), AccessibilityError> {
        if self.message.is_empty() {
            Err(AccessibilityError::InvalidAnnouncement)
        } else {
            Ok(())
        }
    }
}

impl AccessibilityConfig {
    /// Builder-style method to set verbosity.
    pub fn with_verbosity(mut self, v: Verbosity) -> Self {
        self.verbosity = v;
        self
    }
}

// ---------------------------------------------------------------------------
// Announcement queue management, focus history, landmark registry, tree depth
// ---------------------------------------------------------------------------

impl AccessibilityService {
    /// Return only assertive announcements from the pending queue.
    pub fn assertive_announcements(&self) -> Vec<&Announcement> {
        self.announcements
            .iter()
            .filter(|a| a.priority == AnnouncementPriority::Assertive)
            .collect()
    }

    /// Return only polite announcements from the pending queue.
    pub fn polite_announcements(&self) -> Vec<&Announcement> {
        self.announcements
            .iter()
            .filter(|a| a.priority == AnnouncementPriority::Polite)
            .collect()
    }

    /// Drain announcements of a specific priority, leaving others.
    pub fn take_by_priority(&mut self, priority: AnnouncementPriority) -> Vec<Announcement> {
        let mut taken = Vec::new();
        let mut remaining = Vec::new();
        for ann in std::mem::take(&mut self.announcements) {
            if ann.priority == priority {
                taken.push(ann);
            } else {
                remaining.push(ann);
            }
        }
        self.announcements = remaining;
        taken
    }
}

/// Tracks the history of focused element IDs.
#[derive(Debug, Clone)]
pub struct FocusHistory {
    history: Vec<String>,
    max_size: usize,
}

impl FocusHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            history: Vec::new(),
            max_size: if max_size == 0 { 1 } else { max_size },
        }
    }

    /// Record a focus change to `element_id`.
    pub fn record(&mut self, element_id: impl Into<String>) {
        if self.history.len() >= self.max_size {
            self.history.remove(0);
        }
        self.history.push(element_id.into());
    }

    /// Return the most recently focused element.
    pub fn current(&self) -> Option<&str> {
        self.history.last().map(|s| s.as_str())
    }

    /// Return the previously focused element (one before current).
    pub fn previous(&self) -> Option<&str> {
        if self.history.len() >= 2 {
            Some(&self.history[self.history.len() - 2])
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }
}

/// Registry for ARIA landmark regions.
#[derive(Debug, Clone)]
pub struct LandmarkRegistry {
    landmarks: Vec<(String, AriaRole)>,
}

impl LandmarkRegistry {
    pub fn new() -> Self {
        Self {
            landmarks: Vec::new(),
        }
    }

    /// Register a landmark with the given label and role.
    pub fn register(&mut self, label: impl Into<String>, role: AriaRole) {
        self.landmarks.push((label.into(), role));
    }

    /// Remove a landmark by label. Returns true if found.
    pub fn unregister(&mut self, label: &str) -> bool {
        let before = self.landmarks.len();
        self.landmarks.retain(|(l, _)| l != label);
        self.landmarks.len() < before
    }

    /// Find landmarks by role.
    pub fn find_by_role(&self, role: AriaRole) -> Vec<&str> {
        self.landmarks
            .iter()
            .filter(|(_, r)| *r == role)
            .map(|(l, _)| l.as_str())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.landmarks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.landmarks.is_empty()
    }
}

impl Default for LandmarkRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A node in an accessibility tree for computing depth.
#[derive(Debug, Clone)]
pub struct AccessibilityNode {
    pub label: String,
    pub role: AriaRole,
    pub children: Vec<AccessibilityNode>,
}

impl AccessibilityNode {
    pub fn new(label: impl Into<String>, role: AriaRole) -> Self {
        Self {
            label: label.into(),
            role,
            children: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child: AccessibilityNode) {
        self.children.push(child);
    }

    /// Compute the maximum depth of this subtree (1 for a leaf).
    pub fn depth(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
        }
    }

    /// Count total nodes in this subtree (including self).
    pub fn node_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.node_count()).sum::<usize>()
    }
}

// ---------------------------------------------------------------------------
// AnnouncementQueue — FIFO queue for screen reader announcements
// ---------------------------------------------------------------------------

use std::collections::VecDeque;

/// A FIFO queue for delivering announcements to screen readers in order.
///
/// Unlike the Vec-based queue in [`AccessibilityService`], this provides
/// bounded capacity with overflow policy and peek/dequeue semantics.
#[derive(Debug, Clone)]
pub struct AnnouncementQueue {
    queue: VecDeque<Announcement>,
    capacity: usize,
}

impl AnnouncementQueue {
    /// Create a queue with the given maximum capacity.
    /// Capacity is clamped to at least 1.
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            capacity: capacity.max(1),
        }
    }

    /// Enqueue an announcement. If the queue is full, the oldest announcement
    /// is dropped to make room.
    pub fn enqueue(&mut self, announcement: Announcement) {
        if self.queue.len() >= self.capacity {
            self.queue.pop_front();
        }
        self.queue.push_back(announcement);
    }

    /// Dequeue the oldest announcement.
    pub fn dequeue(&mut self) -> Option<Announcement> {
        self.queue.pop_front()
    }

    /// Peek at the oldest announcement without removing it.
    pub fn peek(&self) -> Option<&Announcement> {
        self.queue.front()
    }

    /// Drain all announcements in FIFO order.
    pub fn drain_all(&mut self) -> Vec<Announcement> {
        self.queue.drain(..).collect()
    }

    /// Return the number of queued announcements.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Return `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Return `true` if the queue is at capacity.
    pub fn is_full(&self) -> bool {
        self.queue.len() >= self.capacity
    }

    /// Return the maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all queued announcements.
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Enqueue a polite announcement with the given message.
    pub fn enqueue_polite(&mut self, message: impl Into<String>) {
        self.enqueue(Announcement::polite(message));
    }

    /// Enqueue an assertive announcement with the given message.
    pub fn enqueue_assertive(&mut self, message: impl Into<String>) {
        self.enqueue(Announcement::assertive(message));
    }
}

// ---------------------------------------------------------------------------
// announce_change — construct accessible change descriptions
// ---------------------------------------------------------------------------

/// Construct an accessible description of a change event.
///
/// Combines the `role` of the widget, a human-readable `label`, and an
/// `action` verb (e.g. "expanded", "selected") into a string suitable for
/// screen reader announcement.
pub fn announce_change(role: AriaRole, label: &str, action: &str) -> Announcement {
    let message = format!("{} \"{}\": {}", role, label, action);
    Announcement::polite(message)
}

/// Construct an assertive change announcement for critical events.
pub fn announce_change_assertive(role: AriaRole, label: &str, action: &str) -> Announcement {
    let message = format!("{} \"{}\": {}", role, label, action);
    Announcement::assertive(message)
}

// ---------------------------------------------------------------------------
// AriaRole helpers
// ---------------------------------------------------------------------------

impl AriaRole {
    /// Returns all defined ARIA roles.
    pub fn all() -> &'static [AriaRole] {
        &[
            AriaRole::Button,
            AriaRole::Checkbox,
            AriaRole::Dialog,
            AriaRole::Grid,
            AriaRole::List,
            AriaRole::ListItem,
            AriaRole::Menu,
            AriaRole::MenuItem,
            AriaRole::Tab,
            AriaRole::TabPanel,
            AriaRole::Tree,
            AriaRole::TreeItem,
            AriaRole::TextBox,
            AriaRole::Status,
            AriaRole::Alert,
        ]
    }

    /// Returns `true` if this role is an interactive widget role.
    pub fn is_interactive(&self) -> bool {
        matches!(
            self,
            AriaRole::Button
                | AriaRole::Checkbox
                | AriaRole::Menu
                | AriaRole::MenuItem
                | AriaRole::Tab
                | AriaRole::TextBox
                | AriaRole::TreeItem
        )
    }

    /// Returns `true` if this role is a container role.
    pub fn is_container(&self) -> bool {
        matches!(
            self,
            AriaRole::Dialog
                | AriaRole::Grid
                | AriaRole::List
                | AriaRole::Menu
                | AriaRole::TabPanel
                | AriaRole::Tree
        )
    }

    /// Parse an ARIA role from its string representation.
    pub fn from_str(s: &str) -> Option<AriaRole> {
        match s {
            "button" => Some(AriaRole::Button),
            "checkbox" => Some(AriaRole::Checkbox),
            "dialog" => Some(AriaRole::Dialog),
            "grid" => Some(AriaRole::Grid),
            "list" => Some(AriaRole::List),
            "listitem" => Some(AriaRole::ListItem),
            "menu" => Some(AriaRole::Menu),
            "menuitem" => Some(AriaRole::MenuItem),
            "tab" => Some(AriaRole::Tab),
            "tabpanel" => Some(AriaRole::TabPanel),
            "tree" => Some(AriaRole::Tree),
            "treeitem" => Some(AriaRole::TreeItem),
            "textbox" => Some(AriaRole::TextBox),
            "status" => Some(AriaRole::Status),
            "alert" => Some(AriaRole::Alert),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// AccessibilityNode — tree traversal helpers
// ---------------------------------------------------------------------------

impl AccessibilityNode {
    /// Collect all leaf nodes (nodes with no children).
    pub fn leaves(&self) -> Vec<&AccessibilityNode> {
        let mut result = Vec::new();
        self.collect_leaves(&mut result);
        result
    }

    fn collect_leaves<'a>(&'a self, out: &mut Vec<&'a AccessibilityNode>) {
        if self.children.is_empty() {
            out.push(self);
        } else {
            for child in &self.children {
                child.collect_leaves(out);
            }
        }
    }

    /// Find the first node in the tree with the given role (depth-first).
    pub fn find_by_role(&self, role: AriaRole) -> Option<&AccessibilityNode> {
        if self.role == role {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_role(role) {
                return Some(found);
            }
        }
        None
    }

    /// Collect all node labels in depth-first order.
    pub fn all_labels(&self) -> Vec<&str> {
        let mut labels = Vec::new();
        self.collect_labels(&mut labels);
        labels
    }

    fn collect_labels<'a>(&'a self, out: &mut Vec<&'a str>) {
        out.push(&self.label);
        for child in &self.children {
            child.collect_labels(out);
        }
    }
}

// ---------------------------------------------------------------------------
// FocusTracker — wrap-around navigation
// ---------------------------------------------------------------------------

impl FocusTracker {
    /// Move focus to the next element, wrapping around to the first.
    pub fn focus_next_wrap(&mut self) -> Option<&str> {
        let len = self.focus_chain.len();
        if len == 0 {
            return None;
        }
        let idx = match self.current_index {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.current_index = Some(idx);
        Some(&self.focus_chain[idx])
    }

    /// Move focus to the previous element, wrapping around to the last.
    pub fn focus_previous_wrap(&mut self) -> Option<&str> {
        let len = self.focus_chain.len();
        if len == 0 {
            return None;
        }
        let idx = match self.current_index {
            Some(0) => len - 1,
            Some(i) => i - 1,
            None => len - 1,
        };
        self.current_index = Some(idx);
        Some(&self.focus_chain[idx])
    }

    /// Set focus to the element with the given id. Returns true if found.
    pub fn focus_by_id(&mut self, id: &str) -> bool {
        if let Some(pos) = self.focus_chain.iter().position(|s| s == id) {
            self.current_index = Some(pos);
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// ARIA role mapping to HTML element semantics
// ---------------------------------------------------------------------------

/// Maps an [`AriaRole`] to its corresponding default HTML element name.
///
/// Returns `None` for roles that have no single canonical HTML element.
pub fn role_to_html_element(role: AriaRole) -> Option<&'static str> {
    match role {
        AriaRole::Button => Some("button"),
        AriaRole::Checkbox => Some("input[type=checkbox]"),
        AriaRole::TextBox => Some("input[type=text]"),
        AriaRole::List => Some("ul"),
        AriaRole::ListItem => Some("li"),
        AriaRole::Menu => Some("menu"),
        AriaRole::Dialog => Some("dialog"),
        AriaRole::Alert => Some("div[role=alert]"),
        _ => None,
    }
}

/// Returns whether the given role should be focusable by default.
pub fn is_default_focusable(role: AriaRole) -> bool {
    matches!(
        role,
        AriaRole::Button
            | AriaRole::Checkbox
            | AriaRole::TextBox
            | AriaRole::MenuItem
            | AriaRole::Tab
            | AriaRole::TreeItem
    )
}

// ---------------------------------------------------------------------------
// Screen reader announcement formatting
// ---------------------------------------------------------------------------

/// Formats a screen reader announcement string with context breadcrumbs.
///
/// Produces output like: `"[Editor > File > main.rs] Cursor moved to line 42"`
pub fn format_announcement_with_context(context_path: &[&str], message: &str) -> String {
    if context_path.is_empty() {
        return message.to_string();
    }
    format!("[{}] {}", context_path.join(" > "), message)
}

/// Formats a count announcement (e.g., "3 errors", "1 warning", "No results").
pub fn format_count_announcement(label: &str, count: usize) -> String {
    match count {
        0 => format!("No {label}s"),
        1 => format!("1 {label}"),
        n => format!("{n} {label}s"),
    }
}

// ---------------------------------------------------------------------------
// Contrast ratio calculation (WCAG 2.1)
// ---------------------------------------------------------------------------

/// Compute the relative luminance of an sRGB colour channel (0–255).
fn channel_luminance(c: u8) -> f64 {
    let s = c as f64 / 255.0;
    if s <= 0.03928 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// Compute the relative luminance of an sRGB colour (each channel 0–255).
pub fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    0.2126 * channel_luminance(r) + 0.7152 * channel_luminance(g) + 0.0722 * channel_luminance(b)
}

/// Compute the WCAG 2.1 contrast ratio between two colours.
///
/// Returns a value in the range [1.0, 21.0].
pub fn contrast_ratio(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> f64 {
    let l1 = relative_luminance(r1, g1, b1);
    let l2 = relative_luminance(r2, g2, b2);
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// WCAG conformance level for a contrast ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WcagConformance {
    /// Fails both AA and AAA.
    Fail,
    /// Passes AA for normal text (≥ 4.5:1).
    AA,
    /// Passes AAA for normal text (≥ 7:1).
    AAA,
}

/// Evaluate the WCAG conformance level for a given contrast ratio.
pub fn evaluate_contrast(ratio: f64) -> WcagConformance {
    if ratio >= 7.0 {
        WcagConformance::AAA
    } else if ratio >= 4.5 {
        WcagConformance::AA
    } else {
        WcagConformance::Fail
    }
}

// ---------------------------------------------------------------------------
// Accessibility tree traversal
// ---------------------------------------------------------------------------

impl AccessibilityNode {
    /// Collect all nodes matching a predicate in depth-first order.
    pub fn find_all<F>(&self, predicate: F) -> Vec<&AccessibilityNode>
    where
        F: Fn(&AccessibilityNode) -> bool,
    {
        let mut result = Vec::new();
        self.collect_matching(&predicate, &mut result);
        result
    }

    fn collect_matching<'a, F>(&'a self, predicate: &F, out: &mut Vec<&'a AccessibilityNode>)
    where
        F: Fn(&AccessibilityNode) -> bool,
    {
        if predicate(self) {
            out.push(self);
        }
        for child in &self.children {
            child.collect_matching(predicate, out);
        }
    }

    /// Compute the path (list of labels) from the root to the first node
    /// matching the given label. Returns `None` if not found.
    pub fn path_to(&self, target_label: &str) -> Option<Vec<String>> {
        let mut path = Vec::new();
        if self.path_to_inner(target_label, &mut path) {
            Some(path)
        } else {
            None
        }
    }

    fn path_to_inner(&self, target: &str, path: &mut Vec<String>) -> bool {
        path.push(self.label.clone());
        if self.label == target {
            return true;
        }
        for child in &self.children {
            if child.path_to_inner(target, path) {
                return true;
            }
        }
        path.pop();
        false
    }
}

impl AccessibilityService {
    /// Return all announcements without draining them.
    pub fn peek_announcements(&self) -> &[Announcement] {
        &self.announcements
    }

    /// Return true if there are any assertive announcements pending.
    pub fn has_assertive_pending(&self) -> bool {
        self.announcements
            .iter()
            .any(|a| matches!(a.priority, AnnouncementPriority::Assertive))
    }

    /// Return the total number of characters across all pending announcements.
    pub fn total_announcement_chars(&self) -> usize {
        self.announcements.iter().map(|a| a.message.len()).sum()
    }
}

impl FocusTracker {
    /// Return the ids of all tracked elements.
    pub fn all_ids(&self) -> Vec<&str> {
        self.focus_chain.iter().map(|s| s.as_str()).collect()
    }

    /// Return the index of the currently focused element, if any.
    pub fn focused_index(&self) -> Option<usize> {
        self.current_index
    }
}

impl AnnouncementQueue {
    /// Return the number of remaining slots before the queue is full.
    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.len())
    }

    /// Return all messages as strings without draining.
    pub fn peek_messages(&self) -> Vec<&str> {
        self.queue.iter().map(|a| a.message.as_str()).collect()
    }
}

impl AccessibilityNode {
    /// Return the number of direct children.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Return true if this node has no children (is a leaf).
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Return the role of this node.
    pub fn role(&self) -> AriaRole {
        self.role
    }
}

/// Format a summary of focus tracker state for screen readers.
pub fn format_focus_summary(tracker: &FocusTracker) -> String {
    match tracker.current_focus() {
        Some(id) => format!("Focused on {} ({} of {})", id, tracker.focused_index().unwrap_or(0) + 1, tracker.len()),
        None => "No element focused".to_string(),
    }
}

/// Build a textual description of an accessibility tree suitable for debugging.
pub fn describe_tree(node: &AccessibilityNode, indent: usize) -> String {
    let mut out = String::new();
    let prefix = " ".repeat(indent * 2);
    out.push_str(&format!("{}{} \"{}\"\n", prefix, node.role, node.label));
    for child in &node.children {
        out.push_str(&describe_tree(child, indent + 1));
    }
    out
}

/// Count the total number of interactive roles in an accessibility tree.
pub fn count_interactive_nodes(node: &AccessibilityNode) -> usize {
    let self_count = if node.role.is_interactive() { 1 } else { 0 };
    self_count + node.children.iter().map(count_interactive_nodes).sum::<usize>()
}

impl AccessibilityConfig {
    /// Return true if announcements should be verbose (High verbosity).
    pub fn is_verbose(&self) -> bool {
        self.verbosity == Verbosity::High
    }

    /// Return true if announcements should be minimal (Low verbosity).
    pub fn is_minimal(&self) -> bool {
        self.verbosity == Verbosity::Low
    }
}

impl KeyboardNavigation {
    /// Return true if there are any skip links configured.
    pub fn has_skip_links(&self) -> bool {
        !self.skip_links.is_empty()
    }

    /// Clear all skip links.
    pub fn clear_skip_links(&mut self) {
        self.skip_links.clear();
    }

    /// Return the skip links as a comma-separated string.
    pub fn skip_links_display(&self) -> String {
        self.skip_links.join(", ")
    }
}

impl LandmarkRegistry {
    /// Return all registered landmark labels.
    pub fn all_labels(&self) -> Vec<&str> {
        self.landmarks.iter().map(|(l, _)| l.as_str()).collect()
    }

    /// Return all unique roles that have landmarks registered.
    pub fn unique_roles(&self) -> Vec<AriaRole> {
        let mut roles: Vec<AriaRole> = self.landmarks.iter().map(|(_, r)| *r).collect();
        roles.sort_by_key(|r| format!("{:?}", r));
        roles.dedup();
        roles
    }
}

impl FocusHistory {
    /// Return the full history as a slice.
    pub fn entries(&self) -> &[String] {
        &self.history
    }

    /// Return the number of unique elements that have been focused.
    pub fn unique_count(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        for h in &self.history {
            seen.insert(h.as_str());
        }
        seen.len()
    }
}

// ---------------------------------------------------------------------------
// A11yAnnouncer – priority-queue announcement system
// ---------------------------------------------------------------------------

/// An announcer that queues announcements by priority and delivers them in order.
#[derive(Debug, Clone)]
pub struct A11yAnnouncer {
    queue: Vec<Announcement>,
    max_queue: usize,
}

impl A11yAnnouncer {
    pub fn new(max_queue: usize) -> Self {
        Self {
            queue: Vec::new(),
            max_queue,
        }
    }

    /// Enqueue an announcement. Assertive announcements go to the front.
    pub fn enqueue(&mut self, announcement: Announcement) {
        if self.queue.len() >= self.max_queue {
            // Drop oldest polite announcement
            if let Some(pos) = self.queue.iter().position(|a| a.priority == AnnouncementPriority::Polite) {
                self.queue.remove(pos);
            } else {
                self.queue.remove(0);
            }
        }
        match announcement.priority {
            AnnouncementPriority::Assertive => self.queue.insert(0, announcement),
            AnnouncementPriority::Polite => self.queue.push(announcement),
        }
    }

    /// Dequeue the next announcement (highest priority first).
    pub fn dequeue(&mut self) -> Option<Announcement> {
        if self.queue.is_empty() {
            None
        } else {
            Some(self.queue.remove(0))
        }
    }

    /// Peek at the next announcement without removing it.
    pub fn peek(&self) -> Option<&Announcement> {
        self.queue.first()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Count of assertive announcements in the queue.
    pub fn assertive_count(&self) -> usize {
        self.queue.iter().filter(|a| a.priority == AnnouncementPriority::Assertive).count()
    }
}

impl Default for A11yAnnouncer {
    fn default() -> Self {
        Self::new(50)
    }
}

impl fmt::Display for A11yAnnouncer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A11yAnnouncer({} queued, {} assertive)", self.len(), self.assertive_count())
    }
}

// ---------------------------------------------------------------------------
// A11yNavigator – landmark navigation
// ---------------------------------------------------------------------------

/// A landmark region for accessibility navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A11yLandmark {
    pub role: AriaRole,
    pub label: String,
    pub id: String,
}

impl fmt::Display for A11yLandmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Landmark({}: {})", self.role, self.label)
    }
}

/// Navigates between landmarks in the UI.
#[derive(Debug, Clone)]
pub struct A11yNavigator {
    landmarks: Vec<A11yLandmark>,
    current: Option<usize>,
}

impl A11yNavigator {
    pub fn new() -> Self {
        Self { landmarks: Vec::new(), current: None }
    }

    pub fn add_landmark(&mut self, landmark: A11yLandmark) {
        self.landmarks.push(landmark);
    }

    /// Navigate to the next landmark.
    pub fn next(&mut self) -> Option<&A11yLandmark> {
        if self.landmarks.is_empty() {
            return None;
        }
        let idx = match self.current {
            Some(i) => (i + 1) % self.landmarks.len(),
            None => 0,
        };
        self.current = Some(idx);
        Some(&self.landmarks[idx])
    }

    /// Navigate to the previous landmark.
    pub fn previous(&mut self) -> Option<&A11yLandmark> {
        if self.landmarks.is_empty() {
            return None;
        }
        let idx = match self.current {
            Some(0) | None => self.landmarks.len() - 1,
            Some(i) => i - 1,
        };
        self.current = Some(idx);
        Some(&self.landmarks[idx])
    }

    /// Current landmark.
    pub fn current(&self) -> Option<&A11yLandmark> {
        self.current.and_then(|i| self.landmarks.get(i))
    }

    /// Find landmarks by role.
    pub fn find_by_role(&self, role: &AriaRole) -> Vec<&A11yLandmark> {
        self.landmarks.iter().filter(|l| l.role == *role).collect()
    }

    pub fn landmark_count(&self) -> usize {
        self.landmarks.len()
    }

    pub fn clear(&mut self) {
        self.landmarks.clear();
        self.current = None;
    }
}

impl Default for A11yNavigator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for A11yNavigator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A11yNavigator({} landmarks)", self.landmarks.len())
    }
}

// ---------------------------------------------------------------------------
// A11yContrastChecker – WCAG contrast ratio calculation
// ---------------------------------------------------------------------------

/// Checks color contrast ratios per WCAG 2.1 guidelines.
pub struct A11yContrastChecker;

impl A11yContrastChecker {
    /// Compute the relative luminance of an sRGB color (0-255 per channel).
    pub fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
        fn linearize(c: u8) -> f64 {
            let s = c as f64 / 255.0;
            if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        }
        0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
    }

    /// Compute the contrast ratio between two colors.
    pub fn contrast_ratio(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> f64 {
        let l1 = Self::relative_luminance(r1, g1, b1);
        let l2 = Self::relative_luminance(r2, g2, b2);
        let lighter = l1.max(l2);
        let darker = l1.min(l2);
        (lighter + 0.05) / (darker + 0.05)
    }

    /// Whether the contrast ratio meets WCAG AA for normal text (>= 4.5:1).
    pub fn meets_aa(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> bool {
        Self::contrast_ratio(r1, g1, b1, r2, g2, b2) >= 4.5
    }

    /// Whether the contrast ratio meets WCAG AAA for normal text (>= 7.0:1).
    pub fn meets_aaa(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> bool {
        Self::contrast_ratio(r1, g1, b1, r2, g2, b2) >= 7.0
    }

    /// Whether the contrast ratio meets WCAG AA for large text (>= 3.0:1).
    pub fn meets_aa_large(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> bool {
        Self::contrast_ratio(r1, g1, b1, r2, g2, b2) >= 3.0
    }
}

// ---------------------------------------------------------------------------
// Screen reader text builder
// ---------------------------------------------------------------------------

/// Builds descriptive text for screen reader consumption.
#[derive(Debug, Clone)]
pub struct ScreenReaderTextBuilder {
    parts: Vec<String>,
    separator: String,
}

impl ScreenReaderTextBuilder {
    pub fn new() -> Self {
        Self { parts: Vec::new(), separator: ", ".into() }
    }

    pub fn with_separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    /// Add a text segment.
    pub fn add(&mut self, text: impl Into<String>) -> &mut Self {
        let t = text.into();
        if !t.is_empty() {
            self.parts.push(t);
        }
        self
    }

    /// Add a labeled value: "Label: value".
    pub fn add_labeled(&mut self, label: &str, value: &str) -> &mut Self {
        if !value.is_empty() {
            self.parts.push(format!("{label}: {value}"));
        }
        self
    }

    /// Build the final text.
    pub fn build(&self) -> String {
        self.parts.join(&self.separator)
    }

    pub fn part_count(&self) -> usize {
        self.parts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    pub fn clear(&mut self) {
        self.parts.clear();
    }
}

impl Default for ScreenReaderTextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ScreenReaderTextBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.build())
    }
}

// ---------------------------------------------------------------------------
// AuditSeverity
// ---------------------------------------------------------------------------

/// Severity of an accessibility audit finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuditSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for AuditSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// A single finding from an accessibility audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFinding {
    pub severity: AuditSeverity,
    pub rule_id: String,
    pub message: String,
    pub element_path: String,
    pub suggestion: Option<String>,
}

impl AuditFinding {
    pub fn new(severity: AuditSeverity, rule_id: &str, message: &str, element_path: &str) -> Self {
        Self {
            severity,
            rule_id: rule_id.to_string(),
            message: message.to_string(),
            element_path: element_path.to_string(),
            suggestion: None,
        }
    }

    pub fn with_suggestion(mut self, suggestion: &str) -> Self {
        self.suggestion = Some(suggestion.to_string());
        self
    }

    pub fn is_critical(&self) -> bool {
        self.severity == AuditSeverity::Critical
    }

    pub fn format_report_line(&self) -> String {
        let sug = self.suggestion.as_deref().unwrap_or("N/A");
        format!(
            "[{}] {} at {}: {} (suggestion: {})",
            self.severity, self.rule_id, self.element_path, self.message, sug
        )
    }
}

impl fmt::Display for AuditFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.rule_id, self.message)
    }
}

/// Collects accessibility audit findings and generates reports.
#[derive(Debug, Clone)]
pub struct AuditReporter {
    findings: Vec<AuditFinding>,
    scope: String,
}

impl AuditReporter {
    pub fn new(scope: &str) -> Self {
        Self { findings: Vec::new(), scope: scope.to_string() }
    }

    pub fn add_finding(&mut self, finding: AuditFinding) {
        self.findings.push(finding);
    }

    pub fn finding_count(&self) -> usize { self.findings.len() }

    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity >= AuditSeverity::Error)
    }

    pub fn has_critical(&self) -> bool {
        self.findings.iter().any(|f| f.is_critical())
    }

    pub fn findings_by_severity(&self, severity: AuditSeverity) -> Vec<&AuditFinding> {
        self.findings.iter().filter(|f| f.severity == severity).collect()
    }

    pub fn error_count(&self) -> usize {
        self.findings.iter().filter(|f| f.severity >= AuditSeverity::Error).count()
    }

    pub fn warning_count(&self) -> usize {
        self.findings.iter().filter(|f| f.severity == AuditSeverity::Warning).count()
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "Audit scope: {} | Total: {} | Errors: {} | Warnings: {}",
            self.scope, self.findings.len(), self.error_count(), self.warning_count()
        )
    }

    pub fn generate_full_report(&self) -> String {
        let mut lines = vec![self.generate_summary()];
        for f in &self.findings {
            lines.push(f.format_report_line());
        }
        lines.join("\n")
    }

    pub fn clear(&mut self) { self.findings.clear(); }

    pub fn sorted_findings(&self) -> Vec<&AuditFinding> {
        let mut sorted: Vec<&AuditFinding> = self.findings.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }
}

/// A keyboard shortcut with modifier keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyboardShortcut {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl KeyboardShortcut {
    pub fn new(key: &str) -> Self {
        Self { key: key.to_string(), ctrl: false, shift: false, alt: false, meta: false }
    }
    pub fn with_ctrl(mut self) -> Self { self.ctrl = true; self }
    pub fn with_shift(mut self) -> Self { self.shift = true; self }
    pub fn with_alt(mut self) -> Self { self.alt = true; self }
    pub fn with_meta(mut self) -> Self { self.meta = true; self }

    pub fn modifier_count(&self) -> u8 {
        self.ctrl as u8 + self.shift as u8 + self.alt as u8 + self.meta as u8
    }

    pub fn format_announcement(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl { parts.push("Ctrl"); }
        if self.shift { parts.push("Shift"); }
        if self.alt { parts.push("Alt"); }
        if self.meta { parts.push("Meta"); }
        parts.push(&self.key);
        parts.join("+")
    }

    pub fn is_simple(&self) -> bool { self.modifier_count() == 0 }
}

impl fmt::Display for KeyboardShortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_announcement())
    }
}

/// Queues and announces keyboard shortcuts for screen reader users.
#[derive(Debug, Clone)]
pub struct ShortcutAnnouncer {
    announcements: Vec<String>,
    prefix: String,
    max_queue: usize,
}

impl ShortcutAnnouncer {
    pub fn new() -> Self {
        Self { announcements: Vec::new(), prefix: "Shortcut: ".to_string(), max_queue: 50 }
    }
    pub fn with_prefix(mut self, prefix: &str) -> Self { self.prefix = prefix.to_string(); self }
    pub fn with_max_queue(mut self, max: usize) -> Self { self.max_queue = max; self }

    pub fn announce(&mut self, shortcut: &KeyboardShortcut, action: &str) {
        let msg = format!("{}{} - {}", self.prefix, shortcut.format_announcement(), action);
        if self.announcements.len() >= self.max_queue {
            self.announcements.remove(0);
        }
        self.announcements.push(msg);
    }

    pub fn pending_count(&self) -> usize { self.announcements.len() }

    pub fn drain_announcements(&mut self) -> Vec<String> {
        std::mem::take(&mut self.announcements)
    }

    pub fn last_announcement(&self) -> Option<&str> {
        self.announcements.last().map(|s| s.as_str())
    }

    pub fn clear(&mut self) { self.announcements.clear(); }
}

impl Default for ShortcutAnnouncer {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// AriaRole helpers
// ---------------------------------------------------------------------------

impl AriaRole {
    /// Returns `true` if the role represents an interactive widget (v2).
    pub fn is_interactive_v2(&self) -> bool {
        matches!(
            self,
            AriaRole::Button
                | AriaRole::Checkbox
                | AriaRole::TextBox
                | AriaRole::Tab
                | AriaRole::MenuItem
        )
    }

    /// Returns `true` if the role acts as a structural container (v2).
    pub fn is_container_v2(&self) -> bool {
        matches!(
            self,
            AriaRole::List
                | AriaRole::Grid
                | AriaRole::Tree
                | AriaRole::TreeItem
                | AriaRole::Menu
                | AriaRole::Dialog
        )
    }

    /// Returns a short label a screen reader might use for this role.
    pub fn screen_reader_label(&self) -> &'static str {
        match self {
            AriaRole::Button => "button",
            AriaRole::Checkbox => "check box",
            AriaRole::TextBox => "edit",
            AriaRole::Tab => "tab",
            AriaRole::List => "list",
            AriaRole::ListItem => "list item",
            AriaRole::Grid => "grid",
            AriaRole::Tree => "tree",
            AriaRole::TreeItem => "tree item",
            AriaRole::Menu => "menu",
            AriaRole::MenuItem => "menu item",
            AriaRole::TabPanel => "tab panel",
            AriaRole::Dialog => "dialog",
            AriaRole::Alert => "alert",
            AriaRole::Status => "status",
        }
    }
}

// ---------------------------------------------------------------------------
// AccessibleDescriptionBuilder — fluent API for building accessible labels
// ---------------------------------------------------------------------------

/// Fluent builder for rich accessible descriptions.
#[derive(Debug, Clone)]
pub struct AccessibleDescriptionBuilder {
    role: Option<AriaRole>,
    label: Option<String>,
    states: Vec<String>,
    value: Option<String>,
}

impl AccessibleDescriptionBuilder {
    pub fn new() -> Self {
        Self { role: None, label: None, states: Vec::new(), value: None }
    }

    pub fn add_role(mut self, role: AriaRole) -> Self {
        self.role = Some(role);
        self
    }

    pub fn add_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn add_state(mut self, state: impl Into<String>) -> Self {
        self.states.push(state.into());
        self
    }

    pub fn add_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Build the full description string.
    pub fn build(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(ref label) = self.label {
            parts.push(label.clone());
        }
        if let Some(role) = &self.role {
            parts.push(role.screen_reader_label().to_string());
        }
        for s in &self.states {
            parts.push(s.clone());
        }
        if let Some(ref v) = self.value {
            parts.push(format!("value: {v}"));
        }
        parts.join(", ")
    }
}

impl Default for AccessibleDescriptionBuilder {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// FocusOrderTracker — ordered focus ring for keyboard navigation
// ---------------------------------------------------------------------------

/// Tracks an ordered list of focusable element IDs and the current index.
#[derive(Debug, Clone)]
pub struct FocusOrderTracker {
    ids: Vec<String>,
    interactive_flags: Vec<bool>,
    current_index: Option<usize>,
}

impl FocusOrderTracker {
    pub fn new() -> Self {
        Self { ids: Vec::new(), interactive_flags: Vec::new(), current_index: None }
    }

    /// Register an element as focusable. `interactive` marks it as a widget.
    pub fn register(&mut self, id: impl Into<String>, interactive: bool) {
        self.ids.push(id.into());
        self.interactive_flags.push(interactive);
    }

    /// Unregister by id, adjusting current index.
    pub fn unregister(&mut self, id: &str) {
        if let Some(pos) = self.ids.iter().position(|s| s == id) {
            self.ids.remove(pos);
            self.interactive_flags.remove(pos);
            match self.current_index {
                Some(ci) if ci == pos => {
                    self.current_index = if self.ids.is_empty() { None } else { Some(ci.min(self.ids.len() - 1)) };
                }
                Some(ci) if ci > pos => self.current_index = Some(ci - 1),
                _ => {}
            }
        }
    }

    /// Move focus forward, wrapping around.
    pub fn focus_next(&mut self) -> Option<&str> {
        if self.ids.is_empty() { return None; }
        let next = match self.current_index {
            Some(i) => (i + 1) % self.ids.len(),
            None => 0,
        };
        self.current_index = Some(next);
        Some(&self.ids[next])
    }

    /// Move focus backward, wrapping around.
    pub fn focus_prev(&mut self) -> Option<&str> {
        if self.ids.is_empty() { return None; }
        let prev = match self.current_index {
            Some(0) => self.ids.len() - 1,
            Some(i) => i - 1,
            None => self.ids.len() - 1,
        };
        self.current_index = Some(prev);
        Some(&self.ids[prev])
    }

    /// Currently focused element id.
    pub fn current(&self) -> Option<&str> {
        self.current_index.map(|i| self.ids[i].as_str())
    }

    /// Return all ids that are marked interactive.
    pub fn interactive_elements(&self) -> Vec<&str> {
        self.ids.iter().zip(self.interactive_flags.iter())
            .filter_map(|(id, &f)| if f { Some(id.as_str()) } else { None })
            .collect()
    }

    /// Total registered elements.
    pub fn len(&self) -> usize { self.ids.len() }

    pub fn is_empty(&self) -> bool { self.ids.is_empty() }
}

impl Default for FocusOrderTracker {
    fn default() -> Self { Self::new() }
}


/// Configuration manager for a11y functionality.
pub struct A11yConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl A11yConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &A11yConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for a11y operations.
pub struct A11yRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl A11yRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for a11y.
pub struct A11yValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl A11yValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &A11yValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
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
// xa_ extended helpers for a11y
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaA11yRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaA11yRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaA11yCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaA11yCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaA11yCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 2
// ---------------------------------------------------------------------------

/// Generic object pool `Xc2Pool<T>`.
pub struct Xc2Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc2Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc2PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc2Pool<T> {
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
    pub fn stats(&self) -> Xc2PoolStats {
        Xc2PoolStats {
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

impl<T> Default for Xc2Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc2Scheduler`.
pub struct Xc2Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc2Scheduler {
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

impl Default for Xc2Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_2 hash for the given byte slice.
pub fn xc_2_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_2 convention.
pub fn xc_2_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe6 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe6Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe6PipelineError {
    pub stage: Xe6Stage,
    pub message: String,
}

impl std::fmt::Display for Xe6PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe6Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe6Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe6PipelineError>>>,
    stage_names: Vec<Xe6Stage>,
}

impl Xe6Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe6PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe6Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe6PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe6Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe6PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe6Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe6PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe6Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe6PipelineError> {
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

    pub fn compose(mut self, other: Xe6Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe6CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe6CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe6Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe6CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe6CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe6Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe6CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_6_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe6CacheEntry {
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

    fn xe_6_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe6CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_6_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe6PipelineError> {
    Ok(data)
}

pub fn xe_6_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe6PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_6_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe6PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_6_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe6PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_6_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe6PipelineError> {
    Err(Xe6PipelineError {
        stage: Xe6Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #69
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf69Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf69TrieNode {
    children: std::collections::HashMap<char, Xf69TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf69Trie {
    root: Xf69TrieNode,
    count: usize,
}

impl Xf69Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf69TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf69TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf69TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf69BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf69BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 1).
pub struct Xh1SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh1SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 43 as u64,
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

/// A compact bit set supporting boolean operations (variant 1).
pub struct Xh1BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh1BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 1).
pub struct Xi1Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi1Deque<T> {
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
pub struct Xi1Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi1Interval {
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

/// A simple interval tree (variant 1).
pub struct Xi1IntervalTree {
    xi_intervals: Vec<Xi1Interval>,
}

impl Xi1IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi1Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi1Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi1Interval) -> Vec<&Xi1Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi1Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi1Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi1Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi1Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi1Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi1Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 1) ---

/// Disjoint set / union-find for crate 1.
pub struct Xj1UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj1UnionFind {
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

const XJ1_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 1.
pub struct Xj1BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj1BTreeNode<K, V>>>,
    len: usize,
}

struct Xj1BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj1BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj1BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ1_BTREE_ORDER - 1
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
        let mid = XJ1_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj1BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj1BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj1BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj1BTreeNode::xj_new_leaf();
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


// --- xk_0 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk0SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk0SegmentTree {
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
pub struct Xk0DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk0DisjointIntervals {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_support_is_unknown() {
        let svc = AccessibilityService::new();
        assert_eq!(svc.get_support(), AccessibilitySupport::Unknown);
        assert!(!svc.is_screen_reader_optimized());
    }

    #[test]
    fn enabling_support_activates_screen_reader() {
        let mut svc = AccessibilityService::new();
        svc.set_support(AccessibilitySupport::Enabled);
        assert_eq!(svc.get_support(), AccessibilitySupport::Enabled);
        assert!(svc.is_screen_reader_optimized());

        svc.set_support(AccessibilitySupport::Disabled);
        assert!(!svc.is_screen_reader_optimized());
    }

    #[test]
    fn announcements_are_queued_and_drained() {
        let mut svc = AccessibilityService::new();
        svc.announce("File saved", AnnouncementPriority::Polite);
        svc.announce("Error occurred", AnnouncementPriority::Assertive);

        let announcements = svc.take_announcements();
        assert_eq!(announcements.len(), 2);
        assert_eq!(announcements[0].message, "File saved");
        assert_eq!(announcements[0].priority, AnnouncementPriority::Polite);
        assert_eq!(announcements[1].priority, AnnouncementPriority::Assertive);

        assert!(svc.take_announcements().is_empty());
    }

    #[test]
    fn aria_roles_are_distinct() {
        assert_ne!(AriaRole::Button, AriaRole::TextBox);
        assert_eq!(AriaRole::Alert, AriaRole::Alert);
    }

    #[test]
    fn announcement_polite_constructor() {
        let a = Announcement::polite("hello");
        assert_eq!(a.message, "hello");
        assert_eq!(a.priority, AnnouncementPriority::Polite);
    }

    #[test]
    fn announcement_assertive_constructor() {
        let a = Announcement::assertive("danger");
        assert_eq!(a.message, "danger");
        assert_eq!(a.priority, AnnouncementPriority::Assertive);
    }

    #[test]
    fn announce_status_and_alert() {
        let mut svc = AccessibilityService::new();
        svc.announce_status("Saved");
        svc.announce_alert("Crash");
        let items = svc.take_announcements();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].priority, AnnouncementPriority::Polite);
        assert_eq!(items[0].message, "Saved");
        assert_eq!(items[1].priority, AnnouncementPriority::Assertive);
        assert_eq!(items[1].message, "Crash");
    }

    #[test]
    fn announcement_count_and_last() {
        let mut svc = AccessibilityService::new();
        assert_eq!(svc.announcement_count(), 0);
        assert!(svc.last_announcement().is_none());

        svc.announce_status("one");
        svc.announce_alert("two");
        assert_eq!(svc.announcement_count(), 2);
        assert_eq!(svc.last_announcement().unwrap().message, "two");
    }

    #[test]
    fn clear_announcements_removes_all() {
        let mut svc = AccessibilityService::new();
        svc.announce_status("a");
        svc.announce_alert("b");
        assert_eq!(svc.announcement_count(), 2);
        svc.clear_announcements();
        assert_eq!(svc.announcement_count(), 0);
        assert!(svc.last_announcement().is_none());
    }

    #[test]
    fn display_accessibility_support() {
        assert_eq!(format!("{}", AccessibilitySupport::Unknown), "Unknown");
        assert_eq!(format!("{}", AccessibilitySupport::Disabled), "Disabled");
        assert_eq!(format!("{}", AccessibilitySupport::Enabled), "Enabled");
    }

    #[test]
    fn display_announcement_priority() {
        assert_eq!(format!("{}", AnnouncementPriority::Polite), "Polite");
        assert_eq!(format!("{}", AnnouncementPriority::Assertive), "Assertive");
    }

    #[test]
    fn display_aria_role() {
        assert_eq!(format!("{}", AriaRole::Button), "button");
        assert_eq!(format!("{}", AriaRole::TextBox), "textbox");
        assert_eq!(format!("{}", AriaRole::Alert), "alert");
    }

    #[test]
    fn config_defaults() {
        let cfg = AccessibilityConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.verbosity, Verbosity::Medium);
        assert!(!cfg.reduce_motion);
    }

    #[test]
    fn aria_description_display() {
        let desc = AriaDescription {
            role: AriaRole::Button,
            label: "Save".to_string(),
            description: "Save the current document".to_string(),
        };
        assert_eq!(format!("{desc}"), "button \"Save\" - Save the current document");
    }

    #[test]
    fn error_display() {
        assert_eq!(
            format!("{}", AccessibilityError::ServiceDisabled),
            "accessibility service is disabled"
        );
        assert_eq!(
            format!("{}", AccessibilityError::InvalidAnnouncement),
            "invalid announcement"
        );
        assert_eq!(
            format!("{}", AccessibilityError::RoleNotSupported(AriaRole::Grid)),
            "role not supported: grid"
        );
    }

    #[test]
    fn test_focus_tracker_new_empty() {
        let ft = FocusTracker::new();
        assert!(ft.is_empty());
        assert_eq!(ft.len(), 0);
        assert!(ft.current_focus().is_none());
    }

    #[test]
    fn test_focus_tracker_push_and_navigate() {
        let mut ft = FocusTracker::new();
        ft.push("a".to_string());
        ft.push("b".to_string());
        ft.push("c".to_string());
        assert_eq!(ft.len(), 3);
        assert_eq!(ft.current_focus(), Some("a"));
        assert_eq!(ft.focus_next(), Some("b"));
        assert_eq!(ft.focus_next(), Some("c"));
        assert_eq!(ft.current_focus(), Some("c"));
    }

    #[test]
    fn test_focus_tracker_focus_next_wraps() {
        let mut ft = FocusTracker::new();
        ft.push("x".to_string());
        ft.push("y".to_string());
        // Move to last element
        ft.focus_next();
        // Calling focus_next again should NOT wrap, returns last
        assert_eq!(ft.focus_next(), Some("y"));
    }

    #[test]
    fn test_focus_tracker_focus_previous_clamps() {
        let mut ft = FocusTracker::new();
        ft.push("a".to_string());
        ft.push("b".to_string());
        // Already at index 0
        assert_eq!(ft.focus_previous(), Some("a"));
        assert_eq!(ft.focus_previous(), Some("a"));
    }

    #[test]
    fn test_focus_tracker_remove() {
        let mut ft = FocusTracker::new();
        ft.push("a".to_string());
        ft.push("b".to_string());
        ft.push("c".to_string());
        assert!(ft.remove("b"));
        assert_eq!(ft.len(), 2);
        assert!(!ft.remove("z"));
    }

    #[test]
    fn test_focus_tracker_clear() {
        let mut ft = FocusTracker::new();
        ft.push("a".to_string());
        ft.push("b".to_string());
        ft.clear();
        assert!(ft.is_empty());
        assert!(ft.current_focus().is_none());
    }

    #[test]
    fn test_keyboard_nav_defaults() {
        let kn = KeyboardNavigation::new();
        assert_eq!(kn.tab_index, 0);
        assert!(!kn.is_focus_trapped());
        assert_eq!(kn.skip_link_count(), 0);
    }

    #[test]
    fn test_keyboard_nav_tab_index() {
        let mut kn = KeyboardNavigation::new();
        kn.set_tab_index(-1);
        assert_eq!(kn.tab_index, -1);
        kn.set_tab_index(5);
        assert_eq!(kn.tab_index, 5);
    }

    #[test]
    fn test_keyboard_nav_skip_links() {
        let mut kn = KeyboardNavigation::new();
        kn.add_skip_link("main-content".to_string());
        kn.add_skip_link("footer".to_string());
        assert_eq!(kn.skip_link_count(), 2);
    }

    #[test]
    fn test_keyboard_nav_trap_focus() {
        let mut kn = KeyboardNavigation::new();
        assert!(!kn.is_focus_trapped());
        kn.set_trap_focus(true);
        assert!(kn.is_focus_trapped());
        kn.set_trap_focus(false);
        assert!(!kn.is_focus_trapped());
    }

    #[test]
    fn test_announcement_validate_ok() {
        let a = Announcement::polite("hello");
        assert!(a.validate().is_ok());
    }

    #[test]
    fn test_announcement_validate_empty() {
        let a = Announcement::polite("");
        assert_eq!(a.validate(), Err(AccessibilityError::InvalidAnnouncement));
    }

    #[test]
    fn test_verbosity_display() {
        assert_eq!(format!("{}", Verbosity::Low), "Low");
        assert_eq!(format!("{}", Verbosity::Medium), "Medium");
        assert_eq!(format!("{}", Verbosity::High), "High");
    }

    #[test]
    fn test_config_display() {
        let cfg = AccessibilityConfig::default();
        let s = format!("{cfg}");
        assert!(s.contains("enabled=true"));
        assert!(s.contains("verbosity=Medium"));
        assert!(s.contains("reduce_motion=false"));
    }

    #[test]
    fn test_config_with_verbosity() {
        let cfg = AccessibilityConfig::default().with_verbosity(Verbosity::High);
        assert_eq!(cfg.verbosity, Verbosity::High);
    }

    #[test]
    fn test_assertive_and_polite_filter() {
        let mut svc = AccessibilityService::new();
        svc.announce_status("info");
        svc.announce_alert("danger");
        svc.announce_status("more info");
        assert_eq!(svc.assertive_announcements().len(), 1);
        assert_eq!(svc.polite_announcements().len(), 2);
    }

    #[test]
    fn test_take_by_priority() {
        let mut svc = AccessibilityService::new();
        svc.announce_status("a");
        svc.announce_alert("b");
        svc.announce_status("c");
        let polites = svc.take_by_priority(AnnouncementPriority::Polite);
        assert_eq!(polites.len(), 2);
        assert_eq!(svc.announcement_count(), 1);
    }

    #[test]
    fn test_focus_history() {
        let mut fh = FocusHistory::new(3);
        assert!(fh.is_empty());
        fh.record("btn1");
        fh.record("btn2");
        assert_eq!(fh.current(), Some("btn2"));
        assert_eq!(fh.previous(), Some("btn1"));
        assert_eq!(fh.len(), 2);
        fh.record("btn3");
        fh.record("btn4"); // evicts "btn1"
        assert_eq!(fh.len(), 3);
        fh.clear();
        assert!(fh.is_empty());
    }

    #[test]
    fn test_landmark_registry() {
        let mut reg = LandmarkRegistry::new();
        assert!(reg.is_empty());
        reg.register("Main Content", AriaRole::Status);
        reg.register("Sidebar", AriaRole::List);
        reg.register("Footer", AriaRole::Status);
        assert_eq!(reg.len(), 3);
        assert_eq!(reg.find_by_role(AriaRole::Status).len(), 2);
        assert!(reg.unregister("Sidebar"));
        assert!(!reg.unregister("Nonexistent"));
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn test_accessibility_node_depth() {
        let mut root = AccessibilityNode::new("root", AriaRole::Tree);
        let mut child = AccessibilityNode::new("child", AriaRole::TreeItem);
        child.add_child(AccessibilityNode::new("grandchild", AriaRole::TreeItem));
        root.add_child(child);
        root.add_child(AccessibilityNode::new("leaf", AriaRole::TreeItem));
        assert_eq!(root.depth(), 3);
        assert_eq!(root.node_count(), 4);
    }

    #[test]
    fn test_accessibility_node_leaf() {
        let leaf = AccessibilityNode::new("leaf", AriaRole::Button);
        assert_eq!(leaf.depth(), 1);
        assert_eq!(leaf.node_count(), 1);
    }

    // ---- AnnouncementQueue tests ----

    #[test]
    fn test_announcement_queue_fifo_order() {
        let mut q = AnnouncementQueue::new(10);
        q.enqueue_polite("first");
        q.enqueue_polite("second");
        q.enqueue_assertive("third");
        assert_eq!(q.len(), 3);
        assert_eq!(q.peek().unwrap().message, "first");
        assert_eq!(q.dequeue().unwrap().message, "first");
        assert_eq!(q.dequeue().unwrap().message, "second");
        assert_eq!(q.dequeue().unwrap().message, "third");
        assert!(q.is_empty());
    }

    #[test]
    fn test_announcement_queue_overflow() {
        let mut q = AnnouncementQueue::new(2);
        q.enqueue_polite("a");
        q.enqueue_polite("b");
        assert!(q.is_full());
        q.enqueue_polite("c"); // drops "a"
        assert_eq!(q.len(), 2);
        assert_eq!(q.dequeue().unwrap().message, "b");
        assert_eq!(q.dequeue().unwrap().message, "c");
    }

    #[test]
    fn test_announcement_queue_drain() {
        let mut q = AnnouncementQueue::new(5);
        q.enqueue_polite("x");
        q.enqueue_assertive("y");
        let all = q.drain_all();
        assert_eq!(all.len(), 2);
        assert!(q.is_empty());
    }

    #[test]
    fn test_announcement_queue_capacity() {
        let q = AnnouncementQueue::new(0); // clamped to 1
        assert_eq!(q.capacity(), 1);
    }

    // ---- announce_change tests ----

    #[test]
    fn test_announce_change_polite() {
        let ann = announce_change(AriaRole::TreeItem, "src/main.rs", "expanded");
        assert_eq!(ann.priority, AnnouncementPriority::Polite);
        assert!(ann.message.contains("treeitem"));
        assert!(ann.message.contains("src/main.rs"));
        assert!(ann.message.contains("expanded"));
    }

    #[test]
    fn test_announce_change_assertive() {
        let ann = announce_change_assertive(AriaRole::Alert, "Error", "file not found");
        assert_eq!(ann.priority, AnnouncementPriority::Assertive);
        assert!(ann.message.contains("alert"));
    }

    // ---- AriaRole helpers ----

    #[test]
    fn test_aria_role_all_variants() {
        let all = AriaRole::all();
        assert_eq!(all.len(), 15);
        assert!(all.contains(&AriaRole::Button));
        assert!(all.contains(&AriaRole::Alert));
    }

    #[test]
    fn test_aria_role_interactive() {
        assert!(AriaRole::Button.is_interactive());
        assert!(AriaRole::Checkbox.is_interactive());
        assert!(AriaRole::TextBox.is_interactive());
        assert!(!AriaRole::Dialog.is_interactive());
        assert!(!AriaRole::List.is_interactive());
        assert!(!AriaRole::Status.is_interactive());
    }

    #[test]
    fn test_aria_role_container() {
        assert!(AriaRole::Dialog.is_container());
        assert!(AriaRole::Tree.is_container());
        assert!(AriaRole::List.is_container());
        assert!(!AriaRole::Button.is_container());
        assert!(!AriaRole::TextBox.is_container());
    }

    #[test]
    fn test_aria_role_from_str() {
        assert_eq!(AriaRole::from_str("button"), Some(AriaRole::Button));
        assert_eq!(AriaRole::from_str("dialog"), Some(AriaRole::Dialog));
        assert_eq!(AriaRole::from_str("unknown"), None);
    }

    // ---- AccessibilityNode tree helpers ----

    #[test]
    fn test_node_leaves() {
        let mut root = AccessibilityNode::new("root", AriaRole::Tree);
        let mut branch = AccessibilityNode::new("branch", AriaRole::TreeItem);
        branch.add_child(AccessibilityNode::new("leaf1", AriaRole::TreeItem));
        branch.add_child(AccessibilityNode::new("leaf2", AriaRole::TreeItem));
        root.add_child(branch);
        root.add_child(AccessibilityNode::new("leaf3", AriaRole::TreeItem));
        let leaves = root.leaves();
        assert_eq!(leaves.len(), 3);
        assert_eq!(leaves[0].label, "leaf1");
        assert_eq!(leaves[2].label, "leaf3");
    }

    #[test]
    fn test_node_find_by_role() {
        let mut root = AccessibilityNode::new("root", AriaRole::Tree);
        let mut child = AccessibilityNode::new("child", AriaRole::List);
        child.add_child(AccessibilityNode::new("btn", AriaRole::Button));
        root.add_child(child);
        assert_eq!(root.find_by_role(AriaRole::Button).unwrap().label, "btn");
        assert!(root.find_by_role(AriaRole::Alert).is_none());
    }

    #[test]
    fn test_node_all_labels() {
        let mut root = AccessibilityNode::new("root", AriaRole::Tree);
        root.add_child(AccessibilityNode::new("a", AriaRole::TreeItem));
        root.add_child(AccessibilityNode::new("b", AriaRole::TreeItem));
        let labels = root.all_labels();
        assert_eq!(labels, vec!["root", "a", "b"]);
    }

    // ---- FocusTracker wrap-around ----

    #[test]
    fn test_focus_tracker_wrap_around_next() {
        let mut ft = FocusTracker::new();
        ft.push("a".to_string());
        ft.push("b".to_string());
        ft.push("c".to_string());
        assert_eq!(ft.focus_next_wrap(), Some("b"));
        assert_eq!(ft.focus_next_wrap(), Some("c"));
        assert_eq!(ft.focus_next_wrap(), Some("a")); // wraps
        assert_eq!(ft.focus_next_wrap(), Some("b"));
    }

    #[test]
    fn test_focus_tracker_wrap_around_prev() {
        let mut ft = FocusTracker::new();
        ft.push("a".to_string());
        ft.push("b".to_string());
        ft.push("c".to_string());
        assert_eq!(ft.focus_previous_wrap(), Some("c")); // wraps from 0
        assert_eq!(ft.focus_previous_wrap(), Some("b"));
        assert_eq!(ft.focus_previous_wrap(), Some("a"));
        assert_eq!(ft.focus_previous_wrap(), Some("c")); // wraps again
    }

    #[test]
    fn test_focus_tracker_focus_by_id() {
        let mut ft = FocusTracker::new();
        ft.push("alpha".to_string());
        ft.push("beta".to_string());
        ft.push("gamma".to_string());
        assert!(ft.focus_by_id("beta"));
        assert_eq!(ft.current_focus(), Some("beta"));
        assert!(!ft.focus_by_id("delta"));
    }

    #[test]
    fn role_to_html_element_maps_button() {
        assert_eq!(role_to_html_element(AriaRole::Button), Some("button"));
        assert_eq!(role_to_html_element(AriaRole::List), Some("ul"));
        assert_eq!(role_to_html_element(AriaRole::Tab), None);
    }

    #[test]
    fn is_default_focusable_for_interactive_roles() {
        assert!(is_default_focusable(AriaRole::Button));
        assert!(is_default_focusable(AriaRole::TextBox));
        assert!(!is_default_focusable(AriaRole::List));
        assert!(!is_default_focusable(AriaRole::Dialog));
    }

    #[test]
    fn format_announcement_with_context_breadcrumbs() {
        let msg = format_announcement_with_context(&["Editor", "File"], "Saved");
        assert_eq!(msg, "[Editor > File] Saved");

        let msg_empty = format_announcement_with_context(&[], "Saved");
        assert_eq!(msg_empty, "Saved");
    }

    #[test]
    fn format_count_announcement_pluralization() {
        assert_eq!(format_count_announcement("error", 0), "No errors");
        assert_eq!(format_count_announcement("error", 1), "1 error");
        assert_eq!(format_count_announcement("warning", 5), "5 warnings");
    }

    #[test]
    fn contrast_ratio_black_white() {
        let ratio = contrast_ratio(0, 0, 0, 255, 255, 255);
        assert!(ratio > 20.9 && ratio < 21.1);
        assert_eq!(evaluate_contrast(ratio), WcagConformance::AAA);
    }

    #[test]
    fn contrast_ratio_same_colour_is_one() {
        let ratio = contrast_ratio(128, 128, 128, 128, 128, 128);
        assert!((ratio - 1.0).abs() < 0.001);
        assert_eq!(evaluate_contrast(ratio), WcagConformance::Fail);
    }

    #[test]
    fn accessibility_node_find_all_by_role() {
        let mut root = AccessibilityNode::new("root", AriaRole::Tree);
        root.add_child(AccessibilityNode::new("btn1", AriaRole::Button));
        let mut sub = AccessibilityNode::new("menu", AriaRole::Menu);
        sub.add_child(AccessibilityNode::new("btn2", AriaRole::Button));
        root.add_child(sub);
        let buttons = root.find_all(|n| n.role == AriaRole::Button);
        assert_eq!(buttons.len(), 2);
    }

    #[test]
    fn accessibility_node_path_to_label() {
        let mut root = AccessibilityNode::new("root", AriaRole::Tree);
        let mut child = AccessibilityNode::new("branch", AriaRole::TreeItem);
        child.add_child(AccessibilityNode::new("leaf", AriaRole::TreeItem));
        root.add_child(child);
        let path = root.path_to("leaf").unwrap();
        assert_eq!(path, vec!["root", "branch", "leaf"]);
        assert!(root.path_to("missing").is_none());
    }

    #[test]
    fn peek_announcements_does_not_drain() {
        let mut svc = AccessibilityService::new();
        svc.announce_status("hello");
        assert_eq!(svc.peek_announcements().len(), 1);
        assert_eq!(svc.peek_announcements().len(), 1);
    }

    #[test]
    fn has_assertive_pending_true() {
        let mut svc = AccessibilityService::new();
        svc.announce_alert("critical");
        assert!(svc.has_assertive_pending());
    }

    #[test]
    fn has_assertive_pending_false_when_only_polite() {
        let mut svc = AccessibilityService::new();
        svc.announce_status("info");
        assert!(!svc.has_assertive_pending());
    }

    #[test]
    fn total_announcement_chars() {
        let mut svc = AccessibilityService::new();
        svc.announce_status("abc");
        svc.announce_alert("de");
        assert_eq!(svc.total_announcement_chars(), 5);
    }

    #[test]
    fn focus_tracker_all_ids() {
        let mut ft = FocusTracker::new();
        ft.push("a".into());
        ft.push("b".into());
        assert_eq!(ft.all_ids(), vec!["a", "b"]);
    }

    #[test]
    fn focus_tracker_current_index_empty() {
        let ft = FocusTracker::new();
        assert!(ft.focused_index().is_none());
    }

    #[test]
    fn focus_tracker_current_index_with_items() {
        let mut ft = FocusTracker::new();
        ft.push("x".into());
        assert_eq!(ft.focused_index(), Some(0));
    }

    #[test]
    fn announcement_queue_remaining_capacity() {
        let mut q = AnnouncementQueue::new(3);
        q.enqueue_polite("a");
        assert_eq!(q.remaining_capacity(), 2);
    }

    #[test]
    fn announcement_queue_peek_messages() {
        let mut q = AnnouncementQueue::new(10);
        q.enqueue_polite("hello");
        q.enqueue_assertive("alert");
        let msgs = q.peek_messages();
        assert_eq!(msgs, vec!["hello", "alert"]);
    }

    #[test]
    fn node_child_count_and_is_leaf() {
        let mut node = AccessibilityNode::new("parent", AriaRole::Tree);
        assert!(node.is_leaf());
        assert_eq!(node.child_count(), 0);
        node.add_child(AccessibilityNode::new("child", AriaRole::TreeItem));
        assert!(!node.is_leaf());
        assert_eq!(node.child_count(), 1);
    }

    #[test]
    fn node_role_accessor() {
        let node = AccessibilityNode::new("btn", AriaRole::Button);
        assert_eq!(node.role(), AriaRole::Button);
    }

    #[test]
    fn format_focus_summary_empty() {
        let ft = FocusTracker::new();
        assert_eq!(format_focus_summary(&ft), "No element focused");
    }

    #[test]
    fn format_focus_summary_with_focus() {
        let mut ft = FocusTracker::new();
        ft.push("editor".into());
        ft.push("panel".into());
        let s = format_focus_summary(&ft);
        assert!(s.contains("editor"));
        assert!(s.contains("1 of 2"));
    }

    #[test]
    fn describe_tree_output() {
        let mut root = AccessibilityNode::new("root", AriaRole::Tree);
        root.add_child(AccessibilityNode::new("item1", AriaRole::TreeItem));
        root.add_child(AccessibilityNode::new("item2", AriaRole::TreeItem));
        let desc = describe_tree(&root, 0);
        assert!(desc.contains("root"));
        assert!(desc.contains("item1"));
        assert!(desc.contains("item2"));
    }

    #[test]
    fn count_interactive_nodes_mixed() {
        let mut root = AccessibilityNode::new("list", AriaRole::List);
        root.add_child(AccessibilityNode::new("btn", AriaRole::Button));
        root.add_child(AccessibilityNode::new("text", AriaRole::TextBox));
        root.add_child(AccessibilityNode::new("item", AriaRole::ListItem));
        // Button and TextBox are interactive; List and ListItem are not
        assert_eq!(count_interactive_nodes(&root), 2);
    }

    #[test]
    fn config_verbose_and_minimal() {
        let cfg = AccessibilityConfig::default().with_verbosity(Verbosity::High);
        assert!(cfg.is_verbose());
        assert!(!cfg.is_minimal());
        let cfg2 = AccessibilityConfig::default().with_verbosity(Verbosity::Low);
        assert!(cfg2.is_minimal());
        assert!(!cfg2.is_verbose());
    }

    #[test]
    fn keyboard_nav_skip_links() {
        let mut nav = KeyboardNavigation::new();
        assert!(!nav.has_skip_links());
        nav.add_skip_link("main-content".into());
        nav.add_skip_link("sidebar".into());
        assert!(nav.has_skip_links());
        assert_eq!(nav.skip_links_display(), "main-content, sidebar");
        nav.clear_skip_links();
        assert!(!nav.has_skip_links());
    }

    #[test]
    fn landmark_registry_all_labels() {
        let mut reg = LandmarkRegistry::new();
        reg.register("main", AriaRole::Menu);
        reg.register("sidebar", AriaRole::List);
        let labels = reg.all_labels();
        assert_eq!(labels, vec!["main", "sidebar"]);
    }

    #[test]
    fn landmark_registry_unique_roles() {
        let mut reg = LandmarkRegistry::new();
        reg.register("a", AriaRole::Menu);
        reg.register("b", AriaRole::Menu);
        reg.register("c", AriaRole::List);
        let roles = reg.unique_roles();
        assert_eq!(roles.len(), 2);
    }

    #[test]
    fn focus_history_entries_and_unique() {
        let mut fh = FocusHistory::new(10);
        fh.record("editor");
        fh.record("panel");
        fh.record("editor");
        assert_eq!(fh.entries().len(), 3);
        assert_eq!(fh.unique_count(), 2);
    }

    // -- A11yAnnouncer -----------------------------------------------------

    #[test]
    fn announcer_enqueue_dequeue() {
        let mut ann = A11yAnnouncer::new(10);
        ann.enqueue(Announcement::polite("hello"));
        ann.enqueue(Announcement::assertive("urgent"));
        assert_eq!(ann.len(), 2);
        // Assertive should come first
        let first = ann.dequeue().unwrap();
        assert_eq!(first.priority, AnnouncementPriority::Assertive);
    }

    #[test]
    fn announcer_max_queue() {
        let mut ann = A11yAnnouncer::new(2);
        ann.enqueue(Announcement::polite("a"));
        ann.enqueue(Announcement::polite("b"));
        ann.enqueue(Announcement::polite("c"));
        assert_eq!(ann.len(), 2);
    }

    #[test]
    fn announcer_assertive_count() {
        let mut ann = A11yAnnouncer::new(10);
        ann.enqueue(Announcement::polite("x"));
        ann.enqueue(Announcement::assertive("y"));
        assert_eq!(ann.assertive_count(), 1);
    }

    #[test]
    fn announcer_display() {
        let ann = A11yAnnouncer::default();
        let s = format!("{ann}");
        assert!(s.contains("0 queued"));
    }

    // -- A11yNavigator -----------------------------------------------------

    #[test]
    fn navigator_next_previous() {
        let mut nav = A11yNavigator::new();
        nav.add_landmark(A11yLandmark { role: AriaRole::Grid, label: "Main".into(), id: "main".into() });
        nav.add_landmark(A11yLandmark { role: AriaRole::Menu, label: "Nav".into(), id: "nav".into() });

        let first = nav.next().unwrap();
        assert_eq!(first.label, "Main");
        let second = nav.next().unwrap();
        assert_eq!(second.label, "Nav");
        let wrap = nav.next().unwrap();
        assert_eq!(wrap.label, "Main");
    }

    #[test]
    fn navigator_previous() {
        let mut nav = A11yNavigator::new();
        nav.add_landmark(A11yLandmark { role: AriaRole::Grid, label: "A".into(), id: "a".into() });
        nav.add_landmark(A11yLandmark { role: AriaRole::Status, label: "B".into(), id: "b".into() });
        let last = nav.previous().unwrap();
        assert_eq!(last.label, "B");
    }

    #[test]
    fn navigator_find_by_role() {
        let mut nav = A11yNavigator::new();
        nav.add_landmark(A11yLandmark { role: AriaRole::Grid, label: "M".into(), id: "m".into() });
        nav.add_landmark(A11yLandmark { role: AriaRole::Menu, label: "N".into(), id: "n".into() });
        let grids = nav.find_by_role(&AriaRole::Grid);
        assert_eq!(grids.len(), 1);
    }

    #[test]
    fn navigator_display() {
        let nav = A11yNavigator::default();
        assert!(format!("{nav}").contains("0 landmarks"));
    }

    // -- A11yContrastChecker -----------------------------------------------

    #[test]
    fn contrast_black_white() {
        let ratio = A11yContrastChecker::contrast_ratio(0, 0, 0, 255, 255, 255);
        assert!(ratio >= 21.0);
        assert!(A11yContrastChecker::meets_aa(0, 0, 0, 255, 255, 255));
        assert!(A11yContrastChecker::meets_aaa(0, 0, 0, 255, 255, 255));
    }

    #[test]
    fn contrast_same_color_is_one() {
        let ratio = A11yContrastChecker::contrast_ratio(128, 128, 128, 128, 128, 128);
        assert!((ratio - 1.0).abs() < 0.01);
    }

    #[test]
    fn contrast_aa_large() {
        assert!(A11yContrastChecker::meets_aa_large(0, 0, 0, 255, 255, 255));
    }

    // -- ScreenReaderTextBuilder -------------------------------------------

    #[test]
    fn sr_text_builder_basic() {
        let mut b = ScreenReaderTextBuilder::new();
        b.add("Line 5");
        b.add_labeled("Column", "10");
        assert_eq!(b.build(), "Line 5, Column: 10");
    }

    #[test]
    fn sr_text_builder_empty_skipped() {
        let mut b = ScreenReaderTextBuilder::new();
        b.add("");
        b.add("hello");
        assert_eq!(b.part_count(), 1);
    }

    #[test]
    fn sr_text_builder_custom_separator() {
        let mut b = ScreenReaderTextBuilder::new().with_separator(" | ");
        b.add("a");
        b.add("b");
        assert_eq!(b.build(), "a | b");
    }

    #[test]
    fn sr_text_builder_display() {
        let mut b = ScreenReaderTextBuilder::new();
        b.add("test");
        let s = format!("{b}");
        assert_eq!(s, "test");
    }

#[test]
    fn audit_severity_ordering() {
        assert!(AuditSeverity::Critical > AuditSeverity::Error);
        assert!(AuditSeverity::Error > AuditSeverity::Warning);
        assert!(AuditSeverity::Warning > AuditSeverity::Info);
    }

    #[test]
    fn audit_severity_display() {
        assert_eq!(AuditSeverity::Info.to_string(), "info");
        assert_eq!(AuditSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn audit_finding_creation() {
        let f = AuditFinding::new(AuditSeverity::Error, "ARIA-01", "Missing label", "/div/input");
        assert_eq!(f.severity, AuditSeverity::Error);
        assert_eq!(f.rule_id, "ARIA-01");
        assert!(f.suggestion.is_none());
    }

    #[test]
    fn audit_finding_with_suggestion() {
        let f = AuditFinding::new(AuditSeverity::Warning, "ARIA-02", "Low contrast", "/p")
            .with_suggestion("Increase contrast ratio");
        assert_eq!(f.suggestion.as_deref(), Some("Increase contrast ratio"));
    }

    #[test]
    fn audit_finding_is_critical() {
        let crit = AuditFinding::new(AuditSeverity::Critical, "C1", "msg", "/");
        let warn = AuditFinding::new(AuditSeverity::Warning, "W1", "msg", "/");
        assert!(crit.is_critical());
        assert!(!warn.is_critical());
    }

    #[test]
    fn audit_reporter_summary() {
        let mut r = AuditReporter::new("editor");
        r.add_finding(AuditFinding::new(AuditSeverity::Error, "E1", "msg", "/"));
        r.add_finding(AuditFinding::new(AuditSeverity::Warning, "W1", "msg", "/"));
        assert_eq!(r.finding_count(), 2);
        assert!(r.has_errors());
        assert!(!r.has_critical());
    }

    #[test]
    fn audit_reporter_clear() {
        let mut r = AuditReporter::new("test");
        r.add_finding(AuditFinding::new(AuditSeverity::Info, "I1", "msg", "/"));
        r.clear();
        assert_eq!(r.finding_count(), 0);
    }

    #[test]
    fn audit_reporter_sorted() {
        let mut r = AuditReporter::new("scope");
        r.add_finding(AuditFinding::new(AuditSeverity::Info, "I1", "info", "/"));
        r.add_finding(AuditFinding::new(AuditSeverity::Critical, "C1", "crit", "/"));
        let sorted = r.sorted_findings();
        assert_eq!(sorted[0].severity, AuditSeverity::Critical);
    }

    #[test]
    fn keyboard_shortcut_format() {
        let s = KeyboardShortcut::new("S").with_ctrl().with_shift();
        assert_eq!(s.format_announcement(), "Ctrl+Shift+S");
        assert_eq!(s.modifier_count(), 2);
    }

    #[test]
    fn keyboard_shortcut_simple() {
        let s = KeyboardShortcut::new("F1");
        assert!(s.is_simple());
        assert_eq!(s.format_announcement(), "F1");
    }

    #[test]
    fn shortcut_announcer_flow() {
        let mut a = ShortcutAnnouncer::new();
        let s = KeyboardShortcut::new("S").with_ctrl();
        a.announce(&s, "Save file");
        assert_eq!(a.pending_count(), 1);
        let drained = a.drain_announcements();
        assert_eq!(drained.len(), 1);
        assert_eq!(a.pending_count(), 0);
    }

    #[test]
    fn shortcut_announcer_max_queue() {
        let mut a = ShortcutAnnouncer::new().with_max_queue(2);
        let s = KeyboardShortcut::new("A");
        a.announce(&s, "a1");
        a.announce(&s, "a2");
        a.announce(&s, "a3");
        assert_eq!(a.pending_count(), 2);
    }

    #[test]
    fn audit_finding_display() {
        let f = AuditFinding::new(AuditSeverity::Error, "ARIA-01", "Missing label", "/input");
        let s = format!("{f}");
        assert!(s.contains("ARIA-01"));
    }

    // -- AriaRole helpers ---------------------------------------------------

    #[test]
    fn aria_role_button_is_interactive() {
        assert!(AriaRole::Button.is_interactive_v2());
        assert!(AriaRole::Checkbox.is_interactive_v2());
        assert!(AriaRole::TextBox.is_interactive_v2());
    }

    #[test]
    fn aria_role_list_is_not_interactive() {
        assert!(!AriaRole::List.is_interactive_v2());
        assert!(!AriaRole::Grid.is_interactive_v2());
    }

    #[test]
    fn aria_role_container_check() {
        assert!(AriaRole::List.is_container_v2());
        assert!(AriaRole::Tree.is_container_v2());
        assert!(!AriaRole::Button.is_container_v2());
    }

    #[test]
    fn aria_role_screen_reader_labels() {
        assert_eq!(AriaRole::Button.screen_reader_label(), "button");
        assert_eq!(AriaRole::Status.screen_reader_label(), "status");
        assert_eq!(AriaRole::Alert.screen_reader_label(), "alert");
    }

    // -- AccessibleDescriptionBuilder ----------------------------------------

    #[test]
    fn description_builder_empty() {
        let desc = AccessibleDescriptionBuilder::new().build();
        assert!(desc.is_empty());
    }

    #[test]
    fn description_builder_full() {
        let desc = AccessibleDescriptionBuilder::new()
            .add_role(AriaRole::Button)
            .add_label("Save")
            .add_state("pressed")
            .add_value("on")
            .build();
        assert!(desc.contains("Save"));
        assert!(desc.contains("button"));
        assert!(desc.contains("pressed"));
        assert!(desc.contains("value: on"));
    }

    #[test]
    fn description_builder_label_only() {
        let desc = AccessibleDescriptionBuilder::new()
            .add_label("Close")
            .build();
        assert_eq!(desc, "Close");
    }

    // -- FocusOrderTracker ---------------------------------------------------

    #[test]
    fn focus_order_empty() {
        let mut t = FocusOrderTracker::new();
        assert!(t.is_empty());
        assert_eq!(t.focus_next(), None);
        assert_eq!(t.focus_prev(), None);
    }

    #[test]
    fn focus_order_next_wraps() {
        let mut t = FocusOrderTracker::new();
        t.register("a", true);
        t.register("b", false);
        assert_eq!(t.focus_next(), Some("a"));
        assert_eq!(t.focus_next(), Some("b"));
        assert_eq!(t.focus_next(), Some("a")); // wraps
    }

    #[test]
    fn focus_order_prev_wraps() {
        let mut t = FocusOrderTracker::new();
        t.register("x", true);
        t.register("y", true);
        assert_eq!(t.focus_prev(), Some("y")); // starts at end
        assert_eq!(t.focus_prev(), Some("x"));
        assert_eq!(t.focus_prev(), Some("y"));
    }

    #[test]
    fn focus_order_unregister_adjusts() {
        let mut t = FocusOrderTracker::new();
        t.register("a", true);
        t.register("b", true);
        t.register("c", true);
        t.focus_next(); // a
        t.focus_next(); // b
        t.unregister("a"); // now ["b","c"], current was 1->0
        assert_eq!(t.current(), Some("b"));
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn focus_order_interactive_elements() {
        let mut t = FocusOrderTracker::new();
        t.register("btn", true);
        t.register("label", false);
        t.register("input", true);
        let ie = t.interactive_elements();
        assert_eq!(ie, vec!["btn", "input"]);
    }

    #[test]
    fn a11y_config_new() {
        let cfg = A11yConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn a11y_config_set_get() {
        let mut cfg = A11yConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn a11y_config_remove() {
        let mut cfg = A11yConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn a11y_config_keys_sorted() {
        let mut cfg = A11yConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn a11y_config_bump_version() {
        let mut cfg = A11yConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn a11y_config_clear() {
        let mut cfg = A11yConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn a11y_config_merge() {
        let mut cfg1 = A11yConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = A11yConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn a11y_config_disable() {
        let mut cfg = A11yConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn a11y_rate_tracker_empty() {
        let rt = A11yRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn a11y_rate_tracker_record() {
        let mut rt = A11yRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn a11y_rate_tracker_prune() {
        let mut rt = A11yRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn a11y_validator_valid() {
        let v = A11yValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn a11y_validator_errors() {
        let mut v = A11yValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn a11y_validator_clear() {
        let mut v = A11yValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn a11y_validator_merge() {
        let mut v1 = A11yValidator::new();
        v1.add_error("e1");
        let mut v2 = A11yValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn a11y_rate_tracker_clear() {
        let mut rt = A11yRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
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


    // xa_ extended tests for a11y
    #[test]
    fn xa_a11y_ring_new() {
        let rb = super::XaA11yRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_a11y_ring_push_len() {
        let mut rb = super::XaA11yRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_a11y_ring_wrap() {
        let mut rb = super::XaA11yRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_a11y_ring_mean_empty() {
        let rb = super::XaA11yRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_a11y_ring_mean_values() {
        let mut rb = super::XaA11yRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_a11y_ring_min_max() {
        let mut rb = super::XaA11yRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_a11y_ring_iter() {
        let mut rb = super::XaA11yRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_a11y_counter_new() {
        let c = super::XaA11yCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_a11y_counter_inc() {
        let mut c = super::XaA11yCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_a11y_counter_inc_by() {
        let mut c = super::XaA11yCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_a11y_counter_reset() {
        let mut c = super::XaA11yCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_a11y_counter_clear() {
        let mut c = super::XaA11yCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_a11y_counter_default() {
        let c = super::XaA11yCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 2 ----

    #[test]
    fn xc_2_pool_new_empty() {
        let pool: super::Xc2Pool<i32> = super::Xc2Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_2_pool_release_acquire() {
        let mut pool = super::Xc2Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_2_pool_acquire_empty() {
        let mut pool: super::Xc2Pool<i32> = super::Xc2Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_2_pool_full() {
        let mut pool = super::Xc2Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_2_pool_drain() {
        let mut pool = super::Xc2Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_2_pool_stats() {
        let mut pool = super::Xc2Pool::new(8);
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
    fn xc_2_pool_clear() {
        let mut pool = super::Xc2Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_2_pool_shrink() {
        let mut pool = super::Xc2Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_2_pool_default() {
        let pool: super::Xc2Pool<String> = super::Xc2Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_2_pool_extend() {
        let mut pool = super::Xc2Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_2_pool_retain() {
        let mut pool = super::Xc2Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_2_scheduler_round_robin() {
        let mut sched = super::Xc2Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_2_scheduler_empty() {
        let mut sched = super::Xc2Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_2_scheduler_reset() {
        let mut sched = super::Xc2Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_2_scheduler_add_remove() {
        let mut sched = super::Xc2Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_2_scheduler_targets() {
        let sched = super::Xc2Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_2_hash_empty() {
        assert_eq!(super::xc_2_hash(b""), 5381);
    }

    #[test]
    fn xc_2_hash_data() {
        let h = super::xc_2_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_2_hash(b"hello"), h);
    }

    #[test]
    fn xc_2_reverse_str() {
        assert_eq!(super::xc_2_reverse("abc"), "cba");
        assert_eq!(super::xc_2_reverse(""), "");
    }


    #[test]
    fn xe_6_pipeline_empty() {
        let p = super::Xe6Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_6_pipeline_parse_stage() {
        let p = super::Xe6Pipeline::new()
            .add_parse(super::xe_6_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_6_pipeline_transform_double() {
        let p = super::Xe6Pipeline::new()
            .add_transform(super::xe_6_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_6_pipeline_validate_reverse() {
        let p = super::Xe6Pipeline::new()
            .add_validate(super::xe_6_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_6_pipeline_emit_filter() {
        let p = super::Xe6Pipeline::new()
            .add_emit(super::xe_6_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_6_pipeline_multi_stage() {
        let p = super::Xe6Pipeline::new()
            .add_parse(super::xe_6_pipeline_identity)
            .add_transform(super::xe_6_pipeline_double)
            .add_validate(super::xe_6_pipeline_reverse)
            .add_emit(super::xe_6_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_6_pipeline_error_propagation() {
        let p = super::Xe6Pipeline::new()
            .add_parse(super::xe_6_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe6Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_6_pipeline_compose() {
        let p1 = super::Xe6Pipeline::new()
            .add_parse(super::xe_6_pipeline_identity);
        let p2 = super::Xe6Pipeline::new()
            .add_transform(super::xe_6_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_6_pipeline_error_display() {
        let e = super::Xe6PipelineError {
            stage: super::Xe6Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_6_cache_put_get() {
        let mut c = super::Xe6Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_6_cache_miss() {
        let mut c: super::Xe6Cache<&str, i32> = super::Xe6Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_6_cache_ttl_expiry() {
        let mut c = super::Xe6Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_6_cache_evict() {
        let mut c = super::Xe6Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_6_cache_capacity() {
        let mut c = super::Xe6Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_6_cache_stats() {
        let mut c = super::Xe6Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_6_cache_clear() {
        let mut c = super::Xe6Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #69 --

    #[test]
    fn xf69_trie_insert_search() {
        let mut t = Xf69Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf69_trie_starts_with() {
        let mut t = Xf69Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf69_trie_remove() {
        let mut t = Xf69Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf69_trie_word_count() {
        let mut t = Xf69Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf69_trie_longest_prefix() {
        let mut t = Xf69Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf69_trie_all_words() {
        let mut t = Xf69Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf69_trie_autocomplete() {
        let mut t = Xf69Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf69_trie_empty_search() {
        let t = Xf69Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf69_bloom_add_contains() {
        let mut bf = Xf69BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf69_bloom_probably_absent() {
        let bf = Xf69BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf69_bloom_false_positive_rate() {
        let mut bf = Xf69BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf69_bloom_clear() {
        let mut bf = Xf69BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf69_bloom_union() {
        let mut a = Xf69BloomFilter::xf_new(512, 2);
        let mut b = Xf69BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf69_bloom_intersection_estimate() {
        let mut a = Xf69BloomFilter::xf_new(512, 2);
        let mut b = Xf69BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf69_bloom_union_size_mismatch() {
        let a = Xf69BloomFilter::xf_new(256, 2);
        let b = Xf69BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh1_skip_insert_contains() {
        let mut sl = super::Xh1SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh1_skip_remove() {
        let mut sl = super::Xh1SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh1_skip_len() {
        let mut sl = super::Xh1SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh1_skip_range_query() {
        let mut sl = super::Xh1SkipList::xh_new(4);
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
    fn xh1_skip_floor_ceiling() {
        let mut sl = super::Xh1SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh1_skip_rank() {
        let mut sl = super::Xh1SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh1_skip_empty() {
        let sl = super::Xh1SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh1_skip_duplicates() {
        let mut sl = super::Xh1SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh1_bitset_set_test() {
        let mut bs = super::Xh1BitSet::xh_new(256);
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
    fn xh1_bitset_clear_count() {
        let mut bs = super::Xh1BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh1_bitset_and_or_xor() {
        let mut a = super::Xh1BitSet::xh_new(128);
        let mut b = super::Xh1BitSet::xh_new(128);
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
    fn xh1_bitset_iter_ones() {
        let mut bs = super::Xh1BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh1_bitset_first_last() {
        let mut bs = super::Xh1BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh1_bitset_empty() {
        let bs = super::Xh1BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi1_deque_push_pop_back() {
        let mut dq = super::Xi1Deque::xi_new(4);
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
    fn xi1_deque_push_pop_front() {
        let mut dq = super::Xi1Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi1_deque_mixed_ops() {
        let mut dq = super::Xi1Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi1_deque_get_and_split() {
        let mut dq = super::Xi1Deque::xi_new(8);
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
    fn xi1_deque_rotate_left() {
        let mut dq = super::Xi1Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi1_deque_rotate_right() {
        let mut dq = super::Xi1Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi1_deque_grow() {
        let mut dq = super::Xi1Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi1_deque_empty() {
        let dq = super::Xi1Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi1_interval_tree_insert_query() {
        let mut tree = super::Xi1IntervalTree::xi_new();
        tree.xi_insert(super::Xi1Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi1Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi1Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi1_interval_tree_overlap() {
        let mut tree = super::Xi1IntervalTree::xi_new();
        tree.xi_insert(super::Xi1Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi1Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi1Interval::xi_new(12, 20));
        let q = super::Xi1Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi1_interval_tree_remove() {
        let mut tree = super::Xi1IntervalTree::xi_new();
        tree.xi_insert(super::Xi1Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi1Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi1_interval_tree_gaps() {
        let mut tree = super::Xi1IntervalTree::xi_new();
        tree.xi_insert(super::Xi1Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi1Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi1Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi1Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi1Interval::xi_new(8, 10));
    }

    #[test]
    fn xi1_interval_tree_merge() {
        let mut tree = super::Xi1IntervalTree::xi_new();
        tree.xi_insert(super::Xi1Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi1Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi1Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi1Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi1Interval::xi_new(10, 15));
    }

    #[test]
    fn xi1_interval_tree_all() {
        let mut tree = super::Xi1IntervalTree::xi_new();
        tree.xi_insert(super::Xi1Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi1Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi1_interval_tree_empty() {
        let tree = super::Xi1IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi1_interval_tree_contains_point() {
        let iv = super::Xi1Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 1) ---

    #[test]
    fn xj_1_uf_make_and_find() {
        let mut uf = super::Xj1UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_1_uf_union_connected() {
        let mut uf = super::Xj1UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_1_uf_component_count() {
        let mut uf = super::Xj1UnionFind::xj_new();
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
    fn xj_1_uf_component_size() {
        let mut uf = super::Xj1UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_1_uf_largest_component() {
        let mut uf = super::Xj1UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_1_uf_many_elements() {
        let mut uf = super::Xj1UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_1_uf_separate_components() {
        let mut uf = super::Xj1UnionFind::xj_new();
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
    fn xj_1_uf_path_compression() {
        let mut uf = super::Xj1UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_1_bt_insert_get() {
        let mut bt = super::Xj1BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_1_bt_contains_len() {
        let mut bt = super::Xj1BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_1_bt_replace() {
        let mut bt = super::Xj1BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
        assert!(bt.xj_contains_key(&1));
        assert!(bt.xj_contains_key(&2));
    }

    #[test]
    fn xj_1_bt_remove() {
        let mut bt = super::Xj1BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_1_bt_keys_values() {
        let mut bt = super::Xj1BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_1_bt_range() {
        let mut bt = super::Xj1BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_1_bt_min_max() {
        let mut bt = super::Xj1BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_1_bt_many_inserts() {
        let mut bt = super::Xj1BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_0 segment tree tests ---

    #[test]
    fn xk_0_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk0SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_0_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk0SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_0_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk0SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_0_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk0SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_0_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk0SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_0_st_single_element() {
        let data = vec![42];
        let st = super::Xk0SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_0_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk0SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_0_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk0SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_0 disjoint intervals tests ---

    #[test]
    fn xk_0_di_add_and_count() {
        let mut di = super::Xk0DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_0_di_merge_overlap() {
        let mut di = super::Xk0DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_0_di_contains() {
        let mut di = super::Xk0DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_0_di_remove() {
        let mut di = super::Xk0DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_0_di_covered_length() {
        let mut di = super::Xk0DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_0_di_gaps() {
        let mut di = super::Xk0DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_0_di_merge_adjacent() {
        let mut di = super::Xk0DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_0_di_empty() {
        let di = super::Xk0DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
