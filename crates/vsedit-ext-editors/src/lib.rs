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


// ---------------------------------------------------------------------------
// EditorDecorationManager - manages decorations by type
// ---------------------------------------------------------------------------

/// Manages editor decorations organized by decoration type.
#[derive(Debug, Clone, Default)]
pub struct EditorDecorationManager {
    types: HashMap<String, DecorationStyle>,
    decorations: HashMap<String, Vec<TextEditorDecoration>>,
}

/// Style for a decoration type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorationStyle {
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub background_color: Option<String>,
    #[serde(default)]
    pub font_weight: Option<String>,
    #[serde(default)]
    pub font_style: Option<String>,
    #[serde(default)]
    pub border: Option<String>,
}

impl DecorationStyle {
    /// Create an empty style.
    pub fn new() -> Self {
        Self {
            color: None,
            background_color: None,
            font_weight: None,
            font_style: None,
            border: None,
        }
    }

    /// Set the foreground color.
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Set the background color.
    pub fn with_background(mut self, bg: impl Into<String>) -> Self {
        self.background_color = Some(bg.into());
        self
    }

    /// Set the font weight.
    pub fn with_font_weight(mut self, weight: impl Into<String>) -> Self {
        self.font_weight = Some(weight.into());
        self
    }

    /// Set the font style.
    pub fn with_font_style(mut self, style: impl Into<String>) -> Self {
        self.font_style = Some(style.into());
        self
    }

    /// Set the border.
    pub fn with_border(mut self, border: impl Into<String>) -> Self {
        self.border = Some(border.into());
        self
    }

    /// Returns true if no style properties are set.
    pub fn is_empty(&self) -> bool {
        self.color.is_none()
            && self.background_color.is_none()
            && self.font_weight.is_none()
            && self.font_style.is_none()
            && self.border.is_none()
    }
}

impl Default for DecorationStyle {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorDecorationManager {
    /// Create a new empty decoration manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new decoration type with a style.
    pub fn add_type(&mut self, type_name: impl Into<String>, style: DecorationStyle) {
        let name = type_name.into();
        self.types.insert(name.clone(), style);
        self.decorations.entry(name).or_default();
    }

    /// Remove a decoration type and all its decorations.
    pub fn remove_type(&mut self, type_name: &str) -> bool {
        self.decorations.remove(type_name);
        self.types.remove(type_name).is_some()
    }

    /// Set decorations for a given type. Replaces any existing decorations.
    pub fn set_decorations(&mut self, type_name: &str, decorations: Vec<TextEditorDecoration>) {
        if self.types.contains_key(type_name) {
            self.decorations.insert(type_name.to_string(), decorations);
        }
    }

    /// Get decorations for a given type.
    pub fn get_decorations(&self, type_name: &str) -> &[TextEditorDecoration] {
        self.decorations
            .get(type_name)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Clear all decoration types and decorations.
    pub fn clear_all(&mut self) {
        self.types.clear();
        self.decorations.clear();
    }

    /// Number of registered decoration types.
    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    /// Total number of decorations across all types.
    pub fn total_decoration_count(&self) -> usize {
        self.decorations.values().map(|v| v.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// EditorCommandDispatcher - dispatches named commands
// ---------------------------------------------------------------------------

/// Dispatches named editor commands to registered handlers.
#[derive(Debug, Clone, Default)]
pub struct EditorCommandDispatcher {
    commands: HashMap<String, EditorCommandEntry>,
}

/// A registered command.
#[derive(Debug, Clone)]
pub struct EditorCommandEntry {
    pub id: String,
    pub title: String,
    pub category: Option<String>,
    pub invocation_count: u64,
}

impl EditorCommandDispatcher {
    /// Create a new empty dispatcher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a command.
    pub fn register(&mut self, id: impl Into<String>, title: impl Into<String>, category: Option<String>) {
        let id = id.into();
        self.commands.insert(
            id.clone(),
            EditorCommandEntry {
                id,
                title: title.into(),
                category,
                invocation_count: 0,
            },
        );
    }

    /// Dispatch (invoke) a command by id. Returns true if the command existed.
    pub fn dispatch(&mut self, id: &str) -> bool {
        if let Some(entry) = self.commands.get_mut(id) {
            entry.invocation_count += 1;
            true
        } else {
            false
        }
    }

    /// List all registered command ids.
    pub fn list_commands(&self) -> Vec<&str> {
        self.commands.keys().map(|s| s.as_str()).collect()
    }

    /// Get a command entry by id.
    pub fn get_command(&self, id: &str) -> Option<&EditorCommandEntry> {
        self.commands.get(id)
    }

    /// Number of registered commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns true if no commands are registered.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

// ---------------------------------------------------------------------------
// EditorEditBatch - batches text edits
// ---------------------------------------------------------------------------

/// A batch of text edits to apply atomically.
#[derive(Debug, Clone, Default)]
pub struct EditorEditBatch {
    edits: Vec<EditorBatchEdit>,
}

/// A single text edit in a batch.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorBatchEdit {
    pub range: EditorRange,
    pub new_text: String,
}

impl EditorEditBatch {
    /// Create a new empty batch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an edit to the batch.
    pub fn add_edit(&mut self, range: EditorRange, new_text: impl Into<String>) {
        self.edits.push(EditorBatchEdit {
            range,
            new_text: new_text.into(),
        });
    }

    /// Apply all edits, returning them sorted by position (bottom-up for safe application).
    pub fn apply_all(&mut self) -> Vec<EditorBatchEdit> {
        self.edits.sort_by(|a, b| {
            (b.range.start_line, b.range.start_col).cmp(&(a.range.start_line, a.range.start_col))
        });
        std::mem::take(&mut self.edits)
    }

    /// Number of edits in the batch.
    pub fn len(&self) -> usize {
        self.edits.len()
    }

    /// Returns true if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Total characters of new text across all edits.
    pub fn total_new_chars(&self) -> usize {
        self.edits.iter().map(|e| e.new_text.len()).sum()
    }
}

impl fmt::Display for EditorEditBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EditBatch({} edits, {} chars)", self.len(), self.total_new_chars())
    }
}

// ---------------------------------------------------------------------------
// EditorGroupManager - manages split editor groups
// ---------------------------------------------------------------------------

/// Identifies an editor group's layout direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GroupDirection {
    Left,
    Right,
    Up,
    Down,
}

impl fmt::Display for GroupDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GroupDirection::Left => write!(f, "left"),
            GroupDirection::Right => write!(f, "right"),
            GroupDirection::Up => write!(f, "up"),
            GroupDirection::Down => write!(f, "down"),
        }
    }
}

/// A single editor group containing ordered editor tab URIs.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorGroup {
    pub id: String,
    pub tabs: Vec<String>,
    pub active_tab: Option<usize>,
    pub size_fraction: f64,
}

impl EditorGroup {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            tabs: Vec::new(),
            active_tab: None,
            size_fraction: 1.0,
        }
    }

    /// Open a URI in this group. If already present, activates it.
    /// Returns the tab index.
    pub fn open(&mut self, uri: impl Into<String>) -> usize {
        let uri = uri.into();
        if let Some(idx) = self.tabs.iter().position(|t| *t == uri) {
            self.active_tab = Some(idx);
            idx
        } else {
            let idx = self.tabs.len();
            self.tabs.push(uri);
            self.active_tab = Some(idx);
            idx
        }
    }

    /// Close a tab by index. Returns the closed URI if valid.
    pub fn close(&mut self, index: usize) -> Option<String> {
        if index >= self.tabs.len() {
            return None;
        }
        let uri = self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.active_tab = None;
        } else if let Some(active) = self.active_tab {
            if active == index {
                self.active_tab = Some(active.min(self.tabs.len() - 1));
            } else if active > index {
                self.active_tab = Some(active - 1);
            }
        }
        Some(uri)
    }

    /// Number of tabs in this group.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Whether the group is empty.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// The currently active URI, if any.
    pub fn active_uri(&self) -> Option<&str> {
        self.active_tab.and_then(|i| self.tabs.get(i).map(|s| s.as_str()))
    }
}

impl fmt::Display for EditorGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Group({}, {} tabs, size={:.0}%)",
            self.id,
            self.tabs.len(),
            self.size_fraction * 100.0
        )
    }
}

/// Manages multiple editor groups for split-view layouts.
#[derive(Debug, Clone, Default)]
pub struct EditorGroupManager {
    groups: Vec<EditorGroup>,
    active_group: Option<usize>,
}

impl EditorGroupManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new group. Returns its index.
    pub fn add_group(&mut self, id: impl Into<String>) -> usize {
        let idx = self.groups.len();
        self.groups.push(EditorGroup::new(id));
        if self.active_group.is_none() {
            self.active_group = Some(idx);
        }
        self.rebalance();
        idx
    }

    /// Remove a group by index. Returns the removed group if valid.
    pub fn remove_group(&mut self, index: usize) -> Option<EditorGroup> {
        if index >= self.groups.len() {
            return None;
        }
        let group = self.groups.remove(index);
        if self.groups.is_empty() {
            self.active_group = None;
        } else if let Some(active) = self.active_group {
            if active == index {
                self.active_group = Some(active.min(self.groups.len() - 1));
            } else if active > index {
                self.active_group = Some(active - 1);
            }
        }
        self.rebalance();
        Some(group)
    }

    /// Set the active group by index.
    pub fn set_active_group(&mut self, index: usize) -> bool {
        if index < self.groups.len() {
            self.active_group = Some(index);
            true
        } else {
            false
        }
    }

    /// Get the active group.
    pub fn active_group(&self) -> Option<&EditorGroup> {
        self.active_group.and_then(|i| self.groups.get(i))
    }

    /// Get a mutable reference to the active group.
    pub fn active_group_mut(&mut self) -> Option<&mut EditorGroup> {
        self.active_group.and_then(|i| self.groups.get_mut(i))
    }

    /// Get a group by index.
    pub fn group(&self, index: usize) -> Option<&EditorGroup> {
        self.groups.get(index)
    }

    /// Number of groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Total number of open tabs across all groups.
    pub fn total_tab_count(&self) -> usize {
        self.groups.iter().map(|g| g.tab_count()).sum()
    }

    /// Move a tab from one group to another. Returns true on success.
    pub fn move_tab(
        &mut self,
        from_group: usize,
        tab_index: usize,
        to_group: usize,
    ) -> bool {
        if from_group == to_group
            || from_group >= self.groups.len()
            || to_group >= self.groups.len()
        {
            return false;
        }
        // Take the URI out of the source group
        let uri = match self.groups[from_group].close(tab_index) {
            Some(u) => u,
            None => return false,
        };
        self.groups[to_group].open(uri);
        true
    }

    /// Rebalance group sizes to equal fractions.
    fn rebalance(&mut self) {
        let count = self.groups.len();
        if count > 0 {
            let fraction = 1.0 / count as f64;
            for g in &mut self.groups {
                g.size_fraction = fraction;
            }
        }
    }
}

impl fmt::Display for EditorGroupManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GroupManager({} groups, {} total tabs, active={:?})",
            self.groups.len(),
            self.total_tab_count(),
            self.active_group,
        )
    }
}

// ---------------------------------------------------------------------------
// EditorCloseOrder - MRU-based close ordering
// ---------------------------------------------------------------------------

/// Tracks most-recently-used order for editor close operations.
#[derive(Debug, Clone, Default)]
pub struct EditorCloseOrder {
    /// Editor IDs in MRU order (most recent first).
    order: Vec<String>,
}

impl EditorCloseOrder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that an editor was accessed (moves it to front).
    pub fn touch(&mut self, editor_id: impl Into<String>) {
        let id = editor_id.into();
        self.order.retain(|e| e != &id);
        self.order.insert(0, id);
    }

    /// Remove an editor from the tracking.
    pub fn remove(&mut self, editor_id: &str) {
        self.order.retain(|e| e != editor_id);
    }

    /// The most recently used editor ID.
    pub fn most_recent(&self) -> Option<&str> {
        self.order.first().map(|s| s.as_str())
    }

    /// The least recently used editor ID (next to close on LRU policy).
    pub fn least_recent(&self) -> Option<&str> {
        self.order.last().map(|s| s.as_str())
    }

    /// Return the close order as a slice (most recent first).
    pub fn as_slice(&self) -> &[String] {
        &self.order
    }

    /// Number of tracked editors.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Get the MRU rank of an editor (0 = most recent). None if not tracked.
    pub fn rank(&self, editor_id: &str) -> Option<usize> {
        self.order.iter().position(|e| e == editor_id)
    }
}

impl fmt::Display for EditorCloseOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CloseOrder({} editors)", self.order.len())
    }
}

// ---------------------------------------------------------------------------
// EditorTitleResolver - computes display titles from URIs
// ---------------------------------------------------------------------------

/// Resolves display titles for editor tabs from their URIs.
#[derive(Debug, Clone, Default)]
pub struct EditorTitleResolver {
    /// Custom overrides: URI -> display title
    overrides: HashMap<String, String>,
}

impl EditorTitleResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a custom title for a specific URI.
    pub fn set_override(&mut self, uri: impl Into<String>, title: impl Into<String>) {
        self.overrides.insert(uri.into(), title.into());
    }

    /// Remove a custom title override.
    pub fn remove_override(&mut self, uri: &str) -> bool {
        self.overrides.remove(uri).is_some()
    }

    /// Resolve the display title for a URI.
    /// Uses override if present, otherwise extracts the filename from the path.
    pub fn resolve(&self, uri: &str) -> String {
        if let Some(title) = self.overrides.get(uri) {
            return title.clone();
        }
        Self::filename_from_uri(uri)
    }

    /// Resolve a short title suitable for narrow tab bars.
    /// Truncates to `max_len` characters with ellipsis if needed.
    pub fn resolve_short(&self, uri: &str, max_len: usize) -> String {
        let title = self.resolve(uri);
        if title.len() <= max_len {
            title
        } else if max_len <= 3 {
            title[..max_len].to_string()
        } else {
            format!("{}...", &title[..max_len - 3])
        }
    }

    /// Resolve a disambiguated title when multiple editors share the same filename.
    /// Includes the parent directory to differentiate.
    pub fn resolve_disambiguated(&self, uri: &str) -> String {
        if let Some(title) = self.overrides.get(uri) {
            return title.clone();
        }
        let path = Self::path_from_uri(uri);
        let parts: Vec<&str> = path.rsplitn(3, '/').collect();
        if parts.len() >= 2 {
            format!("{}/{}", parts[1], parts[0])
        } else {
            Self::filename_from_uri(uri)
        }
    }

    /// Extract the filename portion from a URI.
    fn filename_from_uri(uri: &str) -> String {
        let path = Self::path_from_uri(uri);
        path.rsplit('/').next().unwrap_or(uri).to_string()
    }

    /// Strip the scheme from a URI to get the path portion.
    fn path_from_uri(uri: &str) -> &str {
        if let Some(idx) = uri.find("://") {
            &uri[idx + 3..]
        } else {
            uri
        }
    }

    /// Number of active overrides.
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }
}

impl fmt::Display for EditorTitleResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TitleResolver({} overrides)", self.overrides.len())
    }
}

// ---------------------------------------------------------------------------
// EditorIconResolver - resolves file type icons by extension
// ---------------------------------------------------------------------------

/// Resolves icon identifiers for editor tabs based on file extensions.
#[derive(Debug, Clone, Default)]
pub struct EditorIconResolver {
    /// Extension (without dot) -> icon identifier
    extension_map: HashMap<String, String>,
    /// Exact filename -> icon identifier
    filename_map: HashMap<String, String>,
    /// Default icon when no match is found
    default_icon: String,
}

impl EditorIconResolver {
    /// Create a resolver with sensible defaults for common file types.
    pub fn with_defaults() -> Self {
        let mut resolver = Self {
            extension_map: HashMap::new(),
            filename_map: HashMap::new(),
            default_icon: "file".to_string(),
        };
        // Common language icons
        resolver.add_extension("rs", "rust");
        resolver.add_extension("ts", "typescript");
        resolver.add_extension("tsx", "react");
        resolver.add_extension("js", "javascript");
        resolver.add_extension("jsx", "react");
        resolver.add_extension("py", "python");
        resolver.add_extension("go", "go");
        resolver.add_extension("java", "java");
        resolver.add_extension("c", "c");
        resolver.add_extension("cpp", "cpp");
        resolver.add_extension("h", "c-header");
        resolver.add_extension("md", "markdown");
        resolver.add_extension("json", "json");
        resolver.add_extension("toml", "toml");
        resolver.add_extension("yaml", "yaml");
        resolver.add_extension("yml", "yaml");
        resolver.add_extension("html", "html");
        resolver.add_extension("css", "css");
        // Common filenames
        resolver.add_filename("Cargo.toml", "cargo");
        resolver.add_filename("Cargo.lock", "cargo");
        resolver.add_filename("package.json", "npm");
        resolver.add_filename("Makefile", "makefile");
        resolver.add_filename("Dockerfile", "docker");
        resolver
    }

    /// Register an icon for a file extension.
    pub fn add_extension(&mut self, ext: impl Into<String>, icon: impl Into<String>) {
        self.extension_map.insert(ext.into(), icon.into());
    }

    /// Register an icon for an exact filename.
    pub fn add_filename(&mut self, name: impl Into<String>, icon: impl Into<String>) {
        self.filename_map.insert(name.into(), icon.into());
    }

    /// Set the default icon for unrecognized files.
    pub fn set_default_icon(&mut self, icon: impl Into<String>) {
        self.default_icon = icon.into();
    }

    /// Resolve the icon identifier for a URI.
    /// Priority: exact filename match > extension match > default.
    pub fn resolve(&self, uri: &str) -> &str {
        let filename = uri.rsplit('/').next().unwrap_or(uri);
        if let Some(icon) = self.filename_map.get(filename) {
            return icon.as_str();
        }
        if let Some(dot_pos) = filename.rfind('.') {
            let ext = &filename[dot_pos + 1..];
            if let Some(icon) = self.extension_map.get(ext) {
                return icon.as_str();
            }
        }
        &self.default_icon
    }

    /// Number of registered extension mappings.
    pub fn extension_count(&self) -> usize {
        self.extension_map.len()
    }

    /// Number of registered filename mappings.
    pub fn filename_count(&self) -> usize {
        self.filename_map.len()
    }
}

impl fmt::Display for EditorIconResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IconResolver({} ext, {} filename, default={})",
            self.extension_map.len(),
            self.filename_map.len(),
            self.default_icon,
        )
    }
}

// ---------------------------------------------------------------------------
// EditorTabSerializer extensions for pinning and preview
// ---------------------------------------------------------------------------

impl EditorTabSerializer {
    /// Pin a tab by index. Returns false if index is out of bounds.
    pub fn pin_tab(&mut self, index: usize) -> bool {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.is_pinned = true;
            tab.is_preview = false; // pinned tabs cannot be previews
            true
        } else {
            false
        }
    }

    /// Unpin a tab by index.
    pub fn unpin_tab(&mut self, index: usize) -> bool {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.is_pinned = false;
            true
        } else {
            false
        }
    }

    /// Promote a preview tab to a normal (non-preview) tab.
    /// Returns false if index is out of bounds or tab is not a preview.
    pub fn promote_preview(&mut self, index: usize) -> bool {
        if let Some(tab) = self.tabs.get_mut(index) {
            if tab.is_preview {
                tab.is_preview = false;
                return true;
            }
        }
        false
    }

    /// Find the current preview tab index, if any.
    /// By convention, at most one tab should be in preview mode.
    pub fn preview_tab_index(&self) -> Option<usize> {
        self.tabs.iter().position(|t| t.is_preview)
    }

    /// Open a URI in preview mode. If a preview tab already exists, it is
    /// replaced. Returns the index of the preview tab.
    pub fn open_preview(&mut self, uri: impl Into<String>, cursor_line: u32, cursor_col: u32) -> usize {
        let uri = uri.into();
        // Replace existing preview tab if present
        if let Some(idx) = self.preview_tab_index() {
            self.tabs[idx].uri = uri;
            self.tabs[idx].cursor_line = cursor_line;
            self.tabs[idx].cursor_col = cursor_col;
            self.tabs[idx].scroll_top = 0;
            self.tabs[idx].is_dirty = false;
            self.active_tab_index = Some(idx);
            idx
        } else {
            let mut tab = EditorTabState::new(uri, cursor_line, cursor_col);
            tab.is_preview = true;
            let idx = self.tabs.len();
            self.tabs.push(tab);
            self.active_tab_index = Some(idx);
            idx
        }
    }

    /// Move all pinned tabs to the front, preserving relative order.
    pub fn sort_pinned_first(&mut self) {
        let active_uri = self.active_tab().map(|t| t.uri.clone());
        self.tabs.sort_by_key(|t| !t.is_pinned);
        // Restore active_tab_index based on URI
        if let Some(uri) = active_uri {
            self.active_tab_index = self.find_tab_by_uri(&uri);
        }
    }

    /// Close all tabs that are not pinned and not dirty. Returns count closed.
    pub fn close_saved_unpinned(&mut self) -> usize {
        let before = self.tabs.len();
        self.tabs.retain(|t| t.is_pinned || t.is_dirty);
        let closed = before - self.tabs.len();
        if closed > 0 {
            // Fix active index
            if self.tabs.is_empty() {
                self.active_tab_index = None;
            } else if let Some(active) = self.active_tab_index {
                if active >= self.tabs.len() {
                    self.active_tab_index = Some(self.tabs.len() - 1);
                }
            }
        }
        closed
    }

    /// Return URIs of all dirty tabs.
    pub fn dirty_uris(&self) -> Vec<&str> {
        self.tabs.iter().filter(|t| t.is_dirty).map(|t| t.uri.as_str()).collect()
    }

    /// Return URIs of all pinned tabs.
    pub fn pinned_uris(&self) -> Vec<&str> {
        self.tabs.iter().filter(|t| t.is_pinned).map(|t| t.uri.as_str()).collect()
    }
}


// ---------------------------------------------------------------------------
// ext_editors – Extension protocol helpers
// ---------------------------------------------------------------------------

/// Activation event kinds for extension lifecycle management.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum XExtEditorsActivationKind {
    /// Activate on a specific language.
    Language(String),
    /// Activate on a command.
    Command(String),
    /// Activate on a workspace-contains glob.
    WorkspaceContains(String),
    /// Activate on a custom URI scheme.
    UriScheme(String),
    /// Activate on startup.
    Star,
}

impl XExtEditorsActivationKind {
    /// Parse an activation event string like `"onLanguage:rust"`.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw == "*" {
            return Some(Self::Star);
        }
        let (kind, value) = raw.split_once(':')?;
        match kind {
            "onLanguage" => Some(Self::Language(value.to_string())),
            "onCommand" => Some(Self::Command(value.to_string())),
            "workspaceContains" => Some(Self::WorkspaceContains(value.to_string())),
            "onUri" => Some(Self::UriScheme(value.to_string())),
            _ => None,
        }
    }

    /// Returns true if this activation kind targets a specific language.
    pub fn is_language(&self) -> bool {
        matches!(self, Self::Language(_))
    }
}

/// Message envelope for extension host RPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XExtEditorsRpcEnvelope {
    pub seq: u64,
    pub method: String,
    pub payload: String,
}

impl XExtEditorsRpcEnvelope {
    /// Create a new RPC envelope.
    pub fn new(seq: u64, method: impl Into<String>, payload: impl Into<String>) -> Self {
        Self { seq, method: method.into(), payload: payload.into() }
    }

    /// Returns true when the envelope carries a response (method starts with `$/`).
    pub fn is_response(&self) -> bool {
        self.method.starts_with("$/")
    }

    /// Compute a simple checksum of the payload (sum of bytes mod 2^32).
    pub fn payload_checksum(&self) -> u32 {
        self.payload.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32))
    }
}

/// Batch multiple RPC envelopes and return their sequence numbers.
pub fn x_ext_editors_collect_sequences(envelopes: &[XExtEditorsRpcEnvelope]) -> Vec<u64> {
    envelopes.iter().map(|e| e.seq).collect()
}

/// Filter envelopes by method prefix.
pub fn x_ext_editors_filter_by_method<'a>(
    envelopes: &'a [XExtEditorsRpcEnvelope],
    method_prefix: &str,
) -> Vec<&'a XExtEditorsRpcEnvelope> {
    envelopes.iter().filter(|e| e.method.starts_with(method_prefix)).collect()
}

/// Deduplicate envelopes by sequence number, keeping the first occurrence.
pub fn x_ext_editors_dedup_by_seq(envelopes: Vec<XExtEditorsRpcEnvelope>) -> Vec<XExtEditorsRpcEnvelope> {
    let mut seen = std::collections::HashSet::new();
    envelopes.into_iter().filter(|e| seen.insert(e.seq)).collect()
}

/// Simple capability negotiation: given requested and available feature sets,
/// return the intersection.
pub fn x_ext_editors_negotiate_capabilities(
    requested: &[&str],
    available: &[&str],
) -> Vec<String> {
    requested.iter()
        .filter(|r| available.contains(r))
        .map(|s| s.to_string())
        .collect()
}

/// Version tuple for extension API compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct XExtEditorsApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl XExtEditorsApiVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
    /// Check if this version satisfies a minimum requirement.
    pub fn satisfies(&self, min: &Self) -> bool {
        (self.major, self.minor, self.patch) >= (min.major, min.minor, min.patch)
    }
}

impl std::fmt::Display for XExtEditorsApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}


/// Configuration manager for ext_editors functionality.
pub struct ExtEditorsConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ExtEditorsConfig {
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

    pub fn merge(&mut self, other: &ExtEditorsConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for ext_editors operations.
pub struct ExtEditorsRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ExtEditorsRateTracker {
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

/// Validation result collector for ext_editors.
pub struct ExtEditorsValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ExtEditorsValidator {
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

    pub fn merge(&mut self, other: &ExtEditorsValidator) {
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
// xa_ extended helpers for ext_editors
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtEditorsRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtEditorsRingBuf {
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
pub struct XaExtEditorsCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtEditorsCounter {
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

impl Default for XaExtEditorsCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 58
// ---------------------------------------------------------------------------

/// Generic object pool `Xc58Pool<T>`.
pub struct Xc58Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc58Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc58PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc58Pool<T> {
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
    pub fn stats(&self) -> Xc58PoolStats {
        Xc58PoolStats {
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

impl<T> Default for Xc58Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc58Scheduler`.
pub struct Xc58Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc58Scheduler {
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

impl Default for Xc58Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_58 hash for the given byte slice.
pub fn xc_58_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_58 convention.
pub fn xc_58_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_93 deepening: state machine + event bus ---

/// States for the Xd93 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd93State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd93State {
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
pub struct Xd93Transition {
    pub from: Xd93State,
    pub to: Xd93State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd93StateMachine {
    current: Xd93State,
    history: Vec<Xd93Transition>,
    step_counter: usize,
}

impl Xd93StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd93State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd93State {
        self.current
    }

    pub fn history(&self) -> &[Xd93Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd93State) -> Result<Xd93State, String> {
        let allowed = match (self.current, target) {
            (Xd93State::Idle, Xd93State::Running) => true,
            (Xd93State::Running, Xd93State::Paused) => true,
            (Xd93State::Running, Xd93State::Done) => true,
            (Xd93State::Paused, Xd93State::Running) => true,
            (Xd93State::Paused, Xd93State::Done) => true,
            (Xd93State::Done, Xd93State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_93: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd93Transition {
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
            "Xd93SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd93State> {
        let prefix = "Xd93SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd93State::Idle),
            "Running" => Some(Xd93State::Running),
            "Paused" => Some(Xd93State::Paused),
            "Done" => Some(Xd93State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd93State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd93 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd93Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd93Event {
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

type Xd93HandlerFn = Box<dyn Fn(&Xd93Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd93EventBus {
    handlers: Vec<(usize, Option<String>, Xd93HandlerFn)>,
    next_id: usize,
    published: Vec<Xd93Event>,
}

impl Xd93EventBus {
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
        F: Fn(&Xd93Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd93Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd93Event) {
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

    pub fn published_events(&self) -> &[Xd93Event] {
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
// xg_16: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg16Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg16Graph {
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

impl Default for Xg16Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_16: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg16Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg16Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg16Heap<T>) {
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

impl<T: Ord> Default for Xg16Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 57).
pub struct Xh57SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh57SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 99 as u64,
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

/// A compact bit set supporting boolean operations (variant 57).
pub struct Xh57BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh57BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 57).
pub struct Xi57Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi57Deque<T> {
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
pub struct Xi57Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi57Interval {
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

/// A simple interval tree (variant 57).
pub struct Xi57IntervalTree {
    xi_intervals: Vec<Xi57Interval>,
}

impl Xi57IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi57Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi57Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi57Interval) -> Vec<&Xi57Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi57Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi57Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi57Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi57Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi57Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi57Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 57) ---

/// Disjoint set / union-find for crate 57.
pub struct Xj57UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj57UnionFind {
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

const XJ57_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 57.
pub struct Xj57BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj57BTreeNode<K, V>>>,
    len: usize,
}

struct Xj57BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj57BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj57BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ57_BTREE_ORDER - 1
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
        let mid = XJ57_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj57BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj57BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj57BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj57BTreeNode::xj_new_leaf();
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


// --- xk_57 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk57SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk57SegmentTree {
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
pub struct Xk57DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk57DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_57).
#[derive(Debug, Clone)]
pub struct Xl57Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl57Rope {
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

/// Suffix array for efficient string searching (xl_57).
#[derive(Debug, Clone)]
pub struct Xl57SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl57SuffixArray {
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
pub struct Xm57MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm57MatrixSparse {
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
pub struct Xm57Tokenizer {
    text: String,
}

impl Xm57Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 57.
pub struct Xn57Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn57Fenwick {
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

// ----- AVL tree map — crate 57 -----

#[derive(Debug, Clone)]
struct Xn57AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn57AvlNode<K, V>>>,
    right: Option<Box<Xn57AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 57.
#[derive(Debug, Clone)]
pub struct Xn57AVL<K, V> {
    root: Option<Box<Xn57AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn57AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn57AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn57AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn57AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn57AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn57AvlNode<K, V>>) -> Box<Xn57AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn57AvlNode<K, V>>) -> Box<Xn57AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn57AvlNode<K, V>>) -> Box<Xn57AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn57AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn57AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn57AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn57AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn57AvlNode<K, V>>) -> &Xn57AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn57AvlNode<K, V>>) -> (Box<Xn57AvlNode<K, V>>, Option<Box<Xn57AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn57AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn57AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn57AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn57AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn57AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn57AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn57AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo57RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo57Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo57RBNode<K, V> {
    key: K,
    value: V,
    color: Xo57Color,
    left: Option<Box<Xo57RBNode<K, V>>>,
    right: Option<Box<Xo57RBNode<K, V>>>,
}

/// A red-black tree map for crate 57.
#[derive(Debug, Clone)]
pub struct Xo57RedBlack<K, V> {
    root: Option<Box<Xo57RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo57RedBlack<K, V> {
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
            r.color = Xo57Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo57RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo57RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo57RBNode {
                    key, value, color: Xo57Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo57RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo57Color::Red)
    }

    fn xo_balance(mut h: Box<Xo57RBNode<K, V>>) -> Box<Xo57RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo57Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo57RBNode<K, V>>) -> Box<Xo57RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo57Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo57RBNode<K, V>>) -> Box<Xo57RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo57Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo57RBNode<K, V>>) {
        h.color = Xo57Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo57Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo57Color::Black; }
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
            r.color = Xo57Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo57RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo57RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo57RBNode<K, V>) -> (K, V, Option<Box<Xo57RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo57RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo57Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo57RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo57ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 57.
#[derive(Debug, Clone)]
pub struct Xo57ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo57ConsistentHash {
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
            let vkey = format!("{}#xo57#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo57#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 57).
#[derive(Debug)]
pub struct Xp57SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp57Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp57Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp57Node<K, V>>>,
    xp_right: Option<Box<Xp57Node<K, V>>>,
}

impl<K: Ord, V> Xp57Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp57SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp57SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp57Node<K, V>>>, key: &K) -> Option<Box<Xp57Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp57Node<K, V>>) -> Box<Xp57Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp57Node<K, V>>) -> Box<Xp57Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp57Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp57Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp57Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
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

    #[test]
    fn decoration_style_builder() {
        let style = DecorationStyle::new()
            .with_color("red")
            .with_background("yellow")
            .with_font_weight("bold");
        assert!(!style.is_empty());
        assert_eq!(style.color, Some("red".to_string()));
        assert_eq!(style.background_color, Some("yellow".to_string()));
        assert_eq!(style.font_weight, Some("bold".to_string()));
        assert!(style.font_style.is_none());
    }

    #[test]
    fn decoration_manager_operations() {
        let mut mgr = EditorDecorationManager::new();
        mgr.add_type("highlight", DecorationStyle::new().with_color("blue"));
        mgr.add_type("error", DecorationStyle::new().with_color("red"));
        assert_eq!(mgr.type_count(), 2);

        let decs = vec![TextEditorDecoration {
            range: EditorRange { start_line: 1, start_col: 1, end_line: 1, end_col: 5 },
            hover_message: Some("test".into()),
            style: None,
        }];
        mgr.set_decorations("highlight", decs);
        assert_eq!(mgr.get_decorations("highlight").len(), 1);
        assert_eq!(mgr.total_decoration_count(), 1);

        mgr.remove_type("highlight");
        assert_eq!(mgr.type_count(), 1);
        mgr.clear_all();
        assert_eq!(mgr.type_count(), 0);
    }

    #[test]
    fn command_dispatcher_register_dispatch() {
        let mut disp = EditorCommandDispatcher::new();
        disp.register("editor.save", "Save File", Some("File".into()));
        disp.register("editor.copy", "Copy", Some("Edit".into()));
        assert_eq!(disp.len(), 2);

        assert!(disp.dispatch("editor.save"));
        assert!(disp.dispatch("editor.save"));
        assert!(!disp.dispatch("unknown.cmd"));
        let cmd = disp.get_command("editor.save").unwrap();
        assert_eq!(cmd.invocation_count, 2);
    }

    #[test]
    fn command_dispatcher_list() {
        let mut disp = EditorCommandDispatcher::new();
        disp.register("a", "A", None);
        disp.register("b", "B", None);
        let mut cmds = disp.list_commands();
        cmds.sort();
        assert_eq!(cmds, vec!["a", "b"]);
    }

    #[test]
    fn edit_batch_operations() {
        let mut batch = EditorEditBatch::new();
        assert!(batch.is_empty());
        batch.add_edit(
            EditorRange { start_line: 1, start_col: 1, end_line: 1, end_col: 5 },
            "hello",
        );
        batch.add_edit(
            EditorRange { start_line: 3, start_col: 1, end_line: 3, end_col: 3 },
            "world",
        );
        assert_eq!(batch.len(), 2);
        assert_eq!(batch.total_new_chars(), 10);
        let s = format!("{batch}");
        assert!(s.contains("2 edits"));
    }

    #[test]
    fn edit_batch_apply_sorted() {
        let mut batch = EditorEditBatch::new();
        batch.add_edit(
            EditorRange { start_line: 1, start_col: 1, end_line: 1, end_col: 1 },
            "first",
        );
        batch.add_edit(
            EditorRange { start_line: 5, start_col: 1, end_line: 5, end_col: 1 },
            "second",
        );
        let edits = batch.apply_all();
        // Should be sorted bottom-up (line 5 first)
        assert_eq!(edits[0].range.start_line, 5);
        assert_eq!(edits[1].range.start_line, 1);
        assert!(batch.is_empty());
    }

    // ── Editor group management tests ──

    #[test]
    fn editor_group_open_close() {
        let mut group = EditorGroup::new("g1");
        assert!(group.is_empty());

        let idx = group.open("file:///a.rs");
        assert_eq!(idx, 0);
        assert_eq!(group.tab_count(), 1);
        assert_eq!(group.active_uri(), Some("file:///a.rs"));

        group.open("file:///b.rs");
        assert_eq!(group.tab_count(), 2);
        assert_eq!(group.active_uri(), Some("file:///b.rs"));

        // Opening same URI again just activates it
        let idx2 = group.open("file:///a.rs");
        assert_eq!(idx2, 0);
        assert_eq!(group.tab_count(), 2);
        assert_eq!(group.active_uri(), Some("file:///a.rs"));

        let closed = group.close(0).unwrap();
        assert_eq!(closed, "file:///a.rs");
        assert_eq!(group.tab_count(), 1);
        assert_eq!(group.active_uri(), Some("file:///b.rs"));

        let s = format!("{group}");
        assert!(s.contains("g1"));
    }

    #[test]
    fn editor_group_manager_split_and_move() {
        let mut mgr = EditorGroupManager::new();
        assert_eq!(mgr.group_count(), 0);

        let g0 = mgr.add_group("left");
        let g1 = mgr.add_group("right");
        assert_eq!(mgr.group_count(), 2);

        // Open files in group 0
        mgr.active_group_mut().unwrap().open("file:///a.rs");
        mgr.active_group_mut().unwrap().open("file:///b.rs");
        assert_eq!(mgr.total_tab_count(), 2);

        // Move tab from group 0 to group 1
        assert!(mgr.move_tab(g0, 0, g1));
        assert_eq!(mgr.group(g0).unwrap().tab_count(), 1);
        assert_eq!(mgr.group(g1).unwrap().tab_count(), 1);
        assert_eq!(mgr.group(g1).unwrap().active_uri(), Some("file:///a.rs"));

        // Can't move to same group
        assert!(!mgr.move_tab(g0, 0, g0));

        // Remove a group
        let removed = mgr.remove_group(g1).unwrap();
        assert_eq!(removed.tabs.len(), 1);
        assert_eq!(mgr.group_count(), 1);

        let s = format!("{mgr}");
        assert!(s.contains("1 groups"));
    }

    #[test]
    fn editor_group_manager_active_tracking() {
        let mut mgr = EditorGroupManager::new();
        mgr.add_group("a");
        mgr.add_group("b");
        mgr.add_group("c");

        assert!(mgr.set_active_group(2));
        assert_eq!(mgr.active_group().unwrap().id, "c");

        assert!(!mgr.set_active_group(5)); // out of bounds

        // Remove active group
        mgr.remove_group(2);
        assert_eq!(mgr.active_group().unwrap().id, "b");
    }

    // ── Close order tests ──

    #[test]
    fn close_order_mru_tracking() {
        let mut order = EditorCloseOrder::new();
        assert!(order.is_empty());

        order.touch("e1");
        order.touch("e2");
        order.touch("e3");
        assert_eq!(order.len(), 3);
        assert_eq!(order.most_recent(), Some("e3"));
        assert_eq!(order.least_recent(), Some("e1"));
        assert_eq!(order.rank("e3"), Some(0));
        assert_eq!(order.rank("e1"), Some(2));

        // Touching e1 moves it to front
        order.touch("e1");
        assert_eq!(order.most_recent(), Some("e1"));
        assert_eq!(order.least_recent(), Some("e2"));

        order.remove("e3");
        assert_eq!(order.len(), 2);
        assert!(order.rank("e3").is_none());

        let s = format!("{order}");
        assert!(s.contains("2 editors"));
    }

    // ── Title resolver tests ──

    #[test]
    fn title_resolver_filename_extraction() {
        let resolver = EditorTitleResolver::new();
        assert_eq!(resolver.resolve("file:///home/user/src/main.rs"), "main.rs");
        assert_eq!(resolver.resolve("file:///a.txt"), "a.txt");
        assert_eq!(resolver.resolve("untitled"), "untitled");
    }

    #[test]
    fn title_resolver_overrides_and_disambiguation() {
        let mut resolver = EditorTitleResolver::new();
        resolver.set_override("file:///special", "My Special File");
        assert_eq!(resolver.resolve("file:///special"), "My Special File");
        assert_eq!(resolver.override_count(), 1);

        resolver.remove_override("file:///special");
        assert_eq!(resolver.override_count(), 0);

        // Disambiguated includes parent dir
        let title = resolver.resolve_disambiguated("file:///src/utils/helpers.rs");
        assert_eq!(title, "utils/helpers.rs");

        // Short title truncation
        let short = resolver.resolve_short("file:///very_long_filename.rs", 10);
        assert_eq!(short, "very_lo...");
        let exact = resolver.resolve_short("file:///a.rs", 4);
        assert_eq!(exact, "a.rs");

        let s = format!("{resolver}");
        assert!(s.contains("0 overrides"));
    }

    // ── Icon resolver tests ──

    #[test]
    fn icon_resolver_defaults() {
        let resolver = EditorIconResolver::with_defaults();
        assert_eq!(resolver.resolve("file:///src/main.rs"), "rust");
        assert_eq!(resolver.resolve("file:///index.ts"), "typescript");
        assert_eq!(resolver.resolve("file:///app.jsx"), "react");
        assert_eq!(resolver.resolve("file:///Cargo.toml"), "cargo");
        assert_eq!(resolver.resolve("file:///Dockerfile"), "docker");
        assert_eq!(resolver.resolve("file:///unknown.xyz"), "file");
        assert!(resolver.extension_count() > 10);
        assert!(resolver.filename_count() > 0);
    }

    #[test]
    fn icon_resolver_custom() {
        let mut resolver = EditorIconResolver::with_defaults();
        resolver.add_extension("vue", "vue");
        resolver.set_default_icon("document");
        assert_eq!(resolver.resolve("file:///app.vue"), "vue");
        assert_eq!(resolver.resolve("file:///noext"), "document");

        let s = format!("{resolver}");
        assert!(s.contains("ext"));
    }

    // ── Tab pinning/preview tests ──

    #[test]
    fn tab_serializer_pin_unpin() {
        let mut ser = EditorTabSerializer::new();
        ser.add_tab(EditorTabState::new("file:///a.rs", 0, 0));
        ser.add_tab(EditorTabState::new("file:///b.rs", 0, 0));

        assert!(ser.pin_tab(0));
        assert!(ser.tabs[0].is_pinned);
        assert!(!ser.tabs[1].is_pinned);
        assert_eq!(ser.pinned_count(), 1);

        assert!(ser.unpin_tab(0));
        assert!(!ser.tabs[0].is_pinned);

        // Out of bounds
        assert!(!ser.pin_tab(5));
        assert!(!ser.unpin_tab(5));
    }

    #[test]
    fn tab_serializer_preview_lifecycle() {
        let mut ser = EditorTabSerializer::new();
        ser.add_tab(EditorTabState::new("file:///a.rs", 0, 0));

        // Open preview
        let idx = ser.open_preview("file:///preview.rs", 10, 5);
        assert_eq!(ser.tab_count(), 2);
        assert!(ser.tabs[idx].is_preview);
        assert_eq!(ser.active_tab_index, Some(idx));

        // Opening another preview replaces the existing one
        let idx2 = ser.open_preview("file:///preview2.rs", 20, 0);
        assert_eq!(idx2, idx); // same slot reused
        assert_eq!(ser.tab_count(), 2);
        assert_eq!(ser.tabs[idx].uri, "file:///preview2.rs");

        // Promote preview to normal
        assert!(ser.promote_preview(idx));
        assert!(!ser.tabs[idx].is_preview);
        assert!(ser.preview_tab_index().is_none());

        // Can't promote non-preview
        assert!(!ser.promote_preview(0));
    }

    #[test]
    fn tab_serializer_sort_pinned_first() {
        let mut ser = EditorTabSerializer::new();
        ser.add_tab(EditorTabState::new("file:///a.rs", 0, 0));
        ser.add_tab(EditorTabState::new("file:///b.rs", 0, 0));
        ser.add_tab(EditorTabState::new("file:///c.rs", 0, 0));
        ser.set_active(2);
        ser.pin_tab(2); // pin c.rs

        ser.sort_pinned_first();
        assert_eq!(ser.tabs[0].uri, "file:///c.rs");
        assert!(ser.tabs[0].is_pinned);
        // Active should still track c.rs
        assert_eq!(ser.active_tab().unwrap().uri, "file:///c.rs");
    }

    #[test]
    fn tab_serializer_close_saved_unpinned() {
        let mut ser = EditorTabSerializer::new();
        let mut dirty = EditorTabState::new("file:///dirty.rs", 0, 0);
        dirty.is_dirty = true;
        ser.add_tab(dirty);
        ser.add_tab(EditorTabState::new("file:///clean.rs", 0, 0));
        ser.pin_tab(0); // pin the dirty one too

        let mut pinned_clean = EditorTabState::new("file:///pinned.rs", 0, 0);
        pinned_clean.is_pinned = true;
        ser.add_tab(pinned_clean);
        ser.add_tab(EditorTabState::new("file:///other_clean.rs", 0, 0));

        let closed = ser.close_saved_unpinned();
        assert_eq!(closed, 2); // clean.rs and other_clean.rs
        assert_eq!(ser.tab_count(), 2);

        let dirty_uris = ser.dirty_uris();
        assert!(dirty_uris.contains(&"file:///dirty.rs"));
        let pinned_uris = ser.pinned_uris();
        assert!(pinned_uris.contains(&"file:///dirty.rs"));
        assert!(pinned_uris.contains(&"file:///pinned.rs"));
    }

    #[test]
    fn group_direction_display() {
        assert_eq!(format!("{}", GroupDirection::Left), "left");
        assert_eq!(format!("{}", GroupDirection::Right), "right");
        assert_eq!(format!("{}", GroupDirection::Up), "up");
        assert_eq!(format!("{}", GroupDirection::Down), "down");

        // Serde round-trip
        let json = serde_json::to_string(&GroupDirection::Right).unwrap();
        let parsed: GroupDirection = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, GroupDirection::Right);
    }

    // -- ext_editors additional tests -------------------------------------------

    #[test]
    fn x_ext_editors_activation_parse_language() {
        let ak = XExtEditorsActivationKind::parse("onLanguage:rust").unwrap();
        assert_eq!(ak, XExtEditorsActivationKind::Language("rust".into()));
        assert!(ak.is_language());
    }

    #[test]
    fn x_ext_editors_activation_parse_command() {
        let ak = XExtEditorsActivationKind::parse("onCommand:editor.action.format").unwrap();
        assert_eq!(ak, XExtEditorsActivationKind::Command("editor.action.format".into()));
        assert!(!ak.is_language());
    }

    #[test]
    fn x_ext_editors_activation_parse_star() {
        assert_eq!(XExtEditorsActivationKind::parse("*"), Some(XExtEditorsActivationKind::Star));
    }

    #[test]
    fn x_ext_editors_activation_parse_unknown() {
        assert!(XExtEditorsActivationKind::parse("badKind:thing").is_none());
    }

    #[test]
    fn x_ext_editors_activation_parse_workspace() {
        let ak = XExtEditorsActivationKind::parse("workspaceContains:**/Cargo.toml").unwrap();
        assert_eq!(ak, XExtEditorsActivationKind::WorkspaceContains("**/" .to_owned() + "Cargo.toml"));
    }

    #[test]
    fn x_ext_editors_rpc_envelope_basic() {
        let env = XExtEditorsRpcEnvelope::new(1, "textDocument/didOpen", "{}" );
        assert_eq!(env.seq, 1);
        assert!(!env.is_response());
    }

    #[test]
    fn x_ext_editors_rpc_envelope_response() {
        let env = XExtEditorsRpcEnvelope::new(2, "$/cancelRequest", "");
        assert!(env.is_response());
    }

    #[test]
    fn x_ext_editors_rpc_payload_checksum() {
        let env = XExtEditorsRpcEnvelope::new(1, "m", "AB");
        assert_eq!(env.payload_checksum(), 65 + 66);
    }

    #[test]
    fn x_ext_editors_collect_sequences_works() {
        let envs = vec![
            XExtEditorsRpcEnvelope::new(10, "a", ""),
            XExtEditorsRpcEnvelope::new(20, "b", ""),
        ];
        assert_eq!(x_ext_editors_collect_sequences(&envs), vec![10, 20]);
    }

    #[test]
    fn x_ext_editors_filter_by_method_works() {
        let envs = vec![
            XExtEditorsRpcEnvelope::new(1, "textDocument/open", ""),
            XExtEditorsRpcEnvelope::new(2, "workspace/config", ""),
            XExtEditorsRpcEnvelope::new(3, "textDocument/close", ""),
        ];
        let filtered = x_ext_editors_filter_by_method(&envs, "textDocument/");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_ext_editors_dedup_by_seq_works() {
        let envs = vec![
            XExtEditorsRpcEnvelope::new(1, "a", "first"),
            XExtEditorsRpcEnvelope::new(1, "a", "second"),
            XExtEditorsRpcEnvelope::new(2, "b", "third"),
        ];
        let deduped = x_ext_editors_dedup_by_seq(envs);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].payload, "first");
    }

    #[test]
    fn x_ext_editors_negotiate_capabilities_basic() {
        let result = x_ext_editors_negotiate_capabilities(
            &["hover", "completion", "rename"],
            &["hover", "rename", "format"],
        );
        assert_eq!(result, vec!["hover", "rename"]);
    }

    #[test]
    fn x_ext_editors_api_version_satisfies() {
        let v1 = XExtEditorsApiVersion::new(1, 80, 0);
        let min = XExtEditorsApiVersion::new(1, 70, 0);
        assert!(v1.satisfies(&min));
        assert!(!min.satisfies(&v1));
    }

    #[test]
    fn x_ext_editors_api_version_display() {
        let v = XExtEditorsApiVersion::new(2, 3, 4);
        assert_eq!(v.to_string(), "2.3.4");
    }

    #[test]
    fn x_ext_editors_api_version_ord() {
        let v1 = XExtEditorsApiVersion::new(1, 0, 0);
        let v2 = XExtEditorsApiVersion::new(1, 1, 0);
        assert!(v1 < v2);
    }


    #[test]
    fn ext_editors_config_new() {
        let cfg = ExtEditorsConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn ext_editors_config_set_get() {
        let mut cfg = ExtEditorsConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn ext_editors_config_remove() {
        let mut cfg = ExtEditorsConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn ext_editors_config_keys_sorted() {
        let mut cfg = ExtEditorsConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn ext_editors_config_bump_version() {
        let mut cfg = ExtEditorsConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn ext_editors_config_clear() {
        let mut cfg = ExtEditorsConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn ext_editors_config_merge() {
        let mut cfg1 = ExtEditorsConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ExtEditorsConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn ext_editors_config_disable() {
        let mut cfg = ExtEditorsConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn ext_editors_rate_tracker_empty() {
        let rt = ExtEditorsRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn ext_editors_rate_tracker_record() {
        let mut rt = ExtEditorsRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn ext_editors_rate_tracker_prune() {
        let mut rt = ExtEditorsRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn ext_editors_validator_valid() {
        let v = ExtEditorsValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn ext_editors_validator_errors() {
        let mut v = ExtEditorsValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn ext_editors_validator_clear() {
        let mut v = ExtEditorsValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn ext_editors_validator_merge() {
        let mut v1 = ExtEditorsValidator::new();
        v1.add_error("e1");
        let mut v2 = ExtEditorsValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn ext_editors_rate_tracker_clear() {
        let mut rt = ExtEditorsRateTracker::new(1000);
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


    // xa_ extended tests for ext_editors
    #[test]
    fn xa_ext_editors_ring_new() {
        let rb = super::XaExtEditorsRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_editors_ring_push_len() {
        let mut rb = super::XaExtEditorsRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_editors_ring_wrap() {
        let mut rb = super::XaExtEditorsRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_editors_ring_mean_empty() {
        let rb = super::XaExtEditorsRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_editors_ring_mean_values() {
        let mut rb = super::XaExtEditorsRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_editors_ring_min_max() {
        let mut rb = super::XaExtEditorsRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_editors_ring_iter() {
        let mut rb = super::XaExtEditorsRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_editors_counter_new() {
        let c = super::XaExtEditorsCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_editors_counter_inc() {
        let mut c = super::XaExtEditorsCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_editors_counter_inc_by() {
        let mut c = super::XaExtEditorsCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_editors_counter_reset() {
        let mut c = super::XaExtEditorsCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_editors_counter_clear() {
        let mut c = super::XaExtEditorsCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_editors_counter_default() {
        let c = super::XaExtEditorsCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 58 ----

    #[test]
    fn xc_58_pool_new_empty() {
        let pool: super::Xc58Pool<i32> = super::Xc58Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_58_pool_release_acquire() {
        let mut pool = super::Xc58Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_58_pool_acquire_empty() {
        let mut pool: super::Xc58Pool<i32> = super::Xc58Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_58_pool_full() {
        let mut pool = super::Xc58Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_58_pool_drain() {
        let mut pool = super::Xc58Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_58_pool_stats() {
        let mut pool = super::Xc58Pool::new(8);
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
    fn xc_58_pool_clear() {
        let mut pool = super::Xc58Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_58_pool_shrink() {
        let mut pool = super::Xc58Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_58_pool_default() {
        let pool: super::Xc58Pool<String> = super::Xc58Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_58_pool_extend() {
        let mut pool = super::Xc58Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_58_pool_retain() {
        let mut pool = super::Xc58Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_58_scheduler_round_robin() {
        let mut sched = super::Xc58Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_58_scheduler_empty() {
        let mut sched = super::Xc58Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_58_scheduler_reset() {
        let mut sched = super::Xc58Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_58_scheduler_add_remove() {
        let mut sched = super::Xc58Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_58_scheduler_targets() {
        let sched = super::Xc58Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_58_hash_empty() {
        assert_eq!(super::xc_58_hash(b""), 5381);
    }

    #[test]
    fn xc_58_hash_data() {
        let h = super::xc_58_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_58_hash(b"hello"), h);
    }

    #[test]
    fn xc_58_reverse_str() {
        assert_eq!(super::xc_58_reverse("abc"), "cba");
        assert_eq!(super::xc_58_reverse(""), "");
    }


    // --- xd_93 deepening tests ---

    #[test]
    fn xd_93_sm_initial_state() {
        let sm = Xd93StateMachine::new();
        assert_eq!(sm.current_state(), Xd93State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_93_sm_valid_idle_to_running() {
        let mut sm = Xd93StateMachine::new();
        assert!(sm.transition(Xd93State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd93State::Running);
    }

    #[test]
    fn xd_93_sm_valid_running_to_paused() {
        let mut sm = Xd93StateMachine::new();
        sm.transition(Xd93State::Running).unwrap();
        assert!(sm.transition(Xd93State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd93State::Paused);
    }

    #[test]
    fn xd_93_sm_valid_running_to_done() {
        let mut sm = Xd93StateMachine::new();
        sm.transition(Xd93State::Running).unwrap();
        assert!(sm.transition(Xd93State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd93State::Done);
    }

    #[test]
    fn xd_93_sm_valid_paused_to_running() {
        let mut sm = Xd93StateMachine::new();
        sm.transition(Xd93State::Running).unwrap();
        sm.transition(Xd93State::Paused).unwrap();
        assert!(sm.transition(Xd93State::Running).is_ok());
    }

    #[test]
    fn xd_93_sm_valid_done_to_idle() {
        let mut sm = Xd93StateMachine::new();
        sm.transition(Xd93State::Running).unwrap();
        sm.transition(Xd93State::Done).unwrap();
        assert!(sm.transition(Xd93State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd93State::Idle);
    }

    #[test]
    fn xd_93_sm_invalid_idle_to_done() {
        let mut sm = Xd93StateMachine::new();
        assert!(sm.transition(Xd93State::Done).is_err());
    }

    #[test]
    fn xd_93_sm_invalid_idle_to_paused() {
        let mut sm = Xd93StateMachine::new();
        assert!(sm.transition(Xd93State::Paused).is_err());
    }

    #[test]
    fn xd_93_sm_history_tracking() {
        let mut sm = Xd93StateMachine::new();
        sm.transition(Xd93State::Running).unwrap();
        sm.transition(Xd93State::Paused).unwrap();
        sm.transition(Xd93State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd93State::Idle);
        assert_eq!(sm.history()[0].to, Xd93State::Running);
        assert_eq!(sm.history()[1].from, Xd93State::Running);
        assert_eq!(sm.history()[2].to, Xd93State::Done);
    }

    #[test]
    fn xd_93_sm_serialize_deserialize() {
        let mut sm = Xd93StateMachine::new();
        sm.transition(Xd93State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd93StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd93State::Running));
    }

    #[test]
    fn xd_93_sm_deserialize_invalid() {
        assert_eq!(Xd93StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_93_sm_reset() {
        let mut sm = Xd93StateMachine::new();
        sm.transition(Xd93State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd93State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_93_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd93EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd93Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_93_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd93EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd93Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd93Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_93_bus_unsubscribe() {
        let mut bus = Xd93EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_93_event_kind_and_payload() {
        let e = Xd93Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd93Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_93_bus_clear_history() {
        let mut bus = Xd93EventBus::new();
        bus.publish(Xd93Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_93_sm_step_counter_increments() {
        let mut sm = Xd93StateMachine::new();
        sm.transition(Xd93State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd93State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_16 graph tests ------------------------------------------------

    #[test]
    fn xg_16_graph_empty() {
        let g = super::Xg16Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_16_graph_add_node() {
        let mut g = super::Xg16Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_16_graph_add_edge() {
        let mut g = super::Xg16Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_16_graph_neighbors() {
        let mut g = super::Xg16Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_16_graph_has_path() {
        let mut g = super::Xg16Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_16_graph_self_path() {
        let g = super::Xg16Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_16_graph_topo_sort() {
        let mut g = super::Xg16Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_16_graph_cycle_detect_false() {
        let mut g = super::Xg16Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_16_graph_cycle_detect_true() {
        let mut g = super::Xg16Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_16 heap tests -------------------------------------------------

    #[test]
    fn xg_16_heap_empty() {
        let h: super::Xg16Heap<i32> = super::Xg16Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_16_heap_push_pop() {
        let mut h = super::Xg16Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_16_heap_peek() {
        let mut h = super::Xg16Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_16_heap_drain_sorted() {
        let mut h = super::Xg16Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_16_heap_merge() {
        let mut a = super::Xg16Heap::new();
        let mut b = super::Xg16Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_16_heap_default() {
        let h: super::Xg16Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_16_graph_default() {
        let g: super::Xg16Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh57_skip_insert_contains() {
        let mut sl = super::Xh57SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh57_skip_remove() {
        let mut sl = super::Xh57SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh57_skip_len() {
        let mut sl = super::Xh57SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh57_skip_range_query() {
        let mut sl = super::Xh57SkipList::xh_new(4);
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
    fn xh57_skip_floor_ceiling() {
        let mut sl = super::Xh57SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh57_skip_rank() {
        let mut sl = super::Xh57SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh57_skip_empty() {
        let sl = super::Xh57SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh57_skip_duplicates() {
        let mut sl = super::Xh57SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh57_bitset_set_test() {
        let mut bs = super::Xh57BitSet::xh_new(256);
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
    fn xh57_bitset_clear_count() {
        let mut bs = super::Xh57BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh57_bitset_and_or_xor() {
        let mut a = super::Xh57BitSet::xh_new(128);
        let mut b = super::Xh57BitSet::xh_new(128);
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
    fn xh57_bitset_iter_ones() {
        let mut bs = super::Xh57BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh57_bitset_first_last() {
        let mut bs = super::Xh57BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh57_bitset_empty() {
        let bs = super::Xh57BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi57_deque_push_pop_back() {
        let mut dq = super::Xi57Deque::xi_new(4);
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
    fn xi57_deque_push_pop_front() {
        let mut dq = super::Xi57Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi57_deque_mixed_ops() {
        let mut dq = super::Xi57Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi57_deque_get_and_split() {
        let mut dq = super::Xi57Deque::xi_new(8);
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
    fn xi57_deque_rotate_left() {
        let mut dq = super::Xi57Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi57_deque_rotate_right() {
        let mut dq = super::Xi57Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi57_deque_grow() {
        let mut dq = super::Xi57Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi57_deque_empty() {
        let dq = super::Xi57Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi57_interval_tree_insert_query() {
        let mut tree = super::Xi57IntervalTree::xi_new();
        tree.xi_insert(super::Xi57Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi57Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi57Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi57_interval_tree_overlap() {
        let mut tree = super::Xi57IntervalTree::xi_new();
        tree.xi_insert(super::Xi57Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi57Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi57Interval::xi_new(12, 20));
        let q = super::Xi57Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi57_interval_tree_remove() {
        let mut tree = super::Xi57IntervalTree::xi_new();
        tree.xi_insert(super::Xi57Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi57Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi57_interval_tree_gaps() {
        let mut tree = super::Xi57IntervalTree::xi_new();
        tree.xi_insert(super::Xi57Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi57Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi57Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi57Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi57Interval::xi_new(8, 10));
    }

    #[test]
    fn xi57_interval_tree_merge() {
        let mut tree = super::Xi57IntervalTree::xi_new();
        tree.xi_insert(super::Xi57Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi57Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi57Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi57Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi57Interval::xi_new(10, 15));
    }

    #[test]
    fn xi57_interval_tree_all() {
        let mut tree = super::Xi57IntervalTree::xi_new();
        tree.xi_insert(super::Xi57Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi57Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi57_interval_tree_empty() {
        let tree = super::Xi57IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi57_interval_tree_contains_point() {
        let iv = super::Xi57Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 57) ---

    #[test]
    fn xj_57_uf_make_and_find() {
        let mut uf = super::Xj57UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_57_uf_union_connected() {
        let mut uf = super::Xj57UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_57_uf_component_count() {
        let mut uf = super::Xj57UnionFind::xj_new();
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
    fn xj_57_uf_component_size() {
        let mut uf = super::Xj57UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_57_uf_largest_component() {
        let mut uf = super::Xj57UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_57_uf_many_elements() {
        let mut uf = super::Xj57UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_57_uf_separate_components() {
        let mut uf = super::Xj57UnionFind::xj_new();
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
    fn xj_57_uf_path_compression() {
        let mut uf = super::Xj57UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_57_bt_insert_get() {
        let mut bt = super::Xj57BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_57_bt_contains_len() {
        let mut bt = super::Xj57BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_57_bt_replace() {
        let mut bt = super::Xj57BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_57_bt_remove() {
        let mut bt = super::Xj57BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_57_bt_keys_values() {
        let mut bt = super::Xj57BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_57_bt_range() {
        let mut bt = super::Xj57BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_57_bt_min_max() {
        let mut bt = super::Xj57BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_57_bt_many_inserts() {
        let mut bt = super::Xj57BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_57 segment tree tests ---

    #[test]
    fn xk_57_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk57SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_57_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk57SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_57_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk57SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_57_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk57SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_57_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk57SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_57_st_single_element() {
        let data = vec![42];
        let st = super::Xk57SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_57_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk57SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_57_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk57SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_57 disjoint intervals tests ---

    #[test]
    fn xk_57_di_add_and_count() {
        let mut di = super::Xk57DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_57_di_merge_overlap() {
        let mut di = super::Xk57DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_57_di_contains() {
        let mut di = super::Xk57DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_57_di_remove() {
        let mut di = super::Xk57DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_57_di_covered_length() {
        let mut di = super::Xk57DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_57_di_gaps() {
        let mut di = super::Xk57DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_57_di_merge_adjacent() {
        let mut di = super::Xk57DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_57_di_empty() {
        let di = super::Xk57DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_57_rope_new_empty() {
        let rope = super::Xl57Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_57_rope_from_str() {
        let rope = super::Xl57Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_57_rope_insert_at() {
        let mut rope = super::Xl57Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_57_rope_delete_range() {
        let mut rope = super::Xl57Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_57_rope_char_at() {
        let rope = super::Xl57Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_57_rope_split_concat() {
        let rope = super::Xl57Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_57_rope_line_count() {
        let rope = super::Xl57Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_57_rope_line_at() {
        let rope = super::Xl57Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_57_sa_build_and_search() {
        let sa = super::Xl57SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_57_sa_count() {
        let sa = super::Xl57SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_57_sa_longest_repeated() {
        let sa = super::Xl57SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_57_sa_all_positions() {
        let sa = super::Xl57SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_57_sa_len() {
        let sa = super::Xl57SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_57_sa_empty() {
        let sa = super::Xl57SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_57_rope_slice() {
        let rope = super::Xl57Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_57_sa_search_start() {
        let sa = super::Xl57SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_57_sparse_set_get() {
        let mut m = super::Xm57MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_57_sparse_row_col() {
        let mut m = super::Xm57MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_57_sparse_transpose() {
        let mut m = super::Xm57MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_57_sparse_multiply_vec() {
        let mut m = super::Xm57MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_57_sparse_nnz_density() {
        let mut m = super::Xm57MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_57_sparse_clear() {
        let mut m = super::Xm57MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_57_sparse_overwrite_zero() {
        let mut m = super::Xm57MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_57_tokenizer_basic() {
        let t = super::Xm57Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_57_tokenizer_count() {
        let t = super::Xm57Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_57_tokenizer_unique() {
        let t = super::Xm57Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_57_tokenizer_frequency() {
        let t = super::Xm57Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_57_tokenizer_delimiter() {
        let t = super::Xm57Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_57_tokenizer_whitespace() {
        let t = super::Xm57Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_57_tokenizer_empty() {
        let t = super::Xm57Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 57 ----

    #[test]
    fn xn_57_fenwick_prefix_sum() {
        let mut ft = super::Xn57Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_57_fenwick_range_sum() {
        let mut ft = super::Xn57Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_57_fenwick_point_query() {
        let mut ft = super::Xn57Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_57_fenwick_len() {
        let ft = super::Xn57Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_57_fenwick_multiple_updates() {
        let mut ft = super::Xn57Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_57_fenwick_single_element() {
        let mut ft = super::Xn57Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_57_fenwick_find_kth() {
        let mut ft = super::Xn57Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_57_fenwick_negative_delta() {
        let mut ft = super::Xn57Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 57 ----

    #[test]
    fn xn_57_avl_insert_get() {
        let mut m = super::Xn57AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_57_avl_remove() {
        let mut m = super::Xn57AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_57_avl_in_order() {
        let mut m = super::Xn57AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_57_avl_min_max() {
        let mut m = super::Xn57AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_57_avl_floor_ceiling() {
        let mut m = super::Xn57AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_57_avl_height_balanced() {
        let mut m = super::Xn57AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_57_avl_overwrite() {
        let mut m = super::Xn57AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_57_avl_empty() {
        let m: super::Xn57AVL<i32, i32> = super::Xn57AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo57RedBlack tests ---

    #[test]
    fn xo_57_rb_insert_and_get() {
        let mut tree = super::Xo57RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_57_rb_len_and_empty() {
        let mut tree = super::Xo57RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_57_rb_min_max() {
        let mut tree = super::Xo57RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_57_rb_contains() {
        let mut tree = super::Xo57RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_57_rb_remove() {
        let mut tree = super::Xo57RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_57_rb_in_order() {
        let mut tree = super::Xo57RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_57_rb_black_height() {
        let mut tree = super::Xo57RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_57_rb_overwrite() {
        let mut tree = super::Xo57RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo57ConsistentHash tests ---

    #[test]
    fn xo_57_ch_add_and_count() {
        let mut ring = super::Xo57ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_57_ch_remove_node() {
        let mut ring = super::Xo57ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_57_ch_get_node() {
        let mut ring = super::Xo57ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_57_ch_empty_ring() {
        let ring = super::Xo57ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_57_ch_distribution() {
        let mut ring = super::Xo57ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_57_ch_rebalance() {
        let mut ring = super::Xo57ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_57_ch_virtual_nodes() {
        let mut ring = super::Xo57ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_57_ch_consistent_lookup() {
        let mut ring = super::Xo57ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_57_splay_insert_get() {
        let mut t = super::Xp57SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_57_splay_remove() {
        let mut t = super::Xp57SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_57_splay_count_increases() {
        let mut t = super::Xp57SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_57_splay_depth() {
        let mut t = super::Xp57SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_57_splay_len_empty() {
        let t = super::Xp57SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_57_splay_min_max() {
        let mut t = super::Xp57SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_57_splay_overwrite() {
        let mut t = super::Xp57SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_57_splay_remove_missing() {
        let mut t = super::Xp57SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }

}
