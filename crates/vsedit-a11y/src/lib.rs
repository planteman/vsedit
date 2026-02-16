//! Accessibility service.

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
}
