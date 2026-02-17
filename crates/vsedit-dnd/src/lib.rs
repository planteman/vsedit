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

// ---------------------------------------------------------------------------
// DragData extensions
// ---------------------------------------------------------------------------

impl DragData {
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn has_files(&self) -> bool {
        self.mime_type == "text/uri-list"
    }

    pub fn file_count(&self) -> usize {
        if !self.has_files() {
            return 0;
        }
        let text = match std::str::from_utf8(&self.data) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        text.split("\r\n").filter(|s| !s.is_empty()).count()
    }
}

// ---------------------------------------------------------------------------
// DragEvent extensions
// ---------------------------------------------------------------------------

impl DragEvent {
    pub fn has_data(&self) -> bool {
        !self.data.is_empty()
    }

    pub fn data_count(&self) -> usize {
        self.data.len()
    }

    pub fn mime_types(&self) -> Vec<&str> {
        self.data.iter().map(|d| d.mime_type.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// DropResult extensions
// ---------------------------------------------------------------------------

impl DropResult {
    pub fn is_success(&self) -> bool {
        *self == DropResult::Accepted
    }

    pub fn is_cancelled(&self) -> bool {
        *self == DropResult::Cancelled
    }

    pub fn is_rejected(&self) -> bool {
        *self == DropResult::Rejected
    }
}

// ---------------------------------------------------------------------------
// DragEffect extensions
// ---------------------------------------------------------------------------

impl DragEffect {
    pub fn is_move(&self) -> bool {
        *self == DragEffect::Move
    }

    pub fn is_copy(&self) -> bool {
        *self == DragEffect::Copy
    }

    pub fn is_link(&self) -> bool {
        *self == DragEffect::Link
    }

    pub fn is_none(&self) -> bool {
        *self == DragEffect::None
    }

    pub fn label(&self) -> &str {
        match self {
            DragEffect::Copy => "Copy",
            DragEffect::Move => "Move",
            DragEffect::Link => "Link",
            DragEffect::None => "None",
        }
    }
}

// ---------------------------------------------------------------------------
// DragHistory extensions
// ---------------------------------------------------------------------------

impl DragHistory {
    pub fn most_recent(&self) -> Option<&HistoryEntry> {
        self.entries.last()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, HistoryEntry> {
        self.entries.iter()
    }

    pub fn accepted_count(&self) -> usize {
        self.entries.iter().filter(|e| e.result.is_success()).count()
    }

    pub fn rejected_count(&self) -> usize {
        self.entries.iter().filter(|e| e.result.is_rejected()).count()
    }
}

// ---------------------------------------------------------------------------
// DropZoneRegistry extensions
// ---------------------------------------------------------------------------

impl DropZoneRegistry {
    pub fn zone_count(&self) -> usize {
        self.zones.len()
    }

    pub fn zone_ids(&self) -> Vec<&str> {
        self.zones.keys().map(|k| k.as_str()).collect()
    }

    pub fn clear(&mut self) {
        self.zones.clear();
    }

    pub fn enabled_count(&self) -> usize {
        self.zones.values().filter(|z| z.enabled).count()
    }
}

// ---------------------------------------------------------------------------
// DragMetrics extensions
// ---------------------------------------------------------------------------

impl fmt::Display for DragMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} drops ({:.0}% success, {:.1} items/drop)",
            self.total_drops,
            self.success_rate * 100.0,
            self.avg_items_per_drop,
        )
    }
}

impl DragMetrics {
    pub fn is_empty(&self) -> bool {
        self.total_drops == 0
    }

    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate
    }
}

// ---------------------------------------------------------------------------
// DragDataTransfer extensions
// ---------------------------------------------------------------------------

impl DragDataTransfer {
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn mime_types(&self) -> Vec<&str> {
        let mut types: Vec<&str> = self.items.iter().map(|i| i.mime_type.as_str()).collect();
        types.sort_unstable();
        types.dedup();
        types
    }
}

// ---------------------------------------------------------------------------
// DragAndDropService extensions
// ---------------------------------------------------------------------------

impl DragAndDropService {
    pub fn history_count(&self) -> usize {
        self.history.len()
    }

    pub fn active_source(&self) -> Option<&str> {
        self.active_drag.as_ref().and_then(|e| e.source.as_deref())
    }
}

// ---------------------------------------------------------------------------
// Rect – axis-aligned bounding box for hit testing
// ---------------------------------------------------------------------------

/// An axis-aligned rectangle used for drop-zone hit testing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    /// Returns `true` if the point `(px, py)` lies inside this rectangle
    /// (inclusive on all edges).
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x
            && px <= self.x + self.width
            && py >= self.y
            && py <= self.y + self.height
    }

    /// Returns `true` if `self` and `other` overlap.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    /// The center point of the rectangle.
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rect({}, {}, {}×{})", self.x, self.y, self.width, self.height)
    }
}

// ---------------------------------------------------------------------------
// DragFeedbackState – visual feedback state machine
// ---------------------------------------------------------------------------

/// Visual feedback states during a drag operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragFeedbackState {
    /// Mouse pressed but drag threshold not yet exceeded.
    Pending,
    /// Threshold exceeded – drag is visually active.
    Dragging,
    /// Cursor is over a valid drop zone.
    OverValidTarget,
    /// Cursor is over an invalid (disabled / wrong mime) drop zone.
    OverInvalidTarget,
    /// Drop completed successfully.
    Dropped,
    /// Drag was cancelled (e.g. Escape key).
    Cancelled,
}

impl DragFeedbackState {
    /// Returns `true` for terminal states (`Dropped` or `Cancelled`).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Dropped | Self::Cancelled)
    }

    /// Returns `true` when a drag ghost / preview should be rendered.
    pub fn should_show_preview(&self) -> bool {
        matches!(self, Self::Dragging | Self::OverValidTarget | Self::OverInvalidTarget)
    }
}

impl fmt::Display for DragFeedbackState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Pending => "pending",
            Self::Dragging => "dragging",
            Self::OverValidTarget => "over-valid",
            Self::OverInvalidTarget => "over-invalid",
            Self::Dropped => "dropped",
            Self::Cancelled => "cancelled",
        };
        f.write_str(label)
    }
}

// ---------------------------------------------------------------------------
// MultiDragTracker – track multiple selected items during a drag
// ---------------------------------------------------------------------------

/// Tracks a set of items that are being dragged together.
#[derive(Debug, Clone)]
pub struct MultiDragTracker {
    items: Vec<DragPayload>,
}

impl MultiDragTracker {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(&mut self, payload: DragPayload) {
        self.items.push(payload);
    }

    pub fn remove_by_index(&mut self, index: usize) -> Option<DragPayload> {
        if index < self.items.len() {
            Some(self.items.remove(index))
        } else {
            None
        }
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn items(&self) -> &[DragPayload] {
        &self.items
    }

    /// Returns all items that match a specific payload type name.
    pub fn items_of_type(&self, type_name: &str) -> Vec<&DragPayload> {
        self.items.iter().filter(|p| p.payload_type() == type_name).collect()
    }

    /// Drain all items, returning them as a `Vec`.
    pub fn take_all(&mut self) -> Vec<DragPayload> {
        std::mem::take(&mut self.items)
    }
}

impl Default for MultiDragTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// DropEffectResolver – decide copy / move / link based on modifiers
// ---------------------------------------------------------------------------

/// Keyboard modifier state used to decide the drop effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModifierKeys {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

/// Resolves the [`DragEffect`] from the current modifier-key state and the
/// set of effects the source allows.
pub fn resolve_drop_effect(
    modifiers: &ModifierKeys,
    allowed: &[DragEffect],
) -> DragEffect {
    let preferred = if modifiers.ctrl && modifiers.shift {
        DragEffect::Link
    } else if modifiers.ctrl {
        DragEffect::Copy
    } else if modifiers.shift {
        DragEffect::Move
    } else if allowed.contains(&DragEffect::Move) {
        DragEffect::Move
    } else if allowed.contains(&DragEffect::Copy) {
        DragEffect::Copy
    } else {
        DragEffect::None
    };

    if allowed.contains(&preferred) {
        preferred
    } else {
        DragEffect::None
    }
}

// ---------------------------------------------------------------------------
// AutoScrollRegion – detect when cursor is near viewport edges
// ---------------------------------------------------------------------------

/// Describes which edge of the viewport the cursor is near.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollEdge {
    Top,
    Bottom,
    Left,
    Right,
}

/// Check whether a cursor position is within `margin` pixels of any edge of
/// the given viewport rectangle.  Returns the set of edges that are "hot".
pub fn detect_auto_scroll_edges(
    cursor_x: f64,
    cursor_y: f64,
    viewport: &Rect,
    margin: f64,
) -> Vec<ScrollEdge> {
    let mut edges = Vec::new();
    if cursor_y >= viewport.y && cursor_y <= viewport.y + margin {
        edges.push(ScrollEdge::Top);
    }
    if cursor_y >= viewport.y + viewport.height - margin
        && cursor_y <= viewport.y + viewport.height
    {
        edges.push(ScrollEdge::Bottom);
    }
    if cursor_x >= viewport.x && cursor_x <= viewport.x + margin {
        edges.push(ScrollEdge::Left);
    }
    if cursor_x >= viewport.x + viewport.width - margin
        && cursor_x <= viewport.x + viewport.width
    {
        edges.push(ScrollEdge::Right);
    }
    edges
}

// ---------------------------------------------------------------------------
// DndThreshold – drag activation threshold based on distance and time
// ---------------------------------------------------------------------------

/// Controls how far and how long a pointer must move before a drag begins.
pub struct DndThreshold {
    pub min_distance: f64,
    pub min_time_ms: u64,
}

impl DndThreshold {
    pub fn new(min_distance: f64, min_time_ms: u64) -> Self {
        Self {
            min_distance,
            min_time_ms,
        }
    }

    pub fn default() -> Self {
        Self::new(5.0, 150)
    }

    /// Returns `true` when both the Euclidean distance **and** elapsed time
    /// exceed the configured thresholds.
    pub fn is_exceeded(
        &self,
        start_x: f64,
        start_y: f64,
        current_x: f64,
        current_y: f64,
        elapsed_ms: u64,
    ) -> bool {
        self.distance_only(start_x, start_y, current_x, current_y)
            && elapsed_ms >= self.min_time_ms
    }

    pub fn distance_only(
        &self,
        start_x: f64,
        start_y: f64,
        current_x: f64,
        current_y: f64,
    ) -> bool {
        let dx = current_x - start_x;
        let dy = current_y - start_y;
        (dx * dx + dy * dy).sqrt() >= self.min_distance
    }
}

// ---------------------------------------------------------------------------
// DndAutoScroll – edge-proximity auto-scrolling
// ---------------------------------------------------------------------------

/// Computes scroll deltas when the cursor is near a viewport edge.
pub struct DndAutoScroll {
    pub margin: f64,
    pub speed: f64,
    pub active: bool,
}

impl DndAutoScroll {
    pub fn new(margin: f64, speed: f64) -> Self {
        Self {
            margin,
            speed,
            active: true,
        }
    }

    /// Returns `(dx, dy)` scroll deltas. Positive values scroll right / down.
    pub fn compute_scroll(
        &self,
        cursor_x: f64,
        cursor_y: f64,
        viewport: &Rect,
    ) -> (f64, f64) {
        if !self.active {
            return (0.0, 0.0);
        }

        let mut dx = 0.0;
        let mut dy = 0.0;

        // left edge
        if cursor_x < viewport.x + self.margin && cursor_x >= viewport.x {
            let ratio = 1.0 - (cursor_x - viewport.x) / self.margin;
            dx = -self.speed * ratio;
        }
        // right edge
        let right = viewport.x + viewport.width;
        if cursor_x > right - self.margin && cursor_x <= right {
            let ratio = 1.0 - (right - cursor_x) / self.margin;
            dx = self.speed * ratio;
        }
        // top edge
        if cursor_y < viewport.y + self.margin && cursor_y >= viewport.y {
            let ratio = 1.0 - (cursor_y - viewport.y) / self.margin;
            dy = -self.speed * ratio;
        }
        // bottom edge
        let bottom = viewport.y + viewport.height;
        if cursor_y > bottom - self.margin && cursor_y <= bottom {
            let ratio = 1.0 - (bottom - cursor_y) / self.margin;
            dy = self.speed * ratio;
        }

        (dx, dy)
    }

    pub fn is_near_edge(
        &self,
        cursor_x: f64,
        cursor_y: f64,
        viewport: &Rect,
    ) -> bool {
        let (dx, dy) = self.compute_scroll(cursor_x, cursor_y, viewport);
        dx.abs() > 0.0 || dy.abs() > 0.0
    }
}

// ---------------------------------------------------------------------------
// DndGhostRenderer – visual feedback positioning for drag ghosts
// ---------------------------------------------------------------------------

pub struct DndGhostRenderer {
    pub offset_x: f64,
    pub offset_y: f64,
    pub opacity: f64,
    pub label: String,
}

impl DndGhostRenderer {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            opacity: 0.7,
            label: label.into(),
        }
    }

    pub fn with_offset(mut self, x: f64, y: f64) -> Self {
        self.offset_x = x;
        self.offset_y = y;
        self
    }

    pub fn with_opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity;
        self
    }

    /// Computes the rendered position of the ghost relative to the cursor.
    pub fn render_position(&self, cursor_x: f64, cursor_y: f64) -> (f64, f64) {
        (cursor_x + self.offset_x, cursor_y + self.offset_y)
    }

    pub fn render_info(&self) -> String {
        format!(
            "Ghost '{}' offset=({}, {}) opacity={}",
            self.label, self.offset_x, self.offset_y, self.opacity
        )
    }
}

// ---------------------------------------------------------------------------
// DragCancellation – tracks cancelled drag state
// ---------------------------------------------------------------------------

pub struct DragCancellation {
    pub cancelled: bool,
    pub reason: Option<String>,
    pub cancel_count: u32,
}

impl DragCancellation {
    pub fn new() -> Self {
        Self {
            cancelled: false,
            reason: None,
            cancel_count: 0,
        }
    }

    pub fn cancel(&mut self, reason: impl Into<String>) {
        self.cancelled = true;
        self.reason = Some(reason.into());
        self.cancel_count += 1;
    }

    pub fn reset(&mut self) {
        self.cancelled = false;
        self.reason = None;
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn total_cancellations(&self) -> u32 {
        self.cancel_count
    }
}


// ---------------------------------------------------------------------------
// DragPreviewRenderer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DragPreviewRenderer {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl DragPreviewRenderer {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for DragPreviewRenderer {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for DragPreviewRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "DragPreviewRenderer({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// DropTargetValidator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DropTargetValidator {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl DropTargetValidator {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for DropTargetValidator {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for DropTargetValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "DropTargetValidator({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// DragPreviewRendererSnapshot — point-in-time snapshot of DragPreviewRenderer state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DragPreviewRendererSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl DragPreviewRendererSnapshot {
    pub fn capture(source: &DragPreviewRenderer, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for DragPreviewRendererSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// DropTargetValidatorStats — aggregate statistics for DropTargetValidator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct DropTargetValidatorStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl DropTargetValidatorStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for DropTargetValidatorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// DragPreviewRendererConfig — configuration for DragPreviewRenderer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DragPreviewRendererConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl DragPreviewRendererConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for DragPreviewRendererConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for DragPreviewRendererConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
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

    // --- extension tests ---

    #[test]
    fn drag_data_is_empty_and_file_helpers() {
        let empty = DragData::new("text/plain", vec![]);
        assert!(empty.is_empty());
        assert!(!empty.has_files());
        assert_eq!(empty.file_count(), 0);

        let files = DragData::files(&["file:///a.txt", "file:///b.txt", "file:///c.txt"]);
        assert!(!files.is_empty());
        assert!(files.has_files());
        assert_eq!(files.file_count(), 3);

        let single = DragData::files(&["file:///only.txt"]);
        assert_eq!(single.file_count(), 1);

        let text = DragData::text("hello");
        assert_eq!(text.file_count(), 0);
        assert!(!text.has_files());
    }

    #[test]
    fn drag_event_extensions() {
        let event = DragEvent {
            data: vec![DragData::text("a"), DragData::files(&["file:///x"])],
            source: Some("tab-1".into()),
        };
        assert!(event.has_data());
        assert_eq!(event.data_count(), 2);
        let mimes = event.mime_types();
        assert!(mimes.contains(&"text/plain"));
        assert!(mimes.contains(&"text/uri-list"));

        let empty_event = DragEvent { data: vec![], source: None };
        assert!(!empty_event.has_data());
        assert_eq!(empty_event.data_count(), 0);
    }

    #[test]
    fn drop_result_extensions() {
        assert!(DropResult::Accepted.is_success());
        assert!(!DropResult::Rejected.is_success());
        assert!(!DropResult::Cancelled.is_success());

        assert!(DropResult::Cancelled.is_cancelled());
        assert!(!DropResult::Accepted.is_cancelled());

        assert!(DropResult::Rejected.is_rejected());
        assert!(!DropResult::Accepted.is_rejected());
    }

    #[test]
    fn drag_effect_extensions() {
        assert!(DragEffect::Move.is_move());
        assert!(!DragEffect::Move.is_copy());
        assert!(DragEffect::Copy.is_copy());
        assert!(DragEffect::Link.is_link());
        assert!(DragEffect::None.is_none());

        assert_eq!(DragEffect::Copy.label(), "Copy");
        assert_eq!(DragEffect::Move.label(), "Move");
        assert_eq!(DragEffect::Link.label(), "Link");
        assert_eq!(DragEffect::None.label(), "None");
    }

    #[test]
    fn drag_history_extensions() {
        let mut history = DragHistory::default();
        assert!(history.most_recent().is_none());
        assert_eq!(history.accepted_count(), 0);
        assert_eq!(history.rejected_count(), 0);
        assert_eq!(history.iter().count(), 0);

        let target = DropTarget::from_position("file:///a.rs", 0, 0);
        history.record(HistoryEntry {
            data: vec![DragData::text("first")],
            target: target.clone(),
            result: DropResult::Accepted,
        });
        history.record(HistoryEntry {
            data: vec![DragData::text("second")],
            target: target.clone(),
            result: DropResult::Rejected,
        });
        history.record(HistoryEntry {
            data: vec![DragData::text("third")],
            target: target.clone(),
            result: DropResult::Accepted,
        });

        assert_eq!(history.most_recent().unwrap().result, DropResult::Accepted);
        assert_eq!(history.accepted_count(), 2);
        assert_eq!(history.rejected_count(), 1);
        assert_eq!(history.iter().count(), 3);
    }

    #[test]
    fn drag_metrics_display_and_extensions() {
        let mut history = DragHistory::default();
        let m = DragMetrics::from_history(&history);
        assert!(m.is_empty());
        assert_eq!(m.failure_rate(), 1.0);
        assert_eq!(format!("{m}"), "0 drops (0% success, 0.0 items/drop)");

        let target = DropTarget::from_position("file:///a.rs", 0, 0);
        history.record(HistoryEntry {
            data: vec![DragData::text("x"), DragData::text("y")],
            target: target.clone(),
            result: DropResult::Accepted,
        });
        history.record(HistoryEntry {
            data: vec![DragData::text("z")],
            target: target.clone(),
            result: DropResult::Rejected,
        });

        let m = DragMetrics::from_history(&history);
        assert!(!m.is_empty());
        assert!((m.failure_rate() - 0.5).abs() < f64::EPSILON);
        let display = format!("{m}");
        assert!(display.contains("2 drops"));
        assert!(display.contains("50%"));
    }

    #[test]
    fn drop_zone_registry_extensions() {
        let mut registry = DropZoneRegistry::new();
        assert_eq!(registry.zone_count(), 0);
        assert_eq!(registry.enabled_count(), 0);

        let mimes: HashSet<String> = ["text/plain"].iter().map(|s| s.to_string()).collect();
        registry.register(DropZone::new("a", mimes.clone()));
        registry.register(DropZone::new("b", mimes.clone()));
        let mut zone_c = DropZone::new("c", mimes);
        zone_c.enabled = false;
        registry.register(zone_c);

        assert_eq!(registry.zone_count(), 3);
        assert_eq!(registry.enabled_count(), 2);

        let mut ids = registry.zone_ids();
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c"]);

        registry.clear();
        assert_eq!(registry.zone_count(), 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn drag_data_transfer_extensions() {
        let mut transfer = DragDataTransfer::new();
        assert!(transfer.is_empty());
        assert_eq!(transfer.item_count(), 0);

        transfer.add_item(DragData::text("hello"));
        transfer.add_item(DragData::new("image/png", vec![0x89]));
        transfer.add_item(DragData::text("world"));

        assert!(!transfer.is_empty());
        assert_eq!(transfer.item_count(), 3);

        let mimes = transfer.mime_types();
        assert_eq!(mimes, vec!["image/png", "text/plain"]);
    }

    #[test]
    fn service_history_count_and_active_source() {
        let mut svc = DragAndDropService::new();
        assert_eq!(svc.history_count(), 0);
        assert!(svc.active_source().is_none());

        svc.start_drag_from("editor", vec![DragData::text("x")]).unwrap();
        assert_eq!(svc.active_source(), Some("editor"));

        let target = DropTarget::from_position("file:///a.rs", 0, 0);
        svc.try_drop_on(&target).unwrap();
        assert_eq!(svc.history_count(), 1);
        assert!(svc.active_source().is_none());
    }

    // --- Rect hit-testing tests ---

    #[test]
    fn rect_contains_and_edges() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        // inside
        assert!(r.contains(50.0, 40.0));
        // on top-left corner (inclusive)
        assert!(r.contains(10.0, 20.0));
        // on bottom-right corner (inclusive)
        assert!(r.contains(110.0, 70.0));
        // outside
        assert!(!r.contains(9.9, 20.0));
        assert!(!r.contains(50.0, 70.1));
    }

    #[test]
    fn rect_intersects() {
        let a = Rect::new(0.0, 0.0, 50.0, 50.0);
        let b = Rect::new(25.0, 25.0, 50.0, 50.0);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));

        let c = Rect::new(100.0, 100.0, 10.0, 10.0);
        assert!(!a.intersects(&c));
    }

    #[test]
    fn rect_center_and_display() {
        let r = Rect::new(0.0, 0.0, 100.0, 200.0);
        assert_eq!(r.center(), (50.0, 100.0));
        let s = format!("{r}");
        assert!(s.contains("Rect("));
    }

    // --- DragFeedbackState tests ---

    #[test]
    fn drag_feedback_state_transitions() {
        assert!(!DragFeedbackState::Pending.is_terminal());
        assert!(!DragFeedbackState::Dragging.is_terminal());
        assert!(DragFeedbackState::Dropped.is_terminal());
        assert!(DragFeedbackState::Cancelled.is_terminal());

        assert!(!DragFeedbackState::Pending.should_show_preview());
        assert!(DragFeedbackState::Dragging.should_show_preview());
        assert!(DragFeedbackState::OverValidTarget.should_show_preview());
        assert!(DragFeedbackState::OverInvalidTarget.should_show_preview());
        assert!(!DragFeedbackState::Dropped.should_show_preview());

        assert_eq!(format!("{}", DragFeedbackState::Dragging), "dragging");
    }

    // --- MultiDragTracker tests ---

    #[test]
    fn multi_drag_tracker_operations() {
        let mut tracker = MultiDragTracker::new();
        assert!(tracker.is_empty());

        tracker.add(DragPayload::Tab { tab_id: "t1".into(), group_id: 0 });
        tracker.add(DragPayload::Tab { tab_id: "t2".into(), group_id: 0 });
        tracker.add(DragPayload::File { uri: "file:///a.rs".into(), mime_type: "text/plain".into() });
        assert_eq!(tracker.count(), 3);

        let tabs = tracker.items_of_type("tab");
        assert_eq!(tabs.len(), 2);

        let removed = tracker.remove_by_index(0);
        assert!(removed.is_some());
        assert_eq!(tracker.count(), 2);
        assert!(tracker.remove_by_index(99).is_none());

        let all = tracker.take_all();
        assert_eq!(all.len(), 2);
        assert!(tracker.is_empty());
    }

    // --- resolve_drop_effect tests ---

    #[test]
    fn resolve_drop_effect_from_modifiers() {
        let all = vec![DragEffect::Copy, DragEffect::Move, DragEffect::Link];

        // no modifiers → default Move
        let mods = ModifierKeys::default();
        assert_eq!(resolve_drop_effect(&mods, &all), DragEffect::Move);

        // ctrl → Copy
        let mods = ModifierKeys { ctrl: true, ..Default::default() };
        assert_eq!(resolve_drop_effect(&mods, &all), DragEffect::Copy);

        // shift → Move
        let mods = ModifierKeys { shift: true, ..Default::default() };
        assert_eq!(resolve_drop_effect(&mods, &all), DragEffect::Move);

        // ctrl+shift → Link
        let mods = ModifierKeys { ctrl: true, shift: true, ..Default::default() };
        assert_eq!(resolve_drop_effect(&mods, &all), DragEffect::Link);

        // ctrl but Copy not allowed → None
        let mods = ModifierKeys { ctrl: true, ..Default::default() };
        assert_eq!(resolve_drop_effect(&mods, &[DragEffect::Move]), DragEffect::None);
    }

    // --- detect_auto_scroll_edges tests ---

    #[test]
    fn auto_scroll_edge_detection() {
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let margin = 20.0;

        // center → no edges
        let edges = detect_auto_scroll_edges(400.0, 300.0, &viewport, margin);
        assert!(edges.is_empty());

        // top-left corner → Top + Left
        let edges = detect_auto_scroll_edges(5.0, 5.0, &viewport, margin);
        assert!(edges.contains(&ScrollEdge::Top));
        assert!(edges.contains(&ScrollEdge::Left));

        // bottom edge
        let edges = detect_auto_scroll_edges(400.0, 595.0, &viewport, margin);
        assert!(edges.contains(&ScrollEdge::Bottom));
        assert!(!edges.contains(&ScrollEdge::Top));

        // right edge
        let edges = detect_auto_scroll_edges(790.0, 300.0, &viewport, margin);
        assert!(edges.contains(&ScrollEdge::Right));
    }

    // ------- DndThreshold tests -------

    #[test]
    fn test_threshold_not_exceeded() {
        let t = DndThreshold::new(10.0, 200);
        // moved only 3 pixels, not enough
        assert!(!t.is_exceeded(0.0, 0.0, 2.0, 2.0, 300));
    }

    #[test]
    fn test_threshold_exceeded() {
        let t = DndThreshold::new(5.0, 100);
        // distance ~14.1, time 150 – both exceed
        assert!(t.is_exceeded(0.0, 0.0, 10.0, 10.0, 150));
    }

    #[test]
    fn test_threshold_default() {
        let t = DndThreshold::default();
        assert_eq!(t.min_distance, 5.0);
        assert_eq!(t.min_time_ms, 150);
    }

    #[test]
    fn test_threshold_distance_only() {
        let t = DndThreshold::new(5.0, 500);
        // distance exceeded, time irrelevant for this method
        assert!(t.distance_only(0.0, 0.0, 10.0, 0.0));
        // distance NOT exceeded
        assert!(!t.distance_only(0.0, 0.0, 2.0, 0.0));
    }

    // ------- DndAutoScroll tests -------

    #[test]
    fn test_auto_scroll_center_no_scroll() {
        let scroller = DndAutoScroll::new(30.0, 10.0);
        let vp = Rect::new(0.0, 0.0, 800.0, 600.0);
        let (dx, dy) = scroller.compute_scroll(400.0, 300.0, &vp);
        assert_eq!(dx, 0.0);
        assert_eq!(dy, 0.0);
    }

    #[test]
    fn test_auto_scroll_near_left_edge() {
        let scroller = DndAutoScroll::new(30.0, 10.0);
        let vp = Rect::new(0.0, 0.0, 800.0, 600.0);
        let (dx, _dy) = scroller.compute_scroll(10.0, 300.0, &vp);
        // should scroll left (negative dx)
        assert!(dx < 0.0);
    }

    #[test]
    fn test_auto_scroll_near_bottom() {
        let scroller = DndAutoScroll::new(30.0, 10.0);
        let vp = Rect::new(0.0, 0.0, 800.0, 600.0);
        let (_dx, dy) = scroller.compute_scroll(400.0, 590.0, &vp);
        assert!(dy > 0.0);
    }

    #[test]
    fn test_auto_scroll_is_near_edge() {
        let scroller = DndAutoScroll::new(30.0, 10.0);
        let vp = Rect::new(0.0, 0.0, 800.0, 600.0);
        assert!(scroller.is_near_edge(5.0, 300.0, &vp));
        assert!(!scroller.is_near_edge(400.0, 300.0, &vp));
    }

    // ------- DndGhostRenderer tests -------

    #[test]
    fn test_ghost_renderer_default() {
        let g = DndGhostRenderer::new("item");
        assert_eq!(g.offset_x, 0.0);
        assert_eq!(g.offset_y, 0.0);
        assert_eq!(g.opacity, 0.7);
        assert_eq!(g.label, "item");
    }

    #[test]
    fn test_ghost_renderer_position() {
        let g = DndGhostRenderer::new("x").with_offset(5.0, -3.0);
        let (px, py) = g.render_position(100.0, 200.0);
        assert_eq!(px, 105.0);
        assert_eq!(py, 197.0);
    }

    #[test]
    fn test_ghost_renderer_info() {
        let g = DndGhostRenderer::new("drag-label")
            .with_offset(2.0, 4.0)
            .with_opacity(0.5);
        let info = g.render_info();
        assert!(info.contains("drag-label"));
        assert!(info.contains("0.5"));
    }

    // ------- DragCancellation tests -------

    #[test]
    fn test_cancellation_lifecycle() {
        let mut c = DragCancellation::new();
        assert!(!c.is_cancelled());
        assert_eq!(c.total_cancellations(), 0);

        c.cancel("escape pressed");
        assert!(c.is_cancelled());
        assert_eq!(c.reason.as_deref(), Some("escape pressed"));
        assert_eq!(c.total_cancellations(), 1);

        c.reset();
        assert!(!c.is_cancelled());
        assert!(c.reason.is_none());

        c.cancel("second cancel");
        assert_eq!(c.total_cancellations(), 2);
    }

    #[test] fn dragPreviewRenderer_new() { let s = DragPreviewRenderer::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn dragPreviewRenderer_add() { let mut s = DragPreviewRenderer::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn dragPreviewRenderer_remove() { let mut s = DragPreviewRenderer::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn dragPreviewRenderer_config() { let mut s = DragPreviewRenderer::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn dragPreviewRenderer_nav() { let mut s = DragPreviewRenderer::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn dragPreviewRenderer_filter() { let mut s = DragPreviewRenderer::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn dragPreviewRenderer_display() { assert!(format!("{}", DragPreviewRenderer::new()).contains("DragPreviewRenderer")); }
    #[test] fn dropTargetValidator_new() { let s = DropTargetValidator::new(); assert!(s.is_empty()); }
    #[test] fn dropTargetValidator_add() { let mut s = DropTargetValidator::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn dropTargetValidator_active() { let mut s = DropTargetValidator::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn dropTargetValidator_error() { let mut s = DropTargetValidator::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn dropTargetValidator_rm_group() { let mut s = DropTargetValidator::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn dropTargetValidator_display() { assert!(format!("{}", DropTargetValidator::new()).contains("DropTargetValidator")); }


    #[test] fn dragPreviewRenderer_snap_capture() {
        let s = DragPreviewRenderer::new();
        let snap = DragPreviewRendererSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn dragPreviewRenderer_snap_stale() {
        let s = DragPreviewRenderer::new();
        let snap = DragPreviewRendererSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn dragPreviewRenderer_snap_diff() {
        let s = DragPreviewRenderer::new();
        let s1v = DragPreviewRendererSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn dragPreviewRenderer_snap_display() {
        let s = DragPreviewRenderer::new();
        let snap = DragPreviewRendererSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn dropTargetValidator_stats_record() {
        let mut st = DropTargetValidatorStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn dropTargetValidator_stats_hit_ratio() {
        let mut st = DropTargetValidatorStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn dropTargetValidator_stats_merge() {
        let mut a = DropTargetValidatorStats::new();
        a.total_adds = 5;
        let mut b = DropTargetValidatorStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn dropTargetValidator_stats_display() {
        let st = DropTargetValidatorStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn dragPreviewRenderer_config_default() {
        let c = DragPreviewRendererConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn dragPreviewRenderer_config_builder() {
        let c = DragPreviewRendererConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn dragPreviewRenderer_config_labels() {
        let mut c = DragPreviewRendererConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn dragPreviewRenderer_config_cleanup_threshold() {
        let c = DragPreviewRendererConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn dragPreviewRenderer_config_display() {
        assert!(format!("{}", DragPreviewRendererConfig::new()).contains("Config"));
    }
    #[test] fn dropTargetValidator_stats_peaks() {
        let mut st = DropTargetValidatorStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

}
