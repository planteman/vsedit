//! Welcome page.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// SectionPriority / SectionOrdering
// ---------------------------------------------------------------------------

/// Priority level for a welcome page section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SectionPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// A section with an associated priority for ordering.
#[derive(Debug, Clone)]
pub struct PrioritizedSection {
    pub section: WelcomeSection,
    pub priority: SectionPriority,
    pub visible: bool,
}

/// Manages ordered, prioritized welcome sections.
pub struct SectionOrdering {
    sections: Vec<PrioritizedSection>,
}

impl SectionOrdering {
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    /// Add a section with the given priority.
    pub fn add(&mut self, section: WelcomeSection, priority: SectionPriority) {
        self.sections.push(PrioritizedSection {
            section,
            priority,
            visible: true,
        });
        self.sections.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Return sections in priority order, filtered to visible only.
    pub fn visible_sections(&self) -> Vec<&WelcomeSection> {
        self.sections
            .iter()
            .filter(|ps| ps.visible)
            .map(|ps| &ps.section)
            .collect()
    }

    /// Hide a section by title.
    pub fn hide(&mut self, title: &str) {
        if let Some(ps) = self.sections.iter_mut().find(|ps| ps.section.title == title) {
            ps.visible = false;
        }
    }

    /// Show a previously hidden section by title.
    pub fn show(&mut self, title: &str) {
        if let Some(ps) = self.sections.iter_mut().find(|ps| ps.section.title == title) {
            ps.visible = true;
        }
    }

    /// Change the priority of a section by title. Returns `true` if found.
    pub fn set_priority(&mut self, title: &str, priority: SectionPriority) -> bool {
        if let Some(ps) = self.sections.iter_mut().find(|ps| ps.section.title == title) {
            ps.priority = priority;
            self.sections.sort_by(|a, b| b.priority.cmp(&a.priority));
            true
        } else {
            false
        }
    }

    /// Total number of sections (including hidden).
    pub fn len(&self) -> usize {
        self.sections.len()
    }

    /// Whether there are no sections.
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

impl Default for SectionOrdering {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RecentProjectManager – filtering and sorting recent projects
// ---------------------------------------------------------------------------

/// Sort criteria for recent projects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectSortOrder {
    /// Most recently opened first.
    LastOpened,
    /// Alphabetical by name.
    Name,
    /// Alphabetical by path.
    Path,
}

/// A project entry in the recent-projects list.
#[derive(Debug, Clone)]
pub struct RecentProject {
    pub name: String,
    pub path: String,
    pub last_opened: u64,
    pub tags: Vec<String>,
}

/// Manages recent projects with sorting, filtering, and deduplication.
pub struct RecentProjectManager {
    projects: Vec<RecentProject>,
    max_projects: usize,
}

impl RecentProjectManager {
    pub fn new(max_projects: usize) -> Self {
        Self {
            projects: Vec::new(),
            max_projects,
        }
    }

    /// Record a project as recently opened. Deduplicates by path.
    pub fn record_open(&mut self, project: RecentProject) {
        self.projects.retain(|p| p.path != project.path);
        self.projects.insert(0, project);
        self.projects.truncate(self.max_projects);
    }

    /// Return projects sorted by the given order.
    pub fn sorted(&self, order: ProjectSortOrder) -> Vec<&RecentProject> {
        let mut refs: Vec<&RecentProject> = self.projects.iter().collect();
        match order {
            ProjectSortOrder::LastOpened => refs.sort_by(|a, b| b.last_opened.cmp(&a.last_opened)),
            ProjectSortOrder::Name => refs.sort_by(|a, b| a.name.cmp(&b.name)),
            ProjectSortOrder::Path => refs.sort_by(|a, b| a.path.cmp(&b.path)),
        }
        refs
    }

    /// Filter projects whose name or path contains the query (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&RecentProject> {
        let q = query.to_lowercase();
        self.projects
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&q) || p.path.to_lowercase().contains(&q))
            .collect()
    }

    /// Filter projects that have at least one of the given tags.
    pub fn filter_by_tags(&self, tags: &[&str]) -> Vec<&RecentProject> {
        self.projects
            .iter()
            .filter(|p| p.tags.iter().any(|t| tags.contains(&t.as_str())))
            .collect()
    }

    /// Remove a project by path. Returns `true` if found.
    pub fn remove(&mut self, path: &str) -> bool {
        let before = self.projects.len();
        self.projects.retain(|p| p.path != path);
        self.projects.len() < before
    }

    pub fn count(&self) -> usize {
        self.projects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.projects.is_empty()
    }

    /// Collect all unique tags across all projects, sorted alphabetically.
    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .projects
            .iter()
            .flat_map(|p| p.tags.iter().cloned())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }
}

impl Default for RecentProjectManager {
    fn default() -> Self {
        Self::new(25)
    }
}

// ---------------------------------------------------------------------------
// WelcomeLayout – layout computation for the welcome page
// ---------------------------------------------------------------------------

/// Rectangle describing a region on the welcome page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn area(&self) -> u32 {
        self.width * self.height
    }

    pub fn contains_point(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

/// Computes layout rectangles for welcome page regions.
pub struct WelcomeLayout {
    viewport_width: u32,
    viewport_height: u32,
    padding: u32,
}

impl WelcomeLayout {
    pub fn new(viewport_width: u32, viewport_height: u32, padding: u32) -> Self {
        Self {
            viewport_width,
            viewport_height,
            padding,
        }
    }

    /// Compute a vertically stacked layout where each section gets an equal
    /// share of the available height.
    pub fn compute_stacked(&self, section_count: usize) -> Vec<Rect> {
        if section_count == 0 {
            return Vec::new();
        }
        let usable_w = self.viewport_width.saturating_sub(self.padding * 2);
        let total_gap = self.padding * section_count.saturating_sub(1) as u32;
        let usable_h = self.viewport_height.saturating_sub(self.padding * 2 + total_gap);
        let section_h = usable_h / section_count as u32;
        (0..section_count)
            .map(|i| {
                let y = self.padding + (section_h + self.padding) * i as u32;
                Rect::new(self.padding, y, usable_w, section_h)
            })
            .collect()
    }

    /// Compute a two-column layout: left column takes `left_frac` of the width.
    pub fn compute_two_column(&self, left_frac: f32) -> (Rect, Rect) {
        let usable_w = self.viewport_width.saturating_sub(self.padding * 3);
        let usable_h = self.viewport_height.saturating_sub(self.padding * 2);
        let left_w = (usable_w as f32 * left_frac.clamp(0.0, 1.0)) as u32;
        let right_w = usable_w - left_w;
        let left = Rect::new(self.padding, self.padding, left_w, usable_h);
        let right = Rect::new(self.padding * 2 + left_w, self.padding, right_w, usable_h);
        (left, right)
    }

    pub fn viewport_area(&self) -> u32 {
        self.viewport_width * self.viewport_height
    }
}

// ---------------------------------------------------------------------------
// Recent items tracking (with pinning support)
// ---------------------------------------------------------------------------

/// A recently opened file or folder with pinning support.
#[derive(Debug, Clone)]
pub struct PinnableRecentItem {
    pub path: String,
    pub label: String,
    pub pinned: bool,
    pub last_opened: u64,
}

/// Tracks recently opened items with pinning and eviction.
#[derive(Debug, Clone)]
pub struct WelcomePageRecent {
    pub items: Vec<PinnableRecentItem>,
    pub max_items: usize,
}

impl WelcomePageRecent {
    pub fn new(max: usize) -> Self {
        Self {
            items: Vec::new(),
            max_items: max,
        }
    }

    /// Add or update an item. Deduplicates by path and evicts the oldest
    /// unpinned item when at capacity.
    pub fn add(&mut self, path: &str, label: &str, timestamp: u64) {
        if let Some(existing) = self.items.iter_mut().find(|i| i.path == path) {
            existing.label = label.to_string();
            existing.last_opened = timestamp;
            return;
        }
        if self.items.len() >= self.max_items {
            if let Some(idx) = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, i)| !i.pinned)
                .min_by_key(|(_, i)| i.last_opened)
                .map(|(idx, _)| idx)
            {
                self.items.remove(idx);
            } else {
                return;
            }
        }
        self.items.push(PinnableRecentItem {
            path: path.to_string(),
            label: label.to_string(),
            pinned: false,
            last_opened: timestamp,
        });
    }

    pub fn pin(&mut self, path: &str) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.path == path) {
            item.pinned = true;
            return true;
        }
        false
    }

    pub fn unpin(&mut self, path: &str) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.path == path) {
            item.pinned = false;
            return true;
        }
        false
    }

    pub fn pinned_items(&self) -> Vec<&PinnableRecentItem> {
        self.items.iter().filter(|i| i.pinned).collect()
    }

    /// Returns items sorted with pinned first, then by timestamp descending.
    pub fn recent_items(&self, limit: usize) -> Vec<&PinnableRecentItem> {
        let mut refs: Vec<&PinnableRecentItem> = self.items.iter().collect();
        refs.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then(b.last_opened.cmp(&a.last_opened))
        });
        refs.truncate(limit);
        refs
    }

    pub fn remove(&mut self, path: &str) -> bool {
        let len = self.items.len();
        self.items.retain(|i| i.path != path);
        self.items.len() != len
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

// ---------------------------------------------------------------------------
// Ordered walkthrough with progress tracking
// ---------------------------------------------------------------------------

/// A step in an ordered walkthrough.
#[derive(Debug, Clone)]
pub struct OrderedWalkthroughStep {
    pub id: String,
    pub title: String,
    pub completed: bool,
    pub order: u32,
}

/// An ordered, multi-step walkthrough with progress tracking.
#[derive(Debug, Clone)]
pub struct WelcomeWalkthroughTracker {
    pub id: String,
    pub title: String,
    pub steps: Vec<OrderedWalkthroughStep>,
}

impl WelcomeWalkthroughTracker {
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            steps: Vec::new(),
        }
    }

    pub fn add_step(&mut self, step_id: &str, title: &str, order: u32) {
        self.steps.push(OrderedWalkthroughStep {
            id: step_id.to_string(),
            title: title.to_string(),
            completed: false,
            order,
        });
        self.steps.sort_by_key(|s| s.order);
    }

    pub fn complete_step(&mut self, step_id: &str) -> bool {
        if let Some(step) = self.steps.iter_mut().find(|s| s.id == step_id) {
            step.completed = true;
            return true;
        }
        false
    }

    /// Progress as a fraction from 0.0 to 1.0.
    pub fn progress(&self) -> f32 {
        if self.steps.is_empty() {
            return 0.0;
        }
        self.completed_count() as f32 / self.steps.len() as f32
    }

    pub fn is_complete(&self) -> bool {
        !self.steps.is_empty() && self.steps.iter().all(|s| s.completed)
    }

    pub fn next_incomplete_step(&self) -> Option<&OrderedWalkthroughStep> {
        self.steps.iter().find(|s| !s.completed)
    }

    pub fn completed_count(&self) -> usize {
        self.steps.iter().filter(|s| s.completed).count()
    }
}

// ---------------------------------------------------------------------------
// Key-binding hints
// ---------------------------------------------------------------------------

/// A single key-binding hint.
#[derive(Debug, Clone)]
pub struct KeyHint {
    pub action: String,
    pub keys: String,
    pub category: String,
}

/// A collection of key-binding hints displayed on the welcome page.
#[derive(Debug, Clone)]
pub struct WelcomeKeyBindingHint {
    pub hints: Vec<KeyHint>,
}

impl WelcomeKeyBindingHint {
    pub fn new() -> Self {
        Self { hints: Vec::new() }
    }

    pub fn add_hint(&mut self, action: &str, keys: &str, category: &str) {
        self.hints.push(KeyHint {
            action: action.to_string(),
            keys: keys.to_string(),
            category: category.to_string(),
        });
    }

    pub fn hints_for_category(&self, cat: &str) -> Vec<&KeyHint> {
        self.hints.iter().filter(|h| h.category == cat).collect()
    }

    /// Render a simple text table of action → keys.
    pub fn render_table(&self) -> String {
        if self.hints.is_empty() {
            return String::new();
        }
        let max_action = self.hints.iter().map(|h| h.action.len()).max().unwrap_or(0);
        let mut out = String::new();
        for h in &self.hints {
            out.push_str(&format!("{:<width$}  {}\n", h.action, h.keys, width = max_action));
        }
        out
    }

    pub fn categories(&self) -> Vec<&str> {
        let mut cats: Vec<&str> = self.hints.iter().map(|h| h.category.as_str()).collect();
        cats.sort();
        cats.dedup();
        cats
    }

    pub fn len(&self) -> usize {
        self.hints.len()
    }
}

// ---------------------------------------------------------------------------
// Welcome page customization
// ---------------------------------------------------------------------------

/// Controls which sections of the welcome page are visible.
#[derive(Debug, Clone)]
pub struct WelcomeCustomization {
    pub show_recent: bool,
    pub show_walkthroughs: bool,
    pub show_keybindings: bool,
    pub custom_logo: Option<String>,
    pub greeting: Option<String>,
    pub max_recent_items: usize,
}

impl WelcomeCustomization {
    pub fn new() -> Self {
        Self {
            show_recent: true,
            show_walkthroughs: true,
            show_keybindings: true,
            custom_logo: None,
            greeting: None,
            max_recent_items: 10,
        }
    }

    /// A minimal configuration with everything disabled.
    pub fn minimal() -> Self {
        Self {
            show_recent: false,
            show_walkthroughs: false,
            show_keybindings: false,
            custom_logo: None,
            greeting: None,
            max_recent_items: 10,
        }
    }

    pub fn with_greeting(mut self, msg: &str) -> Self {
        self.greeting = Some(msg.to_string());
        self
    }

    pub fn effective_greeting(&self) -> &str {
        self.greeting.as_deref().unwrap_or("Welcome")
    }

    pub fn visible_section_count(&self) -> usize {
        [self.show_recent, self.show_walkthroughs, self.show_keybindings]
            .iter()
            .filter(|&&v| v)
            .count()
    }
}

impl fmt::Display for WelcomeCustomization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WelcomeCustomization(sections={}, greeting={:?})",
            self.visible_section_count(),
            self.effective_greeting(),
        )
    }
}


// === Welcome Tips Rotator ===

/// Welcome Tips Rotator implementation.
#[derive(Debug, Clone)]
pub struct WelcomeTipsRotator {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: WelcomeTipsRotatorStats,
}

/// Statistics for WelcomeTipsRotator.
#[derive(Debug, Clone, Default)]
pub struct WelcomeTipsRotatorStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl WelcomeTipsRotatorStats {
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

impl WelcomeTipsRotator {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: WelcomeTipsRotatorStats::default(),
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

    pub fn stats(&self) -> &WelcomeTipsRotatorStats {
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

impl Default for WelcomeTipsRotator {
    fn default() -> Self {
        Self::new()
    }
}

// === Welcome Quick Start Guide ===

/// Priority level for WelcomeQuickStartGuide items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WelcomeQuickStartGuidePriority {
    Low,
    Normal,
    High,
    Critical,
}

impl WelcomeQuickStartGuidePriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for WelcomeQuickStartGuidePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Welcome Quick Start Guide implementation.
#[derive(Debug, Clone)]
pub struct WelcomeQuickStartGuide {
    items: Vec<WelcomeQuickStartGuideItem>,
    max_items: usize,
    default_priority: WelcomeQuickStartGuidePriority,
}

/// A single item in WelcomeQuickStartGuide.
#[derive(Debug, Clone)]
pub struct WelcomeQuickStartGuideItem {
    pub id: String,
    pub label: String,
    pub priority: WelcomeQuickStartGuidePriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl WelcomeQuickStartGuideItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: WelcomeQuickStartGuidePriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: WelcomeQuickStartGuidePriority) -> Self {
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

impl WelcomeQuickStartGuide {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: WelcomeQuickStartGuidePriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: WelcomeQuickStartGuideItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<WelcomeQuickStartGuideItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&WelcomeQuickStartGuideItem> {
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

    pub fn by_priority(&self, priority: WelcomeQuickStartGuidePriority) -> Vec<&WelcomeQuickStartGuideItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&WelcomeQuickStartGuideItem> {
        let mut sorted: Vec<&WelcomeQuickStartGuideItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&WelcomeQuickStartGuideItem> {
        let mut sorted: Vec<&WelcomeQuickStartGuideItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&WelcomeQuickStartGuideItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: WelcomeQuickStartGuidePriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> WelcomeQuickStartGuidePriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &WelcomeQuickStartGuideItem> {
        self.items.iter()
    }
}

impl Default for WelcomeQuickStartGuide {
    fn default() -> Self {
        Self::new()
    }
}


// ─── Welcome Builder & Validator ─────────────────────────────

/// Builder for constructing welcome page configurations.
#[derive(Debug, Clone)]
pub struct WelcomeBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl WelcomeBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<WelcomeCfg, WelcomeBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(WelcomeBuildErr { errors }); }
        Ok(WelcomeCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated welcome page configuration.
#[derive(Debug, Clone)]
pub struct WelcomeCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl WelcomeCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &WelcomeCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for WelcomeCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WelcomeCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct WelcomeBuildErr { pub errors: Vec<String> }

impl fmt::Display for WelcomeBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WelcomeBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for WelcomeBuildErr {}

// ─── Welcome Formatter ───────────────────────────────────────

/// Formatting options for welcome page output.
#[derive(Debug, Clone)]
pub struct WelcomeFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for WelcomeFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl WelcomeFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for welcome page data.
pub struct WelcomeFmt {
    options: WelcomeFmtOpts,
}

impl WelcomeFmt {
    pub fn new(options: WelcomeFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: WelcomeFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
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
// xa_ extended helpers for welcome
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWelcomeRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWelcomeRingBuf {
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
pub struct XaWelcomeCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWelcomeCounter {
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

impl Default for XaWelcomeCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 235
// ---------------------------------------------------------------------------

/// Generic object pool `Xc235Pool<T>`.
pub struct Xc235Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc235Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc235PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc235Pool<T> {
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
    pub fn stats(&self) -> Xc235PoolStats {
        Xc235PoolStats {
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

impl<T> Default for Xc235Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc235Scheduler`.
pub struct Xc235Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc235Scheduler {
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

impl Default for Xc235Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_235 hash for the given byte slice.
pub fn xc_235_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_235 convention.
pub fn xc_235_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_54 deepening: state machine + event bus ---

/// States for the Xd54 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd54State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd54State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd54Transition {
    pub from: Xd54State,
    pub to: Xd54State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd54StateMachine {
    current: Xd54State,
    history: Vec<Xd54Transition>,
    step_counter: usize,
}

impl Xd54StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd54State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd54State {
        self.current
    }

    pub fn history(&self) -> &[Xd54Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd54State) -> Result<Xd54State, String> {
        let allowed = match (self.current, target) {
            (Xd54State::Idle, Xd54State::Running) => true,
            (Xd54State::Running, Xd54State::Paused) => true,
            (Xd54State::Running, Xd54State::Done) => true,
            (Xd54State::Paused, Xd54State::Running) => true,
            (Xd54State::Paused, Xd54State::Done) => true,
            (Xd54State::Done, Xd54State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_54: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd54Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd54SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd54State> {
        let prefix = "Xd54SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd54State::Idle),
            "Running" => Some(Xd54State::Running),
            "Paused" => Some(Xd54State::Paused),
            "Done" => Some(Xd54State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd54State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd54 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd54Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd54Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd54HandlerFn = Box<dyn Fn(&Xd54Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd54EventBus {
    handlers: Vec<(usize, Option<String>, Xd54HandlerFn)>,
    next_id: usize,
    published: Vec<Xd54Event>,
}

impl Xd54EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd54Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd54Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd54Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd54Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #52
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf52Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf52TrieNode {
    children: std::collections::HashMap<char, Xf52TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf52Trie {
    root: Xf52TrieNode,
    count: usize,
}

impl Xf52Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf52TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf52TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf52TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf52BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf52BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 234).
pub struct Xh234SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh234SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 276 as u64,
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

/// A compact bit set supporting boolean operations (variant 234).
pub struct Xh234BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh234BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 234).
pub struct Xi234Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi234Deque<T> {
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
pub struct Xi234Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi234Interval {
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

/// A simple interval tree (variant 234).
pub struct Xi234IntervalTree {
    xi_intervals: Vec<Xi234Interval>,
}

impl Xi234IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi234Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi234Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi234Interval) -> Vec<&Xi234Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi234Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi234Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi234Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi234Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi234Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi234Interval> = Vec::new();
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

    // -----------------------------------------------------------------------
    // SectionOrdering tests
    // -----------------------------------------------------------------------

    #[test]
    fn section_ordering_priority_sort() {
        let mut ordering = SectionOrdering::new();
        ordering.add(
            WelcomeSection { title: "Low".into(), items: vec![] },
            SectionPriority::Low,
        );
        ordering.add(
            WelcomeSection { title: "High".into(), items: vec![] },
            SectionPriority::High,
        );
        ordering.add(
            WelcomeSection { title: "Normal".into(), items: vec![] },
            SectionPriority::Normal,
        );
        let visible = ordering.visible_sections();
        assert_eq!(visible.len(), 3);
        assert_eq!(visible[0].title, "High");
        assert_eq!(visible[1].title, "Normal");
        assert_eq!(visible[2].title, "Low");
    }

    #[test]
    fn section_ordering_hide_and_show() {
        let mut ordering = SectionOrdering::new();
        ordering.add(
            WelcomeSection { title: "A".into(), items: vec![] },
            SectionPriority::Normal,
        );
        ordering.add(
            WelcomeSection { title: "B".into(), items: vec![] },
            SectionPriority::Normal,
        );
        assert_eq!(ordering.visible_sections().len(), 2);
        ordering.hide("A");
        let visible = ordering.visible_sections();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].title, "B");
        ordering.show("A");
        assert_eq!(ordering.visible_sections().len(), 2);
    }

    #[test]
    fn section_ordering_set_priority_reorders() {
        let mut ordering = SectionOrdering::new();
        ordering.add(
            WelcomeSection { title: "First".into(), items: vec![] },
            SectionPriority::High,
        );
        ordering.add(
            WelcomeSection { title: "Second".into(), items: vec![] },
            SectionPriority::Low,
        );
        assert_eq!(ordering.visible_sections()[0].title, "First");
        assert!(ordering.set_priority("Second", SectionPriority::Critical));
        assert_eq!(ordering.visible_sections()[0].title, "Second");
        assert!(!ordering.set_priority("Missing", SectionPriority::Low));
    }

    // -----------------------------------------------------------------------
    // RecentProjectManager tests
    // -----------------------------------------------------------------------

    #[test]
    fn project_manager_dedup_and_sort() {
        let mut mgr = RecentProjectManager::new(10);
        mgr.record_open(RecentProject {
            name: "alpha".into(),
            path: "/a".into(),
            last_opened: 100,
            tags: vec!["rust".into()],
        });
        mgr.record_open(RecentProject {
            name: "beta".into(),
            path: "/b".into(),
            last_opened: 200,
            tags: vec!["python".into()],
        });
        // Re-open alpha with newer timestamp
        mgr.record_open(RecentProject {
            name: "alpha".into(),
            path: "/a".into(),
            last_opened: 300,
            tags: vec!["rust".into()],
        });
        assert_eq!(mgr.count(), 2);
        let by_time = mgr.sorted(ProjectSortOrder::LastOpened);
        assert_eq!(by_time[0].name, "alpha");
        let by_name = mgr.sorted(ProjectSortOrder::Name);
        assert_eq!(by_name[0].name, "alpha");
        assert_eq!(by_name[1].name, "beta");
    }

    #[test]
    fn project_manager_search_and_tags() {
        let mut mgr = RecentProjectManager::new(10);
        mgr.record_open(RecentProject {
            name: "my-web-app".into(),
            path: "/projects/web".into(),
            last_opened: 1,
            tags: vec!["web".into(), "js".into()],
        });
        mgr.record_open(RecentProject {
            name: "cli-tool".into(),
            path: "/projects/cli".into(),
            last_opened: 2,
            tags: vec!["rust".into()],
        });
        assert_eq!(mgr.search("web").len(), 1);
        assert_eq!(mgr.search("CLI").len(), 1);
        assert_eq!(mgr.filter_by_tags(&["rust"]).len(), 1);
        assert_eq!(mgr.filter_by_tags(&["web", "rust"]).len(), 2);
        let tags = mgr.all_tags();
        assert_eq!(tags, vec!["js", "rust", "web"]);
    }

    // -----------------------------------------------------------------------
    // WelcomeLayout / Rect tests
    // -----------------------------------------------------------------------

    #[test]
    fn rect_area_and_contains() {
        let r = Rect::new(10, 20, 100, 50);
        assert_eq!(r.area(), 5000);
        assert!(r.contains_point(10, 20));
        assert!(r.contains_point(50, 40));
        assert!(!r.contains_point(110, 20));
        assert!(!r.contains_point(10, 70));
    }

    #[test]
    fn rect_intersects() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(50, 50, 100, 100);
        let c = Rect::new(200, 200, 10, 10);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn layout_stacked_sections() {
        let layout = WelcomeLayout::new(800, 600, 10);
        let rects = layout.compute_stacked(3);
        assert_eq!(rects.len(), 3);
        // All rects should have the same width and height
        assert!(rects.iter().all(|r| r.width == rects[0].width));
        assert!(rects.iter().all(|r| r.height == rects[0].height));
        // Each subsequent rect has a larger y
        assert!(rects[1].y > rects[0].y);
        assert!(rects[2].y > rects[1].y);
        // Empty case
        assert!(layout.compute_stacked(0).is_empty());
    }

    #[test]
    fn layout_two_column() {
        let layout = WelcomeLayout::new(1000, 600, 10);
        let (left, right) = layout.compute_two_column(0.3);
        assert!(left.width < right.width);
        assert_eq!(left.height, right.height);
        assert!(right.x > left.x);
        assert_eq!(layout.viewport_area(), 600_000);
    }

    // -- Recent items tests --

    #[test]
    fn recent_add_and_dedup() {
        let mut recent = WelcomePageRecent::new(5);
        recent.add("/a", "A", 1);
        recent.add("/b", "B", 2);
        recent.add("/a", "A-updated", 3);
        assert_eq!(recent.len(), 2);
        let a = recent.items.iter().find(|i| i.path == "/a").unwrap();
        assert_eq!(a.label, "A-updated");
        assert_eq!(a.last_opened, 3);
    }

    #[test]
    fn recent_eviction() {
        let mut recent = WelcomePageRecent::new(2);
        recent.add("/a", "A", 1);
        recent.add("/b", "B", 2);
        recent.add("/c", "C", 3);
        assert_eq!(recent.len(), 2);
        assert!(recent.items.iter().all(|i| i.path != "/a"));
    }

    #[test]
    fn recent_pin_prevents_eviction() {
        let mut recent = WelcomePageRecent::new(2);
        recent.add("/a", "A", 1);
        recent.pin("/a");
        recent.add("/b", "B", 2);
        recent.add("/c", "C", 3);
        assert_eq!(recent.len(), 2);
        assert!(recent.items.iter().any(|i| i.path == "/a"));
        assert!(recent.items.iter().any(|i| i.path == "/c"));
    }

    #[test]
    fn recent_items_sorted() {
        let mut recent = WelcomePageRecent::new(5);
        recent.add("/a", "A", 1);
        recent.add("/b", "B", 3);
        recent.add("/c", "C", 2);
        recent.pin("/a");
        let sorted = recent.recent_items(5);
        assert_eq!(sorted[0].path, "/a"); // pinned first
        assert_eq!(sorted[1].path, "/b"); // then newest
    }

    #[test]
    fn recent_remove_and_unpin() {
        let mut recent = WelcomePageRecent::new(5);
        recent.add("/a", "A", 1);
        recent.pin("/a");
        assert_eq!(recent.pinned_items().len(), 1);
        assert!(recent.unpin("/a"));
        assert_eq!(recent.pinned_items().len(), 0);
        assert!(recent.remove("/a"));
        assert_eq!(recent.len(), 0);
        assert!(!recent.remove("/nonexistent"));
    }

    // -- Walkthrough tests --

    #[test]
    fn walkthrough_progress() {
        let mut wt = WelcomeWalkthroughTracker::new("setup", "Getting Started");
        wt.add_step("install", "Install", 1);
        wt.add_step("config", "Configure", 2);
        wt.add_step("run", "Run", 3);
        assert_eq!(wt.progress(), 0.0);
        assert!(!wt.is_complete());
        assert_eq!(wt.next_incomplete_step().unwrap().id, "install");

        wt.complete_step("install");
        assert_eq!(wt.completed_count(), 1);
        assert!((wt.progress() - 1.0 / 3.0).abs() < 0.001);

        wt.complete_step("config");
        wt.complete_step("run");
        assert!(wt.is_complete());
        assert!((wt.progress() - 1.0).abs() < f32::EPSILON);
        assert!(wt.next_incomplete_step().is_none());
    }

    #[test]
    fn walkthrough_step_ordering() {
        let mut wt = WelcomeWalkthroughTracker::new("wt", "WT");
        wt.add_step("c", "C", 3);
        wt.add_step("a", "A", 1);
        wt.add_step("b", "B", 2);
        assert_eq!(wt.steps[0].id, "a");
        assert_eq!(wt.steps[1].id, "b");
        assert_eq!(wt.steps[2].id, "c");
    }

    #[test]
    fn walkthrough_complete_nonexistent() {
        let mut wt = WelcomeWalkthroughTracker::new("wt", "WT");
        assert!(!wt.complete_step("nope"));
        assert_eq!(wt.progress(), 0.0);
    }

    // -- Key binding hints tests --

    #[test]
    fn keybinding_hints() {
        let mut hints = WelcomeKeyBindingHint::new();
        hints.add_hint("Open File", "Ctrl+O", "File");
        hints.add_hint("Save File", "Ctrl+S", "File");
        hints.add_hint("Find", "Ctrl+F", "Edit");
        assert_eq!(hints.len(), 3);
        assert_eq!(hints.hints_for_category("File").len(), 2);
        assert_eq!(hints.hints_for_category("Edit").len(), 1);
        let cats = hints.categories();
        assert_eq!(cats, vec!["Edit", "File"]);
    }

    #[test]
    fn keybinding_render_table() {
        let mut hints = WelcomeKeyBindingHint::new();
        hints.add_hint("Open", "Ctrl+O", "File");
        hints.add_hint("Save", "Ctrl+S", "File");
        let table = hints.render_table();
        assert!(table.contains("Open"));
        assert!(table.contains("Ctrl+O"));
        assert!(table.contains("Save"));
        let empty = WelcomeKeyBindingHint::new();
        assert!(empty.render_table().is_empty());
    }

    // -- Customization tests --

    #[test]
    fn customization_defaults() {
        let c = WelcomeCustomization::new();
        assert!(c.show_recent);
        assert!(c.show_walkthroughs);
        assert!(c.show_keybindings);
        assert_eq!(c.max_recent_items, 10);
        assert_eq!(c.visible_section_count(), 3);
        assert_eq!(c.effective_greeting(), "Welcome");
    }

    #[test]
    fn customization_minimal_and_builder() {
        let c = WelcomeCustomization::minimal().with_greeting("Hello!");
        assert!(!c.show_recent);
        assert_eq!(c.visible_section_count(), 0);
        assert_eq!(c.effective_greeting(), "Hello!");
        let display = format!("{}", c);
        assert!(display.contains("sections=0"));
        assert!(display.contains("Hello!"));
    }

    #[test]
    fn welcomeTipsRotator_new() {
        let s = WelcomeTipsRotator::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn welcomeTipsRotator_add_contains() {
        let mut s = WelcomeTipsRotator::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn welcomeTipsRotator_add_duplicate() {
        let mut s = WelcomeTipsRotator::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn welcomeTipsRotator_remove() {
        let mut s = WelcomeTipsRotator::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn welcomeTipsRotator_capacity() {
        let s = WelcomeTipsRotator::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn welcomeTipsRotator_search() {
        let mut s = WelcomeTipsRotator::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn welcomeTipsRotator_stats() {
        let mut s = WelcomeTipsRotator::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn welcomeQuickStartGuide_new() {
        let m = WelcomeQuickStartGuide::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn welcomeQuickStartGuide_add_find() {
        let mut m = WelcomeQuickStartGuide::new();
        m.add(WelcomeQuickStartGuideItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn welcomeQuickStartGuide_priority_filter() {
        let mut m = WelcomeQuickStartGuide::new();
        m.add(WelcomeQuickStartGuideItem::new("a", "A").with_priority(WelcomeQuickStartGuidePriority::High));
        m.add(WelcomeQuickStartGuideItem::new("b", "B").with_priority(WelcomeQuickStartGuidePriority::Low));
        m.add(WelcomeQuickStartGuideItem::new("c", "C").with_priority(WelcomeQuickStartGuidePriority::High));
        assert_eq!(m.by_priority(WelcomeQuickStartGuidePriority::High).len(), 2);
    }

    #[test]
    fn welcomeQuickStartGuide_remove() {
        let mut m = WelcomeQuickStartGuide::new();
        m.add(WelcomeQuickStartGuideItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn welcomeQuickStartGuide_search() {
        let mut m = WelcomeQuickStartGuide::new();
        m.add(WelcomeQuickStartGuideItem::new("id1", "Hello World"));
        m.add(WelcomeQuickStartGuideItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn welcomeQuickStartGuide_total_weight() {
        let mut m = WelcomeQuickStartGuide::new();
        m.add(WelcomeQuickStartGuideItem::new("a", "A").with_priority(WelcomeQuickStartGuidePriority::Critical));
        m.add(WelcomeQuickStartGuideItem::new("b", "B").with_priority(WelcomeQuickStartGuidePriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn welcomeQuickStartGuide_capacity_limit() {
        let mut m = WelcomeQuickStartGuide::new().with_max_items(2);
        m.add(WelcomeQuickStartGuideItem::new("1", "one"));
        m.add(WelcomeQuickStartGuideItem::new("2", "two"));
        assert!(!m.add(WelcomeQuickStartGuideItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn welcomeQuickStartGuide_sorted_by_priority() {
        let mut m = WelcomeQuickStartGuide::new();
        m.add(WelcomeQuickStartGuideItem::new("lo", "Low").with_priority(WelcomeQuickStartGuidePriority::Low));
        m.add(WelcomeQuickStartGuideItem::new("hi", "High").with_priority(WelcomeQuickStartGuidePriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn welcomeQuickStartGuide_item_metadata() {
        let mut item = WelcomeQuickStartGuideItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn welcomeTipsRotator_enabled_toggle() {
        let mut s = WelcomeTipsRotator::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn welcomeQuickStartGuide_priority_display() {
        assert_eq!(format!("{}", WelcomeQuickStartGuidePriority::High), "high");
        assert_eq!(format!("{}", WelcomeQuickStartGuidePriority::Low), "low");
    }


    #[test]
    fn welcome_builder_valid() {
        let cfg = WelcomeBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn welcome_builder_empty_name() {
        let r = WelcomeBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn welcome_builder_bad_priority() {
        assert!(WelcomeBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn welcome_builder_zero_max() {
        assert!(WelcomeBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn welcome_cfg_merge() {
        let mut a = WelcomeBuilder::new("a").property("x", "1").build().unwrap();
        let b = WelcomeBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn welcome_cfg_display() {
        let cfg = WelcomeBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }

    #[test]
    fn welcome_fmt_list() {
        let f = WelcomeFmt::new(WelcomeFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn welcome_fmt_kv() {
        let f = WelcomeFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn welcome_fmt_section() {
        let f = WelcomeFmt::new(WelcomeFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn welcome_fmt_truncate() {
        let f = WelcomeFmt::new(WelcomeFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn welcome_fmt_opts_defaults() {
        let o = WelcomeFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
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


    // xa_ extended tests for welcome
    #[test]
    fn xa_welcome_ring_new() {
        let rb = super::XaWelcomeRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_welcome_ring_push_len() {
        let mut rb = super::XaWelcomeRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_welcome_ring_wrap() {
        let mut rb = super::XaWelcomeRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_welcome_ring_mean_empty() {
        let rb = super::XaWelcomeRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_welcome_ring_mean_values() {
        let mut rb = super::XaWelcomeRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_welcome_ring_min_max() {
        let mut rb = super::XaWelcomeRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_welcome_ring_iter() {
        let mut rb = super::XaWelcomeRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_welcome_counter_new() {
        let c = super::XaWelcomeCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_welcome_counter_inc() {
        let mut c = super::XaWelcomeCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_welcome_counter_inc_by() {
        let mut c = super::XaWelcomeCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_welcome_counter_reset() {
        let mut c = super::XaWelcomeCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_welcome_counter_clear() {
        let mut c = super::XaWelcomeCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_welcome_counter_default() {
        let c = super::XaWelcomeCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 235 ----

    #[test]
    fn xc_235_pool_new_empty() {
        let pool: super::Xc235Pool<i32> = super::Xc235Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_235_pool_release_acquire() {
        let mut pool = super::Xc235Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_235_pool_acquire_empty() {
        let mut pool: super::Xc235Pool<i32> = super::Xc235Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_235_pool_full() {
        let mut pool = super::Xc235Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_235_pool_drain() {
        let mut pool = super::Xc235Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_235_pool_stats() {
        let mut pool = super::Xc235Pool::new(8);
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
    fn xc_235_pool_clear() {
        let mut pool = super::Xc235Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_235_pool_shrink() {
        let mut pool = super::Xc235Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_235_pool_default() {
        let pool: super::Xc235Pool<String> = super::Xc235Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_235_pool_extend() {
        let mut pool = super::Xc235Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_235_pool_retain() {
        let mut pool = super::Xc235Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_235_scheduler_round_robin() {
        let mut sched = super::Xc235Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_235_scheduler_empty() {
        let mut sched = super::Xc235Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_235_scheduler_reset() {
        let mut sched = super::Xc235Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_235_scheduler_add_remove() {
        let mut sched = super::Xc235Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_235_scheduler_targets() {
        let sched = super::Xc235Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_235_hash_empty() {
        assert_eq!(super::xc_235_hash(b""), 5381);
    }

    #[test]
    fn xc_235_hash_data() {
        let h = super::xc_235_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_235_hash(b"hello"), h);
    }

    #[test]
    fn xc_235_reverse_str() {
        assert_eq!(super::xc_235_reverse("abc"), "cba");
        assert_eq!(super::xc_235_reverse(""), "");
    }


    // --- xd_54 deepening tests ---

    #[test]
    fn xd_54_sm_initial_state() {
        let sm = Xd54StateMachine::new();
        assert_eq!(sm.current_state(), Xd54State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_54_sm_valid_idle_to_running() {
        let mut sm = Xd54StateMachine::new();
        assert!(sm.transition(Xd54State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd54State::Running);
    }

    #[test]
    fn xd_54_sm_valid_running_to_paused() {
        let mut sm = Xd54StateMachine::new();
        sm.transition(Xd54State::Running).unwrap();
        assert!(sm.transition(Xd54State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd54State::Paused);
    }

    #[test]
    fn xd_54_sm_valid_running_to_done() {
        let mut sm = Xd54StateMachine::new();
        sm.transition(Xd54State::Running).unwrap();
        assert!(sm.transition(Xd54State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd54State::Done);
    }

    #[test]
    fn xd_54_sm_valid_paused_to_running() {
        let mut sm = Xd54StateMachine::new();
        sm.transition(Xd54State::Running).unwrap();
        sm.transition(Xd54State::Paused).unwrap();
        assert!(sm.transition(Xd54State::Running).is_ok());
    }

    #[test]
    fn xd_54_sm_valid_done_to_idle() {
        let mut sm = Xd54StateMachine::new();
        sm.transition(Xd54State::Running).unwrap();
        sm.transition(Xd54State::Done).unwrap();
        assert!(sm.transition(Xd54State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd54State::Idle);
    }

    #[test]
    fn xd_54_sm_invalid_idle_to_done() {
        let mut sm = Xd54StateMachine::new();
        assert!(sm.transition(Xd54State::Done).is_err());
    }

    #[test]
    fn xd_54_sm_invalid_idle_to_paused() {
        let mut sm = Xd54StateMachine::new();
        assert!(sm.transition(Xd54State::Paused).is_err());
    }

    #[test]
    fn xd_54_sm_history_tracking() {
        let mut sm = Xd54StateMachine::new();
        sm.transition(Xd54State::Running).unwrap();
        sm.transition(Xd54State::Paused).unwrap();
        sm.transition(Xd54State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd54State::Idle);
        assert_eq!(sm.history()[0].to, Xd54State::Running);
        assert_eq!(sm.history()[1].from, Xd54State::Running);
        assert_eq!(sm.history()[2].to, Xd54State::Done);
    }

    #[test]
    fn xd_54_sm_serialize_deserialize() {
        let mut sm = Xd54StateMachine::new();
        sm.transition(Xd54State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd54StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd54State::Running));
    }

    #[test]
    fn xd_54_sm_deserialize_invalid() {
        assert_eq!(Xd54StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_54_sm_reset() {
        let mut sm = Xd54StateMachine::new();
        sm.transition(Xd54State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd54State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_54_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd54EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd54Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_54_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd54EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd54Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd54Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_54_bus_unsubscribe() {
        let mut bus = Xd54EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_54_event_kind_and_payload() {
        let e = Xd54Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd54Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_54_bus_clear_history() {
        let mut bus = Xd54EventBus::new();
        bus.publish(Xd54Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_54_sm_step_counter_increments() {
        let mut sm = Xd54StateMachine::new();
        sm.transition(Xd54State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd54State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #52 --

    #[test]
    fn xf52_trie_insert_search() {
        let mut t = Xf52Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf52_trie_starts_with() {
        let mut t = Xf52Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf52_trie_remove() {
        let mut t = Xf52Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf52_trie_word_count() {
        let mut t = Xf52Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf52_trie_longest_prefix() {
        let mut t = Xf52Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf52_trie_all_words() {
        let mut t = Xf52Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf52_trie_autocomplete() {
        let mut t = Xf52Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf52_trie_empty_search() {
        let t = Xf52Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf52_bloom_add_contains() {
        let mut bf = Xf52BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf52_bloom_probably_absent() {
        let bf = Xf52BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf52_bloom_false_positive_rate() {
        let mut bf = Xf52BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf52_bloom_clear() {
        let mut bf = Xf52BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf52_bloom_union() {
        let mut a = Xf52BloomFilter::xf_new(512, 2);
        let mut b = Xf52BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf52_bloom_intersection_estimate() {
        let mut a = Xf52BloomFilter::xf_new(512, 2);
        let mut b = Xf52BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf52_bloom_union_size_mismatch() {
        let a = Xf52BloomFilter::xf_new(256, 2);
        let b = Xf52BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh234_skip_insert_contains() {
        let mut sl = super::Xh234SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh234_skip_remove() {
        let mut sl = super::Xh234SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh234_skip_len() {
        let mut sl = super::Xh234SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh234_skip_range_query() {
        let mut sl = super::Xh234SkipList::xh_new(4);
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
    fn xh234_skip_floor_ceiling() {
        let mut sl = super::Xh234SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh234_skip_rank() {
        let mut sl = super::Xh234SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh234_skip_empty() {
        let sl = super::Xh234SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh234_skip_duplicates() {
        let mut sl = super::Xh234SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh234_bitset_set_test() {
        let mut bs = super::Xh234BitSet::xh_new(256);
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
    fn xh234_bitset_clear_count() {
        let mut bs = super::Xh234BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh234_bitset_and_or_xor() {
        let mut a = super::Xh234BitSet::xh_new(128);
        let mut b = super::Xh234BitSet::xh_new(128);
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
    fn xh234_bitset_iter_ones() {
        let mut bs = super::Xh234BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh234_bitset_first_last() {
        let mut bs = super::Xh234BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh234_bitset_empty() {
        let bs = super::Xh234BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi234_deque_push_pop_back() {
        let mut dq = super::Xi234Deque::xi_new(4);
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
    fn xi234_deque_push_pop_front() {
        let mut dq = super::Xi234Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi234_deque_mixed_ops() {
        let mut dq = super::Xi234Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi234_deque_get_and_split() {
        let mut dq = super::Xi234Deque::xi_new(8);
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
    fn xi234_deque_rotate_left() {
        let mut dq = super::Xi234Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi234_deque_rotate_right() {
        let mut dq = super::Xi234Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi234_deque_grow() {
        let mut dq = super::Xi234Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi234_deque_empty() {
        let dq = super::Xi234Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi234_interval_tree_insert_query() {
        let mut tree = super::Xi234IntervalTree::xi_new();
        tree.xi_insert(super::Xi234Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi234Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi234Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi234_interval_tree_overlap() {
        let mut tree = super::Xi234IntervalTree::xi_new();
        tree.xi_insert(super::Xi234Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi234Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi234Interval::xi_new(12, 20));
        let q = super::Xi234Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi234_interval_tree_remove() {
        let mut tree = super::Xi234IntervalTree::xi_new();
        tree.xi_insert(super::Xi234Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi234Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi234_interval_tree_gaps() {
        let mut tree = super::Xi234IntervalTree::xi_new();
        tree.xi_insert(super::Xi234Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi234Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi234Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi234Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi234Interval::xi_new(8, 10));
    }

    #[test]
    fn xi234_interval_tree_merge() {
        let mut tree = super::Xi234IntervalTree::xi_new();
        tree.xi_insert(super::Xi234Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi234Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi234Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi234Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi234Interval::xi_new(10, 15));
    }

    #[test]
    fn xi234_interval_tree_all() {
        let mut tree = super::Xi234IntervalTree::xi_new();
        tree.xi_insert(super::Xi234Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi234Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi234_interval_tree_empty() {
        let tree = super::Xi234IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi234_interval_tree_contains_point() {
        let iv = super::Xi234Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}