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


// ---------------------------------------------------------------------------
// xa_ extended helpers for snippet
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaSnippetRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaSnippetRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaSnippetCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaSnippetCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaSnippetCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 161
// ---------------------------------------------------------------------------

/// Generic object pool `Xc161Pool<T>`.
pub struct Xc161Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc161Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc161PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc161Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc161PoolStats {
        Xc161PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc161Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc161Scheduler`.
pub struct Xc161Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc161Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc161Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_161 hash for the given byte slice.
pub fn xc_161_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_161 convention.
pub fn xc_161_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_50 deepening: state machine + event bus ---

/// States for the Xd50 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd50State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd50State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd50Transition {
    pub from: Xd50State,
    pub to: Xd50State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd50StateMachine {
    current: Xd50State,
    history: Vec<Xd50Transition>,
    step_counter: usize,
}

impl Xd50StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd50State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd50State {
        self.current
    }

    pub fn history(&self) -> &[Xd50Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd50State) -> Result<Xd50State, String> {
        let allowed = match (self.current, target) {
            (Xd50State::Idle, Xd50State::Running) => true,
            (Xd50State::Running, Xd50State::Paused) => true,
            (Xd50State::Running, Xd50State::Done) => true,
            (Xd50State::Paused, Xd50State::Running) => true,
            (Xd50State::Paused, Xd50State::Done) => true,
            (Xd50State::Done, Xd50State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_50: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd50Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd50SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd50State> {
        let prefix = "Xd50SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd50State::Idle),
            "Running" => Some(Xd50State::Running),
            "Paused" => Some(Xd50State::Paused),
            "Done" => Some(Xd50State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd50State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd50 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd50Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd50Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd50HandlerFn = Box<dyn Fn(&Xd50Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd50EventBus {
    handlers: Vec<(usize, Option<String>, Xd50HandlerFn)>,
    next_id: usize,
    published: Vec<Xd50Event>,
}

impl Xd50EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd50Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd50Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd50Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd50Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #48
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf48Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf48TrieNode {
    children: std::collections::HashMap<char, Xf48TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf48Trie {
    root: Xf48TrieNode,
    count: usize,
}

impl Xf48Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf48TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf48TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf48TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf48BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf48BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 160).
pub struct Xh160SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh160SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 202 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 160).
pub struct Xh160BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh160BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 160).
pub struct Xi160Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi160Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi160Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi160Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 160).
pub struct Xi160IntervalTree {
    xi_intervals: Vec<Xi160Interval>,
}

impl Xi160IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi160Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi160Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi160Interval) -> Vec<&Xi160Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi160Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi160Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi160Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi160Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi160Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi160Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 161) ---

/// Disjoint set / union-find for crate 161.
pub struct Xj161UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj161UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ161_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 161.
pub struct Xj161BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj161BTreeNode<K, V>>>,
    len: usize,
}

struct Xj161BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj161BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj161BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ161_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ161_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj161BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj161BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj161BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj161BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_160 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk160SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk160SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk160DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk160DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_161).
#[derive(Debug, Clone)]
pub struct Xl161Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl161Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_161).
#[derive(Debug, Clone)]
pub struct Xl161SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl161SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm161MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm161MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm161Tokenizer {
    text: String,
}

impl Xm161Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 160.
pub struct Xn160Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn160Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 160 -----

#[derive(Debug, Clone)]
struct Xn160AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn160AvlNode<K, V>>>,
    right: Option<Box<Xn160AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 160.
#[derive(Debug, Clone)]
pub struct Xn160AVL<K, V> {
    root: Option<Box<Xn160AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn160AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn160AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn160AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn160AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn160AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn160AvlNode<K, V>>) -> Box<Xn160AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn160AvlNode<K, V>>) -> Box<Xn160AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn160AvlNode<K, V>>) -> Box<Xn160AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn160AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn160AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn160AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn160AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn160AvlNode<K, V>>) -> &Xn160AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn160AvlNode<K, V>>) -> (Box<Xn160AvlNode<K, V>>, Option<Box<Xn160AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn160AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn160AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn160AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn160AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn160AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn160AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn160AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo160RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo160Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo160RBNode<K, V> {
    key: K,
    value: V,
    color: Xo160Color,
    left: Option<Box<Xo160RBNode<K, V>>>,
    right: Option<Box<Xo160RBNode<K, V>>>,
}

/// A red-black tree map for crate 160.
#[derive(Debug, Clone)]
pub struct Xo160RedBlack<K, V> {
    root: Option<Box<Xo160RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo160RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo160Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo160RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo160RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo160RBNode {
                    key, value, color: Xo160Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo160RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo160Color::Red)
    }

    fn xo_balance(mut h: Box<Xo160RBNode<K, V>>) -> Box<Xo160RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo160Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo160RBNode<K, V>>) -> Box<Xo160RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo160Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo160RBNode<K, V>>) -> Box<Xo160RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo160Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo160RBNode<K, V>>) {
        h.color = Xo160Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo160Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo160Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo160Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo160RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo160RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo160RBNode<K, V>) -> (K, V, Option<Box<Xo160RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo160RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo160Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo160RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo160ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 160.
#[derive(Debug, Clone)]
pub struct Xo160ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo160ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo160#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo160#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 160).
#[derive(Debug)]
pub struct Xp160SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp160Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp160Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp160Node<K, V>>>,
    xp_right: Option<Box<Xp160Node<K, V>>>,
}

impl<K: Ord, V> Xp160Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp160SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp160SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp160Node<K, V>>>, key: &K) -> Option<Box<Xp160Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp160Node<K, V>>) -> Box<Xp160Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp160Node<K, V>>) -> Box<Xp160Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp160Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp160Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp160Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
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


    // xa_ extended tests for snippet
    #[test]
    fn xa_snippet_ring_new() {
        let rb = super::XaSnippetRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_snippet_ring_push_len() {
        let mut rb = super::XaSnippetRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_snippet_ring_wrap() {
        let mut rb = super::XaSnippetRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_snippet_ring_mean_empty() {
        let rb = super::XaSnippetRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_snippet_ring_mean_values() {
        let mut rb = super::XaSnippetRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_snippet_ring_min_max() {
        let mut rb = super::XaSnippetRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_snippet_ring_iter() {
        let mut rb = super::XaSnippetRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_snippet_counter_new() {
        let c = super::XaSnippetCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_snippet_counter_inc() {
        let mut c = super::XaSnippetCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_snippet_counter_inc_by() {
        let mut c = super::XaSnippetCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_snippet_counter_reset() {
        let mut c = super::XaSnippetCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_snippet_counter_clear() {
        let mut c = super::XaSnippetCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_snippet_counter_default() {
        let c = super::XaSnippetCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 161 ----

    #[test]
    fn xc_161_pool_new_empty() {
        let pool: super::Xc161Pool<i32> = super::Xc161Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_161_pool_release_acquire() {
        let mut pool = super::Xc161Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_161_pool_acquire_empty() {
        let mut pool: super::Xc161Pool<i32> = super::Xc161Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_161_pool_full() {
        let mut pool = super::Xc161Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_161_pool_drain() {
        let mut pool = super::Xc161Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_161_pool_stats() {
        let mut pool = super::Xc161Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_161_pool_clear() {
        let mut pool = super::Xc161Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_161_pool_shrink() {
        let mut pool = super::Xc161Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_161_pool_default() {
        let pool: super::Xc161Pool<String> = super::Xc161Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_161_pool_extend() {
        let mut pool = super::Xc161Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_161_pool_retain() {
        let mut pool = super::Xc161Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_161_scheduler_round_robin() {
        let mut sched = super::Xc161Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_161_scheduler_empty() {
        let mut sched = super::Xc161Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_161_scheduler_reset() {
        let mut sched = super::Xc161Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_161_scheduler_add_remove() {
        let mut sched = super::Xc161Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_161_scheduler_targets() {
        let sched = super::Xc161Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_161_hash_empty() {
        assert_eq!(super::xc_161_hash(b""), 5381);
    }

    #[test]
    fn xc_161_hash_data() {
        let h = super::xc_161_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_161_hash(b"hello"), h);
    }

    #[test]
    fn xc_161_reverse_str() {
        assert_eq!(super::xc_161_reverse("abc"), "cba");
        assert_eq!(super::xc_161_reverse(""), "");
    }


    // --- xd_50 deepening tests ---

    #[test]
    fn xd_50_sm_initial_state() {
        let sm = Xd50StateMachine::new();
        assert_eq!(sm.current_state(), Xd50State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_50_sm_valid_idle_to_running() {
        let mut sm = Xd50StateMachine::new();
        assert!(sm.transition(Xd50State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd50State::Running);
    }

    #[test]
    fn xd_50_sm_valid_running_to_paused() {
        let mut sm = Xd50StateMachine::new();
        sm.transition(Xd50State::Running).unwrap();
        assert!(sm.transition(Xd50State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd50State::Paused);
    }

    #[test]
    fn xd_50_sm_valid_running_to_done() {
        let mut sm = Xd50StateMachine::new();
        sm.transition(Xd50State::Running).unwrap();
        assert!(sm.transition(Xd50State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd50State::Done);
    }

    #[test]
    fn xd_50_sm_valid_paused_to_running() {
        let mut sm = Xd50StateMachine::new();
        sm.transition(Xd50State::Running).unwrap();
        sm.transition(Xd50State::Paused).unwrap();
        assert!(sm.transition(Xd50State::Running).is_ok());
    }

    #[test]
    fn xd_50_sm_valid_done_to_idle() {
        let mut sm = Xd50StateMachine::new();
        sm.transition(Xd50State::Running).unwrap();
        sm.transition(Xd50State::Done).unwrap();
        assert!(sm.transition(Xd50State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd50State::Idle);
    }

    #[test]
    fn xd_50_sm_invalid_idle_to_done() {
        let mut sm = Xd50StateMachine::new();
        assert!(sm.transition(Xd50State::Done).is_err());
    }

    #[test]
    fn xd_50_sm_invalid_idle_to_paused() {
        let mut sm = Xd50StateMachine::new();
        assert!(sm.transition(Xd50State::Paused).is_err());
    }

    #[test]
    fn xd_50_sm_history_tracking() {
        let mut sm = Xd50StateMachine::new();
        sm.transition(Xd50State::Running).unwrap();
        sm.transition(Xd50State::Paused).unwrap();
        sm.transition(Xd50State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd50State::Idle);
        assert_eq!(sm.history()[0].to, Xd50State::Running);
        assert_eq!(sm.history()[1].from, Xd50State::Running);
        assert_eq!(sm.history()[2].to, Xd50State::Done);
    }

    #[test]
    fn xd_50_sm_serialize_deserialize() {
        let mut sm = Xd50StateMachine::new();
        sm.transition(Xd50State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd50StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd50State::Running));
    }

    #[test]
    fn xd_50_sm_deserialize_invalid() {
        assert_eq!(Xd50StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_50_sm_reset() {
        let mut sm = Xd50StateMachine::new();
        sm.transition(Xd50State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd50State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_50_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd50EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd50Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_50_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd50EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd50Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd50Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_50_bus_unsubscribe() {
        let mut bus = Xd50EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_50_event_kind_and_payload() {
        let e = Xd50Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd50Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_50_bus_clear_history() {
        let mut bus = Xd50EventBus::new();
        bus.publish(Xd50Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_50_sm_step_counter_increments() {
        let mut sm = Xd50StateMachine::new();
        sm.transition(Xd50State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd50State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #48 --

    #[test]
    fn xf48_trie_insert_search() {
        let mut t = Xf48Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf48_trie_starts_with() {
        let mut t = Xf48Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf48_trie_remove() {
        let mut t = Xf48Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf48_trie_word_count() {
        let mut t = Xf48Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf48_trie_longest_prefix() {
        let mut t = Xf48Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf48_trie_all_words() {
        let mut t = Xf48Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf48_trie_autocomplete() {
        let mut t = Xf48Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf48_trie_empty_search() {
        let t = Xf48Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf48_bloom_add_contains() {
        let mut bf = Xf48BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf48_bloom_probably_absent() {
        let bf = Xf48BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf48_bloom_false_positive_rate() {
        let mut bf = Xf48BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf48_bloom_clear() {
        let mut bf = Xf48BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf48_bloom_union() {
        let mut a = Xf48BloomFilter::xf_new(512, 2);
        let mut b = Xf48BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf48_bloom_intersection_estimate() {
        let mut a = Xf48BloomFilter::xf_new(512, 2);
        let mut b = Xf48BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf48_bloom_union_size_mismatch() {
        let a = Xf48BloomFilter::xf_new(256, 2);
        let b = Xf48BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh160_skip_insert_contains() {
        let mut sl = super::Xh160SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh160_skip_remove() {
        let mut sl = super::Xh160SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh160_skip_len() {
        let mut sl = super::Xh160SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh160_skip_range_query() {
        let mut sl = super::Xh160SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh160_skip_floor_ceiling() {
        let mut sl = super::Xh160SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh160_skip_rank() {
        let mut sl = super::Xh160SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh160_skip_empty() {
        let sl = super::Xh160SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh160_skip_duplicates() {
        let mut sl = super::Xh160SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh160_bitset_set_test() {
        let mut bs = super::Xh160BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh160_bitset_clear_count() {
        let mut bs = super::Xh160BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh160_bitset_and_or_xor() {
        let mut a = super::Xh160BitSet::xh_new(128);
        let mut b = super::Xh160BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh160_bitset_iter_ones() {
        let mut bs = super::Xh160BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh160_bitset_first_last() {
        let mut bs = super::Xh160BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh160_bitset_empty() {
        let bs = super::Xh160BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi160_deque_push_pop_back() {
        let mut dq = super::Xi160Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi160_deque_push_pop_front() {
        let mut dq = super::Xi160Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi160_deque_mixed_ops() {
        let mut dq = super::Xi160Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi160_deque_get_and_split() {
        let mut dq = super::Xi160Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi160_deque_rotate_left() {
        let mut dq = super::Xi160Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi160_deque_rotate_right() {
        let mut dq = super::Xi160Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi160_deque_grow() {
        let mut dq = super::Xi160Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi160_deque_empty() {
        let dq = super::Xi160Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi160_interval_tree_insert_query() {
        let mut tree = super::Xi160IntervalTree::xi_new();
        tree.xi_insert(super::Xi160Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi160Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi160Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi160_interval_tree_overlap() {
        let mut tree = super::Xi160IntervalTree::xi_new();
        tree.xi_insert(super::Xi160Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi160Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi160Interval::xi_new(12, 20));
        let q = super::Xi160Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi160_interval_tree_remove() {
        let mut tree = super::Xi160IntervalTree::xi_new();
        tree.xi_insert(super::Xi160Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi160Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi160_interval_tree_gaps() {
        let mut tree = super::Xi160IntervalTree::xi_new();
        tree.xi_insert(super::Xi160Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi160Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi160Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi160Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi160Interval::xi_new(8, 10));
    }

    #[test]
    fn xi160_interval_tree_merge() {
        let mut tree = super::Xi160IntervalTree::xi_new();
        tree.xi_insert(super::Xi160Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi160Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi160Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi160Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi160Interval::xi_new(10, 15));
    }

    #[test]
    fn xi160_interval_tree_all() {
        let mut tree = super::Xi160IntervalTree::xi_new();
        tree.xi_insert(super::Xi160Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi160Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi160_interval_tree_empty() {
        let tree = super::Xi160IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi160_interval_tree_contains_point() {
        let iv = super::Xi160Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 161) ---

    #[test]
    fn xj_161_uf_make_and_find() {
        let mut uf = super::Xj161UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_161_uf_union_connected() {
        let mut uf = super::Xj161UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_161_uf_component_count() {
        let mut uf = super::Xj161UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_161_uf_component_size() {
        let mut uf = super::Xj161UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_161_uf_largest_component() {
        let mut uf = super::Xj161UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_161_uf_many_elements() {
        let mut uf = super::Xj161UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_161_uf_separate_components() {
        let mut uf = super::Xj161UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_161_uf_path_compression() {
        let mut uf = super::Xj161UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_161_bt_insert_get() {
        let mut bt = super::Xj161BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_161_bt_contains_len() {
        let mut bt = super::Xj161BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_161_bt_replace() {
        let mut bt = super::Xj161BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_161_bt_remove() {
        let mut bt = super::Xj161BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_161_bt_keys_values() {
        let mut bt = super::Xj161BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_161_bt_range() {
        let mut bt = super::Xj161BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_161_bt_min_max() {
        let mut bt = super::Xj161BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_161_bt_many_inserts() {
        let mut bt = super::Xj161BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_160 segment tree tests ---

    #[test]
    fn xk_160_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk160SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_160_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk160SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_160_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk160SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_160_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk160SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_160_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk160SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_160_st_single_element() {
        let data = vec![42];
        let st = super::Xk160SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_160_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk160SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_160_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk160SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_160 disjoint intervals tests ---

    #[test]
    fn xk_160_di_add_and_count() {
        let mut di = super::Xk160DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_160_di_merge_overlap() {
        let mut di = super::Xk160DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_160_di_contains() {
        let mut di = super::Xk160DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_160_di_remove() {
        let mut di = super::Xk160DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_160_di_covered_length() {
        let mut di = super::Xk160DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_160_di_gaps() {
        let mut di = super::Xk160DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_160_di_merge_adjacent() {
        let mut di = super::Xk160DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_160_di_empty() {
        let di = super::Xk160DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_161_rope_new_empty() {
        let rope = super::Xl161Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_161_rope_from_str() {
        let rope = super::Xl161Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_161_rope_insert_at() {
        let mut rope = super::Xl161Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_161_rope_delete_range() {
        let mut rope = super::Xl161Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_161_rope_char_at() {
        let rope = super::Xl161Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_161_rope_split_concat() {
        let rope = super::Xl161Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_161_rope_line_count() {
        let rope = super::Xl161Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_161_rope_line_at() {
        let rope = super::Xl161Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_161_sa_build_and_search() {
        let sa = super::Xl161SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_161_sa_count() {
        let sa = super::Xl161SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_161_sa_longest_repeated() {
        let sa = super::Xl161SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_161_sa_all_positions() {
        let sa = super::Xl161SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_161_sa_len() {
        let sa = super::Xl161SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_161_sa_empty() {
        let sa = super::Xl161SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_161_rope_slice() {
        let rope = super::Xl161Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_161_sa_search_start() {
        let sa = super::Xl161SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_161_sparse_set_get() {
        let mut m = super::Xm161MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_161_sparse_row_col() {
        let mut m = super::Xm161MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_161_sparse_transpose() {
        let mut m = super::Xm161MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_161_sparse_multiply_vec() {
        let mut m = super::Xm161MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_161_sparse_nnz_density() {
        let mut m = super::Xm161MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_161_sparse_clear() {
        let mut m = super::Xm161MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_161_sparse_overwrite_zero() {
        let mut m = super::Xm161MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_161_tokenizer_basic() {
        let t = super::Xm161Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_161_tokenizer_count() {
        let t = super::Xm161Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_161_tokenizer_unique() {
        let t = super::Xm161Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_161_tokenizer_frequency() {
        let t = super::Xm161Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_161_tokenizer_delimiter() {
        let t = super::Xm161Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_161_tokenizer_whitespace() {
        let t = super::Xm161Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_161_tokenizer_empty() {
        let t = super::Xm161Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 160 ----

    #[test]
    fn xn_160_fenwick_prefix_sum() {
        let mut ft = super::Xn160Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_160_fenwick_range_sum() {
        let mut ft = super::Xn160Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_160_fenwick_point_query() {
        let mut ft = super::Xn160Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_160_fenwick_len() {
        let ft = super::Xn160Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_160_fenwick_multiple_updates() {
        let mut ft = super::Xn160Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_160_fenwick_single_element() {
        let mut ft = super::Xn160Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_160_fenwick_find_kth() {
        let mut ft = super::Xn160Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_160_fenwick_negative_delta() {
        let mut ft = super::Xn160Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 160 ----

    #[test]
    fn xn_160_avl_insert_get() {
        let mut m = super::Xn160AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_160_avl_remove() {
        let mut m = super::Xn160AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_160_avl_in_order() {
        let mut m = super::Xn160AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_160_avl_min_max() {
        let mut m = super::Xn160AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_160_avl_floor_ceiling() {
        let mut m = super::Xn160AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_160_avl_height_balanced() {
        let mut m = super::Xn160AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_160_avl_overwrite() {
        let mut m = super::Xn160AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_160_avl_empty() {
        let m: super::Xn160AVL<i32, i32> = super::Xn160AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo160RedBlack tests ---

    #[test]
    fn xo_160_rb_insert_and_get() {
        let mut tree = super::Xo160RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_160_rb_len_and_empty() {
        let mut tree = super::Xo160RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_160_rb_min_max() {
        let mut tree = super::Xo160RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_160_rb_contains() {
        let mut tree = super::Xo160RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_160_rb_remove() {
        let mut tree = super::Xo160RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_160_rb_in_order() {
        let mut tree = super::Xo160RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_160_rb_black_height() {
        let mut tree = super::Xo160RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_160_rb_overwrite() {
        let mut tree = super::Xo160RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo160ConsistentHash tests ---

    #[test]
    fn xo_160_ch_add_and_count() {
        let mut ring = super::Xo160ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_160_ch_remove_node() {
        let mut ring = super::Xo160ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_160_ch_get_node() {
        let mut ring = super::Xo160ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_160_ch_empty_ring() {
        let ring = super::Xo160ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_160_ch_distribution() {
        let mut ring = super::Xo160ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_160_ch_rebalance() {
        let mut ring = super::Xo160ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_160_ch_virtual_nodes() {
        let mut ring = super::Xo160ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_160_ch_consistent_lookup() {
        let mut ring = super::Xo160ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_160_splay_insert_get() {
        let mut t = super::Xp160SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_160_splay_remove() {
        let mut t = super::Xp160SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_160_splay_count_increases() {
        let mut t = super::Xp160SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_160_splay_depth() {
        let mut t = super::Xp160SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_160_splay_len_empty() {
        let t = super::Xp160SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_160_splay_min_max() {
        let mut t = super::Xp160SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_160_splay_overwrite() {
        let mut t = super::Xp160SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_160_splay_remove_missing() {
        let mut t = super::Xp160SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }

}
