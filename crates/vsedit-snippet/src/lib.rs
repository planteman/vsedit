//! Snippet engine.
//!
//! Parses and processes VS Code snippet syntax (TextMate-compatible).
//! Supports tabstops, placeholders, choices, variables, and transforms.

use std::collections::HashMap;

/// A parsed snippet.
#[derive(Debug, Clone)]
pub struct Snippet {
    pub elements: Vec<SnippetElement>,
}

/// An element in a parsed snippet.
#[derive(Debug, Clone)]
pub enum SnippetElement {
    /// Plain text.
    Text(String),
    /// A tabstop: $N or ${N}.
    Tabstop(u32),
    /// A placeholder: ${N:default}.
    Placeholder {
        index: u32,
        default: Vec<SnippetElement>,
    },
    /// A choice: ${N|one,two,three|}.
    Choice {
        index: u32,
        choices: Vec<String>,
    },
    /// A variable: $VAR or ${VAR:default}.
    Variable {
        name: String,
        default: Option<Vec<SnippetElement>>,
    },
}

/// Variables available during snippet insertion.
pub struct SnippetVariables {
    values: HashMap<String, String>,
}

impl SnippetVariables {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn set(&mut self, name: &str, value: &str) {
        self.values.insert(name.to_string(), value.to_string());
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(|s| s.as_str())
    }

    /// Populate with common VS Code snippet variables.
    pub fn with_defaults(mut self, filename: &str, clipboard: &str) -> Self {
        self.set("TM_FILENAME", filename);
        self.set(
            "TM_FILENAME_BASE",
            filename.rsplit('.').nth(1).unwrap_or(filename),
        );
        self.set("CLIPBOARD", clipboard);
        self.set("TM_CURRENT_LINE", "");
        self.set("TM_CURRENT_WORD", "");
        self.set("TM_LINE_INDEX", "0");
        self.set("TM_LINE_NUMBER", "1");
        self
    }
}

impl Default for SnippetVariables {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a snippet body string into elements.
pub fn parse_snippet(body: &str) -> Snippet {
    let mut elements = Vec::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;
    let mut text_buf = String::new();

    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            // Escaped character
            text_buf.push(chars[i + 1]);
            i += 2;
            continue;
        }

        if chars[i] == '$' {
            // Flush text buffer
            if !text_buf.is_empty() {
                elements.push(SnippetElement::Text(text_buf.clone()));
                text_buf.clear();
            }

            if i + 1 < chars.len() && chars[i + 1] == '{' {
                // ${...} form
                if let Some((elem, end)) = parse_braced(&chars, i + 2) {
                    elements.push(elem);
                    i = end;
                    continue;
                }
            } else if i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
                // $N form
                let mut num = String::new();
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    num.push(chars[j]);
                    j += 1;
                }
                let index: u32 = num.parse().unwrap_or(0);
                elements.push(SnippetElement::Tabstop(index));
                i = j;
                continue;
            } else if i + 1 < chars.len() && (chars[i + 1].is_alphabetic() || chars[i + 1] == '_')
            {
                // $VAR form
                let mut name = String::new();
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                    name.push(chars[j]);
                    j += 1;
                }
                elements.push(SnippetElement::Variable {
                    name,
                    default: None,
                });
                i = j;
                continue;
            }
        }

        text_buf.push(chars[i]);
        i += 1;
    }

    if !text_buf.is_empty() {
        elements.push(SnippetElement::Text(text_buf));
    }

    Snippet { elements }
}

fn parse_braced(chars: &[char], start: usize) -> Option<(SnippetElement, usize)> {
    let mut i = start;

    // Check for number (tabstop/placeholder/choice) or variable name
    if i < chars.len() && chars[i].is_ascii_digit() {
        let mut num = String::new();
        while i < chars.len() && chars[i].is_ascii_digit() {
            num.push(chars[i]);
            i += 1;
        }
        let index: u32 = num.parse().unwrap_or(0);

        if i < chars.len() && chars[i] == '}' {
            return Some((SnippetElement::Tabstop(index), i + 1));
        }

        if i < chars.len() && chars[i] == ':' {
            // Placeholder ${N:default}
            i += 1;
            let mut default_text = String::new();
            let mut depth = 1;
            while i < chars.len() && depth > 0 {
                if chars[i] == '{' {
                    depth += 1;
                } else if chars[i] == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                default_text.push(chars[i]);
                i += 1;
            }
            if i < chars.len() && chars[i] == '}' {
                return Some((
                    SnippetElement::Placeholder {
                        index,
                        default: vec![SnippetElement::Text(default_text)],
                    },
                    i + 1,
                ));
            }
        }

        if i < chars.len() && chars[i] == '|' {
            // Choice ${N|one,two,three|}
            i += 1;
            let mut choices = Vec::new();
            let mut current = String::new();
            while i < chars.len() {
                if chars[i] == '|' && i + 1 < chars.len() && chars[i + 1] == '}' {
                    choices.push(current);
                    return Some((SnippetElement::Choice { index, choices }, i + 2));
                } else if chars[i] == ',' {
                    choices.push(current.clone());
                    current.clear();
                } else {
                    current.push(chars[i]);
                }
                i += 1;
            }
        }
    } else if i < chars.len() && (chars[i].is_alphabetic() || chars[i] == '_') {
        // Variable ${VAR} or ${VAR:default}
        let mut name = String::new();
        while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
            name.push(chars[i]);
            i += 1;
        }

        if i < chars.len() && chars[i] == '}' {
            return Some((
                SnippetElement::Variable {
                    name,
                    default: None,
                },
                i + 1,
            ));
        }

        if i < chars.len() && chars[i] == ':' {
            i += 1;
            let mut default_text = String::new();
            let mut depth = 1;
            while i < chars.len() && depth > 0 {
                if chars[i] == '{' {
                    depth += 1;
                } else if chars[i] == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                default_text.push(chars[i]);
                i += 1;
            }
            if i < chars.len() && chars[i] == '}' {
                return Some((
                    SnippetElement::Variable {
                        name,
                        default: Some(vec![SnippetElement::Text(default_text)]),
                    },
                    i + 1,
                ));
            }
        }
    }

    None
}

/// Expand a snippet to its initial text, resolving variables and using defaults.
pub fn expand_snippet(snippet: &Snippet, variables: &SnippetVariables) -> String {
    let mut result = String::new();
    for element in &snippet.elements {
        expand_element(element, variables, &mut result);
    }
    result
}

fn expand_element(element: &SnippetElement, variables: &SnippetVariables, out: &mut String) {
    match element {
        SnippetElement::Text(t) => out.push_str(t),
        SnippetElement::Tabstop(_) => {}
        SnippetElement::Placeholder { default, .. } => {
            for d in default {
                expand_element(d, variables, out);
            }
        }
        SnippetElement::Choice { choices, .. } => {
            if let Some(first) = choices.first() {
                out.push_str(first);
            }
        }
        SnippetElement::Variable { name, default } => {
            if let Some(value) = variables.get(name) {
                out.push_str(value);
            } else if let Some(default_elements) = default {
                for d in default_elements {
                    expand_element(d, variables, out);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_text() {
        let s = parse_snippet("hello world");
        assert_eq!(s.elements.len(), 1);
        assert!(matches!(&s.elements[0], SnippetElement::Text(t) if t == "hello world"));
    }

    #[test]
    fn parse_tabstop() {
        let s = parse_snippet("fn $1() {\n\t$0\n}");
        assert!(s.elements.len() >= 3);
        assert!(matches!(&s.elements[1], SnippetElement::Tabstop(1)));
    }

    #[test]
    fn parse_placeholder() {
        let s = parse_snippet("${1:name}");
        assert_eq!(s.elements.len(), 1);
        if let SnippetElement::Placeholder { index, default } = &s.elements[0] {
            assert_eq!(*index, 1);
            assert!(matches!(&default[0], SnippetElement::Text(t) if t == "name"));
        } else {
            panic!("Expected placeholder");
        }
    }

    #[test]
    fn parse_choice() {
        let s = parse_snippet("${1|yes,no,maybe|}");
        assert_eq!(s.elements.len(), 1);
        if let SnippetElement::Choice { index, choices } = &s.elements[0] {
            assert_eq!(*index, 1);
            assert_eq!(choices, &["yes", "no", "maybe"]);
        } else {
            panic!("Expected choice");
        }
    }

    #[test]
    fn parse_variable() {
        let s = parse_snippet("$TM_FILENAME");
        assert_eq!(s.elements.len(), 1);
        assert!(
            matches!(&s.elements[0], SnippetElement::Variable { name, default: None } if name == "TM_FILENAME")
        );
    }

    #[test]
    fn parse_variable_with_default() {
        let s = parse_snippet("${TM_FILENAME:untitled}");
        if let SnippetElement::Variable { name, default } = &s.elements[0] {
            assert_eq!(name, "TM_FILENAME");
            assert!(default.is_some());
        } else {
            panic!("Expected variable");
        }
    }

    #[test]
    fn expand_with_variables() {
        let s = parse_snippet("Hello $TM_FILENAME!");
        let mut vars = SnippetVariables::new();
        vars.set("TM_FILENAME", "test.rs");
        let result = expand_snippet(&s, &vars);
        assert_eq!(result, "Hello test.rs!");
    }

    #[test]
    fn expand_variable_default() {
        let s = parse_snippet("${UNKNOWN:fallback}");
        let vars = SnippetVariables::new();
        assert_eq!(expand_snippet(&s, &vars), "fallback");
    }

    #[test]
    fn expand_choice_uses_first() {
        let s = parse_snippet("${1|public,private|}");
        let vars = SnippetVariables::new();
        assert_eq!(expand_snippet(&s, &vars), "public");
    }

    #[test]
    fn escaped_dollar() {
        let s = parse_snippet("cost is \\$100");
        assert_eq!(s.elements.len(), 1);
        assert!(matches!(&s.elements[0], SnippetElement::Text(t) if t == "cost is $100"));
    }
}
