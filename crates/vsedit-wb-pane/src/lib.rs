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
}
