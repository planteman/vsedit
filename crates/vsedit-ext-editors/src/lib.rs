//! Ext API: Editors.
//!
//! RPC bridge between the extension host and the main thread for editors.

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
}
