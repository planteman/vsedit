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

}
