//! Editor group and tab management.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur during editor group operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorError {
    /// The specified group was not found.
    GroupNotFound(u64),
    /// The specified tab URI was not found in the group.
    TabNotFound { group_id: u64, uri: String },
    /// Cannot close a tab that has unsaved changes without force.
    UnsavedChanges { uri: String },
    /// The tab index is out of bounds.
    IndexOutOfBounds { index: usize, len: usize },
    /// A validation error from the builder.
    ValidationError(String),
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditorError::GroupNotFound(id) => write!(f, "editor group {id} not found"),
            EditorError::TabNotFound { group_id, uri } => {
                write!(f, "tab '{uri}' not found in group {group_id}")
            }
            EditorError::UnsavedChanges { uri } => {
                write!(f, "tab '{uri}' has unsaved changes")
            }
            EditorError::IndexOutOfBounds { index, len } => {
                write!(f, "index {index} out of bounds (len {len})")
            }
            EditorError::ValidationError(msg) => write!(f, "validation error: {msg}"),
        }
    }
}

/// Layout direction for editor groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorGroupLayout {
    Horizontal,
    Vertical,
    Grid,
}

impl fmt::Display for EditorGroupLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditorGroupLayout::Horizontal => write!(f, "Horizontal"),
            EditorGroupLayout::Vertical => write!(f, "Vertical"),
            EditorGroupLayout::Grid => write!(f, "Grid"),
        }
    }
}

/// Metadata for a single editor tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorTabInfo {
    pub uri: String,
    pub label: String,
    pub dirty: bool,
    pub pinned: bool,
    pub preview: bool,
}

/// Builder for constructing an [`EditorTabInfo`] with validation.
#[derive(Debug, Clone)]
pub struct EditorTabBuilder {
    uri: Option<String>,
    label: Option<String>,
    dirty: bool,
    pinned: bool,
    preview: bool,
}

impl EditorTabBuilder {
    pub fn new() -> Self {
        Self {
            uri: None,
            label: None,
            dirty: false,
            pinned: false,
            preview: false,
        }
    }

    pub fn uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    pub fn preview(mut self, preview: bool) -> Self {
        self.preview = preview;
        self
    }

    /// Builds the [`EditorTabInfo`], returning an error if required fields are missing.
    pub fn build(self) -> Result<EditorTabInfo, EditorError> {
        let uri = self.uri.ok_or_else(|| {
            EditorError::ValidationError("uri is required".into())
        })?;
        if uri.is_empty() {
            return Err(EditorError::ValidationError("uri must not be empty".into()));
        }
        let label = self.label.unwrap_or_else(|| {
            uri.rsplit('/').next().unwrap_or(&uri).to_string()
        });
        Ok(EditorTabInfo {
            uri,
            label,
            dirty: self.dirty,
            pinned: self.pinned,
            preview: self.preview,
        })
    }
}

impl Default for EditorTabBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EditorTabInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let markers = [
            if self.dirty { Some("dirty") } else { None },
            if self.pinned { Some("pinned") } else { None },
            if self.preview { Some("preview") } else { None },
        ];
        let flags: Vec<&str> = markers.iter().filter_map(|m| *m).collect();
        if flags.is_empty() {
            write!(f, "{}", self.label)
        } else {
            write!(f, "{} [{}]", self.label, flags.join(", "))
        }
    }
}

/// A group of editor tabs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorGroup {
    pub id: u64,
    pub editors: Vec<EditorTabInfo>,
    pub active_editor: Option<usize>,
}

impl EditorGroup {
    /// Returns the currently active tab, if any.
    pub fn active_tab(&self) -> Option<&EditorTabInfo> {
        self.active_editor.and_then(|i| self.editors.get(i))
    }

    /// Returns `true` if any tab in this group has unsaved changes.
    pub fn has_dirty_tabs(&self) -> bool {
        self.editors.iter().any(|e| e.dirty)
    }

    /// Returns the URIs of all dirty tabs.
    pub fn dirty_uris(&self) -> Vec<&str> {
        self.editors.iter().filter(|e| e.dirty).map(|e| e.uri.as_str()).collect()
    }

    /// Returns the number of tabs in this group.
    pub fn tab_count(&self) -> usize {
        self.editors.len()
    }

    /// Moves a tab from one index to another within the group.
    pub fn move_tab(&mut self, from: usize, to: usize) -> Result<(), EditorError> {
        let len = self.editors.len();
        if from >= len {
            return Err(EditorError::IndexOutOfBounds { index: from, len });
        }
        if to >= len {
            return Err(EditorError::IndexOutOfBounds { index: to, len });
        }
        let tab = self.editors.remove(from);
        self.editors.insert(to, tab);
        // Adjust active editor index to follow the moved tab.
        if let Some(active) = self.active_editor {
            if active == from {
                self.active_editor = Some(to);
            } else if from < active && active <= to {
                self.active_editor = Some(active - 1);
            } else if to <= active && active < from {
                self.active_editor = Some(active + 1);
            }
        }
        Ok(())
    }
}

impl fmt::Display for EditorGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Group {} ({} tabs)", self.id, self.editors.len())
    }
}

/// Service that manages editor groups and their tabs.
#[derive(Debug)]
pub struct EditorGroupService {
    pub groups: Vec<EditorGroup>,
    pub active_group: Option<usize>,
    pub next_id: u64,
}

impl EditorGroupService {
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            active_group: None,
            next_id: 1,
        }
    }

    /// Creates a new empty editor group and returns its id.
    pub fn create_group(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.groups.push(EditorGroup {
            id,
            editors: Vec::new(),
            active_editor: None,
        });
        if self.active_group.is_none() {
            self.active_group = Some(self.groups.len() - 1);
        }
        id
    }

    /// Closes the group with the given id. Returns `true` if found and removed.
    pub fn close_group(&mut self, id: u64) -> bool {
        if let Some(pos) = self.groups.iter().position(|g| g.id == id) {
            self.groups.remove(pos);
            // Adjust active_group index after removal.
            if self.groups.is_empty() {
                self.active_group = None;
            } else if let Some(active) = self.active_group {
                if active == pos {
                    self.active_group = Some(active.min(self.groups.len() - 1));
                } else if active > pos {
                    self.active_group = Some(active - 1);
                }
            }
            true
        } else {
            false
        }
    }

    /// Opens an editor tab in the specified group.
    pub fn open_editor(&mut self, group_id: u64, uri: String, label: String) {
        if let Some(group) = self.groups.iter_mut().find(|g| g.id == group_id) {
            // If tab already open, just activate it.
            if let Some(idx) = group.editors.iter().position(|e| e.uri == uri) {
                group.active_editor = Some(idx);
                return;
            }
            group.editors.push(EditorTabInfo {
                uri,
                label,
                dirty: false,
                pinned: false,
                preview: false,
            });
            group.active_editor = Some(group.editors.len() - 1);
        }
    }

    /// Closes the editor tab matching `uri` in the specified group.
    pub fn close_editor(&mut self, group_id: u64, uri: &str) {
        if let Some(group) = self.groups.iter_mut().find(|g| g.id == group_id) {
            if let Some(pos) = group.editors.iter().position(|e| e.uri == uri) {
                group.editors.remove(pos);
                if group.editors.is_empty() {
                    group.active_editor = None;
                } else if let Some(active) = group.active_editor {
                    if active >= group.editors.len() {
                        group.active_editor = Some(group.editors.len() - 1);
                    }
                }
            }
        }
    }

    /// Sets the active group by id.
    pub fn set_active_group(&mut self, id: u64) {
        if let Some(pos) = self.groups.iter().position(|g| g.id == id) {
            self.active_group = Some(pos);
        }
    }

    /// Returns a reference to the active editor group, if any.
    pub fn get_active_group(&self) -> Option<&EditorGroup> {
        self.active_group.and_then(|i| self.groups.get(i))
    }

    /// Returns the number of editor groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Finds a group by its id.
    pub fn find_group(&self, id: u64) -> Result<&EditorGroup, EditorError> {
        self.groups
            .iter()
            .find(|g| g.id == id)
            .ok_or(EditorError::GroupNotFound(id))
    }

    /// Finds a mutable reference to a group by its id.
    pub fn find_group_mut(&mut self, id: u64) -> Result<&mut EditorGroup, EditorError> {
        self.groups
            .iter_mut()
            .find(|g| g.id == id)
            .ok_or(EditorError::GroupNotFound(id))
    }

    /// Marks a tab as dirty (unsaved changes). Returns an error if group or tab not found.
    pub fn set_dirty(&mut self, group_id: u64, uri: &str, dirty: bool) -> Result<(), EditorError> {
        let group = self.find_group_mut(group_id)?;
        let tab = group
            .editors
            .iter_mut()
            .find(|e| e.uri == uri)
            .ok_or_else(|| EditorError::TabNotFound {
                group_id,
                uri: uri.to_string(),
            })?;
        tab.dirty = dirty;
        Ok(())
    }

    /// Pins or unpins a tab. Returns an error if group or tab not found.
    pub fn set_pinned(&mut self, group_id: u64, uri: &str, pinned: bool) -> Result<(), EditorError> {
        let group = self.find_group_mut(group_id)?;
        let tab = group
            .editors
            .iter_mut()
            .find(|e| e.uri == uri)
            .ok_or_else(|| EditorError::TabNotFound {
                group_id,
                uri: uri.to_string(),
            })?;
        tab.pinned = pinned;
        Ok(())
    }

    /// Closes a tab only if it is not dirty. Returns an error on unsaved changes.
    pub fn safe_close_editor(&mut self, group_id: u64, uri: &str) -> Result<(), EditorError> {
        let group = self.find_group_mut(group_id)?;
        let pos = group
            .editors
            .iter()
            .position(|e| e.uri == uri)
            .ok_or_else(|| EditorError::TabNotFound {
                group_id,
                uri: uri.to_string(),
            })?;
        if group.editors[pos].dirty {
            return Err(EditorError::UnsavedChanges {
                uri: uri.to_string(),
            });
        }
        group.editors.remove(pos);
        if group.editors.is_empty() {
            group.active_editor = None;
        } else if let Some(active) = group.active_editor {
            if active >= group.editors.len() {
                group.active_editor = Some(group.editors.len() - 1);
            }
        }
        Ok(())
    }

    /// Returns a flat list of all URIs across all groups that have unsaved changes.
    pub fn all_dirty_uris(&self) -> Vec<(u64, &str)> {
        self.groups
            .iter()
            .flat_map(|g| {
                g.editors
                    .iter()
                    .filter(|e| e.dirty)
                    .map(move |e| (g.id, e.uri.as_str()))
            })
            .collect()
    }

    /// Returns the total number of open tabs across all groups.
    pub fn total_tab_count(&self) -> usize {
        self.groups.iter().map(|g| g.editors.len()).sum()
    }
}

impl Default for EditorGroupService {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EditorGroupService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EditorGroupService({} groups, {} total tabs)",
            self.group_count(),
            self.total_tab_count(),
        )
    }
}

/// Accumulated statistics for wb-editor operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbEditorStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbEditorStats {
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
    pub fn merge(&mut self, other: &WbEditorStats) {
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

impl Default for WbEditorStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbEditorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbEditorStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-editor.
#[derive(Debug, Clone)]
pub struct WbEditorValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbEditorValidator {
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

impl Default for WbEditorValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EditorTab label formatting with modified indicator
// ---------------------------------------------------------------------------

/// Options for formatting an editor tab label.
#[derive(Debug, Clone)]
pub struct TabLabelOptions {
    /// Character to show when the file is modified/dirty.
    pub dirty_indicator: char,
    /// Character to show when the tab is pinned.
    pub pin_indicator: char,
    /// Maximum label width (0 = unlimited).
    pub max_width: usize,
    /// Whether to show the file extension.
    pub show_extension: bool,
}

impl Default for TabLabelOptions {
    fn default() -> Self {
        Self {
            dirty_indicator: '●',
            pin_indicator: '📌',
            max_width: 0,
            show_extension: true,
        }
    }
}

/// Format an editor tab label for display, including modified and pin indicators.
pub fn format_tab_label(tab: &EditorTabInfo, opts: &TabLabelOptions) -> String {
    let mut name = if opts.show_extension {
        extract_filename(&tab.label)
    } else {
        strip_extension(&extract_filename(&tab.label))
    };

    if opts.max_width > 0 && name.len() > opts.max_width {
        name = truncate_label(&name, opts.max_width);
    }

    let mut result = String::new();
    if tab.pinned {
        result.push(opts.pin_indicator);
        result.push(' ');
    }
    result.push_str(&name);
    if tab.dirty {
        result.push(' ');
        result.push(opts.dirty_indicator);
    }
    result
}

/// Extract the filename portion from a path or URI.
fn extract_filename(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// Strip the file extension from a filename.
fn strip_extension(filename: &str) -> String {
    match filename.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => filename.to_string(),
    }
}

/// Truncate a label to `max_width`, appending "…" if truncated.
fn truncate_label(s: &str, max_width: usize) -> String {
    if max_width == 0 || s.len() <= max_width {
        return s.to_string();
    }
    let mut truncated: String = s.chars().take(max_width.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}

/// Build a tab label suitable for a breadcrumb or title bar display.
/// Format: "filename — folder"
pub fn format_tab_breadcrumb(tab: &EditorTabInfo) -> String {
    let filename = extract_filename(&tab.uri);
    let parent = extract_parent(&tab.uri);
    if parent.is_empty() {
        filename
    } else {
        format!("{filename} — {parent}")
    }
}

fn extract_parent(path: &str) -> String {
    let parts: Vec<&str> = path.rsplitn(2, ['/', '\\']).collect();
    if parts.len() > 1 {
        parts[1].rsplit(['/', '\\']).next().unwrap_or(parts[1]).to_string()
    } else {
        String::new()
    }
}

// ---------------------------------------------------------------------------
// Tab filtering and sorting
// ---------------------------------------------------------------------------

/// Criteria for filtering tabs.
#[derive(Debug, Clone, Default)]
pub struct TabFilter {
    /// If set, only include tabs with this language ID.
    pub language: Option<String>,
    /// If true, only dirty tabs.
    pub dirty_only: bool,
    /// If true, only pinned tabs.
    pub pinned_only: bool,
}

impl TabFilter {
    /// Create a filter for dirty tabs only.
    pub fn dirty() -> Self {
        Self { dirty_only: true, ..Default::default() }
    }

    /// Create a filter for pinned tabs only.
    pub fn pinned() -> Self {
        Self { pinned_only: true, ..Default::default() }
    }

    /// Create a filter for a specific language.
    pub fn for_language(lang: impl Into<String>) -> Self {
        Self { language: Some(lang.into()), ..Default::default() }
    }

    /// Check if a tab matches this filter.
    pub fn matches(&self, tab: &EditorTabInfo) -> bool {
        if self.dirty_only && !tab.dirty {
            return false;
        }
        if self.pinned_only && !tab.pinned {
            return false;
        }
        if let Some(ref lang) = self.language {
            let ext = tab.uri.rsplit('.').next().unwrap_or("");
            if ext != lang {
                return false;
            }
        }
        true
    }
}

/// Sort order for tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabSortOrder {
    /// By URI alphabetically.
    ByUri,
    /// Dirty tabs first.
    DirtyFirst,
    /// Pinned tabs first.
    PinnedFirst,
}

impl fmt::Display for TabSortOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TabSortOrder::ByUri => write!(f, "by URI"),
            TabSortOrder::DirtyFirst => write!(f, "dirty first"),
            TabSortOrder::PinnedFirst => write!(f, "pinned first"),
        }
    }
}

/// Sort a slice of tabs by the given order.
pub fn sort_tabs(tabs: &mut [EditorTabInfo], order: TabSortOrder) {
    match order {
        TabSortOrder::ByUri => tabs.sort_by(|a, b| a.uri.cmp(&b.uri)),
        TabSortOrder::DirtyFirst => tabs.sort_by(|a, b| b.dirty.cmp(&a.dirty)),
        TabSortOrder::PinnedFirst => tabs.sort_by(|a, b| b.pinned.cmp(&a.pinned)),
    }
}

/// Filter tabs by the given criteria.
pub fn filter_tabs(tabs: &[EditorTabInfo], filter: &TabFilter) -> Vec<EditorTabInfo> {
    tabs.iter().filter(|t| filter.matches(t)).cloned().collect()
}

// ---------------------------------------------------------------------------
// Tab summary
// ---------------------------------------------------------------------------

/// Summary statistics about tabs in a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabSummary {
    pub total: usize,
    pub dirty: usize,
    pub pinned: usize,
    pub preview: usize,
}

impl TabSummary {
    /// Compute a summary from a slice of tabs.
    pub fn from_tabs(tabs: &[EditorTabInfo]) -> Self {
        Self {
            total: tabs.len(),
            dirty: tabs.iter().filter(|t| t.dirty).count(),
            pinned: tabs.iter().filter(|t| t.pinned).count(),
            preview: tabs.iter().filter(|t| t.preview).count(),
        }
    }

    /// Returns true if there are any unsaved tabs.
    pub fn has_unsaved(&self) -> bool {
        self.dirty > 0
    }
}

impl fmt::Display for TabSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} tabs ({} dirty, {} pinned, {} preview)",
            self.total, self.dirty, self.pinned, self.preview
        )
    }
}

/// Extract unique file extensions from tabs.
pub fn tab_languages(tabs: &[EditorTabInfo]) -> Vec<String> {
    let mut langs: Vec<String> = tabs.iter()
        .filter_map(|t| t.uri.rsplit('.').next().map(String::from))
        .collect();
    langs.sort();
    langs.dedup();
    langs
}

/// Find duplicate URIs in a tab list.
pub fn find_duplicate_uris(tabs: &[EditorTabInfo]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut dupes = Vec::new();
    for tab in tabs {
        if !seen.insert(&tab.uri) {
            dupes.push(tab.uri.clone());
        }
    }
    dupes
}

// ---------------------------------------------------------------------------
// EditorGroupManager – manages multiple EditorGroups
// ---------------------------------------------------------------------------

/// Manages a collection of editor groups with tab-level operations.
pub struct EditorGroupManager {
    groups: Vec<(u64, EditorGroup)>,
    next_id: u64,
    active: Option<u64>,
}

impl EditorGroupManager {
    pub fn new() -> Self {
        Self { groups: Vec::new(), next_id: 1, active: None }
    }

    /// Create a new empty group and return its id.
    pub fn create_group(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let group = EditorGroup {
            id,
            editors: Vec::new(),
            active_editor: None,
        };
        self.groups.push((id, group));
        if self.active.is_none() {
            self.active = Some(id);
        }
        id
    }

    /// Remove a group by id, returning true if found.
    pub fn remove_group(&mut self, id: u64) -> bool {
        let before = self.groups.len();
        self.groups.retain(|(gid, _)| *gid != id);
        if self.active == Some(id) {
            self.active = self.groups.first().map(|(gid, _)| *gid);
        }
        self.groups.len() < before
    }

    /// Move a tab identified by URI from one group to another.
    pub fn move_tab(&mut self, from_group: u64, to_group: u64, uri: &str) -> Result<(), EditorError> {
        let tab = {
            let src = self.groups.iter_mut().find(|(id, _)| *id == from_group)
                .ok_or(EditorError::GroupNotFound(from_group))?;
            let idx = src.1.editors.iter().position(|t| t.uri == uri)
                .ok_or(EditorError::TabNotFound { group_id: from_group, uri: uri.to_string() })?;
            src.1.editors.remove(idx)
        };
        let dst = self.groups.iter_mut().find(|(id, _)| *id == to_group)
            .ok_or(EditorError::GroupNotFound(to_group))?;
        dst.1.editors.push(tab);
        Ok(())
    }

    /// Return the id of the active group.
    pub fn active_group(&self) -> Option<u64> {
        self.active
    }

    /// Set the active group id.
    pub fn set_active_group(&mut self, id: u64) {
        if self.groups.iter().any(|(gid, _)| *gid == id) {
            self.active = Some(id);
        }
    }

    /// Number of groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Collect all tab URIs across every group.
    pub fn all_tabs(&self) -> Vec<&str> {
        self.groups.iter()
            .flat_map(|(_, g)| g.editors.iter().map(|t| t.uri.as_str()))
            .collect()
    }

    /// Find which group contains a tab with the given URI.
    pub fn find_tab_across_groups(&self, uri: &str) -> Option<u64> {
        self.groups.iter()
            .find(|(_, g)| g.editors.iter().any(|t| t.uri == uri))
            .map(|(id, _)| *id)
    }

    /// Get an immutable reference to a group by id.
    pub fn get_group(&self, id: u64) -> Option<&EditorGroup> {
        self.groups.iter().find(|(gid, _)| *gid == id).map(|(_, g)| g)
    }

    /// Get a mutable reference to a group by id.
    pub fn get_group_mut(&mut self, id: u64) -> Option<&mut EditorGroup> {
        self.groups.iter_mut().find(|(gid, _)| *gid == id).map(|(_, g)| g)
    }
}

// ---------------------------------------------------------------------------
// TabHistoryTracker – navigation history (back/forward)
// ---------------------------------------------------------------------------

/// Tracks tab navigation history with back/forward support.
pub struct TabHistoryTracker {
    entries: Vec<String>,
    cursor: usize,
}

impl TabHistoryTracker {
    pub fn new() -> Self {
        Self { entries: Vec::new(), cursor: 0 }
    }

    /// Push a new URI, truncating any forward history.
    pub fn push(&mut self, uri: impl Into<String>) {
        let uri = uri.into();
        if self.entries.last().map(|s| s.as_str()) == Some(&uri) {
            return;
        }
        if !self.entries.is_empty() && self.cursor < self.entries.len() - 1 {
            self.entries.truncate(self.cursor + 1);
        }
        self.entries.push(uri);
        self.cursor = self.entries.len() - 1;
    }

    /// Move back, returning the URI if possible.
    pub fn back(&mut self) -> Option<&str> {
        if self.cursor > 0 {
            self.cursor -= 1;
            Some(&self.entries[self.cursor])
        } else {
            None
        }
    }

    /// Move forward, returning the URI if possible.
    pub fn forward(&mut self) -> Option<&str> {
        if self.cursor + 1 < self.entries.len() {
            self.cursor += 1;
            Some(&self.entries[self.cursor])
        } else {
            None
        }
    }

    /// Current URI in the history.
    pub fn current(&self) -> Option<&str> {
        self.entries.get(self.cursor).map(|s| s.as_str())
    }

    /// Total number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// EditorGroupStats – statistics with Display
// ---------------------------------------------------------------------------

/// Statistics about an editor group.
pub struct EditorGroupStats {
    pub open_tabs: usize,
    pub dirty_count: usize,
    pub pinned_count: usize,
    pub preview_count: usize,
    pub group_id: u64,
}

impl EditorGroupStats {
    pub fn from_group(id: u64, group: &EditorGroup) -> Self {
        Self {
            group_id: id,
            open_tabs: group.editors.len(),
            dirty_count: group.editors.iter().filter(|t| t.dirty).count(),
            pinned_count: group.editors.iter().filter(|t| t.pinned).count(),
            preview_count: group.editors.iter().filter(|t| t.preview).count(),
        }
    }
}

impl fmt::Display for EditorGroupStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Group {} — {} tab(s), {} dirty, {} pinned, {} preview",
            self.group_id, self.open_tabs, self.dirty_count,
            self.pinned_count, self.preview_count,
        )
    }
}

// ---------------------------------------------------------------------------
// EditorGroupLayout utilities
// ---------------------------------------------------------------------------

impl EditorGroupLayout {
    /// Returns the number of visible splits for this layout given a group count.
    /// Horizontal and Vertical show all groups linearly; Grid arranges in rows.
    pub fn split_count(&self, group_count: usize) -> (usize, usize) {
        match self {
            EditorGroupLayout::Horizontal => (group_count, 1),
            EditorGroupLayout::Vertical => (1, group_count),
            EditorGroupLayout::Grid => {
                if group_count == 0 {
                    return (0, 0);
                }
                let cols = (group_count as f64).sqrt().ceil() as usize;
                let rows = (group_count + cols - 1) / cols;
                (cols, rows)
            }
        }
    }

    /// Returns the next layout in a cycle: Horizontal → Vertical → Grid → Horizontal.
    pub fn cycle_next(self) -> Self {
        match self {
            EditorGroupLayout::Horizontal => EditorGroupLayout::Vertical,
            EditorGroupLayout::Vertical => EditorGroupLayout::Grid,
            EditorGroupLayout::Grid => EditorGroupLayout::Horizontal,
        }
    }
}

// ---------------------------------------------------------------------------
// EditorTabInfo utilities
// ---------------------------------------------------------------------------

impl EditorTabInfo {
    /// Returns the file extension from the URI, if any.
    pub fn extension(&self) -> Option<&str> {
        let filename = self.uri.rsplit(['/', '\\']).next().unwrap_or(&self.uri);
        match filename.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => Some(ext),
            _ => None,
        }
    }

    /// Returns the filename portion of the URI (last path component).
    pub fn filename(&self) -> &str {
        self.uri.rsplit(['/', '\\']).next().unwrap_or(&self.uri)
    }

    /// Returns true if this tab is in a "clean" state (not dirty, not preview).
    pub fn is_stable(&self) -> bool {
        !self.dirty && !self.preview
    }
}

// ---------------------------------------------------------------------------
// EditorGroup – close_all and retain operations
// ---------------------------------------------------------------------------

impl EditorGroup {
    /// Closes all tabs in this group, returning the removed tabs.
    pub fn close_all(&mut self) -> Vec<EditorTabInfo> {
        self.active_editor = None;
        std::mem::take(&mut self.editors)
    }

    /// Closes all tabs except those that match the predicate.
    /// Returns the removed tabs. Active editor is reset if its tab was removed.
    pub fn close_others<F>(&mut self, keep: F) -> Vec<EditorTabInfo>
    where
        F: Fn(&EditorTabInfo) -> bool,
    {
        let mut removed = Vec::new();
        let mut kept = Vec::new();
        for tab in self.editors.drain(..) {
            if keep(&tab) {
                kept.push(tab);
            } else {
                removed.push(tab);
            }
        }
        self.editors = kept;
        if self.editors.is_empty() {
            self.active_editor = None;
        } else if let Some(active) = self.active_editor {
            if active >= self.editors.len() {
                self.active_editor = Some(self.editors.len() - 1);
            }
        }
        removed
    }

    /// Returns the index of the tab with the given URI, if present.
    pub fn find_tab_index(&self, uri: &str) -> Option<usize> {
        self.editors.iter().position(|e| e.uri == uri)
    }

    /// Activates the next tab (wrapping around). Returns the new active index.
    pub fn activate_next(&mut self) -> Option<usize> {
        if self.editors.is_empty() {
            return None;
        }
        let next = match self.active_editor {
            Some(i) => (i + 1) % self.editors.len(),
            None => 0,
        };
        self.active_editor = Some(next);
        Some(next)
    }

    /// Activates the previous tab (wrapping around). Returns the new active index.
    pub fn activate_prev(&mut self) -> Option<usize> {
        if self.editors.is_empty() {
            return None;
        }
        let prev = match self.active_editor {
            Some(0) => self.editors.len() - 1,
            Some(i) => i - 1,
            None => self.editors.len() - 1,
        };
        self.active_editor = Some(prev);
        Some(prev)
    }

    /// Returns the URIs of all pinned tabs.
    pub fn pinned_uris(&self) -> Vec<&str> {
        self.editors.iter().filter(|e| e.pinned).map(|e| e.uri.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// EditorGroupService – batch operations
// ---------------------------------------------------------------------------

impl EditorGroupService {
    /// Closes all tabs across all groups that are not dirty.
    /// Returns the total number of tabs closed.
    pub fn close_all_saved(&mut self) -> usize {
        let mut closed = 0;
        for group in &mut self.groups {
            let before = group.editors.len();
            group.editors.retain(|t| t.dirty);
            closed += before - group.editors.len();
            if group.editors.is_empty() {
                group.active_editor = None;
            } else if let Some(active) = group.active_editor {
                if active >= group.editors.len() {
                    group.active_editor = Some(group.editors.len() - 1);
                }
            }
        }
        closed
    }

    /// Returns a summary of all groups as a vector of `TabSummary` paired with group id.
    pub fn group_summaries(&self) -> Vec<(u64, TabSummary)> {
        self.groups
            .iter()
            .map(|g| (g.id, TabSummary::from_tabs(&g.editors)))
            .collect()
    }

    /// Finds all groups containing a tab with the given URI.
    pub fn find_groups_with_uri(&self, uri: &str) -> Vec<u64> {
        self.groups
            .iter()
            .filter(|g| g.editors.iter().any(|e| e.uri == uri))
            .map(|g| g.id)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// EditorGridResize – tracks per-group width/height percentages for grid layout
// ---------------------------------------------------------------------------

/// Stores the percentage-based size of each editor group in a grid layout.
#[derive(Debug, Clone)]
pub struct EditorGridResize {
    group_sizes: HashMap<u64, (u32, u32)>,
}

impl EditorGridResize {
    /// Creates an empty resize map.
    pub fn new() -> Self {
        Self {
            group_sizes: HashMap::new(),
        }
    }

    /// Sets the `(width_pct, height_pct)` for the given group.
    pub fn set_size(&mut self, group_id: u64, width_pct: u32, height_pct: u32) {
        self.group_sizes.insert(group_id, (width_pct, height_pct));
    }

    /// Returns the stored size for the given group, if any.
    pub fn get_size(&self, group_id: u64) -> Option<(u32, u32)> {
        self.group_sizes.get(&group_id).copied()
    }

    /// Removes a group from the map, returning whether it was present.
    pub fn remove(&mut self, group_id: u64) -> bool {
        self.group_sizes.remove(&group_id).is_some()
    }

    /// Normalizes all width percentages so they sum to exactly 100.
    /// Heights are left untouched. Does nothing when the map is empty.
    pub fn normalize(&mut self) {
        let total_w: u32 = self.group_sizes.values().map(|(w, _)| *w).sum();
        if total_w == 0 || self.group_sizes.is_empty() {
            return;
        }
        for (_id, (w, _h)) in self.group_sizes.iter_mut() {
            *w = (*w * 100) / total_w;
        }
        // Distribute any rounding remainder to the first entry.
        let new_total: u32 = self.group_sizes.values().map(|(w, _)| *w).sum();
        if new_total < 100 {
            if let Some((w, _)) = self.group_sizes.values_mut().next() {
                *w += 100 - new_total;
            }
        }
    }

    /// Returns `true` when every percentage is in `1..=100`.
    pub fn is_valid(&self) -> bool {
        self.group_sizes
            .values()
            .all(|(w, h)| (1..=100).contains(w) && (1..=100).contains(h))
    }

    /// Number of groups being tracked.
    pub fn group_count(&self) -> usize {
        self.group_sizes.len()
    }

    /// Clears all stored sizes.
    pub fn reset(&mut self) {
        self.group_sizes.clear();
    }
}

// ---------------------------------------------------------------------------
// EditorTabDecorations – visual decorations (icons / badges) for tabs
// ---------------------------------------------------------------------------

/// A single tab decoration with optional icon, badge text, and badge colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabDecoration {
    pub icon: Option<String>,
    pub badge: Option<String>,
    pub badge_color: Option<String>,
}

/// Manages decorations keyed by tab URI.
#[derive(Debug, Clone)]
pub struct EditorTabDecorations {
    decorations: HashMap<String, TabDecoration>,
}

impl EditorTabDecorations {
    pub fn new() -> Self {
        Self {
            decorations: HashMap::new(),
        }
    }

    pub fn set_decoration(&mut self, uri: &str, decoration: TabDecoration) {
        self.decorations.insert(uri.to_string(), decoration);
    }

    pub fn get_decoration(&self, uri: &str) -> Option<&TabDecoration> {
        self.decorations.get(uri)
    }

    pub fn remove_decoration(&mut self, uri: &str) -> bool {
        self.decorations.remove(uri).is_some()
    }

    pub fn clear(&mut self) {
        self.decorations.clear();
    }

    pub fn has_decoration(&self, uri: &str) -> bool {
        self.decorations.contains_key(uri)
    }

    pub fn count(&self) -> usize {
        self.decorations.len()
    }

    /// Returns sorted list of URIs that have a non-`None` badge.
    pub fn uris_with_badges(&self) -> Vec<String> {
        let mut uris: Vec<String> = self
            .decorations
            .iter()
            .filter(|(_, d)| d.badge.is_some())
            .map(|(uri, _)| uri.clone())
            .collect();
        uris.sort();
        uris
    }
}

// ---------------------------------------------------------------------------
// EditorUntitledSequencer – generates sequential "Untitled-N" names
// ---------------------------------------------------------------------------

/// Produces sequentially numbered untitled document names.
#[derive(Debug, Clone)]
pub struct EditorUntitledSequencer {
    counter: u32,
    prefix: String,
}

impl EditorUntitledSequencer {
    pub fn new(prefix: &str) -> Self {
        Self {
            counter: 0,
            prefix: prefix.to_string(),
        }
    }

    /// Returns the next name, e.g. `"Untitled-1"`, `"Untitled-2"`, …
    pub fn next(&mut self) -> String {
        self.counter += 1;
        format!("{}-{}", self.prefix, self.counter)
    }

    pub fn current_count(&self) -> u32 {
        self.counter
    }

    pub fn reset(&mut self) {
        self.counter = 0;
    }
}

impl fmt::Display for EditorUntitledSequencer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(count={})", self.prefix, self.counter)
    }
}

// ---------------------------------------------------------------------------
// EditorCloseConfirmation – tracks dirty files that need save-confirmation
// ---------------------------------------------------------------------------

/// Collects dirty file URIs and records whether the user has confirmed close.
#[derive(Debug, Clone)]
pub struct EditorCloseConfirmation {
    dirty_uris: Vec<String>,
    confirmed: bool,
}

impl EditorCloseConfirmation {
    pub fn new() -> Self {
        Self {
            dirty_uris: Vec::new(),
            confirmed: false,
        }
    }

    pub fn add_dirty(&mut self, uri: &str) {
        if !self.dirty_uris.contains(&uri.to_string()) {
            self.dirty_uris.push(uri.to_string());
        }
    }

    /// Returns `true` when there is at least one dirty URI requiring confirmation.
    pub fn needs_confirmation(&self) -> bool {
        !self.dirty_uris.is_empty()
    }

    pub fn confirm(&mut self) {
        self.confirmed = true;
    }

    pub fn cancel(&mut self) {
        self.confirmed = false;
    }

    pub fn dirty_count(&self) -> usize {
        self.dirty_uris.len()
    }

    pub fn dirty_list(&self) -> &[String] {
        &self.dirty_uris
    }

    /// Human-readable confirmation message.
    pub fn message(&self) -> String {
        let n = self.dirty_uris.len();
        if n == 0 {
            "No unsaved changes.".to_string()
        } else {
            format!("Save changes to {} file{}?", n, if n == 1 { "" } else { "s" })
        }
    }

    pub fn is_confirmed(&self) -> bool {
        self.confirmed
    }
}

impl fmt::Display for EditorCloseConfirmation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CloseConfirmation({} dirty, confirmed={})",
            self.dirty_uris.len(),
            self.confirmed
        )
    }
}


// === Editor Group Serializer ===

/// Editor Group Serializer implementation.
#[derive(Debug, Clone)]
pub struct EditorGroupSerializer {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: EditorGroupSerializerStats,
}

/// Statistics for EditorGroupSerializer.
#[derive(Debug, Clone, Default)]
pub struct EditorGroupSerializerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl EditorGroupSerializerStats {
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

impl EditorGroupSerializer {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: EditorGroupSerializerStats::default(),
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

    pub fn stats(&self) -> &EditorGroupSerializerStats {
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

impl Default for EditorGroupSerializer {
    fn default() -> Self {
        Self::new()
    }
}

// === Editor Tab Decoration Merger ===

/// Priority level for EditorTabDecorationMerger items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EditorTabDecorationMergerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl EditorTabDecorationMergerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for EditorTabDecorationMergerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Editor Tab Decoration Merger implementation.
#[derive(Debug, Clone)]
pub struct EditorTabDecorationMerger {
    items: Vec<EditorTabDecorationMergerItem>,
    max_items: usize,
    default_priority: EditorTabDecorationMergerPriority,
}

/// A single item in EditorTabDecorationMerger.
#[derive(Debug, Clone)]
pub struct EditorTabDecorationMergerItem {
    pub id: String,
    pub label: String,
    pub priority: EditorTabDecorationMergerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl EditorTabDecorationMergerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: EditorTabDecorationMergerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: EditorTabDecorationMergerPriority) -> Self {
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

impl EditorTabDecorationMerger {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: EditorTabDecorationMergerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: EditorTabDecorationMergerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<EditorTabDecorationMergerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&EditorTabDecorationMergerItem> {
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

    pub fn by_priority(&self, priority: EditorTabDecorationMergerPriority) -> Vec<&EditorTabDecorationMergerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&EditorTabDecorationMergerItem> {
        let mut sorted: Vec<&EditorTabDecorationMergerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&EditorTabDecorationMergerItem> {
        let mut sorted: Vec<&EditorTabDecorationMergerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&EditorTabDecorationMergerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: EditorTabDecorationMergerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> EditorTabDecorationMergerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &EditorTabDecorationMergerItem> {
        self.items.iter()
    }
}

impl Default for EditorTabDecorationMerger {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// vsedit-wb-editor: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WbEditorXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl WbEditorXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for WbEditorXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct WbEditorXRegistry {
    entries: Vec<WbEditorXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl WbEditorXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: WbEditorXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&WbEditorXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut WbEditorXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<WbEditorXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&WbEditorXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&WbEditorXConfig> {
        let mut sorted: Vec<&WbEditorXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&WbEditorXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> WbEditorXIterator<'_> {
        WbEditorXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct WbEditorXIterator<'a> {
    inner: std::slice::Iter<'a, WbEditorXConfig>,
}

impl<'a> Iterator for WbEditorXIterator<'a> {
    type Item = &'a WbEditorXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct WbEditorXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl WbEditorXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct WbEditorXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl WbEditorXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &WbEditorXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &WbEditorXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &WbEditorXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for WbEditorXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct WbEditorXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl WbEditorXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &WbEditorXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &WbEditorXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for WbEditorXValidator {
    fn default() -> Self {
        Self::new()
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
// xb_ utilities – batch 101
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer101 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer101 {
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
pub fn xb_fnv1a_101(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_101<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_101<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_101(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_101(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 207
// ---------------------------------------------------------------------------

/// Generic object pool `Xc207Pool<T>`.
pub struct Xc207Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc207Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc207PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc207Pool<T> {
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
    pub fn stats(&self) -> Xc207PoolStats {
        Xc207PoolStats {
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

impl<T> Default for Xc207Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc207Scheduler`.
pub struct Xc207Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc207Scheduler {
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

impl Default for Xc207Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_207 hash for the given byte slice.
pub fn xc_207_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_207 convention.
pub fn xc_207_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe114 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe114Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe114PipelineError {
    pub stage: Xe114Stage,
    pub message: String,
}

impl std::fmt::Display for Xe114PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe114Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe114Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe114PipelineError>>>,
    stage_names: Vec<Xe114Stage>,
}

impl Xe114Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe114PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe114Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe114PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe114Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe114PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe114Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe114PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe114Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe114PipelineError> {
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

    pub fn compose(mut self, other: Xe114Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe114CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe114CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe114Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe114CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe114CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe114Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe114CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_114_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe114CacheEntry {
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

    fn xe_114_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe114CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_114_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe114PipelineError> {
    Ok(data)
}

pub fn xe_114_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe114PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_114_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe114PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_114_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe114PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_114_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe114PipelineError> {
    Err(Xe114PipelineError {
        stage: Xe114Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_112: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg112Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg112Graph {
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

impl Default for Xg112Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_112: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg112Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg112Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg112Heap<T>) {
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

impl<T: Ord> Default for Xg112Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 206).
pub struct Xh206SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh206SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 248 as u64,
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

/// A compact bit set supporting boolean operations (variant 206).
pub struct Xh206BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh206BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 206).
pub struct Xi206Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi206Deque<T> {
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
pub struct Xi206Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi206Interval {
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

/// A simple interval tree (variant 206).
pub struct Xi206IntervalTree {
    xi_intervals: Vec<Xi206Interval>,
}

impl Xi206IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi206Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi206Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi206Interval) -> Vec<&Xi206Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi206Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi206Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi206Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi206Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi206Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi206Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 206) ---

/// Disjoint set / union-find for crate 206.
pub struct Xj206UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj206UnionFind {
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

const XJ206_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 206.
pub struct Xj206BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj206BTreeNode<K, V>>>,
    len: usize,
}

struct Xj206BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj206BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj206BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ206_BTREE_ORDER - 1
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
        let mid = XJ206_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj206BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj206BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj206BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj206BTreeNode::xj_new_leaf();
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


// --- xk_206 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk206SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk206SegmentTree {
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
pub struct Xk206DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk206DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_206).
#[derive(Debug, Clone)]
pub struct Xl206Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl206Rope {
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

/// Suffix array for efficient string searching (xl_206).
#[derive(Debug, Clone)]
pub struct Xl206SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl206SuffixArray {
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
pub struct Xm206MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm206MatrixSparse {
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
pub struct Xm206Tokenizer {
    text: String,
}

impl Xm206Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 206.
pub struct Xn206Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn206Fenwick {
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

// ----- AVL tree map — crate 206 -----

#[derive(Debug, Clone)]
struct Xn206AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn206AvlNode<K, V>>>,
    right: Option<Box<Xn206AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 206.
#[derive(Debug, Clone)]
pub struct Xn206AVL<K, V> {
    root: Option<Box<Xn206AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn206AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn206AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn206AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn206AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn206AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn206AvlNode<K, V>>) -> Box<Xn206AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn206AvlNode<K, V>>) -> Box<Xn206AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn206AvlNode<K, V>>) -> Box<Xn206AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn206AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn206AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn206AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn206AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn206AvlNode<K, V>>) -> &Xn206AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn206AvlNode<K, V>>) -> (Box<Xn206AvlNode<K, V>>, Option<Box<Xn206AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn206AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn206AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn206AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn206AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn206AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn206AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn206AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo206RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo206Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo206RBNode<K, V> {
    key: K,
    value: V,
    color: Xo206Color,
    left: Option<Box<Xo206RBNode<K, V>>>,
    right: Option<Box<Xo206RBNode<K, V>>>,
}

/// A red-black tree map for crate 206.
#[derive(Debug, Clone)]
pub struct Xo206RedBlack<K, V> {
    root: Option<Box<Xo206RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo206RedBlack<K, V> {
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
            r.color = Xo206Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo206RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo206RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo206RBNode {
                    key, value, color: Xo206Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo206RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo206Color::Red)
    }

    fn xo_balance(mut h: Box<Xo206RBNode<K, V>>) -> Box<Xo206RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo206Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo206RBNode<K, V>>) -> Box<Xo206RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo206Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo206RBNode<K, V>>) -> Box<Xo206RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo206Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo206RBNode<K, V>>) {
        h.color = Xo206Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo206Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo206Color::Black; }
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
            r.color = Xo206Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo206RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo206RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo206RBNode<K, V>) -> (K, V, Option<Box<Xo206RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo206RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo206Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo206RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo206ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 206.
#[derive(Debug, Clone)]
pub struct Xo206ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo206ConsistentHash {
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
            let vkey = format!("{}#xo206#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo206#{}", node, i);
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
    fn create_and_close_groups() {
        let mut svc = EditorGroupService::new();
        assert_eq!(svc.group_count(), 0);

        let id1 = svc.create_group();
        let id2 = svc.create_group();
        assert_eq!(svc.group_count(), 2);

        assert!(svc.close_group(id1));
        assert_eq!(svc.group_count(), 1);
        assert_eq!(svc.get_active_group().unwrap().id, id2);

        assert!(!svc.close_group(999));
    }

    #[test]
    fn open_and_close_editors() {
        let mut svc = EditorGroupService::new();
        let gid = svc.create_group();

        svc.open_editor(gid, "file:///a.rs".into(), "a.rs".into());
        svc.open_editor(gid, "file:///b.rs".into(), "b.rs".into());

        let group = svc.get_active_group().unwrap();
        assert_eq!(group.editors.len(), 2);
        assert_eq!(group.active_editor, Some(1));

        // Re-opening existing tab just activates it.
        svc.open_editor(gid, "file:///a.rs".into(), "a.rs".into());
        let group = svc.get_active_group().unwrap();
        assert_eq!(group.editors.len(), 2);
        assert_eq!(group.active_editor, Some(0));

        svc.close_editor(gid, "file:///a.rs");
        let group = svc.get_active_group().unwrap();
        assert_eq!(group.editors.len(), 1);
        assert_eq!(group.editors[0].uri, "file:///b.rs");
    }

    #[test]
    fn set_active_group() {
        let mut svc = EditorGroupService::new();
        let id1 = svc.create_group();
        let id2 = svc.create_group();

        assert_eq!(svc.get_active_group().unwrap().id, id1);
        svc.set_active_group(id2);
        assert_eq!(svc.get_active_group().unwrap().id, id2);
    }

    #[test]
    fn layout_enum_clone() {
        let layout = EditorGroupLayout::Grid;
        let cloned = layout.clone();
        assert_eq!(layout, cloned);
    }

    // --- New tests ---

    #[test]
    fn editor_tab_builder_success() {
        let tab = EditorTabBuilder::new()
            .uri("file:///main.rs")
            .label("main.rs")
            .dirty(true)
            .pinned(true)
            .build()
            .unwrap();
        assert_eq!(tab.uri, "file:///main.rs");
        assert_eq!(tab.label, "main.rs");
        assert!(tab.dirty);
        assert!(tab.pinned);
        assert!(!tab.preview);
    }

    #[test]
    fn editor_tab_builder_auto_label() {
        let tab = EditorTabBuilder::new()
            .uri("file:///src/utils.rs")
            .build()
            .unwrap();
        assert_eq!(tab.label, "utils.rs");
    }

    #[test]
    fn editor_tab_builder_missing_uri() {
        let result = EditorTabBuilder::new().build();
        assert_eq!(
            result,
            Err(EditorError::ValidationError("uri is required".into()))
        );
    }

    #[test]
    fn editor_tab_builder_empty_uri() {
        let result = EditorTabBuilder::new().uri("").build();
        assert_eq!(
            result,
            Err(EditorError::ValidationError("uri must not be empty".into()))
        );
    }

    #[test]
    fn set_dirty_and_safe_close() {
        let mut svc = EditorGroupService::new();
        let gid = svc.create_group();
        svc.open_editor(gid, "file:///x.rs".into(), "x.rs".into());

        svc.set_dirty(gid, "file:///x.rs", true).unwrap();
        assert!(svc.find_group(gid).unwrap().has_dirty_tabs());

        // safe_close should fail because tab is dirty.
        let err = svc.safe_close_editor(gid, "file:///x.rs").unwrap_err();
        assert_eq!(
            err,
            EditorError::UnsavedChanges {
                uri: "file:///x.rs".into()
            }
        );

        // Mark clean, then safe close should succeed.
        svc.set_dirty(gid, "file:///x.rs", false).unwrap();
        svc.safe_close_editor(gid, "file:///x.rs").unwrap();
        assert_eq!(svc.find_group(gid).unwrap().tab_count(), 0);
    }

    #[test]
    fn find_group_errors() {
        let svc = EditorGroupService::new();
        assert_eq!(svc.find_group(42), Err(EditorError::GroupNotFound(42)));
    }

    #[test]
    fn set_dirty_on_missing_tab() {
        let mut svc = EditorGroupService::new();
        let gid = svc.create_group();
        let err = svc.set_dirty(gid, "file:///nope.rs", true).unwrap_err();
        assert_eq!(
            err,
            EditorError::TabNotFound {
                group_id: gid,
                uri: "file:///nope.rs".into()
            }
        );
    }

    #[test]
    fn pin_and_unpin_tab() {
        let mut svc = EditorGroupService::new();
        let gid = svc.create_group();
        svc.open_editor(gid, "file:///p.rs".into(), "p.rs".into());

        svc.set_pinned(gid, "file:///p.rs", true).unwrap();
        assert!(svc.find_group(gid).unwrap().editors[0].pinned);

        svc.set_pinned(gid, "file:///p.rs", false).unwrap();
        assert!(!svc.find_group(gid).unwrap().editors[0].pinned);
    }

    #[test]
    fn move_tab_within_group() {
        let mut svc = EditorGroupService::new();
        let gid = svc.create_group();
        svc.open_editor(gid, "file:///a.rs".into(), "a.rs".into());
        svc.open_editor(gid, "file:///b.rs".into(), "b.rs".into());
        svc.open_editor(gid, "file:///c.rs".into(), "c.rs".into());

        // Active is c.rs (index 2). Move a.rs from 0 to 2.
        let group = svc.find_group_mut(gid).unwrap();
        group.move_tab(0, 2).unwrap();
        assert_eq!(group.editors[0].uri, "file:///b.rs");
        assert_eq!(group.editors[1].uri, "file:///c.rs");
        assert_eq!(group.editors[2].uri, "file:///a.rs");
    }

    #[test]
    fn move_tab_out_of_bounds() {
        let mut svc = EditorGroupService::new();
        let gid = svc.create_group();
        svc.open_editor(gid, "file:///a.rs".into(), "a.rs".into());

        let group = svc.find_group_mut(gid).unwrap();
        let err = group.move_tab(0, 5).unwrap_err();
        assert_eq!(err, EditorError::IndexOutOfBounds { index: 5, len: 1 });
    }

    #[test]
    fn total_tab_count_across_groups() {
        let mut svc = EditorGroupService::new();
        let g1 = svc.create_group();
        let g2 = svc.create_group();
        svc.open_editor(g1, "file:///a.rs".into(), "a.rs".into());
        svc.open_editor(g1, "file:///b.rs".into(), "b.rs".into());
        svc.open_editor(g2, "file:///c.rs".into(), "c.rs".into());
        assert_eq!(svc.total_tab_count(), 3);
    }

    #[test]
    fn all_dirty_uris_across_groups() {
        let mut svc = EditorGroupService::new();
        let g1 = svc.create_group();
        let g2 = svc.create_group();
        svc.open_editor(g1, "file:///a.rs".into(), "a.rs".into());
        svc.open_editor(g1, "file:///b.rs".into(), "b.rs".into());
        svc.open_editor(g2, "file:///c.rs".into(), "c.rs".into());

        svc.set_dirty(g1, "file:///a.rs", true).unwrap();
        svc.set_dirty(g2, "file:///c.rs", true).unwrap();

        let dirty = svc.all_dirty_uris();
        assert_eq!(dirty.len(), 2);
        assert!(dirty.contains(&(g1, "file:///a.rs")));
        assert!(dirty.contains(&(g2, "file:///c.rs")));
    }

    #[test]
    fn display_impls() {
        let tab = EditorTabBuilder::new()
            .uri("file:///test.rs")
            .label("test.rs")
            .dirty(true)
            .pinned(true)
            .build()
            .unwrap();
        assert_eq!(format!("{tab}"), "test.rs [dirty, pinned]");

        let clean_tab = EditorTabBuilder::new()
            .uri("file:///clean.rs")
            .build()
            .unwrap();
        assert_eq!(format!("{clean_tab}"), "clean.rs");

        let layout = EditorGroupLayout::Horizontal;
        assert_eq!(format!("{layout}"), "Horizontal");

        let svc = EditorGroupService::new();
        assert_eq!(
            format!("{svc}"),
            "EditorGroupService(0 groups, 0 total tabs)"
        );

        let err = EditorError::GroupNotFound(7);
        assert_eq!(format!("{err}"), "editor group 7 not found");
    }

    #[test]
    fn wb_editor_stats_new_defaults() {
        let stats = WbEditorStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_editor_stats_record_success() {
        let mut stats = WbEditorStats::new();
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
    fn wb_editor_stats_record_failure() {
        let mut stats = WbEditorStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_editor_stats_reset() {
        let mut stats = WbEditorStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_editor_stats_merge() {
        let mut a = WbEditorStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbEditorStats::new();
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
    fn wb_editor_stats_display() {
        let mut stats = WbEditorStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_editor_stats_default() {
        let stats = WbEditorStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_editor_validator_accepts_valid_name() {
        let v = WbEditorValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_editor_validator_rejects_empty() {
        let v = WbEditorValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_editor_validator_rejects_too_long() {
        let v = WbEditorValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_editor_validator_forbidden_prefix() {
        let v = WbEditorValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_editor_validator_allowed_chars() {
        let v = WbEditorValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_editor_validator_range() {
        let v = WbEditorValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_editor_sanitize_removes_control() {
        let result = WbEditorValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_editor_truncate_short_string() {
        assert_eq!(WbEditorValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_editor_truncate_long_string() {
        let result = WbEditorValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_editor_is_ascii_printable() {
        assert!(WbEditorValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbEditorValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn format_tab_label_clean() {
        let tab = EditorTabInfo {
            uri: "file:///src/main.rs".to_string(),
            label: "main.rs".to_string(),
            dirty: false,
            pinned: false,
            preview: false,
        };
        let result = format_tab_label(&tab, &TabLabelOptions::default());
        assert_eq!(result, "main.rs");
    }

    #[test]
    fn format_tab_label_dirty() {
        let tab = EditorTabInfo {
            uri: "file:///src/main.rs".to_string(),
            label: "main.rs".to_string(),
            dirty: true,
            pinned: false,
            preview: false,
        };
        let result = format_tab_label(&tab, &TabLabelOptions::default());
        assert!(result.contains('●'));
        assert!(result.starts_with("main.rs"));
    }

    #[test]
    fn format_tab_label_pinned() {
        let tab = EditorTabInfo {
            uri: "file:///src/main.rs".to_string(),
            label: "main.rs".to_string(),
            dirty: false,
            pinned: true,
            preview: false,
        };
        let result = format_tab_label(&tab, &TabLabelOptions::default());
        assert!(result.contains('📌'));
    }

    #[test]
    fn format_tab_label_no_extension() {
        let tab = EditorTabInfo {
            uri: "file:///src/main.rs".to_string(),
            label: "main.rs".to_string(),
            dirty: false,
            pinned: false,
            preview: false,
        };
        let opts = TabLabelOptions { show_extension: false, ..TabLabelOptions::default() };
        let result = format_tab_label(&tab, &opts);
        assert_eq!(result, "main");
    }

    #[test]
    fn format_tab_label_truncated() {
        let tab = EditorTabInfo {
            uri: "file:///src/very_long_filename.rs".to_string(),
            label: "very_long_filename.rs".to_string(),
            dirty: false,
            pinned: false,
            preview: false,
        };
        let opts = TabLabelOptions { max_width: 10, ..TabLabelOptions::default() };
        let result = format_tab_label(&tab, &opts);
        assert!(result.chars().count() <= 10); // char count respects multi-byte '…'
        assert!(result.ends_with('…'));
    }

    #[test]
    fn format_tab_breadcrumb_with_parent() {
        let tab = EditorTabInfo {
            uri: "/home/user/project/src/main.rs".to_string(),
            label: "main.rs".to_string(),
            dirty: false,
            pinned: false,
            preview: false,
        };
        let result = format_tab_breadcrumb(&tab);
        assert!(result.contains("main.rs"));
        assert!(result.contains("src"));
    }

    #[test]
    fn test_tab_filter_dirty() {
        let tabs = vec![
            EditorTabInfo { uri: "a.rs".into(), label: "a.rs".into(), dirty: true, pinned: false, preview: false },
            EditorTabInfo { uri: "b.rs".into(), label: "b.rs".into(), dirty: false, pinned: false, preview: false },
        ];
        let filtered = filter_tabs(&tabs, &TabFilter::dirty());
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].uri, "a.rs");
    }

    #[test]
    fn test_tab_filter_language() {
        let tabs = vec![
            EditorTabInfo { uri: "a.rs".into(), label: "a.rs".into(), dirty: false, pinned: false, preview: false },
            EditorTabInfo { uri: "b.py".into(), label: "b.py".into(), dirty: false, pinned: false, preview: false },
        ];
        let filtered = filter_tabs(&tabs, &TabFilter::for_language("rs"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].uri, "a.rs");
    }

    #[test]
    fn test_sort_tabs_by_uri() {
        let mut tabs = vec![
            EditorTabInfo { uri: "c.rs".into(), label: "c.rs".into(), dirty: false, pinned: false, preview: false },
            EditorTabInfo { uri: "a.rs".into(), label: "a.rs".into(), dirty: false, pinned: false, preview: false },
        ];
        sort_tabs(&mut tabs, TabSortOrder::ByUri);
        assert_eq!(tabs[0].uri, "a.rs");
    }

    #[test]
    fn test_tab_summary() {
        let tabs = vec![
            EditorTabInfo { uri: "a.rs".into(), label: "a.rs".into(), dirty: true, pinned: true, preview: false },
            EditorTabInfo { uri: "b.rs".into(), label: "b.rs".into(), dirty: false, pinned: false, preview: true },
            EditorTabInfo { uri: "c.rs".into(), label: "c.rs".into(), dirty: true, pinned: false, preview: false },
        ];
        let summary = TabSummary::from_tabs(&tabs);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.dirty, 2);
        assert_eq!(summary.pinned, 1);
        assert_eq!(summary.preview, 1);
        assert!(summary.has_unsaved());
        assert!(format!("{summary}").contains("3 tabs"));
    }

    #[test]
    fn test_tab_languages() {
        let tabs = vec![
            EditorTabInfo { uri: "a.rs".into(), label: "a.rs".into(), dirty: false, pinned: false, preview: false },
            EditorTabInfo { uri: "b.py".into(), label: "b.py".into(), dirty: false, pinned: false, preview: false },
            EditorTabInfo { uri: "c.rs".into(), label: "c.rs".into(), dirty: false, pinned: false, preview: false },
        ];
        let langs = tab_languages(&tabs);
        assert_eq!(langs, vec!["py", "rs"]);
    }

    #[test]
    fn test_find_duplicate_uris() {
        let tabs = vec![
            EditorTabInfo { uri: "a.rs".into(), label: "a.rs".into(), dirty: false, pinned: false, preview: false },
            EditorTabInfo { uri: "a.rs".into(), label: "a.rs".into(), dirty: true, pinned: false, preview: false },
            EditorTabInfo { uri: "b.rs".into(), label: "b.rs".into(), dirty: false, pinned: false, preview: false },
        ];
        let dupes = find_duplicate_uris(&tabs);
        assert_eq!(dupes, vec!["a.rs"]);
    }

    #[test]
    fn test_tab_sort_order_display() {
        assert_eq!(format!("{}", TabSortOrder::DirtyFirst), "dirty first");
        assert_eq!(format!("{}", TabSortOrder::PinnedFirst), "pinned first");
    }

    #[test]
    fn test_editor_group_manager_create_remove() {
        let mut mgr = EditorGroupManager::new();
        let g1 = mgr.create_group();
        let g2 = mgr.create_group();
        assert_eq!(mgr.group_count(), 2);
        assert_eq!(mgr.active_group(), Some(g1));
        mgr.remove_group(g1);
        assert_eq!(mgr.group_count(), 1);
        assert_eq!(mgr.active_group(), Some(g2));
    }

    #[test]
    fn test_editor_group_manager_move_tab() {
        let mut mgr = EditorGroupManager::new();
        let g1 = mgr.create_group();
        let g2 = mgr.create_group();
        let grp = mgr.get_group_mut(g1).unwrap();
        grp.editors.push(EditorTabInfo {
            uri: "main.rs".into(), label: "main.rs".into(),
            dirty: false, pinned: false, preview: false,
        });
        mgr.move_tab(g1, g2, "main.rs").unwrap();
        assert!(mgr.get_group(g1).unwrap().editors.is_empty());
        assert_eq!(mgr.get_group(g2).unwrap().editors.len(), 1);
    }

    #[test]
    fn test_editor_group_manager_find_tab() {
        let mut mgr = EditorGroupManager::new();
        let g1 = mgr.create_group();
        let grp = mgr.get_group_mut(g1).unwrap();
        grp.editors.push(EditorTabInfo {
            uri: "lib.rs".into(), label: "lib.rs".into(),
            dirty: false, pinned: false, preview: false,
        });
        assert_eq!(mgr.find_tab_across_groups("lib.rs"), Some(g1));
        assert_eq!(mgr.find_tab_across_groups("nope.rs"), None);
        assert_eq!(mgr.all_tabs(), vec!["lib.rs"]);
    }

    #[test]
    fn test_tab_history_tracker_push_back_forward() {
        let mut h = TabHistoryTracker::new();
        assert!(h.is_empty());
        h.push("a.rs");
        h.push("b.rs");
        h.push("c.rs");
        assert_eq!(h.len(), 3);
        assert_eq!(h.current(), Some("c.rs"));
        assert_eq!(h.back(), Some("b.rs"));
        assert_eq!(h.back(), Some("a.rs"));
        assert_eq!(h.forward(), Some("b.rs"));
    }

    #[test]
    fn test_tab_history_tracker_truncates_forward() {
        let mut h = TabHistoryTracker::new();
        h.push("a.rs");
        h.push("b.rs");
        h.push("c.rs");
        h.back();
        h.back();
        h.push("d.rs");
        assert_eq!(h.len(), 2);
        assert_eq!(h.current(), Some("d.rs"));
        assert!(h.forward().is_none());
    }

    #[test]
    fn test_editor_group_stats_display() {
        let group = EditorGroup {
            id: 42,
            editors: vec![
                EditorTabInfo { uri: "a.rs".into(), label: "a".into(), dirty: true, pinned: true, preview: false },
                EditorTabInfo { uri: "b.rs".into(), label: "b".into(), dirty: false, pinned: false, preview: true },
            ],
            active_editor: Some(0),
        };
        let stats = EditorGroupStats::from_group(42, &group);
        let display = format!("{}", stats);
        assert!(display.contains("42"));
        assert!(display.contains("2 tab(s)"));
        assert!(display.contains("1 dirty"));
    }

    // ------- EditorGridResize tests -------

    #[test]
    fn grid_resize_set_and_get() {
        let mut gr = EditorGridResize::new();
        assert_eq!(gr.group_count(), 0);
        gr.set_size(1, 50, 100);
        gr.set_size(2, 50, 100);
        assert_eq!(gr.get_size(1), Some((50, 100)));
        assert_eq!(gr.get_size(2), Some((50, 100)));
        assert_eq!(gr.get_size(99), None);
        assert_eq!(gr.group_count(), 2);
    }

    #[test]
    fn grid_resize_remove_and_reset() {
        let mut gr = EditorGridResize::new();
        gr.set_size(1, 50, 50);
        gr.set_size(2, 50, 50);
        assert!(gr.remove(1));
        assert!(!gr.remove(1));
        assert_eq!(gr.group_count(), 1);
        gr.reset();
        assert_eq!(gr.group_count(), 0);
    }

    #[test]
    fn grid_resize_normalize() {
        let mut gr = EditorGridResize::new();
        gr.set_size(1, 30, 100);
        gr.set_size(2, 70, 100);
        gr.normalize();
        let total: u32 = [gr.get_size(1).unwrap().0, gr.get_size(2).unwrap().0]
            .iter()
            .sum();
        assert_eq!(total, 100);
    }

    #[test]
    fn grid_resize_is_valid() {
        let mut gr = EditorGridResize::new();
        gr.set_size(1, 50, 50);
        assert!(gr.is_valid());
        gr.set_size(2, 0, 50);
        assert!(!gr.is_valid());
        gr.set_size(2, 50, 101);
        assert!(!gr.is_valid());
    }

    // ------- EditorTabDecorations tests -------

    #[test]
    fn tab_decorations_crud() {
        let mut dec = EditorTabDecorations::new();
        assert_eq!(dec.count(), 0);
        dec.set_decoration(
            "file:///a.rs",
            TabDecoration { icon: Some("⚡".into()), badge: Some("3".into()), badge_color: None },
        );
        assert!(dec.has_decoration("file:///a.rs"));
        assert!(!dec.has_decoration("file:///b.rs"));
        assert_eq!(dec.count(), 1);
        assert!(dec.remove_decoration("file:///a.rs"));
        assert!(!dec.has_decoration("file:///a.rs"));
    }

    #[test]
    fn tab_decorations_uris_with_badges() {
        let mut dec = EditorTabDecorations::new();
        dec.set_decoration("z.rs", TabDecoration { icon: None, badge: Some("!".into()), badge_color: None });
        dec.set_decoration("a.rs", TabDecoration { icon: None, badge: Some("2".into()), badge_color: None });
        dec.set_decoration("m.rs", TabDecoration { icon: Some("i".into()), badge: None, badge_color: None });
        let badged = dec.uris_with_badges();
        assert_eq!(badged, vec!["a.rs".to_string(), "z.rs".to_string()]);
    }

    #[test]
    fn tab_decorations_clear() {
        let mut dec = EditorTabDecorations::new();
        dec.set_decoration("x.rs", TabDecoration { icon: None, badge: None, badge_color: None });
        dec.clear();
        assert_eq!(dec.count(), 0);
    }

    // ------- EditorUntitledSequencer tests -------

    #[test]
    fn untitled_sequencer_names() {
        let mut seq = EditorUntitledSequencer::new("Untitled");
        assert_eq!(seq.current_count(), 0);
        assert_eq!(seq.next(), "Untitled-1");
        assert_eq!(seq.next(), "Untitled-2");
        assert_eq!(seq.current_count(), 2);
    }

    #[test]
    fn untitled_sequencer_reset_and_display() {
        let mut seq = EditorUntitledSequencer::new("New");
        seq.next();
        seq.next();
        seq.reset();
        assert_eq!(seq.current_count(), 0);
        assert_eq!(seq.next(), "New-1");
        let display = format!("{}", seq);
        assert!(display.contains("New"));
        assert!(display.contains("1"));
    }

    // ------- EditorCloseConfirmation tests -------

    #[test]
    fn close_confirmation_basic() {
        let mut cc = EditorCloseConfirmation::new();
        assert!(!cc.needs_confirmation());
        assert_eq!(cc.message(), "No unsaved changes.");
        cc.add_dirty("a.rs");
        cc.add_dirty("b.rs");
        assert!(cc.needs_confirmation());
        assert_eq!(cc.dirty_count(), 2);
        assert_eq!(cc.message(), "Save changes to 2 files?");
    }

    #[test]
    fn close_confirmation_confirm_cancel() {
        let mut cc = EditorCloseConfirmation::new();
        cc.add_dirty("x.rs");
        cc.confirm();
        assert!(cc.is_confirmed());
        cc.cancel();
        assert!(!cc.is_confirmed());
    }

    #[test]
    fn close_confirmation_dedup_and_display() {
        let mut cc = EditorCloseConfirmation::new();
        cc.add_dirty("a.rs");
        cc.add_dirty("a.rs");
        assert_eq!(cc.dirty_count(), 1);
        assert_eq!(cc.dirty_list(), &["a.rs".to_string()]);
        let display = format!("{}", cc);
        assert!(display.contains("1 dirty"));
    }

    #[test]
    fn close_confirmation_single_file_message() {
        let mut cc = EditorCloseConfirmation::new();
        cc.add_dirty("only.rs");
        assert_eq!(cc.message(), "Save changes to 1 file?");
    }

    #[test]
    fn editorGroupSerializer_new() {
        let s = EditorGroupSerializer::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn editorGroupSerializer_add_contains() {
        let mut s = EditorGroupSerializer::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn editorGroupSerializer_add_duplicate() {
        let mut s = EditorGroupSerializer::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn editorGroupSerializer_remove() {
        let mut s = EditorGroupSerializer::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn editorGroupSerializer_capacity() {
        let s = EditorGroupSerializer::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn editorGroupSerializer_search() {
        let mut s = EditorGroupSerializer::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn editorGroupSerializer_stats() {
        let mut s = EditorGroupSerializer::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn editorTabDecorationMerger_new() {
        let m = EditorTabDecorationMerger::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn editorTabDecorationMerger_add_find() {
        let mut m = EditorTabDecorationMerger::new();
        m.add(EditorTabDecorationMergerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn editorTabDecorationMerger_priority_filter() {
        let mut m = EditorTabDecorationMerger::new();
        m.add(EditorTabDecorationMergerItem::new("a", "A").with_priority(EditorTabDecorationMergerPriority::High));
        m.add(EditorTabDecorationMergerItem::new("b", "B").with_priority(EditorTabDecorationMergerPriority::Low));
        m.add(EditorTabDecorationMergerItem::new("c", "C").with_priority(EditorTabDecorationMergerPriority::High));
        assert_eq!(m.by_priority(EditorTabDecorationMergerPriority::High).len(), 2);
    }

    #[test]
    fn editorTabDecorationMerger_remove() {
        let mut m = EditorTabDecorationMerger::new();
        m.add(EditorTabDecorationMergerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn editorTabDecorationMerger_search() {
        let mut m = EditorTabDecorationMerger::new();
        m.add(EditorTabDecorationMergerItem::new("id1", "Hello World"));
        m.add(EditorTabDecorationMergerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn editorTabDecorationMerger_total_weight() {
        let mut m = EditorTabDecorationMerger::new();
        m.add(EditorTabDecorationMergerItem::new("a", "A").with_priority(EditorTabDecorationMergerPriority::Critical));
        m.add(EditorTabDecorationMergerItem::new("b", "B").with_priority(EditorTabDecorationMergerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn editorTabDecorationMerger_capacity_limit() {
        let mut m = EditorTabDecorationMerger::new().with_max_items(2);
        m.add(EditorTabDecorationMergerItem::new("1", "one"));
        m.add(EditorTabDecorationMergerItem::new("2", "two"));
        assert!(!m.add(EditorTabDecorationMergerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn editorTabDecorationMerger_sorted_by_priority() {
        let mut m = EditorTabDecorationMerger::new();
        m.add(EditorTabDecorationMergerItem::new("lo", "Low").with_priority(EditorTabDecorationMergerPriority::Low));
        m.add(EditorTabDecorationMergerItem::new("hi", "High").with_priority(EditorTabDecorationMergerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn editorTabDecorationMerger_item_metadata() {
        let mut item = EditorTabDecorationMergerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn editorGroupSerializer_enabled_toggle() {
        let mut s = EditorGroupSerializer::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn editorTabDecorationMerger_priority_display() {
        assert_eq!(format!("{}", EditorTabDecorationMergerPriority::High), "high");
        assert_eq!(format!("{}", EditorTabDecorationMergerPriority::Low), "low");
    }


    #[test]
    fn wbEditor_x_config_new() {
        let c = WbEditorXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn wbEditor_x_config_builder() {
        let c = WbEditorXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn wbEditor_x_config_display() {
        let c = WbEditorXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn wbEditor_x_registry_insert_get() {
        let mut reg = WbEditorXRegistry::new();
        reg.insert(WbEditorXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn wbEditor_x_registry_duplicate() {
        let mut reg = WbEditorXRegistry::new();
        reg.insert(WbEditorXConfig::new("a")).unwrap();
        assert!(reg.insert(WbEditorXConfig::new("a")).is_err());
    }

    #[test]
    fn wbEditor_x_registry_remove() {
        let mut reg = WbEditorXRegistry::new();
        reg.insert(WbEditorXConfig::new("a")).unwrap();
        reg.insert(WbEditorXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn wbEditor_x_registry_active_entries() {
        let mut reg = WbEditorXRegistry::new();
        reg.insert(WbEditorXConfig::new("a")).unwrap();
        reg.insert(WbEditorXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn wbEditor_x_registry_by_weight() {
        let mut reg = WbEditorXRegistry::new();
        reg.insert(WbEditorXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(WbEditorXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn wbEditor_x_registry_tags() {
        let mut reg = WbEditorXRegistry::new();
        reg.insert(WbEditorXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(WbEditorXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn wbEditor_x_registry_total_weight() {
        let mut reg = WbEditorXRegistry::new();
        reg.insert(WbEditorXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(WbEditorXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn wbEditor_x_registry_iterator() {
        let mut reg = WbEditorXRegistry::new();
        reg.insert(WbEditorXConfig::new("a")).unwrap();
        reg.insert(WbEditorXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn wbEditor_x_cache_put_get() {
        let mut cache = WbEditorXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn wbEditor_x_cache_eviction() {
        let mut cache = WbEditorXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn wbEditor_x_cache_lru_order() {
        let mut cache = WbEditorXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn wbEditor_x_cache_most_least_recent() {
        let mut cache = WbEditorXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn wbEditor_x_formatter_entry() {
        let e = WbEditorXConfig::new("k").with_value("v");
        let fmt = WbEditorXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn wbEditor_x_formatter_summary() {
        let mut reg = WbEditorXRegistry::new();
        reg.insert(WbEditorXConfig::new("a").with_weight(5)).unwrap();
        let fmt = WbEditorXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn wbEditor_x_validator_valid() {
        let v = WbEditorXValidator::new();
        let c = WbEditorXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn wbEditor_x_validator_empty_key() {
        let v = WbEditorXValidator::new();
        let c = WbEditorXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbEditor_x_validator_require_value() {
        let v = WbEditorXValidator::new().require_value(true);
        let c = WbEditorXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbEditor_x_validator_allowed_tags() {
        let v = WbEditorXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = WbEditorXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbEditor_x_validator_validate_all() {
        let v = WbEditorXValidator::new();
        let mut reg = WbEditorXRegistry::new();
        reg.insert(WbEditorXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
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


    #[test]
    fn xb_ring_buffer_101_push_and_len() {
        let mut rb = super::XbRingBuffer101::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_101_overwrite() {
        let mut rb = super::XbRingBuffer101::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_101_get_out_of_bounds() {
        let rb = super::XbRingBuffer101::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_101_drain_all() {
        let mut rb = super::XbRingBuffer101::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_101_peek_front_back() {
        let mut rb = super::XbRingBuffer101::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_101_clear() {
        let mut rb = super::XbRingBuffer101::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_101_capacity() {
        let rb = super::XbRingBuffer101::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_101_basic() {
        let h = super::xb_fnv1a_101(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_101(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_101_different_inputs() {
        let h1 = super::xb_fnv1a_101(b"abc");
        let h2 = super::xb_fnv1a_101(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_101_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_101(&data);
        let dec = super::xb_rle_decode_101(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_101_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_101(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_101(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_101_values() {
        assert!((super::xb_clamp_101(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_101(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_101(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_101_values() {
        assert!((super::xb_lerp_101(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_101(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_101(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_101_wrap_around_twice() {
        let mut rb = super::XbRingBuffer101::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 207 ----

    #[test]
    fn xc_207_pool_new_empty() {
        let pool: super::Xc207Pool<i32> = super::Xc207Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_207_pool_release_acquire() {
        let mut pool = super::Xc207Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_207_pool_acquire_empty() {
        let mut pool: super::Xc207Pool<i32> = super::Xc207Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_207_pool_full() {
        let mut pool = super::Xc207Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_207_pool_drain() {
        let mut pool = super::Xc207Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_207_pool_stats() {
        let mut pool = super::Xc207Pool::new(8);
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
    fn xc_207_pool_clear() {
        let mut pool = super::Xc207Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_207_pool_shrink() {
        let mut pool = super::Xc207Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_207_pool_default() {
        let pool: super::Xc207Pool<String> = super::Xc207Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_207_pool_extend() {
        let mut pool = super::Xc207Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_207_pool_retain() {
        let mut pool = super::Xc207Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_207_scheduler_round_robin() {
        let mut sched = super::Xc207Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_207_scheduler_empty() {
        let mut sched = super::Xc207Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_207_scheduler_reset() {
        let mut sched = super::Xc207Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_207_scheduler_add_remove() {
        let mut sched = super::Xc207Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_207_scheduler_targets() {
        let sched = super::Xc207Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_207_hash_empty() {
        assert_eq!(super::xc_207_hash(b""), 5381);
    }

    #[test]
    fn xc_207_hash_data() {
        let h = super::xc_207_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_207_hash(b"hello"), h);
    }

    #[test]
    fn xc_207_reverse_str() {
        assert_eq!(super::xc_207_reverse("abc"), "cba");
        assert_eq!(super::xc_207_reverse(""), "");
    }


    #[test]
    fn xe_114_pipeline_empty() {
        let p = super::Xe114Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_114_pipeline_parse_stage() {
        let p = super::Xe114Pipeline::new()
            .add_parse(super::xe_114_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_114_pipeline_transform_double() {
        let p = super::Xe114Pipeline::new()
            .add_transform(super::xe_114_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_114_pipeline_validate_reverse() {
        let p = super::Xe114Pipeline::new()
            .add_validate(super::xe_114_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_114_pipeline_emit_filter() {
        let p = super::Xe114Pipeline::new()
            .add_emit(super::xe_114_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_114_pipeline_multi_stage() {
        let p = super::Xe114Pipeline::new()
            .add_parse(super::xe_114_pipeline_identity)
            .add_transform(super::xe_114_pipeline_double)
            .add_validate(super::xe_114_pipeline_reverse)
            .add_emit(super::xe_114_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_114_pipeline_error_propagation() {
        let p = super::Xe114Pipeline::new()
            .add_parse(super::xe_114_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe114Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_114_pipeline_compose() {
        let p1 = super::Xe114Pipeline::new()
            .add_parse(super::xe_114_pipeline_identity);
        let p2 = super::Xe114Pipeline::new()
            .add_transform(super::xe_114_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_114_pipeline_error_display() {
        let e = super::Xe114PipelineError {
            stage: super::Xe114Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_114_cache_put_get() {
        let mut c = super::Xe114Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_114_cache_miss() {
        let mut c: super::Xe114Cache<&str, i32> = super::Xe114Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_114_cache_ttl_expiry() {
        let mut c = super::Xe114Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_114_cache_evict() {
        let mut c = super::Xe114Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_114_cache_capacity() {
        let mut c = super::Xe114Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_114_cache_stats() {
        let mut c = super::Xe114Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_114_cache_clear() {
        let mut c = super::Xe114Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_112 graph tests ------------------------------------------------

    #[test]
    fn xg_112_graph_empty() {
        let g = super::Xg112Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_112_graph_add_node() {
        let mut g = super::Xg112Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_112_graph_add_edge() {
        let mut g = super::Xg112Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_112_graph_neighbors() {
        let mut g = super::Xg112Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_112_graph_has_path() {
        let mut g = super::Xg112Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_112_graph_self_path() {
        let g = super::Xg112Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_112_graph_topo_sort() {
        let mut g = super::Xg112Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_112_graph_cycle_detect_false() {
        let mut g = super::Xg112Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_112_graph_cycle_detect_true() {
        let mut g = super::Xg112Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_112 heap tests -------------------------------------------------

    #[test]
    fn xg_112_heap_empty() {
        let h: super::Xg112Heap<i32> = super::Xg112Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_112_heap_push_pop() {
        let mut h = super::Xg112Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_112_heap_peek() {
        let mut h = super::Xg112Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_112_heap_drain_sorted() {
        let mut h = super::Xg112Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_112_heap_merge() {
        let mut a = super::Xg112Heap::new();
        let mut b = super::Xg112Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_112_heap_default() {
        let h: super::Xg112Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_112_graph_default() {
        let g: super::Xg112Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh206_skip_insert_contains() {
        let mut sl = super::Xh206SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh206_skip_remove() {
        let mut sl = super::Xh206SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh206_skip_len() {
        let mut sl = super::Xh206SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh206_skip_range_query() {
        let mut sl = super::Xh206SkipList::xh_new(4);
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
    fn xh206_skip_floor_ceiling() {
        let mut sl = super::Xh206SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh206_skip_rank() {
        let mut sl = super::Xh206SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh206_skip_empty() {
        let sl = super::Xh206SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh206_skip_duplicates() {
        let mut sl = super::Xh206SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh206_bitset_set_test() {
        let mut bs = super::Xh206BitSet::xh_new(256);
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
    fn xh206_bitset_clear_count() {
        let mut bs = super::Xh206BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh206_bitset_and_or_xor() {
        let mut a = super::Xh206BitSet::xh_new(128);
        let mut b = super::Xh206BitSet::xh_new(128);
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
    fn xh206_bitset_iter_ones() {
        let mut bs = super::Xh206BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh206_bitset_first_last() {
        let mut bs = super::Xh206BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh206_bitset_empty() {
        let bs = super::Xh206BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi206_deque_push_pop_back() {
        let mut dq = super::Xi206Deque::xi_new(4);
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
    fn xi206_deque_push_pop_front() {
        let mut dq = super::Xi206Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi206_deque_mixed_ops() {
        let mut dq = super::Xi206Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi206_deque_get_and_split() {
        let mut dq = super::Xi206Deque::xi_new(8);
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
    fn xi206_deque_rotate_left() {
        let mut dq = super::Xi206Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi206_deque_rotate_right() {
        let mut dq = super::Xi206Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi206_deque_grow() {
        let mut dq = super::Xi206Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi206_deque_empty() {
        let dq = super::Xi206Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi206_interval_tree_insert_query() {
        let mut tree = super::Xi206IntervalTree::xi_new();
        tree.xi_insert(super::Xi206Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi206Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi206Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi206_interval_tree_overlap() {
        let mut tree = super::Xi206IntervalTree::xi_new();
        tree.xi_insert(super::Xi206Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi206Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi206Interval::xi_new(12, 20));
        let q = super::Xi206Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi206_interval_tree_remove() {
        let mut tree = super::Xi206IntervalTree::xi_new();
        tree.xi_insert(super::Xi206Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi206Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi206_interval_tree_gaps() {
        let mut tree = super::Xi206IntervalTree::xi_new();
        tree.xi_insert(super::Xi206Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi206Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi206Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi206Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi206Interval::xi_new(8, 10));
    }

    #[test]
    fn xi206_interval_tree_merge() {
        let mut tree = super::Xi206IntervalTree::xi_new();
        tree.xi_insert(super::Xi206Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi206Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi206Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi206Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi206Interval::xi_new(10, 15));
    }

    #[test]
    fn xi206_interval_tree_all() {
        let mut tree = super::Xi206IntervalTree::xi_new();
        tree.xi_insert(super::Xi206Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi206Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi206_interval_tree_empty() {
        let tree = super::Xi206IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi206_interval_tree_contains_point() {
        let iv = super::Xi206Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 206) ---

    #[test]
    fn xj_206_uf_make_and_find() {
        let mut uf = super::Xj206UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_206_uf_union_connected() {
        let mut uf = super::Xj206UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_206_uf_component_count() {
        let mut uf = super::Xj206UnionFind::xj_new();
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
    fn xj_206_uf_component_size() {
        let mut uf = super::Xj206UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_206_uf_largest_component() {
        let mut uf = super::Xj206UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_206_uf_many_elements() {
        let mut uf = super::Xj206UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_206_uf_separate_components() {
        let mut uf = super::Xj206UnionFind::xj_new();
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
    fn xj_206_uf_path_compression() {
        let mut uf = super::Xj206UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_206_bt_insert_get() {
        let mut bt = super::Xj206BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_206_bt_contains_len() {
        let mut bt = super::Xj206BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_206_bt_replace() {
        let mut bt = super::Xj206BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_206_bt_remove() {
        let mut bt = super::Xj206BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_206_bt_keys_values() {
        let mut bt = super::Xj206BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_206_bt_range() {
        let mut bt = super::Xj206BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_206_bt_min_max() {
        let mut bt = super::Xj206BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_206_bt_many_inserts() {
        let mut bt = super::Xj206BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_206 segment tree tests ---

    #[test]
    fn xk_206_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk206SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_206_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk206SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_206_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk206SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_206_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk206SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_206_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk206SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_206_st_single_element() {
        let data = vec![42];
        let st = super::Xk206SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_206_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk206SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_206_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk206SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_206 disjoint intervals tests ---

    #[test]
    fn xk_206_di_add_and_count() {
        let mut di = super::Xk206DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_206_di_merge_overlap() {
        let mut di = super::Xk206DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_206_di_contains() {
        let mut di = super::Xk206DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_206_di_remove() {
        let mut di = super::Xk206DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_206_di_covered_length() {
        let mut di = super::Xk206DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_206_di_gaps() {
        let mut di = super::Xk206DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_206_di_merge_adjacent() {
        let mut di = super::Xk206DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_206_di_empty() {
        let di = super::Xk206DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_206_rope_new_empty() {
        let rope = super::Xl206Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_206_rope_from_str() {
        let rope = super::Xl206Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_206_rope_insert_at() {
        let mut rope = super::Xl206Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_206_rope_delete_range() {
        let mut rope = super::Xl206Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_206_rope_char_at() {
        let rope = super::Xl206Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_206_rope_split_concat() {
        let rope = super::Xl206Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_206_rope_line_count() {
        let rope = super::Xl206Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_206_rope_line_at() {
        let rope = super::Xl206Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_206_sa_build_and_search() {
        let sa = super::Xl206SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_206_sa_count() {
        let sa = super::Xl206SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_206_sa_longest_repeated() {
        let sa = super::Xl206SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_206_sa_all_positions() {
        let sa = super::Xl206SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_206_sa_len() {
        let sa = super::Xl206SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_206_sa_empty() {
        let sa = super::Xl206SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_206_rope_slice() {
        let rope = super::Xl206Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_206_sa_search_start() {
        let sa = super::Xl206SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_206_sparse_set_get() {
        let mut m = super::Xm206MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_206_sparse_row_col() {
        let mut m = super::Xm206MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_206_sparse_transpose() {
        let mut m = super::Xm206MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_206_sparse_multiply_vec() {
        let mut m = super::Xm206MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_206_sparse_nnz_density() {
        let mut m = super::Xm206MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_206_sparse_clear() {
        let mut m = super::Xm206MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_206_sparse_overwrite_zero() {
        let mut m = super::Xm206MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_206_tokenizer_basic() {
        let t = super::Xm206Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_206_tokenizer_count() {
        let t = super::Xm206Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_206_tokenizer_unique() {
        let t = super::Xm206Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_206_tokenizer_frequency() {
        let t = super::Xm206Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_206_tokenizer_delimiter() {
        let t = super::Xm206Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_206_tokenizer_whitespace() {
        let t = super::Xm206Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_206_tokenizer_empty() {
        let t = super::Xm206Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 206 ----

    #[test]
    fn xn_206_fenwick_prefix_sum() {
        let mut ft = super::Xn206Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_206_fenwick_range_sum() {
        let mut ft = super::Xn206Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_206_fenwick_point_query() {
        let mut ft = super::Xn206Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_206_fenwick_len() {
        let ft = super::Xn206Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_206_fenwick_multiple_updates() {
        let mut ft = super::Xn206Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_206_fenwick_single_element() {
        let mut ft = super::Xn206Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_206_fenwick_find_kth() {
        let mut ft = super::Xn206Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_206_fenwick_negative_delta() {
        let mut ft = super::Xn206Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 206 ----

    #[test]
    fn xn_206_avl_insert_get() {
        let mut m = super::Xn206AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_206_avl_remove() {
        let mut m = super::Xn206AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_206_avl_in_order() {
        let mut m = super::Xn206AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_206_avl_min_max() {
        let mut m = super::Xn206AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_206_avl_floor_ceiling() {
        let mut m = super::Xn206AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_206_avl_height_balanced() {
        let mut m = super::Xn206AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_206_avl_overwrite() {
        let mut m = super::Xn206AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_206_avl_empty() {
        let m: super::Xn206AVL<i32, i32> = super::Xn206AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo206RedBlack tests ---

    #[test]
    fn xo_206_rb_insert_and_get() {
        let mut tree = super::Xo206RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_206_rb_len_and_empty() {
        let mut tree = super::Xo206RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_206_rb_min_max() {
        let mut tree = super::Xo206RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_206_rb_contains() {
        let mut tree = super::Xo206RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_206_rb_remove() {
        let mut tree = super::Xo206RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_206_rb_in_order() {
        let mut tree = super::Xo206RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_206_rb_black_height() {
        let mut tree = super::Xo206RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_206_rb_overwrite() {
        let mut tree = super::Xo206RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo206ConsistentHash tests ---

    #[test]
    fn xo_206_ch_add_and_count() {
        let mut ring = super::Xo206ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_206_ch_remove_node() {
        let mut ring = super::Xo206ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_206_ch_get_node() {
        let mut ring = super::Xo206ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_206_ch_empty_ring() {
        let ring = super::Xo206ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_206_ch_distribution() {
        let mut ring = super::Xo206ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_206_ch_rebalance() {
        let mut ring = super::Xo206ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_206_ch_virtual_nodes() {
        let mut ring = super::Xo206ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_206_ch_consistent_lookup() {
        let mut ring = super::Xo206ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}
