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


}
