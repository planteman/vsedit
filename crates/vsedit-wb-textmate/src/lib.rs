//! TextMate grammar tokenization.

#[derive(Debug, Clone)]
pub struct TextMateScope {
    pub name: String,
    pub parent: Option<Box<TextMateScope>>,
}

impl TextMateScope {
    pub fn depth(&self) -> usize {
        match &self.parent {
            Some(p) => 1 + p.depth(),
            None => 1,
        }
    }

    pub fn root_scope(&self) -> &str {
        match &self.parent {
            Some(p) => p.root_scope(),
            None => &self.name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextMatePattern {
    pub name: Option<String>,
    pub match_pattern: Option<String>,
    pub begin: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TextMateGrammar {
    pub scope_name: String,
    pub language_id: String,
    pub patterns: Vec<TextMatePattern>,
}

/// Service for TextMate grammar management.
pub struct TextMateService {
    grammars: Vec<TextMateGrammar>,
}

impl TextMateService {
    pub fn new() -> Self {
        Self {
            grammars: Vec::new(),
        }
    }

    pub fn register_grammar(&mut self, grammar: TextMateGrammar) {
        self.grammars.push(grammar);
    }

    pub fn get_grammar(&self, scope_name: &str) -> Option<&TextMateGrammar> {
        self.grammars.iter().find(|g| g.scope_name == scope_name)
    }

    pub fn get_grammar_for_language(&self, language_id: &str) -> Option<&TextMateGrammar> {
        self.grammars.iter().find(|g| g.language_id == language_id)
    }

    pub fn grammar_count(&self) -> usize {
        self.grammars.len()
    }
}

impl Default for TextMateService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust_grammar() -> TextMateGrammar {
        TextMateGrammar {
            scope_name: "source.rust".into(),
            language_id: "rust".into(),
            patterns: vec![TextMatePattern {
                name: Some("keyword.control.rust".into()),
                match_pattern: Some(r"\b(if|else|match|loop)\b".into()),
                begin: None,
                end: None,
            }],
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut svc = TextMateService::new();
        svc.register_grammar(rust_grammar());
        assert_eq!(svc.grammar_count(), 1);
        assert!(svc.get_grammar("source.rust").is_some());
        assert!(svc.get_grammar("source.python").is_none());
    }

    #[test]
    fn lookup_by_language() {
        let mut svc = TextMateService::new();
        svc.register_grammar(rust_grammar());
        let g = svc.get_grammar_for_language("rust").unwrap();
        assert_eq!(g.scope_name, "source.rust");
        assert!(svc.get_grammar_for_language("python").is_none());
    }

    #[test]
    fn scope_depth_and_root() {
        let root = TextMateScope {
            name: "source.rust".into(),
            parent: None,
        };
        let child = TextMateScope {
            name: "keyword.control".into(),
            parent: Some(Box::new(root)),
        };
        let grandchild = TextMateScope {
            name: "keyword.control.if".into(),
            parent: Some(Box::new(child)),
        };
        assert_eq!(grandchild.depth(), 3);
        assert_eq!(grandchild.root_scope(), "source.rust");
    }
}
