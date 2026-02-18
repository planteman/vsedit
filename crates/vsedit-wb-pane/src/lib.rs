//! Sidebar pane container.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur during pane operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneError {
    PaneNotFound(String),
    DuplicatePane(String),
    InvalidSize { width: u32, height: u32 },
}

impl fmt::Display for PaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaneError::PaneNotFound(id) => write!(f, "pane not found: {id}"),
            PaneError::DuplicatePane(id) => write!(f, "duplicate pane: {id}"),
            PaneError::InvalidSize { width, height } => {
                write!(f, "invalid size: {width}x{height}")
            }
        }
    }
}

/// Location of a pane in the workbench layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneLocation {
    Editor,
    Panel,
    Sidebar,
    AuxiliaryBar,
}

impl fmt::Display for PaneLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaneLocation::Editor => write!(f, "Editor"),
            PaneLocation::Panel => write!(f, "Panel"),
            PaneLocation::Sidebar => write!(f, "Sidebar"),
            PaneLocation::AuxiliaryBar => write!(f, "AuxiliaryBar"),
        }
    }
}

/// Size constraints for a pane.
#[derive(Debug, Clone)]
pub struct PaneSize {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub min_width: u32,
    pub min_height: u32,
}

/// A single pane in the workbench.
#[derive(Debug, Clone)]
pub struct Pane {
    pub id: String,
    pub title: String,
    pub location: PaneLocation,
    pub size: PaneSize,
    pub visible: bool,
    pub maximized: bool,
}

impl fmt::Display for Pane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Pane({}, \"{}\", {}{}{})",
            self.id,
            self.title,
            self.location,
            if self.visible { "" } else { ", hidden" },
            if self.maximized { ", maximized" } else { "" },
        )
    }
}

/// Builder for constructing a [`Pane`] with defaults.
pub struct PaneBuilder {
    id: String,
    title: String,
    location: PaneLocation,
    width: Option<u32>,
    height: Option<u32>,
    min_width: u32,
    min_height: u32,
    visible: bool,
    maximized: bool,
}

impl PaneBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            title: id.clone(),
            id,
            location: PaneLocation::Sidebar,
            width: None,
            height: None,
            min_width: 100,
            min_height: 100,
            visible: true,
            maximized: false,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn location(mut self, location: PaneLocation) -> Self {
        self.location = location;
        self
    }

    pub fn width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn min_width(mut self, min_width: u32) -> Self {
        self.min_width = min_width;
        self
    }

    pub fn min_height(mut self, min_height: u32) -> Self {
        self.min_height = min_height;
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn maximized(mut self, maximized: bool) -> Self {
        self.maximized = maximized;
        self
    }

    pub fn build(self) -> Pane {
        Pane {
            id: self.id,
            title: self.title,
            location: self.location,
            size: PaneSize {
                width: self.width,
                height: self.height,
                min_width: self.min_width,
                min_height: self.min_height,
            },
            visible: self.visible,
            maximized: self.maximized,
        }
    }
}

/// Service for managing panes.
pub struct PaneService {
    panes: Vec<Pane>,
}

impl PaneService {
    pub fn new() -> Self {
        Self { panes: Vec::new() }
    }

    pub fn add_pane(&mut self, pane: Pane) {
        self.panes.push(pane);
    }

    pub fn remove_pane(&mut self, id: &str) -> bool {
        let len = self.panes.len();
        self.panes.retain(|p| p.id != id);
        self.panes.len() < len
    }

    pub fn toggle_visibility(&mut self, id: &str) {
        if let Some(p) = self.panes.iter_mut().find(|p| p.id == id) {
            p.visible = !p.visible;
        }
    }

    pub fn maximize(&mut self, id: &str) {
        if let Some(p) = self.panes.iter_mut().find(|p| p.id == id) {
            p.maximized = true;
        }
    }

    pub fn restore(&mut self, id: &str) {
        if let Some(p) = self.panes.iter_mut().find(|p| p.id == id) {
            p.maximized = false;
        }
    }

    pub fn get_panes_at(&self, location: PaneLocation) -> Vec<&Pane> {
        self.panes
            .iter()
            .filter(|p| p.location == location)
            .collect()
    }

    pub fn get_pane(&self, id: &str) -> Option<&Pane> {
        self.panes.iter().find(|p| p.id == id)
    }

    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// Add a pane, returning an error if a pane with the same id already exists.
    pub fn try_add_pane(&mut self, pane: Pane) -> Result<(), PaneError> {
        if self.panes.iter().any(|p| p.id == pane.id) {
            return Err(PaneError::DuplicatePane(pane.id));
        }
        self.panes.push(pane);
        Ok(())
    }

    /// Get a mutable reference to a pane by id.
    pub fn get_pane_mut(&mut self, id: &str) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    /// Return all currently visible panes.
    pub fn get_visible_panes(&self) -> Vec<&Pane> {
        self.panes.iter().filter(|p| p.visible).collect()
    }

    /// Move a pane to a new location.
    pub fn move_pane(&mut self, id: &str, location: PaneLocation) -> Result<(), PaneError> {
        let pane = self
            .panes
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| PaneError::PaneNotFound(id.to_string()))?;
        pane.location = location;
        Ok(())
    }

    /// Resize a pane. Returns an error if the pane is not found or the new
    /// dimensions are smaller than the pane's minimum size.
    pub fn resize_pane(&mut self, id: &str, width: u32, height: u32) -> Result<(), PaneError> {
        let pane = self
            .panes
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| PaneError::PaneNotFound(id.to_string()))?;
        if width < pane.size.min_width || height < pane.size.min_height {
            return Err(PaneError::InvalidSize { width, height });
        }
        pane.size.width = Some(width);
        pane.size.height = Some(height);
        Ok(())
    }

    /// Toggle the maximized state of a pane.
    pub fn toggle_maximized(&mut self, id: &str) -> Result<(), PaneError> {
        let pane = self
            .panes
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| PaneError::PaneNotFound(id.to_string()))?;
        pane.maximized = !pane.maximized;
        Ok(())
    }

    /// Find panes whose title contains the given substring (case-insensitive).
    pub fn find_by_title(&self, query: &str) -> Vec<&Pane> {
        let query_lower = query.to_lowercase();
        self.panes
            .iter()
            .filter(|p| p.title.to_lowercase().contains(&query_lower))
            .collect()
    }
}

impl Default for PaneService {
    fn default() -> Self {
        Self::new()
    }
}

/// A rectangle representing a region of the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rectangle {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rectangle {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    /// Total area of this rectangle.
    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

impl fmt::Display for Rectangle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rect({}, {}, {}x{})", self.x, self.y, self.width, self.height)
    }
}

/// Computes layout rectangles for a set of panes within a bounding area.
#[derive(Debug, Clone)]
pub struct PaneLayout {
    pub bounds: Rectangle,
}

impl PaneLayout {
    pub fn new(bounds: Rectangle) -> Self {
        Self { bounds }
    }

    /// Split the bounding rectangle horizontally (side by side) among visible panes.
    /// Returns one `Rectangle` per visible pane, in order.
    pub fn split_horizontal(&self, panes: &[Pane]) -> Vec<Rectangle> {
        let visible: Vec<&Pane> = panes.iter().filter(|p| p.visible).collect();
        let count = visible.len() as u32;
        if count == 0 {
            return Vec::new();
        }
        let each_width = self.bounds.width / count;
        let remainder = self.bounds.width % count;
        let mut rects = Vec::with_capacity(visible.len());
        let mut x = self.bounds.x;
        for (i, _) in visible.iter().enumerate() {
            let w = if (i as u32) < remainder { each_width + 1 } else { each_width };
            rects.push(Rectangle::new(x, self.bounds.y, w, self.bounds.height));
            x += w;
        }
        rects
    }

    /// Split the bounding rectangle vertically (stacked) among visible panes.
    /// Returns one `Rectangle` per visible pane, in order.
    pub fn split_vertical(&self, panes: &[Pane]) -> Vec<Rectangle> {
        let visible: Vec<&Pane> = panes.iter().filter(|p| p.visible).collect();
        let count = visible.len() as u32;
        if count == 0 {
            return Vec::new();
        }
        let each_height = self.bounds.height / count;
        let remainder = self.bounds.height % count;
        let mut rects = Vec::with_capacity(visible.len());
        let mut y = self.bounds.y;
        for (i, _) in visible.iter().enumerate() {
            let h = if (i as u32) < remainder { each_height + 1 } else { each_height };
            rects.push(Rectangle::new(self.bounds.x, y, self.bounds.width, h));
            y += h;
        }
        rects
    }
}

/// Sort order for panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSortOrder {
    Alphabetical,
    ByLocation,
    ByVisibility,
    Custom,
}

/// Sort a slice of panes in place according to the given order.
/// `Custom` leaves the slice unchanged.
pub fn sort_panes(panes: &mut [Pane], order: PaneSortOrder) {
    match order {
        PaneSortOrder::Alphabetical => panes.sort_by(|a, b| a.title.cmp(&b.title)),
        PaneSortOrder::ByLocation => {
            panes.sort_by_key(|p| match p.location {
                PaneLocation::Sidebar => 0,
                PaneLocation::Editor => 1,
                PaneLocation::Panel => 2,
                PaneLocation::AuxiliaryBar => 3,
            });
        }
        PaneSortOrder::ByVisibility => {
            panes.sort_by_key(|p| if p.visible { 0 } else { 1 });
        }
        PaneSortOrder::Custom => {}
    }
}

/// Serialize pane layout as a simple semicolon-separated string.
/// Each pane is represented as `id:location:visible`.
pub fn serialize_layout(panes: &[Pane]) -> String {
    panes
        .iter()
        .map(|p| format!("{}:{}:{}", p.id, p.location, p.visible))
        .collect::<Vec<_>>()
        .join(";")
}

/// Count panes grouped by their location.
pub fn count_by_location(panes: &[Pane]) -> HashMap<PaneLocation, usize> {
    let mut counts = HashMap::new();
    for p in panes {
        *counts.entry(p.location).or_insert(0) += 1;
    }
    counts
}

// Implement Hash for PaneLocation so it can be used as a HashMap key.
impl std::hash::Hash for PaneLocation {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

/// A named group of panes that share a common title/purpose.
#[derive(Debug, Clone)]
pub struct PaneGroup {
    pub title: String,
    pub pane_ids: Vec<String>,
}

impl PaneGroup {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            pane_ids: Vec::new(),
        }
    }

    pub fn add(&mut self, id: impl Into<String>) {
        self.pane_ids.push(id.into());
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.pane_ids.len();
        self.pane_ids.retain(|i| i != id);
        self.pane_ids.len() < len
    }

    pub fn contains(&self, id: &str) -> bool {
        self.pane_ids.iter().any(|i| i == id)
    }

    pub fn len(&self) -> usize {
        self.pane_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pane_ids.is_empty()
    }

    /// Resolve pane references against a `PaneService`, returning found panes.
    pub fn resolve<'a>(&self, service: &'a PaneService) -> Vec<&'a Pane> {
        self.pane_ids
            .iter()
            .filter_map(|id| service.get_pane(id))
            .collect()
    }
}

impl fmt::Display for PaneGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PaneGroup(\"{}\", {} panes)", self.title, self.pane_ids.len())
    }
}

/// A snapshot of all panes at a particular point in time.
#[derive(Debug, Clone)]
pub struct PaneSnapshot {
    pub panes: Vec<Pane>,
    pub label: String,
}

impl PaneSnapshot {
    /// Capture the current state of all panes in a service.
    pub fn capture(service: &PaneService, label: impl Into<String>) -> Self {
        Self {
            panes: service.panes.clone(),
            label: label.into(),
        }
    }

    /// Restore the snapshot into a service, replacing all existing panes.
    pub fn restore(self, service: &mut PaneService) {
        service.panes = self.panes;
    }

    /// Number of panes in the snapshot.
    pub fn len(&self) -> usize {
        self.panes.len()
    }

    /// Whether the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    /// Return pane ids in the snapshot.
    pub fn pane_ids(&self) -> Vec<&str> {
        self.panes.iter().map(|p| p.id.as_str()).collect()
    }
}

impl fmt::Display for PaneSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(\"{}\", {} panes)", self.label, self.panes.len())
    }
}

/// An iterator over panes with optional filtering.
pub struct PaneIterator<'a> {
    panes: &'a [Pane],
    index: usize,
    location_filter: Option<PaneLocation>,
    visible_only: bool,
}

impl<'a> PaneIterator<'a> {
    /// Create an iterator over all panes in a service.
    pub fn new(service: &'a PaneService) -> Self {
        Self {
            panes: &service.panes,
            index: 0,
            location_filter: None,
            visible_only: false,
        }
    }

    /// Filter to only panes at the given location.
    pub fn at_location(mut self, location: PaneLocation) -> Self {
        self.location_filter = Some(location);
        self
    }

    /// Filter to only visible panes.
    pub fn visible(mut self) -> Self {
        self.visible_only = true;
        self
    }
}

impl<'a> Iterator for PaneIterator<'a> {
    type Item = &'a Pane;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.panes.len() {
            let pane = &self.panes[self.index];
            self.index += 1;
            if let Some(loc) = self.location_filter {
                if pane.location != loc {
                    continue;
                }
            }
            if self.visible_only && !pane.visible {
                continue;
            }
            return Some(pane);
        }
        None
    }
}

/// Deserialize a layout string produced by [`serialize_layout`] back into panes.
///
/// Each segment has the format `id:Location:visible`. Unknown locations default
/// to `Sidebar`.
pub fn deserialize_layout(input: &str) -> Vec<Pane> {
    if input.is_empty() {
        return Vec::new();
    }
    input
        .split(';')
        .filter_map(|segment| {
            let parts: Vec<&str> = segment.split(':').collect();
            if parts.len() < 3 {
                return None;
            }
            let id = parts[0].to_string();
            let location = match parts[1] {
                "Editor" => PaneLocation::Editor,
                "Panel" => PaneLocation::Panel,
                "Sidebar" => PaneLocation::Sidebar,
                "AuxiliaryBar" => PaneLocation::AuxiliaryBar,
                _ => PaneLocation::Sidebar,
            };
            let visible = parts[2] == "true";
            Some(Pane {
                id: id.clone(),
                title: id,
                location,
                size: PaneSize {
                    width: None,
                    height: None,
                    min_width: 100,
                    min_height: 100,
                },
                visible,
                maximized: false,
            })
        })
        .collect()
}

/// Tracks a drag-and-drop reordering operation on panes.
#[derive(Debug, Clone)]
pub struct PaneDragDrop {
    /// The id of the pane being dragged.
    pub dragged_id: String,
    /// The original index in the pane list.
    pub origin_index: usize,
    /// The current hover/target index.
    pub target_index: Option<usize>,
    /// Whether the drop has been committed.
    pub committed: bool,
}

impl PaneDragDrop {
    /// Begin a drag operation for the pane at `origin_index`.
    pub fn begin(dragged_id: impl Into<String>, origin_index: usize) -> Self {
        Self {
            dragged_id: dragged_id.into(),
            origin_index,
            target_index: None,
            committed: false,
        }
    }

    /// Update the hover target.
    pub fn hover(&mut self, target_index: usize) {
        self.target_index = Some(target_index);
    }

    /// Cancel the drag operation.
    pub fn cancel(&mut self) {
        self.target_index = None;
        self.committed = false;
    }

    /// Commit the drag, applying the reorder to the given pane list.
    /// Returns `true` if the reorder was applied.
    pub fn commit(&mut self, panes: &mut Vec<Pane>) -> bool {
        let target = match self.target_index {
            Some(t) => t,
            None => return false,
        };
        if self.origin_index >= panes.len() || target >= panes.len() {
            return false;
        }
        if self.origin_index == target {
            self.committed = true;
            return true;
        }
        let pane = panes.remove(self.origin_index);
        let insert_at = if target > self.origin_index {
            target - 1
        } else {
            target
        };
        let insert_at = insert_at.min(panes.len());
        panes.insert(insert_at, pane);
        self.committed = true;
        true
    }

    /// Whether this drag moved the pane from its original position.
    pub fn did_move(&self) -> bool {
        self.committed && self.target_index.map_or(false, |t| t != self.origin_index)
    }
}

impl fmt::Display for PaneDragDrop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DragDrop({}, from={}, to={:?}, committed={})",
            self.dragged_id, self.origin_index, self.target_index, self.committed,
        )
    }
}

/// Direction for a resize operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    Horizontal,
    Vertical,
}

/// Manages resize operations on a pane.
#[derive(Debug, Clone)]
pub struct PaneResizeHandle {
    pub pane_id: String,
    pub direction: ResizeDirection,
    pub min_size: u32,
    pub max_size: u32,
    pub current_size: u32,
}

impl PaneResizeHandle {
    pub fn new(pane_id: impl Into<String>, direction: ResizeDirection, current_size: u32) -> Self {
        Self {
            pane_id: pane_id.into(),
            direction,
            min_size: 50,
            max_size: 1000,
            current_size,
        }
    }

    /// Applies delta clamped to min/max, returns new size.
    pub fn resize(&mut self, delta: i32) -> u32 {
        let new = (self.current_size as i64 + delta as i64).clamp(self.min_size as i64, self.max_size as i64) as u32;
        self.current_size = new;
        new
    }

    /// Sets to exact size clamped to min/max, returns new size.
    pub fn resize_to(&mut self, size: u32) -> u32 {
        let new = size.clamp(self.min_size, self.max_size);
        self.current_size = new;
        new
    }

    pub fn is_at_minimum(&self) -> bool {
        self.current_size <= self.min_size
    }

    pub fn is_at_maximum(&self) -> bool {
        self.current_size >= self.max_size
    }

    pub fn with_limits(mut self, min: u32, max: u32) -> Self {
        self.min_size = min;
        self.max_size = max;
        self
    }
}

/// Tracks navigation history across panes, supporting back/forward traversal.
#[derive(Debug, Clone)]
pub struct PaneHistory {
    entries: Vec<String>,
    cursor: usize,
    capacity: usize,
}

impl PaneHistory {
    /// Create a new history with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            capacity: capacity.max(1),
        }
    }

    /// Record a visit to the pane with the given id.
    /// Truncates any forward history beyond the current cursor.
    pub fn visit(&mut self, pane_id: impl Into<String>) {
        let id = pane_id.into();
        // Truncate forward history
        self.entries.truncate(self.cursor);
        self.entries.push(id);
        // Evict oldest if over capacity
        if self.entries.len() > self.capacity {
            let overflow = self.entries.len() - self.capacity;
            self.entries.drain(..overflow);
        }
        self.cursor = self.entries.len();
    }

    /// Navigate back, returning the previous pane id if available.
    pub fn back(&mut self) -> Option<&str> {
        if self.cursor > 1 {
            self.cursor -= 1;
            Some(&self.entries[self.cursor - 1])
        } else {
            None
        }
    }

    /// Navigate forward, returning the next pane id if available.
    pub fn forward(&mut self) -> Option<&str> {
        if self.cursor < self.entries.len() {
            let entry = &self.entries[self.cursor];
            self.cursor += 1;
            Some(entry)
        } else {
            None
        }
    }

    /// The pane id at the current cursor position, if any.
    pub fn current(&self) -> Option<&str> {
        if self.cursor > 0 && self.cursor <= self.entries.len() {
            Some(&self.entries[self.cursor - 1])
        } else {
            None
        }
    }

    /// Number of entries in the history.
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
        self.cursor = 0;
    }

    /// Whether a back navigation is possible.
    pub fn can_go_back(&self) -> bool {
        self.cursor > 1
    }

    /// Whether a forward navigation is possible.
    pub fn can_go_forward(&self) -> bool {
        self.cursor < self.entries.len()
    }
}

/// Direction in which a pane can be split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Result of splitting a pane into two regions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitResult {
    pub original: Rectangle,
    pub new_pane: Rectangle,
}

/// Splits a [`Rectangle`] in a given direction at a ratio (0.0–1.0).
/// The ratio determines how much space the original pane keeps.
pub fn split_rectangle(rect: Rectangle, direction: SplitDirection, ratio: f64) -> SplitResult {
    let ratio = ratio.clamp(0.1, 0.9);
    match direction {
        SplitDirection::Left | SplitDirection::Right => {
            let orig_w = (rect.width as f64 * ratio) as u32;
            let new_w = rect.width - orig_w;
            if direction == SplitDirection::Right {
                SplitResult {
                    original: Rectangle::new(rect.x, rect.y, orig_w, rect.height),
                    new_pane: Rectangle::new(rect.x + orig_w, rect.y, new_w, rect.height),
                }
            } else {
                SplitResult {
                    original: Rectangle::new(rect.x + new_w, rect.y, orig_w, rect.height),
                    new_pane: Rectangle::new(rect.x, rect.y, new_w, rect.height),
                }
            }
        }
        SplitDirection::Up | SplitDirection::Down => {
            let orig_h = (rect.height as f64 * ratio) as u32;
            let new_h = rect.height - orig_h;
            if direction == SplitDirection::Down {
                SplitResult {
                    original: Rectangle::new(rect.x, rect.y, rect.width, orig_h),
                    new_pane: Rectangle::new(rect.x, rect.y + orig_h, rect.width, new_h),
                }
            } else {
                SplitResult {
                    original: Rectangle::new(rect.x, rect.y + new_h, rect.width, orig_h),
                    new_pane: Rectangle::new(rect.x, rect.y, rect.width, new_h),
                }
            }
        }
    }
}

/// Manages a linear focus chain across panes, supporting next/previous cycling.
#[derive(Debug, Clone)]
pub struct PaneFocusChain {
    order: Vec<String>,
    active: Option<usize>,
}

impl PaneFocusChain {
    pub fn new() -> Self {
        Self {
            order: Vec::new(),
            active: None,
        }
    }

    /// Build a focus chain from the visible panes in a service.
    pub fn from_service(service: &PaneService) -> Self {
        let order: Vec<String> = service
            .panes
            .iter()
            .filter(|p| p.visible)
            .map(|p| p.id.clone())
            .collect();
        let active = if order.is_empty() { None } else { Some(0) };
        Self { order, active }
    }

    /// Move focus to the next pane in the chain, wrapping around.
    pub fn focus_next(&mut self) -> Option<&str> {
        if self.order.is_empty() {
            return None;
        }
        let idx = match self.active {
            Some(i) => (i + 1) % self.order.len(),
            None => 0,
        };
        self.active = Some(idx);
        Some(&self.order[idx])
    }

    /// Move focus to the previous pane in the chain, wrapping around.
    pub fn focus_prev(&mut self) -> Option<&str> {
        if self.order.is_empty() {
            return None;
        }
        let idx = match self.active {
            Some(0) => self.order.len() - 1,
            Some(i) => i - 1,
            None => self.order.len() - 1,
        };
        self.active = Some(idx);
        Some(&self.order[idx])
    }

    /// The currently focused pane id.
    pub fn current(&self) -> Option<&str> {
        self.active.map(|i| self.order[i].as_str())
    }

    /// Number of panes in the focus chain.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Whether the focus chain is empty.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Set focus to a specific pane id. Returns false if not in the chain.
    pub fn focus_on(&mut self, id: &str) -> bool {
        if let Some(idx) = self.order.iter().position(|s| s == id) {
            self.active = Some(idx);
            true
        } else {
            false
        }
    }
}

impl Default for PaneFocusChain {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PaneVisibilityPolicy – rules for automatic show/hide
// ---------------------------------------------------------------------------

/// Controls automatic visibility toggling of panes based on conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisibilityRule {
    /// Always visible.
    AlwaysVisible,
    /// Visible only when a file with the given extension is open.
    OnFileExtension(String),
    /// Visible only when the pane has content.
    WhenNotEmpty,
    /// Follow another pane's visibility.
    FollowPane(String),
}

/// Manages visibility policies for panes.
#[derive(Debug, Clone)]
pub struct PaneVisibilityPolicy {
    rules: HashMap<String, VisibilityRule>,
}

impl PaneVisibilityPolicy {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }

    /// Set a visibility rule for a pane.
    pub fn set_rule(&mut self, pane_id: impl Into<String>, rule: VisibilityRule) {
        self.rules.insert(pane_id.into(), rule);
    }

    /// Remove the rule for a pane.
    pub fn remove_rule(&mut self, pane_id: &str) {
        self.rules.remove(pane_id);
    }

    /// Get the rule for a pane, if any.
    pub fn rule_for(&self, pane_id: &str) -> Option<&VisibilityRule> {
        self.rules.get(pane_id)
    }

    /// Evaluate which panes should be visible given the current open file extension.
    pub fn evaluate_visibility(&self, open_extension: Option<&str>) -> HashMap<String, bool> {
        let mut result = HashMap::new();
        for (id, rule) in &self.rules {
            let visible = match rule {
                VisibilityRule::AlwaysVisible => true,
                VisibilityRule::OnFileExtension(ext) => {
                    open_extension.map_or(false, |oe| oe == ext.as_str())
                }
                VisibilityRule::WhenNotEmpty => true, // caller must check content
                VisibilityRule::FollowPane(other_id) => {
                    // Follow the resolved visibility of the other pane, default true.
                    result.get(other_id).copied().unwrap_or(true)
                }
            };
            result.insert(id.clone(), visible);
        }
        result
    }

    /// Number of registered rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for PaneVisibilityPolicy {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PaneStats – aggregated pane service statistics
// ---------------------------------------------------------------------------

/// Summary statistics about panes in a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneStats {
    pub total: usize,
    pub visible: usize,
    pub hidden: usize,
    pub maximized: usize,
    pub by_location: HashMap<PaneLocation, usize>,
}

impl PaneStats {
    /// Compute statistics from a [`PaneService`].
    pub fn from_service(service: &PaneService) -> Self {
        let total = service.pane_count();
        let visible = service.panes.iter().filter(|p| p.visible).count();
        let hidden = total - visible;
        let maximized = service.panes.iter().filter(|p| p.maximized).count();
        let by_location = count_by_location(&service.panes);
        Self {
            total,
            visible,
            hidden,
            maximized,
            by_location,
        }
    }

    /// Fraction of panes that are visible.
    pub fn visibility_ratio(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.visible as f64 / self.total as f64
    }
}

impl fmt::Display for PaneStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PaneStats(total={}, visible={}, hidden={}, maximized={})",
            self.total, self.visible, self.hidden, self.maximized,
        )
    }
}

/// Swaps the titles and visible status of two panes in the service.
pub fn pane_swap(service: &mut PaneService, id_a: &str, id_b: &str) -> Result<(), String> {
    let idx_a = service
        .panes
        .iter()
        .position(|p| p.id == id_a)
        .ok_or_else(|| format!("pane not found: {id_a}"))?;
    let idx_b = service
        .panes
        .iter()
        .position(|p| p.id == id_b)
        .ok_or_else(|| format!("pane not found: {id_b}"))?;

    let title_a = service.panes[idx_a].title.clone();
    let visible_a = service.panes[idx_a].visible;

    service.panes[idx_a].title = service.panes[idx_b].title.clone();
    service.panes[idx_a].visible = service.panes[idx_b].visible;

    service.panes[idx_b].title = title_a;
    service.panes[idx_b].visible = visible_a;

    Ok(())
}

// ---------------------------------------------------------------------------
// PaneGrid – 2D grid layout for neighbor detection and navigation
// ---------------------------------------------------------------------------

/// A cell in a pane grid, mapping a pane id to its bounding rectangle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridCell {
    pub pane_id: String,
    pub bounds: Rectangle,
}

/// Cardinal direction for pane navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// A 2D grid of pane cells, supporting neighbor detection and navigation.
#[derive(Debug, Clone)]
pub struct PaneGrid {
    cells: Vec<GridCell>,
}

impl PaneGrid {
    pub fn new() -> Self {
        Self { cells: Vec::new() }
    }

    /// Build a grid from a layout and a list of panes using horizontal splitting.
    pub fn from_horizontal_layout(layout: &PaneLayout, panes: &[Pane]) -> Self {
        let rects = layout.split_horizontal(panes);
        let visible: Vec<&Pane> = panes.iter().filter(|p| p.visible).collect();
        let cells = visible
            .iter()
            .zip(rects.iter())
            .map(|(p, r)| GridCell {
                pane_id: p.id.clone(),
                bounds: *r,
            })
            .collect();
        Self { cells }
    }

    /// Build a grid from a layout and a list of panes using vertical splitting.
    pub fn from_vertical_layout(layout: &PaneLayout, panes: &[Pane]) -> Self {
        let rects = layout.split_vertical(panes);
        let visible: Vec<&Pane> = panes.iter().filter(|p| p.visible).collect();
        let cells = visible
            .iter()
            .zip(rects.iter())
            .map(|(p, r)| GridCell {
                pane_id: p.id.clone(),
                bounds: *r,
            })
            .collect();
        Self { cells }
    }

    /// Add a cell manually.
    pub fn add_cell(&mut self, pane_id: impl Into<String>, bounds: Rectangle) {
        self.cells.push(GridCell {
            pane_id: pane_id.into(),
            bounds,
        });
    }

    /// Number of cells in the grid.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether the grid is empty.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Get the cell for a given pane id.
    pub fn cell_for(&self, pane_id: &str) -> Option<&GridCell> {
        self.cells.iter().find(|c| c.pane_id == pane_id)
    }

    /// Find the neighbor of a pane in the given direction.
    ///
    /// Neighbor detection uses the center point of each cell. For a given
    /// direction, we look for the closest cell whose center is strictly in
    /// that direction from the source cell's center.
    pub fn neighbor(&self, pane_id: &str, direction: Direction) -> Option<&str> {
        let source = self.cell_for(pane_id)?;
        let src_cx = source.bounds.x as i64 + source.bounds.width as i64 / 2;
        let src_cy = source.bounds.y as i64 + source.bounds.height as i64 / 2;

        let mut best: Option<(&GridCell, i64)> = None;

        for cell in &self.cells {
            if cell.pane_id == pane_id {
                continue;
            }
            let cx = cell.bounds.x as i64 + cell.bounds.width as i64 / 2;
            let cy = cell.bounds.y as i64 + cell.bounds.height as i64 / 2;

            let qualifies = match direction {
                Direction::Right => cx > src_cx,
                Direction::Left => cx < src_cx,
                Direction::Down => cy > src_cy,
                Direction::Up => cy < src_cy,
            };

            if !qualifies {
                continue;
            }

            let dist = (cx - src_cx).abs() + (cy - src_cy).abs();
            if best.map_or(true, |(_, d)| dist < d) {
                best = Some((cell, dist));
            }
        }

        best.map(|(cell, _)| cell.pane_id.as_str())
    }

    /// Return all pane ids in the grid.
    pub fn pane_ids(&self) -> Vec<&str> {
        self.cells.iter().map(|c| c.pane_id.as_str()).collect()
    }

    /// Validate that no cells overlap. Returns the ids of overlapping pairs.
    pub fn find_overlaps(&self) -> Vec<(&str, &str)> {
        let mut overlaps = Vec::new();
        for i in 0..self.cells.len() {
            for j in (i + 1)..self.cells.len() {
                if rects_overlap(&self.cells[i].bounds, &self.cells[j].bounds) {
                    overlaps.push((
                        self.cells[i].pane_id.as_str(),
                        self.cells[j].pane_id.as_str(),
                    ));
                }
            }
        }
        overlaps
    }

    /// Validate that all cells fit within the given container bounds.
    pub fn all_within(&self, container: &Rectangle) -> bool {
        self.cells.iter().all(|c| rect_within(&c.bounds, container))
    }
}

impl Default for PaneGrid {
    fn default() -> Self {
        Self::new()
    }
}

/// Check whether two rectangles overlap (share any interior area).
fn rects_overlap(a: &Rectangle, b: &Rectangle) -> bool {
    let a_right = a.x + a.width;
    let b_right = b.x + b.width;
    let a_bottom = a.y + a.height;
    let b_bottom = b.y + b.height;

    a.x < b_right && b.x < a_right && a.y < b_bottom && b.y < a_bottom
}

/// Check whether `inner` is fully contained within `outer`.
fn rect_within(inner: &Rectangle, outer: &Rectangle) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

// ---------------------------------------------------------------------------
// Proportional resize – resize panes while maintaining proportions
// ---------------------------------------------------------------------------

/// Distributes a total size among `count` items proportionally to `weights`.
/// Each weight is a positive f64. Returns a vector of integer sizes that sum
/// to exactly `total`.
pub fn distribute_proportional(total: u32, weights: &[f64]) -> Vec<u32> {
    if weights.is_empty() {
        return Vec::new();
    }
    let weight_sum: f64 = weights.iter().sum();
    if weight_sum <= 0.0 {
        let each = total / weights.len() as u32;
        let mut result = vec![each; weights.len()];
        let remainder = total - each * weights.len() as u32;
        for r in result.iter_mut().take(remainder as usize) {
            *r += 1;
        }
        return result;
    }

    let mut sizes: Vec<u32> = weights
        .iter()
        .map(|w| ((w / weight_sum) * total as f64).floor() as u32)
        .collect();

    let assigned: u32 = sizes.iter().sum();
    let mut remainder = total.saturating_sub(assigned);

    // Distribute remaining pixels to the items with the largest fractional parts.
    let mut fractionals: Vec<(usize, f64)> = weights
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let exact = (w / weight_sum) * total as f64;
            (i, exact - exact.floor())
        })
        .collect();
    fractionals.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (idx, _) in fractionals {
        if remainder == 0 {
            break;
        }
        sizes[idx] += 1;
        remainder -= 1;
    }

    sizes
}

/// Resize a list of pane rectangles horizontally so they fill `total_width`,
/// keeping their current proportions. Returns the new rectangles.
pub fn resize_horizontal_proportional(rects: &[Rectangle], total_width: u32) -> Vec<Rectangle> {
    if rects.is_empty() {
        return Vec::new();
    }
    let weights: Vec<f64> = rects.iter().map(|r| r.width as f64).collect();
    let widths = distribute_proportional(total_width, &weights);

    let mut result = Vec::with_capacity(rects.len());
    let mut x = rects.first().map_or(0, |r| r.x);
    for (r, &w) in rects.iter().zip(widths.iter()) {
        result.push(Rectangle::new(x, r.y, w, r.height));
        x += w;
    }
    result
}

/// Resize a list of pane rectangles vertically so they fill `total_height`,
/// keeping their current proportions. Returns the new rectangles.
pub fn resize_vertical_proportional(rects: &[Rectangle], total_height: u32) -> Vec<Rectangle> {
    if rects.is_empty() {
        return Vec::new();
    }
    let weights: Vec<f64> = rects.iter().map(|r| r.height as f64).collect();
    let heights = distribute_proportional(total_height, &weights);

    let mut result = Vec::with_capacity(rects.len());
    let mut y = rects.first().map_or(0, |r| r.y);
    for (r, &h) in rects.iter().zip(heights.iter()) {
        result.push(Rectangle::new(r.x, y, r.width, h));
        y += h;
    }
    result
}

// ---------------------------------------------------------------------------
// PaneSizeConstraints – validate and enforce min/max size limits
// ---------------------------------------------------------------------------

/// Enforces min/max width and height constraints on pane sizes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSizeConstraints {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl PaneSizeConstraints {
    pub fn new(min_width: u32, max_width: u32, min_height: u32, max_height: u32) -> Self {
        Self {
            min_width: min_width.min(max_width),
            max_width: max_width.max(min_width),
            min_height: min_height.min(max_height),
            max_height: max_height.max(min_height),
        }
    }

    /// Clamp width and height to these constraints.
    pub fn clamp(&self, width: u32, height: u32) -> (u32, u32) {
        (
            width.clamp(self.min_width, self.max_width),
            height.clamp(self.min_height, self.max_height),
        )
    }

    /// Check whether the given dimensions satisfy these constraints.
    pub fn satisfies(&self, width: u32, height: u32) -> bool {
        width >= self.min_width
            && width <= self.max_width
            && height >= self.min_height
            && height <= self.max_height
    }

    /// Apply constraints to a Rectangle, clamping its width and height.
    pub fn clamp_rect(&self, rect: &Rectangle) -> Rectangle {
        let (w, h) = self.clamp(rect.width, rect.height);
        Rectangle::new(rect.x, rect.y, w, h)
    }
}

impl Default for PaneSizeConstraints {
    fn default() -> Self {
        Self::new(50, 2000, 50, 2000)
    }
}

// ---------------------------------------------------------------------------
// Layout validation
// ---------------------------------------------------------------------------

/// Errors found during layout validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutValidationError {
    /// Two panes overlap.
    Overlap { a: String, b: String },
    /// A pane extends beyond the container.
    OutOfBounds { pane_id: String },
    /// A pane is below minimum size.
    BelowMinSize { pane_id: String, width: u32, height: u32 },
    /// No panes in the layout.
    Empty,
}

/// Validate a grid layout against a container and minimum size.
pub fn validate_layout(
    grid: &PaneGrid,
    container: &Rectangle,
    min_width: u32,
    min_height: u32,
) -> Vec<LayoutValidationError> {
    let mut errors = Vec::new();

    if grid.is_empty() {
        errors.push(LayoutValidationError::Empty);
        return errors;
    }

    // Check overlaps
    for (a, b) in grid.find_overlaps() {
        errors.push(LayoutValidationError::Overlap {
            a: a.to_string(),
            b: b.to_string(),
        });
    }

    // Check bounds and minimum sizes
    for cell in &grid.cells {
        if !rect_within(&cell.bounds, container) {
            errors.push(LayoutValidationError::OutOfBounds {
                pane_id: cell.pane_id.clone(),
            });
        }
        if cell.bounds.width < min_width || cell.bounds.height < min_height {
            errors.push(LayoutValidationError::BelowMinSize {
                pane_id: cell.pane_id.clone(),
                width: cell.bounds.width,
                height: cell.bounds.height,
            });
        }
    }

    errors
}


// === Pane Resize Constraints ===

/// Pane Resize Constraints implementation.
#[derive(Debug, Clone)]
pub struct PaneResizeConstraints {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: PaneResizeConstraintsStats,
}

/// Statistics for PaneResizeConstraints.
#[derive(Debug, Clone, Default)]
pub struct PaneResizeConstraintsStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl PaneResizeConstraintsStats {
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

impl PaneResizeConstraints {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: PaneResizeConstraintsStats::default(),
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

    pub fn stats(&self) -> &PaneResizeConstraintsStats {
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

impl Default for PaneResizeConstraints {
    fn default() -> Self {
        Self::new()
    }
}

// === Pane Drag Handler ===

/// Priority level for PaneDragHandler items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PaneDragHandlerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl PaneDragHandlerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for PaneDragHandlerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Pane Drag Handler implementation.
#[derive(Debug, Clone)]
pub struct PaneDragHandler {
    items: Vec<PaneDragHandlerItem>,
    max_items: usize,
    default_priority: PaneDragHandlerPriority,
}

/// A single item in PaneDragHandler.
#[derive(Debug, Clone)]
pub struct PaneDragHandlerItem {
    pub id: String,
    pub label: String,
    pub priority: PaneDragHandlerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl PaneDragHandlerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: PaneDragHandlerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: PaneDragHandlerPriority) -> Self {
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

impl PaneDragHandler {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: PaneDragHandlerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: PaneDragHandlerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<PaneDragHandlerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&PaneDragHandlerItem> {
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

    pub fn by_priority(&self, priority: PaneDragHandlerPriority) -> Vec<&PaneDragHandlerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&PaneDragHandlerItem> {
        let mut sorted: Vec<&PaneDragHandlerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&PaneDragHandlerItem> {
        let mut sorted: Vec<&PaneDragHandlerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&PaneDragHandlerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: PaneDragHandlerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> PaneDragHandlerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &PaneDragHandlerItem> {
        self.items.iter()
    }
}

impl Default for PaneDragHandler {
    fn default() -> Self {
        Self::new()
    }
}


/// Configuration manager for wb_pane functionality.
pub struct WbPaneConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl WbPaneConfig {
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

    pub fn merge(&mut self, other: &WbPaneConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for wb_pane operations.
pub struct WbPaneRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl WbPaneRateTracker {
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

/// Validation result collector for wb_pane.
pub struct WbPaneValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl WbPaneValidator {
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

    pub fn merge(&mut self, other: &WbPaneValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Bottom pane panel management — extended utilities (qd)
// ---------------------------------------------------------------------------

/// Metric accumulator for wb_pane operations.
#[derive(Debug, Clone)]
pub struct QdMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QdMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for wb_pane.
#[derive(Debug, Clone)]
pub struct QdRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QdRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for wb_pane lookups.
#[derive(Debug, Clone)]
pub struct QdLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QdLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for wb_pane
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbPaneRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbPaneRingBuf {
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
pub struct XaWbPaneCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbPaneCounter {
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

impl Default for XaWbPaneCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: &str, loc: PaneLocation) -> Pane {
        Pane {
            id: id.to_string(),
            title: id.to_string(),
            location: loc,
            size: PaneSize {
                width: None,
                height: None,
                min_width: 100,
                min_height: 100,
            },
            visible: true,
            maximized: false,
        }
    }

    #[test]
    fn add_and_remove() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("p1", PaneLocation::Editor));
        assert_eq!(svc.pane_count(), 1);
        assert!(svc.remove_pane("p1"));
        assert!(!svc.remove_pane("p1"));
    }

    #[test]
    fn toggle_and_maximize() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("p1", PaneLocation::Sidebar));
        svc.toggle_visibility("p1");
        assert!(!svc.get_pane("p1").unwrap().visible);
        svc.maximize("p1");
        assert!(svc.get_pane("p1").unwrap().maximized);
        svc.restore("p1");
        assert!(!svc.get_pane("p1").unwrap().maximized);
    }

    #[test]
    fn get_panes_at_location() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("p1", PaneLocation::Panel));
        svc.add_pane(pane("p2", PaneLocation::Editor));
        svc.add_pane(pane("p3", PaneLocation::Panel));
        assert_eq!(svc.get_panes_at(PaneLocation::Panel).len(), 2);
        assert_eq!(svc.get_panes_at(PaneLocation::Editor).len(), 1);
    }

    #[test]
    fn try_add_pane_rejects_duplicate() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("p1", PaneLocation::Editor));
        let result = svc.try_add_pane(pane("p1", PaneLocation::Panel));
        assert_eq!(result, Err(PaneError::DuplicatePane("p1".to_string())));
        assert_eq!(svc.pane_count(), 1);
    }

    #[test]
    fn try_add_pane_succeeds_for_unique_id() {
        let mut svc = PaneService::new();
        assert!(svc.try_add_pane(pane("p1", PaneLocation::Editor)).is_ok());
        assert!(svc.try_add_pane(pane("p2", PaneLocation::Editor)).is_ok());
        assert_eq!(svc.pane_count(), 2);
    }

    #[test]
    fn get_pane_mut_updates_title() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("p1", PaneLocation::Sidebar));
        svc.get_pane_mut("p1").unwrap().title = "New Title".to_string();
        assert_eq!(svc.get_pane("p1").unwrap().title, "New Title");
    }

    #[test]
    fn get_pane_mut_returns_none() {
        let mut svc = PaneService::new();
        assert!(svc.get_pane_mut("missing").is_none());
    }

    #[test]
    fn get_visible_panes_filters() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("p1", PaneLocation::Editor));
        svc.add_pane(pane("p2", PaneLocation::Editor));
        svc.toggle_visibility("p2");
        let visible = svc.get_visible_panes();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "p1");
    }

    #[test]
    fn move_pane_changes_location() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("p1", PaneLocation::Editor));
        assert!(svc.move_pane("p1", PaneLocation::Panel).is_ok());
        assert_eq!(svc.get_pane("p1").unwrap().location, PaneLocation::Panel);
    }

    #[test]
    fn move_pane_not_found() {
        let mut svc = PaneService::new();
        assert_eq!(
            svc.move_pane("missing", PaneLocation::Panel),
            Err(PaneError::PaneNotFound("missing".to_string()))
        );
    }

    #[test]
    fn resize_pane_success_and_invalid() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("p1", PaneLocation::Editor));
        assert!(svc.resize_pane("p1", 200, 200).is_ok());
        assert_eq!(svc.get_pane("p1").unwrap().size.width, Some(200));
        assert_eq!(svc.get_pane("p1").unwrap().size.height, Some(200));
        // Below minimum
        assert_eq!(
            svc.resize_pane("p1", 50, 50),
            Err(PaneError::InvalidSize {
                width: 50,
                height: 50
            })
        );
    }

    #[test]
    fn toggle_maximized_flips_state() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("p1", PaneLocation::Editor));
        assert!(svc.toggle_maximized("p1").is_ok());
        assert!(svc.get_pane("p1").unwrap().maximized);
        assert!(svc.toggle_maximized("p1").is_ok());
        assert!(!svc.get_pane("p1").unwrap().maximized);
    }

    #[test]
    fn toggle_maximized_not_found() {
        let mut svc = PaneService::new();
        assert!(svc.toggle_maximized("missing").is_err());
    }

    #[test]
    fn pane_builder_defaults_and_custom() {
        let p = PaneBuilder::new("b1")
            .title("My Pane")
            .location(PaneLocation::Panel)
            .width(300)
            .height(400)
            .min_width(50)
            .min_height(50)
            .visible(false)
            .maximized(true)
            .build();
        assert_eq!(p.id, "b1");
        assert_eq!(p.title, "My Pane");
        assert_eq!(p.location, PaneLocation::Panel);
        assert_eq!(p.size.width, Some(300));
        assert_eq!(p.size.height, Some(400));
        assert_eq!(p.size.min_width, 50);
        assert_eq!(p.size.min_height, 50);
        assert!(!p.visible);
        assert!(p.maximized);

        // Default builder
        let p2 = PaneBuilder::new("b2").build();
        assert_eq!(p2.title, "b2");
        assert_eq!(p2.location, PaneLocation::Sidebar);
        assert!(p2.visible);
        assert!(!p2.maximized);
    }

    #[test]
    fn find_by_title_case_insensitive() {
        let mut svc = PaneService::new();
        let mut p1 = pane("p1", PaneLocation::Editor);
        p1.title = "File Explorer".to_string();
        let mut p2 = pane("p2", PaneLocation::Panel);
        p2.title = "Terminal Output".to_string();
        let mut p3 = pane("p3", PaneLocation::Sidebar);
        p3.title = "Search Files".to_string();
        svc.add_pane(p1);
        svc.add_pane(p2);
        svc.add_pane(p3);

        let results = svc.find_by_title("file");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|p| p.id == "p1"));
        assert!(results.iter().any(|p| p.id == "p3"));

        assert!(svc.find_by_title("zzz").is_empty());
    }

    #[test]
    fn display_impls() {
        assert_eq!(format!("{}", PaneLocation::Editor), "Editor");
        assert_eq!(format!("{}", PaneLocation::AuxiliaryBar), "AuxiliaryBar");

        let p = pane("x", PaneLocation::Sidebar);
        let s = format!("{p}");
        assert!(s.contains("x"));
        assert!(s.contains("Sidebar"));

        assert_eq!(
            format!("{}", PaneError::PaneNotFound("abc".into())),
            "pane not found: abc"
        );
        assert_eq!(
            format!("{}", PaneError::DuplicatePane("abc".into())),
            "duplicate pane: abc"
        );
        assert_eq!(
            format!("{}", PaneError::InvalidSize { width: 1, height: 2 }),
            "invalid size: 1x2"
        );
    }

    // ---- New tests ----

    #[test]
    fn rectangle_area_and_display() {
        let r = Rectangle::new(10, 20, 300, 400);
        assert_eq!(r.area(), 120_000);
        assert_eq!(format!("{r}"), "Rect(10, 20, 300x400)");
    }

    #[test]
    fn layout_split_horizontal() {
        let layout = PaneLayout::new(Rectangle::new(0, 0, 900, 600));
        let panes = vec![
            pane("a", PaneLocation::Editor),
            pane("b", PaneLocation::Editor),
            pane("c", PaneLocation::Editor),
        ];
        let rects = layout.split_horizontal(&panes);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].x, 0);
        assert_eq!(rects[1].x, 300);
        assert_eq!(rects[2].x, 600);
        assert_eq!(rects[0].width + rects[1].width + rects[2].width, 900);
    }

    #[test]
    fn layout_split_vertical_skips_hidden() {
        let layout = PaneLayout::new(Rectangle::new(0, 0, 800, 600));
        let mut p2 = pane("b", PaneLocation::Editor);
        p2.visible = false;
        let panes = vec![pane("a", PaneLocation::Editor), p2];
        let rects = layout.split_vertical(&panes);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].height, 600);
    }

    #[test]
    fn layout_split_empty() {
        let layout = PaneLayout::new(Rectangle::new(0, 0, 100, 100));
        let panes: Vec<Pane> = Vec::new();
        assert!(layout.split_horizontal(&panes).is_empty());
        assert!(layout.split_vertical(&panes).is_empty());
    }

    #[test]
    fn sort_panes_alphabetical() {
        let mut panes = vec![
            pane("c", PaneLocation::Editor),
            pane("a", PaneLocation::Editor),
            pane("b", PaneLocation::Editor),
        ];
        sort_panes(&mut panes, PaneSortOrder::Alphabetical);
        assert_eq!(panes[0].id, "a");
        assert_eq!(panes[1].id, "b");
        assert_eq!(panes[2].id, "c");
    }

    #[test]
    fn sort_panes_by_visibility() {
        let mut hidden = pane("h", PaneLocation::Editor);
        hidden.visible = false;
        let mut panes = vec![hidden, pane("v", PaneLocation::Editor)];
        sort_panes(&mut panes, PaneSortOrder::ByVisibility);
        assert!(panes[0].visible);
        assert!(!panes[1].visible);
    }

    #[test]
    fn serialize_and_count_by_location() {
        let panes = vec![
            pane("p1", PaneLocation::Editor),
            pane("p2", PaneLocation::Panel),
            pane("p3", PaneLocation::Editor),
        ];
        let s = serialize_layout(&panes);
        assert!(s.contains("p1:Editor:true"));
        assert!(s.contains("p2:Panel:true"));
        assert!(s.contains(';'));

        let counts = count_by_location(&panes);
        assert_eq!(counts[&PaneLocation::Editor], 2);
        assert_eq!(counts[&PaneLocation::Panel], 1);
        assert_eq!(counts.get(&PaneLocation::Sidebar), None);
    }

    #[test]
    fn pane_group_operations() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("g1", PaneLocation::Sidebar));
        svc.add_pane(pane("g2", PaneLocation::Sidebar));

        let mut group = PaneGroup::new("My Group");
        assert!(group.is_empty());
        group.add("g1");
        group.add("g2");
        group.add("missing");
        assert_eq!(group.len(), 3);
        assert!(group.contains("g1"));
        assert!(group.remove("missing"));
        assert!(!group.contains("missing"));

        let resolved = group.resolve(&svc);
        assert_eq!(resolved.len(), 2);
        assert_eq!(format!("{group}"), "PaneGroup(\"My Group\", 2 panes)");
    }

    #[test]
    fn snapshot_capture_and_restore() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("s1", PaneLocation::Editor));
        svc.add_pane(pane("s2", PaneLocation::Panel));
        let snap = PaneSnapshot::capture(&svc, "before-change");
        assert_eq!(snap.len(), 2);
        assert_eq!(snap.label, "before-change");
        assert_eq!(snap.pane_ids(), vec!["s1", "s2"]);

        svc.add_pane(pane("s3", PaneLocation::Sidebar));
        assert_eq!(svc.pane_count(), 3);

        snap.restore(&mut svc);
        assert_eq!(svc.pane_count(), 2);
        assert!(svc.get_pane("s3").is_none());
        assert_eq!(format!("{}", PaneSnapshot::capture(&svc, "x")), "Snapshot(\"x\", 2 panes)");
    }

    #[test]
    fn pane_iterator_filters() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("i1", PaneLocation::Editor));
        svc.add_pane(pane("i2", PaneLocation::Panel));
        let mut hidden = pane("i3", PaneLocation::Editor);
        hidden.visible = false;
        svc.add_pane(hidden);
        svc.add_pane(pane("i4", PaneLocation::Editor));

        let all: Vec<_> = PaneIterator::new(&svc).collect();
        assert_eq!(all.len(), 4);

        let editors: Vec<_> = PaneIterator::new(&svc).at_location(PaneLocation::Editor).collect();
        assert_eq!(editors.len(), 3);

        let visible_editors: Vec<_> = PaneIterator::new(&svc)
            .at_location(PaneLocation::Editor)
            .visible()
            .collect();
        assert_eq!(visible_editors.len(), 2);
    }

    #[test]
    fn deserialize_layout_roundtrip() {
        let panes = vec![
            pane("d1", PaneLocation::Editor),
            pane("d2", PaneLocation::Panel),
        ];
        let serialized = serialize_layout(&panes);
        let deserialized = deserialize_layout(&serialized);
        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized[0].id, "d1");
        assert_eq!(deserialized[0].location, PaneLocation::Editor);
        assert!(deserialized[0].visible);
        assert_eq!(deserialized[1].id, "d2");
        assert_eq!(deserialized[1].location, PaneLocation::Panel);
    }

    #[test]
    fn deserialize_layout_empty() {
        let deserialized = deserialize_layout("");
        assert!(deserialized.is_empty());
    }

    #[test]
    fn drag_drop_reorder() {
        let mut panes = vec![
            pane("dd1", PaneLocation::Editor),
            pane("dd2", PaneLocation::Editor),
            pane("dd3", PaneLocation::Editor),
        ];
        let mut drag = PaneDragDrop::begin("dd1", 0);
        assert!(!drag.did_move());

        drag.hover(2);
        assert!(drag.commit(&mut panes));
        assert!(drag.did_move());
        assert_eq!(panes[0].id, "dd2");
        assert_eq!(panes[1].id, "dd1");
        assert_eq!(panes[2].id, "dd3");
    }

    #[test]
    fn drag_drop_cancel() {
        let mut panes = vec![
            pane("c1", PaneLocation::Editor),
            pane("c2", PaneLocation::Editor),
        ];
        let mut drag = PaneDragDrop::begin("c1", 0);
        drag.hover(1);
        drag.cancel();
        assert!(!drag.commit(&mut panes));
        assert!(!drag.did_move());
        assert_eq!(panes[0].id, "c1");
    }

    #[test]
    fn test_resize_handle_basic() {
        let mut h = PaneResizeHandle::new("p1", ResizeDirection::Horizontal, 200);
        assert_eq!(h.current_size, 200);
        assert_eq!(h.resize(50), 250);
        assert_eq!(h.current_size, 250);
        assert_eq!(h.resize(-100), 150);
        assert_eq!(h.current_size, 150);
    }

    #[test]
    fn test_resize_handle_clamping() {
        let mut h = PaneResizeHandle::new("p1", ResizeDirection::Vertical, 100);
        assert_eq!(h.resize(-200), 50);
        assert_eq!(h.resize(2000), 1000);
        let mut h2 = PaneResizeHandle::new("p2", ResizeDirection::Horizontal, 500);
        assert_eq!(h2.resize_to(10), 50);
        assert_eq!(h2.resize_to(5000), 1000);
    }

    #[test]
    fn test_resize_handle_at_limits() {
        let mut h = PaneResizeHandle::new("p1", ResizeDirection::Horizontal, 50);
        assert!(h.is_at_minimum());
        assert!(!h.is_at_maximum());
        h.resize_to(1000);
        assert!(!h.is_at_minimum());
        assert!(h.is_at_maximum());
    }

    #[test]
    fn test_resize_handle_with_limits() {
        let mut h = PaneResizeHandle::new("p1", ResizeDirection::Vertical, 300)
            .with_limits(100, 500);
        assert_eq!(h.min_size, 100);
        assert_eq!(h.max_size, 500);
        assert_eq!(h.resize(-300), 100);
        assert_eq!(h.resize(600), 500);
    }

    #[test]
    fn test_pane_swap_success() {
        let mut svc = PaneService::new();
        let mut a = pane("a", PaneLocation::Editor);
        a.title = "Alpha".to_string();
        a.visible = true;
        let mut b = pane("b", PaneLocation::Panel);
        b.title = "Beta".to_string();
        b.visible = false;
        svc.add_pane(a);
        svc.add_pane(b);
        pane_swap(&mut svc, "a", "b").unwrap();
        let pa = svc.get_pane("a").unwrap();
        assert_eq!(pa.title, "Beta");
        assert!(!pa.visible);
        let pb = svc.get_pane("b").unwrap();
        assert_eq!(pb.title, "Alpha");
        assert!(pb.visible);
    }

    #[test]
    fn test_pane_swap_missing_pane() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("x", PaneLocation::Editor));
        assert!(pane_swap(&mut svc, "x", "missing").is_err());
        assert!(pane_swap(&mut svc, "missing", "x").is_err());
    }

    #[test]
    fn history_back_forward() {
        let mut h = PaneHistory::new(10);
        assert!(h.is_empty());
        assert!(h.current().is_none());

        h.visit("p1");
        h.visit("p2");
        h.visit("p3");
        assert_eq!(h.len(), 3);
        assert_eq!(h.current(), Some("p3"));

        assert!(h.can_go_back());
        assert_eq!(h.back(), Some("p2"));
        assert_eq!(h.current(), Some("p2"));
        assert_eq!(h.back(), Some("p1"));
        assert!(!h.can_go_back());
        assert_eq!(h.back(), None);

        assert!(h.can_go_forward());
        assert_eq!(h.forward(), Some("p2"));
        assert_eq!(h.forward(), Some("p3"));
        assert!(!h.can_go_forward());
        assert_eq!(h.forward(), None);
    }

    #[test]
    fn history_truncates_forward_on_visit() {
        let mut h = PaneHistory::new(10);
        h.visit("a");
        h.visit("b");
        h.visit("c");
        h.back(); // cursor at b
        h.visit("d"); // should drop "c"
        assert_eq!(h.len(), 3);
        assert_eq!(h.current(), Some("d"));
        assert!(!h.can_go_forward());
    }

    #[test]
    fn history_capacity_eviction() {
        let mut h = PaneHistory::new(3);
        h.visit("a");
        h.visit("b");
        h.visit("c");
        h.visit("d");
        assert_eq!(h.len(), 3);
        // oldest "a" should have been evicted
        assert_eq!(h.current(), Some("d"));
        h.back();
        assert_eq!(h.current(), Some("c"));
        h.back();
        assert_eq!(h.current(), Some("b"));
        assert!(!h.can_go_back());
    }

    #[test]
    fn history_clear() {
        let mut h = PaneHistory::new(10);
        h.visit("a");
        h.visit("b");
        h.clear();
        assert!(h.is_empty());
        assert_eq!(h.current(), None);
    }

    #[test]
    fn split_rectangle_right() {
        let r = Rectangle::new(0, 0, 1000, 600);
        let result = split_rectangle(r, SplitDirection::Right, 0.5);
        assert_eq!(result.original.width, 500);
        assert_eq!(result.new_pane.width, 500);
        assert_eq!(result.original.x, 0);
        assert_eq!(result.new_pane.x, 500);
        assert_eq!(result.original.height, 600);
        assert_eq!(result.new_pane.height, 600);
    }

    #[test]
    fn split_rectangle_down() {
        let r = Rectangle::new(10, 20, 800, 400);
        let result = split_rectangle(r, SplitDirection::Down, 0.75);
        let orig_h = (400.0 * 0.75) as u32;
        assert_eq!(result.original.height, orig_h);
        assert_eq!(result.new_pane.height, 400 - orig_h);
        assert_eq!(result.original.y, 20);
        assert_eq!(result.new_pane.y, 20 + orig_h);
    }

    #[test]
    fn split_rectangle_left_and_up() {
        let r = Rectangle::new(0, 0, 600, 400);
        let left = split_rectangle(r, SplitDirection::Left, 0.5);
        // new pane goes to the left, original shifts right
        assert_eq!(left.new_pane.x, 0);
        assert_eq!(left.original.x, 300);

        let up = split_rectangle(r, SplitDirection::Up, 0.5);
        assert_eq!(up.new_pane.y, 0);
        assert_eq!(up.original.y, 200);
    }

    #[test]
    fn split_rectangle_ratio_clamped() {
        let r = Rectangle::new(0, 0, 1000, 500);
        // Ratio below 0.1 should clamp to 0.1
        let result = split_rectangle(r, SplitDirection::Right, 0.01);
        assert_eq!(result.original.width, 100); // 10% of 1000
        // Ratio above 0.9 should clamp to 0.9
        let result2 = split_rectangle(r, SplitDirection::Right, 0.99);
        assert_eq!(result2.original.width, 900);
    }

    #[test]
    fn focus_chain_cycling() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("f1", PaneLocation::Editor));
        svc.add_pane(pane("f2", PaneLocation::Editor));
        svc.add_pane(pane("f3", PaneLocation::Panel));

        let mut chain = PaneFocusChain::from_service(&svc);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain.current(), Some("f1"));

        assert_eq!(chain.focus_next(), Some("f2"));
        assert_eq!(chain.focus_next(), Some("f3"));
        // wraps around
        assert_eq!(chain.focus_next(), Some("f1"));

        // go previous wraps
        assert_eq!(chain.focus_prev(), Some("f3"));
        assert_eq!(chain.focus_prev(), Some("f2"));
    }

    #[test]
    fn focus_chain_skips_hidden() {
        let mut svc = PaneService::new();
        svc.add_pane(pane("v1", PaneLocation::Editor));
        let mut h = pane("h1", PaneLocation::Editor);
        h.visible = false;
        svc.add_pane(h);
        svc.add_pane(pane("v2", PaneLocation::Editor));

        let chain = PaneFocusChain::from_service(&svc);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.current(), Some("v1"));
    }

    #[test]
    fn focus_chain_focus_on() {
        let mut chain = PaneFocusChain::new();
        chain.order = vec!["a".into(), "b".into(), "c".into()];
        chain.active = Some(0);

        assert!(chain.focus_on("c"));
        assert_eq!(chain.current(), Some("c"));
        assert!(!chain.focus_on("missing"));
        assert_eq!(chain.current(), Some("c"));
    }

    #[test]
    fn focus_chain_empty() {
        let mut chain = PaneFocusChain::new();
        assert!(chain.is_empty());
        assert_eq!(chain.current(), None);
        assert_eq!(chain.focus_next(), None);
        assert_eq!(chain.focus_prev(), None);
    }

    #[test]
    fn visibility_policy_always_visible() {
        let mut policy = PaneVisibilityPolicy::new();
        policy.set_rule("explorer", VisibilityRule::AlwaysVisible);
        let vis = policy.evaluate_visibility(None);
        assert_eq!(*vis.get("explorer").unwrap(), true);
    }

    #[test]
    fn visibility_policy_on_file_extension() {
        let mut policy = PaneVisibilityPolicy::new();
        policy.set_rule("rust-panel", VisibilityRule::OnFileExtension("rs".into()));
        policy.set_rule("js-panel", VisibilityRule::OnFileExtension("js".into()));

        let vis = policy.evaluate_visibility(Some("rs"));
        assert_eq!(*vis.get("rust-panel").unwrap(), true);
        assert_eq!(*vis.get("js-panel").unwrap(), false);

        let vis2 = policy.evaluate_visibility(None);
        assert_eq!(*vis2.get("rust-panel").unwrap(), false);
    }

    #[test]
    fn visibility_policy_remove_and_count() {
        let mut policy = PaneVisibilityPolicy::new();
        policy.set_rule("a", VisibilityRule::AlwaysVisible);
        policy.set_rule("b", VisibilityRule::WhenNotEmpty);
        assert_eq!(policy.rule_count(), 2);
        policy.remove_rule("a");
        assert_eq!(policy.rule_count(), 1);
        assert!(policy.rule_for("a").is_none());
        assert!(policy.rule_for("b").is_some());
    }

    #[test]
    fn pane_stats_from_service() {
        let mut svc = PaneService::new();
        svc.add_pane(
            PaneBuilder::new("a").title("A").location(PaneLocation::Sidebar).visible(true).build(),
        );
        svc.add_pane(
            PaneBuilder::new("b").title("B").location(PaneLocation::Panel).visible(false).build(),
        );
        svc.add_pane(
            PaneBuilder::new("c").title("C").location(PaneLocation::Sidebar).visible(true).maximized(true).build(),
        );
        let stats = PaneStats::from_service(&svc);
        assert_eq!(stats.total, 3);
        assert_eq!(stats.visible, 2);
        assert_eq!(stats.hidden, 1);
        assert_eq!(stats.maximized, 1);
        assert_eq!(*stats.by_location.get(&PaneLocation::Sidebar).unwrap(), 2);
        assert_eq!(*stats.by_location.get(&PaneLocation::Panel).unwrap(), 1);
        assert!((stats.visibility_ratio() - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn pane_stats_display() {
        let svc = PaneService::new();
        let stats = PaneStats::from_service(&svc);
        let display = format!("{stats}");
        assert!(display.contains("PaneStats"));
        assert_eq!(stats.visibility_ratio(), 0.0);
    }

    #[test]
    fn visibility_policy_follow_pane() {
        let mut policy = PaneVisibilityPolicy::new();
        policy.set_rule("main", VisibilityRule::AlwaysVisible);
        policy.set_rule("follower", VisibilityRule::FollowPane("main".into()));
        let vis = policy.evaluate_visibility(None);
        assert_eq!(*vis.get("follower").unwrap(), true);
    }

    // ---- PaneGrid tests ----

    #[test]
    fn grid_neighbor_horizontal() {
        let layout = PaneLayout::new(Rectangle::new(0, 0, 900, 300));
        let panes = vec![
            pane("left", PaneLocation::Editor),
            pane("mid", PaneLocation::Editor),
            pane("right", PaneLocation::Editor),
        ];
        let grid = PaneGrid::from_horizontal_layout(&layout, &panes);
        assert_eq!(grid.len(), 3);
        assert_eq!(grid.neighbor("left", Direction::Right), Some("mid"));
        assert_eq!(grid.neighbor("mid", Direction::Right), Some("right"));
        assert_eq!(grid.neighbor("right", Direction::Left), Some("mid"));
        assert_eq!(grid.neighbor("left", Direction::Left), None);
        assert_eq!(grid.neighbor("right", Direction::Right), None);
    }

    #[test]
    fn grid_neighbor_vertical() {
        let layout = PaneLayout::new(Rectangle::new(0, 0, 400, 600));
        let panes = vec![
            pane("top", PaneLocation::Editor),
            pane("bottom", PaneLocation::Editor),
        ];
        let grid = PaneGrid::from_vertical_layout(&layout, &panes);
        assert_eq!(grid.len(), 2);
        assert_eq!(grid.neighbor("top", Direction::Down), Some("bottom"));
        assert_eq!(grid.neighbor("bottom", Direction::Up), Some("top"));
        assert_eq!(grid.neighbor("top", Direction::Up), None);
    }

    #[test]
    fn grid_no_overlaps_from_layout() {
        let layout = PaneLayout::new(Rectangle::new(0, 0, 1000, 500));
        let panes = vec![
            pane("a", PaneLocation::Editor),
            pane("b", PaneLocation::Editor),
            pane("c", PaneLocation::Editor),
        ];
        let grid = PaneGrid::from_horizontal_layout(&layout, &panes);
        assert!(grid.find_overlaps().is_empty());
        assert!(grid.all_within(&Rectangle::new(0, 0, 1000, 500)));
    }

    #[test]
    fn grid_detects_overlaps() {
        let mut grid = PaneGrid::new();
        grid.add_cell("a", Rectangle::new(0, 0, 200, 200));
        grid.add_cell("b", Rectangle::new(100, 0, 200, 200));
        let overlaps = grid.find_overlaps();
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0], ("a", "b"));
    }

    #[test]
    fn grid_all_within_check() {
        let mut grid = PaneGrid::new();
        grid.add_cell("inside", Rectangle::new(10, 10, 80, 80));
        assert!(grid.all_within(&Rectangle::new(0, 0, 100, 100)));
        grid.add_cell("outside", Rectangle::new(90, 90, 20, 20));
        assert!(!grid.all_within(&Rectangle::new(0, 0, 100, 100)));
    }

    // ---- Proportional resize tests ----

    #[test]
    fn distribute_proportional_even() {
        let sizes = distribute_proportional(900, &[1.0, 1.0, 1.0]);
        assert_eq!(sizes, vec![300, 300, 300]);
        let total: u32 = sizes.iter().sum();
        assert_eq!(total, 900);
    }

    #[test]
    fn distribute_proportional_uneven() {
        let sizes = distribute_proportional(100, &[1.0, 2.0, 2.0]);
        let total: u32 = sizes.iter().sum();
        assert_eq!(total, 100);
        // The 2x-weight items should be roughly double the 1x-weight item
        assert!(sizes[1] >= sizes[0]);
        assert!(sizes[2] >= sizes[0]);
    }

    #[test]
    fn resize_horizontal_proportional_preserves_total() {
        let rects = vec![
            Rectangle::new(0, 0, 200, 100),
            Rectangle::new(200, 0, 300, 100),
            Rectangle::new(500, 0, 500, 100),
        ];
        let resized = resize_horizontal_proportional(&rects, 1200);
        let total_width: u32 = resized.iter().map(|r| r.width).sum();
        assert_eq!(total_width, 1200);
        // x-coordinates should be contiguous
        assert_eq!(resized[0].x, 0);
        assert_eq!(resized[1].x, resized[0].width);
        assert_eq!(resized[2].x, resized[0].width + resized[1].width);
    }

    #[test]
    fn resize_vertical_proportional_preserves_total() {
        let rects = vec![
            Rectangle::new(0, 0, 400, 100),
            Rectangle::new(0, 100, 400, 300),
        ];
        let resized = resize_vertical_proportional(&rects, 800);
        let total_height: u32 = resized.iter().map(|r| r.height).sum();
        assert_eq!(total_height, 800);
    }

    // ---- PaneSizeConstraints tests ----

    #[test]
    fn size_constraints_clamp() {
        let c = PaneSizeConstraints::new(100, 500, 80, 400);
        assert_eq!(c.clamp(50, 50), (100, 80));
        assert_eq!(c.clamp(600, 600), (500, 400));
        assert_eq!(c.clamp(200, 200), (200, 200));
    }

    #[test]
    fn size_constraints_satisfies() {
        let c = PaneSizeConstraints::new(100, 500, 100, 400);
        assert!(c.satisfies(200, 200));
        assert!(!c.satisfies(50, 200));
        assert!(!c.satisfies(200, 50));
        assert!(!c.satisfies(600, 200));
    }

    #[test]
    fn size_constraints_clamp_rect() {
        let c = PaneSizeConstraints::new(100, 500, 100, 400);
        let r = Rectangle::new(10, 20, 50, 600);
        let clamped = c.clamp_rect(&r);
        assert_eq!(clamped.x, 10);
        assert_eq!(clamped.y, 20);
        assert_eq!(clamped.width, 100);
        assert_eq!(clamped.height, 400);
    }

    // ---- Layout validation tests ----

    #[test]
    fn validate_layout_valid() {
        let container = Rectangle::new(0, 0, 1000, 500);
        let layout = PaneLayout::new(container);
        let panes = vec![
            pane("a", PaneLocation::Editor),
            pane("b", PaneLocation::Editor),
        ];
        let grid = PaneGrid::from_horizontal_layout(&layout, &panes);
        let errors = validate_layout(&grid, &container, 100, 100);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_layout_empty_grid() {
        let container = Rectangle::new(0, 0, 100, 100);
        let grid = PaneGrid::new();
        let errors = validate_layout(&grid, &container, 50, 50);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0], LayoutValidationError::Empty);
    }

    #[test]
    fn validate_layout_detects_below_min() {
        let container = Rectangle::new(0, 0, 1000, 1000);
        let mut grid = PaneGrid::new();
        grid.add_cell("tiny", Rectangle::new(0, 0, 30, 30));
        let errors = validate_layout(&grid, &container, 50, 50);
        assert!(errors.iter().any(|e| matches!(
            e,
            LayoutValidationError::BelowMinSize { pane_id, .. } if pane_id == "tiny"
        )));
    }

    #[test]
    fn validate_layout_detects_out_of_bounds() {
        let container = Rectangle::new(0, 0, 100, 100);
        let mut grid = PaneGrid::new();
        grid.add_cell("oob", Rectangle::new(90, 90, 50, 50));
        let errors = validate_layout(&grid, &container, 10, 10);
        assert!(errors.iter().any(|e| matches!(
            e,
            LayoutValidationError::OutOfBounds { pane_id } if pane_id == "oob"
        )));
    }

    #[test]
    fn paneResizeConstraints_new() {
        let s = PaneResizeConstraints::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn paneResizeConstraints_add_contains() {
        let mut s = PaneResizeConstraints::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn paneResizeConstraints_add_duplicate() {
        let mut s = PaneResizeConstraints::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn paneResizeConstraints_remove() {
        let mut s = PaneResizeConstraints::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn paneResizeConstraints_capacity() {
        let s = PaneResizeConstraints::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn paneResizeConstraints_search() {
        let mut s = PaneResizeConstraints::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn paneResizeConstraints_stats() {
        let mut s = PaneResizeConstraints::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn paneDragHandler_new() {
        let m = PaneDragHandler::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn paneDragHandler_add_find() {
        let mut m = PaneDragHandler::new();
        m.add(PaneDragHandlerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn paneDragHandler_priority_filter() {
        let mut m = PaneDragHandler::new();
        m.add(PaneDragHandlerItem::new("a", "A").with_priority(PaneDragHandlerPriority::High));
        m.add(PaneDragHandlerItem::new("b", "B").with_priority(PaneDragHandlerPriority::Low));
        m.add(PaneDragHandlerItem::new("c", "C").with_priority(PaneDragHandlerPriority::High));
        assert_eq!(m.by_priority(PaneDragHandlerPriority::High).len(), 2);
    }

    #[test]
    fn paneDragHandler_remove() {
        let mut m = PaneDragHandler::new();
        m.add(PaneDragHandlerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn paneDragHandler_search() {
        let mut m = PaneDragHandler::new();
        m.add(PaneDragHandlerItem::new("id1", "Hello World"));
        m.add(PaneDragHandlerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn paneDragHandler_total_weight() {
        let mut m = PaneDragHandler::new();
        m.add(PaneDragHandlerItem::new("a", "A").with_priority(PaneDragHandlerPriority::Critical));
        m.add(PaneDragHandlerItem::new("b", "B").with_priority(PaneDragHandlerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn paneDragHandler_capacity_limit() {
        let mut m = PaneDragHandler::new().with_max_items(2);
        m.add(PaneDragHandlerItem::new("1", "one"));
        m.add(PaneDragHandlerItem::new("2", "two"));
        assert!(!m.add(PaneDragHandlerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn paneDragHandler_sorted_by_priority() {
        let mut m = PaneDragHandler::new();
        m.add(PaneDragHandlerItem::new("lo", "Low").with_priority(PaneDragHandlerPriority::Low));
        m.add(PaneDragHandlerItem::new("hi", "High").with_priority(PaneDragHandlerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn paneDragHandler_item_metadata() {
        let mut item = PaneDragHandlerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn paneResizeConstraints_enabled_toggle() {
        let mut s = PaneResizeConstraints::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn paneDragHandler_priority_display() {
        assert_eq!(format!("{}", PaneDragHandlerPriority::High), "high");
        assert_eq!(format!("{}", PaneDragHandlerPriority::Low), "low");
    }


    #[test]
    fn wb_pane_config_new() {
        let cfg = WbPaneConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn wb_pane_config_set_get() {
        let mut cfg = WbPaneConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn wb_pane_config_remove() {
        let mut cfg = WbPaneConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn wb_pane_config_keys_sorted() {
        let mut cfg = WbPaneConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn wb_pane_config_bump_version() {
        let mut cfg = WbPaneConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn wb_pane_config_clear() {
        let mut cfg = WbPaneConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn wb_pane_config_merge() {
        let mut cfg1 = WbPaneConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = WbPaneConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn wb_pane_config_disable() {
        let mut cfg = WbPaneConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn wb_pane_rate_tracker_empty() {
        let rt = WbPaneRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn wb_pane_rate_tracker_record() {
        let mut rt = WbPaneRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn wb_pane_rate_tracker_prune() {
        let mut rt = WbPaneRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn wb_pane_validator_valid() {
        let v = WbPaneValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn wb_pane_validator_errors() {
        let mut v = WbPaneValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn wb_pane_validator_clear() {
        let mut v = WbPaneValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn wb_pane_validator_merge() {
        let mut v1 = WbPaneValidator::new();
        v1.add_error("e1");
        let mut v2 = WbPaneValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn wb_pane_rate_tracker_clear() {
        let mut rt = WbPaneRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn qd_metrics_empty() {
        let m = QdMetrics::new("wb_pane");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qd_metrics_record_and_mean() {
        let mut m = QdMetrics::new("wb_pane");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qd_metrics_min_max() {
        let mut m = QdMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qd_metrics_variance_and_std() {
        let mut m = QdMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qd_metrics_percentile() {
        let mut m = QdMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qd_metrics_merge() {
        let mut a = QdMetrics::new("a");
        a.record(1.0);
        let mut b = QdMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qd_metrics_reset() {
        let mut m = QdMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qd_rate_window_empty() {
        let rw = QdRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qd_rate_window_tick_and_rate() {
        let mut rw = QdRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qd_lru_cache_basic() {
        let mut c = QdLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qd_lru_cache_contains_and_keys() {
        let mut c = QdLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qd_lru_cache_remove() {
        let mut c = QdLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qd_metrics_sum() {
        let mut m = QdMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qd_metrics_label() {
        let m = QdMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qd_lru_cache_clear() {
        let mut c = QdLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for wb_pane
    #[test]
    fn xa_wb_pane_ring_new() {
        let rb = super::XaWbPaneRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_pane_ring_push_len() {
        let mut rb = super::XaWbPaneRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_pane_ring_wrap() {
        let mut rb = super::XaWbPaneRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_pane_ring_mean_empty() {
        let rb = super::XaWbPaneRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_pane_ring_mean_values() {
        let mut rb = super::XaWbPaneRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_pane_ring_min_max() {
        let mut rb = super::XaWbPaneRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_pane_ring_iter() {
        let mut rb = super::XaWbPaneRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_pane_counter_new() {
        let c = super::XaWbPaneCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_pane_counter_inc() {
        let mut c = super::XaWbPaneCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_pane_counter_inc_by() {
        let mut c = super::XaWbPaneCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_pane_counter_reset() {
        let mut c = super::XaWbPaneCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_pane_counter_clear() {
        let mut c = super::XaWbPaneCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_pane_counter_default() {
        let c = super::XaWbPaneCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }

}
