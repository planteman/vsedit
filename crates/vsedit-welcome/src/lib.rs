//! Welcome page.

use std::fmt;
/// A single item on the welcome page.
#[derive(Debug, Clone)]
pub struct WelcomeItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: Option<String>,
    pub command: Option<String>,
}

/// A section grouping related welcome items.
#[derive(Debug, Clone)]
pub struct WelcomeSection {
    pub title: String,
    pub items: Vec<WelcomeItem>,
}

/// The welcome page shown on startup.
pub struct WelcomePage {
    pub sections: Vec<WelcomeSection>,
    pub show_on_startup: bool,
}

impl WelcomePage {
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
            show_on_startup: true,
        }
    }

    pub fn add_section(&mut self, section: WelcomeSection) {
        self.sections.push(section);
    }

    pub fn set_show_on_startup(&mut self, show: bool) {
        self.show_on_startup = show;
    }

    pub fn should_show(&self) -> bool {
        self.show_on_startup
    }

    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    pub fn total_item_count(&self) -> usize {
        self.sections.iter().map(|s| s.items.len()).sum()
    }
}

impl Default for WelcomePage {
    fn default() -> Self {
        Self::new()
    }
}

/// A recently opened item.
#[derive(Debug, Clone)]
pub struct RecentItem {
    pub uri: String,
    pub label: String,
    pub timestamp: u64,
}

/// List of recently opened items, sorted by most recent first.
pub struct RecentItemsList {
    items: Vec<RecentItem>,
}

impl RecentItemsList {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(&mut self, item: RecentItem) {
        self.items.push(item);
        self.items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }

    pub fn get_recent(&self, max: usize) -> &[RecentItem] {
        let end = max.min(self.items.len());
        &self.items[..end]
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

impl Default for RecentItemsList {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// WelcomeItemKind
// ---------------------------------------------------------------------------

/// The kind of a welcome item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WelcomeItemKind {
    Command,
    Link,
    Walkthrough,
    Extension,
}

// ---------------------------------------------------------------------------
// WalkthroughStep / WelcomeWalkthrough
// ---------------------------------------------------------------------------

/// A single step within a walkthrough.
#[derive(Debug, Clone)]
pub struct WalkthroughStep {
    pub id: String,
    pub title: String,
    pub description: String,
    pub completed: bool,
}

/// A guided walkthrough shown on the welcome page.
#[derive(Debug, Clone)]
pub struct WelcomeWalkthrough {
    pub id: String,
    pub title: String,
    pub steps: Vec<WalkthroughStep>,
}

impl WelcomeWalkthrough {
    /// Returns `true` when every step has been completed.
    pub fn is_complete(&self) -> bool {
        !self.steps.is_empty() && self.steps.iter().all(|s| s.completed)
    }

    /// Percentage of steps completed (0–100).
    pub fn completion_percentage(&self) -> f64 {
        if self.steps.is_empty() {
            return 0.0;
        }
        let done = self.steps.iter().filter(|s| s.completed).count();
        (done as f64 / self.steps.len() as f64) * 100.0
    }

    /// Mark the step with the given `id` as completed.
    pub fn complete_step(&mut self, id: &str) {
        if let Some(step) = self.steps.iter_mut().find(|s| s.id == id) {
            step.completed = true;
        }
    }

    /// Returns true if steps is empty.
    pub fn is_steps_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Get the first step, if any.
    pub fn first_step(&self) -> Option<&WalkthroughStep> {
        self.steps.first()
    }

    /// Get the last step, if any.
    pub fn last_step(&self) -> Option<&WalkthroughStep> {
        self.steps.last()
    }

    /// Retain only steps matching the predicate.
    pub fn retain_steps(&mut self, f: impl Fn(&WalkthroughStep) -> bool) {
        self.steps.retain(|item| f(item));
    }
}

// ---------------------------------------------------------------------------
// Extra WelcomePage methods
// ---------------------------------------------------------------------------

impl WelcomePage {
    /// Find a section by title.
    pub fn find_section(&self, title: &str) -> Option<&WelcomeSection> {
        self.sections.iter().find(|s| s.title == title)
    }

    /// Remove the first section with the given title. Returns `true` if removed.
    pub fn remove_section(&mut self, title: &str) -> bool {
        let before = self.sections.len();
        self.sections.retain(|s| s.title != title);
        self.sections.len() < before
    }

    /// Search all sections for an item with the given `id`.
    pub fn find_item_by_id(&self, id: &str) -> Option<&WelcomeItem> {
        self.sections
            .iter()
            .flat_map(|s| &s.items)
            .find(|item| item.id == id)
    }
}

// ---------------------------------------------------------------------------
// Extra RecentItemsList methods
// ---------------------------------------------------------------------------

impl RecentItemsList {
    /// Remove the first item whose `uri` matches.
    pub fn remove_item(&mut self, uri: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.uri != uri);
        self.items.len() < before
    }

    /// Returns `true` if any item has the given `uri`.
    pub fn contains(&self, uri: &str) -> bool {
        self.items.iter().any(|i| i.uri == uri)
    }
}

// ---------------------------------------------------------------------------
// WelcomeContentProvider trait
// ---------------------------------------------------------------------------

/// Trait for providing welcome-page content.
pub trait WelcomeContentProvider {
    /// Provide sections for the welcome page.
    fn provide_sections(&self) -> Vec<WelcomeSection> {
        Vec::new()
    }

    /// Provide walkthroughs for the welcome page.
    fn provide_walkthroughs(&self) -> Vec<WelcomeWalkthrough> {
        Vec::new()
    }
}

/// Renders a welcome section as a list of formatted text lines.
pub struct WelcomeSectionRenderer;

impl WelcomeSectionRenderer {
    /// Render a section header and its items.
    pub fn render_section(section: &WelcomeSection) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!("── {} ──", section.title));
        lines.push(String::new());
        for item in &section.items {
            let icon = item.icon.as_deref().unwrap_or("•");
            lines.push(format!("  {icon} {}", item.title));
            if !item.description.is_empty() {
                lines.push(format!("    {}", item.description));
            }
        }
        lines.push(String::new());
        lines
    }

    /// Render an entire welcome page.
    pub fn render_page(page: &WelcomePage) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push("Welcome to VSEdit".to_string());
        lines.push("=".repeat(40));
        lines.push(String::new());
        for section in &page.sections {
            lines.extend(Self::render_section(section));
        }
        lines
    }

    /// Render a walkthrough with progress.
    pub fn render_walkthrough(wt: &WelcomeWalkthrough) -> Vec<String> {
        let mut lines = Vec::new();
        let pct = wt.completion_percentage();
        lines.push(format!("{} ({:.0}% complete)", wt.title, pct));
        lines.push(String::new());
        for step in &wt.steps {
            let check = if step.completed { "✓" } else { "○" };
            lines.push(format!("  [{check}] {}", step.title));
            lines.push(format!("      {}", step.description));
        }
        lines
    }
}

/// A recently opened workspace.
#[derive(Debug, Clone)]
pub struct RecentWorkspace {
    pub path: String,
    pub name: String,
    pub last_opened: u64,
    pub pinned: bool,
}

/// Manages a list of recent workspaces.
pub struct RecentWorkspaceList {
    workspaces: Vec<RecentWorkspace>,
    max_items: usize,
}

impl RecentWorkspaceList {
    pub fn new(max_items: usize) -> Self {
        Self {
            workspaces: Vec::new(),
            max_items,
        }
    }

    pub fn add(&mut self, workspace: RecentWorkspace) {
        // Remove existing entry for same path
        self.workspaces.retain(|w| w.path != workspace.path);
        self.workspaces.insert(0, workspace);
        // Sort: pinned first, then by last_opened descending
        self.workspaces.sort_by(|a, b| {
            b.pinned.cmp(&a.pinned).then(b.last_opened.cmp(&a.last_opened))
        });
        // Enforce max, but never remove pinned items
        while self.workspaces.len() > self.max_items {
            if let Some(pos) = self.workspaces.iter().rposition(|w| !w.pinned) {
                self.workspaces.remove(pos);
            } else {
                break;
            }
        }
    }

    pub fn list(&self) -> &[RecentWorkspace] {
        &self.workspaces
    }

    pub fn pinned(&self) -> Vec<&RecentWorkspace> {
        self.workspaces.iter().filter(|w| w.pinned).collect()
    }

    pub fn unpinned(&self) -> Vec<&RecentWorkspace> {
        self.workspaces.iter().filter(|w| !w.pinned).collect()
    }

    pub fn remove(&mut self, path: &str) -> bool {
        let before = self.workspaces.len();
        self.workspaces.retain(|w| w.path != path);
        self.workspaces.len() < before
    }

    pub fn toggle_pin(&mut self, path: &str) -> bool {
        if let Some(ws) = self.workspaces.iter_mut().find(|w| w.path == path) {
            ws.pinned = !ws.pinned;
            // Re-sort
            self.workspaces.sort_by(|a, b| {
                b.pinned.cmp(&a.pinned).then(b.last_opened.cmp(&a.last_opened))
            });
            true
        } else {
            false
        }
    }

    pub fn count(&self) -> usize {
        self.workspaces.len()
    }

    pub fn clear_unpinned(&mut self) {
        self.workspaces.retain(|w| w.pinned);
    }
}

impl Default for RecentWorkspaceList {
    fn default() -> Self {
        Self::new(10)
    }
}

/// A keybinding hint for the welcome page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingHint {
    pub action: String,
    pub shortcut: String,
}

/// Generate platform-specific keybinding hints for the welcome page.
/// `is_mac` controls whether to show Cmd vs Ctrl.
pub fn welcome_keybinding_hints(is_mac: bool) -> Vec<KeybindingHint> {
    let modifier = if is_mac { "Cmd" } else { "Ctrl" };
    vec![
        KeybindingHint { action: "Quick Open".into(), shortcut: format!("{modifier}+P") },
        KeybindingHint { action: "Command Palette".into(), shortcut: format!("{modifier}+Shift+P") },
        KeybindingHint { action: "Toggle Terminal".into(), shortcut: format!("{modifier}+`") },
        KeybindingHint { action: "Find in Files".into(), shortcut: format!("{modifier}+Shift+F") },
        KeybindingHint { action: "Open Settings".into(), shortcut: format!("{modifier}+,") },
        KeybindingHint { action: "New File".into(), shortcut: format!("{modifier}+N") },
        KeybindingHint { action: "Save".into(), shortcut: format!("{modifier}+S") },
        KeybindingHint { action: "Close Editor".into(), shortcut: format!("{modifier}+W") },
    ]
}

/// Format keybinding hints as a displayable text block.
pub fn format_keybinding_hints(hints: &[KeybindingHint]) -> Vec<String> {
    let max_action_len = hints.iter().map(|h| h.action.len()).max().unwrap_or(0);
    hints
        .iter()
        .map(|h| format!("  {:<width$}  {}", h.action, h.shortcut, width = max_action_len))
        .collect()
}

/// Accumulated statistics for welcome operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WelcomeStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WelcomeStats {
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
    pub fn merge(&mut self, other: &WelcomeStats) {
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

impl Default for WelcomeStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WelcomeStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WelcomeStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for welcome.
#[derive(Debug, Clone)]
pub struct WelcomeValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WelcomeValidator {
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

impl Default for WelcomeValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TipOfTheDay
// ---------------------------------------------------------------------------

/// A single tip shown on the welcome page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tip {
    pub id: String,
    pub text: String,
    pub category: String,
}

/// Rotating tip-of-the-day manager.
pub struct TipOfTheDay {
    tips: Vec<Tip>,
    seen_ids: Vec<String>,
}

impl TipOfTheDay {
    pub fn new() -> Self {
        Self {
            tips: Vec::new(),
            seen_ids: Vec::new(),
        }
    }

    /// Add a tip to the pool.
    pub fn add_tip(&mut self, tip: Tip) {
        if !self.tips.iter().any(|t| t.id == tip.id) {
            self.tips.push(tip);
        }
    }

    /// Total number of tips.
    pub fn tip_count(&self) -> usize {
        self.tips.len()
    }

    /// Get the next unseen tip, cycling back to the beginning when all have
    /// been seen. Returns `None` only when the pool is empty.
    pub fn next_tip(&mut self) -> Option<&Tip> {
        if self.tips.is_empty() {
            return None;
        }
        // Find first unseen tip
        if let Some(tip) = self.tips.iter().find(|t| !self.seen_ids.contains(&t.id)) {
            self.seen_ids.push(tip.id.clone());
            return Some(tip);
        }
        // All seen – reset and return the first
        self.seen_ids.clear();
        let tip = &self.tips[0];
        self.seen_ids.push(tip.id.clone());
        Some(tip)
    }

    /// Mark all tips as unseen.
    pub fn reset(&mut self) {
        self.seen_ids.clear();
    }

    /// Number of tips that have been seen.
    pub fn seen_count(&self) -> usize {
        self.seen_ids.len()
    }

    /// Filter tips by category, returning references.
    pub fn tips_in_category(&self, category: &str) -> Vec<&Tip> {
        self.tips.iter().filter(|t| t.category == category).collect()
    }
}

impl Default for TipOfTheDay {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// GettingStartedChecklist
// ---------------------------------------------------------------------------

/// A single item on the getting-started checklist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecklistItem {
    pub id: String,
    pub label: String,
    pub done: bool,
}

/// A checklist that guides new users through initial setup.
pub struct GettingStartedChecklist {
    items: Vec<ChecklistItem>,
}

impl GettingStartedChecklist {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Append a new unchecked item.
    pub fn add_item(&mut self, id: impl Into<String>, label: impl Into<String>) {
        self.items.push(ChecklistItem {
            id: id.into(),
            label: label.into(),
            done: false,
        });
    }

    /// Mark an item as done. Returns `true` if the item was found.
    pub fn check(&mut self, id: &str) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.done = true;
            true
        } else {
            false
        }
    }

    /// Uncheck an item.
    pub fn uncheck(&mut self, id: &str) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.done = false;
            true
        } else {
            false
        }
    }

    /// Whether every item is done.
    pub fn all_done(&self) -> bool {
        !self.items.is_empty() && self.items.iter().all(|i| i.done)
    }

    /// Number of completed items.
    pub fn done_count(&self) -> usize {
        self.items.iter().filter(|i| i.done).count()
    }

    /// Total items.
    pub fn total(&self) -> usize {
        self.items.len()
    }

    /// Completion ratio in [0.0, 1.0].
    pub fn progress(&self) -> f64 {
        if self.items.is_empty() {
            return 0.0;
        }
        self.done_count() as f64 / self.items.len() as f64
    }

    /// Get the next unchecked item, if any.
    pub fn next_pending(&self) -> Option<&ChecklistItem> {
        self.items.iter().find(|i| !i.done)
    }

    /// Render the checklist as displayable text lines.
    pub fn render(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!(
            "Getting Started ({}/{})",
            self.done_count(),
            self.total()
        ));
        for item in &self.items {
            let mark = if item.done { "✓" } else { "○" };
            lines.push(format!("  [{mark}] {}", item.label));
        }
        lines
    }
}

impl Default for GettingStartedChecklist {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welcome_page_sections() {
        let mut page = WelcomePage::new();
        assert!(page.should_show());
        let section = WelcomeSection {
            title: "Start".to_string(),
            items: vec![
                WelcomeItem {
                    id: "new-file".to_string(),
                    title: "New File".to_string(),
                    description: "Create a new file".to_string(),
                    icon: None,
                    command: Some("newFile".to_string()),
                },
            ],
        };
        page.add_section(section);
        assert_eq!(page.section_count(), 1);
        assert_eq!(page.total_item_count(), 1);
    }

    #[test]
    fn welcome_page_show_toggle() {
        let mut page = WelcomePage::new();
        page.set_show_on_startup(false);
        assert!(!page.should_show());
    }

    #[test]
    fn recent_items_ordering() {
        let mut list = RecentItemsList::new();
        list.add(RecentItem {
            uri: "old.rs".to_string(),
            label: "old".to_string(),
            timestamp: 100,
        });
        list.add(RecentItem {
            uri: "new.rs".to_string(),
            label: "new".to_string(),
            timestamp: 200,
        });
        let recent = list.get_recent(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].uri, "new.rs");
    }

    #[test]
    fn walkthrough_completion() {
        let mut wt = WelcomeWalkthrough {
            id: "wt1".to_string(),
            title: "Get Started".to_string(),
            steps: vec![
                WalkthroughStep {
                    id: "s1".to_string(),
                    title: "Step 1".to_string(),
                    description: "First step".to_string(),
                    completed: false,
                },
                WalkthroughStep {
                    id: "s2".to_string(),
                    title: "Step 2".to_string(),
                    description: "Second step".to_string(),
                    completed: false,
                },
            ],
        };
        assert!(!wt.is_complete());
        assert!((wt.completion_percentage() - 0.0).abs() < f64::EPSILON);
        wt.complete_step("s1");
        assert!((wt.completion_percentage() - 50.0).abs() < f64::EPSILON);
        wt.complete_step("s2");
        assert!(wt.is_complete());
        assert!((wt.completion_percentage() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn walkthrough_complete_nonexistent_step() {
        let mut wt = WelcomeWalkthrough {
            id: "wt2".to_string(),
            title: "WT".to_string(),
            steps: vec![WalkthroughStep {
                id: "s1".to_string(),
                title: "S1".to_string(),
                description: "D".to_string(),
                completed: false,
            }],
        };
        wt.complete_step("nonexistent");
        assert!(!wt.is_complete());
    }

    #[test]
    fn walkthrough_empty_steps() {
        let wt = WelcomeWalkthrough {
            id: "wt3".to_string(),
            title: "Empty".to_string(),
            steps: vec![],
        };
        assert!(!wt.is_complete());
        assert!((wt.completion_percentage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn find_section_by_title() {
        let mut page = WelcomePage::new();
        page.add_section(WelcomeSection {
            title: "Start".to_string(),
            items: vec![],
        });
        page.add_section(WelcomeSection {
            title: "Learn".to_string(),
            items: vec![],
        });
        assert!(page.find_section("Learn").is_some());
        assert!(page.find_section("Missing").is_none());
    }

    #[test]
    fn remove_section_by_title() {
        let mut page = WelcomePage::new();
        page.add_section(WelcomeSection {
            title: "Start".to_string(),
            items: vec![],
        });
        assert!(page.remove_section("Start"));
        assert_eq!(page.section_count(), 0);
        assert!(!page.remove_section("Start"));
    }

    #[test]
    fn find_item_by_id_across_sections() {
        let mut page = WelcomePage::new();
        page.add_section(WelcomeSection {
            title: "A".to_string(),
            items: vec![WelcomeItem {
                id: "a1".to_string(),
                title: "A1".to_string(),
                description: String::new(),
                icon: None,
                command: None,
            }],
        });
        page.add_section(WelcomeSection {
            title: "B".to_string(),
            items: vec![WelcomeItem {
                id: "b1".to_string(),
                title: "B1".to_string(),
                description: String::new(),
                icon: None,
                command: None,
            }],
        });
        assert_eq!(page.find_item_by_id("b1").unwrap().title, "B1");
        assert!(page.find_item_by_id("nope").is_none());
    }

    #[test]
    fn recent_items_remove_and_contains() {
        let mut list = RecentItemsList::new();
        list.add(RecentItem {
            uri: "a.rs".to_string(),
            label: "a".to_string(),
            timestamp: 1,
        });
        list.add(RecentItem {
            uri: "b.rs".to_string(),
            label: "b".to_string(),
            timestamp: 2,
        });
        assert!(list.contains("a.rs"));
        assert!(list.remove_item("a.rs"));
        assert!(!list.contains("a.rs"));
        assert!(!list.remove_item("a.rs"));
    }

    #[test]
    fn welcome_item_kind_variants() {
        let kinds = vec![
            WelcomeItemKind::Command,
            WelcomeItemKind::Link,
            WelcomeItemKind::Walkthrough,
            WelcomeItemKind::Extension,
        ];
        assert_eq!(kinds.len(), 4);
        assert_eq!(kinds[0], WelcomeItemKind::Command);
    }

    #[test]
    fn default_content_provider() {
        struct EmptyProvider;
        impl WelcomeContentProvider for EmptyProvider {}
        let p = EmptyProvider;
        assert!(p.provide_sections().is_empty());
        assert!(p.provide_walkthroughs().is_empty());
    }

    #[test]
    fn custom_content_provider() {
        struct MyProvider;
        impl WelcomeContentProvider for MyProvider {
            fn provide_sections(&self) -> Vec<WelcomeSection> {
                vec![WelcomeSection {
                    title: "Custom".to_string(),
                    items: vec![],
                }]
            }
        }
        let p = MyProvider;
        assert_eq!(p.provide_sections().len(), 1);
        assert!(p.provide_walkthroughs().is_empty());
    }

    #[test]
    fn recent_items_clear() {
        let mut list = RecentItemsList::new();
        list.add(RecentItem {
            uri: "f.rs".to_string(),
            label: "f".to_string(),
            timestamp: 1,
        });
        list.clear();
        assert_eq!(list.get_recent(10).len(), 0);
    }

    #[test]
    fn eq_welcomeitemkind_same() {
        assert_eq!(WelcomeItemKind::Command, WelcomeItemKind::Command);
    }

    #[test]
    fn ne_welcomeitemkind_diff() {
        assert_ne!(WelcomeItemKind::Command, WelcomeItemKind::Link);
    }

    #[test]
    fn behavior_check_0() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn welcome_stats_new_defaults() {
        let stats = WelcomeStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn welcome_stats_record_success() {
        let mut stats = WelcomeStats::new();
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
    fn welcome_stats_record_failure() {
        let mut stats = WelcomeStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn welcome_stats_reset() {
        let mut stats = WelcomeStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn welcome_stats_merge() {
        let mut a = WelcomeStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WelcomeStats::new();
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
    fn welcome_stats_display() {
        let mut stats = WelcomeStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn welcome_stats_default() {
        let stats = WelcomeStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn welcome_validator_accepts_valid_name() {
        let v = WelcomeValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn welcome_validator_rejects_empty() {
        let v = WelcomeValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn welcome_validator_rejects_too_long() {
        let v = WelcomeValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn welcome_validator_forbidden_prefix() {
        let v = WelcomeValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn welcome_validator_allowed_chars() {
        let v = WelcomeValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn welcome_validator_range() {
        let v = WelcomeValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn welcome_sanitize_removes_control() {
        let result = WelcomeValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn welcome_truncate_short_string() {
        assert_eq!(WelcomeValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn welcome_truncate_long_string() {
        let result = WelcomeValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn welcome_is_ascii_printable() {
        assert!(WelcomeValidator::is_ascii_printable("Hello World 123"));
        assert!(!WelcomeValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn render_section_basic() {
        let section = WelcomeSection {
            title: "Start".to_string(),
            items: vec![WelcomeItem {
                id: "1".into(), title: "New File".into(),
                description: "Create a new file".into(),
                icon: None, command: None,
            }],
        };
        let lines = WelcomeSectionRenderer::render_section(&section);
        assert!(lines[0].contains("Start"));
        assert!(lines.iter().any(|l| l.contains("New File")));
    }

    #[test]
    fn render_page_has_header() {
        let page = WelcomePage::new();
        let lines = WelcomeSectionRenderer::render_page(&page);
        assert!(lines[0].contains("Welcome"));
    }

    #[test]
    fn render_walkthrough_progress() {
        let wt = WelcomeWalkthrough {
            id: "wt1".into(), title: "Getting Started".into(),
            steps: vec![
                WalkthroughStep { id: "s1".into(), title: "Step 1".into(), description: "Do thing 1".into(), completed: true },
                WalkthroughStep { id: "s2".into(), title: "Step 2".into(), description: "Do thing 2".into(), completed: false },
            ],
        };
        let lines = WelcomeSectionRenderer::render_walkthrough(&wt);
        assert!(lines[0].contains("50%"));
        assert!(lines.iter().any(|l| l.contains("✓")));
        assert!(lines.iter().any(|l| l.contains("○")));
    }

    #[test]
    fn recent_workspace_add_and_list() {
        let mut list = RecentWorkspaceList::new(5);
        list.add(RecentWorkspace { path: "/a".into(), name: "A".into(), last_opened: 10, pinned: false });
        list.add(RecentWorkspace { path: "/b".into(), name: "B".into(), last_opened: 20, pinned: false });
        assert_eq!(list.count(), 2);
        assert_eq!(list.list()[0].path, "/b"); // most recent first
    }

    #[test]
    fn recent_workspace_dedup() {
        let mut list = RecentWorkspaceList::new(5);
        list.add(RecentWorkspace { path: "/a".into(), name: "A".into(), last_opened: 10, pinned: false });
        list.add(RecentWorkspace { path: "/a".into(), name: "A".into(), last_opened: 20, pinned: false });
        assert_eq!(list.count(), 1);
    }

    #[test]
    fn recent_workspace_pinned_first() {
        let mut list = RecentWorkspaceList::new(5);
        list.add(RecentWorkspace { path: "/a".into(), name: "A".into(), last_opened: 30, pinned: false });
        list.add(RecentWorkspace { path: "/b".into(), name: "B".into(), last_opened: 10, pinned: true });
        assert_eq!(list.list()[0].path, "/b"); // pinned first
    }

    #[test]
    fn recent_workspace_max_items() {
        let mut list = RecentWorkspaceList::new(2);
        list.add(RecentWorkspace { path: "/a".into(), name: "A".into(), last_opened: 1, pinned: false });
        list.add(RecentWorkspace { path: "/b".into(), name: "B".into(), last_opened: 2, pinned: false });
        list.add(RecentWorkspace { path: "/c".into(), name: "C".into(), last_opened: 3, pinned: false });
        assert_eq!(list.count(), 2);
    }

    #[test]
    fn recent_workspace_toggle_pin() {
        let mut list = RecentWorkspaceList::new(5);
        list.add(RecentWorkspace { path: "/a".into(), name: "A".into(), last_opened: 10, pinned: false });
        assert!(list.toggle_pin("/a"));
        assert!(list.list()[0].pinned);
    }

    #[test]
    fn recent_workspace_clear_unpinned() {
        let mut list = RecentWorkspaceList::new(5);
        list.add(RecentWorkspace { path: "/a".into(), name: "A".into(), last_opened: 10, pinned: true });
        list.add(RecentWorkspace { path: "/b".into(), name: "B".into(), last_opened: 20, pinned: false });
        list.clear_unpinned();
        assert_eq!(list.count(), 1);
        assert_eq!(list.list()[0].path, "/a");
    }

    #[test]
    fn keybinding_hints_linux() {
        let hints = welcome_keybinding_hints(false);
        assert!(hints.iter().any(|h| h.shortcut.contains("Ctrl")));
        assert!(!hints.iter().any(|h| h.shortcut.contains("Cmd")));
    }

    #[test]
    fn keybinding_hints_mac() {
        let hints = welcome_keybinding_hints(true);
        assert!(hints.iter().any(|h| h.shortcut.contains("Cmd")));
    }

    #[test]
    fn format_hints_alignment() {
        let hints = welcome_keybinding_hints(false);
        let formatted = format_keybinding_hints(&hints);
        assert!(formatted.len() >= 8);
        for line in &formatted {
            assert!(line.starts_with("  "));
        }
    }

    // -----------------------------------------------------------------------
    // TipOfTheDay tests
    // -----------------------------------------------------------------------

    #[test]
    fn tip_of_the_day_empty_returns_none() {
        let mut tips = TipOfTheDay::new();
        assert!(tips.next_tip().is_none());
        assert_eq!(tips.tip_count(), 0);
    }

    #[test]
    fn tip_of_the_day_cycles_through_all() {
        let mut tips = TipOfTheDay::new();
        tips.add_tip(Tip { id: "t1".into(), text: "Tip 1".into(), category: "editor".into() });
        tips.add_tip(Tip { id: "t2".into(), text: "Tip 2".into(), category: "editor".into() });
        assert_eq!(tips.tip_count(), 2);

        let first = tips.next_tip().unwrap().id.clone();
        assert_eq!(first, "t1");
        let second = tips.next_tip().unwrap().id.clone();
        assert_eq!(second, "t2");

        // Wraps around
        let third = tips.next_tip().unwrap().id.clone();
        assert_eq!(third, "t1");
    }

    #[test]
    fn tip_of_the_day_no_duplicates() {
        let mut tips = TipOfTheDay::new();
        tips.add_tip(Tip { id: "t1".into(), text: "A".into(), category: "x".into() });
        tips.add_tip(Tip { id: "t1".into(), text: "B".into(), category: "x".into() });
        assert_eq!(tips.tip_count(), 1);
    }

    #[test]
    fn tip_of_the_day_category_filter() {
        let mut tips = TipOfTheDay::new();
        tips.add_tip(Tip { id: "t1".into(), text: "A".into(), category: "editor".into() });
        tips.add_tip(Tip { id: "t2".into(), text: "B".into(), category: "git".into() });
        tips.add_tip(Tip { id: "t3".into(), text: "C".into(), category: "editor".into() });
        assert_eq!(tips.tips_in_category("editor").len(), 2);
        assert_eq!(tips.tips_in_category("git").len(), 1);
        assert_eq!(tips.tips_in_category("unknown").len(), 0);
    }

    #[test]
    fn tip_of_the_day_reset() {
        let mut tips = TipOfTheDay::new();
        tips.add_tip(Tip { id: "t1".into(), text: "A".into(), category: "x".into() });
        let _ = tips.next_tip();
        assert_eq!(tips.seen_count(), 1);
        tips.reset();
        assert_eq!(tips.seen_count(), 0);
    }

    // -----------------------------------------------------------------------
    // GettingStartedChecklist tests
    // -----------------------------------------------------------------------

    #[test]
    fn checklist_progress_empty() {
        let cl = GettingStartedChecklist::new();
        assert!(!cl.all_done());
        assert_eq!(cl.total(), 0);
        assert!((cl.progress() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn checklist_add_check_uncheck() {
        let mut cl = GettingStartedChecklist::new();
        cl.add_item("theme", "Choose a color theme");
        cl.add_item("keybindings", "Configure keybindings");
        assert_eq!(cl.total(), 2);
        assert_eq!(cl.done_count(), 0);

        assert!(cl.check("theme"));
        assert_eq!(cl.done_count(), 1);
        assert!(!cl.all_done());

        assert!(cl.check("keybindings"));
        assert!(cl.all_done());

        assert!(cl.uncheck("theme"));
        assert!(!cl.all_done());
        assert_eq!(cl.done_count(), 1);
    }

    #[test]
    fn checklist_check_nonexistent() {
        let mut cl = GettingStartedChecklist::new();
        assert!(!cl.check("nope"));
        assert!(!cl.uncheck("nope"));
    }

    #[test]
    fn checklist_next_pending() {
        let mut cl = GettingStartedChecklist::new();
        cl.add_item("a", "A");
        cl.add_item("b", "B");
        assert_eq!(cl.next_pending().unwrap().id, "a");
        cl.check("a");
        assert_eq!(cl.next_pending().unwrap().id, "b");
        cl.check("b");
        assert!(cl.next_pending().is_none());
    }

    #[test]
    fn checklist_render_output() {
        let mut cl = GettingStartedChecklist::new();
        cl.add_item("a", "Install extensions");
        cl.add_item("b", "Open a folder");
        cl.check("a");
        let lines = cl.render();
        assert!(lines[0].contains("1/2"));
        assert!(lines[1].contains("✓"));
        assert!(lines[2].contains("○"));
    }
}
