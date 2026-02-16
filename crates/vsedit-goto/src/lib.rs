//! Go to definition, declaration, implementation, type definition, and references.
//!
//! Provides the location link model and a service that aggregates multiple
//! providers to resolve go-to requests.

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
#[derive(Debug, Clone)]
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
}

impl Default for GotoService {
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
}
