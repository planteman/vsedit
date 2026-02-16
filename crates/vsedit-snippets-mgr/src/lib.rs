//! Snippets manager.

/// Where a snippet originated from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnippetSource {
    User,
    Workspace,
    Extension,
}

/// A single code snippet.
#[derive(Debug, Clone, PartialEq)]
pub struct Snippet {
    pub name: String,
    pub prefix: Vec<String>,
    pub body: Vec<String>,
    pub description: Option<String>,
    pub scope: Option<String>,
    pub source: SnippetSource,
}

/// Service that manages a collection of snippets.
pub struct SnippetService {
    pub snippets: Vec<Snippet>,
}

impl SnippetService {
    pub fn new() -> Self {
        Self {
            snippets: Vec::new(),
        }
    }

    pub fn add_snippet(&mut self, snippet: Snippet) {
        self.snippets.push(snippet);
    }

    /// Return all snippets whose prefix list contains an entry starting with the given prefix.
    pub fn find_by_prefix(&self, prefix: &str) -> Vec<&Snippet> {
        self.snippets
            .iter()
            .filter(|s| s.prefix.iter().any(|p| p.starts_with(prefix)))
            .collect()
    }

    /// Return all snippets matching the given scope (or snippets with no scope restriction).
    pub fn find_by_scope(&self, scope: &str) -> Vec<&Snippet> {
        self.snippets
            .iter()
            .filter(|s| match &s.scope {
                Some(sc) => sc.split(',').any(|s| s.trim() == scope),
                None => true,
            })
            .collect()
    }

    /// Join body lines with newlines.
    pub fn expand_body(&self, snippet: &Snippet) -> String {
        snippet.body.join("\n")
    }

    /// Remove a snippet by name. Returns `true` if a snippet was removed.
    pub fn remove_snippet(&mut self, name: &str) -> bool {
        let before = self.snippets.len();
        self.snippets.retain(|s| s.name != name);
        self.snippets.len() < before
    }
}

impl Default for SnippetService {
    fn default() -> Self {
        Self::new()
    }
}

/// Strip basic tab-stop placeholders (`$1`, `$2`, `${1:default}`) for preview.
pub fn parse_snippet_body(body: &str) -> String {
    let mut result = String::with_capacity(body.len());
    let chars: Vec<char> = body.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '$' && i + 1 < len {
            if chars[i + 1] == '{' {
                // ${N:default} — emit the default text, or nothing if no colon
                if let Some(close) = chars[i + 2..].iter().position(|&c| c == '}') {
                    let inner_start = i + 2;
                    let inner_end = inner_start + close;
                    let inner: String = chars[inner_start..inner_end].iter().collect();
                    if let Some(colon) = inner.find(':') {
                        result.push_str(&inner[colon + 1..]);
                    }
                    i = inner_end + 1; // skip past '}'
                } else {
                    result.push(chars[i]);
                    i += 1;
                }
            } else if chars[i + 1].is_ascii_digit() {
                // $N — skip the dollar and all following digits
                i += 1;
                while i < len && chars[i].is_ascii_digit() {
                    i += 1;
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_snippet(name: &str, prefix: &str, scope: Option<&str>) -> Snippet {
        Snippet {
            name: name.into(),
            prefix: vec![prefix.into()],
            body: vec!["line1".into(), "line2".into()],
            description: None,
            scope: scope.map(String::from),
            source: SnippetSource::User,
        }
    }

    #[test]
    fn add_and_find_by_prefix() {
        let mut svc = SnippetService::new();
        svc.add_snippet(sample_snippet("for-loop", "for", Some("rust")));
        svc.add_snippet(sample_snippet("fn-def", "fn", Some("rust")));

        let found = svc.find_by_prefix("fo");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "for-loop");

        let found_all = svc.find_by_prefix("f");
        assert_eq!(found_all.len(), 2);
    }

    #[test]
    fn find_by_scope_and_remove() {
        let mut svc = SnippetService::new();
        svc.add_snippet(sample_snippet("a", "a", Some("rust")));
        svc.add_snippet(sample_snippet("b", "b", Some("python")));
        svc.add_snippet(sample_snippet("c", "c", None)); // no scope — matches everything

        let rust = svc.find_by_scope("rust");
        assert_eq!(rust.len(), 2); // "a" + "c"

        assert!(svc.remove_snippet("a"));
        assert!(!svc.remove_snippet("nonexistent"));
        assert_eq!(svc.snippets.len(), 2);
    }

    #[test]
    fn expand_body_joins_lines() {
        let svc = SnippetService::new();
        let s = sample_snippet("x", "x", None);
        assert_eq!(svc.expand_body(&s), "line1\nline2");
    }

    #[test]
    fn parse_snippet_body_strips_tabstops() {
        assert_eq!(parse_snippet_body("hello $1 world"), "hello  world");
        assert_eq!(
            parse_snippet_body("fn ${1:name}($2) { $0 }"),
            "fn name() {  }"
        );
        assert_eq!(parse_snippet_body("no placeholders"), "no placeholders");
        assert_eq!(parse_snippet_body("${1:a} ${2:b}"), "a b");
    }
}
