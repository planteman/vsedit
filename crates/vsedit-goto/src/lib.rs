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

}
