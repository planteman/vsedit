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

}
