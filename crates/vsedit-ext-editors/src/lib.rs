//! Ext API: Editors.
//!
//! RPC bridge between the extension host and the main thread for editors.

use std::fmt;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_editors";

// ── RPC message types ──

/// Messages exchanged for the `TextEditor` API surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EditorMessage {
    ShowDocument { uri: String, options: Option<ShowDocumentOptions> },
    GetActiveEditor,
    SetSelection { editor_id: String, selections: Vec<EditorSelection> },
    RevealRange { editor_id: String, range: EditorRange, reveal_type: RevealType },
    SetDecorations { editor_id: String, decoration_type: String, decorations: Vec<TextEditorDecoration> },
    InsertSnippet { editor_id: String, snippet: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowDocumentOptions {
    #[serde(default)]
    pub preview: bool,
    pub selection: Option<EditorRange>,
}

/// A selection in a text editor, defined by anchor and active positions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSelection {
    pub anchor_line: u32,
    pub anchor_col: u32,
    pub active_line: u32,
    pub active_col: u32,
}

/// A range within a text editor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// How a range should be revealed in the editor viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RevealType {
    Default,
    InCenter,
    InCenterIfOutsideViewport,
    AtTop,
}

/// A decoration applied to a text editor range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEditorDecoration {
    pub range: EditorRange,
    #[serde(default)]
    pub hover_message: Option<String>,
    #[serde(default)]
    pub style: Option<String>,
}

/// Response payload returned by editor queries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EditorResponse {
    ActiveEditor { editor_id: Option<String>, uri: Option<String> },
    Ok,
}

// ── Bridge ──

/// Tracks extension-side editor state and maps IDs to internal editors.
#[derive(Debug, Default)]
pub struct EditorBridge {
    active_editor: Option<String>,
    editors: HashMap<String, EditorState>,
}

#[derive(Debug, Clone)]
struct EditorState {
    uri: String,
    selections: Vec<EditorSelection>,
}

impl EditorBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an editor visible to extensions.
    pub fn add_editor(&mut self, editor_id: String, uri: String) {
        self.editors.insert(editor_id.clone(), EditorState { uri, selections: Vec::new() });
        self.active_editor = Some(editor_id);
    }

    /// Process an incoming editor message and return a response.
    pub fn handle(&mut self, msg: EditorMessage) -> EditorResponse {
        match msg {
            EditorMessage::ShowDocument { uri, .. } => {
                let id = format!("editor-{}", self.editors.len());
                self.add_editor(id, uri);
                EditorResponse::Ok
            }
            EditorMessage::GetActiveEditor => {
                let (editor_id, uri) = self.active_editor.as_ref().map_or(
                    (None, None),
                    |id| (Some(id.clone()), self.editors.get(id).map(|s| s.uri.clone())),
                );
                EditorResponse::ActiveEditor { editor_id, uri }
            }
            EditorMessage::SetSelection { editor_id, selections } => {
                if let Some(state) = self.editors.get_mut(&editor_id) {
                    state.selections = selections;
                }
                EditorResponse::Ok
            }
            EditorMessage::RevealRange { .. }
            | EditorMessage::SetDecorations { .. }
            | EditorMessage::InsertSnippet { .. } => EditorResponse::Ok,
        }
    }

    pub fn editor_count(&self) -> usize {
        self.editors.len()
    }
}

// ── Error types ──

/// Errors that can occur when processing editor operations.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorError {
    /// The referenced editor ID does not exist.
    EditorNotFound(String),
    /// A range is invalid (start is after end).
    InvalidRange { start_line: u32, start_col: u32, end_line: u32, end_col: u32 },
    /// A selection is empty when a non-empty one is required.
    EmptySelection,
    /// The URI is empty or malformed.
    InvalidUri(String),
}

impl std::fmt::Display for EditorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditorError::EditorNotFound(id) => write!(f, "editor not found: {id}"),
            EditorError::InvalidRange { start_line, start_col, end_line, end_col } => {
                write!(f, "invalid range: ({start_line},{start_col})..({end_line},{end_col})")
            }
            EditorError::EmptySelection => write!(f, "selection must not be empty"),
            EditorError::InvalidUri(uri) => write!(f, "invalid URI: {uri}"),
        }
    }
}

impl std::error::Error for EditorError {}

// ── Validation ──

impl EditorRange {
    /// Returns `true` if the range is empty (start equals end).
    pub fn is_empty(&self) -> bool {
        self.start_line == self.end_line && self.start_col == self.end_col
    }

    /// Returns `true` if the range is well-formed (start <= end).
    pub fn is_valid(&self) -> bool {
        (self.start_line, self.start_col) <= (self.end_line, self.end_col)
    }

    /// Validates this range, returning an error if malformed.
    pub fn validate(&self) -> Result<(), EditorError> {
        if self.is_valid() {
            Ok(())
        } else {
            Err(EditorError::InvalidRange {
                start_line: self.start_line,
                start_col: self.start_col,
                end_line: self.end_line,
                end_col: self.end_col,
            })
        }
    }

    /// Number of lines spanned by this range (inclusive).
    pub fn line_count(&self) -> u32 {
        if self.is_valid() {
            self.end_line - self.start_line + 1
        } else {
            0
        }
    }

    /// Returns `true` if the given line/col position is inside this range.
    pub fn contains(&self, line: u32, col: u32) -> bool {
        if !self.is_valid() {
            return false;
        }
        let after_start =
            line > self.start_line || (line == self.start_line && col >= self.start_col);
        let before_end = line < self.end_line || (line == self.end_line && col <= self.end_col);
        after_start && before_end
    }

    /// Merge two ranges into the smallest range that contains both.
    pub fn union(&self, other: &EditorRange) -> EditorRange {
        let (sl, sc) = if (self.start_line, self.start_col) <= (other.start_line, other.start_col) {
            (self.start_line, self.start_col)
        } else {
            (other.start_line, other.start_col)
        };
        let (el, ec) = if (self.end_line, self.end_col) >= (other.end_line, other.end_col) {
            (self.end_line, self.end_col)
        } else {
            (other.end_line, other.end_col)
        };
        EditorRange { start_line: sl, start_col: sc, end_line: el, end_col: ec }
    }
}

impl std::fmt::Display for EditorRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}:{}..{}:{}]", self.start_line, self.start_col, self.end_line, self.end_col)
    }
}

impl EditorSelection {
    /// Returns the range spanned by this selection (ordered start..end).
    pub fn to_range(&self) -> EditorRange {
        let (sl, sc, el, ec) =
            if (self.anchor_line, self.anchor_col) <= (self.active_line, self.active_col) {
                (self.anchor_line, self.anchor_col, self.active_line, self.active_col)
            } else {
                (self.active_line, self.active_col, self.anchor_line, self.anchor_col)
            };
        EditorRange { start_line: sl, start_col: sc, end_line: el, end_col: ec }
    }

    /// Returns `true` when anchor equals active (cursor with no selection).
    pub fn is_cursor(&self) -> bool {
        self.anchor_line == self.active_line && self.anchor_col == self.active_col
    }

    /// Returns `true` when the selection direction is reversed (active before anchor).
    pub fn is_reversed(&self) -> bool {
        (self.active_line, self.active_col) < (self.anchor_line, self.anchor_col)
    }
}

impl std::fmt::Display for EditorSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Sel({}:{} -> {}:{})",
            self.anchor_line, self.anchor_col, self.active_line, self.active_col
        )
    }
}

// ── Builder for ShowDocumentOptions ──

/// Fluent builder for [`ShowDocumentOptions`].
#[derive(Debug, Default)]
pub struct ShowDocumentOptionsBuilder {
    preview: bool,
    selection: Option<EditorRange>,
}

impl ShowDocumentOptionsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn preview(mut self, preview: bool) -> Self {
        self.preview = preview;
        self
    }

    pub fn selection(mut self, range: EditorRange) -> Self {
        self.selection = Some(range);
        self
    }

    /// Builds the options, validating the selection range if present.
    pub fn build(self) -> Result<ShowDocumentOptions, EditorError> {
        if let Some(ref range) = self.selection {
            range.validate()?;
        }
        Ok(ShowDocumentOptions { preview: self.preview, selection: self.selection })
    }
}

// ── Extended bridge methods ──

impl EditorBridge {
    /// Remove an editor from the bridge, clearing active if it was selected.
    pub fn remove_editor(&mut self, editor_id: &str) -> bool {
        let removed = self.editors.remove(editor_id).is_some();
        if removed {
            if self.active_editor.as_deref() == Some(editor_id) {
                self.active_editor = self.editors.keys().next().cloned();
            }
        }
        removed
    }

    /// Set the active editor, returning an error if the ID is unknown.
    pub fn set_active_editor(&mut self, editor_id: &str) -> Result<(), EditorError> {
        if self.editors.contains_key(editor_id) {
            self.active_editor = Some(editor_id.to_owned());
            Ok(())
        } else {
            Err(EditorError::EditorNotFound(editor_id.to_owned()))
        }
    }

    /// Returns the active editor ID, if any.
    pub fn active_editor_id(&self) -> Option<&str> {
        self.active_editor.as_deref()
    }

    /// Returns the URI for the given editor.
    pub fn editor_uri(&self, editor_id: &str) -> Option<&str> {
        self.editors.get(editor_id).map(|s| s.uri.as_str())
    }

    /// Returns all editor IDs currently tracked.
    pub fn editor_ids(&self) -> Vec<&str> {
        self.editors.keys().map(String::as_str).collect()
    }

    /// Returns selections for the given editor, or an error if not found.
    pub fn selections(&self, editor_id: &str) -> Result<&[EditorSelection], EditorError> {
        self.editors
            .get(editor_id)
            .map(|s| s.selections.as_slice())
            .ok_or_else(|| EditorError::EditorNotFound(editor_id.to_owned()))
    }

    /// Validates a URI for use with the bridge.
    pub fn validate_uri(uri: &str) -> Result<(), EditorError> {
        if uri.is_empty() || !uri.contains("://") {
            Err(EditorError::InvalidUri(uri.to_owned()))
        } else {
            Ok(())
        }
    }

    /// Process a message with full validation, returning typed errors.
    pub fn handle_checked(&mut self, msg: EditorMessage) -> Result<EditorResponse, EditorError> {
        match msg {
            EditorMessage::ShowDocument { ref uri, .. } => {
                Self::validate_uri(uri)?;
            }
            EditorMessage::SetSelection { ref editor_id, ref selections } => {
                if !self.editors.contains_key(editor_id) {
                    return Err(EditorError::EditorNotFound(editor_id.clone()));
                }
                if selections.is_empty() {
                    return Err(EditorError::EmptySelection);
                }
            }
            EditorMessage::RevealRange { ref editor_id, ref range, .. } => {
                if !self.editors.contains_key(editor_id) {
                    return Err(EditorError::EditorNotFound(editor_id.clone()));
                }
                range.validate()?;
            }
            EditorMessage::SetDecorations { ref editor_id, .. }
            | EditorMessage::InsertSnippet { ref editor_id, .. } => {
                if !self.editors.contains_key(editor_id) {
                    return Err(EditorError::EditorNotFound(editor_id.clone()));
                }
            }
            EditorMessage::GetActiveEditor => {}
        }
        Ok(self.handle(msg))
    }
}

impl std::fmt::Display for EditorBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EditorBridge(editors={}, active={:?})",
            self.editors.len(),
            self.active_editor
        )
    }
}

impl PartialEq for EditorBridge {
    fn eq(&self, other: &Self) -> bool {
        self.active_editor == other.active_editor && self.editor_count() == other.editor_count()
    }
}

impl Clone for EditorBridge {
    fn clone(&self) -> Self {
        let mut new = EditorBridge::new();
        for (id, state) in &self.editors {
            new.editors.insert(id.clone(), state.clone());
        }
        new.active_editor = self.active_editor.clone();
        new
    }
}

impl std::fmt::Display for RevealType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RevealType::Default => write!(f, "default"),
            RevealType::InCenter => write!(f, "center"),
            RevealType::InCenterIfOutsideViewport => write!(f, "center-if-outside"),
            RevealType::AtTop => write!(f, "top"),
        }
    }
}

/// Initialize the editors extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Represents a snapshot of an editor's state at a point in time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorSnapshot {
    pub editor_id: String,
    pub uri: String,
    pub selections: Vec<EditorSelection>,
}

/// Diff between two editor states.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorStateDiff {
    pub uri_changed: bool,
    pub selections_added: usize,
    pub selections_removed: usize,
    pub old_uri: String,
    pub new_uri: String,
}

impl EditorBridge {
    /// Take a snapshot of a specific editor's current state.
    pub fn snapshot(&self, editor_id: &str) -> Result<EditorSnapshot, EditorError> {
        let state = self
            .editors
            .get(editor_id)
            .ok_or_else(|| EditorError::EditorNotFound(editor_id.to_owned()))?;
        Ok(EditorSnapshot {
            editor_id: editor_id.to_owned(),
            uri: state.uri.clone(),
            selections: state.selections.clone(),
        })
    }

    /// Compute the diff between two editor snapshots.
    pub fn diff_snapshots(old: &EditorSnapshot, new: &EditorSnapshot) -> EditorStateDiff {
        let old_count = old.selections.len();
        let new_count = new.selections.len();
        EditorStateDiff {
            uri_changed: old.uri != new.uri,
            selections_added: if new_count > old_count { new_count - old_count } else { 0 },
            selections_removed: if old_count > new_count { old_count - new_count } else { 0 },
            old_uri: old.uri.clone(),
            new_uri: new.uri.clone(),
        }
    }
}

/// Merge two selections into one that covers both ranges.
pub fn merge_selections(a: &EditorSelection, b: &EditorSelection) -> EditorSelection {
    let range_a = a.to_range();
    let range_b = b.to_range();
    let merged = range_a.union(&range_b);
    EditorSelection {
        anchor_line: merged.start_line,
        anchor_col: merged.start_col,
        active_line: merged.end_line,
        active_col: merged.end_col,
    }
}

/// Check if two decoration ranges overlap, indicating a conflict.
pub fn decorations_conflict(a: &TextEditorDecoration, b: &TextEditorDecoration) -> bool {
    if !a.range.is_valid() || !b.range.is_valid() {
        return false;
    }
    let a_before_b = (a.range.end_line, a.range.end_col) <= (b.range.start_line, b.range.start_col);
    let b_before_a = (b.range.end_line, b.range.end_col) <= (a.range.start_line, a.range.start_col);
    !(a_before_b || b_before_a)
}

/// Resolve conflicting decorations by merging overlapping ones.
/// Returns a new list with non-overlapping decorations.
pub fn resolve_decoration_conflicts(decorations: &[TextEditorDecoration]) -> Vec<TextEditorDecoration> {
    if decorations.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<TextEditorDecoration> = decorations.to_vec();
    sorted.sort_by_key(|d| (d.range.start_line, d.range.start_col));
    let mut result: Vec<TextEditorDecoration> = vec![sorted[0].clone()];
    for dec in sorted.iter().skip(1) {
        let last = result.last().unwrap().clone();
        if decorations_conflict(&last, dec) {
            let merged_range = last.range.union(&dec.range);
            let merged = TextEditorDecoration {
                range: merged_range,
                hover_message: last.hover_message.or(dec.hover_message.clone()),
                style: last.style.or(dec.style.clone()),
            };
            *result.last_mut().unwrap() = merged;
        } else {
            result.push(dec.clone());
        }
    }
    result
}

// ── Tab serialization ──

/// Saved state of a single editor tab, suitable for persistence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorTabState {
    pub uri: String,
    pub cursor_line: u32,
    pub cursor_col: u32,
    pub scroll_top: u32,
    pub view_state: Option<String>,
    pub is_pinned: bool,
    pub is_preview: bool,
    pub is_dirty: bool,
}

impl EditorTabState {
    /// Create a new tab state with minimal required fields.
    pub fn new(uri: impl Into<String>, cursor_line: u32, cursor_col: u32) -> Self {
        Self {
            uri: uri.into(),
            cursor_line,
            cursor_col,
            scroll_top: 0,
            view_state: None,
            is_pinned: false,
            is_preview: false,
            is_dirty: false,
        }
    }

    /// Validate that the tab state has a non-empty URI.
    pub fn validate(&self) -> Result<(), EditorError> {
        if self.uri.is_empty() {
            return Err(EditorError::InvalidUri(self.uri.clone()));
        }
        Ok(())
    }
}

impl std::fmt::Display for EditorTabState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Tab({} @{}:{} scroll={}{}{})",
            self.uri,
            self.cursor_line,
            self.cursor_col,
            self.scroll_top,
            if self.is_pinned { " pinned" } else { "" },
            if self.is_dirty { " dirty" } else { "" },
        )
    }
}

/// Serializer/deserializer for a collection of editor tab states.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorTabSerializer {
    pub tabs: Vec<EditorTabState>,
    pub active_tab_index: Option<usize>,
}

impl EditorTabSerializer {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_index: None,
        }
    }

    /// Add a tab state to the serializer. Returns the index of the added tab.
    pub fn add_tab(&mut self, tab: EditorTabState) -> usize {
        let idx = self.tabs.len();
        self.tabs.push(tab);
        if self.active_tab_index.is_none() {
            self.active_tab_index = Some(idx);
        }
        idx
    }

    /// Remove a tab by index, adjusting active_tab_index as needed.
    pub fn remove_tab(&mut self, index: usize) -> Option<EditorTabState> {
        if index >= self.tabs.len() {
            return None;
        }
        let tab = self.tabs.remove(index);
        if let Some(active) = self.active_tab_index {
            if active == index {
                self.active_tab_index = if self.tabs.is_empty() {
                    None
                } else {
                    Some(active.min(self.tabs.len() - 1))
                };
            } else if active > index {
                self.active_tab_index = Some(active - 1);
            }
        }
        Some(tab)
    }

    /// Set the active tab index.
    pub fn set_active(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active_tab_index = Some(index);
            true
        } else {
            false
        }
    }

    /// Get the active tab state, if any.
    pub fn active_tab(&self) -> Option<&EditorTabState> {
        self.active_tab_index.and_then(|i| self.tabs.get(i))
    }

    /// Serialize all tab states to a JSON string.
    pub fn serialize(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    /// Deserialize tab states from a JSON string.
    pub fn deserialize(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    /// Find the index of the first tab with the given URI.
    pub fn find_tab_by_uri(&self, uri: &str) -> Option<usize> {
        self.tabs.iter().position(|t| t.uri == uri)
    }

    /// Return the number of dirty (unsaved) tabs.
    pub fn dirty_count(&self) -> usize {
        self.tabs.iter().filter(|t| t.is_dirty).count()
    }

    /// Return the number of pinned tabs.
    pub fn pinned_count(&self) -> usize {
        self.tabs.iter().filter(|t| t.is_pinned).count()
    }

    /// Validate all tabs in the serializer.
    pub fn validate_all(&self) -> Result<(), EditorError> {
        for tab in &self.tabs {
            tab.validate()?;
        }
        if let Some(idx) = self.active_tab_index {
            if idx >= self.tabs.len() {
                return Err(EditorError::InvalidUri(format!(
                    "active_tab_index {} out of bounds (len {})",
                    idx,
                    self.tabs.len()
                )));
            }
        }
        Ok(())
    }

    /// Number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
}

// ── EditorSelection helpers ──

impl EditorSelection {
    pub fn length(&self) -> u32 {
        let r = self.to_range();
        if r.start_line == r.end_line {
            r.end_col.saturating_sub(r.start_col)
        } else {
            // Cross-line length is not well-defined without line lengths,
            // but we report the column span of the bounding rectangle.
            let full_lines = r.end_line - r.start_line - 1;
            // Approximate: chars remaining on first line + full intermediate lines + chars on last line
            // Without line-length info, count columns on first + last and 1 per intermediate newline.
            (r.end_col) + full_lines + (u32::MAX - r.start_col).min(r.start_col) + 1
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor_line == self.active_line && self.anchor_col == self.active_col
    }

    pub fn contains_line(&self, line: u32) -> bool {
        let r = self.to_range();
        line >= r.start_line && line <= r.end_line
    }
}

// ── EditorRange additional methods ──

impl EditorRange {
    pub fn merge(&self, other: &EditorRange) -> EditorRange {
        self.union(other)
    }

    pub fn overlaps(&self, other: &EditorRange) -> bool {
        if !self.is_valid() || !other.is_valid() {
            return false;
        }
        let self_before = (self.end_line, self.end_col) <= (other.start_line, other.start_col);
        let other_before = (other.end_line, other.end_col) <= (self.start_line, self.start_col);
        !(self_before || other_before)
    }

    pub fn contains_line(&self, line: u32) -> bool {
        self.is_valid() && line >= self.start_line && line <= self.end_line
    }
}

// ── EditorSelectionSet ──

#[derive(Debug, Clone, PartialEq)]
pub struct EditorSelectionSet {
    selections: Vec<EditorSelection>,
}

impl EditorSelectionSet {
    pub fn new(selections: Vec<EditorSelection>) -> Self {
        Self { selections }
    }

    pub fn from_slice(selections: &[EditorSelection]) -> Self {
        Self { selections: selections.to_vec() }
    }

    pub fn len(&self) -> usize {
        self.selections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }

    pub fn total_lines(&self) -> u32 {
        self.selections.iter().map(|s| s.to_range().line_count()).sum()
    }

    pub fn has_overlaps(&self) -> bool {
        let ranges: Vec<EditorRange> = self.selections.iter().map(|s| s.to_range()).collect();
        for i in 0..ranges.len() {
            for j in (i + 1)..ranges.len() {
                if ranges[i].overlaps(&ranges[j]) {
                    return true;
                }
            }
        }
        false
    }

    pub fn sorted(&self) -> Self {
        let mut sels = self.selections.clone();
        sels.sort_by(|a, b| {
            let ra = a.to_range();
            let rb = b.to_range();
            (ra.start_line, ra.start_col).cmp(&(rb.start_line, rb.start_col))
        });
        Self { selections: sels }
    }

    pub fn merge_overlapping(&self) -> Self {
        if self.selections.is_empty() {
            return Self::new(Vec::new());
        }
        let sorted = self.sorted();
        let mut ranges: Vec<EditorRange> = sorted.selections.iter().map(|s| s.to_range()).collect();
        let mut merged: Vec<EditorRange> = vec![ranges.remove(0)];
        for r in &ranges {
            let last = merged.last().unwrap().clone();
            if last.overlaps(r) || (last.end_line, last.end_col) == (r.start_line, r.start_col) {
                *merged.last_mut().unwrap() = last.merge(r);
            } else {
                merged.push(r.clone());
            }
        }
        Self {
            selections: merged
                .into_iter()
                .map(|r| EditorSelection {
                    anchor_line: r.start_line,
                    anchor_col: r.start_col,
                    active_line: r.end_line,
                    active_col: r.end_col,
                })
                .collect(),
        }
    }

    pub fn selections(&self) -> &[EditorSelection] {
        &self.selections
    }
}

impl IntoIterator for EditorSelectionSet {
    type Item = EditorSelection;
    type IntoIter = std::vec::IntoIter<EditorSelection>;

    fn into_iter(self) -> Self::IntoIter {
        self.selections.into_iter()
    }
}

impl<'a> IntoIterator for &'a EditorSelectionSet {
    type Item = &'a EditorSelection;
    type IntoIter = std::slice::Iter<'a, EditorSelection>;

    fn into_iter(self) -> Self::IntoIter {
        self.selections.iter()
    }
}

impl fmt::Display for EditorSelectionSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SelectionSet({} selections, {} total lines)", self.len(), self.total_lines())
    }
}

impl fmt::Display for EditorStateDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Diff(uri_changed={}, +{}/-{} selections)",
            self.uri_changed, self.selections_added, self.selections_removed
        )
    }
}

impl fmt::Display for EditorSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Snapshot({}: {} @ {} selections)",
            self.editor_id,
            self.uri,
            self.selections.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn show_document_adds_editor() {
        let mut bridge = EditorBridge::new();
        bridge.handle(EditorMessage::ShowDocument {
            uri: "file:///a.rs".into(),
            options: None,
        });
        assert_eq!(bridge.editor_count(), 1);
    }

    #[test]
    fn get_active_editor() {
        let mut bridge = EditorBridge::new();
        bridge.add_editor("e1".into(), "file:///a.rs".into());
        let resp = bridge.handle(EditorMessage::GetActiveEditor);
        assert_eq!(
            resp,
            EditorResponse::ActiveEditor {
                editor_id: Some("e1".into()),
                uri: Some("file:///a.rs".into()),
            }
        );
    }

    #[test]
    fn set_selection() {
        let mut bridge = EditorBridge::new();
        bridge.add_editor("e1".into(), "file:///a.rs".into());
        let sel = EditorSelection { anchor_line: 0, anchor_col: 0, active_line: 0, active_col: 5 };
        bridge.handle(EditorMessage::SetSelection {
            editor_id: "e1".into(),
            selections: vec![sel],
        });
        assert_eq!(bridge.editor_count(), 1);
    }

    #[test]
    fn no_active_editor_when_empty() {
        let bridge = EditorBridge::new();
        assert_eq!(bridge.editor_count(), 0);
    }

    #[test]
    fn serde_round_trip() {
        let msg = EditorMessage::RevealRange {
            editor_id: "e1".into(),
            range: EditorRange { start_line: 0, start_col: 0, end_line: 10, end_col: 0 },
            reveal_type: RevealType::InCenter,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: EditorMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }

    // ── Additional tests ──

    #[test]
    fn range_validation_valid() {
        let r = EditorRange { start_line: 1, start_col: 0, end_line: 5, end_col: 10 };
        assert!(r.is_valid());
        assert!(r.validate().is_ok());
    }

    #[test]
    fn range_validation_invalid() {
        let r = EditorRange { start_line: 10, start_col: 0, end_line: 2, end_col: 0 };
        assert!(!r.is_valid());
        assert!(matches!(r.validate(), Err(EditorError::InvalidRange { .. })));
    }

    #[test]
    fn range_contains_position() {
        let r = EditorRange { start_line: 5, start_col: 0, end_line: 10, end_col: 20 };
        assert!(r.contains(7, 3));
        assert!(r.contains(5, 0)); // start boundary
        assert!(r.contains(10, 20)); // end boundary
        assert!(!r.contains(4, 0));
        assert!(!r.contains(10, 21));
    }

    #[test]
    fn range_line_count() {
        let r = EditorRange { start_line: 3, start_col: 0, end_line: 7, end_col: 0 };
        assert_eq!(r.line_count(), 5);
    }

    #[test]
    fn range_union() {
        let a = EditorRange { start_line: 2, start_col: 5, end_line: 4, end_col: 10 };
        let b = EditorRange { start_line: 1, start_col: 0, end_line: 3, end_col: 8 };
        let u = a.union(&b);
        assert_eq!(u.start_line, 1);
        assert_eq!(u.start_col, 0);
        assert_eq!(u.end_line, 4);
        assert_eq!(u.end_col, 10);
    }

    #[test]
    fn range_is_empty() {
        let empty = EditorRange { start_line: 3, start_col: 5, end_line: 3, end_col: 5 };
        let non_empty = EditorRange { start_line: 3, start_col: 5, end_line: 3, end_col: 6 };
        assert!(empty.is_empty());
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn selection_to_range_and_cursor() {
        let cursor = EditorSelection { anchor_line: 3, anchor_col: 5, active_line: 3, active_col: 5 };
        assert!(cursor.is_cursor());
        let r = cursor.to_range();
        assert!(r.is_empty());

        let sel = EditorSelection { anchor_line: 1, anchor_col: 0, active_line: 5, active_col: 3 };
        assert!(!sel.is_cursor());
        assert!(!sel.is_reversed());
    }

    #[test]
    fn selection_reversed() {
        let sel = EditorSelection { anchor_line: 10, anchor_col: 0, active_line: 2, active_col: 5 };
        assert!(sel.is_reversed());
        let r = sel.to_range();
        assert_eq!(r.start_line, 2);
        assert_eq!(r.end_line, 10);
    }

    #[test]
    fn remove_editor_updates_active() {
        let mut bridge = EditorBridge::new();
        bridge.add_editor("e1".into(), "file:///a.rs".into());
        bridge.add_editor("e2".into(), "file:///b.rs".into());
        assert_eq!(bridge.active_editor_id(), Some("e2"));
        bridge.remove_editor("e2");
        assert_eq!(bridge.editor_count(), 1);
        // active should fall back to remaining editor
        assert!(bridge.active_editor_id().is_some());
    }

    #[test]
    fn set_active_editor_error() {
        let mut bridge = EditorBridge::new();
        let err = bridge.set_active_editor("nonexistent").unwrap_err();
        assert_eq!(err, EditorError::EditorNotFound("nonexistent".into()));
        assert_eq!(err.to_string(), "editor not found: nonexistent");
    }

    #[test]
    fn handle_checked_validates_uri() {
        let mut bridge = EditorBridge::new();
        let result = bridge.handle_checked(EditorMessage::ShowDocument {
            uri: "bad-uri".into(),
            options: None,
        });
        assert!(matches!(result, Err(EditorError::InvalidUri(_))));
    }

    #[test]
    fn handle_checked_validates_editor_exists() {
        let mut bridge = EditorBridge::new();
        let result = bridge.handle_checked(EditorMessage::SetSelection {
            editor_id: "missing".into(),
            selections: vec![EditorSelection {
                anchor_line: 0,
                anchor_col: 0,
                active_line: 0,
                active_col: 5,
            }],
        });
        assert!(matches!(result, Err(EditorError::EditorNotFound(_))));
    }

    #[test]
    fn handle_checked_rejects_empty_selections() {
        let mut bridge = EditorBridge::new();
        bridge.add_editor("e1".into(), "file:///a.rs".into());
        let result = bridge.handle_checked(EditorMessage::SetSelection {
            editor_id: "e1".into(),
            selections: vec![],
        });
        assert!(matches!(result, Err(EditorError::EmptySelection)));
    }

    #[test]
    fn show_document_options_builder() {
        let opts = ShowDocumentOptionsBuilder::new()
            .preview(true)
            .selection(EditorRange { start_line: 1, start_col: 0, end_line: 5, end_col: 10 })
            .build()
            .unwrap();
        assert!(opts.preview);
        assert!(opts.selection.is_some());
    }

    #[test]
    fn show_document_options_builder_invalid_range() {
        let result = ShowDocumentOptionsBuilder::new()
            .selection(EditorRange { start_line: 10, start_col: 0, end_line: 1, end_col: 0 })
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn bridge_display() {
        let mut bridge = EditorBridge::new();
        bridge.add_editor("e1".into(), "file:///a.rs".into());
        let s = format!("{bridge}");
        assert!(s.contains("editors=1"));
    }

    #[test]
    fn reveal_type_display() {
        assert_eq!(format!("{}", RevealType::InCenter), "center");
        assert_eq!(format!("{}", RevealType::AtTop), "top");
    }

    #[test]
    fn range_display() {
        let r = EditorRange { start_line: 1, start_col: 2, end_line: 3, end_col: 4 };
        assert_eq!(format!("{r}"), "[1:2..3:4]");
    }

    #[test]
    fn selection_display() {
        let s = EditorSelection { anchor_line: 1, anchor_col: 2, active_line: 3, active_col: 4 };
        assert_eq!(format!("{s}"), "Sel(1:2 -> 3:4)");
    }

    #[test]
    fn editor_error_display() {
        let e = EditorError::EmptySelection;
        assert_eq!(e.to_string(), "selection must not be empty");
    }

    #[test]
    fn bridge_clone_and_eq() {
        let mut bridge = EditorBridge::new();
        bridge.add_editor("e1".into(), "file:///a.rs".into());
        let clone = bridge.clone();
        assert_eq!(bridge, clone);
    }

    #[test]
    fn validate_uri_accepts_valid() {
        assert!(EditorBridge::validate_uri("file:///foo.rs").is_ok());
        assert!(EditorBridge::validate_uri("").is_err());
        assert!(EditorBridge::validate_uri("no-scheme").is_err());
    }

    #[test]
    fn selections_returns_error_for_missing_editor() {
        let bridge = EditorBridge::new();
        assert!(bridge.selections("nope").is_err());
    }

    #[test]
    fn editor_ids_returns_all_editors() {
        let mut bridge = EditorBridge::new();
        bridge.add_editor("e1".into(), "file:///a.rs".into());
        bridge.add_editor("e2".into(), "file:///b.rs".into());
        let ids = bridge.editor_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"e1"));
        assert!(ids.contains(&"e2"));
    }

    #[test]
    fn snapshot_and_diff() {
        let mut bridge = EditorBridge::new();
        bridge.add_editor("e1".into(), "file:///a.rs".into());
        let snap1 = bridge.snapshot("e1").unwrap();
        assert_eq!(snap1.uri, "file:///a.rs");
        assert!(snap1.selections.is_empty());

        bridge.handle(EditorMessage::SetSelection {
            editor_id: "e1".into(),
            selections: vec![
                EditorSelection { anchor_line: 0, anchor_col: 0, active_line: 1, active_col: 5 },
            ],
        });
        let snap2 = bridge.snapshot("e1").unwrap();
        let diff = EditorBridge::diff_snapshots(&snap1, &snap2);
        assert!(!diff.uri_changed);
        assert_eq!(diff.selections_added, 1);
        assert_eq!(diff.selections_removed, 0);
    }

    #[test]
    fn snapshot_missing_editor() {
        let bridge = EditorBridge::new();
        assert!(bridge.snapshot("nope").is_err());
    }

    #[test]
    fn merge_selections_covers_both() {
        let a = EditorSelection { anchor_line: 1, anchor_col: 0, active_line: 3, active_col: 5 };
        let b = EditorSelection { anchor_line: 2, anchor_col: 3, active_line: 7, active_col: 10 };
        let merged = merge_selections(&a, &b);
        assert_eq!(merged.anchor_line, 1);
        assert_eq!(merged.anchor_col, 0);
        assert_eq!(merged.active_line, 7);
        assert_eq!(merged.active_col, 10);
    }

    #[test]
    fn decorations_conflict_detection() {
        let d1 = TextEditorDecoration {
            range: EditorRange { start_line: 1, start_col: 0, end_line: 5, end_col: 10 },
            hover_message: None,
            style: None,
        };
        let d2 = TextEditorDecoration {
            range: EditorRange { start_line: 3, start_col: 0, end_line: 8, end_col: 0 },
            hover_message: None,
            style: None,
        };
        let d3 = TextEditorDecoration {
            range: EditorRange { start_line: 10, start_col: 0, end_line: 12, end_col: 0 },
            hover_message: None,
            style: None,
        };
        assert!(decorations_conflict(&d1, &d2));
        assert!(!decorations_conflict(&d1, &d3));
    }

    #[test]
    fn resolve_decoration_conflicts_merges() {
        let decs = vec![
            TextEditorDecoration {
                range: EditorRange { start_line: 1, start_col: 0, end_line: 5, end_col: 0 },
                hover_message: Some("first".into()),
                style: None,
            },
            TextEditorDecoration {
                range: EditorRange { start_line: 3, start_col: 0, end_line: 8, end_col: 0 },
                hover_message: None,
                style: Some("bold".into()),
            },
            TextEditorDecoration {
                range: EditorRange { start_line: 20, start_col: 0, end_line: 25, end_col: 0 },
                hover_message: None,
                style: None,
            },
        ];
        let resolved = resolve_decoration_conflicts(&decs);
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].range.start_line, 1);
        assert_eq!(resolved[0].range.end_line, 8);
        assert_eq!(resolved[1].range.start_line, 20);
    }

    #[test]
    fn diff_snapshots_uri_changed() {
        let snap1 = EditorSnapshot {
            editor_id: "e1".into(),
            uri: "file:///old.rs".into(),
            selections: vec![],
        };
        let snap2 = EditorSnapshot {
            editor_id: "e1".into(),
            uri: "file:///new.rs".into(),
            selections: vec![],
        };
        let diff = EditorBridge::diff_snapshots(&snap1, &snap2);
        assert!(diff.uri_changed);
        assert_eq!(diff.old_uri, "file:///old.rs");
        assert_eq!(diff.new_uri, "file:///new.rs");
    }

    #[test]
    fn tab_state_new_defaults() {
        let tab = EditorTabState::new("file:///main.rs", 10, 5);
        assert_eq!(tab.uri, "file:///main.rs");
        assert_eq!(tab.cursor_line, 10);
        assert_eq!(tab.cursor_col, 5);
        assert_eq!(tab.scroll_top, 0);
        assert!(tab.view_state.is_none());
        assert!(!tab.is_pinned);
        assert!(!tab.is_preview);
        assert!(!tab.is_dirty);
    }

    #[test]
    fn tab_state_validate() {
        let valid = EditorTabState::new("file:///a.rs", 0, 0);
        assert!(valid.validate().is_ok());
        let invalid = EditorTabState::new("", 0, 0);
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn tab_state_display() {
        let mut tab = EditorTabState::new("file:///a.rs", 5, 3);
        let s = format!("{tab}");
        assert!(s.contains("file:///a.rs"));
        assert!(s.contains("5:3"));
        tab.is_pinned = true;
        tab.is_dirty = true;
        let s = format!("{tab}");
        assert!(s.contains("pinned"));
        assert!(s.contains("dirty"));
    }

    #[test]
    fn tab_serializer_add_and_active() {
        let mut ser = EditorTabSerializer::new();
        assert_eq!(ser.tab_count(), 0);
        assert!(ser.active_tab().is_none());
        let idx = ser.add_tab(EditorTabState::new("file:///a.rs", 0, 0));
        assert_eq!(idx, 0);
        assert_eq!(ser.active_tab_index, Some(0));
        ser.add_tab(EditorTabState::new("file:///b.rs", 1, 0));
        assert_eq!(ser.tab_count(), 2);
        // active still points to first
        assert_eq!(ser.active_tab().unwrap().uri, "file:///a.rs");
    }

    #[test]
    fn tab_serializer_remove_adjusts_active() {
        let mut ser = EditorTabSerializer::new();
        ser.add_tab(EditorTabState::new("file:///a.rs", 0, 0));
        ser.add_tab(EditorTabState::new("file:///b.rs", 0, 0));
        ser.add_tab(EditorTabState::new("file:///c.rs", 0, 0));
        ser.set_active(2);
        // Remove middle tab: active was 2, after removing index 1, active becomes 1
        ser.remove_tab(1);
        assert_eq!(ser.active_tab_index, Some(1));
        assert_eq!(ser.active_tab().unwrap().uri, "file:///c.rs");
    }

    #[test]
    fn tab_serializer_remove_active_tab() {
        let mut ser = EditorTabSerializer::new();
        ser.add_tab(EditorTabState::new("file:///a.rs", 0, 0));
        ser.add_tab(EditorTabState::new("file:///b.rs", 0, 0));
        ser.set_active(0);
        ser.remove_tab(0);
        // After removing active tab at index 0, active should be 0 (now "b.rs")
        assert_eq!(ser.active_tab().unwrap().uri, "file:///b.rs");
    }

    #[test]
    fn tab_serializer_serialize_deserialize_roundtrip() {
        let mut ser = EditorTabSerializer::new();
        let mut tab = EditorTabState::new("file:///main.rs", 42, 10);
        tab.scroll_top = 100;
        tab.view_state = Some("collapsed".to_string());
        tab.is_pinned = true;
        tab.is_dirty = true;
        ser.add_tab(tab);
        ser.add_tab(EditorTabState::new("file:///lib.rs", 0, 0));
        ser.set_active(1);

        let json = ser.serialize().unwrap();
        let restored = EditorTabSerializer::deserialize(&json).unwrap();
        assert_eq!(restored.tab_count(), 2);
        assert_eq!(restored.active_tab_index, Some(1));
        assert_eq!(restored.tabs[0].cursor_line, 42);
        assert_eq!(restored.tabs[0].scroll_top, 100);
        assert!(restored.tabs[0].is_pinned);
        assert!(restored.tabs[0].is_dirty);
        assert_eq!(restored.tabs[0].view_state.as_deref(), Some("collapsed"));
    }

    #[test]
    fn tab_serializer_find_and_counts() {
        let mut ser = EditorTabSerializer::new();
        let mut tab1 = EditorTabState::new("file:///a.rs", 0, 0);
        tab1.is_dirty = true;
        tab1.is_pinned = true;
        ser.add_tab(tab1);
        let mut tab2 = EditorTabState::new("file:///b.rs", 0, 0);
        tab2.is_dirty = true;
        ser.add_tab(tab2);
        ser.add_tab(EditorTabState::new("file:///c.rs", 0, 0));

        assert_eq!(ser.find_tab_by_uri("file:///b.rs"), Some(1));
        assert!(ser.find_tab_by_uri("file:///missing.rs").is_none());
        assert_eq!(ser.dirty_count(), 2);
        assert_eq!(ser.pinned_count(), 1);
    }

    #[test]
    fn tab_serializer_validate_all() {
        let mut ser = EditorTabSerializer::new();
        ser.add_tab(EditorTabState::new("file:///a.rs", 0, 0));
        assert!(ser.validate_all().is_ok());
        ser.add_tab(EditorTabState::new("", 0, 0));
        assert!(ser.validate_all().is_err());
    }

    #[test]
    fn tab_serializer_deserialize_invalid_json() {
        assert!(EditorTabSerializer::deserialize("not json").is_err());
    }

    #[test]
    fn selection_length_single_line() {
        let sel = EditorSelection { anchor_line: 3, anchor_col: 2, active_line: 3, active_col: 10 };
        assert_eq!(sel.length(), 8);
    }

    #[test]
    fn selection_is_empty_and_nonempty() {
        let empty = EditorSelection { anchor_line: 5, anchor_col: 3, active_line: 5, active_col: 3 };
        assert!(empty.is_empty());
        let nonempty = EditorSelection { anchor_line: 5, anchor_col: 3, active_line: 5, active_col: 4 };
        assert!(!nonempty.is_empty());
    }

    #[test]
    fn selection_contains_line_check() {
        let sel = EditorSelection { anchor_line: 3, anchor_col: 0, active_line: 7, active_col: 5 };
        assert!(sel.contains_line(3));
        assert!(sel.contains_line(5));
        assert!(sel.contains_line(7));
        assert!(!sel.contains_line(2));
        assert!(!sel.contains_line(8));
    }

    #[test]
    fn range_merge_and_overlaps() {
        let a = EditorRange { start_line: 1, start_col: 0, end_line: 5, end_col: 10 };
        let b = EditorRange { start_line: 3, start_col: 0, end_line: 8, end_col: 5 };
        assert!(a.overlaps(&b));
        let merged = a.merge(&b);
        assert_eq!(merged.start_line, 1);
        assert_eq!(merged.end_line, 8);
        assert_eq!(merged.end_col, 5);

        let c = EditorRange { start_line: 10, start_col: 0, end_line: 12, end_col: 0 };
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn range_contains_line_check() {
        let r = EditorRange { start_line: 5, start_col: 0, end_line: 10, end_col: 0 };
        assert!(r.contains_line(5));
        assert!(r.contains_line(7));
        assert!(r.contains_line(10));
        assert!(!r.contains_line(4));
        assert!(!r.contains_line(11));
    }

    #[test]
    fn selection_set_basic_operations() {
        let sels = vec![
            EditorSelection { anchor_line: 5, anchor_col: 0, active_line: 8, active_col: 0 },
            EditorSelection { anchor_line: 1, anchor_col: 0, active_line: 3, active_col: 0 },
        ];
        let set = EditorSelectionSet::new(sels);
        assert_eq!(set.len(), 2);
        assert!(!set.is_empty());
        assert_eq!(set.total_lines(), 7); // 4 + 3
        assert!(!set.has_overlaps());

        let sorted = set.sorted();
        assert_eq!(sorted.selections()[0].anchor_line, 1);
        assert_eq!(sorted.selections()[1].anchor_line, 5);

        let display = format!("{set}");
        assert!(display.contains("2 selections"));
    }

    #[test]
    fn selection_set_merge_overlapping() {
        let sels = vec![
            EditorSelection { anchor_line: 1, anchor_col: 0, active_line: 5, active_col: 0 },
            EditorSelection { anchor_line: 3, anchor_col: 0, active_line: 8, active_col: 0 },
            EditorSelection { anchor_line: 20, anchor_col: 0, active_line: 22, active_col: 0 },
        ];
        let set = EditorSelectionSet::new(sels);
        assert!(set.has_overlaps());
        let merged = set.merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged.selections()[0].anchor_line, 1);
        assert_eq!(merged.selections()[0].active_line, 8);
        assert_eq!(merged.selections()[1].anchor_line, 20);
    }

    #[test]
    fn selection_set_into_iter() {
        let sels = vec![
            EditorSelection { anchor_line: 1, anchor_col: 0, active_line: 2, active_col: 0 },
            EditorSelection { anchor_line: 5, anchor_col: 0, active_line: 6, active_col: 0 },
        ];
        let set = EditorSelectionSet::new(sels);
        let collected: Vec<_> = set.into_iter().collect();
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn snapshot_and_diff_display() {
        let snap = EditorSnapshot {
            editor_id: "e1".into(),
            uri: "file:///test.rs".into(),
            selections: vec![],
        };
        let s = format!("{snap}");
        assert!(s.contains("e1"));
        assert!(s.contains("file:///test.rs"));

        let diff = EditorStateDiff {
            uri_changed: true,
            selections_added: 2,
            selections_removed: 1,
            old_uri: "file:///old.rs".into(),
            new_uri: "file:///new.rs".into(),
        };
        let d = format!("{diff}");
        assert!(d.contains("uri_changed=true"));
        assert!(d.contains("+2/-1"));
    }
}
