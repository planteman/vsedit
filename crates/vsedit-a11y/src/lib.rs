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

}
