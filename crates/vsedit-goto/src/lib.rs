//! Go to definition, declaration, implementation, type definition, and references.
//!
//! Provides the location link model and a service that aggregates multiple
//! providers to resolve go-to requests.

use std::fmt;
/// Location of a symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub uri: String,
    pub line: u32,
    pub column: u32,
    pub end_line: Option<u32>,
    pub end_column: Option<u32>,
}

impl Location {
    pub fn new(uri: impl Into<String>, line: u32, column: u32) -> Self {
        Self { uri: uri.into(), line, column, end_line: None, end_column: None }
    }

    pub fn with_range(
        uri: impl Into<String>,
        line: u32,
        column: u32,
        end_line: u32,
        end_column: u32,
    ) -> Self {
        Self {
            uri: uri.into(),
            line,
            column,
            end_line: Some(end_line),
            end_column: Some(end_column),
        }
    }

    /// Format as "uri:line:col".
    pub fn display_string(&self) -> String {
        format!("{}:{}:{}", self.uri, self.line, self.column)
    }

    /// Check if two locations refer to the same file.
    pub fn is_same_file(&self, other: &Location) -> bool {
        self.uri == other.uri
    }

    /// Compute absolute line distance between two locations.
    pub fn distance(&self, other: &Location) -> u32 {
        if self.line >= other.line {
            self.line - other.line
        } else {
            other.line - self.line
        }
    }
}

/// A location link with both origin and target ranges (LSP LocationLink).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocationLink {
    /// URI of the target document.
    pub target_uri: String,
    /// Full range of the target (e.g., entire function).
    pub target_range: (u32, u32, u32, u32),
    /// Precise range within the target (e.g., function name).
    pub target_selection_range: (u32, u32, u32, u32),
    /// Range in the originating document that triggered the request.
    pub origin_range: Option<(u32, u32, u32, u32)>,
}

impl LocationLink {
    pub fn new(
        target_uri: impl Into<String>,
        target_range: (u32, u32, u32, u32),
        target_selection_range: (u32, u32, u32, u32),
    ) -> Self {
        Self {
            target_uri: target_uri.into(),
            target_range,
            target_selection_range,
            origin_range: None,
        }
    }

    pub fn with_origin(mut self, origin: (u32, u32, u32, u32)) -> Self {
        self.origin_range = Some(origin);
        self
    }

    /// Format the target location as "target_uri:line:col".
    pub fn display_string(&self) -> String {
        format!(
            "{}:{}:{}",
            self.target_uri,
            self.target_selection_range.0,
            self.target_selection_range.1,
        )
    }

    /// Convert to a simple Location using the selection range.
    pub fn to_location(&self) -> Location {
        Location::with_range(
            &self.target_uri,
            self.target_selection_range.0,
            self.target_selection_range.1,
            self.target_selection_range.2,
            self.target_selection_range.3,
        )
    }
}

/// Result of a go-to operation.
#[derive(Debug, Clone, PartialEq)]
pub enum GotoResult {
    Single(LocationLink),
    Multiple(Vec<LocationLink>),
    None,
}

impl GotoResult {
    /// Number of locations in the result.
    pub fn len(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Multiple(v) => v.len(),
            Self::None => 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Flatten into a vec of location links.
    pub fn into_links(self) -> Vec<LocationLink> {
        match self {
            Self::Single(l) => vec![l],
            Self::Multiple(v) => v,
            Self::None => vec![],
        }
    }

    /// Get the first link, if any.
    pub fn first(&self) -> Option<&LocationLink> {
        match self {
            Self::Single(l) => Some(l),
            Self::Multiple(v) => v.first(),
            Self::None => None,
        }
    }

    /// Merge two `GotoResult`s into one.
    pub fn merge(self, other: GotoResult) -> GotoResult {
        let mut links = self.into_links();
        links.extend(other.into_links());
        match links.len() {
            0 => GotoResult::None,
            1 => GotoResult::Single(links.into_iter().next().unwrap()),
            _ => GotoResult::Multiple(links),
        }
    }

    /// Remove duplicate links by (target_uri, target_selection_range).
    pub fn deduplicate(self) -> GotoResult {
        let links = self.into_links();
        let mut seen = Vec::<(String, (u32, u32, u32, u32))>::new();
        let mut unique = Vec::new();
        for link in links {
            let key = (link.target_uri.clone(), link.target_selection_range);
            if !seen.contains(&key) {
                seen.push(key);
                unique.push(link);
            }
        }
        match unique.len() {
            0 => GotoResult::None,
            1 => GotoResult::Single(unique.into_iter().next().unwrap()),
            _ => GotoResult::Multiple(unique),
        }
    }
}

/// Result of a go-to operation using simple locations (backward compat).
pub type GoToResult = GotoResult;

/// Unified provider trait for all go-to operations.
pub trait GotoProvider: Send + Sync {
    fn definition(&self, uri: &str, line: u32, column: u32) -> GotoResult;
    fn declaration(&self, uri: &str, line: u32, column: u32) -> GotoResult;
    fn implementation(&self, uri: &str, line: u32, column: u32) -> GotoResult;
    fn type_definition(&self, uri: &str, line: u32, column: u32) -> GotoResult;
    fn references(&self, uri: &str, line: u32, column: u32, include_declaration: bool) -> Vec<LocationLink>;
}

/// Provider for go-to definition (backward compat).
pub trait DefinitionProvider: Send + Sync {
    fn provide_definition(&self, uri: &str, line: u32, column: u32) -> GotoResult;
}

pub trait DeclarationProvider: Send + Sync {
    fn provide_declaration(&self, uri: &str, line: u32, column: u32) -> GotoResult;
}

pub trait ReferenceProvider: Send + Sync {
    fn provide_references(&self, uri: &str, line: u32, column: u32, include_declaration: bool) -> Vec<Location>;
}

pub trait TypeDefinitionProvider: Send + Sync {
    fn provide_type_definition(&self, uri: &str, line: u32, column: u32) -> GotoResult;
}

pub trait ImplementationProvider: Send + Sync {
    fn provide_implementation(&self, uri: &str, line: u32, column: u32) -> GotoResult;
}

/// Service that aggregates multiple GotoProvider instances and resolves results.
pub struct GotoService {
    providers: Vec<Box<dyn GotoProvider>>,
}

impl GotoService {
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    pub fn register(&mut self, provider: Box<dyn GotoProvider>) {
        self.providers.push(provider);
    }

    /// Number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    pub fn definition(&self, uri: &str, line: u32, column: u32) -> GotoResult {
        self.resolve(|p| p.definition(uri, line, column))
    }

    pub fn declaration(&self, uri: &str, line: u32, column: u32) -> GotoResult {
        self.resolve(|p| p.declaration(uri, line, column))
    }

    pub fn implementation(&self, uri: &str, line: u32, column: u32) -> GotoResult {
        self.resolve(|p| p.implementation(uri, line, column))
    }

    pub fn type_definition(&self, uri: &str, line: u32, column: u32) -> GotoResult {
        self.resolve(|p| p.type_definition(uri, line, column))
    }

    pub fn references(&self, uri: &str, line: u32, column: u32, include_declaration: bool) -> Vec<LocationLink> {
        let mut all = Vec::new();
        for provider in &self.providers {
            all.extend(provider.references(uri, line, column, include_declaration));
        }
        all
    }

    /// Fallible definition: returns `Err` if no providers or no results.
    pub fn try_definition(&self, uri: &str, line: u32, column: u32) -> Result<GotoResult, GotoError> {
        self.try_resolve(|p| p.definition(uri, line, column))
    }

    /// Fallible declaration.
    pub fn try_declaration(&self, uri: &str, line: u32, column: u32) -> Result<GotoResult, GotoError> {
        self.try_resolve(|p| p.declaration(uri, line, column))
    }

    /// Fallible implementation.
    pub fn try_implementation(&self, uri: &str, line: u32, column: u32) -> Result<GotoResult, GotoError> {
        self.try_resolve(|p| p.implementation(uri, line, column))
    }

    /// Fallible type definition.
    pub fn try_type_definition(&self, uri: &str, line: u32, column: u32) -> Result<GotoResult, GotoError> {
        self.try_resolve(|p| p.type_definition(uri, line, column))
    }

    fn resolve<F>(&self, f: F) -> GotoResult
    where
        F: Fn(&dyn GotoProvider) -> GotoResult,
    {
        let mut all_links = Vec::new();
        for provider in &self.providers {
            match f(provider.as_ref()) {
                GotoResult::Single(link) => all_links.push(link),
                GotoResult::Multiple(links) => all_links.extend(links),
                GotoResult::None => {}
            }
        }
        match all_links.len() {
            0 => GotoResult::None,
            1 => GotoResult::Single(all_links.into_iter().next().unwrap()),
            _ => GotoResult::Multiple(all_links),
        }
    }

    fn try_resolve<F>(&self, f: F) -> Result<GotoResult, GotoError>
    where
        F: Fn(&dyn GotoProvider) -> GotoResult,
    {
        if self.providers.is_empty() {
            return Err(GotoError::NoProviders);
        }
        let result = self.resolve(f);
        if result.is_empty() {
            return Err(GotoError::NoResults);
        }
        Ok(result)
    }
}

impl Default for GotoService {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors returned by fallible go-to operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GotoError {
    /// No providers have been registered.
    NoProviders,
    /// Providers returned no results.
    NoResults,
    /// The requested position is invalid.
    InvalidPosition,
}

impl core::fmt::Display for GotoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoProviders => write!(f, "no goto providers registered"),
            Self::NoResults => write!(f, "no results found"),
            Self::InvalidPosition => write!(f, "invalid position"),
        }
    }
}

/// Browser-style navigation history for visited locations.
#[derive(Debug)]
pub struct GotoHistory {
    entries: Vec<Location>,
    cursor: usize,
}

impl GotoHistory {
    pub fn new() -> Self {
        Self { entries: Vec::new(), cursor: 0 }
    }

    /// Push a new location, discarding any forward history.
    pub fn push(&mut self, location: Location) {
        if !self.entries.is_empty() {
            self.entries.truncate(self.cursor + 1);
        }
        self.entries.push(location);
        self.cursor = self.entries.len() - 1;
    }

    /// Navigate back, returning the previous location if possible.
    pub fn go_back(&mut self) -> Option<&Location> {
        if self.can_go_back() {
            self.cursor -= 1;
            Some(&self.entries[self.cursor])
        } else {
            None
        }
    }

    /// Navigate forward, returning the next location if possible.
    pub fn go_forward(&mut self) -> Option<&Location> {
        if self.can_go_forward() {
            self.cursor += 1;
            Some(&self.entries[self.cursor])
        } else {
            None
        }
    }

    /// Current location, if any.
    pub fn current(&self) -> Option<&Location> {
        self.entries.get(self.cursor)
    }

    pub fn can_go_back(&self) -> bool {
        self.cursor > 0 && !self.entries.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.entries.is_empty() && self.cursor < self.entries.len() - 1
    }
}

impl Default for GotoHistory {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// GoToAction — the kind of navigation requested
// ---------------------------------------------------------------------------

/// Which go-to variant the user invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GoToAction {
    Definition,
    Declaration,
    TypeDefinition,
    Implementation,
    References,
}

impl fmt::Display for GoToAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Definition => write!(f, "Go to Definition"),
            Self::Declaration => write!(f, "Go to Declaration"),
            Self::TypeDefinition => write!(f, "Go to Type Definition"),
            Self::Implementation => write!(f, "Go to Implementation"),
            Self::References => write!(f, "Find All References"),
        }
    }
}

// ---------------------------------------------------------------------------
// DefinitionService — convenience wrapper around GotoService
// ---------------------------------------------------------------------------

/// High-level service for resolving definitions that handles single vs
/// multiple-result logic.
pub struct DefinitionService {
    inner: GotoService,
}

impl DefinitionService {
    pub fn new() -> Self {
        Self { inner: GotoService::new() }
    }

    pub fn register(&mut self, provider: Box<dyn GotoProvider>) {
        self.inner.register(provider);
    }

    /// Resolve a go-to action. Returns the merged, deduplicated result.
    pub fn resolve(&self, action: GoToAction, uri: &str, line: u32, col: u32) -> GotoResult {
        let result = match action {
            GoToAction::Definition => self.inner.definition(uri, line, col),
            GoToAction::Declaration => self.inner.declaration(uri, line, col),
            GoToAction::TypeDefinition => self.inner.type_definition(uri, line, col),
            GoToAction::Implementation => self.inner.implementation(uri, line, col),
            GoToAction::References => {
                let links = self.inner.references(uri, line, col, true);
                match links.len() {
                    0 => GotoResult::None,
                    1 => GotoResult::Single(links.into_iter().next().unwrap()),
                    _ => GotoResult::Multiple(links),
                }
            }
        };
        result.deduplicate()
    }

    /// Returns `true` when exactly one result was found (navigate directly).
    pub fn should_navigate(&self, result: &GotoResult) -> bool {
        result.len() == 1
    }

    /// Returns `true` when multiple results were found (show peek/picker).
    pub fn should_peek(&self, result: &GotoResult) -> bool {
        result.len() > 1
    }
}

impl Default for DefinitionService {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience: resolve a definition for a given position.
pub fn goto_definition(service: &GotoService, uri: &str, line: u32, col: u32) -> GotoResult {
    service.definition(uri, line, col).deduplicate()
}

/// Convenience: resolve references for a given position.
pub fn find_references(
    service: &GotoService,
    uri: &str,
    line: u32,
    col: u32,
    include_declaration: bool,
) -> Vec<LocationLink> {
    service.references(uri, line, col, include_declaration)
}

// ---------------------------------------------------------------------------
// PeekState — model for the inline peek widget
// ---------------------------------------------------------------------------

/// State for an inline peek view showing code at another location.
#[derive(Debug, Clone)]
pub struct PeekState {
    /// All results to display in the peek widget.
    pub results: Vec<LocationLink>,
    /// Index of the currently selected result.
    pub selected_index: usize,
    /// Whether the peek widget is visible.
    pub visible: bool,
    /// Title displayed in the peek title bar.
    pub title: String,
}

impl PeekState {
    pub fn new(title: impl Into<String>, results: Vec<LocationLink>) -> Self {
        Self {
            results,
            selected_index: 0,
            visible: true,
            title: title.into(),
        }
    }

    /// The currently selected result, if any.
    pub fn selected(&self) -> Option<&LocationLink> {
        self.results.get(self.selected_index)
    }

    /// Select the next result, wrapping around.
    pub fn select_next(&mut self) {
        if !self.results.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.results.len();
        }
    }

    /// Select the previous result, wrapping around.
    pub fn select_previous(&mut self) {
        if !self.results.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.results.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Close the peek widget.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Open the peek widget.
    pub fn open(&mut self) {
        self.visible = true;
    }

    /// Number of results.
    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    /// Label for the selected result (e.g. "2 of 5").
    pub fn selection_label(&self) -> String {
        if self.results.is_empty() {
            "No results".to_string()
        } else {
            format!("{} of {}", self.selected_index + 1, self.results.len())
        }
    }

    /// Navigate to the selected result (returns it and closes the peek).
    pub fn accept(&mut self) -> Option<LocationLink> {
        let result = self.selected().cloned();
        self.close();
        result
    }

    /// Group results by target URI.
    pub fn results_by_file(&self) -> Vec<(&str, Vec<&LocationLink>)> {
        let mut files: Vec<&str> = self.results.iter().map(|r| r.target_uri.as_str()).collect();
        files.sort_unstable();
        files.dedup();
        files
            .into_iter()
            .map(|uri| {
                let links: Vec<&LocationLink> =
                    self.results.iter().filter(|r| r.target_uri == uri).collect();
                (uri, links)
            })
            .collect()
    }
}

impl Default for PeekState {
    fn default() -> Self {
        Self {
            results: Vec::new(),
            selected_index: 0,
            visible: false,
            title: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// LSP definition parsing
// ---------------------------------------------------------------------------

/// Parse a single LSP `Location` object into our `Location` type.
fn parse_single_location(val: &serde_json::Value) -> Option<Location> {
    let uri = val.get("uri")?.as_str()?;
    let range = val.get("range")?;
    let start = range.get("start")?;
    let line = start.get("line")?.as_u64()? as u32;
    let col = start.get("character")?.as_u64()? as u32;

    let (end_line, end_col) = range.get("end").and_then(|end| {
        let el = end.get("line")?.as_u64()? as u32;
        let ec = end.get("character")?.as_u64()? as u32;
        Some((el, ec))
    }).unzip();

    Some(Location {
        uri: uri.to_string(),
        line,
        column: col,
        end_line,
        end_column: end_col,
    })
}

/// Parse an LSP definition response into a list of `Location`s.
///
/// Handles both a single `Location` object and a `Location[]` array.
pub fn parse_lsp_definition(response: &serde_json::Value) -> Vec<Location> {
    if let Some(loc) = parse_single_location(response) {
        vec![loc]
    } else if let Some(arr) = response.as_array() {
        arr.iter().filter_map(parse_single_location).collect()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_creation() {
        let loc = Location::new("file:///test.rs", 10, 5);
        assert_eq!(loc.line, 10);
        assert_eq!(loc.column, 5);
        assert!(loc.end_line.is_none());
    }

    #[test]
    fn location_with_range() {
        let loc = Location::with_range("file:///a.rs", 1, 0, 1, 10);
        assert_eq!(loc.end_line, Some(1));
        assert_eq!(loc.end_column, Some(10));
    }

    #[test]
    fn location_link_to_location() {
        let link = LocationLink::new("file:///b.rs", (1, 0, 5, 0), (2, 4, 2, 10));
        let loc = link.to_location();
        assert_eq!(loc.uri, "file:///b.rs");
        assert_eq!(loc.line, 2);
        assert_eq!(loc.column, 4);
    }

    #[test]
    fn location_link_with_origin() {
        let link = LocationLink::new("f", (0, 0, 0, 0), (0, 0, 0, 0))
            .with_origin((10, 5, 10, 15));
        assert_eq!(link.origin_range, Some((10, 5, 10, 15)));
    }

    #[test]
    fn goto_result_len() {
        assert_eq!(GotoResult::None.len(), 0);
        assert!(GotoResult::None.is_empty());
        let single = GotoResult::Single(LocationLink::new("f", (0,0,0,0), (0,0,0,0)));
        assert_eq!(single.len(), 1);
        assert!(!single.is_empty());
    }

    #[test]
    fn goto_result_into_links() {
        let r = GotoResult::Multiple(vec![
            LocationLink::new("a", (0,0,0,0), (0,0,0,0)),
            LocationLink::new("b", (0,0,0,0), (0,0,0,0)),
        ]);
        let links = r.into_links();
        assert_eq!(links.len(), 2);
    }

    struct TestProvider {
        result: GotoResult,
    }

    impl GotoProvider for TestProvider {
        fn definition(&self, _uri: &str, _line: u32, _col: u32) -> GotoResult {
            self.result.clone()
        }
        fn declaration(&self, _uri: &str, _line: u32, _col: u32) -> GotoResult {
            GotoResult::None
        }
        fn implementation(&self, _uri: &str, _line: u32, _col: u32) -> GotoResult {
            GotoResult::None
        }
        fn type_definition(&self, _uri: &str, _line: u32, _col: u32) -> GotoResult {
            GotoResult::None
        }
        fn references(&self, _uri: &str, _line: u32, _col: u32, _incl: bool) -> Vec<LocationLink> {
            vec![]
        }
    }

    #[test]
    fn service_single_provider() {
        let mut svc = GotoService::new();
        svc.register(Box::new(TestProvider {
            result: GotoResult::Single(LocationLink::new("file:///x.rs", (1,0,10,0), (3,4,3,12))),
        }));
        let result = svc.definition("file:///a.rs", 5, 10);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn service_multiple_providers_merge() {
        let mut svc = GotoService::new();
        svc.register(Box::new(TestProvider {
            result: GotoResult::Single(LocationLink::new("a", (0,0,0,0), (0,0,0,0))),
        }));
        svc.register(Box::new(TestProvider {
            result: GotoResult::Single(LocationLink::new("b", (0,0,0,0), (0,0,0,0))),
        }));
        let result = svc.definition("f", 1, 1);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn service_no_providers_returns_none() {
        let svc = GotoService::new();
        assert!(svc.definition("f", 1, 1).is_empty());
    }

    #[test]
    fn goto_result_variants_compat() {
        let r = GotoResult::Single(LocationLink::new("f", (0,0,0,0), (1,1,1,1)));
        assert!(matches!(r, GotoResult::Single(_)));
        let r = GotoResult::Multiple(vec![]);
        assert!(matches!(r, GotoResult::Multiple(_)));
    }

    // --- new tests ---

    #[test]
    fn location_display_string() {
        let loc = Location::new("file:///main.rs", 42, 7);
        assert_eq!(loc.display_string(), "file:///main.rs:42:7");
    }

    #[test]
    fn location_is_same_file() {
        let a = Location::new("file:///a.rs", 1, 0);
        let b = Location::new("file:///a.rs", 50, 3);
        let c = Location::new("file:///b.rs", 1, 0);
        assert!(a.is_same_file(&b));
        assert!(!a.is_same_file(&c));
    }

    #[test]
    fn location_distance() {
        let a = Location::new("f", 10, 0);
        let b = Location::new("f", 25, 0);
        assert_eq!(a.distance(&b), 15);
        assert_eq!(b.distance(&a), 15);
        assert_eq!(a.distance(&a), 0);
    }

    #[test]
    fn location_link_display_string() {
        let link = LocationLink::new("file:///lib.rs", (0,0,100,0), (5, 10, 5, 20));
        assert_eq!(link.display_string(), "file:///lib.rs:5:10");
    }

    #[test]
    fn goto_result_first_single() {
        let link = LocationLink::new("x", (0,0,0,0), (1,2,1,5));
        let r = GotoResult::Single(link.clone());
        assert_eq!(r.first().unwrap().target_uri, "x");
    }

    #[test]
    fn goto_result_first_multiple() {
        let r = GotoResult::Multiple(vec![
            LocationLink::new("a", (0,0,0,0), (0,0,0,0)),
            LocationLink::new("b", (0,0,0,0), (0,0,0,0)),
        ]);
        assert_eq!(r.first().unwrap().target_uri, "a");
    }

    #[test]
    fn goto_result_first_none() {
        assert!(GotoResult::None.first().is_none());
    }

    #[test]
    fn goto_result_merge() {
        let a = GotoResult::Single(LocationLink::new("a", (0,0,0,0), (0,0,0,0)));
        let b = GotoResult::Single(LocationLink::new("b", (0,0,0,0), (0,0,0,0)));
        let merged = a.merge(b);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn goto_result_merge_with_none() {
        let a = GotoResult::Single(LocationLink::new("a", (0,0,0,0), (0,0,0,0)));
        let merged = a.merge(GotoResult::None);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn goto_result_merge_both_none() {
        let merged = GotoResult::None.merge(GotoResult::None);
        assert!(merged.is_empty());
    }

    #[test]
    fn goto_result_deduplicate() {
        let r = GotoResult::Multiple(vec![
            LocationLink::new("a", (0,0,10,0), (1,0,1,5)),
            LocationLink::new("a", (0,0,10,0), (1,0,1,5)),
            LocationLink::new("b", (0,0,10,0), (2,0,2,5)),
        ]);
        let deduped = r.deduplicate();
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn goto_result_deduplicate_none() {
        let deduped = GotoResult::None.deduplicate();
        assert!(deduped.is_empty());
    }

    #[test]
    fn service_provider_count() {
        let mut svc = GotoService::new();
        assert_eq!(svc.provider_count(), 0);
        svc.register(Box::new(TestProvider { result: GotoResult::None }));
        assert_eq!(svc.provider_count(), 1);
        svc.register(Box::new(TestProvider { result: GotoResult::None }));
        assert_eq!(svc.provider_count(), 2);
    }

    #[test]
    fn try_definition_no_providers() {
        let svc = GotoService::new();
        assert_eq!(svc.try_definition("f", 1, 1), Err(GotoError::NoProviders));
    }

    #[test]
    fn try_definition_no_results() {
        let mut svc = GotoService::new();
        svc.register(Box::new(TestProvider { result: GotoResult::None }));
        assert_eq!(svc.try_definition("f", 1, 1), Err(GotoError::NoResults));
    }

    #[test]
    fn try_definition_success() {
        let mut svc = GotoService::new();
        svc.register(Box::new(TestProvider {
            result: GotoResult::Single(LocationLink::new("x", (0,0,0,0), (1,0,1,5))),
        }));
        let result = svc.try_definition("f", 1, 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn try_declaration_no_providers() {
        let svc = GotoService::new();
        assert_eq!(svc.try_declaration("f", 0, 0), Err(GotoError::NoProviders));
    }

    #[test]
    fn goto_error_display() {
        assert_eq!(GotoError::NoProviders.to_string(), "no goto providers registered");
        assert_eq!(GotoError::NoResults.to_string(), "no results found");
        assert_eq!(GotoError::InvalidPosition.to_string(), "invalid position");
    }

    #[test]
    fn history_push_and_current() {
        let mut hist = GotoHistory::new();
        assert!(hist.current().is_none());
        hist.push(Location::new("a", 1, 0));
        assert_eq!(hist.current().unwrap().uri, "a");
    }

    #[test]
    fn history_back_forward() {
        let mut hist = GotoHistory::new();
        hist.push(Location::new("a", 1, 0));
        hist.push(Location::new("b", 2, 0));
        hist.push(Location::new("c", 3, 0));

        assert_eq!(hist.current().unwrap().uri, "c");
        assert!(hist.can_go_back());
        assert!(!hist.can_go_forward());

        let back = hist.go_back().unwrap();
        assert_eq!(back.uri, "b");
        assert!(hist.can_go_forward());

        let back2 = hist.go_back().unwrap();
        assert_eq!(back2.uri, "a");
        assert!(!hist.can_go_back());

        let fwd = hist.go_forward().unwrap();
        assert_eq!(fwd.uri, "b");
    }

    #[test]
    fn history_push_truncates_forward() {
        let mut hist = GotoHistory::new();
        hist.push(Location::new("a", 1, 0));
        hist.push(Location::new("b", 2, 0));
        hist.push(Location::new("c", 3, 0));
        hist.go_back();
        hist.go_back();
        // Now at "a", push "d" should discard "b" and "c"
        hist.push(Location::new("d", 4, 0));
        assert!(!hist.can_go_forward());
        assert_eq!(hist.current().unwrap().uri, "d");
        let back = hist.go_back().unwrap();
        assert_eq!(back.uri, "a");
        assert!(hist.go_forward().is_some());
        assert!(hist.go_forward().is_none());
    }

    #[test]
    fn history_empty_navigation() {
        let mut hist = GotoHistory::new();
        assert!(!hist.can_go_back());
        assert!(!hist.can_go_forward());
        assert!(hist.go_back().is_none());
        assert!(hist.go_forward().is_none());
    }

    #[test]
    fn history_single_entry() {
        let mut hist = GotoHistory::new();
        hist.push(Location::new("only", 0, 0));
        assert!(!hist.can_go_back());
        assert!(!hist.can_go_forward());
        assert!(hist.go_back().is_none());
    }

    // --- GoToAction tests ---

    #[test]
    fn goto_action_display() {
        assert_eq!(GoToAction::Definition.to_string(), "Go to Definition");
        assert_eq!(GoToAction::Declaration.to_string(), "Go to Declaration");
        assert_eq!(GoToAction::TypeDefinition.to_string(), "Go to Type Definition");
        assert_eq!(GoToAction::Implementation.to_string(), "Go to Implementation");
        assert_eq!(GoToAction::References.to_string(), "Find All References");
    }

    #[test]
    fn goto_action_eq() {
        assert_eq!(GoToAction::Definition, GoToAction::Definition);
        assert_ne!(GoToAction::Definition, GoToAction::Declaration);
    }

    // --- DefinitionService tests ---

    #[test]
    fn definition_service_resolve_definition() {
        let mut svc = DefinitionService::new();
        svc.register(Box::new(TestProvider {
            result: GotoResult::Single(LocationLink::new("x", (0,0,0,0), (1,0,1,5))),
        }));
        let result = svc.resolve(GoToAction::Definition, "f", 1, 1);
        assert_eq!(result.len(), 1);
        assert!(svc.should_navigate(&result));
        assert!(!svc.should_peek(&result));
    }

    #[test]
    fn definition_service_resolve_multiple() {
        let mut svc = DefinitionService::new();
        svc.register(Box::new(TestProvider {
            result: GotoResult::Multiple(vec![
                LocationLink::new("a", (0,0,0,0), (0,0,0,0)),
                LocationLink::new("b", (0,0,0,0), (0,0,0,0)),
            ]),
        }));
        let result = svc.resolve(GoToAction::Definition, "f", 1, 1);
        assert_eq!(result.len(), 2);
        assert!(!svc.should_navigate(&result));
        assert!(svc.should_peek(&result));
    }

    #[test]
    fn definition_service_resolve_none() {
        let svc = DefinitionService::default();
        let result = svc.resolve(GoToAction::Definition, "f", 1, 1);
        assert!(result.is_empty());
    }

    #[test]
    fn goto_definition_convenience() {
        let mut svc = GotoService::new();
        svc.register(Box::new(TestProvider {
            result: GotoResult::Single(LocationLink::new("x", (0,0,0,0), (1,0,1,5))),
        }));
        let result = goto_definition(&svc, "f", 1, 1);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn find_references_convenience() {
        let svc = GotoService::new();
        let refs = find_references(&svc, "f", 1, 1, true);
        assert!(refs.is_empty());
    }

    // --- PeekState tests ---

    #[test]
    fn peek_state_new() {
        let links = vec![
            LocationLink::new("a.rs", (0,0,10,0), (5,0,5,10)),
            LocationLink::new("b.rs", (0,0,20,0), (8,0,8,15)),
        ];
        let peek = PeekState::new("Definition", links);
        assert!(peek.visible);
        assert_eq!(peek.result_count(), 2);
        assert_eq!(peek.selected_index, 0);
        assert_eq!(peek.selected().unwrap().target_uri, "a.rs");
    }

    #[test]
    fn peek_state_navigation() {
        let links = vec![
            LocationLink::new("a", (0,0,0,0), (0,0,0,0)),
            LocationLink::new("b", (0,0,0,0), (0,0,0,0)),
            LocationLink::new("c", (0,0,0,0), (0,0,0,0)),
        ];
        let mut peek = PeekState::new("Test", links);

        peek.select_next();
        assert_eq!(peek.selected().unwrap().target_uri, "b");

        peek.select_next();
        assert_eq!(peek.selected().unwrap().target_uri, "c");

        peek.select_next(); // wraps
        assert_eq!(peek.selected().unwrap().target_uri, "a");

        peek.select_previous(); // wraps back
        assert_eq!(peek.selected().unwrap().target_uri, "c");
    }

    #[test]
    fn peek_state_selection_label() {
        let peek = PeekState::new("T", vec![
            LocationLink::new("a", (0,0,0,0), (0,0,0,0)),
            LocationLink::new("b", (0,0,0,0), (0,0,0,0)),
        ]);
        assert_eq!(peek.selection_label(), "1 of 2");

        let empty = PeekState::default();
        assert_eq!(empty.selection_label(), "No results");
    }

    #[test]
    fn peek_state_accept() {
        let mut peek = PeekState::new("T", vec![
            LocationLink::new("x", (0,0,0,0), (1,2,1,5)),
        ]);
        let accepted = peek.accept();
        assert_eq!(accepted.unwrap().target_uri, "x");
        assert!(!peek.visible);
    }

    #[test]
    fn peek_state_close_open() {
        let mut peek = PeekState::new("T", vec![]);
        assert!(peek.visible);
        peek.close();
        assert!(!peek.visible);
        peek.open();
        assert!(peek.visible);
    }

    #[test]
    fn peek_state_results_by_file() {
        let peek = PeekState::new("T", vec![
            LocationLink::new("a.rs", (0,0,0,0), (1,0,1,5)),
            LocationLink::new("b.rs", (0,0,0,0), (2,0,2,5)),
            LocationLink::new("a.rs", (0,0,0,0), (3,0,3,5)),
        ]);
        let grouped = peek.results_by_file();
        assert_eq!(grouped.len(), 2);
    }

    // -----------------------------------------------------------------------
    // LSP definition parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_lsp_definition_single_location() {
        let json = serde_json::json!({
            "uri": "file:///src/main.rs",
            "range": {
                "start": { "line": 10, "character": 4 },
                "end": { "line": 10, "character": 12 }
            }
        });
        let locs = parse_lsp_definition(&json);
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].uri, "file:///src/main.rs");
        assert_eq!(locs[0].line, 10);
        assert_eq!(locs[0].column, 4);
        assert_eq!(locs[0].end_line, Some(10));
        assert_eq!(locs[0].end_column, Some(12));
    }

    #[test]
    fn parse_lsp_definition_array() {
        let json = serde_json::json!([
            {
                "uri": "file:///a.rs",
                "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 1, "character": 5 } }
            },
            {
                "uri": "file:///b.rs",
                "range": { "start": { "line": 20, "character": 3 }, "end": { "line": 20, "character": 10 } }
            },
        ]);
        let locs = parse_lsp_definition(&json);
        assert_eq!(locs.len(), 2);
        assert_eq!(locs[0].uri, "file:///a.rs");
        assert_eq!(locs[1].line, 20);
    }

    #[test]
    fn parse_lsp_definition_empty() {
        assert!(parse_lsp_definition(&serde_json::json!(null)).is_empty());
        assert!(parse_lsp_definition(&serde_json::json!({})).is_empty());
        assert!(parse_lsp_definition(&serde_json::json!([])).is_empty());
    }

    #[test]
    fn parse_lsp_definition_malformed() {
        let json = serde_json::json!([
            { "uri": "file:///a.rs" },                             // missing range
            { "range": { "start": { "line": 0, "character": 0 } } }, // missing uri
            { "uri": "file:///ok.rs", "range": { "start": { "line": 5, "character": 2 }, "end": { "line": 5, "character": 8 } } },
        ]);
        let locs = parse_lsp_definition(&json);
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].uri, "file:///ok.rs");
    }

    #[test]
    fn parse_lsp_definition_no_end_range() {
        let json = serde_json::json!({
            "uri": "file:///test.rs",
            "range": { "start": { "line": 3, "character": 7 } }
        });
        let locs = parse_lsp_definition(&json);
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].line, 3);
        assert_eq!(locs[0].column, 7);
        assert!(locs[0].end_line.is_none());
        assert!(locs[0].end_column.is_none());
    }
}
