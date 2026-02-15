//! Extension point registry for vsedit.
//!
//! Provides a simple registry of named extension points, equivalent to
//! VS Code's extension point infrastructure. Other crates register their
//! extension points here so the system can discover and validate them.

use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// ExtensionPointRegistry
// ---------------------------------------------------------------------------

/// A registry of named extension points.
///
/// Extension points are identified by string IDs (e.g.
/// `"vsedit.configuration"`, `"vsedit.commands"`). Crates register their
/// extension points at startup so other parts of the system can discover them.
pub struct ExtensionPointRegistry {
    points: HashSet<String>,
    metadata: HashMap<String, ExtensionPointMetadata>,
}

/// Optional metadata attached to an extension point.
#[derive(Debug, Clone, Default)]
pub struct ExtensionPointMetadata {
    pub description: String,
}

impl ExtensionPointRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            points: HashSet::new(),
            metadata: HashMap::new(),
        }
    }

    /// Register an extension point by ID.
    pub fn register_point(&mut self, id: &str) {
        self.points.insert(id.to_string());
    }

    /// Register an extension point with metadata.
    pub fn register_point_with_metadata(&mut self, id: &str, meta: ExtensionPointMetadata) {
        self.points.insert(id.to_string());
        self.metadata.insert(id.to_string(), meta);
    }

    /// Returns `true` if the given extension point has been registered.
    pub fn has_point(&self, id: &str) -> bool {
        self.points.contains(id)
    }

    /// Returns the number of registered extension points.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns `true` if no extension points are registered.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Returns metadata for an extension point, if any.
    pub fn get_metadata(&self, id: &str) -> Option<&ExtensionPointMetadata> {
        self.metadata.get(id)
    }

    /// Returns an iterator over all registered extension point IDs.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.points.iter().map(String::as_str)
    }
}

impl Default for ExtensionPointRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_check() {
        let mut reg = ExtensionPointRegistry::new();
        assert!(!reg.has_point("vsedit.configuration"));

        reg.register_point("vsedit.configuration");
        assert!(reg.has_point("vsedit.configuration"));
    }

    #[test]
    fn register_duplicate_is_idempotent() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("vsedit.commands");
        reg.register_point("vsedit.commands");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn len_and_is_empty() {
        let mut reg = ExtensionPointRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);

        reg.register_point("a");
        reg.register_point("b");
        assert_eq!(reg.len(), 2);
        assert!(!reg.is_empty());
    }

    #[test]
    fn metadata() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point_with_metadata(
            "vsedit.themes",
            ExtensionPointMetadata {
                description: "Color themes".into(),
            },
        );
        assert!(reg.has_point("vsedit.themes"));
        let meta = reg.get_metadata("vsedit.themes").unwrap();
        assert_eq!(meta.description, "Color themes");
    }

    #[test]
    fn get_metadata_returns_none_for_simple_point() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("simple");
        assert!(reg.get_metadata("simple").is_none());
    }

    #[test]
    fn default_impl() {
        let reg = ExtensionPointRegistry::default();
        assert!(reg.is_empty());
    }

    #[test]
    fn iter_returns_all_points() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("a");
        reg.register_point("b");
        reg.register_point("c");

        let mut ids: Vec<&str> = reg.iter().collect();
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }
}
