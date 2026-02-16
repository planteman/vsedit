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
}
