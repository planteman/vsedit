//! Mouse-based drag and drop.

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
}

/// A target location for a drop.
#[derive(Debug, Clone, PartialEq)]
pub struct DropTarget {
    pub uri: String,
    pub position_line: u32,
    pub position_col: u32,
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

/// Service that tracks an active drag operation.
#[derive(Debug, Default)]
pub struct DragAndDropService {
    active_drag: Option<DragEvent>,
}

impl DragAndDropService {
    pub fn new() -> Self {
        Self { active_drag: None }
    }

    pub fn start_drag(&mut self, data: Vec<DragData>) {
        self.active_drag = Some(DragEvent {
            data,
            source: None,
        });
    }

    pub fn drop_on(&mut self, _target: &DropTarget) -> DropResult {
        if self.active_drag.take().is_some() {
            DropResult::Accepted
        } else {
            DropResult::Rejected
        }
    }

    pub fn cancel_drag(&mut self) {
        self.active_drag = None;
    }

    pub fn is_dragging(&self) -> bool {
        self.active_drag.is_some()
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
}
