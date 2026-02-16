//! Mouse-based drag and drop.

use std::collections::HashSet;
use std::fmt;

/// Data being dragged.
#[derive(Debug, Clone, PartialEq)]
pub struct DragData {
    pub mime_type: String,
    pub data: Vec<u8>,
    pub label: Option<String>,
}

impl DragData {
    pub fn new(mime_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            mime_type: mime_type.into(),
            data,
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Create drag data carrying plain text.
    pub fn text(s: impl Into<String>) -> Self {
        let s = s.into();
        Self {
            mime_type: "text/plain".into(),
            data: s.into_bytes(),
            label: None,
        }
    }

    /// Create drag data carrying a list of file URIs.
    pub fn files(uris: &[&str]) -> Self {
        let joined = uris.join("\r\n");
        Self {
            mime_type: "text/uri-list".into(),
            data: joined.into_bytes(),
            label: None,
        }
    }
}

impl fmt::Display for DragData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.label {
            Some(label) => write!(f, "[{}] {} ({} bytes)", self.mime_type, label, self.data.len()),
            None => write!(f, "[{}] ({} bytes)", self.mime_type, self.data.len()),
        }
    }
}

/// Errors that can occur during drag-and-drop operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DndError {
    NoDragActive,
    InvalidTarget,
    UnsupportedMimeType(String),
    DragAlreadyActive,
}

impl fmt::Display for DndError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DndError::NoDragActive => write!(f, "no drag operation is active"),
            DndError::InvalidTarget => write!(f, "drop target is invalid"),
            DndError::UnsupportedMimeType(m) => write!(f, "unsupported mime type: {m}"),
            DndError::DragAlreadyActive => write!(f, "a drag operation is already active"),
        }
    }
}

/// A target location for a drop.
#[derive(Debug, Clone, PartialEq)]
pub struct DropTarget {
    pub uri: String,
    pub position_line: u32,
    pub position_col: u32,
}

impl DropTarget {
    /// Create a drop target from a URI and cursor position.
    pub fn from_position(uri: impl Into<String>, line: u32, col: u32) -> Self {
        Self {
            uri: uri.into(),
            position_line: line,
            position_col: col,
        }
    }
}

/// An event describing a drag operation.
#[derive(Debug, Clone, PartialEq)]
pub struct DragEvent {
    pub data: Vec<DragData>,
    pub source: Option<String>,
}

/// The outcome of a drop operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropResult {
    Accepted,
    Rejected,
    Cancelled,
}

impl fmt::Display for DropResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DropResult::Accepted => write!(f, "accepted"),
            DropResult::Rejected => write!(f, "rejected"),
            DropResult::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Service that tracks an active drag operation.
#[derive(Debug)]
pub struct DragAndDropService {
    active_drag: Option<DragEvent>,
    accepted_mime_types: HashSet<String>,
    history: DragHistory,
}

impl Default for DragAndDropService {
    fn default() -> Self {
        Self {
            active_drag: None,
            accepted_mime_types: HashSet::new(),
            history: DragHistory::default(),
        }
    }
}

impl DragAndDropService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_drag(&mut self, data: Vec<DragData>) {
        self.active_drag = Some(DragEvent {
            data,
            source: None,
        });
    }

    /// Start a drag with a source identifier, returning an error if a drag is already active.
    pub fn start_drag_from(
        &mut self,
        source: impl Into<String>,
        data: Vec<DragData>,
    ) -> Result<(), DndError> {
        if self.active_drag.is_some() {
            return Err(DndError::DragAlreadyActive);
        }
        self.active_drag = Some(DragEvent {
            data,
            source: Some(source.into()),
        });
        Ok(())
    }

    pub fn drop_on(&mut self, _target: &DropTarget) -> DropResult {
        if self.active_drag.take().is_some() {
            DropResult::Accepted
        } else {
            DropResult::Rejected
        }
    }

    /// Attempt a drop, returning a detailed error on failure.
    pub fn try_drop_on(&mut self, target: &DropTarget) -> Result<DropResult, DndError> {
        let event = self.active_drag.take().ok_or(DndError::NoDragActive)?;

        if target.uri.is_empty() {
            self.active_drag = Some(event);
            return Err(DndError::InvalidTarget);
        }

        if !self.accepted_mime_types.is_empty() {
            if let Some(bad) = event
                .data
                .iter()
                .find(|item| !self.accepted_mime_types.contains(&item.mime_type))
            {
                let mime = bad.mime_type.clone();
                self.active_drag = Some(event);
                return Err(DndError::UnsupportedMimeType(mime));
            }
        }

        self.history.record(HistoryEntry {
            data: event.data,
            target: target.clone(),
            result: DropResult::Accepted,
        });

        Ok(DropResult::Accepted)
    }

    pub fn cancel_drag(&mut self) {
        self.active_drag = None;
    }

    pub fn is_dragging(&self) -> bool {
        self.active_drag.is_some()
    }

    /// Peek at the data currently being dragged.
    pub fn active_drag_data(&self) -> Option<&[DragData]> {
        self.active_drag.as_ref().map(|e| e.data.as_slice())
    }

    /// Return the set of accepted mime types.
    pub fn accepted_mime_types(&self) -> &HashSet<String> {
        &self.accepted_mime_types
    }

    /// Register a mime type that this service accepts.
    pub fn register_mime_type(&mut self, mime: impl Into<String>) {
        self.accepted_mime_types.insert(mime.into());
    }

    /// Access the drag/drop history.
    pub fn history(&self) -> &DragHistory {
        &self.history
    }
}

/// A single recorded drag-and-drop operation.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    pub data: Vec<DragData>,
    pub target: DropTarget,
    pub result: DropResult,
}

/// Tracks recent drag/drop operations for undo support.
#[derive(Debug, Default, Clone)]
pub struct DragHistory {
    entries: Vec<HistoryEntry>,
}

impl DragHistory {
    /// Record a completed drop operation.
    pub fn record(&mut self, entry: HistoryEntry) {
        self.entries.push(entry);
    }

    /// Return all recorded entries.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Pop the most recent entry (undo).
    pub fn undo(&mut self) -> Option<HistoryEntry> {
        self.entries.pop()
    }

    /// Number of recorded operations.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Trait for types that handle drop events.
pub trait DragAndDropProvider {
    fn handle_drop(&self, target: &DropTarget, event: &DragEvent) -> DropResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_and_drop_lifecycle() {
        let mut svc = DragAndDropService::new();
        assert!(!svc.is_dragging());

        svc.start_drag(vec![DragData::new("text/plain", b"hello".to_vec())]);
        assert!(svc.is_dragging());

        let target = DropTarget {
            uri: "file:///test.rs".into(),
            position_line: 0,
            position_col: 0,
        };
        assert_eq!(svc.drop_on(&target), DropResult::Accepted);
        assert!(!svc.is_dragging());
    }

    #[test]
    fn drop_without_drag_is_rejected() {
        let mut svc = DragAndDropService::new();
        let target = DropTarget {
            uri: "file:///test.rs".into(),
            position_line: 0,
            position_col: 0,
        };
        assert_eq!(svc.drop_on(&target), DropResult::Rejected);
    }

    #[test]
    fn cancel_drag() {
        let mut svc = DragAndDropService::new();
        svc.start_drag(vec![DragData::new("text/uri-list", vec![])]);
        assert!(svc.is_dragging());
        svc.cancel_drag();
        assert!(!svc.is_dragging());
    }

    #[test]
    fn drag_data_with_label() {
        let d = DragData::new("image/png", vec![0x89, 0x50]).with_label("screenshot");
        assert_eq!(d.label.as_deref(), Some("screenshot"));
        assert_eq!(d.mime_type, "image/png");
    }

    #[test]
    fn text_helper() {
        let d = DragData::text("hello world");
        assert_eq!(d.mime_type, "text/plain");
        assert_eq!(d.data, b"hello world");
        assert_eq!(d.label, None);
    }

    #[test]
    fn files_helper() {
        let d = DragData::files(&["file:///a.txt", "file:///b.txt"]);
        assert_eq!(d.mime_type, "text/uri-list");
        assert_eq!(
            String::from_utf8(d.data).unwrap(),
            "file:///a.txt\r\nfile:///b.txt"
        );
    }

    #[test]
    fn try_drop_on_no_drag() {
        let mut svc = DragAndDropService::new();
        let target = DropTarget::from_position("file:///t.rs", 1, 0);
        assert_eq!(svc.try_drop_on(&target), Err(DndError::NoDragActive));
    }

    #[test]
    fn try_drop_on_invalid_target() {
        let mut svc = DragAndDropService::new();
        svc.start_drag(vec![DragData::text("x")]);
        let target = DropTarget::from_position("", 0, 0);
        assert_eq!(svc.try_drop_on(&target), Err(DndError::InvalidTarget));
        assert!(svc.is_dragging(), "drag should be restored after error");
    }

    #[test]
    fn try_drop_on_unsupported_mime() {
        let mut svc = DragAndDropService::new();
        svc.register_mime_type("text/plain");
        svc.start_drag(vec![DragData::new("image/png", vec![0x89])]);
        let target = DropTarget::from_position("file:///t.rs", 0, 0);
        assert_eq!(
            svc.try_drop_on(&target),
            Err(DndError::UnsupportedMimeType("image/png".into()))
        );
        assert!(svc.is_dragging());
    }

    #[test]
    fn try_drop_on_accepted() {
        let mut svc = DragAndDropService::new();
        svc.register_mime_type("text/plain");
        svc.start_drag(vec![DragData::text("payload")]);
        let target = DropTarget::from_position("file:///t.rs", 5, 10);
        assert_eq!(svc.try_drop_on(&target), Ok(DropResult::Accepted));
        assert!(!svc.is_dragging());
    }

    #[test]
    fn active_drag_data_peek() {
        let mut svc = DragAndDropService::new();
        assert!(svc.active_drag_data().is_none());
        svc.start_drag(vec![DragData::text("abc"), DragData::files(&["file:///x"])]);
        let items = svc.active_drag_data().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].mime_type, "text/plain");
        assert_eq!(items[1].mime_type, "text/uri-list");
    }

    #[test]
    fn mime_type_filtering() {
        let mut svc = DragAndDropService::new();
        assert!(svc.accepted_mime_types().is_empty());
        svc.register_mime_type("text/plain");
        svc.register_mime_type("text/uri-list");
        assert_eq!(svc.accepted_mime_types().len(), 2);
        assert!(svc.accepted_mime_types().contains("text/plain"));
    }

    #[test]
    fn display_impls() {
        assert_eq!(format!("{}", DropResult::Accepted), "accepted");
        assert_eq!(format!("{}", DropResult::Rejected), "rejected");
        assert_eq!(format!("{}", DropResult::Cancelled), "cancelled");

        let d = DragData::text("hi");
        assert_eq!(format!("{d}"), "[text/plain] (2 bytes)");

        let d2 = DragData::text("hi").with_label("greeting");
        assert_eq!(format!("{d2}"), "[text/plain] greeting (2 bytes)");
    }

    #[test]
    fn start_drag_from_source() {
        let mut svc = DragAndDropService::new();
        assert!(svc
            .start_drag_from("editor-tab-1", vec![DragData::text("fn main() {}")])
            .is_ok());
        assert!(svc.is_dragging());
        // second start should fail
        let err = svc
            .start_drag_from("editor-tab-2", vec![DragData::text("x")])
            .unwrap_err();
        assert_eq!(err, DndError::DragAlreadyActive);
    }

    #[test]
    fn history_tracking() {
        let mut svc = DragAndDropService::new();
        svc.register_mime_type("text/plain");

        svc.start_drag(vec![DragData::text("first")]);
        let t1 = DropTarget::from_position("file:///a.rs", 1, 0);
        svc.try_drop_on(&t1).unwrap();

        svc.start_drag(vec![DragData::text("second")]);
        let t2 = DropTarget::from_position("file:///b.rs", 2, 5);
        svc.try_drop_on(&t2).unwrap();

        let h = svc.history();
        assert_eq!(h.len(), 2);
        assert!(!h.is_empty());
        assert_eq!(h.entries()[0].target.uri, "file:///a.rs");
        assert_eq!(h.entries()[1].target.uri, "file:///b.rs");
    }

    #[test]
    fn error_display() {
        assert_eq!(
            format!("{}", DndError::NoDragActive),
            "no drag operation is active"
        );
        assert_eq!(
            format!("{}", DndError::InvalidTarget),
            "drop target is invalid"
        );
        assert_eq!(
            format!("{}", DndError::UnsupportedMimeType("image/png".into())),
            "unsupported mime type: image/png"
        );
        assert_eq!(
            format!("{}", DndError::DragAlreadyActive),
            "a drag operation is already active"
        );
    }

    #[test]
    fn drop_target_from_position() {
        let t = DropTarget::from_position("file:///main.rs", 42, 7);
        assert_eq!(t.uri, "file:///main.rs");
        assert_eq!(t.position_line, 42);
        assert_eq!(t.position_col, 7);
    }

    #[test]
    fn history_undo() {
        let mut svc = DragAndDropService::new();
        svc.start_drag(vec![DragData::text("undo me")]);
        let target = DropTarget::from_position("file:///u.rs", 0, 0);
        svc.try_drop_on(&target).unwrap();

        assert_eq!(svc.history().len(), 1);
        let mut h = svc.history().clone();
        let entry = h.undo().unwrap();
        assert_eq!(entry.target.uri, "file:///u.rs");
        assert!(h.is_empty());
    }
}
