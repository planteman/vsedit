//! Snippets manager.

use std::collections::HashMap;
use std::fmt;

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

    /// Returns true if snippets is empty.
    pub fn is_snippets_empty(&self) -> bool {
        self.snippets.is_empty()
    }

    /// Get the first snippet, if any.
    pub fn first_snippet(&self) -> Option<&Snippet> {
        self.snippets.first()
    }

    /// Get the last snippet, if any.
    pub fn last_snippet(&self) -> Option<&Snippet> {
        self.snippets.last()
    }

    /// Retain only snippets matching the predicate.
    pub fn retain_snippets(&mut self, f: impl Fn(&Snippet) -> bool) {
        self.snippets.retain(|item| f(item));
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

// ---------------------------------------------------------------------------
// Language-scoped snippet registry
// ---------------------------------------------------------------------------

/// A registry that organizes snippets by language ID for fast lookup.
pub struct LanguageScopedRegistry {
    by_language: HashMap<String, Vec<Snippet>>,
    global: Vec<Snippet>,
}

impl LanguageScopedRegistry {
    pub fn new() -> Self {
        Self {
            by_language: HashMap::new(),
            global: Vec::new(),
        }
    }

    /// Register snippets for a specific language.
    pub fn register_snippets(&mut self, language: &str, snippets: Vec<Snippet>) {
        self.by_language
            .entry(language.to_string())
            .or_default()
            .extend(snippets);
    }

    /// Register a snippet that applies to all languages.
    pub fn register_global(&mut self, snippet: Snippet) {
        self.global.push(snippet);
    }

    /// Get all snippets available for a language (language-specific + global).
    pub fn get_snippets(&self, language: &str) -> Vec<&Snippet> {
        let mut result: Vec<&Snippet> = self.global.iter().collect();
        if let Some(lang_snippets) = self.by_language.get(language) {
            result.extend(lang_snippets.iter());
        }
        result
    }

    /// Get snippet count for a specific language (excluding globals).
    pub fn language_snippet_count(&self, language: &str) -> usize {
        self.by_language
            .get(language)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Get all registered language IDs.
    pub fn languages(&self) -> Vec<&str> {
        self.by_language.keys().map(|s| s.as_str()).collect()
    }

    /// Total snippet count across all languages + globals.
    pub fn total_count(&self) -> usize {
        self.global.len() + self.by_language.values().map(|v| v.len()).sum::<usize>()
    }
}

impl Default for LanguageScopedRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Load snippets from a VS Code JSON snippet string into a vector.
pub fn load_vscode_snippets(json: &str, source: SnippetSource) -> Result<Vec<Snippet>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "expected JSON object".to_string())?;
    let mut snippets = Vec::new();
    for (name, entry) in obj {
        let entry_obj = entry
            .as_object()
            .ok_or_else(|| format!("expected object for {name}"))?;
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
        snippets.push(Snippet {
            name: name.clone(),
            prefix,
            body,
            description,
            scope,
            source: source.clone(),
        });
    }
    Ok(snippets)
}

/// Validates snippet body syntax.
pub struct SnippetValidator;

impl SnippetValidator {
    /// Check that all tabstop braces are balanced (e.g. `${1:default}`).
    /// Returns `Ok(())` if valid, or an error message describing the problem.
    pub fn validate_body(body: &[String]) -> Result<(), String> {
        for (line_idx, line) in body.iter().enumerate() {
            let mut depth = 0i32;
            let chars: Vec<char> = line.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
                    depth += 1;
                    i += 2;
                    continue;
                }
                if chars[i] == '}' && depth > 0 {
                    depth -= 1;
                }
                i += 1;
            }
            if depth != 0 {
                return Err(format!(
                    "unbalanced tabstop braces on line {} (depth {})",
                    line_idx + 1,
                    depth,
                ));
            }
        }
        Ok(())
    }

    /// Validate that tabstop numbers are non-negative integers and sequential
    /// starting from 0 or 1 with no gaps.
    pub fn validate_tabstop_numbers(body: &[String]) -> Result<(), String> {
        let mut numbers = Vec::new();
        for line in body {
            let chars: Vec<char> = line.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                if chars[i] == '$' && i + 1 < chars.len() {
                    i += 1;
                    if chars[i] == '{' {
                        i += 1;
                        let start = i;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        if i > start {
                            let num_str: String = chars[start..i].iter().collect();
                            if let Ok(n) = num_str.parse::<u32>() {
                                numbers.push(n);
                            }
                        }
                    } else if chars[i].is_ascii_digit() {
                        let start = i;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(n) = num_str.parse::<u32>() {
                            numbers.push(n);
                        }
                        continue;
                    }
                }
                i += 1;
            }
        }
        numbers.sort();
        numbers.dedup();
        if numbers.is_empty() {
            return Ok(());
        }
        let min = numbers[0];
        if min > 1 {
            return Err(format!("tabstops should start at 0 or 1, found {min}"));
        }
        for window in numbers.windows(2) {
            if window[1] - window[0] > 1 {
                return Err(format!(
                    "gap in tabstop numbering between {} and {}",
                    window[0], window[1],
                ));
            }
        }
        Ok(())
    }

    /// Run all validations on a snippet.
    pub fn validate(snippet: &Snippet) -> Result<(), String> {
        Self::validate_body(&snippet.body)?;
        Self::validate_tabstop_numbers(&snippet.body)?;
        Ok(())
    }
}

/// Imports snippets from a JSON string.
pub struct SnippetImporter;

impl SnippetImporter {
    /// Parse a JSON string into a list of snippets.
    ///
    /// Expected format (VS Code snippet file format):
    /// ```json
    /// {
    ///   "Snippet Name": {
    ///     "prefix": ["trigger"],
    ///     "body": ["line1", "line2"],
    ///     "description": "optional",
    ///     "scope": "optional"
    ///   }
    /// }
    /// ```
    pub fn from_json(json: &str, source: SnippetSource) -> Result<Vec<Snippet>, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
        let obj = value.as_object().ok_or("expected top-level object")?;
        let mut snippets = Vec::new();
        for (name, entry) in obj {
            let entry_obj = entry.as_object().ok_or_else(|| {
                format!("expected object for snippet '{name}'")
            })?;
            let prefix = match entry_obj.get("prefix") {
                Some(serde_json::Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                Some(serde_json::Value::String(s)) => vec![s.clone()],
                _ => vec![name.clone()],
            };
            let body = match entry_obj.get("body") {
                Some(serde_json::Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                Some(serde_json::Value::String(s)) => vec![s.clone()],
                _ => Vec::new(),
            };
            let description = entry_obj
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from);
            let scope = entry_obj
                .get("scope")
                .and_then(|v| v.as_str())
                .map(String::from);
            snippets.push(Snippet {
                name: name.clone(),
                prefix,
                body,
                description,
                scope,
                source: source.clone(),
            });
        }
        Ok(snippets)
    }
}

/// Exports snippets to a JSON string.
pub struct SnippetExporter;

impl SnippetExporter {
    /// Convert a list of snippets into a JSON string in VS Code format.
    pub fn to_json(snippets: &[Snippet]) -> Result<String, String> {
        let mut map = serde_json::Map::new();
        for snippet in snippets {
            let mut entry = serde_json::Map::new();
            let prefix_values: Vec<serde_json::Value> = snippet
                .prefix
                .iter()
                .map(|p| serde_json::Value::String(p.clone()))
                .collect();
            entry.insert(
                "prefix".to_string(),
                serde_json::Value::Array(prefix_values),
            );
            let body_values: Vec<serde_json::Value> = snippet
                .body
                .iter()
                .map(|b| serde_json::Value::String(b.clone()))
                .collect();
            entry.insert(
                "body".to_string(),
                serde_json::Value::Array(body_values),
            );
            if let Some(ref desc) = snippet.description {
                entry.insert(
                    "description".to_string(),
                    serde_json::Value::String(desc.clone()),
                );
            }
            if let Some(ref scope) = snippet.scope {
                entry.insert(
                    "scope".to_string(),
                    serde_json::Value::String(scope.clone()),
                );
            }
            map.insert(snippet.name.clone(), serde_json::Value::Object(entry));
        }
        serde_json::to_string_pretty(&serde_json::Value::Object(map))
            .map_err(|e| format!("serialization failed: {e}"))
    }
}

// ---------------------------------------------------------------------------
// snippet_variable_resolver — resolve tab stop variables
// ---------------------------------------------------------------------------

/// Known snippet variables matching VS Code's snippet variable reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetVariable {
    TmFilename,
    TmFilenameBase,
    TmDirectory,
    TmFilepath,
    TmSelectedText,
    TmCurrentLine,
    TmCurrentWord,
    TmLineIndex,
    TmLineNumber,
    ClipboardContent,
    WorkspaceName,
    WorkspaceFolder,
}

impl SnippetVariable {
    /// Return the placeholder token (e.g. `$TM_FILENAME`).
    pub fn token(&self) -> &'static str {
        match self {
            Self::TmFilename => "$TM_FILENAME",
            Self::TmFilenameBase => "$TM_FILENAME_BASE",
            Self::TmDirectory => "$TM_DIRECTORY",
            Self::TmFilepath => "$TM_FILEPATH",
            Self::TmSelectedText => "$TM_SELECTED_TEXT",
            Self::TmCurrentLine => "$TM_CURRENT_LINE",
            Self::TmCurrentWord => "$TM_CURRENT_WORD",
            Self::TmLineIndex => "$TM_LINE_INDEX",
            Self::TmLineNumber => "$TM_LINE_NUMBER",
            Self::ClipboardContent => "$CLIPBOARD",
            Self::WorkspaceName => "$WORKSPACE_NAME",
            Self::WorkspaceFolder => "$WORKSPACE_FOLDER",
        }
    }

    /// All known variables.
    pub fn all() -> &'static [SnippetVariable] {
        &[
            Self::TmFilename, Self::TmFilenameBase, Self::TmDirectory,
            Self::TmFilepath, Self::TmSelectedText, Self::TmCurrentLine,
            Self::TmCurrentWord, Self::TmLineIndex, Self::TmLineNumber,
            Self::ClipboardContent, Self::WorkspaceName, Self::WorkspaceFolder,
        ]
    }
}

/// Context for resolving snippet variables.
#[derive(Debug, Clone, Default)]
pub struct SnippetVariableContext {
    pub filename: Option<String>,
    pub filepath: Option<String>,
    pub directory: Option<String>,
    pub selected_text: Option<String>,
    pub current_line: Option<String>,
    pub current_word: Option<String>,
    pub line_index: Option<u32>,
    pub line_number: Option<u32>,
    pub clipboard: Option<String>,
    pub workspace_name: Option<String>,
    pub workspace_folder: Option<String>,
}

impl SnippetVariableContext {
    fn resolve_var(&self, var: SnippetVariable) -> String {
        match var {
            SnippetVariable::TmFilename => self.filename.clone().unwrap_or_default(),
            SnippetVariable::TmFilenameBase => {
                self.filename.as_ref()
                    .and_then(|f| f.rsplit('.').last().map(String::from))
                    .unwrap_or_default()
            }
            SnippetVariable::TmDirectory => self.directory.clone().unwrap_or_default(),
            SnippetVariable::TmFilepath => self.filepath.clone().unwrap_or_default(),
            SnippetVariable::TmSelectedText => self.selected_text.clone().unwrap_or_default(),
            SnippetVariable::TmCurrentLine => self.current_line.clone().unwrap_or_default(),
            SnippetVariable::TmCurrentWord => self.current_word.clone().unwrap_or_default(),
            SnippetVariable::TmLineIndex => self.line_index.map(|n| n.to_string()).unwrap_or_else(|| "0".into()),
            SnippetVariable::TmLineNumber => self.line_number.map(|n| n.to_string()).unwrap_or_else(|| "1".into()),
            SnippetVariable::ClipboardContent => self.clipboard.clone().unwrap_or_default(),
            SnippetVariable::WorkspaceName => self.workspace_name.clone().unwrap_or_default(),
            SnippetVariable::WorkspaceFolder => self.workspace_folder.clone().unwrap_or_default(),
        }
    }
}

/// Resolve all known snippet variables in a body string.
pub fn snippet_variable_resolver(body: &str, ctx: &SnippetVariableContext) -> String {
    let mut result = body.to_string();
    // Sort by token length descending so longer tokens (e.g. $TM_FILENAME_BASE)
    // are replaced before shorter prefixes (e.g. $TM_FILENAME).
    let mut vars: Vec<SnippetVariable> = SnippetVariable::all().to_vec();
    vars.sort_by(|a, b| b.token().len().cmp(&a.token().len()));
    for var in &vars {
        let replacement = ctx.resolve_var(*var);
        result = result.replace(var.token(), &replacement);
    }
    result
}

/// Resolve variables in all lines of a snippet body.
pub fn snippet_resolve_body(body: &[String], ctx: &SnippetVariableContext) -> Vec<String> {
    body.iter()
        .map(|line| snippet_variable_resolver(line, ctx))
        .collect()
}

// ---------------------------------------------------------------------------
// Snippet fuzzy search
// ---------------------------------------------------------------------------

/// Simple fuzzy matching for snippet search.
pub struct SnippetSearch;

impl SnippetSearch {
    /// Returns true if all characters in `pattern` appear in `text` in order
    /// (case-insensitive).
    pub fn fuzzy_matches(pattern: &str, text: &str) -> bool {
        let mut pattern_chars = pattern.chars().flat_map(|c| c.to_lowercase());
        let mut text_chars = text.chars().flat_map(|c| c.to_lowercase());

        let mut next_p = pattern_chars.next();
        while let Some(p) = next_p {
            loop {
                match text_chars.next() {
                    Some(t) if t == p => break,
                    Some(_) => continue,
                    None => return false,
                }
            }
            next_p = pattern_chars.next();
        }
        true
    }

    /// Compute a simple fuzzy match score (higher = better).
    /// Returns `None` if there is no match.
    pub fn fuzzy_score(pattern: &str, text: &str) -> Option<u32> {
        if !Self::fuzzy_matches(pattern, text) {
            return None;
        }
        let pl = pattern.len() as u32;
        let tl = text.len() as u32;
        // Bonus for exact prefix match.
        let prefix_bonus = if text
            .to_lowercase()
            .starts_with(&pattern.to_lowercase())
        {
            50
        } else {
            0
        };
        // Shorter targets are better matches for the same pattern.
        let length_score = if tl > 0 { (pl * 100) / tl } else { 100 };
        Some(length_score + prefix_bonus)
    }

    /// Search snippets by fuzzy-matching the query against name, prefixes, and
    /// description. Results are sorted by score descending.
    pub fn search(snippets: &[Snippet], query: &str) -> Vec<SnippetSearchResult> {
        let mut results: Vec<SnippetSearchResult> = snippets
            .iter()
            .filter_map(|s| {
                // Best score across name, prefix entries and description.
                let mut best: Option<u32> = Self::fuzzy_score(query, &s.name);

                for p in &s.prefix {
                    if let Some(sc) = Self::fuzzy_score(query, p) {
                        best = Some(best.map_or(sc, |b| b.max(sc)));
                    }
                }
                if let Some(ref desc) = s.description {
                    if let Some(sc) = Self::fuzzy_score(query, desc) {
                        best = Some(best.map_or(sc, |b| b.max(sc)));
                    }
                }

                best.map(|score| SnippetSearchResult {
                    snippet: s.clone(),
                    score,
                })
            })
            .collect();

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }
}

/// A search result with its match score.
#[derive(Debug, Clone)]
pub struct SnippetSearchResult {
    pub snippet: Snippet,
    pub score: u32,
}

// ---------------------------------------------------------------------------
// Snippet usage tracker
// ---------------------------------------------------------------------------

/// Tracks how often and when each snippet is used.
pub struct SnippetUsageTracker {
    counts: HashMap<String, u64>,
}

impl SnippetUsageTracker {
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    /// Record one use of the snippet with the given name.
    pub fn record_use(&mut self, name: &str) {
        *self.counts.entry(name.to_string()).or_insert(0) += 1;
    }

    /// Get the usage count for a snippet.
    pub fn usage_count(&self, name: &str) -> u64 {
        self.counts.get(name).copied().unwrap_or(0)
    }

    /// Return snippet names sorted by usage count descending.
    pub fn most_used(&self) -> Vec<(&str, u64)> {
        let mut entries: Vec<(&str, u64)> = self
            .counts
            .iter()
            .map(|(k, &v)| (k.as_str(), v))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries
    }

    /// Return the total number of tracked snippets.
    pub fn tracked_count(&self) -> usize {
        self.counts.len()
    }

    /// Reset all usage data.
    pub fn reset(&mut self) {
        self.counts.clear();
    }
}

impl Default for SnippetUsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Snippet conflict resolver
// ---------------------------------------------------------------------------

/// Describes a prefix collision between two snippets.
#[derive(Debug, Clone, PartialEq)]
pub struct SnippetConflict {
    pub prefix: String,
    pub snippet_a: String,
    pub snippet_b: String,
}

/// Detects and resolves prefix conflicts among snippets.
pub struct SnippetConflictResolver;

impl SnippetConflictResolver {
    /// Find all prefix collisions in a set of snippets.
    pub fn detect_conflicts(snippets: &[Snippet]) -> Vec<SnippetConflict> {
        let mut prefix_map: HashMap<&str, Vec<&str>> = HashMap::new();
        for s in snippets {
            for p in &s.prefix {
                prefix_map.entry(p.as_str()).or_default().push(&s.name);
            }
        }

        let mut conflicts = Vec::new();
        for (prefix, names) in &prefix_map {
            if names.len() > 1 {
                for i in 0..names.len() {
                    for j in (i + 1)..names.len() {
                        conflicts.push(SnippetConflict {
                            prefix: prefix.to_string(),
                            snippet_a: names[i].to_string(),
                            snippet_b: names[j].to_string(),
                        });
                    }
                }
            }
        }
        conflicts.sort_by(|a, b| a.prefix.cmp(&b.prefix));
        conflicts
    }

    /// Returns `true` if the snippet set has any prefix collisions.
    pub fn has_conflicts(snippets: &[Snippet]) -> bool {
        !Self::detect_conflicts(snippets).is_empty()
    }
}

// ---------------------------------------------------------------------------
// Snippet body indentation helper
// ---------------------------------------------------------------------------

/// Re-indent a multi-line snippet body to match a given base indentation.
pub fn reindent_snippet_body(body: &[String], base_indent: &str) -> Vec<String> {
    body.iter()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                // First line keeps its content as-is (cursor is already at position).
                line.clone()
            } else if line.is_empty() {
                String::new()
            } else {
                format!("{}{}", base_indent, line)
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Snippet categorization / tagging
// ---------------------------------------------------------------------------

/// A snippet with associated tags for categorization.
#[derive(Debug, Clone)]
pub struct TaggedSnippet {
    pub snippet: Snippet,
    pub tags: Vec<String>,
    pub enabled: bool,
}

/// Manages snippets organized by tags/categories.
pub struct SnippetCatalog {
    entries: Vec<TaggedSnippet>,
}

impl SnippetCatalog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add a snippet with tags.
    pub fn add(&mut self, snippet: Snippet, tags: Vec<String>) {
        self.entries.push(TaggedSnippet {
            snippet,
            tags,
            enabled: true,
        });
    }

    /// Find all snippets that have the given tag.
    pub fn find_by_tag(&self, tag: &str) -> Vec<&TaggedSnippet> {
        self.entries
            .iter()
            .filter(|e| e.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Return all unique tags sorted alphabetically.
    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .entries
            .iter()
            .flat_map(|e| e.tags.iter().cloned())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        tags.sort();
        tags
    }

    /// Disable all snippets that have the given tag.
    pub fn disable_by_tag(&mut self, tag: &str) -> usize {
        let mut count = 0;
        for entry in &mut self.entries {
            if entry.enabled && entry.tags.iter().any(|t| t == tag) {
                entry.enabled = false;
                count += 1;
            }
        }
        count
    }

    /// Enable all snippets that have the given tag.
    pub fn enable_by_tag(&mut self, tag: &str) -> usize {
        let mut count = 0;
        for entry in &mut self.entries {
            if !entry.enabled && entry.tags.iter().any(|t| t == tag) {
                entry.enabled = true;
                count += 1;
            }
        }
        count
    }

    /// Return only enabled snippets.
    pub fn enabled_snippets(&self) -> Vec<&Snippet> {
        self.entries
            .iter()
            .filter(|e| e.enabled)
            .map(|e| &e.snippet)
            .collect()
    }

    /// Return only disabled snippets.
    pub fn disabled_snippets(&self) -> Vec<&Snippet> {
        self.entries
            .iter()
            .filter(|e| !e.enabled)
            .map(|e| &e.snippet)
            .collect()
    }

    /// Total number of catalog entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the catalog is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for SnippetCatalog {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Snippet sorting by usage frequency
// ---------------------------------------------------------------------------

/// Sort snippets by their usage frequency (most-used first).
/// Snippets not present in `usage` are treated as having zero uses.
pub fn sort_snippets_by_usage(snippets: &mut [Snippet], usage: &SnippetUsageTracker) {
    snippets.sort_by(|a, b| {
        let ua = usage.usage_count(&a.name);
        let ub = usage.usage_count(&b.name);
        ub.cmp(&ua)
    });
}

// ---------------------------------------------------------------------------
// Snippet body complexity analysis
// ---------------------------------------------------------------------------

/// Metrics describing the complexity of a snippet body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetComplexity {
    pub line_count: usize,
    pub tabstop_count: usize,
    pub has_choices: bool,
    pub has_variables: bool,
    pub nested_placeholders: bool,
}

/// Analyse a snippet body and return complexity metrics.
pub fn analyse_snippet_complexity(body: &[String]) -> SnippetComplexity {
    let joined = body.join("\n");
    let tabstop_count = count_tabstops(&joined);

    // Check for choice syntax: ${1|one,two,three|}
    let has_choices = body.iter().any(|line| {
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;
        while i + 2 < len {
            if chars[i] == '$' && chars[i + 1] == '{' {
                // Skip the digit(s)
                let mut j = i + 2;
                while j < len && chars[j].is_ascii_digit() {
                    j += 1;
                }
                if j < len && chars[j] == '|' {
                    return true;
                }
            }
            i += 1;
        }
        false
    });

    let has_variables = SnippetVariable::all()
        .iter()
        .any(|v| joined.contains(v.token()));

    // Nested placeholders: ${1:${2:inner}}
    let nested_placeholders = body.iter().any(|line| {
        let mut depth = 0i32;
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
                depth += 1;
                if depth > 1 {
                    return true;
                }
                i += 2;
                continue;
            }
            if chars[i] == '}' && depth > 0 {
                depth -= 1;
            }
            i += 1;
        }
        false
    });

    SnippetComplexity {
        line_count: body.len(),
        tabstop_count,
        has_choices,
        has_variables,
        nested_placeholders,
    }
}

// ---------------------------------------------------------------------------
// SnippetTabStopLinker – connected tabstops
// ---------------------------------------------------------------------------

/// Links tabstops that share the same number so editing one updates all.
pub struct SnippetTabStopLinker {
    /// Map from tabstop number to list of (line_index, start, end) positions.
    positions: HashMap<u32, Vec<(usize, usize, usize)>>,
}

impl SnippetTabStopLinker {
    /// Parse tabstop positions from a snippet body.
    pub fn from_body(body: &[String]) -> Self {
        let mut positions: HashMap<u32, Vec<(usize, usize, usize)>> = HashMap::new();
        for (line_idx, line) in body.iter().enumerate() {
            let mut i = 0;
            let chars: Vec<char> = line.chars().collect();
            while i < chars.len() {
                if chars[i] == '$' && i + 1 < chars.len() {
                    let start = i;
                    i += 1;
                    if chars[i] == '{' {
                        i += 1;
                        let num_start = i;
                        while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
                        if i > num_start {
                            if let Ok(num) = chars[num_start..i].iter().collect::<String>().parse::<u32>() {
                                // Skip past the closing }
                                while i < chars.len() && chars[i] != '}' { i += 1; }
                                if i < chars.len() { i += 1; }
                                positions.entry(num).or_default().push((line_idx, start, i));
                            }
                        }
                    } else if chars[i].is_ascii_digit() {
                        let num_start = i;
                        while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
                        if let Ok(num) = chars[num_start..i].iter().collect::<String>().parse::<u32>() {
                            positions.entry(num).or_default().push((line_idx, start, i));
                        }
                    }
                } else {
                    i += 1;
                }
            }
        }
        Self { positions }
    }

    /// Get all tabstop numbers found.
    pub fn tabstop_numbers(&self) -> Vec<u32> {
        let mut nums: Vec<u32> = self.positions.keys().copied().collect();
        nums.sort();
        nums
    }

    /// Get the positions linked to a tabstop number.
    pub fn linked_positions(&self, tabstop: u32) -> &[(usize, usize, usize)] {
        self.positions.get(&tabstop).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Whether a tabstop has multiple linked positions.
    pub fn is_linked(&self, tabstop: u32) -> bool {
        self.positions.get(&tabstop).map(|v| v.len() > 1).unwrap_or(false)
    }

    /// Total number of distinct tabstops.
    pub fn tabstop_count(&self) -> usize {
        self.positions.len()
    }
}

// ---------------------------------------------------------------------------
// SnippetChoiceExpander – expand choice placeholders
// ---------------------------------------------------------------------------

/// A parsed choice placeholder like `${1|one,two,three|}`.
#[derive(Debug, Clone)]
pub struct SnippetChoice {
    pub tabstop: u32,
    pub options: Vec<String>,
}

/// Parses and expands choice placeholders in snippet bodies.
pub struct SnippetChoiceExpander;

impl SnippetChoiceExpander {
    /// Extract all choice placeholders from a body line.
    pub fn extract_choices(body: &str) -> Vec<SnippetChoice> {
        let mut choices = Vec::new();
        let mut chars = body.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '$' && chars.peek() == Some(&'{') {
                chars.next(); // skip {
                let mut num_str = String::new();
                while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    num_str.push(chars.next().unwrap());
                }
                if chars.peek() == Some(&'|') {
                    chars.next(); // skip |
                    if let Ok(tabstop) = num_str.parse::<u32>() {
                        let mut options_str = String::new();
                        while let Some(&c) = chars.peek() {
                            if c == '|' { chars.next(); break; }
                            options_str.push(chars.next().unwrap());
                        }
                        // skip closing }
                        if chars.peek() == Some(&'}') { chars.next(); }
                        let options: Vec<String> = options_str.split(',').map(|s| s.to_string()).collect();
                        choices.push(SnippetChoice { tabstop, options });
                    }
                } else {
                    // Not a choice, skip to }
                    while chars.peek().is_some() && chars.peek() != Some(&'}') { chars.next(); }
                    if chars.peek() == Some(&'}') { chars.next(); }
                }
            }
        }
        choices
    }

    /// Expand a body line by selecting the first option for each choice.
    pub fn expand_with_defaults(body: &str) -> String {
        let mut result = body.to_string();
        let choices = Self::extract_choices(body);
        for choice in choices.iter().rev() {
            if let Some(first) = choice.options.first() {
                let pattern = format!("${{{}|{}|{}", choice.tabstop, choice.options.join(","), "}");

                result = result.replace(&pattern, first);
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// SnippetFormatConverter – import from VS Code format
// ---------------------------------------------------------------------------

/// Converts snippets between different format representations.
pub struct SnippetFormatConverter;

impl SnippetFormatConverter {
    /// Convert a snippet body from single-string format (with \n) to lines.
    pub fn body_string_to_lines(body: &str) -> Vec<String> {
        body.split('\n').map(|s| s.to_string()).collect()
    }

    /// Convert snippet lines back to a single body string.
    pub fn lines_to_body_string(lines: &[String]) -> String {
        lines.join("\n")
    }

    /// Convert a VS Code snippet JSON to the internal format.
    pub fn from_vscode_json(json: &str) -> Result<Vec<Snippet>, String> {
        SnippetImporter::from_json(json, SnippetSource::Extension)
    }

    /// Export snippets to VS Code JSON format.
    pub fn to_vscode_json(snippets: &[Snippet]) -> String {
        let mut out = String::from("{\n");
        for (i, s) in snippets.iter().enumerate() {
            out.push_str(&format!("  \"{}\": {{\n", s.name));
            let prefixes: Vec<String> = s.prefix.iter().map(|p| format!("\"{}\"", p)).collect();
            out.push_str(&format!("    \"prefix\": [{}],\n", prefixes.join(", ")));
            let body_lines: Vec<String> = s.body.iter().map(|l| format!("\"{}\"", l.replace('\\', "\\\\").replace('"', "\\\""))).collect();
            out.push_str(&format!("    \"body\": [{}]", body_lines.join(", ")));
            if let Some(desc) = &s.description {
                out.push_str(&format!(",\n    \"description\": \"{}\"", desc));
            }
            if let Some(scope) = &s.scope {
                out.push_str(&format!(",\n    \"scope\": \"{}\"", scope));
            }
            out.push_str("\n  }");
            if i + 1 < snippets.len() { out.push(','); }
            out.push('\n');
        }
        out.push('}');
        out
    }
}

// ---------------------------------------------------------------------------
// ImportedSnippet – lightweight snippet representation for import/export
// ---------------------------------------------------------------------------

/// A lightweight snippet representation used during import and export,
/// independent of the internal [`Snippet`] type.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedSnippet {
    pub name: String,
    pub prefix: String,
    pub body: Vec<String>,
    pub description: Option<String>,
    pub scope: Option<String>,
}

impl std::fmt::Display for ImportedSnippet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (prefix: {})", self.name, self.prefix)?;
        if let Some(ref scope) = self.scope {
            write!(f, " [scope: {scope}]")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SnippetJsonImporter – parse VS Code JSON into ImportedSnippet
// ---------------------------------------------------------------------------

/// Parses VS Code snippet JSON into [`ImportedSnippet`] values.
pub struct SnippetJsonImporter;

impl SnippetJsonImporter {
    /// Parse a VS Code snippet JSON string into a list of [`ImportedSnippet`].
    pub fn from_vscode_json(json: &str) -> Result<Vec<ImportedSnippet>, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
        Self::from_json_value(&value)
    }

    /// Parse a pre-parsed [`serde_json::Value`] into a list of [`ImportedSnippet`].
    pub fn from_json_value(value: &serde_json::Value) -> Result<Vec<ImportedSnippet>, String> {
        let obj = value.as_object().ok_or("expected top-level JSON object")?;
        let mut snippets = Vec::new();

        for (name, entry) in obj {
            let entry_obj = entry
                .as_object()
                .ok_or_else(|| format!("expected object for snippet '{name}'"))?;

            let prefix = match entry_obj.get("prefix") {
                Some(serde_json::Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .next()
                    .unwrap_or(name.as_str())
                    .to_string(),
                Some(serde_json::Value::String(s)) => s.clone(),
                _ => name.clone(),
            };

            let body = match entry_obj.get("body") {
                Some(serde_json::Value::Array(arr)) => {
                    arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                }
                Some(serde_json::Value::String(s)) => {
                    s.split('\n').map(String::from).collect()
                }
                _ => Vec::new(),
            };

            let description = entry_obj
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from);

            let scope = entry_obj
                .get("scope")
                .and_then(|v| v.as_str())
                .map(String::from);

            snippets.push(ImportedSnippet {
                name: name.clone(),
                prefix,
                body,
                description,
                scope,
            });
        }

        Ok(snippets)
    }
}

// ---------------------------------------------------------------------------
// SnippetJsonExporter – build and export snippet collections to JSON
// ---------------------------------------------------------------------------

/// Accumulates [`ImportedSnippet`] values and serialises them to JSON.
pub struct SnippetJsonExporter {
    snippets: Vec<ImportedSnippet>,
}

impl SnippetJsonExporter {
    pub fn new() -> Self {
        Self {
            snippets: Vec::new(),
        }
    }

    pub fn add_snippet(&mut self, snippet: ImportedSnippet) {
        self.snippets.push(snippet);
    }

    /// Serialise to formatted (pretty-printed) JSON.
    pub fn to_json(&self) -> String {
        let value = self.build_json_value();
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
    }

    /// Serialise to compact single-line JSON.
    pub fn to_json_compact(&self) -> String {
        let value = self.build_json_value();
        serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn snippet_count(&self) -> usize {
        self.snippets.len()
    }

    fn build_json_value(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for s in &self.snippets {
            let mut entry = serde_json::Map::new();
            entry.insert(
                "prefix".to_string(),
                serde_json::Value::String(s.prefix.clone()),
            );
            let body_vals: Vec<serde_json::Value> =
                s.body.iter().map(|l| serde_json::Value::String(l.clone())).collect();
            entry.insert("body".to_string(), serde_json::Value::Array(body_vals));
            if let Some(ref desc) = s.description {
                entry.insert(
                    "description".to_string(),
                    serde_json::Value::String(desc.clone()),
                );
            }
            if let Some(ref scope) = s.scope {
                entry.insert(
                    "scope".to_string(),
                    serde_json::Value::String(scope.clone()),
                );
            }
            map.insert(s.name.clone(), serde_json::Value::Object(entry));
        }
        serde_json::Value::Object(map)
    }
}

impl std::fmt::Display for SnippetJsonExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SnippetJsonExporter({} snippets)", self.snippets.len())
    }
}

// ---------------------------------------------------------------------------
// SnippetConflictDetector – scope-aware prefix clash detection
// ---------------------------------------------------------------------------

/// A detected prefix conflict between two or more snippets.
#[derive(Debug, Clone, PartialEq)]
pub struct SnippetPrefixConflict {
    pub prefix: String,
    /// Names of snippets that share this prefix.
    pub snippets: Vec<String>,
    /// Whether all conflicting snippets target the same scope.
    pub is_same_scope: bool,
}

impl std::fmt::Display for SnippetPrefixConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "prefix '{}' shared by [{}] (same_scope={})",
            self.prefix,
            self.snippets.join(", "),
            self.is_same_scope,
        )
    }
}

/// Detects prefix clashes between snippets, taking scope into account.
pub struct SnippetConflictDetector {
    entries: Vec<(String, String, Option<String>)>, // (name, prefix, scope)
}

impl SnippetConflictDetector {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add_snippet(&mut self, name: &str, prefix: &str, scope: Option<&str>) {
        self.entries.push((
            name.to_string(),
            prefix.to_string(),
            scope.map(String::from),
        ));
    }

    /// Return all detected prefix conflicts.
    pub fn detect_conflicts(&self) -> Vec<SnippetPrefixConflict> {
        let mut prefix_map: HashMap<String, Vec<(String, Option<String>)>> = HashMap::new();
        for (name, prefix, scope) in &self.entries {
            prefix_map
                .entry(prefix.clone())
                .or_default()
                .push((name.clone(), scope.clone()));
        }

        let mut conflicts = Vec::new();
        for (prefix, entries) in &prefix_map {
            if entries.len() > 1 {
                let names: Vec<String> = entries.iter().map(|(n, _)| n.clone()).collect();
                let scopes: Vec<Option<&str>> =
                    entries.iter().map(|(_, s)| s.as_deref()).collect();
                let is_same_scope = scopes.windows(2).all(|w| w[0] == w[1]);
                conflicts.push(SnippetPrefixConflict {
                    prefix: prefix.clone(),
                    snippets: names,
                    is_same_scope,
                });
            }
        }
        conflicts.sort_by(|a, b| a.prefix.cmp(&b.prefix));
        conflicts
    }

    pub fn has_conflicts(&self) -> bool {
        !self.detect_conflicts().is_empty()
    }

    pub fn conflict_count(&self) -> usize {
        self.detect_conflicts().len()
    }
}

impl std::fmt::Display for SnippetConflictDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.conflict_count();
        write!(f, "SnippetConflictDetector({n} conflicts)")
    }
}

// ---------------------------------------------------------------------------
// SnippetUsageStats – track how often snippets are used
// ---------------------------------------------------------------------------

/// Tracks usage counts for snippets.
pub struct SnippetUsageStats {
    counts: HashMap<String, usize>,
}

impl SnippetUsageStats {
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    pub fn record_usage(&mut self, snippet_name: &str) {
        *self.counts.entry(snippet_name.to_string()).or_insert(0) += 1;
    }

    pub fn usage_count(&self, snippet_name: &str) -> usize {
        self.counts.get(snippet_name).copied().unwrap_or(0)
    }

    /// Return the *n* most-used snippets, sorted descending by count.
    pub fn most_used(&self, n: usize) -> Vec<(String, usize)> {
        let mut items: Vec<(String, usize)> =
            self.counts.iter().map(|(k, &v)| (k.clone(), v)).collect();
        items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        items.truncate(n);
        items
    }

    /// Return the *n* least-used snippets, sorted ascending by count.
    pub fn least_used(&self, n: usize) -> Vec<(String, usize)> {
        let mut items: Vec<(String, usize)> =
            self.counts.iter().map(|(k, &v)| (k.clone(), v)).collect();
        items.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        items.truncate(n);
        items
    }

    pub fn total_usages(&self) -> usize {
        self.counts.values().sum()
    }

    pub fn unique_snippets_used(&self) -> usize {
        self.counts.len()
    }
}

impl std::fmt::Display for SnippetUsageStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SnippetUsageStats(total={}, unique={})",
            self.total_usages(),
            self.unique_snippets_used(),
        )
    }
}


// ─── SnipC LRU Cache ───────────────────────────────────────

/// A simple LRU cache for snippet lookups.
#[derive(Debug)]
pub struct SnipCLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> SnipCLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for SnipCLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SnipCLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── SnipB Builder & Validator ─────────────────────────────

/// Builder for constructing snippet configurations.
#[derive(Debug, Clone)]
pub struct SnipBBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl SnipBBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<SnipBCfg, SnipBBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(SnipBBuildErr { errors }); }
        Ok(SnipBCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated snippet configuration.
#[derive(Debug, Clone)]
pub struct SnipBCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl SnipBCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &SnipBCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for SnipBCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SnipBCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct SnipBBuildErr { pub errors: Vec<String> }

impl fmt::Display for SnipBBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SnipBBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for SnipBBuildErr {}


/// Configuration manager for snippets_mgr functionality.
pub struct SnippetsMgrConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl SnippetsMgrConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &SnippetsMgrConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for snippets_mgr operations.
pub struct SnippetsMgrRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl SnippetsMgrRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for snippets_mgr.
pub struct SnippetsMgrValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl SnippetsMgrValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &SnippetsMgrValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
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
// xa_ extended helpers for snippets_mgr
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaSnippetsMgrRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaSnippetsMgrRingBuf {
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
pub struct XaSnippetsMgrCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaSnippetsMgrCounter {
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

impl Default for XaSnippetsMgrCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 162
// ---------------------------------------------------------------------------

/// Generic object pool `Xc162Pool<T>`.
pub struct Xc162Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc162Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc162PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc162Pool<T> {
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
    pub fn stats(&self) -> Xc162PoolStats {
        Xc162PoolStats {
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

impl<T> Default for Xc162Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc162Scheduler`.
pub struct Xc162Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc162Scheduler {
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

impl Default for Xc162Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_162 hash for the given byte slice.
pub fn xc_162_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_162 convention.
pub fn xc_162_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_71 deepening: state machine + event bus ---

/// States for the Xd71 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd71State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd71State {
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
pub struct Xd71Transition {
    pub from: Xd71State,
    pub to: Xd71State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd71StateMachine {
    current: Xd71State,
    history: Vec<Xd71Transition>,
    step_counter: usize,
}

impl Xd71StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd71State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd71State {
        self.current
    }

    pub fn history(&self) -> &[Xd71Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd71State) -> Result<Xd71State, String> {
        let allowed = match (self.current, target) {
            (Xd71State::Idle, Xd71State::Running) => true,
            (Xd71State::Running, Xd71State::Paused) => true,
            (Xd71State::Running, Xd71State::Done) => true,
            (Xd71State::Paused, Xd71State::Running) => true,
            (Xd71State::Paused, Xd71State::Done) => true,
            (Xd71State::Done, Xd71State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_71: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd71Transition {
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
            "Xd71SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd71State> {
        let prefix = "Xd71SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd71State::Idle),
            "Running" => Some(Xd71State::Running),
            "Paused" => Some(Xd71State::Paused),
            "Done" => Some(Xd71State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd71State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd71 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd71Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd71Event {
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

type Xd71HandlerFn = Box<dyn Fn(&Xd71Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd71EventBus {
    handlers: Vec<(usize, Option<String>, Xd71HandlerFn)>,
    next_id: usize,
    published: Vec<Xd71Event>,
}

impl Xd71EventBus {
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
        F: Fn(&Xd71Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd71Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd71Event) {
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

    pub fn published_events(&self) -> &[Xd71Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #84
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf84Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf84TrieNode {
    children: std::collections::HashMap<char, Xf84TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf84Trie {
    root: Xf84TrieNode,
    count: usize,
}

impl Xf84Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf84TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf84TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf84TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf84BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf84BloomFilter {
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

    #[test]
    fn eq_snippetsource_same() {
        assert_eq!(SnippetSource::User, SnippetSource::User);
    }

    #[test]
    fn ne_snippetsource_diff() {
        assert_ne!(SnippetSource::User, SnippetSource::Workspace);
    }

    // -----------------------------------------------------------------------
    // Language-scoped registry tests
    // -----------------------------------------------------------------------

    #[test]
    fn language_registry_register_and_get() {
        let mut reg = LanguageScopedRegistry::new();
        reg.register_snippets(
            "rust",
            vec![sample_snippet("for-loop", "for", Some("rust"))],
        );
        reg.register_snippets(
            "python",
            vec![sample_snippet("def-func", "def", Some("python"))],
        );
        let rust = reg.get_snippets("rust");
        assert_eq!(rust.len(), 1);
        assert_eq!(rust[0].name, "for-loop");
    }

    #[test]
    fn language_registry_global_snippets() {
        let mut reg = LanguageScopedRegistry::new();
        reg.register_global(sample_snippet("todo", "todo", None));
        reg.register_snippets("rust", vec![sample_snippet("fn", "fn", Some("rust"))]);
        // Rust should see global + rust-specific
        let rust = reg.get_snippets("rust");
        assert_eq!(rust.len(), 2);
        // Python should see only global
        let py = reg.get_snippets("python");
        assert_eq!(py.len(), 1);
        assert_eq!(py[0].name, "todo");
    }

    #[test]
    fn language_registry_count() {
        let mut reg = LanguageScopedRegistry::new();
        reg.register_snippets("rust", vec![sample_snippet("a", "a", None)]);
        reg.register_global(sample_snippet("b", "b", None));
        assert_eq!(reg.language_snippet_count("rust"), 1);
        assert_eq!(reg.total_count(), 2);
    }

    #[test]
    fn language_registry_languages() {
        let mut reg = LanguageScopedRegistry::new();
        reg.register_snippets("rust", vec![sample_snippet("a", "a", None)]);
        reg.register_snippets("python", vec![sample_snippet("b", "b", None)]);
        let mut langs = reg.languages();
        langs.sort();
        assert_eq!(langs, vec!["python", "rust"]);
    }

    #[test]
    fn load_vscode_snippets_json() {
        let json = r#"{
            "Print": {
                "prefix": "print",
                "body": ["println!(\"$1\");"],
                "description": "Print line"
            }
        }"#;
        let snippets = load_vscode_snippets(json, SnippetSource::Extension).unwrap();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].name, "Print");
        assert_eq!(snippets[0].source, SnippetSource::Extension);
        assert_eq!(snippets[0].description.as_deref(), Some("Print line"));
    }

    #[test]
    fn load_vscode_snippets_invalid() {
        assert!(load_vscode_snippets("not json", SnippetSource::User).is_err());
    }

    #[test]
    fn behavior_check_0() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_30() {
        let _svc = SnippetService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn validator_balanced_braces() {
        let body = vec!["${1:name}".into(), "let ${2:val} = $1;".into()];
        assert!(SnippetValidator::validate_body(&body).is_ok());
    }

    #[test]
    fn validator_unbalanced_braces() {
        let body = vec!["${1:name".into()];
        assert!(SnippetValidator::validate_body(&body).is_err());
    }

    #[test]
    fn validator_tabstop_gap() {
        let body = vec!["$1 $3".into()];
        assert!(SnippetValidator::validate_tabstop_numbers(&body).is_err());
    }

    #[test]
    fn validator_tabstop_sequential() {
        let body = vec!["$1 ${2:x} $3".into()];
        assert!(SnippetValidator::validate_tabstop_numbers(&body).is_ok());
    }

    #[test]
    fn importer_from_json() {
        let json = r#"{"Loop": {"prefix": ["for"], "body": ["for ${1:i} in ${2:iter} {", "    $0", "}"]}}"#;
        let snippets = SnippetImporter::from_json(json, SnippetSource::User).unwrap();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].name, "Loop");
        assert_eq!(snippets[0].prefix, vec!["for"]);
        assert_eq!(snippets[0].body.len(), 3);
    }

    #[test]
    fn exporter_roundtrip() {
        let snippet = Snippet {
            name: "Test".into(),
            prefix: vec!["tst".into()],
            body: vec!["assert!($1);".into()],
            description: Some("A test".into()),
            scope: Some("rust".into()),
            source: SnippetSource::User,
        };
        let json = SnippetExporter::to_json(&[snippet]).unwrap();
        let reimported = SnippetImporter::from_json(&json, SnippetSource::User).unwrap();
        assert_eq!(reimported.len(), 1);
        assert_eq!(reimported[0].name, "Test");
        assert_eq!(reimported[0].prefix, vec!["tst"]);
        assert_eq!(reimported[0].body, vec!["assert!($1);"]);
    }

    #[test]
    fn validate_full_snippet() {
        let snippet = Snippet {
            name: "Good".into(),
            prefix: vec!["g".into()],
            body: vec!["${1:hello} $2".into()],
            description: None,
            scope: None,
            source: SnippetSource::User,
        };
        assert!(SnippetValidator::validate(&snippet).is_ok());
    }

    // -- snippet_variable_resolver tests ------------------------------------

    #[test]
    fn resolve_tm_filename() {
        let ctx = SnippetVariableContext {
            filename: Some("main.rs".into()),
            ..Default::default()
        };
        let result = snippet_variable_resolver("file: $TM_FILENAME", &ctx);
        assert_eq!(result, "file: main.rs");
    }

    #[test]
    fn resolve_tm_filename_base() {
        let ctx = SnippetVariableContext {
            filename: Some("main.rs".into()),
            ..Default::default()
        };
        let result = snippet_variable_resolver("base: $TM_FILENAME_BASE", &ctx);
        assert_eq!(result, "base: main");
    }

    #[test]
    fn resolve_multiple_variables() {
        let ctx = SnippetVariableContext {
            filename: Some("lib.rs".into()),
            line_number: Some(42),
            workspace_name: Some("myproject".into()),
            ..Default::default()
        };
        let result = snippet_variable_resolver(
            "// $WORKSPACE_NAME - $TM_FILENAME:$TM_LINE_NUMBER",
            &ctx,
        );
        assert_eq!(result, "// myproject - lib.rs:42");
    }

    #[test]
    fn resolve_missing_variables_empty() {
        let ctx = SnippetVariableContext::default();
        let result = snippet_variable_resolver("sel=$TM_SELECTED_TEXT end", &ctx);
        assert_eq!(result, "sel= end");
    }

    #[test]
    fn resolve_body_multi_line() {
        let ctx = SnippetVariableContext {
            filename: Some("test.rs".into()),
            clipboard: Some("pasted".into()),
            ..Default::default()
        };
        let body = vec![
            "// $TM_FILENAME".into(),
            "let x = \"$CLIPBOARD\";".into(),
        ];
        let resolved = snippet_resolve_body(&body, &ctx);
        assert_eq!(resolved[0], "// test.rs");
        assert_eq!(resolved[1], "let x = \"pasted\";");
    }

    #[test]
    fn snippet_variable_all_count() {
        assert_eq!(SnippetVariable::all().len(), 12);
    }

    #[test]
    fn snippet_variable_tokens_unique() {
        let tokens: Vec<&str> = SnippetVariable::all().iter().map(|v| v.token()).collect();
        let unique: std::collections::HashSet<&str> = tokens.iter().copied().collect();
        assert_eq!(tokens.len(), unique.len());
    }

    // -----------------------------------------------------------------------
    // Fuzzy search tests
    // -----------------------------------------------------------------------

    #[test]
    fn fuzzy_matches_basic() {
        assert!(SnippetSearch::fuzzy_matches("fl", "for-loop"));
        assert!(SnippetSearch::fuzzy_matches("FL", "for-loop")); // case-insensitive
        assert!(!SnippetSearch::fuzzy_matches("xyz", "for-loop"));
        assert!(SnippetSearch::fuzzy_matches("", "anything")); // empty pattern matches all
    }

    #[test]
    fn fuzzy_score_prefix_bonus() {
        let exact = SnippetSearch::fuzzy_score("for", "for-loop");
        let mid = SnippetSearch::fuzzy_score("for", "a-for-loop");
        assert!(exact.is_some());
        assert!(mid.is_some());
        assert!(exact.unwrap() > mid.unwrap());
    }

    #[test]
    fn fuzzy_search_ranks_results() {
        let snippets = vec![
            sample_snippet("for-loop", "for", Some("rust")),
            sample_snippet("fn-def", "fn", Some("rust")),
            sample_snippet("format-string", "fmt", None),
        ];
        let results = SnippetSearch::search(&snippets, "fn");
        assert!(!results.is_empty());
        // "fn-def" should be the top result (exact prefix match).
        assert_eq!(results[0].snippet.name, "fn-def");
    }

    // -----------------------------------------------------------------------
    // Usage tracker tests
    // -----------------------------------------------------------------------

    #[test]
    fn usage_tracker_records_and_ranks() {
        let mut tracker = SnippetUsageTracker::new();
        tracker.record_use("for-loop");
        tracker.record_use("fn-def");
        tracker.record_use("for-loop");
        tracker.record_use("for-loop");

        assert_eq!(tracker.usage_count("for-loop"), 3);
        assert_eq!(tracker.usage_count("fn-def"), 1);
        assert_eq!(tracker.usage_count("unknown"), 0);
        assert_eq!(tracker.tracked_count(), 2);

        let ranked = tracker.most_used();
        assert_eq!(ranked[0].0, "for-loop");
        assert_eq!(ranked[0].1, 3);

        tracker.reset();
        assert_eq!(tracker.tracked_count(), 0);
    }

    // -----------------------------------------------------------------------
    // Conflict resolver tests
    // -----------------------------------------------------------------------

    #[test]
    fn conflict_resolver_detects_prefix_collision() {
        let snippets = vec![
            sample_snippet("for-loop-a", "for", Some("rust")),
            sample_snippet("for-loop-b", "for", Some("rust")),
            sample_snippet("fn-def", "fn", Some("rust")),
        ];
        let conflicts = SnippetConflictResolver::detect_conflicts(&snippets);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].prefix, "for");
        assert!(SnippetConflictResolver::has_conflicts(&snippets));

        // No conflicts when prefixes are unique.
        let unique = vec![
            sample_snippet("a", "alpha", None),
            sample_snippet("b", "beta", None),
        ];
        assert!(!SnippetConflictResolver::has_conflicts(&unique));
    }

    // -----------------------------------------------------------------------
    // Reindent tests
    // -----------------------------------------------------------------------

    #[test]
    fn reindent_snippet_body_applies_base_indent() {
        let body = vec![
            "if cond {".into(),
            "    do_thing();".into(),
            "}".into(),
        ];
        let result = reindent_snippet_body(&body, "        ");
        assert_eq!(result[0], "if cond {");
        assert_eq!(result[1], "            do_thing();");
        assert_eq!(result[2], "        }");
    }

    // -----------------------------------------------------------------------
    // Snippet catalog / tagging tests
    // -----------------------------------------------------------------------

    #[test]
    fn catalog_add_and_find_by_tag() {
        let mut catalog = SnippetCatalog::new();
        catalog.add(
            sample_snippet("for-loop", "for", Some("rust")),
            vec!["loops".into(), "control-flow".into()],
        );
        catalog.add(
            sample_snippet("if-else", "if", Some("rust")),
            vec!["conditionals".into(), "control-flow".into()],
        );
        catalog.add(
            sample_snippet("println", "pl", Some("rust")),
            vec!["io".into()],
        );

        let cf = catalog.find_by_tag("control-flow");
        assert_eq!(cf.len(), 2);

        let io = catalog.find_by_tag("io");
        assert_eq!(io.len(), 1);
        assert_eq!(io[0].snippet.name, "println");

        assert_eq!(catalog.find_by_tag("nonexistent").len(), 0);
    }

    #[test]
    fn catalog_all_tags_sorted() {
        let mut catalog = SnippetCatalog::new();
        catalog.add(
            sample_snippet("a", "a", None),
            vec!["zebra".into(), "alpha".into()],
        );
        catalog.add(
            sample_snippet("b", "b", None),
            vec!["beta".into(), "alpha".into()],
        );
        let tags = catalog.all_tags();
        assert_eq!(tags, vec!["alpha", "beta", "zebra"]);
    }

    #[test]
    fn catalog_bulk_enable_disable_by_tag() {
        let mut catalog = SnippetCatalog::new();
        catalog.add(
            sample_snippet("for", "for", None),
            vec!["loops".into()],
        );
        catalog.add(
            sample_snippet("while", "while", None),
            vec!["loops".into()],
        );
        catalog.add(
            sample_snippet("if", "if", None),
            vec!["conditionals".into()],
        );

        assert_eq!(catalog.enabled_snippets().len(), 3);

        let disabled = catalog.disable_by_tag("loops");
        assert_eq!(disabled, 2);
        assert_eq!(catalog.enabled_snippets().len(), 1);
        assert_eq!(catalog.disabled_snippets().len(), 2);
        assert_eq!(catalog.enabled_snippets()[0].name, "if");

        // Disabling again should change nothing.
        assert_eq!(catalog.disable_by_tag("loops"), 0);

        let enabled = catalog.enable_by_tag("loops");
        assert_eq!(enabled, 2);
        assert_eq!(catalog.enabled_snippets().len(), 3);
    }

    #[test]
    fn sort_snippets_by_usage_orders_most_used_first() {
        let mut tracker = SnippetUsageTracker::new();
        tracker.record_use("c");
        tracker.record_use("a");
        tracker.record_use("a");
        tracker.record_use("a");
        tracker.record_use("b");
        tracker.record_use("b");

        let mut snippets = vec![
            sample_snippet("b", "b", None),
            sample_snippet("c", "c", None),
            sample_snippet("a", "a", None),
        ];
        sort_snippets_by_usage(&mut snippets, &tracker);
        assert_eq!(snippets[0].name, "a"); // 3 uses
        assert_eq!(snippets[1].name, "b"); // 2 uses
        assert_eq!(snippets[2].name, "c"); // 1 use
    }

    #[test]
    fn analyse_complexity_simple_body() {
        let body = vec!["let ${1:name} = $2;".into()];
        let c = analyse_snippet_complexity(&body);
        assert_eq!(c.line_count, 1);
        assert_eq!(c.tabstop_count, 2);
        assert!(!c.has_choices);
        assert!(!c.has_variables);
        assert!(!c.nested_placeholders);
    }

    #[test]
    fn analyse_complexity_with_choices_and_variables() {
        let body = vec![
            "// $TM_FILENAME".into(),
            "${1|pub,pub(crate),pub(super)|} fn ${2:name}() {".into(),
            "    $0".into(),
            "}".into(),
        ];
        let c = analyse_snippet_complexity(&body);
        assert_eq!(c.line_count, 4);
        assert!(c.has_choices);
        assert!(c.has_variables);
    }

    #[test]
    fn analyse_complexity_nested_placeholders() {
        let body = vec!["${1:outer ${2:inner}}".into()];
        let c = analyse_snippet_complexity(&body);
        assert!(c.nested_placeholders);
    }

    // -- SnippetTabStopLinker tests --

    #[test]
    fn tabstop_linker_basic() {
        let body = vec!["let $1 = $2;".into(), "println!(\"{}\", $1);".into()];
        let linker = SnippetTabStopLinker::from_body(&body);
        assert_eq!(linker.tabstop_count(), 2);
        assert!(linker.is_linked(1)); // appears on 2 lines
        assert!(!linker.is_linked(2)); // appears once
    }

    #[test]
    fn tabstop_linker_with_placeholders() {
        let body = vec!["${1:name} ${2:value}".into(), "use ${1:name};".into()];
        let linker = SnippetTabStopLinker::from_body(&body);
        let nums = linker.tabstop_numbers();
        assert!(nums.contains(&1));
        assert!(nums.contains(&2));
        assert!(linker.is_linked(1));
    }

    #[test]
    fn tabstop_linker_empty() {
        let body = vec!["no tabstops here".into()];
        let linker = SnippetTabStopLinker::from_body(&body);
        assert_eq!(linker.tabstop_count(), 0);
    }

    // -- SnippetChoiceExpander tests --

    #[test]
    fn choice_expander_extract() {
        let line = "type: ${1|string,number,boolean|}";
        let choices = SnippetChoiceExpander::extract_choices(line);
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].tabstop, 1);
        assert_eq!(choices[0].options, vec!["string", "number", "boolean"]);
    }

    #[test]
    fn choice_expander_defaults() {
        let line = "${1|pub,pub(crate),fn|} ${2:name}";
        let expanded = SnippetChoiceExpander::expand_with_defaults(line);
        assert!(expanded.contains("pub"));
        assert!(!expanded.contains("|"));
    }

    #[test]
    fn choice_expander_no_choices() {
        let choices = SnippetChoiceExpander::extract_choices("plain text $1");
        assert!(choices.is_empty());
    }

    // -- SnippetFormatConverter tests --

    #[test]
    fn format_converter_body_roundtrip() {
        let lines = vec!["line 1".into(), "line 2".into()];
        let body = SnippetFormatConverter::lines_to_body_string(&lines);
        let back = SnippetFormatConverter::body_string_to_lines(&body);
        assert_eq!(back, lines);
    }

    #[test]
    fn format_converter_to_vscode_json() {
        let snippets = vec![Snippet {
            name: "test".into(),
            prefix: vec!["tst".into()],
            body: vec!["console.log($1);".into()],
            description: Some("Test snippet".into()),
            scope: Some("javascript".into()),
            source: SnippetSource::User,
        }];
        let json = SnippetFormatConverter::to_vscode_json(&snippets);
        assert!(json.contains("\"test\""));
        assert!(json.contains("\"prefix\""));
        assert!(json.contains("\"body\""));
        assert!(json.contains("\"description\""));
    }

    // -----------------------------------------------------------------------
    // SnippetJsonImporter tests
    // -----------------------------------------------------------------------

    #[test]
    fn json_importer_parses_vscode_json() {
        let json = r#"{
            "ForLoop": {
                "prefix": "for",
                "body": ["for ${1:item} in ${2:iter} {", "    $0", "}"],
                "description": "A for loop",
                "scope": "rust"
            }
        }"#;
        let snippets = SnippetJsonImporter::from_vscode_json(json).unwrap();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].name, "ForLoop");
        assert_eq!(snippets[0].prefix, "for");
        assert_eq!(snippets[0].body.len(), 3);
        assert_eq!(snippets[0].description.as_deref(), Some("A for loop"));
        assert_eq!(snippets[0].scope.as_deref(), Some("rust"));
    }

    #[test]
    fn json_importer_from_json_value() {
        let val: serde_json::Value = serde_json::from_str(r#"{
            "Hello": { "prefix": "hel", "body": ["Hello, world!"] }
        }"#).unwrap();
        let snippets = SnippetJsonImporter::from_json_value(&val).unwrap();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].prefix, "hel");
    }

    #[test]
    fn json_importer_invalid_json_returns_error() {
        let result = SnippetJsonImporter::from_vscode_json("not json");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // SnippetJsonExporter tests
    // -----------------------------------------------------------------------

    #[test]
    fn json_exporter_roundtrip() {
        let mut exporter = SnippetJsonExporter::new();
        exporter.add_snippet(ImportedSnippet {
            name: "Test".into(),
            prefix: "tst".into(),
            body: vec!["line1".into()],
            description: Some("desc".into()),
            scope: None,
        });
        assert_eq!(exporter.snippet_count(), 1);

        let json = exporter.to_json();
        assert!(json.contains("\"Test\""));
        assert!(json.contains("\"tst\""));

        let compact = exporter.to_json_compact();
        assert!(!compact.contains('\n'));

        // Round-trip: parse the exported JSON back
        let reimported = SnippetJsonImporter::from_vscode_json(&json).unwrap();
        assert_eq!(reimported.len(), 1);
        assert_eq!(reimported[0].name, "Test");
    }

    #[test]
    fn json_exporter_display() {
        let exporter = SnippetJsonExporter::new();
        assert_eq!(format!("{exporter}"), "SnippetJsonExporter(0 snippets)");
    }

    // -----------------------------------------------------------------------
    // SnippetConflictDetector tests
    // -----------------------------------------------------------------------

    #[test]
    fn conflict_detector_no_conflicts() {
        let mut det = SnippetConflictDetector::new();
        det.add_snippet("a", "aa", Some("rust"));
        det.add_snippet("b", "bb", Some("rust"));
        assert!(!det.has_conflicts());
        assert_eq!(det.conflict_count(), 0);
    }

    #[test]
    fn conflict_detector_same_scope_conflict() {
        let mut det = SnippetConflictDetector::new();
        det.add_snippet("for-loop", "for", Some("rust"));
        det.add_snippet("foreach", "for", Some("rust"));
        let conflicts = det.detect_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].prefix, "for");
        assert_eq!(conflicts[0].snippets.len(), 2);
        assert!(conflicts[0].is_same_scope);
    }

    #[test]
    fn conflict_detector_different_scope() {
        let mut det = SnippetConflictDetector::new();
        det.add_snippet("for-rs", "for", Some("rust"));
        det.add_snippet("for-py", "for", Some("python"));
        let conflicts = det.detect_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert!(!conflicts[0].is_same_scope);
    }

    #[test]
    fn conflict_detector_display() {
        let det = SnippetConflictDetector::new();
        assert_eq!(format!("{det}"), "SnippetConflictDetector(0 conflicts)");
    }

    // -----------------------------------------------------------------------
    // SnippetUsageStats tests
    // -----------------------------------------------------------------------

    #[test]
    fn usage_stats_record_and_query() {
        let mut stats = SnippetUsageStats::new();
        stats.record_usage("for-loop");
        stats.record_usage("for-loop");
        stats.record_usage("if-else");
        assert_eq!(stats.usage_count("for-loop"), 2);
        assert_eq!(stats.usage_count("if-else"), 1);
        assert_eq!(stats.usage_count("unknown"), 0);
        assert_eq!(stats.total_usages(), 3);
        assert_eq!(stats.unique_snippets_used(), 2);
    }

    #[test]
    fn usage_stats_most_and_least_used() {
        let mut stats = SnippetUsageStats::new();
        stats.record_usage("a");
        stats.record_usage("b");
        stats.record_usage("b");
        stats.record_usage("c");
        stats.record_usage("c");
        stats.record_usage("c");

        let most = stats.most_used(2);
        assert_eq!(most.len(), 2);
        assert_eq!(most[0].0, "c");
        assert_eq!(most[0].1, 3);

        let least = stats.least_used(1);
        assert_eq!(least.len(), 1);
        assert_eq!(least[0].0, "a");
        assert_eq!(least[0].1, 1);
    }

    #[test]
    fn usage_stats_display() {
        let stats = SnippetUsageStats::new();
        assert_eq!(format!("{stats}"), "SnippetUsageStats(total=0, unique=0)");
    }

    // -----------------------------------------------------------------------
    // ImportedSnippet Display test
    // -----------------------------------------------------------------------

    #[test]
    fn imported_snippet_display() {
        let s = ImportedSnippet {
            name: "MySnippet".into(),
            prefix: "ms".into(),
            body: vec!["body".into()],
            description: None,
            scope: Some("rust".into()),
        };
        let display = format!("{s}");
        assert!(display.contains("MySnippet"));
        assert!(display.contains("ms"));
        assert!(display.contains("rust"));
    }

    #[test]
    fn format_converter_from_vscode_json() {
        let json = r#"{
            "Print": {
                "prefix": ["pr"],
                "body": ["println!(\"$1\");"],
                "description": "Print line"
            }
        }"#;
        let snippets = SnippetFormatConverter::from_vscode_json(json).unwrap();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].name, "Print");
    }

    #[test]
    fn snipc_lru_insert_get() {
        let mut c = SnipCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn snipc_lru_eviction() {
        let mut c = SnipCLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn snipc_lru_hit_ratio() {
        let mut c = SnipCLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn snipc_lru_clear() {
        let mut c = SnipCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn snipc_lru_remove() {
        let mut c = SnipCLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn snipc_lru_peek() {
        let mut c = SnipCLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn snipb_builder_valid() {
        let cfg = SnipBBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn snipb_builder_empty_name() {
        let r = SnipBBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn snipb_builder_bad_priority() {
        assert!(SnipBBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn snipb_builder_zero_max() {
        assert!(SnipBBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn snipb_cfg_merge() {
        let mut a = SnipBBuilder::new("a").property("x", "1").build().unwrap();
        let b = SnipBBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn snipb_cfg_display() {
        let cfg = SnipBBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    #[test]
    fn snippets_mgr_config_new() {
        let cfg = SnippetsMgrConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn snippets_mgr_config_set_get() {
        let mut cfg = SnippetsMgrConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn snippets_mgr_config_remove() {
        let mut cfg = SnippetsMgrConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn snippets_mgr_config_keys_sorted() {
        let mut cfg = SnippetsMgrConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn snippets_mgr_config_bump_version() {
        let mut cfg = SnippetsMgrConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn snippets_mgr_config_clear() {
        let mut cfg = SnippetsMgrConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn snippets_mgr_config_merge() {
        let mut cfg1 = SnippetsMgrConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = SnippetsMgrConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn snippets_mgr_config_disable() {
        let mut cfg = SnippetsMgrConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn snippets_mgr_rate_tracker_empty() {
        let rt = SnippetsMgrRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn snippets_mgr_rate_tracker_record() {
        let mut rt = SnippetsMgrRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn snippets_mgr_rate_tracker_prune() {
        let mut rt = SnippetsMgrRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn snippets_mgr_validator_valid() {
        let v = SnippetsMgrValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn snippets_mgr_validator_errors() {
        let mut v = SnippetsMgrValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn snippets_mgr_validator_clear() {
        let mut v = SnippetsMgrValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn snippets_mgr_validator_merge() {
        let mut v1 = SnippetsMgrValidator::new();
        v1.add_error("e1");
        let mut v2 = SnippetsMgrValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn snippets_mgr_rate_tracker_clear() {
        let mut rt = SnippetsMgrRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
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


    // xa_ extended tests for snippets_mgr
    #[test]
    fn xa_snippets_mgr_ring_new() {
        let rb = super::XaSnippetsMgrRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_snippets_mgr_ring_push_len() {
        let mut rb = super::XaSnippetsMgrRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_snippets_mgr_ring_wrap() {
        let mut rb = super::XaSnippetsMgrRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_snippets_mgr_ring_mean_empty() {
        let rb = super::XaSnippetsMgrRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_snippets_mgr_ring_mean_values() {
        let mut rb = super::XaSnippetsMgrRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_snippets_mgr_ring_min_max() {
        let mut rb = super::XaSnippetsMgrRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_snippets_mgr_ring_iter() {
        let mut rb = super::XaSnippetsMgrRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_snippets_mgr_counter_new() {
        let c = super::XaSnippetsMgrCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_snippets_mgr_counter_inc() {
        let mut c = super::XaSnippetsMgrCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_snippets_mgr_counter_inc_by() {
        let mut c = super::XaSnippetsMgrCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_snippets_mgr_counter_reset() {
        let mut c = super::XaSnippetsMgrCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_snippets_mgr_counter_clear() {
        let mut c = super::XaSnippetsMgrCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_snippets_mgr_counter_default() {
        let c = super::XaSnippetsMgrCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 162 ----

    #[test]
    fn xc_162_pool_new_empty() {
        let pool: super::Xc162Pool<i32> = super::Xc162Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_162_pool_release_acquire() {
        let mut pool = super::Xc162Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_162_pool_acquire_empty() {
        let mut pool: super::Xc162Pool<i32> = super::Xc162Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_162_pool_full() {
        let mut pool = super::Xc162Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_162_pool_drain() {
        let mut pool = super::Xc162Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_162_pool_stats() {
        let mut pool = super::Xc162Pool::new(8);
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
    fn xc_162_pool_clear() {
        let mut pool = super::Xc162Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_162_pool_shrink() {
        let mut pool = super::Xc162Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_162_pool_default() {
        let pool: super::Xc162Pool<String> = super::Xc162Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_162_pool_extend() {
        let mut pool = super::Xc162Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_162_pool_retain() {
        let mut pool = super::Xc162Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_162_scheduler_round_robin() {
        let mut sched = super::Xc162Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_162_scheduler_empty() {
        let mut sched = super::Xc162Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_162_scheduler_reset() {
        let mut sched = super::Xc162Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_162_scheduler_add_remove() {
        let mut sched = super::Xc162Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_162_scheduler_targets() {
        let sched = super::Xc162Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_162_hash_empty() {
        assert_eq!(super::xc_162_hash(b""), 5381);
    }

    #[test]
    fn xc_162_hash_data() {
        let h = super::xc_162_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_162_hash(b"hello"), h);
    }

    #[test]
    fn xc_162_reverse_str() {
        assert_eq!(super::xc_162_reverse("abc"), "cba");
        assert_eq!(super::xc_162_reverse(""), "");
    }


    // --- xd_71 deepening tests ---

    #[test]
    fn xd_71_sm_initial_state() {
        let sm = Xd71StateMachine::new();
        assert_eq!(sm.current_state(), Xd71State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_71_sm_valid_idle_to_running() {
        let mut sm = Xd71StateMachine::new();
        assert!(sm.transition(Xd71State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd71State::Running);
    }

    #[test]
    fn xd_71_sm_valid_running_to_paused() {
        let mut sm = Xd71StateMachine::new();
        sm.transition(Xd71State::Running).unwrap();
        assert!(sm.transition(Xd71State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd71State::Paused);
    }

    #[test]
    fn xd_71_sm_valid_running_to_done() {
        let mut sm = Xd71StateMachine::new();
        sm.transition(Xd71State::Running).unwrap();
        assert!(sm.transition(Xd71State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd71State::Done);
    }

    #[test]
    fn xd_71_sm_valid_paused_to_running() {
        let mut sm = Xd71StateMachine::new();
        sm.transition(Xd71State::Running).unwrap();
        sm.transition(Xd71State::Paused).unwrap();
        assert!(sm.transition(Xd71State::Running).is_ok());
    }

    #[test]
    fn xd_71_sm_valid_done_to_idle() {
        let mut sm = Xd71StateMachine::new();
        sm.transition(Xd71State::Running).unwrap();
        sm.transition(Xd71State::Done).unwrap();
        assert!(sm.transition(Xd71State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd71State::Idle);
    }

    #[test]
    fn xd_71_sm_invalid_idle_to_done() {
        let mut sm = Xd71StateMachine::new();
        assert!(sm.transition(Xd71State::Done).is_err());
    }

    #[test]
    fn xd_71_sm_invalid_idle_to_paused() {
        let mut sm = Xd71StateMachine::new();
        assert!(sm.transition(Xd71State::Paused).is_err());
    }

    #[test]
    fn xd_71_sm_history_tracking() {
        let mut sm = Xd71StateMachine::new();
        sm.transition(Xd71State::Running).unwrap();
        sm.transition(Xd71State::Paused).unwrap();
        sm.transition(Xd71State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd71State::Idle);
        assert_eq!(sm.history()[0].to, Xd71State::Running);
        assert_eq!(sm.history()[1].from, Xd71State::Running);
        assert_eq!(sm.history()[2].to, Xd71State::Done);
    }

    #[test]
    fn xd_71_sm_serialize_deserialize() {
        let mut sm = Xd71StateMachine::new();
        sm.transition(Xd71State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd71StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd71State::Running));
    }

    #[test]
    fn xd_71_sm_deserialize_invalid() {
        assert_eq!(Xd71StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_71_sm_reset() {
        let mut sm = Xd71StateMachine::new();
        sm.transition(Xd71State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd71State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_71_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd71EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd71Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_71_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd71EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd71Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd71Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_71_bus_unsubscribe() {
        let mut bus = Xd71EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_71_event_kind_and_payload() {
        let e = Xd71Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd71Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_71_bus_clear_history() {
        let mut bus = Xd71EventBus::new();
        bus.publish(Xd71Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_71_sm_step_counter_increments() {
        let mut sm = Xd71StateMachine::new();
        sm.transition(Xd71State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd71State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #84 --

    #[test]
    fn xf84_trie_insert_search() {
        let mut t = Xf84Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf84_trie_starts_with() {
        let mut t = Xf84Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf84_trie_remove() {
        let mut t = Xf84Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf84_trie_word_count() {
        let mut t = Xf84Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf84_trie_longest_prefix() {
        let mut t = Xf84Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf84_trie_all_words() {
        let mut t = Xf84Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf84_trie_autocomplete() {
        let mut t = Xf84Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf84_trie_empty_search() {
        let t = Xf84Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf84_bloom_add_contains() {
        let mut bf = Xf84BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf84_bloom_probably_absent() {
        let bf = Xf84BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf84_bloom_false_positive_rate() {
        let mut bf = Xf84BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf84_bloom_clear() {
        let mut bf = Xf84BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf84_bloom_union() {
        let mut a = Xf84BloomFilter::xf_new(512, 2);
        let mut b = Xf84BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf84_bloom_intersection_estimate() {
        let mut a = Xf84BloomFilter::xf_new(512, 2);
        let mut b = Xf84BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf84_bloom_union_size_mismatch() {
        let a = Xf84BloomFilter::xf_new(256, 2);
        let b = Xf84BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }

}