//! Tree-sitter parsing service.

#[derive(Debug, Clone)]
pub struct TreeSitterLanguage {
    pub name: String,
    pub file_types: Vec<String>,
    pub highlight_query: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SyntaxNode {
    pub kind: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub children: Vec<SyntaxNode>,
    pub named: bool,
}

impl SyntaxNode {
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn span_lines(&self) -> u32 {
        self.end_line - self.start_line + 1
    }
}

/// Service for tree-sitter language management.
pub struct TreeSitterService {
    languages: Vec<TreeSitterLanguage>,
}

impl TreeSitterService {
    pub fn new() -> Self {
        Self {
            languages: Vec::new(),
        }
    }

    pub fn register_language(&mut self, lang: TreeSitterLanguage) {
        self.languages.push(lang);
    }

    pub fn get_language(&self, name: &str) -> Option<&TreeSitterLanguage> {
        self.languages.iter().find(|l| l.name == name)
    }

    pub fn get_language_for_file(&self, filename: &str) -> Option<&TreeSitterLanguage> {
        let ext = filename.rsplit('.').next()?;
        self.languages
            .iter()
            .find(|l| l.file_types.iter().any(|ft| ft == ext))
    }

    pub fn language_count(&self) -> usize {
        self.languages.len()
    }
}

impl Default for TreeSitterService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust_lang() -> TreeSitterLanguage {
        TreeSitterLanguage {
            name: "rust".into(),
            file_types: vec!["rs".into()],
            highlight_query: Some("(function_item name: (identifier) @function)".into()),
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut svc = TreeSitterService::new();
        svc.register_language(rust_lang());
        assert_eq!(svc.language_count(), 1);
        assert!(svc.get_language("rust").is_some());
        assert!(svc.get_language("python").is_none());
    }

    #[test]
    fn lookup_by_file_extension() {
        let mut svc = TreeSitterService::new();
        svc.register_language(rust_lang());
        let lang = svc.get_language_for_file("main.rs").unwrap();
        assert_eq!(lang.name, "rust");
        assert!(svc.get_language_for_file("main.py").is_none());
    }

    #[test]
    fn syntax_node_methods() {
        let leaf = SyntaxNode {
            kind: "identifier".into(),
            start_line: 5,
            start_col: 4,
            end_line: 5,
            end_col: 10,
            children: Vec::new(),
            named: true,
        };
        assert!(leaf.is_leaf());
        assert_eq!(leaf.child_count(), 0);
        assert_eq!(leaf.span_lines(), 1);

        let parent = SyntaxNode {
            kind: "function_item".into(),
            start_line: 1,
            start_col: 0,
            end_line: 10,
            end_col: 1,
            children: vec![leaf],
            named: true,
        };
        assert!(!parent.is_leaf());
        assert_eq!(parent.child_count(), 1);
        assert_eq!(parent.span_lines(), 10);
    }
}
