//! Path breadcrumb navigation.
//!
//! Provides breadcrumb data structures and a renderable navigation bar
//! with keyboard-navigable segments — rendered via ratatui.

use std::path::PathBuf;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

/// The kind of a breadcrumb element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreadcrumbKind {
    File,
    Folder,
    Symbol,
    Class,
    Function,
    Method,
    Property,
    Enum,
    Interface,
    Module,
}

/// A single element in a breadcrumb path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbElement {
    pub label: String,
    pub kind: BreadcrumbKind,
    pub uri: Option<String>,
    pub range_start_line: Option<u32>,
}

/// An ordered sequence of breadcrumb elements representing a navigation path.
#[derive(Debug, Clone, Default)]
pub struct BreadcrumbPath {
    pub elements: Vec<BreadcrumbElement>,
}

impl BreadcrumbPath {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }

    pub fn push(&mut self, element: BreadcrumbElement) {
        self.elements.push(element);
    }

    pub fn pop(&mut self) -> Option<BreadcrumbElement> {
        self.elements.pop()
    }

    pub fn last(&self) -> Option<&BreadcrumbElement> {
        self.elements.last()
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Join all element labels with `" > "`.
    pub fn to_path_string(&self) -> String {
        self.elements
            .iter()
            .map(|e| e.label.as_str())
            .collect::<Vec<_>>()
            .join(" > ")
    }
}

/// Trait for types that can produce breadcrumb paths for a given location.
pub trait BreadcrumbProvider {
    fn provide_breadcrumbs(&self, uri: &str, line: u32, col: u32) -> BreadcrumbPath;
}

// ---------------------------------------------------------------------------
// Renderable breadcrumb bar
// ---------------------------------------------------------------------------

/// A single segment in the breadcrumb navigation bar.
#[derive(Debug, Clone)]
pub struct BreadcrumbItem {
    pub label: String,
    pub icon: Option<char>,
    pub path: PathBuf,
    pub is_active: bool,
}

impl BreadcrumbItem {
    pub fn new(label: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            path: path.into(),
            is_active: false,
        }
    }

    pub fn with_icon(mut self, icon: char) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// Breadcrumb navigation bar with rendering support.
#[derive(Debug, Clone)]
pub struct BreadcrumbBar {
    pub items: Vec<BreadcrumbItem>,
    pub selected_index: usize,
    pub is_focused: bool,
}

impl BreadcrumbBar {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected_index: 0,
            is_focused: false,
        }
    }

    /// Replace all breadcrumb items.
    pub fn set_items(&mut self, items: Vec<BreadcrumbItem>) {
        self.items = items;
        if !self.items.is_empty() {
            self.selected_index = self.selected_index.min(self.items.len() - 1);
            if let Some(last) = self.items.last_mut() {
                last.is_active = true;
            }
        } else {
            self.selected_index = 0;
        }
    }

    /// Move selection to the next breadcrumb (right).
    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.items.len() - 1);
        }
    }

    /// Move selection to the previous breadcrumb (left).
    pub fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// Activate the currently selected item, returning its path.
    pub fn activate(&mut self) -> Option<PathBuf> {
        if self.items.is_empty() {
            return None;
        }
        for item in &mut self.items {
            item.is_active = false;
        }
        self.items[self.selected_index].is_active = true;
        Some(self.items[self.selected_index].path.clone())
    }

    /// Render the breadcrumb bar with `›` separators.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width < 3 || self.items.is_empty() {
            return;
        }

        let mut spans = Vec::new();
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    " › ",
                    Style::default().fg(Color::DarkGray),
                ));
            }

            let label = if let Some(icon) = item.icon {
                format!("{} {}", icon, item.label)
            } else {
                item.label.clone()
            };

            let style = if self.is_focused && i == self.selected_index {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::DarkGray)
            } else if item.is_active {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            spans.push(Span::styled(label, style));
        }

        let line = Line::from(spans);
        let render_area = Rect {
            height: 1,
            ..area
        };
        line.render(render_area, buf);
    }
}

impl Default for BreadcrumbBar {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Display implementations
// ---------------------------------------------------------------------------

impl std::fmt::Display for BreadcrumbKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BreadcrumbKind::File => "file",
            BreadcrumbKind::Folder => "folder",
            BreadcrumbKind::Symbol => "symbol",
            BreadcrumbKind::Class => "class",
            BreadcrumbKind::Function => "function",
            BreadcrumbKind::Method => "method",
            BreadcrumbKind::Property => "property",
            BreadcrumbKind::Enum => "enum",
            BreadcrumbKind::Interface => "interface",
            BreadcrumbKind::Module => "module",
        };
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for BreadcrumbElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.kind, self.label)
    }
}

// ---------------------------------------------------------------------------
// PartialEq for BreadcrumbBar
// ---------------------------------------------------------------------------

impl PartialEq for BreadcrumbBar {
    fn eq(&self, other: &Self) -> bool {
        self.items.len() == other.items.len()
            && self.selected_index == other.selected_index
            && self.is_focused == other.is_focused
    }
}

// ---------------------------------------------------------------------------
// BreadcrumbFilter
// ---------------------------------------------------------------------------

/// Filter breadcrumb elements by their kind.
#[derive(Debug, Clone)]
pub struct BreadcrumbFilter {
    pub allowed_kinds: Vec<BreadcrumbKind>,
}

impl BreadcrumbFilter {
    pub fn new() -> Self {
        Self {
            allowed_kinds: Vec::new(),
        }
    }

    pub fn allow(&mut self, kind: BreadcrumbKind) -> &mut Self {
        if !self.allowed_kinds.contains(&kind) {
            self.allowed_kinds.push(kind);
        }
        self
    }

    pub fn is_allowed(&self, kind: &BreadcrumbKind) -> bool {
        self.allowed_kinds.contains(kind)
    }

    pub fn filter_path(&self, path: &BreadcrumbPath) -> BreadcrumbPath {
        let elements = path
            .elements
            .iter()
            .filter(|e| self.is_allowed(&e.kind))
            .cloned()
            .collect();
        BreadcrumbPath { elements }
    }

    pub fn allowed_count(&self) -> usize {
        self.allowed_kinds.len()
    }
}

impl Default for BreadcrumbFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Additional BreadcrumbPath methods
// ---------------------------------------------------------------------------

impl BreadcrumbPath {
    /// Returns `true` if any element has the given kind.
    pub fn contains_kind(&self, kind: &BreadcrumbKind) -> bool {
        self.elements.iter().any(|e| &e.kind == kind)
    }

    /// Collect references to elements matching the given kind.
    pub fn elements_of_kind(&self, kind: &BreadcrumbKind) -> Vec<&BreadcrumbElement> {
        self.elements.iter().filter(|e| &e.kind == kind).collect()
    }

    /// Alias for `len()` — returns the depth of the path.
    pub fn depth(&self) -> usize {
        self.len()
    }

    /// Reverse the order of elements in-place.
    pub fn reverse(&mut self) {
        self.elements.reverse();
    }

    /// Truncate the path to at most `max_depth` elements.
    pub fn truncate(&mut self, max_depth: usize) {
        self.elements.truncate(max_depth);
    }
}

// ---------------------------------------------------------------------------
// Additional BreadcrumbBar methods
// ---------------------------------------------------------------------------

impl BreadcrumbBar {
    /// Return a reference to the currently selected item, if any.
    pub fn selected_item(&self) -> Option<&BreadcrumbItem> {
        self.items.get(self.selected_index)
    }

    /// Remove all items and reset selection.
    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_index = 0;
    }

    /// Number of items in the bar.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Select the first item.
    pub fn select_first(&mut self) {
        self.selected_index = 0;
    }

    /// Select the last item.
    pub fn select_last(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = self.items.len() - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_element(label: &str, kind: BreadcrumbKind) -> BreadcrumbElement {
        BreadcrumbElement {
            label: label.to_string(),
            kind,
            uri: None,
            range_start_line: None,
        }
    }

    fn sample_items() -> Vec<BreadcrumbItem> {
        vec![
            BreadcrumbItem::new("src", "/project/src").with_icon('📁'),
            BreadcrumbItem::new("lib.rs", "/project/src/lib.rs").with_icon('📄'),
            BreadcrumbItem::new("MyStruct", "/project/src/lib.rs#MyStruct"),
        ]
    }

    #[test]
    fn push_pop_and_len() {
        let mut path = BreadcrumbPath::new();
        assert!(path.is_empty());

        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("main.rs", BreadcrumbKind::File));
        assert_eq!(path.len(), 2);

        let popped = path.pop().unwrap();
        assert_eq!(popped.label, "main.rs");
        assert_eq!(path.len(), 1);
    }

    #[test]
    fn to_path_string_formatting() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("project", BreadcrumbKind::Folder));
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("lib.rs", BreadcrumbKind::File));
        assert_eq!(path.to_path_string(), "project > src > lib.rs");
    }

    #[test]
    fn last_returns_tail_element() {
        let mut path = BreadcrumbPath::new();
        assert!(path.last().is_none());

        path.push(sample_element("main", BreadcrumbKind::Function));
        assert_eq!(path.last().unwrap().label, "main");
        assert_eq!(path.last().unwrap().kind, BreadcrumbKind::Function);
    }

    #[test]
    fn empty_path_string() {
        let path = BreadcrumbPath::new();
        assert_eq!(path.to_path_string(), "");
    }

    #[test]
    fn bar_creation() {
        let bar = BreadcrumbBar::new();
        assert!(bar.items.is_empty());
        assert!(!bar.is_focused);
    }

    #[test]
    fn set_items_marks_last_active() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        assert_eq!(bar.items.len(), 3);
        assert!(bar.items.last().unwrap().is_active);
        assert!(!bar.items[0].is_active);
    }

    #[test]
    fn select_next_clamps() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        bar.select_next();
        assert_eq!(bar.selected_index, 1);
        bar.select_next();
        assert_eq!(bar.selected_index, 2);
        bar.select_next();
        assert_eq!(bar.selected_index, 2);
    }

    #[test]
    fn select_previous_clamps() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        bar.select_previous();
        assert_eq!(bar.selected_index, 0);
    }

    #[test]
    fn activate_returns_path() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        bar.select_next();
        let path = bar.activate();
        assert_eq!(path, Some(PathBuf::from("/project/src/lib.rs")));
        assert!(bar.items[1].is_active);
        assert!(!bar.items[2].is_active);
    }

    #[test]
    fn activate_empty_bar() {
        let mut bar = BreadcrumbBar::new();
        assert!(bar.activate().is_none());
    }

    #[test]
    fn render_does_not_panic() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        bar.is_focused = true;
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);
    }

    #[test]
    fn render_empty_no_panic() {
        let bar = BreadcrumbBar::new();
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);
    }

    #[test]
    fn default_impl() {
        let bar = BreadcrumbBar::default();
        assert!(bar.items.is_empty());
    }

    #[test]
    fn breadcrumb_item_with_icon() {
        let item = BreadcrumbItem::new("test", "/test").with_icon('📁');
        assert_eq!(item.icon, Some('📁'));
    }

    #[test]
    fn test_breadcrumb_kind_display() {
        assert_eq!(format!("{}", BreadcrumbKind::File), "file");
        assert_eq!(format!("{}", BreadcrumbKind::Folder), "folder");
        assert_eq!(format!("{}", BreadcrumbKind::Symbol), "symbol");
        assert_eq!(format!("{}", BreadcrumbKind::Class), "class");
        assert_eq!(format!("{}", BreadcrumbKind::Function), "function");
        assert_eq!(format!("{}", BreadcrumbKind::Method), "method");
        assert_eq!(format!("{}", BreadcrumbKind::Property), "property");
        assert_eq!(format!("{}", BreadcrumbKind::Enum), "enum");
        assert_eq!(format!("{}", BreadcrumbKind::Interface), "interface");
        assert_eq!(format!("{}", BreadcrumbKind::Module), "module");
    }

    #[test]
    fn test_breadcrumb_element_display() {
        let elem = sample_element("main.rs", BreadcrumbKind::File);
        assert_eq!(format!("{}", elem), "[file] main.rs");

        let elem2 = sample_element("MyClass", BreadcrumbKind::Class);
        assert_eq!(format!("{}", elem2), "[class] MyClass");
    }

    #[test]
    fn test_breadcrumb_filter_allow_and_check() {
        let mut filter = BreadcrumbFilter::new();
        assert!(!filter.is_allowed(&BreadcrumbKind::File));
        assert_eq!(filter.allowed_count(), 0);

        filter.allow(BreadcrumbKind::File);
        filter.allow(BreadcrumbKind::Folder);
        assert!(filter.is_allowed(&BreadcrumbKind::File));
        assert!(filter.is_allowed(&BreadcrumbKind::Folder));
        assert!(!filter.is_allowed(&BreadcrumbKind::Symbol));
        assert_eq!(filter.allowed_count(), 2);

        // Duplicate allow should not increase count.
        filter.allow(BreadcrumbKind::File);
        assert_eq!(filter.allowed_count(), 2);
    }

    #[test]
    fn test_breadcrumb_filter_filter_path() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("lib.rs", BreadcrumbKind::File));
        path.push(sample_element("MyStruct", BreadcrumbKind::Class));

        let mut filter = BreadcrumbFilter::new();
        filter.allow(BreadcrumbKind::Folder).allow(BreadcrumbKind::File);

        let filtered = filter.filter_path(&path);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered.elements[0].label, "src");
        assert_eq!(filtered.elements[1].label, "lib.rs");
    }

    #[test]
    fn test_breadcrumb_path_contains_kind() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("main.rs", BreadcrumbKind::File));

        assert!(path.contains_kind(&BreadcrumbKind::Folder));
        assert!(path.contains_kind(&BreadcrumbKind::File));
        assert!(!path.contains_kind(&BreadcrumbKind::Class));
    }

    #[test]
    fn test_breadcrumb_path_elements_of_kind() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("src", BreadcrumbKind::Folder));
        path.push(sample_element("lib", BreadcrumbKind::Folder));
        path.push(sample_element("main.rs", BreadcrumbKind::File));

        let folders = path.elements_of_kind(&BreadcrumbKind::Folder);
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].label, "src");
        assert_eq!(folders[1].label, "lib");

        let classes = path.elements_of_kind(&BreadcrumbKind::Class);
        assert!(classes.is_empty());
    }

    #[test]
    fn test_breadcrumb_path_depth() {
        let mut path = BreadcrumbPath::new();
        assert_eq!(path.depth(), 0);
        path.push(sample_element("a", BreadcrumbKind::Folder));
        path.push(sample_element("b", BreadcrumbKind::File));
        assert_eq!(path.depth(), 2);
        assert_eq!(path.depth(), path.len());
    }

    #[test]
    fn test_breadcrumb_path_reverse() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("a", BreadcrumbKind::Folder));
        path.push(sample_element("b", BreadcrumbKind::File));
        path.push(sample_element("c", BreadcrumbKind::Class));

        path.reverse();
        assert_eq!(path.elements[0].label, "c");
        assert_eq!(path.elements[1].label, "b");
        assert_eq!(path.elements[2].label, "a");
    }

    #[test]
    fn test_breadcrumb_path_truncate() {
        let mut path = BreadcrumbPath::new();
        path.push(sample_element("a", BreadcrumbKind::Folder));
        path.push(sample_element("b", BreadcrumbKind::File));
        path.push(sample_element("c", BreadcrumbKind::Class));

        path.truncate(2);
        assert_eq!(path.len(), 2);
        assert_eq!(path.elements[0].label, "a");
        assert_eq!(path.elements[1].label, "b");
    }

    #[test]
    fn test_bar_selected_item() {
        let mut bar = BreadcrumbBar::new();
        assert!(bar.selected_item().is_none());

        bar.set_items(sample_items());
        assert_eq!(bar.selected_item().unwrap().label, "src");

        bar.select_next();
        assert_eq!(bar.selected_item().unwrap().label, "lib.rs");
    }

    #[test]
    fn test_bar_clear() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());
        assert_eq!(bar.item_count(), 3);

        bar.clear();
        assert_eq!(bar.item_count(), 0);
        assert_eq!(bar.selected_index, 0);
        assert!(bar.selected_item().is_none());
    }

    #[test]
    fn test_bar_select_first_last() {
        let mut bar = BreadcrumbBar::new();
        bar.set_items(sample_items());

        bar.select_last();
        assert_eq!(bar.selected_index, 2);
        assert_eq!(bar.selected_item().unwrap().label, "MyStruct");

        bar.select_first();
        assert_eq!(bar.selected_index, 0);
        assert_eq!(bar.selected_item().unwrap().label, "src");
    }

    #[test]
    fn test_bar_partial_eq() {
        let mut bar1 = BreadcrumbBar::new();
        let mut bar2 = BreadcrumbBar::new();
        assert_eq!(bar1, bar2);

        bar1.set_items(sample_items());
        bar2.set_items(sample_items());
        assert_eq!(bar1, bar2);

        bar1.select_next();
        assert_ne!(bar1, bar2);

        bar2.select_next();
        assert_eq!(bar1, bar2);

        bar1.is_focused = true;
        assert_ne!(bar1, bar2);
    }
}
