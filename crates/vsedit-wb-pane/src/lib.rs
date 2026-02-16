//! Sidebar pane container.

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
}
