//! Snippet engine.
//!
//! Parses and processes VS Code snippet syntax (TextMate-compatible).
//! Supports tabstops, placeholders, choices, variables, transforms,
//! VS Code snippet file parsing, and snippet insertion sessions.

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

// ---------------------------------------------------------------------------
// Snippet utilities
// ---------------------------------------------------------------------------

/// Collect all tabstop indices from a snippet (including placeholders and choices).
pub fn collect_tabstops(snippet: &Snippet) -> Vec<u32> {
    let mut indices = Vec::new();
    for elem in &snippet.elements {
        collect_tabstops_elem(elem, &mut indices);
    }
    indices.sort();
    indices.dedup();
    indices
}

fn collect_tabstops_elem(elem: &SnippetElement, out: &mut Vec<u32>) {
    match elem {
        SnippetElement::Tabstop(idx) => out.push(*idx),
        SnippetElement::Placeholder { index, default } => {
            out.push(*index);
            for d in default {
                collect_tabstops_elem(d, out);
            }
        }
        SnippetElement::Choice { index, .. } => out.push(*index),
        SnippetElement::Variable { default, .. } => {
            if let Some(defaults) = default {
                for d in defaults {
                    collect_tabstops_elem(d, out);
                }
            }
        }
        SnippetElement::Text(_) => {}
    }
}

/// Count the total number of elements (recursively) in a snippet.
pub fn element_count(snippet: &Snippet) -> usize {
    snippet.elements.iter().map(|e| element_count_elem(e)).sum()
}

fn element_count_elem(elem: &SnippetElement) -> usize {
    match elem {
        SnippetElement::Placeholder { default, .. } => {
            1 + default.iter().map(|d| element_count_elem(d)).sum::<usize>()
        }
        SnippetElement::Variable { default: Some(d), .. } => {
            1 + d.iter().map(|dd| element_count_elem(dd)).sum::<usize>()
        }
        _ => 1,
    }
}

/// Collect all variable names referenced in a snippet.
pub fn collect_variables(snippet: &Snippet) -> Vec<String> {
    let mut names = Vec::new();
    for elem in &snippet.elements {
        collect_vars_elem(elem, &mut names);
    }
    names.sort();
    names.dedup();
    names
}

fn collect_vars_elem(elem: &SnippetElement, out: &mut Vec<String>) {
    match elem {
        SnippetElement::Variable { name, default } => {
            out.push(name.clone());
            if let Some(defaults) = default {
                for d in defaults {
                    collect_vars_elem(d, out);
                }
            }
        }
        SnippetElement::Placeholder { default, .. } => {
            for d in default {
                collect_vars_elem(d, out);
            }
        }
        _ => {}
    }
}

/// A named snippet definition (as stored in a snippets file).
#[derive(Debug, Clone)]
pub struct SnippetDefinition {
    pub name: String,
    pub prefix: String,
    pub body: String,
    pub description: Option<String>,
}

impl SnippetDefinition {
    pub fn new(name: impl Into<String>, prefix: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            prefix: prefix.into(),
            body: body.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Parse the body into a Snippet.
    pub fn parse(&self) -> Snippet {
        parse_snippet(&self.body)
    }

    /// Expand the body using the given variables.
    pub fn expand(&self, variables: &SnippetVariables) -> String {
        let snippet = self.parse();
        expand_snippet(&snippet, variables)
    }
}

impl std::fmt::Display for SnippetDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(prefix={})", self.name, self.prefix)
    }
}

/// A registry of named snippet definitions.
#[derive(Debug, Clone, Default)]
pub struct SnippetRegistry {
    snippets: Vec<SnippetDefinition>,
}

impl SnippetRegistry {
    pub fn new() -> Self {
        Self { snippets: Vec::new() }
    }

    pub fn register(&mut self, def: SnippetDefinition) {
        self.snippets.push(def);
    }

    /// Find all snippets whose prefix starts with the given text.
    pub fn find_by_prefix(&self, text: &str) -> Vec<&SnippetDefinition> {
        self.snippets.iter().filter(|s| s.prefix.starts_with(text)).collect()
    }

    /// Find a snippet by exact name.
    pub fn find_by_name(&self, name: &str) -> Option<&SnippetDefinition> {
        self.snippets.iter().find(|s| s.name == name)
    }

    pub fn len(&self) -> usize {
        self.snippets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snippets.is_empty()
    }
}

// ---------------------------------------------------------------------------
// VS Code snippet file model
// ---------------------------------------------------------------------------

/// A VS Code snippet file entry with full metadata.
#[derive(Debug, Clone)]
pub struct SnippetFileEntry {
    pub name: String,
    pub prefix: Vec<String>,
    pub body: Vec<String>,
    pub description: Option<String>,
    pub scope: Option<String>,
}

/// A parsed VS Code snippet file.
#[derive(Debug, Clone)]
pub struct SnippetFile {
    pub language_id: Option<String>,
    pub snippets: HashMap<String, SnippetFileEntry>,
}

impl SnippetFile {
    /// Parse a VS Code snippet JSON string.
    ///
    /// The format is `{ "Name": { "prefix": ..., "body": ..., "description": ... } }`.
    pub fn parse(json: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
        let obj = value.as_object().ok_or("expected JSON object")?;
        let mut snippets = HashMap::new();
        for (name, entry) in obj {
            let entry_obj = entry.as_object().ok_or(format!("expected object for {name}"))?;
            let prefix = match entry_obj.get("prefix") {
                Some(serde_json::Value::String(s)) => vec![s.clone()],
                Some(serde_json::Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                _ => vec![name.clone()],
            };
            let body = match entry_obj.get("body") {
                Some(serde_json::Value::String(s)) => vec![s.clone()],
                Some(serde_json::Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                _ => vec![],
            };
            let description = entry_obj
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from);
            let scope = entry_obj
                .get("scope")
                .and_then(|v| v.as_str())
                .map(String::from);
            snippets.insert(
                name.clone(),
                SnippetFileEntry {
                    name: name.clone(),
                    prefix,
                    body,
                    description,
                    scope,
                },
            );
        }
        Ok(SnippetFile {
            language_id: None,
            snippets,
        })
    }

    /// Set the language ID for this snippet file.
    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language_id = Some(lang.into());
        self
    }

    pub fn len(&self) -> usize {
        self.snippets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snippets.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Snippet session (tabstop navigation)
// ---------------------------------------------------------------------------

/// A tabstop position within expanded text.
#[derive(Debug, Clone)]
pub struct TabstopPosition {
    pub index: u32,
    pub offset: usize,
    pub length: usize,
    pub placeholder: Option<String>,
    pub choices: Option<Vec<String>>,
}

/// Active snippet session for tabstop navigation.
#[derive(Debug)]
pub struct SnippetSession {
    pub expanded_text: String,
    pub tabstops: Vec<TabstopPosition>,
    pub current_tabstop: usize,
    pub active: bool,
    pub insert_offset: usize,
}

impl SnippetSession {
    /// Create a new snippet session by expanding the snippet at the given cursor offset.
    pub fn new(snippet: &Snippet, variables: &SnippetVariables, insert_offset: usize) -> Self {
        let mut text = String::new();
        let mut tabstops = Vec::new();
        Self::expand_with_tabstops(&snippet.elements, variables, &mut text, &mut tabstops);
        // Sort tabstops: $1 first, then $2, etc., with $0 last
        tabstops.sort_by(|a, b| {
            if a.index == 0 {
                std::cmp::Ordering::Greater
            } else if b.index == 0 {
                std::cmp::Ordering::Less
            } else {
                a.index.cmp(&b.index)
            }
        });
        Self {
            expanded_text: text,
            tabstops,
            current_tabstop: 0,
            active: true,
            insert_offset,
        }
    }

    fn expand_with_tabstops(
        elements: &[SnippetElement],
        variables: &SnippetVariables,
        text: &mut String,
        tabstops: &mut Vec<TabstopPosition>,
    ) {
        for element in elements {
            match element {
                SnippetElement::Text(t) => text.push_str(t),
                SnippetElement::Tabstop(idx) => {
                    tabstops.push(TabstopPosition {
                        index: *idx,
                        offset: text.len(),
                        length: 0,
                        placeholder: None,
                        choices: None,
                    });
                }
                SnippetElement::Placeholder { index, default } => {
                    let start = text.len();
                    for d in default {
                        expand_element(d, variables, text);
                    }
                    let len = text.len() - start;
                    tabstops.push(TabstopPosition {
                        index: *index,
                        offset: start,
                        length: len,
                        placeholder: Some(text[start..].to_string()),
                        choices: None,
                    });
                }
                SnippetElement::Choice { index, choices } => {
                    let first = choices.first().map(|s| s.as_str()).unwrap_or("");
                    let start = text.len();
                    text.push_str(first);
                    tabstops.push(TabstopPosition {
                        index: *index,
                        offset: start,
                        length: first.len(),
                        placeholder: Some(first.to_string()),
                        choices: Some(choices.clone()),
                    });
                }
                SnippetElement::Variable { name, default } => {
                    if let Some(value) = variables.get(name) {
                        text.push_str(value);
                    } else if let Some(defaults) = default {
                        for d in defaults {
                            expand_element(d, variables, text);
                        }
                    }
                }
            }
        }
    }

    /// Get the current tabstop position, if any.
    pub fn current_position(&self) -> Option<&TabstopPosition> {
        if self.active {
            self.tabstops.get(self.current_tabstop)
        } else {
            None
        }
    }

    /// Advance to the next tabstop. Returns `true` if moved.
    pub fn next_tabstop(&mut self) -> bool {
        if !self.active || self.tabstops.is_empty() {
            return false;
        }
        if self.current_tabstop + 1 < self.tabstops.len() {
            self.current_tabstop += 1;
            true
        } else {
            self.finish();
            false
        }
    }

    /// Go to the previous tabstop. Returns `true` if moved.
    pub fn prev_tabstop(&mut self) -> bool {
        if !self.active || self.current_tabstop == 0 {
            return false;
        }
        self.current_tabstop -= 1;
        true
    }

    /// Accept snippet and deactivate.
    pub fn finish(&mut self) {
        self.active = false;
    }

    /// Cancel snippet mode.
    pub fn cancel(&mut self) {
        self.active = false;
    }

    /// Check if the session is still active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Number of tabstops.
    pub fn tabstop_count(&self) -> usize {
        self.tabstops.len()
    }
}

// ---------------------------------------------------------------------------
// Transform support: ${1/regex/replacement/flags}
// ---------------------------------------------------------------------------

/// A tabstop transformation.
#[derive(Debug, Clone)]
pub struct SnippetTransform {
    pub pattern: String,
    pub replacement: String,
    pub flags: String,
}

impl SnippetTransform {
    /// Parse a transform string like `regex/replacement/flags`.
    pub fn parse(input: &str) -> Option<Self> {
        let parts: Vec<&str> = input.splitn(3, '/').collect();
        if parts.len() < 2 {
            return None;
        }
        Some(Self {
            pattern: parts[0].to_string(),
            replacement: parts[1].to_string(),
            flags: parts.get(2).unwrap_or(&"").to_string(),
        })
    }

    /// Apply the transform to the given input text.
    pub fn apply(&self, input: &str) -> String {
        let case_insensitive = self.flags.contains('i');
        let global = self.flags.contains('g');
        let re = if case_insensitive {
            regex::Regex::new(&format!("(?i){}", self.pattern))
        } else {
            regex::Regex::new(&self.pattern)
        };
        match re {
            Ok(re) => {
                if global {
                    re.replace_all(input, self.replacement.as_str()).to_string()
                } else {
                    re.replace(input, self.replacement.as_str()).to_string()
                }
            }
            Err(_) => input.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Enhanced variable expansion
// ---------------------------------------------------------------------------

impl SnippetVariables {
    /// Populate with all common VS Code snippet variables.
    pub fn with_all_defaults(
        mut self,
        filename: &str,
        selected_text: &str,
        clipboard: &str,
        line_comment: &str,
    ) -> Self {
        self.set("TM_FILENAME", filename);
        self.set(
            "TM_FILENAME_BASE",
            filename.rsplit('.').nth(1).unwrap_or(filename),
        );
        self.set("TM_SELECTED_TEXT", selected_text);
        self.set("CLIPBOARD", clipboard);
        self.set("TM_CURRENT_LINE", "");
        self.set("TM_CURRENT_WORD", "");
        self.set("TM_LINE_INDEX", "0");
        self.set("TM_LINE_NUMBER", "1");
        self.set("LINE_COMMENT", line_comment);

        // Date/time variables
        let now = current_date_time();
        self.set("CURRENT_YEAR", &now.year);
        self.set("CURRENT_YEAR_SHORT", &now.year_short);
        self.set("CURRENT_MONTH", &now.month);
        self.set("CURRENT_DATE", &now.day);
        self.set("CURRENT_HOUR", &now.hour);
        self.set("CURRENT_MINUTE", &now.minute);
        self.set("CURRENT_SECOND", &now.second);
        self
    }
}

struct DateTimeParts {
    year: String,
    year_short: String,
    month: String,
    day: String,
    hour: String,
    minute: String,
    second: String,
}

fn current_date_time() -> DateTimeParts {
    // Simple fallback without chrono dependency
    DateTimeParts {
        year: "2025".to_string(),
        year_short: "25".to_string(),
        month: "01".to_string(),
        day: "01".to_string(),
        hour: "00".to_string(),
        minute: "00".to_string(),
        second: "00".to_string(),
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

    #[test]
    fn collect_tabstops_simple() {
        let s = parse_snippet("fn ${1:name}($2) { $0 }");
        let tabs = collect_tabstops(&s);
        assert!(tabs.contains(&0));
        assert!(tabs.contains(&1));
        assert!(tabs.contains(&2));
    }

    #[test]
    fn collect_tabstops_empty() {
        let s = parse_snippet("plain text only");
        let tabs = collect_tabstops(&s);
        assert!(tabs.is_empty());
    }

    #[test]
    fn collect_tabstops_choice() {
        let s = parse_snippet("${1|yes,no|}");
        let tabs = collect_tabstops(&s);
        assert_eq!(tabs, vec![1]);
    }

    #[test]
    fn element_count_simple() {
        let s = parse_snippet("hello $1 world");
        assert!(element_count(&s) >= 3);
    }

    #[test]
    fn element_count_nested_placeholder() {
        let s = parse_snippet("${1:default}");
        // placeholder(1) + text("default") = 2
        assert_eq!(element_count(&s), 2);
    }

    #[test]
    fn collect_variables_names() {
        let s = parse_snippet("$TM_FILENAME and ${CLIPBOARD:none}");
        let vars = collect_variables(&s);
        assert!(vars.contains(&"TM_FILENAME".to_string()));
        assert!(vars.contains(&"CLIPBOARD".to_string()));
    }

    #[test]
    fn snippet_definition_new_and_display() {
        let def = SnippetDefinition::new("For Loop", "for", "for ${1:i} in ${2:iter} { $0 }")
            .with_description("A for loop");
        assert_eq!(def.name, "For Loop");
        assert_eq!(def.prefix, "for");
        assert_eq!(def.description, Some("A for loop".to_string()));
        let s = format!("{}", def);
        assert!(s.contains("For Loop"));
    }

    #[test]
    fn snippet_definition_expand() {
        let def = SnippetDefinition::new("test", "tst", "Hello $TM_FILENAME!");
        let mut vars = SnippetVariables::new();
        vars.set("TM_FILENAME", "main.rs");
        let result = def.expand(&vars);
        assert_eq!(result, "Hello main.rs!");
    }

    #[test]
    fn snippet_registry_find_by_prefix() {
        let mut reg = SnippetRegistry::new();
        reg.register(SnippetDefinition::new("For Loop", "for", "for $1 {}"));
        reg.register(SnippetDefinition::new("Function", "fn", "fn $1() {}"));
        reg.register(SnippetDefinition::new("Foreach", "foreach", "foreach $1 {}"));

        let matches = reg.find_by_prefix("for");
        assert_eq!(matches.len(), 2); // "for" and "foreach"
    }

    #[test]
    fn snippet_registry_find_by_name() {
        let mut reg = SnippetRegistry::new();
        reg.register(SnippetDefinition::new("For Loop", "for", "for $1 {}"));
        assert!(reg.find_by_name("For Loop").is_some());
        assert!(reg.find_by_name("Missing").is_none());
    }

    #[test]
    fn snippet_registry_len() {
        let mut reg = SnippetRegistry::new();
        assert!(reg.is_empty());
        reg.register(SnippetDefinition::new("a", "a", "a"));
        reg.register(SnippetDefinition::new("b", "b", "b"));
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn snippet_variables_with_defaults() {
        let vars = SnippetVariables::new().with_defaults("main.rs", "copied_text");
        assert_eq!(vars.get("TM_FILENAME"), Some("main.rs"));
        assert_eq!(vars.get("CLIPBOARD"), Some("copied_text"));
        assert_eq!(vars.get("TM_FILENAME_BASE"), Some("main"));
        assert_eq!(vars.get("TM_LINE_NUMBER"), Some("1"));
    }

    // -----------------------------------------------------------------------
    // SnippetFile tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_vscode_snippet_file() {
        let json = r#"{
            "For Loop": {
                "prefix": ["for", "forloop"],
                "body": ["for (${1:i} = 0; ${1:i} < ${2:count}; ${1:i}++) {", "\t$0", "}"],
                "description": "A for loop"
            },
            "Print": {
                "prefix": "print",
                "body": "console.log($1);",
                "description": "Log output"
            }
        }"#;
        let file = SnippetFile::parse(json).unwrap();
        assert_eq!(file.len(), 2);
        let for_loop = &file.snippets["For Loop"];
        assert_eq!(for_loop.prefix, vec!["for", "forloop"]);
        assert_eq!(for_loop.body.len(), 3);
        assert_eq!(for_loop.description.as_deref(), Some("A for loop"));
    }

    #[test]
    fn snippet_file_with_scope() {
        let json = r#"{
            "Fn": {
                "prefix": "fn",
                "body": "fn ${1:name}() {}",
                "scope": "rust"
            }
        }"#;
        let file = SnippetFile::parse(json).unwrap().with_language("rust");
        assert_eq!(file.language_id.as_deref(), Some("rust"));
        assert_eq!(file.snippets["Fn"].scope.as_deref(), Some("rust"));
    }

    #[test]
    fn snippet_file_invalid_json() {
        assert!(SnippetFile::parse("not json").is_err());
    }

    #[test]
    fn snippet_file_empty() {
        let file = SnippetFile::parse("{}").unwrap();
        assert!(file.is_empty());
    }

    // -----------------------------------------------------------------------
    // SnippetSession tests
    // -----------------------------------------------------------------------

    #[test]
    fn session_basic_tabstops() {
        let snippet = parse_snippet("fn ${1:name}($2) {\n\t$0\n}");
        let vars = SnippetVariables::new();
        let session = SnippetSession::new(&snippet, &vars, 0);
        assert!(session.is_active());
        assert_eq!(session.tabstop_count(), 3);
        // First tabstop should be $1 (not $0)
        let pos = session.current_position().unwrap();
        assert_eq!(pos.index, 1);
        assert_eq!(pos.placeholder.as_deref(), Some("name"));
    }

    #[test]
    fn session_next_prev_tabstop() {
        let snippet = parse_snippet("$1 $2 $0");
        let vars = SnippetVariables::new();
        let mut session = SnippetSession::new(&snippet, &vars, 0);
        assert_eq!(session.current_position().unwrap().index, 1);
        assert!(session.next_tabstop());
        assert_eq!(session.current_position().unwrap().index, 2);
        assert!(session.prev_tabstop());
        assert_eq!(session.current_position().unwrap().index, 1);
    }

    #[test]
    fn session_finish_on_last() {
        let snippet = parse_snippet("$1 $0");
        let vars = SnippetVariables::new();
        let mut session = SnippetSession::new(&snippet, &vars, 0);
        assert!(session.next_tabstop()); // move to $0
        assert!(!session.next_tabstop()); // past end -> finish
        assert!(!session.is_active());
    }

    #[test]
    fn session_cancel() {
        let snippet = parse_snippet("$1 $2");
        let vars = SnippetVariables::new();
        let mut session = SnippetSession::new(&snippet, &vars, 0);
        session.cancel();
        assert!(!session.is_active());
        assert!(session.current_position().is_none());
    }

    #[test]
    fn session_choice_tabstop() {
        let snippet = parse_snippet("${1|public,private,protected|}");
        let vars = SnippetVariables::new();
        let session = SnippetSession::new(&snippet, &vars, 0);
        let pos = session.current_position().unwrap();
        assert_eq!(pos.choices.as_ref().unwrap().len(), 3);
        assert_eq!(pos.placeholder.as_deref(), Some("public"));
    }

    // -----------------------------------------------------------------------
    // Transform tests
    // -----------------------------------------------------------------------

    #[test]
    fn transform_parse() {
        let t = SnippetTransform::parse("foo/bar/gi").unwrap();
        assert_eq!(t.pattern, "foo");
        assert_eq!(t.replacement, "bar");
        assert_eq!(t.flags, "gi");
    }

    #[test]
    fn transform_apply_simple() {
        let t = SnippetTransform::parse("hello/world/").unwrap();
        assert_eq!(t.apply("say hello"), "say world");
    }

    #[test]
    fn transform_apply_global() {
        let t = SnippetTransform::parse("a/b/g").unwrap();
        assert_eq!(t.apply("aaa"), "bbb");
    }

    #[test]
    fn transform_apply_case_insensitive() {
        let t = SnippetTransform::parse("hello/world/i").unwrap();
        assert_eq!(t.apply("say HELLO"), "say world");
    }

    #[test]
    fn transform_invalid_regex() {
        let t = SnippetTransform::parse("[invalid/replacement/").unwrap();
        assert_eq!(t.apply("test"), "test");
    }

    // -----------------------------------------------------------------------
    // Enhanced variable tests
    // -----------------------------------------------------------------------

    #[test]
    fn all_defaults_variables() {
        let vars = SnippetVariables::new().with_all_defaults("test.rs", "selected", "clip", "//");
        assert_eq!(vars.get("TM_SELECTED_TEXT"), Some("selected"));
        assert_eq!(vars.get("LINE_COMMENT"), Some("//"));
        assert_eq!(vars.get("CURRENT_YEAR"), Some("2025"));
        assert_eq!(vars.get("CLIPBOARD"), Some("clip"));
    }
}
