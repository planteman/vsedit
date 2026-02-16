//! Type hierarchy view.

use std::collections::{HashMap, HashSet};
use std::fmt;

/// The kind of a symbol in the type hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Class,
    Interface,
    Struct,
    Enum,
    TypeParameter,
    Module,
}

/// A tag that can be applied to a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolTag {
    Deprecated,
}

/// An item in the type hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeHierarchyItem {
    pub name: String,
    pub kind: SymbolKind,
    pub uri: String,
    pub range_start_line: u32,
    pub range_start_col: u32,
    pub range_end_line: u32,
    pub range_end_col: u32,
    pub detail: Option<String>,
    pub tags: Vec<SymbolTag>,
}

impl TypeHierarchyItem {
    pub fn new(
        name: String,
        kind: SymbolKind,
        uri: String,
        range_start_line: u32,
        range_start_col: u32,
        range_end_line: u32,
        range_end_col: u32,
    ) -> Self {
        Self {
            name,
            kind,
            uri,
            range_start_line,
            range_start_col,
            range_end_line,
            range_end_col,
            detail: None,
            tags: Vec::new(),
        }
    }
}

/// Provides type hierarchy information for symbols.
pub trait TypeHierarchyProvider {
    /// Prepare the type hierarchy at the given position.
    fn prepare(&self, uri: &str, line: u32, col: u32) -> Option<Vec<TypeHierarchyItem>>;

    /// Return the supertypes of the given item.
    fn supertypes(&self, item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem>;

    /// Return the subtypes of the given item.
    fn subtypes(&self, item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem>;
}

// ---------------------------------------------------------------------------
// Display implementations
// ---------------------------------------------------------------------------

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            SymbolKind::Class => "Class",
            SymbolKind::Interface => "Interface",
            SymbolKind::Struct => "Struct",
            SymbolKind::Enum => "Enum",
            SymbolKind::TypeParameter => "TypeParameter",
            SymbolKind::Module => "Module",
        };
        f.write_str(name)
    }
}

impl fmt::Display for SymbolTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymbolTag::Deprecated => f.write_str("Deprecated"),
        }
    }
}

impl fmt::Display for TypeHierarchyItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} at {}", self.name, self.kind, self.uri)
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when resolving a type hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeHierarchyError {
    /// No type symbol was found at the requested position.
    NoTypeAtPosition,
    /// The underlying provider failed.
    ProviderFailed(String),
    /// A circular reference was detected in the hierarchy.
    CircularHierarchy(String),
}

impl fmt::Display for TypeHierarchyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeHierarchyError::NoTypeAtPosition => {
                write!(f, "no type symbol found at the given position")
            }
            TypeHierarchyError::ProviderFailed(msg) => {
                write!(f, "type hierarchy provider failed: {msg}")
            }
            TypeHierarchyError::CircularHierarchy(msg) => {
                write!(f, "circular type hierarchy detected: {msg}")
            }
        }
    }
}

impl std::error::Error for TypeHierarchyError {}

// ---------------------------------------------------------------------------
// Builder-style helpers and queries on TypeHierarchyItem
// ---------------------------------------------------------------------------

impl TypeHierarchyItem {
    /// Builder-style method to set the detail field.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Builder-style method to add a tag.
    pub fn with_tag(mut self, tag: SymbolTag) -> Self {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
        self
    }

    /// Returns `true` if the item is marked as deprecated.
    pub fn is_deprecated(&self) -> bool {
        self.tags.contains(&SymbolTag::Deprecated)
    }

    /// Returns `true` if the given `(line, col)` falls within this item's range.
    pub fn contains_position(&self, line: u32, col: u32) -> bool {
        if line < self.range_start_line || line > self.range_end_line {
            return false;
        }
        if line == self.range_start_line && col < self.range_start_col {
            return false;
        }
        if line == self.range_end_line && col > self.range_end_col {
            return false;
        }
        true
    }
}

// ---------------------------------------------------------------------------
// TypeTree – full hierarchy graph built from a provider
// ---------------------------------------------------------------------------

/// A graph-based representation of a full type hierarchy.
///
/// Types are identified by their `TypeHierarchyItem`. The tree stores
/// supertype and subtype edges and can answer ancestry / descendant queries.
#[derive(Debug, Clone)]
pub struct TypeTree {
    items: Vec<TypeHierarchyItem>,
    /// Maps item index → set of supertype indices.
    supertype_edges: HashMap<usize, HashSet<usize>>,
    /// Maps item index → set of subtype indices.
    subtype_edges: HashMap<usize, HashSet<usize>>,
}

impl TypeTree {
    /// Create an empty `TypeTree`.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            supertype_edges: HashMap::new(),
            subtype_edges: HashMap::new(),
        }
    }

    /// Add a type to the tree. Returns the index of the (possibly existing) item.
    pub fn add_type(&mut self, item: TypeHierarchyItem) -> usize {
        if let Some(idx) = self.items.iter().position(|i| i == &item) {
            return idx;
        }
        let idx = self.items.len();
        self.items.push(item);
        idx
    }

    /// Record that `supertype_idx` is a supertype of `item_idx`.
    pub fn add_supertype_edge(&mut self, item_idx: usize, supertype_idx: usize) {
        self.supertype_edges
            .entry(item_idx)
            .or_default()
            .insert(supertype_idx);
        self.subtype_edges
            .entry(supertype_idx)
            .or_default()
            .insert(item_idx);
    }

    /// Record that `subtype_idx` is a subtype of `item_idx`.
    pub fn add_subtype_edge(&mut self, item_idx: usize, subtype_idx: usize) {
        self.subtype_edges
            .entry(item_idx)
            .or_default()
            .insert(subtype_idx);
        self.supertype_edges
            .entry(subtype_idx)
            .or_default()
            .insert(item_idx);
    }

    /// Direct supertypes of the item at `idx`.
    pub fn get_supertypes(&self, idx: usize) -> Vec<&TypeHierarchyItem> {
        self.supertype_edges
            .get(&idx)
            .map(|set| set.iter().filter_map(|&i| self.items.get(i)).collect())
            .unwrap_or_default()
    }

    /// Direct subtypes of the item at `idx`.
    pub fn get_subtypes(&self, idx: usize) -> Vec<&TypeHierarchyItem> {
        self.subtype_edges
            .get(&idx)
            .map(|set| set.iter().filter_map(|&i| self.items.get(i)).collect())
            .unwrap_or_default()
    }

    /// All transitive ancestors (supertypes of supertypes …).
    pub fn all_ancestors(&self, idx: usize) -> Vec<&TypeHierarchyItem> {
        let mut visited = HashSet::new();
        let mut stack = vec![idx];
        while let Some(cur) = stack.pop() {
            if let Some(parents) = self.supertype_edges.get(&cur) {
                for &p in parents {
                    if visited.insert(p) {
                        stack.push(p);
                    }
                }
            }
        }
        visited
            .iter()
            .filter_map(|&i| self.items.get(i))
            .collect()
    }

    /// All transitive descendants (subtypes of subtypes …).
    pub fn all_descendants(&self, idx: usize) -> Vec<&TypeHierarchyItem> {
        let mut visited = HashSet::new();
        let mut stack = vec![idx];
        while let Some(cur) = stack.pop() {
            if let Some(children) = self.subtype_edges.get(&cur) {
                for &c in children {
                    if visited.insert(c) {
                        stack.push(c);
                    }
                }
            }
        }
        visited
            .iter()
            .filter_map(|&i| self.items.get(i))
            .collect()
    }

    /// Maximum depth from `idx` down through subtypes. A leaf has depth 0.
    pub fn depth(&self, idx: usize) -> usize {
        match self.subtype_edges.get(&idx) {
            Some(set) if !set.is_empty() => {}
            _ => return 0,
        }
        let mut seen = HashSet::new();
        seen.insert(idx);
        self.depth_inner(idx, &mut seen)
    }

    fn depth_inner(&self, idx: usize, seen: &mut HashSet<usize>) -> usize {
        let children = match self.subtype_edges.get(&idx) {
            Some(set) if !set.is_empty() => set,
            _ => return 0,
        };
        let mut max_child = 0;
        for &c in children {
            if seen.contains(&c) {
                continue; // avoid infinite recursion on cycles
            }
            seen.insert(c);
            let d = 1 + self.depth_inner(c, seen);
            if d > max_child {
                max_child = d;
            }
        }
        max_child
    }

    /// Returns `true` if there is a cycle reachable from `idx` through
    /// supertype edges.
    pub fn has_circular_reference(&self, idx: usize) -> bool {
        let mut visited = HashSet::new();
        let mut stack = vec![idx];
        while let Some(cur) = stack.pop() {
            if !visited.insert(cur) {
                return true;
            }
            if let Some(parents) = self.supertype_edges.get(&cur) {
                for &p in parents {
                    stack.push(p);
                }
            }
        }
        false
    }
}

impl Default for TypeTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item() -> TypeHierarchyItem {
        TypeHierarchyItem::new(
            "MyClass".into(),
            SymbolKind::Class,
            "file:///src/main.rs".into(),
            10,
            0,
            20,
            1,
        )
    }

    #[test]
    fn new_item_has_defaults() {
        let item = sample_item();
        assert_eq!(item.name, "MyClass");
        assert_eq!(item.kind, SymbolKind::Class);
        assert!(item.detail.is_none());
        assert!(item.tags.is_empty());
    }

    #[test]
    fn item_with_detail_and_tags() {
        let mut item = sample_item();
        item.detail = Some("module::MyClass".into());
        item.tags.push(SymbolTag::Deprecated);
        assert_eq!(item.detail.as_deref(), Some("module::MyClass"));
        assert_eq!(item.tags, vec![SymbolTag::Deprecated]);
    }

    struct DummyProvider;

    impl TypeHierarchyProvider for DummyProvider {
        fn prepare(&self, _uri: &str, _line: u32, _col: u32) -> Option<Vec<TypeHierarchyItem>> {
            Some(vec![sample_item()])
        }

        fn supertypes(&self, _item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem> {
            vec![TypeHierarchyItem::new(
                "BaseClass".into(),
                SymbolKind::Class,
                "file:///src/base.rs".into(),
                1,
                0,
                5,
                1,
            )]
        }

        fn subtypes(&self, _item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem> {
            vec![]
        }
    }

    #[test]
    fn provider_prepare_and_supertypes() {
        let provider = DummyProvider;
        let items = provider.prepare("file:///src/main.rs", 10, 0).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "MyClass");

        let supers = provider.supertypes(&items[0]);
        assert_eq!(supers.len(), 1);
        assert_eq!(supers[0].name, "BaseClass");

        let subs = provider.subtypes(&items[0]);
        assert!(subs.is_empty());
    }

    // -----------------------------------------------------------------------
    // Display impls
    // -----------------------------------------------------------------------

    #[test]
    fn display_symbol_kind() {
        assert_eq!(SymbolKind::Class.to_string(), "Class");
        assert_eq!(SymbolKind::Interface.to_string(), "Interface");
        assert_eq!(SymbolKind::Struct.to_string(), "Struct");
        assert_eq!(SymbolKind::Enum.to_string(), "Enum");
        assert_eq!(SymbolKind::TypeParameter.to_string(), "TypeParameter");
        assert_eq!(SymbolKind::Module.to_string(), "Module");
    }

    #[test]
    fn display_symbol_tag() {
        assert_eq!(SymbolTag::Deprecated.to_string(), "Deprecated");
    }

    #[test]
    fn display_type_hierarchy_item() {
        let item = sample_item();
        assert_eq!(item.to_string(), "MyClass: Class at file:///src/main.rs");
    }

    // -----------------------------------------------------------------------
    // Error display
    // -----------------------------------------------------------------------

    #[test]
    fn display_errors() {
        let e1 = TypeHierarchyError::NoTypeAtPosition;
        assert_eq!(e1.to_string(), "no type symbol found at the given position");

        let e2 = TypeHierarchyError::ProviderFailed("timeout".into());
        assert_eq!(e2.to_string(), "type hierarchy provider failed: timeout");

        let e3 = TypeHierarchyError::CircularHierarchy("A -> B -> A".into());
        assert_eq!(
            e3.to_string(),
            "circular type hierarchy detected: A -> B -> A"
        );
    }

    // -----------------------------------------------------------------------
    // Builder methods and queries
    // -----------------------------------------------------------------------

    #[test]
    fn builder_with_detail() {
        let item = sample_item().with_detail("some::path::MyClass");
        assert_eq!(item.detail.as_deref(), Some("some::path::MyClass"));
    }

    #[test]
    fn builder_with_tag_and_is_deprecated() {
        let item = sample_item().with_tag(SymbolTag::Deprecated);
        assert!(item.is_deprecated());

        let plain = sample_item();
        assert!(!plain.is_deprecated());
    }

    #[test]
    fn with_tag_does_not_duplicate() {
        let item = sample_item()
            .with_tag(SymbolTag::Deprecated)
            .with_tag(SymbolTag::Deprecated);
        assert_eq!(item.tags.len(), 1);
    }

    #[test]
    fn contains_position_inside() {
        let item = sample_item(); // lines 10..20
        assert!(item.contains_position(10, 0));
        assert!(item.contains_position(15, 5));
        assert!(item.contains_position(20, 1));
    }

    #[test]
    fn contains_position_outside() {
        let item = sample_item();
        assert!(!item.contains_position(9, 0));
        assert!(!item.contains_position(21, 0));
        assert!(!item.contains_position(10, 0).then(|| ()).is_none()); // inside, double-check
        // before start col on start line
        let item2 = TypeHierarchyItem::new("X".into(), SymbolKind::Struct, "f".into(), 5, 3, 5, 10);
        assert!(!item2.contains_position(5, 2));
        // after end col on end line
        assert!(!item2.contains_position(5, 11));
    }

    // -----------------------------------------------------------------------
    // TypeTree
    // -----------------------------------------------------------------------

    fn make_item(name: &str, kind: SymbolKind) -> TypeHierarchyItem {
        TypeHierarchyItem::new(name.into(), kind, "file:///test".into(), 0, 0, 0, 0)
    }

    #[test]
    fn type_tree_add_and_query() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("A", SymbolKind::Class));
        let b = tree.add_type(make_item("B", SymbolKind::Class));
        let c = tree.add_type(make_item("C", SymbolKind::Class));

        // B extends A, C extends B
        tree.add_supertype_edge(b, a);
        tree.add_supertype_edge(c, b);

        assert_eq!(tree.get_supertypes(b).len(), 1);
        assert_eq!(tree.get_supertypes(b)[0].name, "A");

        assert_eq!(tree.get_subtypes(a).len(), 1);
        assert_eq!(tree.get_subtypes(a)[0].name, "B");
    }

    #[test]
    fn type_tree_all_ancestors_and_descendants() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("A", SymbolKind::Interface));
        let b = tree.add_type(make_item("B", SymbolKind::Class));
        let c = tree.add_type(make_item("C", SymbolKind::Class));
        let d = tree.add_type(make_item("D", SymbolKind::Class));

        tree.add_supertype_edge(b, a);
        tree.add_supertype_edge(c, b);
        tree.add_subtype_edge(c, d);

        let ancestors_of_c: Vec<String> =
            tree.all_ancestors(c).iter().map(|i| i.name.clone()).collect();
        assert!(ancestors_of_c.contains(&"A".to_string()));
        assert!(ancestors_of_c.contains(&"B".to_string()));
        assert_eq!(ancestors_of_c.len(), 2);

        let descendants_of_a: Vec<String> = tree
            .all_descendants(a)
            .iter()
            .map(|i| i.name.clone())
            .collect();
        assert!(descendants_of_a.contains(&"B".to_string()));
        assert!(descendants_of_a.contains(&"C".to_string()));
        // D is a subtype of C, so also a descendant of A
        assert!(descendants_of_a.contains(&"D".to_string()));
    }

    #[test]
    fn type_tree_depth() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("Root", SymbolKind::Class));
        let b = tree.add_type(make_item("Mid", SymbolKind::Class));
        let c = tree.add_type(make_item("Leaf", SymbolKind::Class));

        tree.add_subtype_edge(a, b);
        tree.add_subtype_edge(b, c);

        assert_eq!(tree.depth(a), 2);
        assert_eq!(tree.depth(b), 1);
        assert_eq!(tree.depth(c), 0);
    }

    #[test]
    fn type_tree_circular_detection() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(make_item("A", SymbolKind::Class));
        let b = tree.add_type(make_item("B", SymbolKind::Class));

        tree.add_supertype_edge(a, b);
        tree.add_supertype_edge(b, a); // cycle!

        assert!(tree.has_circular_reference(a));
        assert!(tree.has_circular_reference(b));

        // A tree without cycles
        let mut clean = TypeTree::new();
        let x = clean.add_type(make_item("X", SymbolKind::Struct));
        let y = clean.add_type(make_item("Y", SymbolKind::Struct));
        clean.add_supertype_edge(y, x);
        assert!(!clean.has_circular_reference(y));
        assert!(!clean.has_circular_reference(x));
    }
}
