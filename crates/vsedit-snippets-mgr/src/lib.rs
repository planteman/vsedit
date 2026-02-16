//! Snippets manager.

use std::collections::HashMap;

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

    /// Get all snippets as a slice.
    pub fn get_all_snippets(&self) -> &[Snippet] {
        &self.snippets
    }

    /// Return the total number of snippets.
    pub fn snippet_count(&self) -> usize {
        self.snippets.len()
    }

    /// Find a snippet by its exact name.
    pub fn find_by_name(&self, name: &str) -> Option<&Snippet> {
        self.snippets.iter().find(|s| s.name == name)
    }

    /// Find all snippets from a specific source.
    pub fn find_by_source(&self, source: &SnippetSource) -> Vec<&Snippet> {
        self.snippets.iter().filter(|s| &s.source == source).collect()
    }

    /// Get all unique scopes across all snippets.
    pub fn get_unique_scopes(&self) -> Vec<String> {
        let mut scopes = std::collections::HashSet::new();
        for s in &self.snippets {
            if let Some(scope) = &s.scope {
                for part in scope.split(',') {
                    let trimmed = part.trim();
                    if !trimmed.is_empty() {
                        scopes.insert(trimmed.to_string());
                    }
                }
            }
        }
        let mut result: Vec<String> = scopes.into_iter().collect();
        result.sort();
        result
    }

    /// Get completions combining prefix and scope matching.
    pub fn get_completions(&self, prefix: &str, scope: &str) -> Vec<SnippetCompletion> {
        self.snippets
            .iter()
            .filter(|s| {
                let prefix_match = s.prefix.iter().any(|p| p.starts_with(prefix));
                let scope_match = match &s.scope {
                    Some(sc) => sc.split(',').any(|part| part.trim() == scope),
                    None => true,
                };
                prefix_match && scope_match
            })
            .map(|s| SnippetCompletion {
                label: s.prefix.first().cloned().unwrap_or_default(),
                detail: s.description.clone(),
                insert_text: s.body.join("\n"),
                snippet_name: s.name.clone(),
            })
            .collect()
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

/// A completion item derived from a snippet.
#[derive(Debug, Clone)]
pub struct SnippetCompletion {
    pub label: String,
    pub detail: Option<String>,
    pub insert_text: String,
    pub snippet_name: String,
}

/// Replace `${VAR}` and `$VAR` placeholders in the body with values from `vars`.
pub fn apply_snippet_variables(body: &str, vars: &HashMap<String, String>) -> String {
    let mut result = body.to_string();
    for (key, value) in vars {
        let braced = format!("${{{}}}", key);
        result = result.replace(&braced, value);
        let dollar = format!("${}", key);
        result = result.replace(&dollar, value);
    }
    result
}

/// Count the number of unique tabstop numbers in a snippet body.
pub fn count_tabstops(body: &str) -> usize {
    let mut seen = std::collections::HashSet::new();
    let chars: Vec<char> = body.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '$' && i + 1 < len {
            if chars[i + 1] == '{' {
                // ${N:...} or ${N}
                let mut num = String::new();
                let mut j = i + 2;
                while j < len && chars[j].is_ascii_digit() {
                    num.push(chars[j]);
                    j += 1;
                }
                if !num.is_empty() {
                    seen.insert(num);
                }
                i = j;
            } else if chars[i + 1].is_ascii_digit() {
                // $N
                let mut num = String::new();
                let mut j = i + 1;
                while j < len && chars[j].is_ascii_digit() {
                    num.push(chars[j]);
                    j += 1;
                }
                seen.insert(num);
                i = j;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    seen.len()
}

/// Trait for providing snippets from different sources.
pub trait SnippetProvider {
    /// Return the display name of this provider.
    fn name(&self) -> &str;

    /// Load snippets from this provider.
    fn load_snippets(&self) -> Vec<Snippet>;

    /// Return the source type for snippets from this provider.
    fn source(&self) -> SnippetSource {
        SnippetSource::User
    }
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

    #[test]
    fn get_all_snippets_returns_slice() {
        let mut svc = SnippetService::new();
        svc.add_snippet(sample_snippet("a", "a", None));
        svc.add_snippet(sample_snippet("b", "b", None));
        assert_eq!(svc.get_all_snippets().len(), 2);
    }

    #[test]
    fn snippet_count_works() {
        let mut svc = SnippetService::new();
        assert_eq!(svc.snippet_count(), 0);
        svc.add_snippet(sample_snippet("a", "a", None));
        assert_eq!(svc.snippet_count(), 1);
    }

    #[test]
    fn find_by_name_found() {
        let mut svc = SnippetService::new();
        svc.add_snippet(sample_snippet("for-loop", "for", Some("rust")));
        let found = svc.find_by_name("for-loop");
        assert!(found.is_some());
        assert_eq!(found.unwrap().prefix[0], "for");
    }

    #[test]
    fn find_by_name_not_found() {
        let svc = SnippetService::new();
        assert!(svc.find_by_name("nonexistent").is_none());
    }

    #[test]
    fn find_by_source_filters() {
        let mut svc = SnippetService::new();
        svc.add_snippet(sample_snippet("a", "a", None));
        svc.add_snippet(Snippet {
            name: "ext".into(),
            prefix: vec!["ext".into()],
            body: vec!["body".into()],
            description: None,
            scope: None,
            source: SnippetSource::Extension,
        });
        let user_snippets = svc.find_by_source(&SnippetSource::User);
        assert_eq!(user_snippets.len(), 1);
        assert_eq!(user_snippets[0].name, "a");
    }

    #[test]
    fn get_unique_scopes_deduplicates() {
        let mut svc = SnippetService::new();
        svc.add_snippet(sample_snippet("a", "a", Some("rust, python")));
        svc.add_snippet(sample_snippet("b", "b", Some("rust")));
        let scopes = svc.get_unique_scopes();
        assert_eq!(scopes, vec!["python", "rust"]);
    }

    #[test]
    fn get_completions_combines_prefix_and_scope() {
        let mut svc = SnippetService::new();
        svc.add_snippet(sample_snippet("for-loop", "for", Some("rust")));
        svc.add_snippet(sample_snippet("fn-def", "fn", Some("rust")));
        svc.add_snippet(sample_snippet("for-py", "for", Some("python")));
        let completions = svc.get_completions("for", "rust");
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].snippet_name, "for-loop");
    }

    #[test]
    fn apply_snippet_variables_replaces() {
        let mut vars = HashMap::new();
        vars.insert("NAME".to_string(), "World".to_string());
        vars.insert("GREETING".to_string(), "Hello".to_string());
        let result = apply_snippet_variables("$GREETING ${NAME}!", &vars);
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn count_tabstops_counts_unique() {
        assert_eq!(count_tabstops("$1 $2 $3"), 3);
        assert_eq!(count_tabstops("$1 $1 $2"), 2);
        assert_eq!(count_tabstops("${1:name} $2 ${1:other}"), 2);
        assert_eq!(count_tabstops("no tabstops"), 0);
    }
}
