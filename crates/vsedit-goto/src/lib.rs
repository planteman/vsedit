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
}
