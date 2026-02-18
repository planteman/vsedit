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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 219
// ---------------------------------------------------------------------------

/// Generic object pool `Xc219Pool<T>`.
pub struct Xc219Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc219Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc219PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc219Pool<T> {
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
    pub fn stats(&self) -> Xc219PoolStats {
        Xc219PoolStats {
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

impl<T> Default for Xc219Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc219Scheduler`.
pub struct Xc219Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc219Scheduler {
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

impl Default for Xc219Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_219 hash for the given byte slice.
pub fn xc_219_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_219 convention.
pub fn xc_219_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_118 deepening: state machine + event bus ---

/// States for the Xd118 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd118State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd118State {
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
pub struct Xd118Transition {
    pub from: Xd118State,
    pub to: Xd118State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd118StateMachine {
    current: Xd118State,
    history: Vec<Xd118Transition>,
    step_counter: usize,
}

impl Xd118StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd118State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd118State {
        self.current
    }

    pub fn history(&self) -> &[Xd118Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd118State) -> Result<Xd118State, String> {
        let allowed = match (self.current, target) {
            (Xd118State::Idle, Xd118State::Running) => true,
            (Xd118State::Running, Xd118State::Paused) => true,
            (Xd118State::Running, Xd118State::Done) => true,
            (Xd118State::Paused, Xd118State::Running) => true,
            (Xd118State::Paused, Xd118State::Done) => true,
            (Xd118State::Done, Xd118State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_118: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd118Transition {
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
            "Xd118SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd118State> {
        let prefix = "Xd118SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd118State::Idle),
            "Running" => Some(Xd118State::Running),
            "Paused" => Some(Xd118State::Paused),
            "Done" => Some(Xd118State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd118State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd118 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd118Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd118Event {
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

type Xd118HandlerFn = Box<dyn Fn(&Xd118Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd118EventBus {
    handlers: Vec<(usize, Option<String>, Xd118HandlerFn)>,
    next_id: usize,
    published: Vec<Xd118Event>,
}

impl Xd118EventBus {
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
        F: Fn(&Xd118Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd118Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd118Event) {
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

    pub fn published_events(&self) -> &[Xd118Event] {
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
// xg_45: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg45Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg45Graph {
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

impl Default for Xg45Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_45: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg45Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg45Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg45Heap<T>) {
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

impl<T: Ord> Default for Xg45Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 218).
pub struct Xh218SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh218SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 260 as u64,
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

/// A compact bit set supporting boolean operations (variant 218).
pub struct Xh218BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh218BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 218).
pub struct Xi218Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi218Deque<T> {
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
pub struct Xi218Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi218Interval {
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

/// A simple interval tree (variant 218).
pub struct Xi218IntervalTree {
    xi_intervals: Vec<Xi218Interval>,
}

impl Xi218IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi218Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi218Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi218Interval) -> Vec<&Xi218Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi218Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi218Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi218Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi218Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi218Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi218Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 218) ---

/// Disjoint set / union-find for crate 218.
pub struct Xj218UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj218UnionFind {
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

const XJ218_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 218.
pub struct Xj218BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj218BTreeNode<K, V>>>,
    len: usize,
}

struct Xj218BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj218BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj218BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ218_BTREE_ORDER - 1
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
        let mid = XJ218_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj218BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj218BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj218BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj218BTreeNode::xj_new_leaf();
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


// --- xk_218 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk218SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk218SegmentTree {
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
pub struct Xk218DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk218DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_218).
#[derive(Debug, Clone)]
pub struct Xl218Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl218Rope {
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

/// Suffix array for efficient string searching (xl_218).
#[derive(Debug, Clone)]
pub struct Xl218SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl218SuffixArray {
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
pub struct Xm218MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm218MatrixSparse {
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
pub struct Xm218Tokenizer {
    text: String,
}

impl Xm218Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 218.
pub struct Xn218Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn218Fenwick {
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

// ----- AVL tree map — crate 218 -----

#[derive(Debug, Clone)]
struct Xn218AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn218AvlNode<K, V>>>,
    right: Option<Box<Xn218AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 218.
#[derive(Debug, Clone)]
pub struct Xn218AVL<K, V> {
    root: Option<Box<Xn218AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn218AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn218AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn218AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn218AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn218AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn218AvlNode<K, V>>) -> Box<Xn218AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn218AvlNode<K, V>>) -> Box<Xn218AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn218AvlNode<K, V>>) -> Box<Xn218AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn218AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn218AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn218AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn218AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn218AvlNode<K, V>>) -> &Xn218AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn218AvlNode<K, V>>) -> (Box<Xn218AvlNode<K, V>>, Option<Box<Xn218AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn218AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn218AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn218AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn218AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn218AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn218AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn218AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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


    // ---- xc_ pool / scheduler tests – block 219 ----

    #[test]
    fn xc_219_pool_new_empty() {
        let pool: super::Xc219Pool<i32> = super::Xc219Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_219_pool_release_acquire() {
        let mut pool = super::Xc219Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_219_pool_acquire_empty() {
        let mut pool: super::Xc219Pool<i32> = super::Xc219Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_219_pool_full() {
        let mut pool = super::Xc219Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_219_pool_drain() {
        let mut pool = super::Xc219Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_219_pool_stats() {
        let mut pool = super::Xc219Pool::new(8);
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
    fn xc_219_pool_clear() {
        let mut pool = super::Xc219Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_219_pool_shrink() {
        let mut pool = super::Xc219Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_219_pool_default() {
        let pool: super::Xc219Pool<String> = super::Xc219Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_219_pool_extend() {
        let mut pool = super::Xc219Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_219_pool_retain() {
        let mut pool = super::Xc219Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_219_scheduler_round_robin() {
        let mut sched = super::Xc219Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_219_scheduler_empty() {
        let mut sched = super::Xc219Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_219_scheduler_reset() {
        let mut sched = super::Xc219Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_219_scheduler_add_remove() {
        let mut sched = super::Xc219Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_219_scheduler_targets() {
        let sched = super::Xc219Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_219_hash_empty() {
        assert_eq!(super::xc_219_hash(b""), 5381);
    }

    #[test]
    fn xc_219_hash_data() {
        let h = super::xc_219_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_219_hash(b"hello"), h);
    }

    #[test]
    fn xc_219_reverse_str() {
        assert_eq!(super::xc_219_reverse("abc"), "cba");
        assert_eq!(super::xc_219_reverse(""), "");
    }


    // --- xd_118 deepening tests ---

    #[test]
    fn xd_118_sm_initial_state() {
        let sm = Xd118StateMachine::new();
        assert_eq!(sm.current_state(), Xd118State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_118_sm_valid_idle_to_running() {
        let mut sm = Xd118StateMachine::new();
        assert!(sm.transition(Xd118State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd118State::Running);
    }

    #[test]
    fn xd_118_sm_valid_running_to_paused() {
        let mut sm = Xd118StateMachine::new();
        sm.transition(Xd118State::Running).unwrap();
        assert!(sm.transition(Xd118State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd118State::Paused);
    }

    #[test]
    fn xd_118_sm_valid_running_to_done() {
        let mut sm = Xd118StateMachine::new();
        sm.transition(Xd118State::Running).unwrap();
        assert!(sm.transition(Xd118State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd118State::Done);
    }

    #[test]
    fn xd_118_sm_valid_paused_to_running() {
        let mut sm = Xd118StateMachine::new();
        sm.transition(Xd118State::Running).unwrap();
        sm.transition(Xd118State::Paused).unwrap();
        assert!(sm.transition(Xd118State::Running).is_ok());
    }

    #[test]
    fn xd_118_sm_valid_done_to_idle() {
        let mut sm = Xd118StateMachine::new();
        sm.transition(Xd118State::Running).unwrap();
        sm.transition(Xd118State::Done).unwrap();
        assert!(sm.transition(Xd118State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd118State::Idle);
    }

    #[test]
    fn xd_118_sm_invalid_idle_to_done() {
        let mut sm = Xd118StateMachine::new();
        assert!(sm.transition(Xd118State::Done).is_err());
    }

    #[test]
    fn xd_118_sm_invalid_idle_to_paused() {
        let mut sm = Xd118StateMachine::new();
        assert!(sm.transition(Xd118State::Paused).is_err());
    }

    #[test]
    fn xd_118_sm_history_tracking() {
        let mut sm = Xd118StateMachine::new();
        sm.transition(Xd118State::Running).unwrap();
        sm.transition(Xd118State::Paused).unwrap();
        sm.transition(Xd118State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd118State::Idle);
        assert_eq!(sm.history()[0].to, Xd118State::Running);
        assert_eq!(sm.history()[1].from, Xd118State::Running);
        assert_eq!(sm.history()[2].to, Xd118State::Done);
    }

    #[test]
    fn xd_118_sm_serialize_deserialize() {
        let mut sm = Xd118StateMachine::new();
        sm.transition(Xd118State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd118StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd118State::Running));
    }

    #[test]
    fn xd_118_sm_deserialize_invalid() {
        assert_eq!(Xd118StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_118_sm_reset() {
        let mut sm = Xd118StateMachine::new();
        sm.transition(Xd118State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd118State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_118_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd118EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd118Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_118_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd118EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd118Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd118Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_118_bus_unsubscribe() {
        let mut bus = Xd118EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_118_event_kind_and_payload() {
        let e = Xd118Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd118Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_118_bus_clear_history() {
        let mut bus = Xd118EventBus::new();
        bus.publish(Xd118Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_118_sm_step_counter_increments() {
        let mut sm = Xd118StateMachine::new();
        sm.transition(Xd118State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd118State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_45 graph tests ------------------------------------------------

    #[test]
    fn xg_45_graph_empty() {
        let g = super::Xg45Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_45_graph_add_node() {
        let mut g = super::Xg45Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_45_graph_add_edge() {
        let mut g = super::Xg45Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_45_graph_neighbors() {
        let mut g = super::Xg45Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_45_graph_has_path() {
        let mut g = super::Xg45Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_45_graph_self_path() {
        let g = super::Xg45Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_45_graph_topo_sort() {
        let mut g = super::Xg45Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_45_graph_cycle_detect_false() {
        let mut g = super::Xg45Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_45_graph_cycle_detect_true() {
        let mut g = super::Xg45Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_45 heap tests -------------------------------------------------

    #[test]
    fn xg_45_heap_empty() {
        let h: super::Xg45Heap<i32> = super::Xg45Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_45_heap_push_pop() {
        let mut h = super::Xg45Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_45_heap_peek() {
        let mut h = super::Xg45Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_45_heap_drain_sorted() {
        let mut h = super::Xg45Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_45_heap_merge() {
        let mut a = super::Xg45Heap::new();
        let mut b = super::Xg45Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_45_heap_default() {
        let h: super::Xg45Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_45_graph_default() {
        let g: super::Xg45Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh218_skip_insert_contains() {
        let mut sl = super::Xh218SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh218_skip_remove() {
        let mut sl = super::Xh218SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh218_skip_len() {
        let mut sl = super::Xh218SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh218_skip_range_query() {
        let mut sl = super::Xh218SkipList::xh_new(4);
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
    fn xh218_skip_floor_ceiling() {
        let mut sl = super::Xh218SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh218_skip_rank() {
        let mut sl = super::Xh218SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh218_skip_empty() {
        let sl = super::Xh218SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh218_skip_duplicates() {
        let mut sl = super::Xh218SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh218_bitset_set_test() {
        let mut bs = super::Xh218BitSet::xh_new(256);
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
    fn xh218_bitset_clear_count() {
        let mut bs = super::Xh218BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh218_bitset_and_or_xor() {
        let mut a = super::Xh218BitSet::xh_new(128);
        let mut b = super::Xh218BitSet::xh_new(128);
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
    fn xh218_bitset_iter_ones() {
        let mut bs = super::Xh218BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh218_bitset_first_last() {
        let mut bs = super::Xh218BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh218_bitset_empty() {
        let bs = super::Xh218BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi218_deque_push_pop_back() {
        let mut dq = super::Xi218Deque::xi_new(4);
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
    fn xi218_deque_push_pop_front() {
        let mut dq = super::Xi218Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi218_deque_mixed_ops() {
        let mut dq = super::Xi218Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi218_deque_get_and_split() {
        let mut dq = super::Xi218Deque::xi_new(8);
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
    fn xi218_deque_rotate_left() {
        let mut dq = super::Xi218Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi218_deque_rotate_right() {
        let mut dq = super::Xi218Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi218_deque_grow() {
        let mut dq = super::Xi218Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi218_deque_empty() {
        let dq = super::Xi218Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi218_interval_tree_insert_query() {
        let mut tree = super::Xi218IntervalTree::xi_new();
        tree.xi_insert(super::Xi218Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi218Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi218Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi218_interval_tree_overlap() {
        let mut tree = super::Xi218IntervalTree::xi_new();
        tree.xi_insert(super::Xi218Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi218Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi218Interval::xi_new(12, 20));
        let q = super::Xi218Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi218_interval_tree_remove() {
        let mut tree = super::Xi218IntervalTree::xi_new();
        tree.xi_insert(super::Xi218Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi218Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi218_interval_tree_gaps() {
        let mut tree = super::Xi218IntervalTree::xi_new();
        tree.xi_insert(super::Xi218Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi218Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi218Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi218Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi218Interval::xi_new(8, 10));
    }

    #[test]
    fn xi218_interval_tree_merge() {
        let mut tree = super::Xi218IntervalTree::xi_new();
        tree.xi_insert(super::Xi218Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi218Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi218Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi218Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi218Interval::xi_new(10, 15));
    }

    #[test]
    fn xi218_interval_tree_all() {
        let mut tree = super::Xi218IntervalTree::xi_new();
        tree.xi_insert(super::Xi218Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi218Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi218_interval_tree_empty() {
        let tree = super::Xi218IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi218_interval_tree_contains_point() {
        let iv = super::Xi218Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 218) ---

    #[test]
    fn xj_218_uf_make_and_find() {
        let mut uf = super::Xj218UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_218_uf_union_connected() {
        let mut uf = super::Xj218UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_218_uf_component_count() {
        let mut uf = super::Xj218UnionFind::xj_new();
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
    fn xj_218_uf_component_size() {
        let mut uf = super::Xj218UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_218_uf_largest_component() {
        let mut uf = super::Xj218UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_218_uf_many_elements() {
        let mut uf = super::Xj218UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_218_uf_separate_components() {
        let mut uf = super::Xj218UnionFind::xj_new();
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
    fn xj_218_uf_path_compression() {
        let mut uf = super::Xj218UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_218_bt_insert_get() {
        let mut bt = super::Xj218BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_218_bt_contains_len() {
        let mut bt = super::Xj218BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_218_bt_replace() {
        let mut bt = super::Xj218BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_218_bt_remove() {
        let mut bt = super::Xj218BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_218_bt_keys_values() {
        let mut bt = super::Xj218BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_218_bt_range() {
        let mut bt = super::Xj218BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_218_bt_min_max() {
        let mut bt = super::Xj218BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_218_bt_many_inserts() {
        let mut bt = super::Xj218BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_218 segment tree tests ---

    #[test]
    fn xk_218_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk218SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_218_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk218SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_218_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk218SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_218_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk218SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_218_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk218SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_218_st_single_element() {
        let data = vec![42];
        let st = super::Xk218SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_218_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk218SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_218_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk218SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_218 disjoint intervals tests ---

    #[test]
    fn xk_218_di_add_and_count() {
        let mut di = super::Xk218DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_218_di_merge_overlap() {
        let mut di = super::Xk218DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_218_di_contains() {
        let mut di = super::Xk218DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_218_di_remove() {
        let mut di = super::Xk218DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_218_di_covered_length() {
        let mut di = super::Xk218DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_218_di_gaps() {
        let mut di = super::Xk218DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_218_di_merge_adjacent() {
        let mut di = super::Xk218DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_218_di_empty() {
        let di = super::Xk218DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_218_rope_new_empty() {
        let rope = super::Xl218Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_218_rope_from_str() {
        let rope = super::Xl218Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_218_rope_insert_at() {
        let mut rope = super::Xl218Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_218_rope_delete_range() {
        let mut rope = super::Xl218Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_218_rope_char_at() {
        let rope = super::Xl218Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_218_rope_split_concat() {
        let rope = super::Xl218Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_218_rope_line_count() {
        let rope = super::Xl218Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_218_rope_line_at() {
        let rope = super::Xl218Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_218_sa_build_and_search() {
        let sa = super::Xl218SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_218_sa_count() {
        let sa = super::Xl218SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_218_sa_longest_repeated() {
        let sa = super::Xl218SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_218_sa_all_positions() {
        let sa = super::Xl218SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_218_sa_len() {
        let sa = super::Xl218SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_218_sa_empty() {
        let sa = super::Xl218SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_218_rope_slice() {
        let rope = super::Xl218Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_218_sa_search_start() {
        let sa = super::Xl218SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_218_sparse_set_get() {
        let mut m = super::Xm218MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_218_sparse_row_col() {
        let mut m = super::Xm218MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_218_sparse_transpose() {
        let mut m = super::Xm218MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_218_sparse_multiply_vec() {
        let mut m = super::Xm218MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_218_sparse_nnz_density() {
        let mut m = super::Xm218MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_218_sparse_clear() {
        let mut m = super::Xm218MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_218_sparse_overwrite_zero() {
        let mut m = super::Xm218MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_218_tokenizer_basic() {
        let t = super::Xm218Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_218_tokenizer_count() {
        let t = super::Xm218Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_218_tokenizer_unique() {
        let t = super::Xm218Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_218_tokenizer_frequency() {
        let t = super::Xm218Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_218_tokenizer_delimiter() {
        let t = super::Xm218Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_218_tokenizer_whitespace() {
        let t = super::Xm218Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_218_tokenizer_empty() {
        let t = super::Xm218Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 218 ----

    #[test]
    fn xn_218_fenwick_prefix_sum() {
        let mut ft = super::Xn218Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_218_fenwick_range_sum() {
        let mut ft = super::Xn218Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_218_fenwick_point_query() {
        let mut ft = super::Xn218Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_218_fenwick_len() {
        let ft = super::Xn218Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_218_fenwick_multiple_updates() {
        let mut ft = super::Xn218Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_218_fenwick_single_element() {
        let mut ft = super::Xn218Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_218_fenwick_find_kth() {
        let mut ft = super::Xn218Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_218_fenwick_negative_delta() {
        let mut ft = super::Xn218Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 218 ----

    #[test]
    fn xn_218_avl_insert_get() {
        let mut m = super::Xn218AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_218_avl_remove() {
        let mut m = super::Xn218AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_218_avl_in_order() {
        let mut m = super::Xn218AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_218_avl_min_max() {
        let mut m = super::Xn218AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_218_avl_floor_ceiling() {
        let mut m = super::Xn218AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_218_avl_height_balanced() {
        let mut m = super::Xn218AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_218_avl_overwrite() {
        let mut m = super::Xn218AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_218_avl_empty() {
        let m: super::Xn218AVL<i32, i32> = super::Xn218AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}
