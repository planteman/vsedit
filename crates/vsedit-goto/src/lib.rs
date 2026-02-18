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

    /// Create a location at a specific line with column defaulting to 0.
    pub fn with_line(uri: impl Into<String>, line: u32) -> Self {
        Self { uri: uri.into(), line, column: 0, end_line: None, end_column: None }
    }

    /// Extract the file name component from the URI.
    pub fn file_name(&self) -> &str {
        self.uri.rsplit('/').next().unwrap_or(&self.uri)
    }

    /// Returns `true` if `self` comes before `other` in the same file.
    ///
    /// Compares by line first, then column. Returns `false` when the
    /// locations are in different files.
    pub fn is_before(&self, other: &Location) -> bool {
        if self.uri != other.uri {
            return false;
        }
        (self.line, self.column) < (other.line, other.column)
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
    /// URI of the originating document.
    pub origin_uri: Option<String>,
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
            origin_uri: None,
        }
    }

    pub fn with_origin(mut self, origin: (u32, u32, u32, u32)) -> Self {
        self.origin_range = Some(origin);
        self
    }

    /// Returns `true` when the origin and target are in the same file.
    pub fn is_same_file(&self) -> bool {
        self.origin_uri.as_deref() == Some(self.target_uri.as_str())
    }

    /// Set the origin URI (builder pattern).
    pub fn with_origin_uri(mut self, uri: impl Into<String>) -> Self {
        self.origin_uri = Some(uri.into());
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

    /// Filter links to only those targeting the given file URI.
    pub fn filter_by_file(&self, uri: &str) -> GotoResult {
        let links: Vec<LocationLink> = match self {
            Self::Single(l) if l.target_uri == uri => vec![l.clone()],
            Self::Multiple(v) => v.iter().filter(|l| l.target_uri == uri).cloned().collect(),
            _ => vec![],
        };
        match links.len() {
            0 => GotoResult::None,
            1 => GotoResult::Single(links.into_iter().next().unwrap()),
            _ => GotoResult::Multiple(links),
        }
    }

    /// Count the number of unique target files across all links.
    pub fn file_count(&self) -> usize {
        let mut uris: Vec<&str> = match self {
            Self::Single(l) => vec![l.target_uri.as_str()],
            Self::Multiple(v) => v.iter().map(|l| l.target_uri.as_str()).collect(),
            Self::None => return 0,
        };
        uris.sort_unstable();
        uris.dedup();
        uris.len()
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.uri, self.line, self.column)
    }
}

impl fmt::Display for GotoResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.len();
        let files = self.file_count();
        write!(f, "{n} results in {files} files")
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

// ---------------------------------------------------------------------------
// GotoBreadcrumb — breadcrumb navigation trail
// ---------------------------------------------------------------------------

/// A breadcrumb entry representing a visited symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbEntry {
    pub label: String,
    pub location: Location,
}

/// Breadcrumb trail showing the navigation path through symbols.
#[derive(Debug, Clone, Default)]
pub struct GotoBreadcrumb {
    entries: Vec<BreadcrumbEntry>,
    max_depth: usize,
}

impl GotoBreadcrumb {
    pub fn new(max_depth: usize) -> Self {
        Self { entries: Vec::new(), max_depth }
    }

    /// Push a new breadcrumb entry.
    pub fn push(&mut self, label: impl Into<String>, location: Location) {
        if self.entries.len() >= self.max_depth {
            self.entries.remove(0);
        }
        self.entries.push(BreadcrumbEntry { label: label.into(), location });
    }

    /// Pop and return the last breadcrumb.
    pub fn pop(&mut self) -> Option<BreadcrumbEntry> {
        self.entries.pop()
    }

    /// Return the current trail as a path string (e.g. "main > parse > token").
    pub fn trail_string(&self, separator: &str) -> String {
        self.entries.iter().map(|e| e.label.as_str()).collect::<Vec<_>>().join(separator)
    }

    /// Return the current depth.
    pub fn depth(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return all entries.
    pub fn entries(&self) -> &[BreadcrumbEntry] {
        &self.entries
    }

    /// Clear the entire trail.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// GotoBookmarkManager — bookmark goto locations
// ---------------------------------------------------------------------------

/// A saved bookmark for a goto location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GotoBookmark {
    pub name: String,
    pub location: Location,
}

/// Manages a collection of named bookmarks.
#[derive(Debug, Clone, Default)]
pub struct GotoBookmarkManager {
    bookmarks: Vec<GotoBookmark>,
}

impl GotoBookmarkManager {
    pub fn new() -> Self {
        Self { bookmarks: Vec::new() }
    }

    /// Add a bookmark. If a bookmark with the same name exists, update it.
    pub fn add(&mut self, name: impl Into<String>, location: Location) {
        let name = name.into();
        if let Some(existing) = self.bookmarks.iter_mut().find(|b| b.name == name) {
            existing.location = location;
        } else {
            self.bookmarks.push(GotoBookmark { name, location });
        }
    }

    /// Remove a bookmark by name. Returns true if found.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.bookmarks.len();
        self.bookmarks.retain(|b| b.name != name);
        self.bookmarks.len() < before
    }

    /// Look up a bookmark by name.
    pub fn get(&self, name: &str) -> Option<&GotoBookmark> {
        self.bookmarks.iter().find(|b| b.name == name)
    }

    /// Return all bookmarks for a given file URI.
    pub fn bookmarks_in_file(&self, uri: &str) -> Vec<&GotoBookmark> {
        self.bookmarks.iter().filter(|b| b.location.uri == uri).collect()
    }

    /// Return the total number of bookmarks.
    pub fn count(&self) -> usize {
        self.bookmarks.len()
    }

    /// List all bookmark names sorted alphabetically.
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.bookmarks.iter().map(|b| b.name.as_str()).collect();
        names.sort_unstable();
        names
    }
}

// ---------------------------------------------------------------------------
// GotoPredictor — predict likely goto targets based on history
// ---------------------------------------------------------------------------

/// Tracks goto target frequency and predicts likely targets.
#[derive(Debug, Clone, Default)]
pub struct GotoPredictor {
    target_counts: std::collections::HashMap<String, u32>,
}

impl GotoPredictor {
    pub fn new() -> Self {
        Self { target_counts: std::collections::HashMap::new() }
    }

    /// Record that the user navigated to a target URI.
    pub fn record_navigation(&mut self, target_uri: &str) {
        *self.target_counts.entry(target_uri.to_string()).or_insert(0) += 1;
    }

    /// Return the top-N most frequently visited targets.
    pub fn predict(&self, limit: usize) -> Vec<(&str, u32)> {
        let mut entries: Vec<(&str, u32)> = self.target_counts.iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(limit);
        entries
    }

    /// Return the visit count for a specific URI.
    pub fn visit_count(&self, uri: &str) -> u32 {
        self.target_counts.get(uri).copied().unwrap_or(0)
    }

    /// Total unique targets tracked.
    pub fn unique_targets(&self) -> usize {
        self.target_counts.len()
    }
}

// ---------------------------------------------------------------------------
// GotoHistory frequency analysis
// ---------------------------------------------------------------------------

impl GotoHistory {
    /// Count how many times each file appears in the history.
    pub fn file_frequency(&self) -> Vec<(&str, usize)> {
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for entry in &self.entries {
            *counts.entry(entry.uri.as_str()).or_insert(0) += 1;
        }
        let mut freq: Vec<(&str, usize)> = counts.into_iter().collect();
        freq.sort_by(|a, b| b.1.cmp(&a.1));
        freq
    }

    /// Return the total number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return all unique file URIs visited.
    pub fn unique_files(&self) -> Vec<&str> {
        let mut uris: Vec<&str> = self.entries.iter().map(|e| e.uri.as_str()).collect();
        uris.sort_unstable();
        uris.dedup();
        uris
    }
}

// ---------------------------------------------------------------------------
// LocationFilter — filter and search goto locations
// ---------------------------------------------------------------------------

/// Filters and scores goto locations for quick-open / symbol-search UI.
#[derive(Debug, Clone)]
pub struct LocationFilter {
    locations: Vec<Location>,
}

impl LocationFilter {
    pub fn new(locations: Vec<Location>) -> Self {
        Self { locations }
    }

    /// Filter locations to those in a specific file.
    pub fn in_file(&self, uri: &str) -> Vec<&Location> {
        self.locations.iter().filter(|l| l.uri == uri).collect()
    }

    /// Filter locations within a line range (inclusive).
    pub fn in_line_range(&self, start: u32, end: u32) -> Vec<&Location> {
        self.locations.iter().filter(|l| l.line >= start && l.line <= end).collect()
    }

    /// Return locations sorted by file URI then line number.
    pub fn sorted(&self) -> Vec<&Location> {
        let mut sorted: Vec<&Location> = self.locations.iter().collect();
        sorted.sort_by(|a, b| (&a.uri, a.line, a.column).cmp(&(&b.uri, b.line, b.column)));
        sorted
    }

    /// Group locations by file URI.
    pub fn group_by_file(&self) -> std::collections::HashMap<&str, Vec<&Location>> {
        let mut groups: std::collections::HashMap<&str, Vec<&Location>> =
            std::collections::HashMap::new();
        for loc in &self.locations {
            groups.entry(loc.uri.as_str()).or_default().push(loc);
        }
        groups
    }

    /// Return the total number of locations.
    pub fn count(&self) -> usize {
        self.locations.len()
    }

    /// Return unique file URIs.
    pub fn unique_files(&self) -> Vec<&str> {
        let mut uris: Vec<&str> = self.locations.iter().map(|l| l.uri.as_str()).collect();
        uris.sort_unstable();
        uris.dedup();
        uris
    }

    /// Return the closest location to a given line/column in a file.
    pub fn nearest(&self, uri: &str, line: u32, col: u32) -> Option<&Location> {
        self.locations
            .iter()
            .filter(|l| l.uri == uri)
            .min_by_key(|l| {
                let dl = (l.line as i64 - line as i64).unsigned_abs();
                let dc = (l.column as i64 - col as i64).unsigned_abs();
                dl * 10000 + dc
            })
    }
}

// ---------------------------------------------------------------------------
// GotoResultSet — aggregate results from multiple providers
// ---------------------------------------------------------------------------

/// Collects results from multiple goto providers and deduplicates them.
#[derive(Debug, Clone, Default)]
pub struct GotoResultSet {
    results: Vec<GotoResult>,
}

impl GotoResultSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a result from a provider.
    pub fn add(&mut self, result: GotoResult) {
        self.results.push(result);
    }

    /// Flatten all results into a single list of locations (converted from LocationLinks).
    pub fn all_locations(&self) -> Vec<Location> {
        let mut locs = Vec::new();
        for r in &self.results {
            match r {
                GotoResult::Single(link) => locs.push(link.to_location()),
                GotoResult::Multiple(v) => {
                    locs.extend(v.iter().map(|link| link.to_location()));
                }
                GotoResult::None => {}
            }
        }
        locs
    }

    /// Deduplicate locations (same uri, line, column).
    pub fn unique_locations(&self) -> Vec<Location> {
        let all = self.all_locations();
        let mut seen = std::collections::HashSet::new();
        let mut unique = Vec::new();
        for loc in all {
            let key = (loc.uri.clone(), loc.line, loc.column);
            if seen.insert(key) {
                unique.push(loc);
            }
        }
        unique
    }

    /// True if no provider returned any location.
    pub fn is_empty(&self) -> bool {
        self.all_locations().is_empty()
    }

    /// Total number of results added (not locations).
    pub fn provider_count(&self) -> usize {
        self.results.len()
    }
}

// ---------------------------------------------------------------------------
// BoundedGotoHistory – tracking navigation jumps with capacity
// ---------------------------------------------------------------------------

/// Tracks navigation jumps for back/forward movement with a bounded capacity.
///
/// Unlike [`GotoHistory`], this variant enforces a maximum number of entries
/// and uses separate back/forward stacks.
#[derive(Debug, Clone)]
pub struct BoundedGotoHistory {
    back_stack: Vec<Location>,
    forward_stack: Vec<Location>,
    max_entries: usize,
}

impl Default for BoundedGotoHistory {
    fn default() -> Self {
        Self {
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            max_entries: 100,
        }
    }
}

impl BoundedGotoHistory {
    /// Create a new history with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            ..Default::default()
        }
    }

    /// Record a navigation jump to a new location.
    pub fn push(&mut self, location: Location) {
        self.forward_stack.clear();
        self.back_stack.push(location);
        if self.back_stack.len() > self.max_entries {
            self.back_stack.remove(0);
        }
    }

    /// Navigate backwards. Returns the previous location if available.
    pub fn go_back(&mut self) -> Option<Location> {
        let loc = self.back_stack.pop()?;
        self.forward_stack.push(loc.clone());
        Some(loc)
    }

    /// Navigate forwards. Returns the next location if available.
    pub fn go_forward(&mut self) -> Option<Location> {
        let loc = self.forward_stack.pop()?;
        self.back_stack.push(loc.clone());
        Some(loc)
    }

    /// Whether there is a location to go back to.
    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    /// Whether there is a location to go forward to.
    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    /// Total number of entries across both stacks.
    pub fn total_entries(&self) -> usize {
        self.back_stack.len() + self.forward_stack.len()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.back_stack.clear();
        self.forward_stack.clear();
    }
}

impl fmt::Display for BoundedGotoHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BoundedGotoHistory(back={}, forward={})",
            self.back_stack.len(),
            self.forward_stack.len()
        )
    }
}

// ---------------------------------------------------------------------------
// GotoSymbolMatcher – fuzzy symbol matching with ranking
// ---------------------------------------------------------------------------

/// A scored symbol match result.
#[derive(Debug, Clone)]
pub struct SymbolMatch {
    /// The symbol name.
    pub name: String,
    /// The file where the symbol is defined.
    pub uri: String,
    /// Line number.
    pub line: u32,
    /// Match score (higher is better).
    pub score: i64,
}

/// Matches symbols using fuzzy scoring.
#[derive(Debug)]
pub struct GotoSymbolMatcher {
    symbols: Vec<(String, String, u32)>, // (name, uri, line)
}

impl GotoSymbolMatcher {
    /// Create a new matcher with registered symbols.
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
        }
    }

    /// Register a symbol for matching.
    pub fn register(&mut self, name: impl Into<String>, uri: impl Into<String>, line: u32) {
        self.symbols.push((name.into(), uri.into(), line));
    }

    /// Find symbols matching the query, sorted by score descending.
    pub fn find(&self, query: &str) -> Vec<SymbolMatch> {
        let query_lower = query.to_lowercase();
        let mut matches: Vec<SymbolMatch> = self
            .symbols
            .iter()
            .filter_map(|(name, uri, line)| {
                let score = Self::fuzzy_score(&query_lower, &name.to_lowercase())?;
                Some(SymbolMatch {
                    name: name.clone(),
                    uri: uri.clone(),
                    line: *line,
                    score,
                })
            })
            .collect();
        matches.sort_by(|a, b| b.score.cmp(&a.score));
        matches
    }

    /// Simple fuzzy scoring: consecutive matches score higher.
    fn fuzzy_score(query: &str, target: &str) -> Option<i64> {
        let mut score: i64 = 0;
        let mut target_iter = target.chars().peekable();
        let mut consecutive = 0i64;
        for qc in query.chars() {
            let mut found = false;
            while let Some(&tc) = target_iter.peek() {
                target_iter.next();
                if tc == qc {
                    consecutive += 1;
                    score += consecutive * 2;
                    found = true;
                    break;
                } else {
                    consecutive = 0;
                }
            }
            if !found {
                return None;
            }
        }
        // Bonus for exact prefix match
        if target.starts_with(query) {
            score += 10;
        }
        Some(score)
    }

    /// Number of registered symbols.
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }
}

impl Default for GotoSymbolMatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// GotoLineColumn – parse "line:column" strings
// ---------------------------------------------------------------------------

/// Error returned when parsing a goto line/column string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GotoParseError {
    /// Input string is empty.
    Empty,
    /// Line number is not a valid integer.
    InvalidLine(String),
    /// Column number is not a valid integer.
    InvalidColumn(String),
    /// Line number is zero (lines are 1-based).
    ZeroLine,
}

impl fmt::Display for GotoParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GotoParseError::Empty => write!(f, "empty input"),
            GotoParseError::InvalidLine(s) => write!(f, "invalid line: {}", s),
            GotoParseError::InvalidColumn(s) => write!(f, "invalid column: {}", s),
            GotoParseError::ZeroLine => write!(f, "line must be >= 1"),
        }
    }
}

/// Parsed goto target with line and optional column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GotoLineColumn {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number (default 1).
    pub column: u32,
}

impl GotoLineColumn {
    /// Parse a string in the format `"line"` or `"line:column"`.
    pub fn parse(input: &str) -> Result<Self, GotoParseError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(GotoParseError::Empty);
        }
        let (line_str, col_str) = match input.split_once(':') {
            Some((l, c)) => (l.trim(), Some(c.trim())),
            None => (input, None),
        };
        let line: u32 = line_str
            .parse()
            .map_err(|_| GotoParseError::InvalidLine(line_str.to_string()))?;
        if line == 0 {
            return Err(GotoParseError::ZeroLine);
        }
        let column = match col_str {
            Some(c) if !c.is_empty() => c
                .parse()
                .map_err(|_| GotoParseError::InvalidColumn(c.to_string()))?,
            _ => 1,
        };
        Ok(Self { line, column })
    }

    /// Convert to a [`Location`] in the given file.
    pub fn to_location(&self, uri: impl Into<String>) -> Location {
        Location::new(uri, self.line, self.column)
    }
}

impl fmt::Display for GotoLineColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

// ---------------------------------------------------------------------------
// GotoDefinitionFallback – chain of definition providers
// ---------------------------------------------------------------------------

/// Chains multiple definition resolution strategies.
///
/// Tries each strategy in order until one returns a result.
pub struct GotoDefinitionFallback {
    strategies: Vec<(String, Box<dyn Fn(&str, u32, u32) -> GotoResult + Send + Sync>)>,
}

impl GotoDefinitionFallback {
    /// Create an empty fallback chain.
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
        }
    }

    /// Add a named fallback strategy.
    pub fn add_strategy(
        &mut self,
        name: impl Into<String>,
        strategy: Box<dyn Fn(&str, u32, u32) -> GotoResult + Send + Sync>,
    ) {
        self.strategies.push((name.into(), strategy));
    }

    /// Resolve a definition by trying each strategy in order.
    ///
    /// Returns the first non-empty result, along with the strategy name.
    pub fn resolve(&self, uri: &str, line: u32, column: u32) -> Option<(String, GotoResult)> {
        for (name, strategy) in &self.strategies {
            let result = strategy(uri, line, column);
            if !result.is_empty() {
                return Some((name.clone(), result));
            }
        }
        None
    }

    /// Number of registered strategies.
    pub fn strategy_count(&self) -> usize {
        self.strategies.len()
    }
}

impl Default for GotoDefinitionFallback {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for GotoDefinitionFallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GotoDefinitionFallback({} strategies)", self.strategies.len())
    }
}


// ── Goto Symbol Ranker ──

/// Scoring factors for goto symbol ranking.
#[derive(Debug, Clone)]
pub struct SymbolScoreFactors {
    pub name_match_score: f64,
    pub kind_boost: f64,
    pub proximity_score: f64,
    pub recency_score: f64,
    pub frequency_score: f64,
}

impl Default for SymbolScoreFactors {
    fn default() -> Self {
        Self {
            name_match_score: 0.0,
            kind_boost: 1.0,
            proximity_score: 0.0,
            recency_score: 0.0,
            frequency_score: 0.0,
        }
    }
}

impl SymbolScoreFactors {
    /// Compute a weighted total score.
    pub fn total(&self) -> f64 {
        self.name_match_score * 3.0
            + self.kind_boost * 1.0
            + self.proximity_score * 2.0
            + self.recency_score * 1.5
            + self.frequency_score * 1.0
    }
}

/// A ranked goto result with associated score.
#[derive(Debug, Clone)]
pub struct RankedGotoResult {
    pub location: Location,
    pub symbol_name: String,
    pub factors: SymbolScoreFactors,
}

impl RankedGotoResult {
    pub fn new(location: Location, symbol_name: impl Into<String>) -> Self {
        Self {
            location,
            symbol_name: symbol_name.into(),
            factors: SymbolScoreFactors::default(),
        }
    }

    pub fn score(&self) -> f64 {
        self.factors.total()
    }
}

/// Ranks goto results by relevance using multiple scoring criteria.
pub struct GotoSymbolRanker {
    results: Vec<RankedGotoResult>,
    current_uri: Option<String>,
    current_line: Option<u32>,
}

impl GotoSymbolRanker {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
            current_uri: None,
            current_line: None,
        }
    }

    /// Set the current cursor position for proximity scoring.
    pub fn set_cursor(&mut self, uri: impl Into<String>, line: u32) {
        self.current_uri = Some(uri.into());
        self.current_line = Some(line);
    }

    /// Add a result with auto-calculated proximity score.
    pub fn add_result(&mut self, mut result: RankedGotoResult) {
        if let (Some(cur_uri), Some(cur_line)) = (&self.current_uri, self.current_line) {
            if result.location.uri == *cur_uri {
                let dist = (result.location.line as f64 - cur_line as f64).abs();
                result.factors.proximity_score = 1.0 / (1.0 + dist * 0.01);
            }
        }
        self.results.push(result);
    }

    /// Score a symbol name against a query using prefix and substring matching.
    pub fn score_name_match(symbol: &str, query: &str) -> f64 {
        if symbol.is_empty() || query.is_empty() {
            return 0.0;
        }
        let sym_lower = symbol.to_lowercase();
        let query_lower = query.to_lowercase();
        if sym_lower == query_lower {
            return 1.0;
        }
        if sym_lower.starts_with(&query_lower) {
            return 0.8;
        }
        if sym_lower.contains(&query_lower) {
            return 0.5;
        }
        // Check camelCase initials match
        let initials: String = symbol
            .chars()
            .filter(|c| c.is_uppercase() || *c == '_')
            .map(|c| c.to_lowercase().next().unwrap_or(c))
            .collect();
        if initials.contains(&query_lower) {
            return 0.3;
        }
        0.0
    }

    /// Return results sorted by score (highest first).
    pub fn ranked_results(&self) -> Vec<&RankedGotoResult> {
        let mut sorted: Vec<&RankedGotoResult> = self.results.iter().collect();
        sorted.sort_by(|a, b| b.score().partial_cmp(&a.score()).unwrap_or(std::cmp::Ordering::Equal));
        sorted
    }

    /// Return the top N results.
    pub fn top_n(&self, n: usize) -> Vec<&RankedGotoResult> {
        self.ranked_results().into_iter().take(n).collect()
    }

    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    pub fn clear(&mut self) {
        self.results.clear();
    }
}

// ── Goto Definition Chain Resolver ──

/// A node in a definition chain (e.g. re-exports).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionChainNode {
    pub location: Location,
    pub symbol_name: String,
    pub is_reexport: bool,
}

impl DefinitionChainNode {
    pub fn new(location: Location, symbol_name: impl Into<String>, is_reexport: bool) -> Self {
        Self {
            location,
            symbol_name: symbol_name.into(),
            is_reexport,
        }
    }
}

/// Resolves chains of definitions (e.g. following re-exports to the original).
pub struct GotoDefinitionChainResolver {
    chains: Vec<Vec<DefinitionChainNode>>,
    max_chain_depth: usize,
}

impl GotoDefinitionChainResolver {
    pub fn new(max_depth: usize) -> Self {
        Self {
            chains: Vec::new(),
            max_chain_depth: max_depth,
        }
    }

    /// Start a new chain with the initial definition.
    pub fn start_chain(&mut self, node: DefinitionChainNode) -> usize {
        let idx = self.chains.len();
        self.chains.push(vec![node]);
        idx
    }

    /// Extend a chain with a subsequent link. Returns false if max depth reached.
    pub fn extend_chain(&mut self, chain_index: usize, node: DefinitionChainNode) -> bool {
        if chain_index >= self.chains.len() {
            return false;
        }
        if self.chains[chain_index].len() >= self.max_chain_depth {
            return false;
        }
        self.chains[chain_index].push(node);
        true
    }

    /// Get the final (deepest) definition of a chain.
    pub fn resolve_final(&self, chain_index: usize) -> Option<&DefinitionChainNode> {
        self.chains.get(chain_index).and_then(|c| c.last())
    }

    /// Get the original (first) definition of a chain.
    pub fn resolve_origin(&self, chain_index: usize) -> Option<&DefinitionChainNode> {
        self.chains.get(chain_index).and_then(|c| c.first())
    }

    /// Get the full chain for a given index.
    pub fn chain(&self, chain_index: usize) -> Option<&[DefinitionChainNode]> {
        self.chains.get(chain_index).map(|c| c.as_slice())
    }

    pub fn chain_count(&self) -> usize {
        self.chains.len()
    }

    /// Get the depth of a chain.
    pub fn chain_depth(&self, chain_index: usize) -> usize {
        self.chains.get(chain_index).map_or(0, |c| c.len())
    }

    /// Count how many re-export hops exist in a chain.
    pub fn reexport_count(&self, chain_index: usize) -> usize {
        self.chains
            .get(chain_index)
            .map_or(0, |c| c.iter().filter(|n| n.is_reexport).count())
    }

    /// Check if a chain forms a cycle (first and last point to same URI+line).
    pub fn is_cyclic(&self, chain_index: usize) -> bool {
        let Some(chain) = self.chains.get(chain_index) else {
            return false;
        };
        if chain.len() < 2 {
            return false;
        }
        let first = &chain[0].location;
        let last = &chain[chain.len() - 1].location;
        first.uri == last.uri && first.line == last.line && first.column == last.column
    }
}



// ---------------------------------------------------------------------------
// vsedit-goto: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GotoXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl GotoXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for GotoXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct GotoXRegistry {
    entries: Vec<GotoXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl GotoXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: GotoXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&GotoXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut GotoXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<GotoXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&GotoXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&GotoXConfig> {
        let mut sorted: Vec<&GotoXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&GotoXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> GotoXIterator<'_> {
        GotoXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct GotoXIterator<'a> {
    inner: std::slice::Iter<'a, GotoXConfig>,
}

impl<'a> Iterator for GotoXIterator<'a> {
    type Item = &'a GotoXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct GotoXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl GotoXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct GotoXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl GotoXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &GotoXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &GotoXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &GotoXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for GotoXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct GotoXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl GotoXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &GotoXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &GotoXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for GotoXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for goto
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaGotoRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaGotoRingBuf {
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
pub struct XaGotoCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaGotoCounter {
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

impl Default for XaGotoCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 87
// ---------------------------------------------------------------------------

/// Generic object pool `Xc87Pool<T>`.
pub struct Xc87Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc87Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc87PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc87Pool<T> {
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
    pub fn stats(&self) -> Xc87PoolStats {
        Xc87PoolStats {
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

impl<T> Default for Xc87Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc87Scheduler`.
pub struct Xc87Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc87Scheduler {
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

impl Default for Xc87Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_87 hash for the given byte slice.
pub fn xc_87_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_87 convention.
pub fn xc_87_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_110 deepening: state machine + event bus ---

/// States for the Xd110 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd110State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd110State {
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
pub struct Xd110Transition {
    pub from: Xd110State,
    pub to: Xd110State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd110StateMachine {
    current: Xd110State,
    history: Vec<Xd110Transition>,
    step_counter: usize,
}

impl Xd110StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd110State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd110State {
        self.current
    }

    pub fn history(&self) -> &[Xd110Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd110State) -> Result<Xd110State, String> {
        let allowed = match (self.current, target) {
            (Xd110State::Idle, Xd110State::Running) => true,
            (Xd110State::Running, Xd110State::Paused) => true,
            (Xd110State::Running, Xd110State::Done) => true,
            (Xd110State::Paused, Xd110State::Running) => true,
            (Xd110State::Paused, Xd110State::Done) => true,
            (Xd110State::Done, Xd110State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_110: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd110Transition {
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
            "Xd110SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd110State> {
        let prefix = "Xd110SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd110State::Idle),
            "Running" => Some(Xd110State::Running),
            "Paused" => Some(Xd110State::Paused),
            "Done" => Some(Xd110State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd110State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd110 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd110Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd110Event {
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

type Xd110HandlerFn = Box<dyn Fn(&Xd110Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd110EventBus {
    handlers: Vec<(usize, Option<String>, Xd110HandlerFn)>,
    next_id: usize,
    published: Vec<Xd110Event>,
}

impl Xd110EventBus {
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
        F: Fn(&Xd110Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd110Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd110Event) {
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

    pub fn published_events(&self) -> &[Xd110Event] {
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
// xg_35: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg35Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg35Graph {
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

impl Default for Xg35Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_35: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg35Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg35Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg35Heap<T>) {
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

impl<T: Ord> Default for Xg35Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 86).
pub struct Xh86SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh86SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 128 as u64,
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

/// A compact bit set supporting boolean operations (variant 86).
pub struct Xh86BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh86BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 86).
pub struct Xi86Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi86Deque<T> {
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
pub struct Xi86Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi86Interval {
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

/// A simple interval tree (variant 86).
pub struct Xi86IntervalTree {
    xi_intervals: Vec<Xi86Interval>,
}

impl Xi86IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi86Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi86Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi86Interval) -> Vec<&Xi86Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi86Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi86Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi86Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi86Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi86Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi86Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 86) ---

/// Disjoint set / union-find for crate 86.
pub struct Xj86UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj86UnionFind {
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

const XJ86_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 86.
pub struct Xj86BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj86BTreeNode<K, V>>>,
    len: usize,
}

struct Xj86BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj86BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj86BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ86_BTREE_ORDER - 1
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
        let mid = XJ86_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj86BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj86BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj86BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj86BTreeNode::xj_new_leaf();
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


// --- xk_86 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk86SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk86SegmentTree {
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
pub struct Xk86DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk86DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_86).
#[derive(Debug, Clone)]
pub struct Xl86Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl86Rope {
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

/// Suffix array for efficient string searching (xl_86).
#[derive(Debug, Clone)]
pub struct Xl86SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl86SuffixArray {
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
pub struct Xm86MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm86MatrixSparse {
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
pub struct Xm86Tokenizer {
    text: String,
}

impl Xm86Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 86.
pub struct Xn86Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn86Fenwick {
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

// ----- AVL tree map — crate 86 -----

#[derive(Debug, Clone)]
struct Xn86AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn86AvlNode<K, V>>>,
    right: Option<Box<Xn86AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 86.
#[derive(Debug, Clone)]
pub struct Xn86AVL<K, V> {
    root: Option<Box<Xn86AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn86AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn86AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn86AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn86AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn86AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn86AvlNode<K, V>>) -> Box<Xn86AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn86AvlNode<K, V>>) -> Box<Xn86AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn86AvlNode<K, V>>) -> Box<Xn86AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn86AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn86AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn86AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn86AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn86AvlNode<K, V>>) -> &Xn86AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn86AvlNode<K, V>>) -> (Box<Xn86AvlNode<K, V>>, Option<Box<Xn86AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn86AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn86AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn86AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn86AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn86AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn86AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn86AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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


// ---------------------------------------------------------------------------
// Xo86RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo86Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo86RBNode<K, V> {
    key: K,
    value: V,
    color: Xo86Color,
    left: Option<Box<Xo86RBNode<K, V>>>,
    right: Option<Box<Xo86RBNode<K, V>>>,
}

/// A red-black tree map for crate 86.
#[derive(Debug, Clone)]
pub struct Xo86RedBlack<K, V> {
    root: Option<Box<Xo86RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo86RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo86Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo86RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo86RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo86RBNode {
                    key, value, color: Xo86Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo86RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo86Color::Red)
    }

    fn xo_balance(mut h: Box<Xo86RBNode<K, V>>) -> Box<Xo86RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo86Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo86RBNode<K, V>>) -> Box<Xo86RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo86Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo86RBNode<K, V>>) -> Box<Xo86RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo86Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo86RBNode<K, V>>) {
        h.color = Xo86Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo86Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo86Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo86Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo86RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo86RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo86RBNode<K, V>) -> (K, V, Option<Box<Xo86RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo86RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo86Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo86RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo86ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 86.
#[derive(Debug, Clone)]
pub struct Xo86ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo86ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo86#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo86#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 86).
#[derive(Debug)]
pub struct Xp86SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp86Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp86Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp86Node<K, V>>>,
    xp_right: Option<Box<Xp86Node<K, V>>>,
}

impl<K: Ord, V> Xp86Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp86SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp86SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp86Node<K, V>>>, key: &K) -> Option<Box<Xp86Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp86Node<K, V>>) -> Box<Xp86Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp86Node<K, V>>) -> Box<Xp86Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp86Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp86Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp86Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq86Treap ---------------

use std::cmp::Ordering as Xq86Ord;

struct Xq86TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq86TreapNode<K, V>>>,
    right: Option<Box<Xq86TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq86Treap<K, V> {
    root: Option<Box<Xq86TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq86TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_86_size<K, V>(node: &Option<Box<Xq86TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_86_update_size<K, V>(node: &mut Xq86TreapNode<K, V>) {
    node.size = 1 + xq_86_size(&node.left) + xq_86_size(&node.right);
}

fn xq_86_rotate_right<K, V>(mut node: Box<Xq86TreapNode<K, V>>) -> Box<Xq86TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_86_update_size(&mut node);
    left.right = Some(node);
    xq_86_update_size(&mut left);
    left
}

fn xq_86_rotate_left<K, V>(mut node: Box<Xq86TreapNode<K, V>>) -> Box<Xq86TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_86_update_size(&mut node);
    right.left = Some(node);
    xq_86_update_size(&mut right);
    right
}

fn xq_86_insert_node<K: Ord, V>(
    node: Option<Box<Xq86TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq86TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq86TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq86Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq86Ord::Less => {
                let (new_left, old) = xq_86_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_86_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_86_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq86Ord::Greater => {
                let (new_right, old) = xq_86_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_86_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_86_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_86_remove_node<K: Ord, V>(
    node: Option<Box<Xq86TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq86TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq86Ord::Less => {
                let (new_left, old) = xq_86_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_86_update_size(&mut n);
                (Some(n), old)
            }
            Xq86Ord::Greater => {
                let (new_right, old) = xq_86_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_86_update_size(&mut n);
                (Some(n), old)
            }
            Xq86Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_86_rotate_right(n);
                    let (new_right, old) = xq_86_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_86_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_86_rotate_left(n);
                    let (new_left, old) = xq_86_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_86_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_86_find_min<K, V>(node: &Option<Box<Xq86TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_86_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_86_find_max<K, V>(node: &Option<Box<Xq86TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_86_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_86_rank<K: Ord, V>(node: &Option<Box<Xq86TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq86Ord::Less => xq_86_rank(&n.left, key),
            Xq86Ord::Equal => xq_86_size(&n.left),
            Xq86Ord::Greater => 1 + xq_86_size(&n.left) + xq_86_rank(&n.right, key),
        },
    }
}

fn xq_86_kth<K, V>(node: &Option<Box<Xq86TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_86_size(&n.left);
        if k < left_size {
            xq_86_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_86_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_86_in_order<K: Clone, V>(node: &Option<Box<Xq86TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_86_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_86_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq86Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 86 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_86_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq86Ord::Equal => return Some(&n.value),
                Xq86Ord::Less => cur = &n.left,
                Xq86Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_86_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_86_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_86_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_86_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_86_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_86_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_86_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq86VEBTree ---------------

pub struct Xq86VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq86VEBTree>>,
    clusters: Vec<Option<Box<Xq86VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq86VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq86VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq86VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr86KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr86KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr86BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr86KDNode {
    xr_point: Xr86KDPoint,
    xr_left: Option<Box<Xr86KDNode>>,
    xr_right: Option<Box<Xr86KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr86KDTree {
    xr_root: Option<Box<Xr86KDNode>>,
    xr_size: usize,
}

impl Xr86KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr86KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr86KDNode>>,
        point: Xr86KDPoint,
        depth: usize,
    ) -> Box<Xr86KDNode> {
        match node {
            None => Box::new(Xr86KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr86KDPoint) -> Option<Xr86KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr86KDNode>,
        query: &Xr86KDPoint,
        depth: usize,
        best: &mut Xr86KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr86KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr86KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr86KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr86KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr86KDNode>>, pts: &mut Vec<Xr86KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr86KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr86BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr86BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs86PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs86PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs86PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs86PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs86ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs86ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs86ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs86RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs86RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs86RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs86CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs86CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs86CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}

/// Auxiliary statistics tracker for xs_86 data structures.
#[derive(Debug, Clone)]
pub struct Xs86StatsTracker {
    xs_samples: Vec<f64>,
    xs_sorted: bool,
}

impl Xs86StatsTracker {
    /// Create a new stats tracker.
    pub fn xs_new() -> Self {
        Xs86StatsTracker {
            xs_samples: Vec::new(),
            xs_sorted: true,
        }
    }

    /// Add a sample value.
    pub fn xs_add(&mut self, value: f64) {
        self.xs_samples.push(value);
        self.xs_sorted = false;
    }

    /// Return the number of samples.
    pub fn xs_count(&self) -> usize {
        self.xs_samples.len()
    }

    /// Return the mean of all samples.
    pub fn xs_mean(&self) -> f64 {
        if self.xs_samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.xs_samples.iter().sum();
        sum / self.xs_samples.len() as f64
    }

    /// Return the minimum value.
    pub fn xs_min(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::min)
    }

    /// Return the maximum value.
    pub fn xs_max(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::max)
    }

    /// Return the variance of all samples.
    pub fn xs_variance(&self) -> f64 {
        if self.xs_samples.len() < 2 {
            return 0.0;
        }
        let mean = self.xs_mean();
        let sum_sq: f64 = self.xs_samples.iter()
            .map(|x| (x - mean) * (x - mean))
            .sum();
        sum_sq / (self.xs_samples.len() - 1) as f64
    }

    /// Return the standard deviation.
    pub fn xs_std_dev(&self) -> f64 {
        self.xs_variance().sqrt()
    }

    /// Return the median value.
    pub fn xs_median(&mut self) -> Option<f64> {
        if self.xs_samples.is_empty() {
            return None;
        }
        if !self.xs_sorted {
            self.xs_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.xs_sorted = true;
        }
        let mid = self.xs_samples.len() / 2;
        if self.xs_samples.len() % 2 == 0 {
            Some((self.xs_samples[mid - 1] + self.xs_samples[mid]) / 2.0)
        } else {
            Some(self.xs_samples[mid])
        }
    }

    /// Check if the tracker is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_samples.is_empty()
    }

    /// Clear all samples.
    pub fn xs_clear(&mut self) {
        self.xs_samples.clear();
        self.xs_sorted = true;
    }

    /// Return the range (max - min).
    pub fn xs_range(&self) -> f64 {
        match (self.xs_min(), self.xs_max()) {
            (Some(min), Some(max)) => max - min,
            _ => 0.0,
        }
    }

    /// Return the sum of all samples.
    pub fn xs_sum(&self) -> f64 {
        self.xs_samples.iter().sum()
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
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

    // -----------------------------------------------------------------------
    // New functionality tests
    // -----------------------------------------------------------------------

    #[test]
    fn location_with_line_factory() {
        let loc = Location::with_line("file:///foo.rs", 42);
        assert_eq!(loc.uri, "file:///foo.rs");
        assert_eq!(loc.line, 42);
        assert_eq!(loc.column, 0);
        assert!(loc.end_line.is_none());
    }

    #[test]
    fn location_file_name() {
        assert_eq!(Location::new("file:///src/main.rs", 0, 0).file_name(), "main.rs");
        assert_eq!(Location::new("file:///a.rs", 0, 0).file_name(), "a.rs");
        assert_eq!(Location::new("no_slash", 0, 0).file_name(), "no_slash");
    }

    #[test]
    fn location_is_before() {
        let a = Location::new("file:///x.rs", 5, 3);
        let b = Location::new("file:///x.rs", 5, 10);
        let c = Location::new("file:///x.rs", 10, 0);
        let d = Location::new("file:///y.rs", 1, 0);

        assert!(a.is_before(&b));
        assert!(a.is_before(&c));
        assert!(!b.is_before(&a));
        assert!(!a.is_before(&a)); // same position is not "before"
        assert!(!a.is_before(&d)); // different file
    }

    #[test]
    fn location_display_trait() {
        let loc = Location::new("file:///lib.rs", 99, 12);
        assert_eq!(format!("{loc}"), "file:///lib.rs:99:12");
    }

    #[test]
    fn location_link_is_same_file() {
        let same = LocationLink::new("file:///a.rs", (0,0,0,0), (1,0,1,5))
            .with_origin_uri("file:///a.rs");
        assert!(same.is_same_file());

        let diff = LocationLink::new("file:///b.rs", (0,0,0,0), (1,0,1,5))
            .with_origin_uri("file:///a.rs");
        assert!(!diff.is_same_file());

        let no_origin = LocationLink::new("file:///a.rs", (0,0,0,0), (1,0,1,5));
        assert!(!no_origin.is_same_file());
    }

    #[test]
    fn goto_result_filter_by_file() {
        let r = GotoResult::Multiple(vec![
            LocationLink::new("file:///a.rs", (0,0,0,0), (1,0,1,5)),
            LocationLink::new("file:///b.rs", (0,0,0,0), (2,0,2,5)),
            LocationLink::new("file:///a.rs", (0,0,0,0), (3,0,3,5)),
        ]);
        let filtered = r.filter_by_file("file:///a.rs");
        assert_eq!(filtered.len(), 2);

        let empty = GotoResult::None.filter_by_file("file:///a.rs");
        assert!(empty.is_empty());
    }

    #[test]
    fn goto_result_file_count() {
        let r = GotoResult::Multiple(vec![
            LocationLink::new("file:///a.rs", (0,0,0,0), (0,0,0,0)),
            LocationLink::new("file:///b.rs", (0,0,0,0), (0,0,0,0)),
            LocationLink::new("file:///a.rs", (0,0,0,0), (1,0,1,5)),
        ]);
        assert_eq!(r.file_count(), 2);
        assert_eq!(GotoResult::None.file_count(), 0);

        let single = GotoResult::Single(LocationLink::new("x", (0,0,0,0), (0,0,0,0)));
        assert_eq!(single.file_count(), 1);
    }

    #[test]
    fn goto_result_display_trait() {
        let r = GotoResult::Multiple(vec![
            LocationLink::new("a", (0,0,0,0), (0,0,0,0)),
            LocationLink::new("b", (0,0,0,0), (0,0,0,0)),
            LocationLink::new("a", (0,0,0,0), (1,0,1,5)),
        ]);
        assert_eq!(format!("{r}"), "3 results in 2 files");
        assert_eq!(format!("{}", GotoResult::None), "0 results in 0 files");
    }

    // -- GotoBreadcrumb tests -----------------------------------------------

    #[test]
    fn breadcrumb_push_pop_and_trail() {
        let mut bc = GotoBreadcrumb::new(10);
        assert!(bc.is_empty());

        bc.push("main", Location::new("file:///main.rs", 1, 0));
        bc.push("parse", Location::new("file:///parser.rs", 20, 5));
        bc.push("token", Location::new("file:///lexer.rs", 50, 0));

        assert_eq!(bc.depth(), 3);
        assert_eq!(bc.trail_string(" > "), "main > parse > token");

        let popped = bc.pop().unwrap();
        assert_eq!(popped.label, "token");
        assert_eq!(bc.depth(), 2);
    }

    #[test]
    fn breadcrumb_max_depth() {
        let mut bc = GotoBreadcrumb::new(2);
        bc.push("a", Location::new("a.rs", 1, 0));
        bc.push("b", Location::new("b.rs", 1, 0));
        bc.push("c", Location::new("c.rs", 1, 0)); // should evict "a"
        assert_eq!(bc.depth(), 2);
        assert_eq!(bc.entries()[0].label, "b");
    }

    // -- GotoBookmarkManager tests ------------------------------------------

    #[test]
    fn bookmark_add_get_remove() {
        let mut mgr = GotoBookmarkManager::new();
        mgr.add("start", Location::new("main.rs", 1, 0));
        mgr.add("loop", Location::new("main.rs", 50, 0));
        mgr.add("helper", Location::new("util.rs", 10, 0));

        assert_eq!(mgr.count(), 3);
        assert!(mgr.get("start").is_some());
        assert_eq!(mgr.bookmarks_in_file("main.rs").len(), 2);

        assert!(mgr.remove("loop"));
        assert_eq!(mgr.count(), 2);
        assert!(!mgr.remove("nonexistent"));

        let names = mgr.names();
        assert_eq!(names, vec!["helper", "start"]);
    }

    #[test]
    fn bookmark_update_existing() {
        let mut mgr = GotoBookmarkManager::new();
        mgr.add("x", Location::new("a.rs", 1, 0));
        mgr.add("x", Location::new("b.rs", 99, 0)); // update
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.get("x").unwrap().location.uri, "b.rs");
    }

    // -- GotoPredictor tests ------------------------------------------------

    #[test]
    fn predictor_tracks_and_predicts() {
        let mut pred = GotoPredictor::new();
        pred.record_navigation("file:///a.rs");
        pred.record_navigation("file:///b.rs");
        pred.record_navigation("file:///a.rs");
        pred.record_navigation("file:///a.rs");
        pred.record_navigation("file:///c.rs");

        assert_eq!(pred.unique_targets(), 3);
        assert_eq!(pred.visit_count("file:///a.rs"), 3);

        let top = pred.predict(2);
        assert_eq!(top[0].0, "file:///a.rs");
        assert_eq!(top[0].1, 3);
    }

    // -- GotoHistory frequency analysis tests --------------------------------

    #[test]
    fn history_file_frequency() {
        let mut hist = GotoHistory::new();
        hist.push(Location::new("a.rs", 1, 0));
        hist.push(Location::new("b.rs", 5, 0));
        hist.push(Location::new("a.rs", 10, 0));

        let freq = hist.file_frequency();
        assert_eq!(freq[0].0, "a.rs");
        assert_eq!(freq[0].1, 2);
        assert_eq!(hist.len(), 3);

        let files = hist.unique_files();
        assert_eq!(files.len(), 2);
    }

    // -- LocationFilter --

    #[test]
    fn location_filter_in_file() {
        let locs = vec![
            Location::new("a.rs", 1, 0),
            Location::new("b.rs", 2, 0),
            Location::new("a.rs", 5, 0),
        ];
        let filter = LocationFilter::new(locs);
        assert_eq!(filter.in_file("a.rs").len(), 2);
        assert_eq!(filter.in_file("c.rs").len(), 0);
    }

    #[test]
    fn location_filter_in_line_range() {
        let locs = vec![
            Location::new("a.rs", 1, 0),
            Location::new("a.rs", 5, 0),
            Location::new("a.rs", 10, 0),
        ];
        let filter = LocationFilter::new(locs);
        let result = filter.in_line_range(3, 8);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].line, 5);
    }

    #[test]
    fn location_filter_sorted() {
        let locs = vec![
            Location::new("b.rs", 10, 0),
            Location::new("a.rs", 5, 0),
            Location::new("a.rs", 1, 0),
        ];
        let filter = LocationFilter::new(locs);
        let sorted = filter.sorted();
        assert_eq!(sorted[0].uri, "a.rs");
        assert_eq!(sorted[0].line, 1);
        assert_eq!(sorted[2].uri, "b.rs");
    }

    #[test]
    fn location_filter_group_by_file() {
        let locs = vec![
            Location::new("a.rs", 1, 0),
            Location::new("b.rs", 2, 0),
            Location::new("a.rs", 3, 0),
        ];
        let filter = LocationFilter::new(locs);
        let groups = filter.group_by_file();
        assert_eq!(groups.get("a.rs").unwrap().len(), 2);
        assert_eq!(groups.get("b.rs").unwrap().len(), 1);
    }

    #[test]
    fn location_filter_nearest() {
        let locs = vec![
            Location::new("a.rs", 1, 0),
            Location::new("a.rs", 10, 5),
            Location::new("a.rs", 20, 0),
        ];
        let filter = LocationFilter::new(locs);
        let nearest = filter.nearest("a.rs", 9, 4).unwrap();
        assert_eq!(nearest.line, 10);
    }

    // -- GotoResultSet --

    #[test]
    fn result_set_all_locations() {
        let mut set = GotoResultSet::new();
        set.add(GotoResult::Single(LocationLink::new("a.rs", (1,0,1,5), (1,0,1,5))));
        set.add(GotoResult::Multiple(vec![
            LocationLink::new("b.rs", (2,0,2,5), (2,0,2,5)),
            LocationLink::new("c.rs", (3,0,3,5), (3,0,3,5)),
        ]));
        set.add(GotoResult::None);
        assert_eq!(set.all_locations().len(), 3);
        assert!(!set.is_empty());
        assert_eq!(set.provider_count(), 3);
    }

    #[test]
    fn result_set_unique_locations() {
        let mut set = GotoResultSet::new();
        set.add(GotoResult::Single(LocationLink::new("a.rs", (1,0,1,5), (1,0,1,5))));
        set.add(GotoResult::Single(LocationLink::new("a.rs", (1,0,1,5), (1,0,1,5)))); // duplicate
        set.add(GotoResult::Single(LocationLink::new("b.rs", (2,0,2,5), (2,0,2,5))));
        let unique = set.unique_locations();
        assert_eq!(unique.len(), 2);
    }

    #[test]
    fn location_filter_unique_files() {
        let locs = vec![
            Location::new("a.rs", 1, 0),
            Location::new("b.rs", 2, 0),
            Location::new("a.rs", 3, 0),
        ];
        let filter = LocationFilter::new(locs);
        assert_eq!(filter.unique_files(), vec!["a.rs", "b.rs"]);
        assert_eq!(filter.count(), 3);
    }

    // -- BoundedGotoHistory tests --

    #[test]
    fn bounded_history_push_and_back() {
        let mut h = BoundedGotoHistory::new(10);
        h.push(Location::new("a.rs", 1, 0));
        h.push(Location::new("b.rs", 2, 0));
        assert!(h.can_go_back());
        let loc = h.go_back().unwrap();
        assert_eq!(loc.uri, "b.rs");
    }

    #[test]
    fn bounded_history_forward_after_back() {
        let mut h = BoundedGotoHistory::new(10);
        h.push(Location::new("a.rs", 1, 0));
        h.push(Location::new("b.rs", 2, 0));
        h.go_back();
        assert!(h.can_go_forward());
        let loc = h.go_forward().unwrap();
        assert_eq!(loc.uri, "b.rs");
    }

    #[test]
    fn bounded_history_push_clears_forward() {
        let mut h = BoundedGotoHistory::new(10);
        h.push(Location::new("a.rs", 1, 0));
        h.push(Location::new("b.rs", 2, 0));
        h.go_back();
        h.push(Location::new("c.rs", 3, 0));
        assert!(!h.can_go_forward());
    }

    #[test]
    fn bounded_history_capacity() {
        let mut h = BoundedGotoHistory::new(3);
        for i in 0..5 {
            h.push(Location::new(format!("{}.rs", i), i, 0));
        }
        assert_eq!(h.total_entries(), 3);
    }

    #[test]
    fn bounded_history_clear() {
        let mut h = BoundedGotoHistory::new(10);
        h.push(Location::new("a.rs", 1, 0));
        h.clear();
        assert!(!h.can_go_back());
        assert!(!h.can_go_forward());
    }

    // -- GotoSymbolMatcher tests --

    #[test]
    fn symbol_matcher_exact() {
        let mut m = GotoSymbolMatcher::new();
        m.register("main", "main.rs", 1);
        m.register("maintenance", "util.rs", 10);
        let results = m.find("main");
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "main");
    }

    #[test]
    fn symbol_matcher_fuzzy() {
        let mut m = GotoSymbolMatcher::new();
        m.register("handleClick", "ui.rs", 5);
        m.register("processData", "data.rs", 10);
        let results = m.find("hclk");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "handleClick");
    }

    #[test]
    fn symbol_matcher_no_match() {
        let mut m = GotoSymbolMatcher::new();
        m.register("foo", "a.rs", 1);
        assert!(m.find("xyz").is_empty());
    }

    // -- GotoLineColumn tests --

    #[test]
    fn parse_line_only() {
        let g = GotoLineColumn::parse("42").unwrap();
        assert_eq!(g.line, 42);
        assert_eq!(g.column, 1);
    }

    #[test]
    fn parse_line_and_column() {
        let g = GotoLineColumn::parse("10:5").unwrap();
        assert_eq!(g.line, 10);
        assert_eq!(g.column, 5);
    }

    #[test]
    fn parse_empty_error() {
        assert_eq!(GotoLineColumn::parse(""), Err(GotoParseError::Empty));
    }

    #[test]
    fn parse_zero_line_error() {
        assert_eq!(GotoLineColumn::parse("0"), Err(GotoParseError::ZeroLine));
    }

    #[test]
    fn parse_invalid_line_error() {
        assert!(matches!(GotoLineColumn::parse("abc"), Err(GotoParseError::InvalidLine(_))));
    }

    #[test]
    fn goto_line_column_display() {
        let g = GotoLineColumn { line: 10, column: 5 };
        assert_eq!(format!("{}", g), "10:5");
    }

    // -- GotoDefinitionFallback tests --

    #[test]
    fn fallback_empty_returns_none() {
        let fb = GotoDefinitionFallback::new();
        assert!(fb.resolve("a.rs", 1, 0).is_none());
    }

    #[test]
    fn fallback_first_match() {
        let mut fb = GotoDefinitionFallback::new();
        fb.add_strategy("lsp", Box::new(|_, _, _| GotoResult::None));
        fb.add_strategy("text", Box::new(|uri, line, col| {
            GotoResult::Single(LocationLink::new(
                "target.rs",
                (line, col, line, col),
                (line, col, line, col),
            ))
        }));
        let (name, _) = fb.resolve("a.rs", 1, 0).unwrap();
        assert_eq!(name, "text");
    }

    #[test]
    fn symbol_score_factors_total() {
        let f = SymbolScoreFactors {
            name_match_score: 1.0,
            kind_boost: 1.0,
            proximity_score: 0.5,
            recency_score: 0.5,
            frequency_score: 0.5,
        };
        let total = f.total();
        assert!((total - 6.25).abs() < 1e-9);
    }

    #[test]
    fn ranker_score_name_exact() {
        assert!((GotoSymbolRanker::score_name_match("Foo", "foo") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ranker_score_name_prefix() {
        let s = GotoSymbolRanker::score_name_match("FooBar", "foo");
        assert!((s - 0.8).abs() < 1e-9);
    }

    #[test]
    fn ranker_score_name_substring() {
        let s = GotoSymbolRanker::score_name_match("MyFooBar", "foo");
        assert!((s - 0.5).abs() < 1e-9);
    }

    #[test]
    fn ranker_score_name_no_match() {
        assert!((GotoSymbolRanker::score_name_match("Baz", "foo")).abs() < 1e-9);
    }

    #[test]
    fn ranker_add_and_rank() {
        let mut ranker = GotoSymbolRanker::new();
        let mut r1 = RankedGotoResult::new(Location::new("a.rs", 1, 0), "foo");
        r1.factors.name_match_score = 0.5;
        let mut r2 = RankedGotoResult::new(Location::new("b.rs", 1, 0), "bar");
        r2.factors.name_match_score = 1.0;
        ranker.add_result(r1);
        ranker.add_result(r2);
        let ranked = ranker.ranked_results();
        assert_eq!(ranked[0].symbol_name, "bar");
    }

    #[test]
    fn ranker_proximity_scoring() {
        let mut ranker = GotoSymbolRanker::new();
        ranker.set_cursor("a.rs", 10);
        let r1 = RankedGotoResult::new(Location::new("a.rs", 11, 0), "near");
        let r2 = RankedGotoResult::new(Location::new("a.rs", 500, 0), "far");
        ranker.add_result(r1);
        ranker.add_result(r2);
        let ranked = ranker.ranked_results();
        assert!(ranked[0].factors.proximity_score > ranked[1].factors.proximity_score);
    }

    #[test]
    fn ranker_top_n() {
        let mut ranker = GotoSymbolRanker::new();
        for i in 0..5 {
            let mut r = RankedGotoResult::new(Location::new("a.rs", i, 0), format!("s{}", i));
            r.factors.name_match_score = i as f64 * 0.1;
            ranker.add_result(r);
        }
        assert_eq!(ranker.top_n(2).len(), 2);
        assert_eq!(ranker.result_count(), 5);
    }

    #[test]
    fn definition_chain_basic() {
        let mut resolver = GotoDefinitionChainResolver::new(5);
        let n0 = DefinitionChainNode::new(Location::new("a.rs", 1, 0), "Foo", false);
        let idx = resolver.start_chain(n0);
        let n1 = DefinitionChainNode::new(Location::new("b.rs", 10, 0), "Foo", true);
        assert!(resolver.extend_chain(idx, n1));
        assert_eq!(resolver.chain_depth(idx), 2);
        assert_eq!(resolver.reexport_count(idx), 1);
        let final_node = resolver.resolve_final(idx).unwrap();
        assert_eq!(final_node.location.uri, "b.rs");
    }

    #[test]
    fn definition_chain_max_depth() {
        let mut resolver = GotoDefinitionChainResolver::new(2);
        let idx = resolver.start_chain(DefinitionChainNode::new(Location::new("a.rs", 1, 0), "X", false));
        assert!(resolver.extend_chain(idx, DefinitionChainNode::new(Location::new("b.rs", 1, 0), "X", true)));
        assert!(!resolver.extend_chain(idx, DefinitionChainNode::new(Location::new("c.rs", 1, 0), "X", true)));
    }

    #[test]
    fn definition_chain_cycle_detection() {
        let mut resolver = GotoDefinitionChainResolver::new(10);
        let idx = resolver.start_chain(DefinitionChainNode::new(Location::new("a.rs", 5, 3), "Y", false));
        resolver.extend_chain(idx, DefinitionChainNode::new(Location::new("b.rs", 1, 0), "Y", true));
        resolver.extend_chain(idx, DefinitionChainNode::new(Location::new("a.rs", 5, 3), "Y", true));
        assert!(resolver.is_cyclic(idx));
    }



    #[test]
    fn goto_x_config_new() {
        let c = GotoXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn goto_x_config_builder() {
        let c = GotoXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn goto_x_config_display() {
        let c = GotoXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn goto_x_registry_insert_get() {
        let mut reg = GotoXRegistry::new();
        reg.insert(GotoXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn goto_x_registry_duplicate() {
        let mut reg = GotoXRegistry::new();
        reg.insert(GotoXConfig::new("a")).unwrap();
        assert!(reg.insert(GotoXConfig::new("a")).is_err());
    }

    #[test]
    fn goto_x_registry_remove() {
        let mut reg = GotoXRegistry::new();
        reg.insert(GotoXConfig::new("a")).unwrap();
        reg.insert(GotoXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn goto_x_registry_active_entries() {
        let mut reg = GotoXRegistry::new();
        reg.insert(GotoXConfig::new("a")).unwrap();
        reg.insert(GotoXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn goto_x_registry_by_weight() {
        let mut reg = GotoXRegistry::new();
        reg.insert(GotoXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(GotoXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn goto_x_registry_tags() {
        let mut reg = GotoXRegistry::new();
        reg.insert(GotoXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(GotoXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn goto_x_registry_total_weight() {
        let mut reg = GotoXRegistry::new();
        reg.insert(GotoXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(GotoXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn goto_x_registry_iterator() {
        let mut reg = GotoXRegistry::new();
        reg.insert(GotoXConfig::new("a")).unwrap();
        reg.insert(GotoXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn goto_x_cache_put_get() {
        let mut cache = GotoXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn goto_x_cache_eviction() {
        let mut cache = GotoXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn goto_x_cache_lru_order() {
        let mut cache = GotoXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn goto_x_cache_most_least_recent() {
        let mut cache = GotoXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn goto_x_formatter_entry() {
        let e = GotoXConfig::new("k").with_value("v");
        let fmt = GotoXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn goto_x_formatter_summary() {
        let mut reg = GotoXRegistry::new();
        reg.insert(GotoXConfig::new("a").with_weight(5)).unwrap();
        let fmt = GotoXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn goto_x_validator_valid() {
        let v = GotoXValidator::new();
        let c = GotoXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn goto_x_validator_empty_key() {
        let v = GotoXValidator::new();
        let c = GotoXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn goto_x_validator_require_value() {
        let v = GotoXValidator::new().require_value(true);
        let c = GotoXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn goto_x_validator_allowed_tags() {
        let v = GotoXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = GotoXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn goto_x_validator_validate_all() {
        let v = GotoXValidator::new();
        let mut reg = GotoXRegistry::new();
        reg.insert(GotoXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for goto
    #[test]
    fn xa_goto_ring_new() {
        let rb = super::XaGotoRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_goto_ring_push_len() {
        let mut rb = super::XaGotoRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_goto_ring_wrap() {
        let mut rb = super::XaGotoRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_goto_ring_mean_empty() {
        let rb = super::XaGotoRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_goto_ring_mean_values() {
        let mut rb = super::XaGotoRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_goto_ring_min_max() {
        let mut rb = super::XaGotoRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_goto_ring_iter() {
        let mut rb = super::XaGotoRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_goto_counter_new() {
        let c = super::XaGotoCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_goto_counter_inc() {
        let mut c = super::XaGotoCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_goto_counter_inc_by() {
        let mut c = super::XaGotoCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_goto_counter_reset() {
        let mut c = super::XaGotoCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_goto_counter_clear() {
        let mut c = super::XaGotoCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_goto_counter_default() {
        let c = super::XaGotoCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 87 ----

    #[test]
    fn xc_87_pool_new_empty() {
        let pool: super::Xc87Pool<i32> = super::Xc87Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_87_pool_release_acquire() {
        let mut pool = super::Xc87Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_87_pool_acquire_empty() {
        let mut pool: super::Xc87Pool<i32> = super::Xc87Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_87_pool_full() {
        let mut pool = super::Xc87Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_87_pool_drain() {
        let mut pool = super::Xc87Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_87_pool_stats() {
        let mut pool = super::Xc87Pool::new(8);
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
    fn xc_87_pool_clear() {
        let mut pool = super::Xc87Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_87_pool_shrink() {
        let mut pool = super::Xc87Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_87_pool_default() {
        let pool: super::Xc87Pool<String> = super::Xc87Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_87_pool_extend() {
        let mut pool = super::Xc87Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_87_pool_retain() {
        let mut pool = super::Xc87Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_87_scheduler_round_robin() {
        let mut sched = super::Xc87Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_87_scheduler_empty() {
        let mut sched = super::Xc87Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_87_scheduler_reset() {
        let mut sched = super::Xc87Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_87_scheduler_add_remove() {
        let mut sched = super::Xc87Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_87_scheduler_targets() {
        let sched = super::Xc87Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_87_hash_empty() {
        assert_eq!(super::xc_87_hash(b""), 5381);
    }

    #[test]
    fn xc_87_hash_data() {
        let h = super::xc_87_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_87_hash(b"hello"), h);
    }

    #[test]
    fn xc_87_reverse_str() {
        assert_eq!(super::xc_87_reverse("abc"), "cba");
        assert_eq!(super::xc_87_reverse(""), "");
    }


    // --- xd_110 deepening tests ---

    #[test]
    fn xd_110_sm_initial_state() {
        let sm = Xd110StateMachine::new();
        assert_eq!(sm.current_state(), Xd110State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_110_sm_valid_idle_to_running() {
        let mut sm = Xd110StateMachine::new();
        assert!(sm.transition(Xd110State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd110State::Running);
    }

    #[test]
    fn xd_110_sm_valid_running_to_paused() {
        let mut sm = Xd110StateMachine::new();
        sm.transition(Xd110State::Running).unwrap();
        assert!(sm.transition(Xd110State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd110State::Paused);
    }

    #[test]
    fn xd_110_sm_valid_running_to_done() {
        let mut sm = Xd110StateMachine::new();
        sm.transition(Xd110State::Running).unwrap();
        assert!(sm.transition(Xd110State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd110State::Done);
    }

    #[test]
    fn xd_110_sm_valid_paused_to_running() {
        let mut sm = Xd110StateMachine::new();
        sm.transition(Xd110State::Running).unwrap();
        sm.transition(Xd110State::Paused).unwrap();
        assert!(sm.transition(Xd110State::Running).is_ok());
    }

    #[test]
    fn xd_110_sm_valid_done_to_idle() {
        let mut sm = Xd110StateMachine::new();
        sm.transition(Xd110State::Running).unwrap();
        sm.transition(Xd110State::Done).unwrap();
        assert!(sm.transition(Xd110State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd110State::Idle);
    }

    #[test]
    fn xd_110_sm_invalid_idle_to_done() {
        let mut sm = Xd110StateMachine::new();
        assert!(sm.transition(Xd110State::Done).is_err());
    }

    #[test]
    fn xd_110_sm_invalid_idle_to_paused() {
        let mut sm = Xd110StateMachine::new();
        assert!(sm.transition(Xd110State::Paused).is_err());
    }

    #[test]
    fn xd_110_sm_history_tracking() {
        let mut sm = Xd110StateMachine::new();
        sm.transition(Xd110State::Running).unwrap();
        sm.transition(Xd110State::Paused).unwrap();
        sm.transition(Xd110State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd110State::Idle);
        assert_eq!(sm.history()[0].to, Xd110State::Running);
        assert_eq!(sm.history()[1].from, Xd110State::Running);
        assert_eq!(sm.history()[2].to, Xd110State::Done);
    }

    #[test]
    fn xd_110_sm_serialize_deserialize() {
        let mut sm = Xd110StateMachine::new();
        sm.transition(Xd110State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd110StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd110State::Running));
    }

    #[test]
    fn xd_110_sm_deserialize_invalid() {
        assert_eq!(Xd110StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_110_sm_reset() {
        let mut sm = Xd110StateMachine::new();
        sm.transition(Xd110State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd110State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_110_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd110EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd110Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_110_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd110EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd110Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd110Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_110_bus_unsubscribe() {
        let mut bus = Xd110EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_110_event_kind_and_payload() {
        let e = Xd110Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd110Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_110_bus_clear_history() {
        let mut bus = Xd110EventBus::new();
        bus.publish(Xd110Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_110_sm_step_counter_increments() {
        let mut sm = Xd110StateMachine::new();
        sm.transition(Xd110State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd110State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_35 graph tests ------------------------------------------------

    #[test]
    fn xg_35_graph_empty() {
        let g = super::Xg35Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_35_graph_add_node() {
        let mut g = super::Xg35Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_35_graph_add_edge() {
        let mut g = super::Xg35Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_35_graph_neighbors() {
        let mut g = super::Xg35Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_35_graph_has_path() {
        let mut g = super::Xg35Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_35_graph_self_path() {
        let g = super::Xg35Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_35_graph_topo_sort() {
        let mut g = super::Xg35Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_35_graph_cycle_detect_false() {
        let mut g = super::Xg35Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_35_graph_cycle_detect_true() {
        let mut g = super::Xg35Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_35 heap tests -------------------------------------------------

    #[test]
    fn xg_35_heap_empty() {
        let h: super::Xg35Heap<i32> = super::Xg35Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_35_heap_push_pop() {
        let mut h = super::Xg35Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_35_heap_peek() {
        let mut h = super::Xg35Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_35_heap_drain_sorted() {
        let mut h = super::Xg35Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_35_heap_merge() {
        let mut a = super::Xg35Heap::new();
        let mut b = super::Xg35Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_35_heap_default() {
        let h: super::Xg35Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_35_graph_default() {
        let g: super::Xg35Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh86_skip_insert_contains() {
        let mut sl = super::Xh86SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh86_skip_remove() {
        let mut sl = super::Xh86SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh86_skip_len() {
        let mut sl = super::Xh86SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh86_skip_range_query() {
        let mut sl = super::Xh86SkipList::xh_new(4);
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
    fn xh86_skip_floor_ceiling() {
        let mut sl = super::Xh86SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh86_skip_rank() {
        let mut sl = super::Xh86SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh86_skip_empty() {
        let sl = super::Xh86SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh86_skip_duplicates() {
        let mut sl = super::Xh86SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh86_bitset_set_test() {
        let mut bs = super::Xh86BitSet::xh_new(256);
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
    fn xh86_bitset_clear_count() {
        let mut bs = super::Xh86BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh86_bitset_and_or_xor() {
        let mut a = super::Xh86BitSet::xh_new(128);
        let mut b = super::Xh86BitSet::xh_new(128);
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
    fn xh86_bitset_iter_ones() {
        let mut bs = super::Xh86BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh86_bitset_first_last() {
        let mut bs = super::Xh86BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh86_bitset_empty() {
        let bs = super::Xh86BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi86_deque_push_pop_back() {
        let mut dq = super::Xi86Deque::xi_new(4);
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
    fn xi86_deque_push_pop_front() {
        let mut dq = super::Xi86Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi86_deque_mixed_ops() {
        let mut dq = super::Xi86Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi86_deque_get_and_split() {
        let mut dq = super::Xi86Deque::xi_new(8);
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
    fn xi86_deque_rotate_left() {
        let mut dq = super::Xi86Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi86_deque_rotate_right() {
        let mut dq = super::Xi86Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi86_deque_grow() {
        let mut dq = super::Xi86Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi86_deque_empty() {
        let dq = super::Xi86Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi86_interval_tree_insert_query() {
        let mut tree = super::Xi86IntervalTree::xi_new();
        tree.xi_insert(super::Xi86Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi86Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi86Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi86_interval_tree_overlap() {
        let mut tree = super::Xi86IntervalTree::xi_new();
        tree.xi_insert(super::Xi86Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi86Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi86Interval::xi_new(12, 20));
        let q = super::Xi86Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi86_interval_tree_remove() {
        let mut tree = super::Xi86IntervalTree::xi_new();
        tree.xi_insert(super::Xi86Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi86Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi86_interval_tree_gaps() {
        let mut tree = super::Xi86IntervalTree::xi_new();
        tree.xi_insert(super::Xi86Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi86Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi86Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi86Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi86Interval::xi_new(8, 10));
    }

    #[test]
    fn xi86_interval_tree_merge() {
        let mut tree = super::Xi86IntervalTree::xi_new();
        tree.xi_insert(super::Xi86Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi86Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi86Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi86Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi86Interval::xi_new(10, 15));
    }

    #[test]
    fn xi86_interval_tree_all() {
        let mut tree = super::Xi86IntervalTree::xi_new();
        tree.xi_insert(super::Xi86Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi86Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi86_interval_tree_empty() {
        let tree = super::Xi86IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi86_interval_tree_contains_point() {
        let iv = super::Xi86Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 86) ---

    #[test]
    fn xj_86_uf_make_and_find() {
        let mut uf = super::Xj86UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_86_uf_union_connected() {
        let mut uf = super::Xj86UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_86_uf_component_count() {
        let mut uf = super::Xj86UnionFind::xj_new();
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
    fn xj_86_uf_component_size() {
        let mut uf = super::Xj86UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_86_uf_largest_component() {
        let mut uf = super::Xj86UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_86_uf_many_elements() {
        let mut uf = super::Xj86UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_86_uf_separate_components() {
        let mut uf = super::Xj86UnionFind::xj_new();
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
    fn xj_86_uf_path_compression() {
        let mut uf = super::Xj86UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_86_bt_insert_get() {
        let mut bt = super::Xj86BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_86_bt_contains_len() {
        let mut bt = super::Xj86BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_86_bt_replace() {
        let mut bt = super::Xj86BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_86_bt_remove() {
        let mut bt = super::Xj86BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_86_bt_keys_values() {
        let mut bt = super::Xj86BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_86_bt_range() {
        let mut bt = super::Xj86BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_86_bt_min_max() {
        let mut bt = super::Xj86BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_86_bt_many_inserts() {
        let mut bt = super::Xj86BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_86 segment tree tests ---

    #[test]
    fn xk_86_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk86SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_86_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk86SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_86_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk86SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_86_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk86SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_86_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk86SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_86_st_single_element() {
        let data = vec![42];
        let st = super::Xk86SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_86_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk86SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_86_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk86SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_86 disjoint intervals tests ---

    #[test]
    fn xk_86_di_add_and_count() {
        let mut di = super::Xk86DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_86_di_merge_overlap() {
        let mut di = super::Xk86DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_86_di_contains() {
        let mut di = super::Xk86DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_86_di_remove() {
        let mut di = super::Xk86DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_86_di_covered_length() {
        let mut di = super::Xk86DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_86_di_gaps() {
        let mut di = super::Xk86DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_86_di_merge_adjacent() {
        let mut di = super::Xk86DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_86_di_empty() {
        let di = super::Xk86DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_86_rope_new_empty() {
        let rope = super::Xl86Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_86_rope_from_str() {
        let rope = super::Xl86Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_86_rope_insert_at() {
        let mut rope = super::Xl86Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_86_rope_delete_range() {
        let mut rope = super::Xl86Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_86_rope_char_at() {
        let rope = super::Xl86Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_86_rope_split_concat() {
        let rope = super::Xl86Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_86_rope_line_count() {
        let rope = super::Xl86Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_86_rope_line_at() {
        let rope = super::Xl86Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_86_sa_build_and_search() {
        let sa = super::Xl86SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_86_sa_count() {
        let sa = super::Xl86SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_86_sa_longest_repeated() {
        let sa = super::Xl86SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_86_sa_all_positions() {
        let sa = super::Xl86SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_86_sa_len() {
        let sa = super::Xl86SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_86_sa_empty() {
        let sa = super::Xl86SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_86_rope_slice() {
        let rope = super::Xl86Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_86_sa_search_start() {
        let sa = super::Xl86SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_86_sparse_set_get() {
        let mut m = super::Xm86MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_86_sparse_row_col() {
        let mut m = super::Xm86MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_86_sparse_transpose() {
        let mut m = super::Xm86MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_86_sparse_multiply_vec() {
        let mut m = super::Xm86MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_86_sparse_nnz_density() {
        let mut m = super::Xm86MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_86_sparse_clear() {
        let mut m = super::Xm86MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_86_sparse_overwrite_zero() {
        let mut m = super::Xm86MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_86_tokenizer_basic() {
        let t = super::Xm86Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_86_tokenizer_count() {
        let t = super::Xm86Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_86_tokenizer_unique() {
        let t = super::Xm86Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_86_tokenizer_frequency() {
        let t = super::Xm86Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_86_tokenizer_delimiter() {
        let t = super::Xm86Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_86_tokenizer_whitespace() {
        let t = super::Xm86Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_86_tokenizer_empty() {
        let t = super::Xm86Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 86 ----

    #[test]
    fn xn_86_fenwick_prefix_sum() {
        let mut ft = super::Xn86Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_86_fenwick_range_sum() {
        let mut ft = super::Xn86Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_86_fenwick_point_query() {
        let mut ft = super::Xn86Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_86_fenwick_len() {
        let ft = super::Xn86Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_86_fenwick_multiple_updates() {
        let mut ft = super::Xn86Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_86_fenwick_single_element() {
        let mut ft = super::Xn86Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_86_fenwick_find_kth() {
        let mut ft = super::Xn86Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_86_fenwick_negative_delta() {
        let mut ft = super::Xn86Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 86 ----

    #[test]
    fn xn_86_avl_insert_get() {
        let mut m = super::Xn86AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_86_avl_remove() {
        let mut m = super::Xn86AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_86_avl_in_order() {
        let mut m = super::Xn86AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_86_avl_min_max() {
        let mut m = super::Xn86AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_86_avl_floor_ceiling() {
        let mut m = super::Xn86AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_86_avl_height_balanced() {
        let mut m = super::Xn86AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_86_avl_overwrite() {
        let mut m = super::Xn86AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_86_avl_empty() {
        let m: super::Xn86AVL<i32, i32> = super::Xn86AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo86RedBlack tests ---

    #[test]
    fn xo_86_rb_insert_and_get() {
        let mut tree = super::Xo86RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_86_rb_len_and_empty() {
        let mut tree = super::Xo86RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_86_rb_min_max() {
        let mut tree = super::Xo86RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_86_rb_contains() {
        let mut tree = super::Xo86RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_86_rb_remove() {
        let mut tree = super::Xo86RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_86_rb_in_order() {
        let mut tree = super::Xo86RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_86_rb_black_height() {
        let mut tree = super::Xo86RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_86_rb_overwrite() {
        let mut tree = super::Xo86RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo86ConsistentHash tests ---

    #[test]
    fn xo_86_ch_add_and_count() {
        let mut ring = super::Xo86ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_86_ch_remove_node() {
        let mut ring = super::Xo86ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_86_ch_get_node() {
        let mut ring = super::Xo86ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_86_ch_empty_ring() {
        let ring = super::Xo86ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_86_ch_distribution() {
        let mut ring = super::Xo86ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_86_ch_rebalance() {
        let mut ring = super::Xo86ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_86_ch_virtual_nodes() {
        let mut ring = super::Xo86ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_86_ch_consistent_lookup() {
        let mut ring = super::Xo86ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_86_splay_insert_get() {
        let mut t = super::Xp86SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_86_splay_remove() {
        let mut t = super::Xp86SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_86_splay_count_increases() {
        let mut t = super::Xp86SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_86_splay_depth() {
        let mut t = super::Xp86SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_86_splay_len_empty() {
        let t = super::Xp86SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_86_splay_min_max() {
        let mut t = super::Xp86SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_86_splay_overwrite() {
        let mut t = super::Xp86SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_86_splay_remove_missing() {
        let mut t = super::Xp86SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_86 treap tests ----
    #[test]
    fn xq_86_treap_empty() {
        let t = super::Xq86Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_86_treap_insert_get() {
        let mut t = super::Xq86Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_86_treap_overwrite() {
        let mut t = super::Xq86Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_86_treap_remove() {
        let mut t = super::Xq86Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_86_treap_min_max() {
        let mut t = super::Xq86Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_86_treap_rank() {
        let mut t = super::Xq86Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_86_treap_kth() {
        let mut t = super::Xq86Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_86_treap_in_order() {
        let mut t = super::Xq86Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_86 VEB tree tests ----
    #[test]
    fn xq_86_veb_empty() {
        let v = super::Xq86VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_86_veb_insert_contains() {
        let mut v = super::Xq86VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_86_veb_min_max() {
        let mut v = super::Xq86VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_86_veb_delete() {
        let mut v = super::Xq86VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_86_veb_successor() {
        let mut v = super::Xq86VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_86_veb_predecessor() {
        let mut v = super::Xq86VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_86_veb_count() {
        let mut v = super::Xq86VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_86_veb_duplicate_insert() {
        let mut v = super::Xq86VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_86_kdtree_empty() {
        let tree = super::Xr86KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_86_kdtree_insert_one() {
        let mut tree = super::Xr86KDTree::xr_new();
        tree.xr_insert(super::Xr86KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_86_kdtree_insert_multiple() {
        let mut tree = super::Xr86KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr86KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_86_kdtree_nearest_neighbor() {
        let mut tree = super::Xr86KDTree::xr_new();
        tree.xr_insert(super::Xr86KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr86KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr86KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_86_kdtree_nn_empty() {
        let tree = super::Xr86KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr86KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_86_kdtree_range_search() {
        let mut tree = super::Xr86KDTree::xr_new();
        tree.xr_insert(super::Xr86KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr86KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr86KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_86_kdtree_range_empty() {
        let mut tree = super::Xr86KDTree::xr_new();
        tree.xr_insert(super::Xr86KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_86_kdtree_all_points() {
        let mut tree = super::Xr86KDTree::xr_new();
        tree.xr_insert(super::Xr86KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr86KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_86_kdtree_depth() {
        let mut tree = super::Xr86KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr86KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_86_kdtree_bounding_box() {
        let mut tree = super::Xr86KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr86KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr86KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_86_persistent_array_new() {
        let arr = super::Xs86PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_86_persistent_array_push() {
        let mut arr = super::Xs86PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_86_persistent_array_set() {
        let mut arr = super::Xs86PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_86_persistent_array_diff() {
        let mut arr = super::Xs86PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_86_persistent_array_rollback() {
        let mut arr = super::Xs86PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_86_persistent_array_history() {
        let mut arr = super::Xs86PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_86_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs86PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_86_persistent_array_from_vec() {
        let arr = super::Xs86PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_86_concurrent_queue_new() {
        let q = super::Xs86ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_86_concurrent_queue_push_pop() {
        let mut q = super::Xs86ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_86_concurrent_queue_full() {
        let mut q = super::Xs86ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_86_concurrent_queue_drain() {
        let mut q = super::Xs86ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_86_concurrent_queue_try_pop() {
        let mut q = super::Xs86ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_86_concurrent_queue_clear() {
        let mut q = super::Xs86ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_86_range_map_new() {
        let rm = super::Xs86RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_86_range_map_insert_get() {
        let mut rm = super::Xs86RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_86_range_map_overlap() {
        let mut rm = super::Xs86RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_86_range_map_remove() {
        let mut rm = super::Xs86RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_86_range_map_gaps() {
        let mut rm = super::Xs86RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_86_range_map_coverage() {
        let mut rm = super::Xs86RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_86_range_map_contains() {
        let mut rm = super::Xs86RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_86_range_map_clear() {
        let mut rm = super::Xs86RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_86_circular_buffer_new() {
        let buf = super::Xs86CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_86_circular_buffer_push_pop() {
        let mut buf = super::Xs86CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_86_circular_buffer_overwrite() {
        let mut buf = super::Xs86CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_86_circular_buffer_peek() {
        let mut buf = super::Xs86CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_86_circular_buffer_is_full() {
        let mut buf = super::Xs86CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_86_circular_buffer_iter() {
        let mut buf = super::Xs86CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_86_circular_buffer_clear() {
        let mut buf = super::Xs86CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_86_circular_buffer_to_vec() {
        let mut buf = super::Xs86CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

    #[test]
    fn xs_86_stats_tracker_new() {
        let tracker = super::Xs86StatsTracker::xs_new();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_86_stats_tracker_mean() {
        let mut tracker = super::Xs86StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        tracker.xs_add(30.0);
        assert!((tracker.xs_mean() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn xs_86_stats_tracker_min_max() {
        let mut tracker = super::Xs86StatsTracker::xs_new();
        tracker.xs_add(5.0);
        tracker.xs_add(15.0);
        tracker.xs_add(10.0);
        assert_eq!(tracker.xs_min(), Some(5.0));
        assert_eq!(tracker.xs_max(), Some(15.0));
    }

    #[test]
    fn xs_86_stats_tracker_median() {
        let mut tracker = super::Xs86StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(3.0);
        tracker.xs_add(2.0);
        assert_eq!(tracker.xs_median(), Some(2.0));
    }

    #[test]
    fn xs_86_stats_tracker_variance() {
        let mut tracker = super::Xs86StatsTracker::xs_new();
        tracker.xs_add(2.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(5.0);
        tracker.xs_add(5.0);
        tracker.xs_add(7.0);
        tracker.xs_add(9.0);
        let var = tracker.xs_variance();
        assert!(var > 0.0);
    }

    #[test]
    fn xs_86_stats_tracker_range() {
        let mut tracker = super::Xs86StatsTracker::xs_new();
        tracker.xs_add(3.0);
        tracker.xs_add(7.0);
        tracker.xs_add(1.0);
        assert!((tracker.xs_range() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn xs_86_stats_tracker_clear() {
        let mut tracker = super::Xs86StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(2.0);
        tracker.xs_clear();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_86_stats_tracker_sum() {
        let mut tracker = super::Xs86StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        assert!((tracker.xs_sum() - 30.0).abs() < 1e-9);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }

}
