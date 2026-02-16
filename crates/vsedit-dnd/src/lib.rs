//! Mouse-based drag and drop.

use std::collections::HashMap;
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

/// The visual effect associated with a drag-and-drop operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DragEffect {
    Copy,
    Move,
    Link,
    None,
}

impl fmt::Display for DragEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DragEffect::Copy => write!(f, "copy"),
            DragEffect::Move => write!(f, "move"),
            DragEffect::Link => write!(f, "link"),
            DragEffect::None => write!(f, "none"),
        }
    }
}

/// A transfer object that bundles multiple [`DragData`] items together with
/// the current and allowed drag effects, mirroring the HTML DataTransfer API.
#[derive(Debug, Clone)]
pub struct DragDataTransfer {
    items: Vec<DragData>,
    drop_effect: DragEffect,
    effect_allowed: Vec<DragEffect>,
}

impl DragDataTransfer {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            drop_effect: DragEffect::None,
            effect_allowed: vec![
                DragEffect::Copy,
                DragEffect::Move,
                DragEffect::Link,
            ],
        }
    }

    /// Append a drag data item to the transfer.
    pub fn add_item(&mut self, item: DragData) {
        self.items.push(item);
    }

    /// Return all items whose MIME type matches `mime`.
    pub fn get_items_by_mime(&self, mime: &str) -> Vec<&DragData> {
        self.items.iter().filter(|i| i.mime_type == mime).collect()
    }

    /// Check whether any item in the transfer carries the given MIME type.
    pub fn has_mime_type(&self, mime: &str) -> bool {
        self.items.iter().any(|i| i.mime_type == mime)
    }

    /// Restrict the set of effects the drag source permits.
    pub fn set_effect_allowed(&mut self, effects: Vec<DragEffect>) {
        self.effect_allowed = effects;
    }

    /// Set the current drop effect, which must be one of the allowed effects.
    /// Returns `false` if the effect is not allowed.
    pub fn set_drop_effect(&mut self, effect: DragEffect) -> bool {
        if effect == DragEffect::None || self.effect_allowed.contains(&effect) {
            self.drop_effect = effect;
            true
        } else {
            false
        }
    }

    /// The current drop effect.
    pub fn drop_effect(&self) -> DragEffect {
        self.drop_effect
    }

    /// The list of allowed effects.
    pub fn effect_allowed(&self) -> &[DragEffect] {
        &self.effect_allowed
    }

    /// All items in the transfer.
    pub fn items(&self) -> &[DragData] {
        &self.items
    }

    /// Remove all items and reset the drop effect.
    pub fn clear(&mut self) {
        self.items.clear();
        self.drop_effect = DragEffect::None;
    }
}

impl Default for DragDataTransfer {
    fn default() -> Self {
        Self::new()
    }
}

/// A region in the UI that can receive drops.
#[derive(Debug, Clone)]
pub struct DropZone {
    pub id: String,
    pub accepted_mimes: HashSet<String>,
    pub enabled: bool,
}

impl DropZone {
    pub fn new(id: impl Into<String>, accepted_mimes: HashSet<String>) -> Self {
        Self {
            id: id.into(),
            accepted_mimes,
            enabled: true,
        }
    }

    /// Returns `true` if this zone accepts the given drag data's MIME type
    /// and the zone is enabled.
    pub fn can_accept(&self, data: &DragData) -> bool {
        self.enabled && self.accepted_mimes.contains(&data.mime_type)
    }

    /// Returns `true` if this zone can accept *any* of the provided items.
    pub fn can_accept_any(&self, items: &[DragData]) -> bool {
        self.enabled && items.iter().any(|d| self.accepted_mimes.contains(&d.mime_type))
    }
}

/// Registry that manages multiple named [`DropZone`]s.
#[derive(Debug, Default)]
pub struct DropZoneRegistry {
    zones: HashMap<String, DropZone>,
}

impl DropZoneRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new drop zone. Replaces any existing zone with the same id.
    pub fn register(&mut self, zone: DropZone) {
        self.zones.insert(zone.id.clone(), zone);
    }

    /// Remove a zone by id, returning it if it existed.
    pub fn unregister(&mut self, id: &str) -> Option<DropZone> {
        self.zones.remove(id)
    }

    /// Return a reference to a zone by id.
    pub fn get_zone(&self, id: &str) -> Option<&DropZone> {
        self.zones.get(id)
    }

    /// Enable or disable a zone by id. Returns `false` if the zone was not found.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        if let Some(zone) = self.zones.get_mut(id) {
            zone.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Find all zones that can accept the given drag data item.
    pub fn find_accepting_zones(&self, data: &DragData) -> Vec<&DropZone> {
        self.zones.values().filter(|z| z.can_accept(data)).collect()
    }

    /// Number of registered zones.
    pub fn len(&self) -> usize {
        self.zones.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.zones.is_empty()
    }
}

/// Preview information shown under the cursor during a drag operation.
#[derive(Debug, Clone, PartialEq)]
pub struct DragPreview {
    pub width: u32,
    pub height: u32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub label: String,
}

impl DragPreview {
    pub fn new(width: u32, height: u32, label: impl Into<String>) -> Self {
        Self {
            width,
            height,
            offset_x: 0,
            offset_y: 0,
            label: label.into(),
        }
    }

    /// Set the offset of the preview relative to the cursor.
    pub fn with_offset(mut self, x: i32, y: i32) -> Self {
        self.offset_x = x;
        self.offset_y = y;
        self
    }
}

impl fmt::Display for DragPreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\"{}\" ({}x{} offset {}, {})",
            self.label, self.width, self.height, self.offset_x, self.offset_y,
        )
    }
}

/// Aggregate metrics computed from a [`DragHistory`].
#[derive(Debug, Clone, PartialEq)]
pub struct DragMetrics {
    pub total_drops: usize,
    pub success_rate: f64,
    pub most_common_mime_type: Option<String>,
    pub avg_items_per_drop: f64,
}

impl DragMetrics {
    /// Compute metrics from the supplied history.
    pub fn from_history(history: &DragHistory) -> Self {
        let entries = history.entries();
        let total_drops = entries.len();

        if total_drops == 0 {
            return Self {
                total_drops: 0,
                success_rate: 0.0,
                most_common_mime_type: None,
                avg_items_per_drop: 0.0,
            };
        }

        let accepted = entries
            .iter()
            .filter(|e| e.result == DropResult::Accepted)
            .count();
        let success_rate = accepted as f64 / total_drops as f64;

        let total_items: usize = entries.iter().map(|e| e.data.len()).sum();
        let avg_items_per_drop = total_items as f64 / total_drops as f64;

        // Count MIME type frequencies across all items in all entries.
        let mut mime_counts: HashMap<&str, usize> = HashMap::new();
        for entry in entries {
            for item in &entry.data {
                *mime_counts.entry(item.mime_type.as_str()).or_insert(0) += 1;
            }
        }

        let most_common_mime_type = mime_counts
            .into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(mime, _)| mime.to_string());

        Self {
            total_drops,
            success_rate,
            most_common_mime_type,
            avg_items_per_drop,
        }
    }
}

// ---------------------------------------------------------------------------
// DragPayload – typed drag content
// ---------------------------------------------------------------------------

/// Typed payload for drag operations.
#[derive(Debug, Clone, PartialEq)]
pub enum DragPayload {
    File { uri: String, mime_type: String },
    Tab { tab_id: String, group_id: usize },
    TreeNode { node_id: String, parent_id: Option<String> },
    Text { content: String },
}

impl DragPayload {
    pub fn payload_type(&self) -> &str {
        match self {
            Self::File { .. } => "file",
            Self::Tab { .. } => "tab",
            Self::TreeNode { .. } => "tree_node",
            Self::Text { .. } => "text",
        }
    }

    pub fn is_file(&self) -> bool {
        matches!(self, Self::File { .. })
    }

    pub fn is_tab(&self) -> bool {
        matches!(self, Self::Tab { .. })
    }

    pub fn is_tree_node(&self) -> bool {
        matches!(self, Self::TreeNode { .. })
    }

    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text { .. })
    }
}

impl fmt::Display for DragPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File { uri, mime_type } => write!(f, "File({uri}, {mime_type})"),
            Self::Tab { tab_id, group_id } => write!(f, "Tab({tab_id}, group {group_id})"),
            Self::TreeNode { node_id, parent_id } => {
                if let Some(pid) = parent_id {
                    write!(f, "TreeNode({node_id}, parent {pid})")
                } else {
                    write!(f, "TreeNode({node_id}, root)")
                }
            }
            Self::Text { content } => write!(f, "Text({content})"),
        }
    }
}

// ---------------------------------------------------------------------------
// DropPosition / DropZoneTarget
// ---------------------------------------------------------------------------

/// Where inside a drop zone the item should land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPosition {
    Before,
    After,
    Into,
}

/// A resolved drop target combining a zone id and a position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropZoneTarget {
    pub zone_id: String,
    pub position: DropPosition,
}

impl DropZoneTarget {
    pub fn new(zone_id: impl Into<String>, position: DropPosition) -> Self {
        Self {
            zone_id: zone_id.into(),
            position,
        }
    }

    pub fn is_before(&self) -> bool {
        self.position == DropPosition::Before
    }

    pub fn is_after(&self) -> bool {
        self.position == DropPosition::After
    }

    pub fn is_into(&self) -> bool {
        self.position == DropPosition::Into
    }
}

// ---------------------------------------------------------------------------
// Drag geometry helpers
// ---------------------------------------------------------------------------

/// Euclidean distance between two points.
pub fn drag_distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt()
}

/// Returns `true` when the drag distance exceeds the given threshold (default 5.0).
pub fn exceeds_drag_threshold(x1: f64, y1: f64, x2: f64, y2: f64, threshold: f64) -> bool {
    drag_distance(x1, y1, x2, y2) > threshold
}

/// Tracks a drag gesture from press to release.
#[derive(Debug, Clone)]
pub struct DragGesture {
    pub start_x: f64,
    pub start_y: f64,
    pub current_x: f64,
    pub current_y: f64,
    pub is_active: bool,
    pub payload: Option<DragPayload>,
}

impl DragGesture {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            start_x: x,
            start_y: y,
            current_x: x,
            current_y: y,
            is_active: true,
            payload: None,
        }
    }

    pub fn update(&mut self, x: f64, y: f64) {
        self.current_x = x;
        self.current_y = y;
    }

    pub fn distance(&self) -> f64 {
        drag_distance(self.start_x, self.start_y, self.current_x, self.current_y)
    }

    pub fn has_exceeded_threshold(&self, threshold: f64) -> bool {
        self.distance() > threshold
    }

    /// Mark the gesture complete and return the payload, if any.
    pub fn complete(&mut self) -> Option<DragPayload> {
        self.is_active = false;
        self.payload.take()
    }
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

    // ---- new tests ----

    #[test]
    fn drag_effect_display() {
        assert_eq!(format!("{}", DragEffect::Copy), "copy");
        assert_eq!(format!("{}", DragEffect::Move), "move");
        assert_eq!(format!("{}", DragEffect::Link), "link");
        assert_eq!(format!("{}", DragEffect::None), "none");
    }

    #[test]
    fn drag_data_transfer_add_and_query() {
        let mut transfer = DragDataTransfer::new();
        transfer.add_item(DragData::text("hello"));
        transfer.add_item(DragData::new("image/png", vec![0x89, 0x50]));
        transfer.add_item(DragData::text("world"));

        assert!(transfer.has_mime_type("text/plain"));
        assert!(transfer.has_mime_type("image/png"));
        assert!(!transfer.has_mime_type("application/json"));

        let text_items = transfer.get_items_by_mime("text/plain");
        assert_eq!(text_items.len(), 2);
        assert_eq!(text_items[0].data, b"hello");
        assert_eq!(text_items[1].data, b"world");

        assert_eq!(transfer.items().len(), 3);
    }

    #[test]
    fn drag_data_transfer_effects() {
        let mut transfer = DragDataTransfer::new();
        assert_eq!(transfer.drop_effect(), DragEffect::None);

        // Copy is allowed by default
        assert!(transfer.set_drop_effect(DragEffect::Copy));
        assert_eq!(transfer.drop_effect(), DragEffect::Copy);

        // Restrict to Move only
        transfer.set_effect_allowed(vec![DragEffect::Move]);
        assert!(!transfer.set_drop_effect(DragEffect::Copy));
        assert!(transfer.set_drop_effect(DragEffect::Move));
        // None is always accepted
        assert!(transfer.set_drop_effect(DragEffect::None));
    }

    #[test]
    fn drag_data_transfer_clear() {
        let mut transfer = DragDataTransfer::new();
        transfer.add_item(DragData::text("a"));
        transfer.add_item(DragData::text("b"));
        assert!(transfer.set_drop_effect(DragEffect::Move));
        assert_eq!(transfer.items().len(), 2);

        transfer.clear();
        assert!(transfer.items().is_empty());
        assert_eq!(transfer.drop_effect(), DragEffect::None);
    }

    #[test]
    fn drop_zone_acceptance() {
        let mimes: HashSet<String> =
            ["text/plain", "text/uri-list"].iter().map(|s| s.to_string()).collect();
        let zone = DropZone::new("editor-main", mimes);

        assert!(zone.can_accept(&DragData::text("hi")));
        assert!(zone.can_accept(&DragData::files(&["file:///a"])));
        assert!(!zone.can_accept(&DragData::new("image/png", vec![0x89])));

        let items = vec![
            DragData::new("image/png", vec![0x89]),
            DragData::text("fallback"),
        ];
        assert!(zone.can_accept_any(&items));

        let bad_items = vec![DragData::new("image/png", vec![0x89])];
        assert!(!zone.can_accept_any(&bad_items));
    }

    #[test]
    fn drop_zone_disabled() {
        let mimes: HashSet<String> = ["text/plain"].iter().map(|s| s.to_string()).collect();
        let mut zone = DropZone::new("sidebar", mimes);
        assert!(zone.can_accept(&DragData::text("x")));

        zone.enabled = false;
        assert!(!zone.can_accept(&DragData::text("x")));
        assert!(!zone.can_accept_any(&[DragData::text("x")]));
    }

    #[test]
    fn drop_zone_registry_operations() {
        let mut registry = DropZoneRegistry::new();
        assert!(registry.is_empty());

        let mimes_text: HashSet<String> = ["text/plain"].iter().map(|s| s.to_string()).collect();
        let mimes_img: HashSet<String> = ["image/png"].iter().map(|s| s.to_string()).collect();

        registry.register(DropZone::new("editor", mimes_text));
        registry.register(DropZone::new("preview", mimes_img));
        assert_eq!(registry.len(), 2);

        assert!(registry.get_zone("editor").is_some());
        assert!(registry.get_zone("nonexistent").is_none());

        // find_accepting_zones
        let text_data = DragData::text("hi");
        let accepting = registry.find_accepting_zones(&text_data);
        assert_eq!(accepting.len(), 1);
        assert_eq!(accepting[0].id, "editor");

        // disable
        assert!(registry.set_enabled("editor", false));
        assert!(registry.find_accepting_zones(&text_data).is_empty());
        assert!(registry.set_enabled("editor", true));
        assert_eq!(registry.find_accepting_zones(&text_data).len(), 1);

        // unregister
        let removed = registry.unregister("preview");
        assert!(removed.is_some());
        assert_eq!(registry.len(), 1);
        assert!(!registry.set_enabled("preview", true));
    }

    #[test]
    fn drag_preview_creation() {
        let preview = DragPreview::new(120, 30, "Move item")
            .with_offset(-10, 5);
        assert_eq!(preview.width, 120);
        assert_eq!(preview.height, 30);
        assert_eq!(preview.offset_x, -10);
        assert_eq!(preview.offset_y, 5);
        assert_eq!(preview.label, "Move item");
        assert_eq!(
            format!("{preview}"),
            "\"Move item\" (120x30 offset -10, 5)"
        );
    }

    #[test]
    fn drag_metrics_from_history() {
        let mut history = DragHistory::default();

        // Empty history
        let m = DragMetrics::from_history(&history);
        assert_eq!(m.total_drops, 0);
        assert_eq!(m.success_rate, 0.0);
        assert_eq!(m.most_common_mime_type, None);
        assert_eq!(m.avg_items_per_drop, 0.0);

        // Add some entries
        let target = DropTarget::from_position("file:///a.rs", 0, 0);
        history.record(HistoryEntry {
            data: vec![DragData::text("a"), DragData::text("b")],
            target: target.clone(),
            result: DropResult::Accepted,
        });
        history.record(HistoryEntry {
            data: vec![DragData::new("image/png", vec![0x89])],
            target: target.clone(),
            result: DropResult::Accepted,
        });
        history.record(HistoryEntry {
            data: vec![DragData::text("c")],
            target: target.clone(),
            result: DropResult::Rejected,
        });

        let m = DragMetrics::from_history(&history);
        assert_eq!(m.total_drops, 3);
        // 2 out of 3 accepted
        assert!((m.success_rate - 2.0 / 3.0).abs() < f64::EPSILON);
        // text/plain appears 3 times vs image/png 1 time
        assert_eq!(m.most_common_mime_type.as_deref(), Some("text/plain"));
        // total items: 2 + 1 + 1 = 4, avg = 4/3
        assert!((m.avg_items_per_drop - 4.0 / 3.0).abs() < f64::EPSILON);
    }

    // --- DragPayload tests ---

    #[test]
    fn drag_payload_type_names() {
        let file = DragPayload::File { uri: "file:///a.txt".into(), mime_type: "text/plain".into() };
        let tab = DragPayload::Tab { tab_id: "t1".into(), group_id: 0 };
        let tree = DragPayload::TreeNode { node_id: "n1".into(), parent_id: None };
        let text = DragPayload::Text { content: "hello".into() };
        assert_eq!(file.payload_type(), "file");
        assert_eq!(tab.payload_type(), "tab");
        assert_eq!(tree.payload_type(), "tree_node");
        assert_eq!(text.payload_type(), "text");
    }

    #[test]
    fn drag_payload_is_variant() {
        let p = DragPayload::File { uri: "u".into(), mime_type: "m".into() };
        assert!(p.is_file());
        assert!(!p.is_tab());
        assert!(!p.is_tree_node());
        assert!(!p.is_text());
    }

    #[test]
    fn drag_payload_display() {
        let p = DragPayload::Tab { tab_id: "t1".into(), group_id: 2 };
        assert_eq!(format!("{p}"), "Tab(t1, group 2)");

        let p2 = DragPayload::TreeNode { node_id: "n".into(), parent_id: Some("p".into()) };
        assert!(format!("{p2}").contains("parent p"));

        let p3 = DragPayload::TreeNode { node_id: "n".into(), parent_id: None };
        assert!(format!("{p3}").contains("root"));
    }

    // --- DropPosition / DropZoneTarget tests ---

    #[test]
    fn drop_zone_target_positions() {
        let t = DropZoneTarget::new("zone1", DropPosition::Before);
        assert!(t.is_before());
        assert!(!t.is_after());
        assert!(!t.is_into());
    }

    #[test]
    fn drop_zone_target_into() {
        let t = DropZoneTarget::new("z", DropPosition::Into);
        assert!(t.is_into());
        assert_eq!(t.zone_id, "z");
    }

    // --- drag_distance / exceeds_drag_threshold tests ---

    #[test]
    fn drag_distance_zero() {
        assert!((drag_distance(1.0, 2.0, 1.0, 2.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn drag_distance_3_4_5() {
        let d = drag_distance(0.0, 0.0, 3.0, 4.0);
        assert!((d - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn exceeds_threshold_true() {
        assert!(exceeds_drag_threshold(0.0, 0.0, 10.0, 0.0, 5.0));
    }

    #[test]
    fn exceeds_threshold_false() {
        assert!(!exceeds_drag_threshold(0.0, 0.0, 3.0, 0.0, 5.0));
    }

    // --- DragGesture tests ---

    #[test]
    fn drag_gesture_lifecycle() {
        let mut g = DragGesture::new(10.0, 20.0);
        assert!(g.is_active);
        assert!((g.distance()).abs() < f64::EPSILON);

        g.update(13.0, 24.0);
        assert!((g.distance() - 5.0).abs() < f64::EPSILON);
        assert!(g.has_exceeded_threshold(4.9));
        assert!(!g.has_exceeded_threshold(5.0));

        g.payload = Some(DragPayload::Text { content: "hi".into() });
        let p = g.complete();
        assert!(!g.is_active);
        assert!(p.is_some());
        assert!(p.unwrap().is_text());
    }

    #[test]
    fn drag_gesture_complete_without_payload() {
        let mut g = DragGesture::new(0.0, 0.0);
        assert!(g.complete().is_none());
        assert!(!g.is_active);
    }
}
