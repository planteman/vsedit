//! Go to definition, declaration, and references.

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
}

/// Result of a go-to operation.
#[derive(Debug, Clone)]
pub enum GoToResult {
    Single(Location),
    Multiple(Vec<Location>),
    None,
}

/// Provider for go-to operations.
pub trait DefinitionProvider: Send + Sync {
    fn provide_definition(&self, uri: &str, line: u32, column: u32) -> GoToResult;
}

pub trait DeclarationProvider: Send + Sync {
    fn provide_declaration(&self, uri: &str, line: u32, column: u32) -> GoToResult;
}

pub trait ReferenceProvider: Send + Sync {
    fn provide_references(&self, uri: &str, line: u32, column: u32, include_declaration: bool) -> Vec<Location>;
}

pub trait TypeDefinitionProvider: Send + Sync {
    fn provide_type_definition(&self, uri: &str, line: u32, column: u32) -> GoToResult;
}

pub trait ImplementationProvider: Send + Sync {
    fn provide_implementation(&self, uri: &str, line: u32, column: u32) -> GoToResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_creation() {
        let loc = Location::new("file:///test.rs", 10, 5);
        assert_eq!(loc.line, 10);
        assert_eq!(loc.column, 5);
    }

    #[test]
    fn goto_result_variants() {
        let r = GoToResult::Single(Location::new("f", 1, 1));
        assert!(matches!(r, GoToResult::Single(_)));
        let r = GoToResult::Multiple(vec![]);
        assert!(matches!(r, GoToResult::Multiple(_)));
    }
}
