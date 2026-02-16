//! Outline view (document structure).

use std::fmt;

/// Errors that can occur when operating on an outline model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutlineError {
    ElementNotFound(String),
    EmptyModel,
    InvalidRange { start: u32, end: u32 },
}

impl fmt::Display for OutlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutlineError::ElementNotFound(label) => write!(f, "element not found: {label}"),
            OutlineError::EmptyModel => write!(f, "outline model is empty"),
            OutlineError::InvalidRange { start, end } => {
                write!(f, "invalid range: {start}..{end}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineKind {
    File,
    Module,
    Namespace,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Struct,
    Event,
}

impl fmt::Display for OutlineKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            OutlineKind::File => "File",
            OutlineKind::Module => "Module",
            OutlineKind::Namespace => "Namespace",
            OutlineKind::Class => "Class",
            OutlineKind::Method => "Method",
            OutlineKind::Property => "Property",
            OutlineKind::Field => "Field",
            OutlineKind::Constructor => "Constructor",
            OutlineKind::Enum => "Enum",
            OutlineKind::Interface => "Interface",
            OutlineKind::Function => "Function",
            OutlineKind::Variable => "Variable",
            OutlineKind::Constant => "Constant",
            OutlineKind::String => "String",
            OutlineKind::Number => "Number",
            OutlineKind::Boolean => "Boolean",
            OutlineKind::Array => "Array",
            OutlineKind::Object => "Object",
            OutlineKind::Key => "Key",
            OutlineKind::Struct => "Struct",
            OutlineKind::Event => "Event",
        };
        write!(f, "{name}")
    }
}

#[derive(Debug, Clone)]
pub struct OutlineElement {
    pub label: String,
    pub detail: Option<String>,
    pub kind: OutlineKind,
    pub range_start_line: u32,
    pub range_end_line: u32,
    pub children: Vec<OutlineElement>,
}

impl fmt::Display for OutlineElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}) [{}-{}]",
            self.label, self.kind, self.range_start_line, self.range_end_line
        )
    }
}

impl OutlineElement {
    /// Builder method: append a child element and return self.
    pub fn with_child(mut self, child: OutlineElement) -> Self {
        self.children.push(child);
        self
    }

    /// Builder method: set the detail string and return self.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

pub struct OutlineModel {
    pub elements: Vec<OutlineElement>,
    pub uri: String,
}

impl OutlineModel {
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            elements: Vec::new(),
            uri: uri.into(),
        }
    }

    pub fn add_element(&mut self, elem: OutlineElement) {
        self.elements.push(elem);
    }

    /// Returns all elements and their descendants in a flat list (pre-order).
    pub fn flatten(&self) -> Vec<&OutlineElement> {
        let mut result = Vec::new();
        fn collect<'a>(elems: &'a [OutlineElement], out: &mut Vec<&'a OutlineElement>) {
            for e in elems {
                out.push(e);
                collect(&e.children, out);
            }
        }
        collect(&self.elements, &mut result);
        result
    }

    /// Find the deepest element whose range contains the given line.
    pub fn find_at_line(&self, line: u32) -> Option<&OutlineElement> {
        fn search(elems: &[OutlineElement], line: u32) -> Option<&OutlineElement> {
            for e in elems {
                if line >= e.range_start_line && line <= e.range_end_line {
                    if let Some(child) = search(&e.children, line) {
                        return Some(child);
                    }
                    return Some(e);
                }
            }
            None
        }
        search(&self.elements, line)
    }

    /// Total count of all elements including nested children.
    pub fn element_count(&self) -> usize {
        self.flatten().len()
    }

    /// Return all elements (and descendants) matching a specific kind.
    pub fn filter_by_kind(&self, kind: OutlineKind) -> Vec<&OutlineElement> {
        self.flatten().into_iter().filter(|e| e.kind == kind).collect()
    }

    /// Find elements whose label contains `query` (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&OutlineElement> {
        let q = query.to_lowercase();
        self.flatten()
            .into_iter()
            .filter(|e| e.label.to_lowercase().contains(&q))
            .collect()
    }

    /// Maximum nesting depth (0 if the model is empty).
    pub fn depth(&self) -> usize {
        fn max_depth(elems: &[OutlineElement], current: usize) -> usize {
            let mut best = if elems.is_empty() { 0 } else { current };
            for e in elems {
                best = best.max(max_depth(&e.children, current + 1));
            }
            best
        }
        max_depth(&self.elements, 1)
    }

    /// Returns the path from root to deepest element containing `line`.
    pub fn breadcrumb_at_line(&self, line: u32) -> Vec<&OutlineElement> {
        fn collect_path<'a>(
            elems: &'a [OutlineElement],
            line: u32,
            path: &mut Vec<&'a OutlineElement>,
        ) -> bool {
            for e in elems {
                if line >= e.range_start_line && line <= e.range_end_line {
                    path.push(e);
                    collect_path(&e.children, line, path);
                    return true;
                }
            }
            false
        }
        let mut path = Vec::new();
        collect_path(&self.elements, line, &mut path);
        path
    }

    /// Sort top-level elements alphabetically by label.
    pub fn sort_by_name(&mut self) {
        fn sort_recursive(elems: &mut [OutlineElement]) {
            elems.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
            for e in elems.iter_mut() {
                sort_recursive(&mut e.children);
            }
        }
        sort_recursive(&mut self.elements);
    }

    /// Sort top-level elements by their start line.
    pub fn sort_by_position(&mut self) {
        fn sort_recursive(elems: &mut [OutlineElement]) {
            elems.sort_by_key(|e| e.range_start_line);
            for e in elems.iter_mut() {
                sort_recursive(&mut e.children);
            }
        }
        sort_recursive(&mut self.elements);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elem(label: &str, kind: OutlineKind, start: u32, end: u32) -> OutlineElement {
        OutlineElement {
            label: label.into(),
            detail: None,
            kind,
            range_start_line: start,
            range_end_line: end,
            children: Vec::new(),
        }
    }

    #[test]
    fn add_and_count() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("main", OutlineKind::Function, 1, 10));
        model.add_element(elem("Foo", OutlineKind::Struct, 12, 20));
        assert_eq!(model.element_count(), 2);
        assert_eq!(model.uri, "file.rs");
    }

    #[test]
    fn flatten_includes_children() {
        let mut model = OutlineModel::new("file.rs");
        let mut parent = elem("MyStruct", OutlineKind::Struct, 1, 30);
        parent.children.push(elem("field_a", OutlineKind::Field, 2, 2));
        parent.children.push(elem("method_b", OutlineKind::Method, 4, 10));
        model.add_element(parent);
        let flat = model.flatten();
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].label, "MyStruct");
        assert_eq!(flat[1].label, "field_a");
    }

    #[test]
    fn find_at_line_deepest() {
        let mut model = OutlineModel::new("file.rs");
        let mut parent = elem("Outer", OutlineKind::Class, 1, 50);
        parent.children.push(elem("inner", OutlineKind::Method, 10, 20));
        model.add_element(parent);
        model.add_element(elem("standalone", OutlineKind::Function, 55, 60));

        let found = model.find_at_line(15).unwrap();
        assert_eq!(found.label, "inner");

        let found = model.find_at_line(5).unwrap();
        assert_eq!(found.label, "Outer");

        let found = model.find_at_line(57).unwrap();
        assert_eq!(found.label, "standalone");

        assert!(model.find_at_line(100).is_none());
    }

    #[test]
    fn outline_error_display() {
        let e = OutlineError::ElementNotFound("foo".into());
        assert_eq!(e.to_string(), "element not found: foo");
        assert_eq!(OutlineError::EmptyModel.to_string(), "outline model is empty");
        let e = OutlineError::InvalidRange { start: 5, end: 2 };
        assert_eq!(e.to_string(), "invalid range: 5..2");
    }

    #[test]
    fn outline_kind_display() {
        assert_eq!(OutlineKind::Function.to_string(), "Function");
        assert_eq!(OutlineKind::Struct.to_string(), "Struct");
        assert_eq!(OutlineKind::Event.to_string(), "Event");
    }

    #[test]
    fn outline_element_display() {
        let e = elem("main", OutlineKind::Function, 1, 10);
        assert_eq!(e.to_string(), "main (Function) [1-10]");
    }

    #[test]
    fn with_child_builder() {
        let e = elem("Parent", OutlineKind::Class, 1, 50)
            .with_child(elem("child_a", OutlineKind::Field, 2, 2))
            .with_child(elem("child_b", OutlineKind::Method, 4, 10));
        assert_eq!(e.children.len(), 2);
        assert_eq!(e.children[0].label, "child_a");
    }

    #[test]
    fn with_detail_builder() {
        let e = elem("foo", OutlineKind::Function, 1, 5).with_detail("returns i32");
        assert_eq!(e.detail.as_deref(), Some("returns i32"));
    }

    #[test]
    fn filter_by_kind() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("main", OutlineKind::Function, 1, 10));
        model.add_element(
            elem("MyStruct", OutlineKind::Struct, 12, 30)
                .with_child(elem("new", OutlineKind::Function, 13, 20)),
        );
        model.add_element(elem("FOO", OutlineKind::Constant, 32, 32));
        let fns = model.filter_by_kind(OutlineKind::Function);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].label, "main");
        assert_eq!(fns[1].label, "new");
    }

    #[test]
    fn search_case_insensitive() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("MyStruct", OutlineKind::Struct, 1, 20));
        model.add_element(elem("my_func", OutlineKind::Function, 22, 30));
        model.add_element(elem("OTHER", OutlineKind::Constant, 32, 32));
        let results = model.search("my");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn depth_empty_model() {
        let model = OutlineModel::new("file.rs");
        assert_eq!(model.depth(), 0);
    }

    #[test]
    fn depth_nested() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("A", OutlineKind::Class, 1, 50)
                .with_child(
                    elem("B", OutlineKind::Method, 2, 40)
                        .with_child(elem("C", OutlineKind::Variable, 3, 3)),
                ),
        );
        model.add_element(elem("flat", OutlineKind::Function, 52, 60));
        assert_eq!(model.depth(), 3);
    }

    #[test]
    fn breadcrumb_at_line() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(
            elem("Outer", OutlineKind::Class, 1, 50)
                .with_child(elem("inner", OutlineKind::Method, 10, 20)),
        );
        let crumbs = model.breadcrumb_at_line(15);
        assert_eq!(crumbs.len(), 2);
        assert_eq!(crumbs[0].label, "Outer");
        assert_eq!(crumbs[1].label, "inner");

        let crumbs = model.breadcrumb_at_line(5);
        assert_eq!(crumbs.len(), 1);
        assert_eq!(crumbs[0].label, "Outer");

        assert!(model.breadcrumb_at_line(100).is_empty());
    }

    #[test]
    fn sort_by_name() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("Zebra", OutlineKind::Struct, 1, 10));
        model.add_element(elem("alpha", OutlineKind::Function, 12, 20));
        model.add_element(elem("Beta", OutlineKind::Constant, 22, 25));
        model.sort_by_name();
        let names: Vec<_> = model.elements.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(names, vec!["alpha", "Beta", "Zebra"]);
    }

    #[test]
    fn sort_by_position() {
        let mut model = OutlineModel::new("file.rs");
        model.add_element(elem("late", OutlineKind::Function, 50, 60));
        model.add_element(elem("early", OutlineKind::Function, 1, 10));
        model.add_element(elem("mid", OutlineKind::Function, 20, 30));
        model.sort_by_position();
        let starts: Vec<_> = model.elements.iter().map(|e| e.range_start_line).collect();
        assert_eq!(starts, vec![1, 20, 50]);
    }
}
