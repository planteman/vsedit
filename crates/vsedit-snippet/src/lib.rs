//! Snippet engine.
//!
//! Parses and processes VS Code snippet syntax (TextMate-compatible).
//! Supports tabstops, placeholders, choices, variables, transforms,
//! VS Code snippet file parsing, and snippet insertion sessions.

use std::fmt;
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

    /// Return `true` if this definition has a description.
    pub fn has_description(&self) -> bool {
        self.description.is_some()
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

    /// Remove a snippet by name. Returns `true` if a snippet was removed.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.snippets.len();
        self.snippets.retain(|s| s.name != name);
        self.snippets.len() < before
    }

    /// Return a slice of all registered snippet definitions.
    pub fn all_definitions(&self) -> &[SnippetDefinition] {
        &self.snippets
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
    /// Return the number of variables stored.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Return `true` if no variables are stored.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Return the names of all stored variables.
    pub fn keys(&self) -> Vec<&str> {
        self.values.keys().map(|k| k.as_str()).collect()
    }

    /// Return `true` if a variable with the given name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

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

impl Snippet {
    /// Return `true` if the snippet contains no elements.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

/// Extract only the plain-text elements from a snippet, ignoring tabstops,
/// placeholders, choices, and variables.
pub fn snippet_to_plain_text(snippet: &Snippet) -> String {
    let mut out = String::new();
    for elem in &snippet.elements {
        snippet_plain_text_elem(elem, &mut out);
    }
    out
}

fn snippet_plain_text_elem(elem: &SnippetElement, out: &mut String) {
    match elem {
        SnippetElement::Text(t) => out.push_str(t),
        SnippetElement::Placeholder { default, .. } => {
            for d in default {
                snippet_plain_text_elem(d, out);
            }
        }
        SnippetElement::Variable { default: Some(d), .. } => {
            for dd in d {
                snippet_plain_text_elem(dd, out);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// SnippetScope — language-specific snippet filtering
// ---------------------------------------------------------------------------

/// Associates a snippet with a set of language scopes.
#[derive(Debug, Clone)]
pub struct SnippetScope {
    /// Language identifiers this snippet applies to (e.g., "rust", "python").
    pub languages: Vec<String>,
    /// The snippet definition.
    pub definition: SnippetDefinition,
}

impl SnippetScope {
    /// Create a new scoped snippet.
    pub fn new(definition: SnippetDefinition, languages: Vec<String>) -> Self {
        Self {
            languages,
            definition,
        }
    }

    /// Check if this snippet applies to a given language.
    pub fn applies_to(&self, language: &str) -> bool {
        self.languages.iter().any(|l| l.eq_ignore_ascii_case(language))
    }

    /// Check if this snippet applies globally (no language restriction).
    pub fn is_global(&self) -> bool {
        self.languages.is_empty()
    }
}

/// A registry that supports language-scoped snippets.
#[derive(Debug, Clone)]
pub struct ScopedSnippetRegistry {
    snippets: Vec<SnippetScope>,
}

impl ScopedSnippetRegistry {
    /// Create a new empty scoped registry.
    pub fn new() -> Self {
        Self {
            snippets: Vec::new(),
        }
    }

    /// Register a scoped snippet.
    pub fn register(&mut self, scoped: SnippetScope) {
        self.snippets.push(scoped);
    }

    /// Find all snippets applicable to a language (includes globals).
    pub fn find_for_language(&self, language: &str) -> Vec<&SnippetDefinition> {
        self.snippets
            .iter()
            .filter(|s| s.is_global() || s.applies_to(language))
            .map(|s| &s.definition)
            .collect()
    }

    /// Find snippets matching a prefix for a specific language.
    pub fn find_by_prefix_for_language(&self, prefix: &str, language: &str) -> Vec<&SnippetDefinition> {
        self.find_for_language(language)
            .into_iter()
            .filter(|d| d.prefix.starts_with(prefix))
            .collect()
    }

    /// Number of registered scoped snippets.
    pub fn len(&self) -> usize {
        self.snippets.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.snippets.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Snippet variable resolution
// ---------------------------------------------------------------------------

/// Resolve a variable name to its value given a context.
///
/// Supports common VS Code snippet variables like TM_FILENAME,
/// CURRENT_YEAR, CLIPBOARD, etc.
pub fn resolve_variable(name: &str, vars: &SnippetVariables) -> Option<String> {
    // First check user-provided variables
    if let Some(v) = vars.get(name) {
        return Some(v.to_string());
    }
    // Built-in dynamic variables
    match name {
        "TM_CURRENT_LINE" => Some(String::new()),
        "TM_CURRENT_WORD" => Some(String::new()),
        "TM_LINE_INDEX" => Some("0".to_string()),
        "TM_LINE_NUMBER" => Some("1".to_string()),
        "RANDOM" => Some("000000".to_string()),
        "RANDOM_HEX" => Some("000000".to_string()),
        "UUID" => Some("00000000-0000-0000-0000-000000000000".to_string()),
        _ => None,
    }
}

/// Resolve all variables in a snippet body string using the provided context.
pub fn resolve_all_variables(body: &str, vars: &SnippetVariables) -> String {
    let snippet = parse_snippet(body);
    expand_snippet(&snippet, vars)
}

// ---------------------------------------------------------------------------
// Snippet transformations (case transforms on placeholder defaults)
// ---------------------------------------------------------------------------

/// A case transformation that can be applied to a placeholder's resolved text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseTransform {
    /// Convert text to uppercase.
    Uppercase,
    /// Convert text to lowercase.
    Lowercase,
    /// Capitalize the first character.
    Capitalize,
    /// Convert to camelCase from snake_case.
    CamelCase,
    /// Convert to snake_case from camelCase.
    SnakeCase,
}

impl CaseTransform {
    /// Apply this transformation to a string.
    pub fn apply(&self, input: &str) -> String {
        match self {
            Self::Uppercase => input.to_uppercase(),
            Self::Lowercase => input.to_lowercase(),
            Self::Capitalize => {
                let mut chars = input.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => {
                        let mut s = c.to_uppercase().to_string();
                        s.extend(chars);
                        s
                    }
                }
            }
            Self::CamelCase => {
                input
                    .split('_')
                    .enumerate()
                    .map(|(i, part)| {
                        if i == 0 {
                            part.to_lowercase()
                        } else {
                            let mut chars = part.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(c) => {
                                    let mut s = c.to_uppercase().to_string();
                                    for ch in chars {
                                        s.push(ch.to_lowercase().next().unwrap_or(ch));
                                    }
                                    s
                                }
                            }
                        }
                    })
                    .collect()
            }
            Self::SnakeCase => {
                let mut result = String::new();
                for (i, ch) in input.chars().enumerate() {
                    if ch.is_uppercase() && i > 0 {
                        result.push('_');
                    }
                    result.push(ch.to_lowercase().next().unwrap_or(ch));
                }
                result
            }
        }
    }
}

/// Apply a chain of case transformations in order.
pub fn apply_case_transforms(input: &str, transforms: &[CaseTransform]) -> String {
    let mut result = input.to_string();
    for t in transforms {
        result = t.apply(&result);
    }
    result
}

// ---------------------------------------------------------------------------
// Snippet complexity analysis
// ---------------------------------------------------------------------------

/// Summary statistics about a snippet's structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetComplexity {
    /// Number of plain text segments.
    pub text_count: usize,
    /// Number of tabstops (bare `$N`).
    pub tabstop_count: usize,
    /// Number of placeholders (`${N:default}`).
    pub placeholder_count: usize,
    /// Number of choice elements (`${N|a,b|}`).
    pub choice_count: usize,
    /// Total number of individual choice options across all choices.
    pub total_choice_options: usize,
    /// Number of variable references.
    pub variable_count: usize,
    /// Maximum nesting depth of placeholders.
    pub max_depth: usize,
}

/// Analyse a snippet and return complexity metrics.
pub fn snippet_complexity(snippet: &Snippet) -> SnippetComplexity {
    let mut cx = SnippetComplexity {
        text_count: 0,
        tabstop_count: 0,
        placeholder_count: 0,
        choice_count: 0,
        total_choice_options: 0,
        variable_count: 0,
        max_depth: 0,
    };
    for elem in &snippet.elements {
        complexity_elem(elem, 1, &mut cx);
    }
    cx
}

fn complexity_elem(elem: &SnippetElement, depth: usize, cx: &mut SnippetComplexity) {
    if depth > cx.max_depth {
        cx.max_depth = depth;
    }
    match elem {
        SnippetElement::Text(_) => cx.text_count += 1,
        SnippetElement::Tabstop(_) => cx.tabstop_count += 1,
        SnippetElement::Placeholder { default, .. } => {
            cx.placeholder_count += 1;
            for d in default {
                complexity_elem(d, depth + 1, cx);
            }
        }
        SnippetElement::Choice { choices, .. } => {
            cx.choice_count += 1;
            cx.total_choice_options += choices.len();
        }
        SnippetElement::Variable { default, .. } => {
            cx.variable_count += 1;
            if let Some(defaults) = default {
                for d in defaults {
                    complexity_elem(d, depth + 1, cx);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Snippet placeholder validation
// ---------------------------------------------------------------------------

/// A validation issue found in a snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnippetIssue {
    /// Two placeholders share the same index but have different default text.
    ConflictingDefaults { index: u32, defaults: Vec<String> },
    /// A tabstop index is referenced but never given a placeholder default.
    BareTabstop { index: u32 },
    /// No final tabstop (`$0`) found.
    MissingFinalTabstop,
}

/// Validate a snippet and return any issues found.
pub fn validate_snippet(snippet: &Snippet) -> Vec<SnippetIssue> {
    let mut issues = Vec::new();
    let mut defaults_map: HashMap<u32, Vec<String>> = HashMap::new();
    let mut has_placeholder: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut all_indices: std::collections::HashSet<u32> = std::collections::HashSet::new();

    collect_validation_info(&snippet.elements, &mut defaults_map, &mut has_placeholder, &mut all_indices);

    // Check for conflicting defaults
    for (index, defs) in &defaults_map {
        let mut unique: Vec<String> = defs.clone();
        unique.sort();
        unique.dedup();
        if unique.len() > 1 {
            issues.push(SnippetIssue::ConflictingDefaults {
                index: *index,
                defaults: unique,
            });
        }
    }

    // Check for bare tabstops (indices with no placeholder default)
    for idx in &all_indices {
        if *idx != 0 && !has_placeholder.contains(idx) {
            issues.push(SnippetIssue::BareTabstop { index: *idx });
        }
    }

    // Check for missing $0
    if !all_indices.contains(&0) {
        issues.push(SnippetIssue::MissingFinalTabstop);
    }

    issues.sort_by_key(|i| match i {
        SnippetIssue::ConflictingDefaults { index, .. } => (0, *index),
        SnippetIssue::BareTabstop { index } => (1, *index),
        SnippetIssue::MissingFinalTabstop => (2, 0),
    });
    issues
}

fn collect_validation_info(
    elements: &[SnippetElement],
    defaults_map: &mut HashMap<u32, Vec<String>>,
    has_placeholder: &mut std::collections::HashSet<u32>,
    all_indices: &mut std::collections::HashSet<u32>,
) {
    for elem in elements {
        match elem {
            SnippetElement::Tabstop(idx) => {
                all_indices.insert(*idx);
            }
            SnippetElement::Placeholder { index, default } => {
                all_indices.insert(*index);
                has_placeholder.insert(*index);
                let text = default
                    .iter()
                    .filter_map(|e| if let SnippetElement::Text(t) = e { Some(t.as_str()) } else { None })
                    .collect::<Vec<_>>()
                    .join("");
                defaults_map.entry(*index).or_default().push(text);
                collect_validation_info(default, defaults_map, has_placeholder, all_indices);
            }
            SnippetElement::Choice { index, .. } => {
                all_indices.insert(*index);
                has_placeholder.insert(*index);
            }
            SnippetElement::Variable { default: Some(d), .. } => {
                collect_validation_info(d, defaults_map, has_placeholder, all_indices);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Snippet normalization — reindex placeholders sequentially
// ---------------------------------------------------------------------------

/// Reindex all tabstop/placeholder/choice indices so they are sequential
/// starting from 1, preserving `$0` as the final tabstop.
pub fn normalize_indices(snippet: &Snippet) -> Snippet {
    let existing = collect_tabstops(snippet);
    // Build mapping: old index -> new index, keeping 0 as 0
    let mut mapping: HashMap<u32, u32> = HashMap::new();
    let mut next = 1u32;
    for idx in &existing {
        if *idx == 0 {
            mapping.insert(0, 0);
        } else {
            mapping.insert(*idx, next);
            next += 1;
        }
    }
    let new_elements = remap_elements(&snippet.elements, &mapping);
    Snippet { elements: new_elements }
}

fn remap_elements(elements: &[SnippetElement], mapping: &HashMap<u32, u32>) -> Vec<SnippetElement> {
    elements
        .iter()
        .map(|elem| match elem {
            SnippetElement::Text(t) => SnippetElement::Text(t.clone()),
            SnippetElement::Tabstop(idx) => {
                SnippetElement::Tabstop(*mapping.get(idx).unwrap_or(idx))
            }
            SnippetElement::Placeholder { index, default } => SnippetElement::Placeholder {
                index: *mapping.get(index).unwrap_or(index),
                default: remap_elements(default, mapping),
            },
            SnippetElement::Choice { index, choices } => SnippetElement::Choice {
                index: *mapping.get(index).unwrap_or(index),
                choices: choices.clone(),
            },
            SnippetElement::Variable { name, default } => SnippetElement::Variable {
                name: name.clone(),
                default: default.as_ref().map(|d| remap_elements(d, mapping)),
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Snippet merging — combine two snippets into one
// ---------------------------------------------------------------------------

/// Merge two snippets end-to-end, shifting the second snippet's tabstop
/// indices so they don't collide with the first. The final tabstop (`$0`)
/// is taken from the second snippet only.
pub fn merge_snippets(a: &Snippet, b: &Snippet) -> Snippet {
    let a_tabs = collect_tabstops(a);
    let max_a = a_tabs.iter().filter(|&&i| i != 0).max().copied().unwrap_or(0);

    // Shift b's indices by max_a (except 0 stays 0)
    let mut mapping: HashMap<u32, u32> = HashMap::new();
    let b_tabs = collect_tabstops(b);
    for idx in &b_tabs {
        if *idx == 0 {
            mapping.insert(0, 0);
        } else {
            mapping.insert(*idx, idx + max_a);
        }
    }

    // Remove $0 from a's elements (strip bare tabstop-0)
    let a_elements: Vec<SnippetElement> = a
        .elements
        .iter()
        .filter(|e| !matches!(e, SnippetElement::Tabstop(0)))
        .cloned()
        .collect();

    let b_elements = remap_elements(&b.elements, &mapping);

    let mut elements = a_elements;
    elements.extend(b_elements);
    Snippet { elements }
}

// ---------------------------------------------------------------------------
// Snippet template rendering with named context
// ---------------------------------------------------------------------------

/// Render a snippet body string by resolving variables from a simple
/// key-value context map and expanding all placeholders to their defaults.
pub fn render_template(body: &str, context: &HashMap<String, String>) -> String {
    let mut vars = SnippetVariables::new();
    for (k, v) in context {
        vars.set(k, v);
    }
    let snippet = parse_snippet(body);
    expand_snippet(&snippet, &vars)
}

// ---------------------------------------------------------------------------
// Choice navigation helper
// ---------------------------------------------------------------------------

/// Given a list of choices and a current selection, return the next choice
/// (wrapping around to the beginning).
pub fn next_choice(choices: &[String], current: &str) -> Option<String> {
    if choices.is_empty() {
        return None;
    }
    let pos = choices.iter().position(|c| c == current);
    match pos {
        Some(i) => Some(choices[(i + 1) % choices.len()].clone()),
        None => Some(choices[0].clone()),
    }
}

/// Given a list of choices and a current selection, return the previous
/// choice (wrapping around to the end).
pub fn prev_choice(choices: &[String], current: &str) -> Option<String> {
    if choices.is_empty() {
        return None;
    }
    let pos = choices.iter().position(|c| c == current);
    match pos {
        Some(0) => Some(choices[choices.len() - 1].clone()),
        Some(i) => Some(choices[i - 1].clone()),
        None => Some(choices[choices.len() - 1].clone()),
    }
}

// ---------------------------------------------------------------------------
// SnippetTransformPipeline – chained regex transforms
// ---------------------------------------------------------------------------

/// A pipeline of [`SnippetTransform`] steps applied in sequence.
#[derive(Debug, Clone)]
pub struct SnippetTransformPipeline {
    transforms: Vec<SnippetTransform>,
}

impl SnippetTransformPipeline {
    pub fn new() -> Self {
        Self { transforms: Vec::new() }
    }

    pub fn add(&mut self, transform: SnippetTransform) {
        self.transforms.push(transform);
    }

    /// Apply all transforms in order.
    pub fn apply(&self, input: &str) -> String {
        let mut result = input.to_string();
        for t in &self.transforms {
            result = t.apply(&result);
        }
        result
    }

    pub fn len(&self) -> usize {
        self.transforms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transforms.is_empty()
    }
}

impl Default for SnippetTransformPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SnippetMirrorLinker – synchronized placeholder edits
// ---------------------------------------------------------------------------

/// Tracks linked tabstop mirrors so editing one updates all copies.
#[derive(Debug, Clone)]
pub struct SnippetMirrorLinker {
    /// Map from tabstop index to list of (line, col) positions.
    mirrors: HashMap<u32, Vec<(u32, u32)>>,
}

impl SnippetMirrorLinker {
    pub fn new() -> Self {
        Self { mirrors: HashMap::new() }
    }

    /// Register a mirror position for a tabstop.
    pub fn add_mirror(&mut self, tabstop: u32, line: u32, col: u32) {
        self.mirrors.entry(tabstop).or_default().push((line, col));
    }

    /// Get all mirror positions for a tabstop.
    pub fn mirrors_for(&self, tabstop: u32) -> &[(u32, u32)] {
        self.mirrors.get(&tabstop).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// True if a tabstop has more than one position (i.e. has mirrors).
    pub fn has_mirrors(&self, tabstop: u32) -> bool {
        self.mirrors.get(&tabstop).map_or(false, |v| v.len() > 1)
    }

    /// All tabstop indices that have mirrors.
    pub fn mirrored_tabstops(&self) -> Vec<u32> {
        let mut result: Vec<u32> = self.mirrors
            .iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|(k, _)| *k)
            .collect();
        result.sort();
        result
    }

    /// Total number of tracked tabstops.
    pub fn tabstop_count(&self) -> usize {
        self.mirrors.len()
    }
}

impl Default for SnippetMirrorLinker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SnippetFinalTabstop – handles $0
// ---------------------------------------------------------------------------

/// Handles the final tabstop ($0) position in a snippet session.
#[derive(Debug, Clone)]
pub struct SnippetFinalTabstop {
    pub line: u32,
    pub col: u32,
    pub has_placeholder: bool,
}

impl SnippetFinalTabstop {
    pub fn new(line: u32, col: u32) -> Self {
        Self { line, col, has_placeholder: false }
    }

    pub fn with_placeholder(mut self) -> Self {
        self.has_placeholder = true;
        self
    }
}

impl fmt::Display for SnippetFinalTabstop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "$0 at {}:{}", self.line, self.col)
    }
}

/// Locate the $0 tabstop in a snippet's elements, returning its index in the
/// flat element list, or `None` if absent.
pub fn find_final_tabstop(elements: &[SnippetElement]) -> Option<usize> {
    elements.iter().position(|e| matches!(e, SnippetElement::Tabstop(0)))
}

// ---------------------------------------------------------------------------
// SnippetScopeFilter – language filtering
// ---------------------------------------------------------------------------

/// A filter that restricts a snippet to specific languages.
#[derive(Debug, Clone)]
pub struct SnippetScopeFilter {
    pub languages: Vec<String>,
}

impl SnippetScopeFilter {
    pub fn new(languages: Vec<String>) -> Self {
        Self { languages }
    }

    /// A filter that accepts all languages.
    pub fn all() -> Self {
        Self { languages: Vec::new() }
    }

    /// Check if this snippet is available for the given language.
    pub fn matches(&self, language_id: &str) -> bool {
        if self.languages.is_empty() {
            return true;
        }
        self.languages.iter().any(|l| l == language_id)
    }

    /// True if the filter is unrestricted.
    pub fn is_unrestricted(&self) -> bool {
        self.languages.is_empty()
    }

    /// Parse a comma-separated scope string into a filter.
    pub fn parse(scope: &str) -> Self {
        let languages: Vec<String> = scope
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self { languages }
    }
}

impl fmt::Display for SnippetScopeFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.languages.is_empty() {
            write!(f, "*")
        } else {
            write!(f, "{}", self.languages.join(", "))
        }
    }
}


// ---------------------------------------------------------------------------
// NestedTabstopResolver
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NestedTabstopResolver {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl NestedTabstopResolver {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for NestedTabstopResolver {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for NestedTabstopResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "NestedTabstopResolver({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// SnippetCompletionTrigger
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SnippetCompletionTrigger {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl SnippetCompletionTrigger {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for SnippetCompletionTrigger {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for SnippetCompletionTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "SnippetCompletionTrigger({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// NestedTabstopResolverSnapshot — point-in-time snapshot of NestedTabstopResolver state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NestedTabstopResolverSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl NestedTabstopResolverSnapshot {
    pub fn capture(source: &NestedTabstopResolver, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for NestedTabstopResolverSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// SnippetCompletionTriggerStats — aggregate statistics for SnippetCompletionTrigger
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct SnippetCompletionTriggerStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl SnippetCompletionTriggerStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for SnippetCompletionTriggerStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// NestedTabstopResolverConfig — configuration for NestedTabstopResolver
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NestedTabstopResolverConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl NestedTabstopResolverConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for NestedTabstopResolverConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for NestedTabstopResolverConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// SnippetVariableResolver – resolve built-in and custom variables
// ---------------------------------------------------------------------------

/// Resolves snippet variables like TM_FILENAME, CURRENT_DATE, etc.
#[derive(Debug, Clone, Default)]
pub struct SnippetVariableResolver {
    custom: HashMap<String, String>,
    filename: Option<String>,
    directory: Option<String>,
}

impl SnippetVariableResolver {
    pub fn new() -> Self { Self::default() }

    /// Set the current file context.
    pub fn set_file_context(&mut self, filename: &str, directory: &str) {
        self.filename = Some(filename.to_string());
        self.directory = Some(directory.to_string());
    }

    /// Register a custom variable.
    pub fn register_custom_variable(&mut self, name: &str, value: &str) {
        self.custom.insert(name.to_string(), value.to_string());
    }

    /// Resolve a variable name to its value.
    pub fn resolve(&self, name: &str) -> Option<String> {
        match name {
            "TM_FILENAME" => self.filename.clone(),
            "TM_DIRECTORY" => self.directory.clone(),
            "TM_FILENAME_BASE" => self.filename.as_ref().map(|f| {
                f.rsplit('.').last().unwrap_or(f).to_string()
            }),
            "CURRENT_YEAR" => Some("2025".to_string()),
            "CURRENT_MONTH" => Some("01".to_string()),
            "CURRENT_DATE" => Some("01".to_string()),
            "CURRENT_DAY_NAME" => Some("Monday".to_string()),
            "CLIPBOARD" => Some(String::new()),
            other => self.custom.get(other).cloned(),
        }
    }

    /// Return names of variables that cannot be resolved.
    pub fn unresolved_variables(&self, names: &[&str]) -> Vec<String> {
        names.iter().filter(|n| self.resolve(n).is_none()).map(|n| n.to_string()).collect()
    }
}

// ---------------------------------------------------------------------------
// SnippetChoice – tabstop with multiple choices
// ---------------------------------------------------------------------------

/// Represents a tabstop that offers a list of choices.
#[derive(Debug, Clone)]
pub struct SnippetChoice {
    choices: Vec<String>,
    current_index: usize,
}

impl SnippetChoice {
    pub fn new(choices: Vec<String>) -> Self {
        Self { choices, current_index: 0 }
    }

    pub fn choices(&self) -> &[String] { &self.choices }

    pub fn current_index(&self) -> usize { self.current_index }

    pub fn selected(&self) -> Option<&str> {
        self.choices.get(self.current_index).map(|s| s.as_str())
    }

    pub fn next(&mut self) {
        if !self.choices.is_empty() {
            self.current_index = (self.current_index + 1) % self.choices.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.choices.is_empty() {
            self.current_index = if self.current_index == 0 {
                self.choices.len() - 1
            } else {
                self.current_index - 1
            };
        }
    }

    /// Cycle through choices, wrapping around.
    pub fn cycle(&mut self, forward: bool) {
        if forward { self.next() } else { self.prev() }
    }

    pub fn display_string(&self) -> String {
        format!("[{}]", self.choices.join("|"))
    }
}

// ---------------------------------------------------------------------------
// SnippetMirrorTracker – track mirrored tabstops
// ---------------------------------------------------------------------------

/// Tracks mirrored tabstops: when a primary tabstop changes, mirrors update.
#[derive(Debug, Clone, Default)]
pub struct SnippetMirrorTracker {
    /// Maps primary tabstop index to list of mirror positions.
    mirrors: HashMap<usize, Vec<usize>>,
    /// Current value of each primary tabstop.
    values: HashMap<usize, String>,
}

impl SnippetMirrorTracker {
    pub fn new() -> Self { Self::default() }

    /// Register a mirror: `mirror_pos` mirrors the value at `primary`.
    pub fn register_mirror(&mut self, primary: usize, mirror_pos: usize) {
        self.mirrors.entry(primary).or_default().push(mirror_pos);
    }

    /// Get all mirror positions for a primary tabstop.
    pub fn get_mirrors(&self, primary: usize) -> Vec<usize> {
        self.mirrors.get(&primary).cloned().unwrap_or_default()
    }

    /// Update the primary tabstop value.
    pub fn update_primary(&mut self, primary: usize, value: &str) {
        self.values.insert(primary, value.to_string());
    }

    /// Get the current mirrored value for a primary tabstop.
    pub fn mirrored_value(&self, primary: usize) -> Option<&str> {
        self.values.get(&primary).map(|s| s.as_str())
    }

    /// Return all mirror values as a map of primary -> value.
    pub fn mirrored_values(&self) -> &HashMap<usize, String> {
        &self.values
    }

    /// Count total mirrors across all primaries.
    pub fn mirror_count(&self) -> usize {
        self.mirrors.values().map(|v| v.len()).sum()
    }
}


/// Code snippet configuration manager.
#[derive(Debug, Clone)]
pub struct SnippetConfig {
    entries: Vec<SnippetEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single code snippet entry.
#[derive(Debug, Clone, PartialEq)]
pub struct SnippetEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl SnippetEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl SnippetConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: SnippetEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&SnippetEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut SnippetEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&SnippetEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&SnippetEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&SnippetEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<SnippetEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
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

    // -----------------------------------------------------------------------
    // New functionality tests
    // -----------------------------------------------------------------------

    #[test]
    fn snippet_variables_len_and_is_empty() {
        let mut vars = SnippetVariables::new();
        assert!(vars.is_empty());
        assert_eq!(vars.len(), 0);
        vars.set("A", "1");
        vars.set("B", "2");
        assert_eq!(vars.len(), 2);
        assert!(!vars.is_empty());
    }

    #[test]
    fn snippet_variables_keys_and_contains() {
        let mut vars = SnippetVariables::new();
        vars.set("FOO", "bar");
        vars.set("BAZ", "qux");
        assert!(vars.contains("FOO"));
        assert!(vars.contains("BAZ"));
        assert!(!vars.contains("MISSING"));
        let keys = vars.keys();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"FOO"));
        assert!(keys.contains(&"BAZ"));
    }

    #[test]
    fn snippet_registry_remove() {
        let mut reg = SnippetRegistry::new();
        reg.register(SnippetDefinition::new("Alpha", "a", "a"));
        reg.register(SnippetDefinition::new("Beta", "b", "b"));
        assert_eq!(reg.len(), 2);
        assert!(reg.remove("Alpha"));
        assert_eq!(reg.len(), 1);
        assert!(reg.find_by_name("Alpha").is_none());
        assert!(!reg.remove("NonExistent"));
    }

    #[test]
    fn snippet_registry_all_definitions() {
        let mut reg = SnippetRegistry::new();
        reg.register(SnippetDefinition::new("X", "x", "x body"));
        reg.register(SnippetDefinition::new("Y", "y", "y body"));
        let defs = reg.all_definitions();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].name, "X");
        assert_eq!(defs[1].name, "Y");
    }

    #[test]
    fn snippet_definition_has_description() {
        let with = SnippetDefinition::new("A", "a", "a").with_description("desc");
        let without = SnippetDefinition::new("B", "b", "b");
        assert!(with.has_description());
        assert!(!without.has_description());
    }

    #[test]
    fn snippet_is_empty() {
        let empty = parse_snippet("");
        assert!(empty.is_empty());
        let nonempty = parse_snippet("hello");
        assert!(!nonempty.is_empty());
    }

    #[test]
    fn snippet_to_plain_text_basic() {
        let s = parse_snippet("hello $1 world ${2:default} end");
        let plain = snippet_to_plain_text(&s);
        assert_eq!(plain, "hello  world default end");
    }

    // -- SnippetScope tests --------------------------------------------------

    #[test]
    fn snippet_scope_applies_to_language() {
        let def = SnippetDefinition::new("test", "tst", "hello $1");
        let scope = SnippetScope::new(def, vec!["rust".into(), "python".into()]);
        assert!(scope.applies_to("rust"));
        assert!(scope.applies_to("Rust"));
        assert!(!scope.applies_to("go"));
        assert!(!scope.is_global());
    }

    #[test]
    fn scoped_registry_filters_by_language() {
        let mut reg = ScopedSnippetRegistry::new();
        reg.register(SnippetScope::new(
            SnippetDefinition::new("rs_fn", "fn", "fn $1() {}"),
            vec!["rust".into()],
        ));
        reg.register(SnippetScope::new(
            SnippetDefinition::new("py_def", "def", "def $1(): pass"),
            vec!["python".into()],
        ));
        reg.register(SnippetScope::new(
            SnippetDefinition::new("comment", "//", "// $1"),
            vec![],
        ));
        assert_eq!(reg.find_for_language("rust").len(), 2); // rs_fn + global
        assert_eq!(reg.find_for_language("python").len(), 2); // py_def + global
        assert_eq!(reg.find_for_language("go").len(), 1); // global only
    }

    // -- Snippet transforms --------------------------------------------------

    #[test]
    fn transform_uppercase_lowercase() {
        assert_eq!(CaseTransform::Uppercase.apply("hello"), "HELLO");
        assert_eq!(CaseTransform::Lowercase.apply("HELLO"), "hello");
        assert_eq!(CaseTransform::Capitalize.apply("hello"), "Hello");
    }

    #[test]
    fn transform_camel_and_snake() {
        assert_eq!(CaseTransform::CamelCase.apply("my_var_name"), "myVarName");
        assert_eq!(CaseTransform::SnakeCase.apply("myVarName"), "my_var_name");
    }

    #[test]
    fn apply_chained_transforms() {
        let chain = vec![CaseTransform::SnakeCase, CaseTransform::Uppercase];
        assert_eq!(apply_case_transforms("myVar", &chain), "MY_VAR");
    }

    // -- Variable resolution -------------------------------------------------

    #[test]
    fn resolve_variable_builtin() {
        let vars = SnippetVariables::new();
        assert_eq!(resolve_variable("TM_LINE_NUMBER", &vars), Some("1".into()));
        assert!(resolve_variable("NONEXISTENT", &vars).is_none());
    }

    #[test]
    fn resolve_variable_user_provided() {
        let mut vars = SnippetVariables::new();
        vars.set("MY_VAR", "custom_value");
        assert_eq!(resolve_variable("MY_VAR", &vars), Some("custom_value".into()));
    }

    // -----------------------------------------------------------------------
    // Snippet complexity analysis tests
    // -----------------------------------------------------------------------

    #[test]
    fn complexity_counts_all_element_types() {
        let s = parse_snippet("hello ${1:name} $2 ${3|a,b,c|} $TM_FILENAME end");
        let cx = snippet_complexity(&s);
        assert_eq!(cx.text_count, 6); // "hello ", "name", " ", " ", " ", " end"
        assert_eq!(cx.placeholder_count, 1);
        assert_eq!(cx.tabstop_count, 1);
        assert_eq!(cx.choice_count, 1);
        assert_eq!(cx.total_choice_options, 3);
        assert_eq!(cx.variable_count, 1);
    }

    #[test]
    fn complexity_nested_depth() {
        // ${1:default} has depth 2 (placeholder at 1, text child at 2)
        let s = parse_snippet("${1:inner}");
        let cx = snippet_complexity(&s);
        assert_eq!(cx.max_depth, 2);
    }

    // -----------------------------------------------------------------------
    // Snippet validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn validate_detects_missing_final_tabstop() {
        let s = parse_snippet("${1:name} ${2:value}");
        let issues = validate_snippet(&s);
        assert!(issues.iter().any(|i| matches!(i, SnippetIssue::MissingFinalTabstop)));
    }

    #[test]
    fn validate_detects_bare_tabstop() {
        let s = parse_snippet("$1 ${2:has_default} $0");
        let issues = validate_snippet(&s);
        assert!(issues.iter().any(|i| matches!(i, SnippetIssue::BareTabstop { index: 1 })));
    }

    #[test]
    fn validate_clean_snippet_has_no_bare_or_missing() {
        let s = parse_snippet("${1:name} $0");
        let issues = validate_snippet(&s);
        assert!(!issues.iter().any(|i| matches!(i, SnippetIssue::MissingFinalTabstop)));
        assert!(!issues.iter().any(|i| matches!(i, SnippetIssue::BareTabstop { .. })));
    }

    // -----------------------------------------------------------------------
    // Snippet normalization tests
    // -----------------------------------------------------------------------

    #[test]
    fn normalize_reindexes_gaps() {
        let s = parse_snippet("${3:first} ${7:second} $0");
        let norm = normalize_indices(&s);
        let tabs = collect_tabstops(&norm);
        assert_eq!(tabs, vec![0, 1, 2]);
        // Verify placeholder defaults are preserved
        if let SnippetElement::Placeholder { index, .. } = &norm.elements[0] {
            assert_eq!(*index, 1);
        } else {
            panic!("expected placeholder");
        }
    }

    // -----------------------------------------------------------------------
    // Snippet merging tests
    // -----------------------------------------------------------------------

    #[test]
    fn merge_shifts_second_snippet_indices() {
        let a = parse_snippet("${1:first} $0");
        let b = parse_snippet("${1:second} $0");
        let merged = merge_snippets(&a, &b);
        let tabs = collect_tabstops(&merged);
        // a had $1, b's $1 becomes $2, and b's $0 stays $0
        assert!(tabs.contains(&1));
        assert!(tabs.contains(&2));
        assert!(tabs.contains(&0));
    }

    // -----------------------------------------------------------------------
    // Render template tests
    // -----------------------------------------------------------------------

    #[test]
    fn render_template_resolves_variables() {
        let mut ctx = HashMap::new();
        ctx.insert("USER".to_string(), "alice".to_string());
        ctx.insert("PROJECT".to_string(), "vsedit".to_string());
        let result = render_template("Author: $USER, Project: $PROJECT", &ctx);
        assert_eq!(result, "Author: alice, Project: vsedit");
    }

    // -----------------------------------------------------------------------
    // Choice navigation tests
    // -----------------------------------------------------------------------

    #[test]
    fn choice_navigation_next_and_prev() {
        let choices: Vec<String> = vec!["public".into(), "private".into(), "protected".into()];
        assert_eq!(next_choice(&choices, "public"), Some("private".into()));
        assert_eq!(next_choice(&choices, "protected"), Some("public".into())); // wraps
        assert_eq!(prev_choice(&choices, "public"), Some("protected".into())); // wraps
        assert_eq!(prev_choice(&choices, "private"), Some("public".into()));
    }

    #[test]
    fn choice_navigation_unknown_current() {
        let choices: Vec<String> = vec!["a".into(), "b".into()];
        assert_eq!(next_choice(&choices, "unknown"), Some("a".into()));
        assert_eq!(prev_choice(&choices, "unknown"), Some("b".into()));
    }

    #[test]
    fn choice_navigation_empty() {
        let choices: Vec<String> = vec![];
        assert_eq!(next_choice(&choices, "x"), None);
        assert_eq!(prev_choice(&choices, "x"), None);
    }

    // -- SnippetTransformPipeline tests --

    #[test]
    fn transform_pipeline_single() {
        let t = SnippetTransform::parse("foo/bar/g").unwrap();
        let mut pipe = SnippetTransformPipeline::new();
        pipe.add(t);
        assert_eq!(pipe.apply("foo baz foo"), "bar baz bar");
        assert_eq!(pipe.len(), 1);
    }

    #[test]
    fn transform_pipeline_chained() {
        let mut pipe = SnippetTransformPipeline::new();
        pipe.add(SnippetTransform::parse("hello/world/g").unwrap());
        pipe.add(SnippetTransform::parse("world/earth/g").unwrap());
        assert_eq!(pipe.apply("hello world"), "earth earth");
        assert_eq!(pipe.len(), 2);
    }

    #[test]
    fn transform_pipeline_empty() {
        let pipe = SnippetTransformPipeline::default();
        assert!(pipe.is_empty());
        assert_eq!(pipe.apply("unchanged"), "unchanged");
    }

    // -- SnippetMirrorLinker tests --

    #[test]
    fn mirror_linker_basic() {
        let mut linker = SnippetMirrorLinker::new();
        linker.add_mirror(1, 0, 5);
        linker.add_mirror(1, 2, 10);
        linker.add_mirror(2, 1, 0);
        assert!(linker.has_mirrors(1));
        assert!(!linker.has_mirrors(2));
        assert_eq!(linker.mirrors_for(1).len(), 2);
        assert_eq!(linker.mirrored_tabstops(), vec![1]);
    }

    #[test]
    fn mirror_linker_empty() {
        let linker = SnippetMirrorLinker::default();
        assert_eq!(linker.tabstop_count(), 0);
        assert!(linker.mirrors_for(1).is_empty());
    }

    // -- SnippetFinalTabstop tests --

    #[test]
    fn final_tabstop_display() {
        let ft = SnippetFinalTabstop::new(5, 10);
        assert_eq!(format!("{}", ft), "$0 at 5:10");
        assert!(!ft.has_placeholder);
    }

    #[test]
    fn final_tabstop_with_placeholder() {
        let ft = SnippetFinalTabstop::new(0, 0).with_placeholder();
        assert!(ft.has_placeholder);
    }

    #[test]
    fn find_final_tabstop_present() {
        let elements = vec![
            SnippetElement::Text("hello ".into()),
            SnippetElement::Tabstop(1),
            SnippetElement::Tabstop(0),
        ];
        assert_eq!(find_final_tabstop(&elements), Some(2));
    }

    #[test]
    fn find_final_tabstop_absent() {
        let elements = vec![SnippetElement::Text("hello".into())];
        assert_eq!(find_final_tabstop(&elements), None);
    }

    // -- SnippetScopeFilter tests --

    #[test]
    fn scope_filter_matches() {
        let f = SnippetScopeFilter::new(vec!["rust".into(), "go".into()]);
        assert!(f.matches("rust"));
        assert!(f.matches("go"));
        assert!(!f.matches("python"));
        assert!(!f.is_unrestricted());
    }

    #[test]
    fn scope_filter_all() {
        let f = SnippetScopeFilter::all();
        assert!(f.matches("anything"));
        assert!(f.is_unrestricted());
        assert_eq!(format!("{}", f), "*");
    }

    #[test]
    fn scope_filter_parse() {
        let f = SnippetScopeFilter::parse("rust, python, go");
        assert_eq!(f.languages.len(), 3);
        assert!(f.matches("python"));
        assert_eq!(format!("{}", f), "rust, python, go");
    }

    #[test] fn nestedTabstopResolver_new() { let s = NestedTabstopResolver::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn nestedTabstopResolver_add() { let mut s = NestedTabstopResolver::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn nestedTabstopResolver_remove() { let mut s = NestedTabstopResolver::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn nestedTabstopResolver_config() { let mut s = NestedTabstopResolver::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn nestedTabstopResolver_nav() { let mut s = NestedTabstopResolver::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn nestedTabstopResolver_filter() { let mut s = NestedTabstopResolver::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn nestedTabstopResolver_display() { assert!(format!("{}", NestedTabstopResolver::new()).contains("NestedTabstopResolver")); }
    #[test] fn snippetCompletionTrigger_new() { let s = SnippetCompletionTrigger::new(); assert!(s.is_empty()); }
    #[test] fn snippetCompletionTrigger_add() { let mut s = SnippetCompletionTrigger::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn snippetCompletionTrigger_active() { let mut s = SnippetCompletionTrigger::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn snippetCompletionTrigger_error() { let mut s = SnippetCompletionTrigger::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn snippetCompletionTrigger_rm_group() { let mut s = SnippetCompletionTrigger::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn snippetCompletionTrigger_display() { assert!(format!("{}", SnippetCompletionTrigger::new()).contains("SnippetCompletionTrigger")); }


    #[test] fn nestedTabstopResolver_snap_capture() {
        let s = NestedTabstopResolver::new();
        let snap = NestedTabstopResolverSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn nestedTabstopResolver_snap_stale() {
        let s = NestedTabstopResolver::new();
        let snap = NestedTabstopResolverSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn nestedTabstopResolver_snap_diff() {
        let s = NestedTabstopResolver::new();
        let s1v = NestedTabstopResolverSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn nestedTabstopResolver_snap_display() {
        let s = NestedTabstopResolver::new();
        let snap = NestedTabstopResolverSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn snippetCompletionTrigger_stats_record() {
        let mut st = SnippetCompletionTriggerStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn snippetCompletionTrigger_stats_hit_ratio() {
        let mut st = SnippetCompletionTriggerStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn snippetCompletionTrigger_stats_merge() {
        let mut a = SnippetCompletionTriggerStats::new();
        a.total_adds = 5;
        let mut b = SnippetCompletionTriggerStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn snippetCompletionTrigger_stats_display() {
        let st = SnippetCompletionTriggerStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn nestedTabstopResolver_config_default() {
        let c = NestedTabstopResolverConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn nestedTabstopResolver_config_builder() {
        let c = NestedTabstopResolverConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn nestedTabstopResolver_config_labels() {
        let mut c = NestedTabstopResolverConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn nestedTabstopResolver_config_cleanup_threshold() {
        let c = NestedTabstopResolverConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn nestedTabstopResolver_config_display() {
        assert!(format!("{}", NestedTabstopResolverConfig::new()).contains("Config"));
    }
    #[test] fn snippetCompletionTrigger_stats_peaks() {
        let mut st = SnippetCompletionTriggerStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- SnippetVariableResolver -------------------------------------------

    #[test]
    fn resolve_builtin_tm_filename() {
        let mut r = SnippetVariableResolver::new();
        r.set_file_context("main.rs", "/src");
        assert_eq!(r.resolve("TM_FILENAME"), Some("main.rs".to_string()));
        assert_eq!(r.resolve("TM_DIRECTORY"), Some("/src".to_string()));
    }

    #[test]
    fn resolve_custom_variable() {
        let mut r = SnippetVariableResolver::new();
        r.register_custom_variable("AUTHOR", "Alice");
        assert_eq!(r.resolve("AUTHOR"), Some("Alice".to_string()));
    }

    #[test]
    fn resolve_unresolved() {
        let r = SnippetVariableResolver::new();
        let unresolved = r.unresolved_variables(&["TM_FILENAME", "CURRENT_YEAR", "UNKNOWN"]);
        assert!(unresolved.contains(&"TM_FILENAME".to_string()));
        assert!(unresolved.contains(&"UNKNOWN".to_string()));
        assert!(!unresolved.contains(&"CURRENT_YEAR".to_string()));
    }

    #[test]
    fn resolve_current_year() {
        let r = SnippetVariableResolver::new();
        assert!(r.resolve("CURRENT_YEAR").is_some());
    }

    // -- SnippetChoice ------------------------------------------------------

    #[test]
    fn choice_basic() {
        let c = SnippetChoice::new(vec!["a".into(), "b".into(), "c".into()]);
        assert_eq!(c.selected(), Some("a"));
        assert_eq!(c.current_index(), 0);
    }

    #[test]
    fn choice_next_wraps() {
        let mut c = SnippetChoice::new(vec!["x".into(), "y".into()]);
        c.next();
        assert_eq!(c.selected(), Some("y"));
        c.next();
        assert_eq!(c.selected(), Some("x"));
    }

    #[test]
    fn choice_prev_wraps() {
        let mut c = SnippetChoice::new(vec!["x".into(), "y".into()]);
        c.prev();
        assert_eq!(c.selected(), Some("y"));
    }

    #[test]
    fn choice_display_string() {
        let c = SnippetChoice::new(vec!["a".into(), "b".into()]);
        assert_eq!(c.display_string(), "[a|b]");
    }

    // -- SnippetMirrorTracker -----------------------------------------------

    #[test]
    fn mirror_register_and_get() {
        let mut m = SnippetMirrorTracker::new();
        m.register_mirror(1, 10);
        m.register_mirror(1, 20);
        assert_eq!(m.get_mirrors(1), vec![10, 20]);
        assert_eq!(m.get_mirrors(2), Vec::<usize>::new());
    }

    #[test]
    fn mirror_update_primary() {
        let mut m = SnippetMirrorTracker::new();
        m.register_mirror(1, 10);
        m.update_primary(1, "hello");
        assert_eq!(m.mirrored_value(1), Some("hello"));
    }

    #[test]
    fn mirror_count() {
        let mut m = SnippetMirrorTracker::new();
        m.register_mirror(1, 10);
        m.register_mirror(1, 20);
        m.register_mirror(2, 30);
        assert_eq!(m.mirror_count(), 3);
    }

    #[test]
    fn mirror_values_map() {
        let mut m = SnippetMirrorTracker::new();
        m.update_primary(1, "a");
        m.update_primary(2, "b");
        assert_eq!(m.mirrored_values().len(), 2);
    }


    #[test]
    fn snippet_entry_creation() {
        let e = SnippetEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn snippet_entry_with_priority() {
        let e = SnippetEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn snippet_entry_metadata() {
        let e = SnippetEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn snippet_entry_remove_meta() {
        let mut e = SnippetEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn snippet_entry_activate_deactivate() {
        let mut e = SnippetEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn snippet_config_add_sorted() {
        let mut c = SnippetConfig::new(10);
        c.add(SnippetEntry::new("lo", "Lo").with_priority(1));
        c.add(SnippetEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn snippet_config_capacity() {
        let mut c = SnippetConfig::new(1);
        assert!(c.add(SnippetEntry::new("a", "A")));
        assert!(!c.add(SnippetEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn snippet_config_remove() {
        let mut c = SnippetConfig::new(10);
        c.add(SnippetEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn snippet_config_get() {
        let mut c = SnippetConfig::new(10);
        c.add(SnippetEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn snippet_config_active_entries() {
        let mut c = SnippetConfig::new(10);
        c.add(SnippetEntry::new("a", "A"));
        c.add(SnippetEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn snippet_config_enable_disable() {
        let mut c = SnippetConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn snippet_config_clear() {
        let mut c = SnippetConfig::new(10);
        c.add(SnippetEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn snippet_config_find_by_label() {
        let mut c = SnippetConfig::new(10);
        c.add(SnippetEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn snippet_config_top_n() {
        let mut c = SnippetConfig::new(10);
        c.add(SnippetEntry::new("a", "A").with_priority(1));
        c.add(SnippetEntry::new("b", "B").with_priority(2));
        c.add(SnippetEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn snippet_config_deactivate_activate_all() {
        let mut c = SnippetConfig::new(10);
        c.add(SnippetEntry::new("a", "A"));
        c.add(SnippetEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn snippet_config_highest_priority() {
        let mut c = SnippetConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(SnippetEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn snippet_config_contains() {
        let mut c = SnippetConfig::new(10);
        c.add(SnippetEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn snippet_config_labels() {
        let mut c = SnippetConfig::new(10);
        c.add(SnippetEntry::new("a", "Alpha"));
        c.add(SnippetEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn snippet_config_drain_inactive() {
        let mut c = SnippetConfig::new(10);
        c.add(SnippetEntry::new("a", "A"));
        c.add(SnippetEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }

}
